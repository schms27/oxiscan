//! `oxiscan mcp`: a Model Context Protocol server over stdio, so AI assistants can scan
//! the local network with the same engine as the app. No window is opened.
//!
//! The server only reads: it never changes the saved settings and offers none of the
//! app's "open" actions. Unless started with `--allow-public` it refuses addresses
//! outside private, link-local, loopback and carrier-grade NAT ranges.

use crate::config::{self, Config, FeederState};
use crate::scanner::{self, Feeder, ScanHandle, ScanMsg, ScanResult, Settings};
use crate::services::{self, DeviceKind};
use rmcp::handler::server::{router::tool::ToolRouter, wrapper::Parameters};
use rmcp::model::{
    Implementation, ProgressNotificationParam, RequestMetaObject, ServerCapabilities, ServerConfig,
};
use rmcp::{tool, tool_handler, tool_router, Json, Peer, RoleServer, ServerHandler, ServiceExt};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const DEFAULT_MAX_HOSTS: u64 = 4096;

/// Command-line options after `oxiscan mcp`.
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub allow_public: bool,
    pub max_hosts: u64,
}

impl Options {
    pub fn parse(args: &[String]) -> Result<Options, String> {
        let mut o = Options {
            allow_public: false,
            max_hosts: DEFAULT_MAX_HOSTS,
        };
        let mut it = args.iter();
        while let Some(a) = it.next() {
            if a == "--allow-public" {
                o.allow_public = true;
            } else if let Some(v) = a.strip_prefix("--max-hosts") {
                let v = match v.strip_prefix('=') {
                    Some(v) => v.to_string(),
                    None if v.is_empty() => it.next().cloned().unwrap_or_default(),
                    None => return Err(format!("Unknown option {a}")),
                };
                o.max_hosts = v
                    .parse()
                    .ok()
                    .filter(|n| *n > 0)
                    .ok_or(format!("--max-hosts needs a positive number, not \"{v}\""))?;
            } else {
                return Err(format!(
                    "Unknown option {a}\nUsage: oxiscan mcp [--allow-public] [--max-hosts N]"
                ));
            }
        }
        Ok(o)
    }
}

/// Runs the server until the client disconnects. Returns the process exit code.
pub fn run(args: &[String]) -> i32 {
    let options = match Options::parse(args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            return 2;
        }
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Cannot start the async runtime: {e}");
            return 1;
        }
    };
    runtime.block_on(async {
        match Server::new(options).serve(rmcp::transport::stdio()).await {
            Ok(service) => {
                let _ = service.waiting().await;
                0
            }
            Err(e) => {
                eprintln!("MCP server failed to start: {e}");
                1
            }
        }
    })
}

// ---------------------------------------------------------------- Tool inputs

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ScanArgs {
    /// What to scan: a subnet ("192.168.1.0/24"), an address, a range ("10.0.0.1-10.0.0.50"
    /// or "10.0.0.1-50"), several of these separated by commas, a host name, or the name of
    /// a saved favorite.
    pub target: String,
    /// Ports to check: "web", "common" (default), "well-known" (1-1024), "none", or a list
    /// such as "22,80,443,8000-8100".
    #[serde(default)]
    pub ports: Option<String>,
    /// Also look up MAC address, NetBIOS name, web server and TTL. Slower. Default false.
    #[serde(default)]
    pub details: bool,
    /// Also list addresses that did not answer. Default false.
    #[serde(default)]
    pub include_dead: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ScanHostArgs {
    /// The address or host name to scan.
    pub address: String,
    /// Ports to check, as for `scan`. Default "well-known" (1-1024).
    #[serde(default)]
    pub ports: Option<String>,
}

// ---------------------------------------------------------------- Tool outputs

#[derive(Debug, Serialize, JsonSchema)]
pub struct LocalNetwork {
    /// This machine's IPv4 address on the network it uses to reach the internet.
    pub local_ip: Option<String>,
    /// The /24 subnet around that address: what the app scans by default.
    pub subnet: String,
    /// Whether this server may scan addresses outside private ranges.
    pub public_addresses_allowed: bool,
    /// The largest number of addresses one scan may cover.
    pub max_hosts_per_scan: u64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct OpenPort {
    pub port: u16,
    /// The usual service on this port, e.g. "SSH" or "HTTPS".
    pub service: Option<String>,
    /// The address to open in a browser, for web ports.
    pub url: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Host {
    pub ip: String,
    pub alive: bool,
    pub hostname: Option<String>,
    /// Average round-trip time of the ping.
    pub ping_ms: Option<u32>,
    pub ttl: Option<u8>,
    /// A rough guess at the operating system from the TTL.
    pub os_hint: Option<String>,
    /// A guess at the kind of device from its name and open ports: router, nas, printer,
    /// tv, phone, computer, single-board or unknown.
    pub device: String,
    pub mac: Option<String>,
    pub netbios: Option<String>,
    /// The web server's name from its HTTP headers.
    pub web_server: Option<String>,
    pub open_ports: Vec<OpenPort>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ScanReport {
    /// The addresses that were scanned, e.g. "192.168.1.0 - 192.168.1.255".
    pub scanned: String,
    pub ports_checked: String,
    pub addresses: u64,
    pub alive: usize,
    pub with_open_ports: usize,
    pub seconds: f64,
    /// Hosts sorted by address: alive ones only, unless include_dead was set.
    pub hosts: Vec<Host>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SavedTarget {
    pub name: String,
    /// Pass this, or the name, as `target` to `scan`.
    pub target: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RecentTarget {
    pub target: String,
    /// When the scan finished, in seconds since 1970 (Unix time).
    pub finished_at: u64,
    pub hosts_online: usize,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Saved {
    pub favorites: Vec<SavedTarget>,
    pub recent_scans: Vec<RecentTarget>,
}

// ---------------------------------------------------------------- Server

#[derive(Clone)]
pub struct Server {
    options: Options,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl Server {
    pub fn new(options: Options) -> Self {
        Server {
            options,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        description = "This machine's IP address and the subnet to scan for devices on the local network."
    )]
    fn local_network(&self) -> Json<LocalNetwork> {
        let (ip, subnet) = config::local_subnet();
        Json(LocalNetwork {
            local_ip: ip.map(|i| i.to_string()),
            subnet,
            public_addresses_allowed: self.options.allow_public,
            max_hosts_per_scan: self.options.max_hosts,
        })
    }

    #[tool(
        description = "Scan a network for hosts that answer, with their names and open ports. A /24 subnet takes about half a minute; progress is reported while it runs."
    )]
    async fn scan(
        &self,
        Parameters(args): Parameters<ScanArgs>,
        meta: RequestMetaObject,
        client: Peer<RoleServer>,
    ) -> Result<Json<ScanReport>, String> {
        let feeder = resolve_target(&args.target)?;
        let mut settings = base_settings();
        settings.ports = port_spec(args.ports.as_deref(), 1)?;
        settings.scan_dead_hosts = false;
        set_details(&mut settings, args.details);
        let report = self
            .run_scan(feeder, settings, args.include_dead, meta, client)
            .await?;
        Ok(Json(report))
    }

    #[tool(
        description = "Scan one host in depth: ports 1-1024 by default, plus MAC address, NetBIOS name, web server and TTL."
    )]
    async fn scan_host(
        &self,
        Parameters(args): Parameters<ScanHostArgs>,
        meta: RequestMetaObject,
        client: Peer<RoleServer>,
    ) -> Result<Json<Host>, String> {
        let ip = match scanner::parse_target(&args.address)? {
            Feeder::Range { start, end } if start == end => start,
            _ => return Err("Give a single address or host name; use scan for networks".into()),
        };
        let mut settings = base_settings();
        settings.ports = port_spec(args.ports.as_deref(), 2)?;
        // Check the ports even when the host ignores ping.
        settings.scan_dead_hosts = true;
        settings.skip_broadcast = false;
        set_details(&mut settings, true);
        let feeder = Feeder::List(vec![ip]);
        let mut report = self.run_scan(feeder, settings, true, meta, client).await?;
        report
            .hosts
            .pop()
            .map(Json)
            .ok_or_else(|| "The scan returned no result".into())
    }

    #[tool(description = "The favorites and recent scans saved in the OxiScan app.")]
    fn list_saved(&self) -> Json<Saved> {
        let c = Config::load();
        Json(Saved {
            favorites: c
                .favorites
                .iter()
                .map(|f| SavedTarget {
                    name: f.name.clone(),
                    target: config::target_text(&f.feeder),
                })
                .collect(),
            recent_scans: c
                .recent
                .iter()
                .map(|r| RecentTarget {
                    target: r.target.clone(),
                    finished_at: r.at,
                    hosts_online: r.online,
                })
                .collect(),
        })
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Server {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(
                Implementation::new("oxiscan", env!("CARGO_PKG_VERSION"))
                    .with_title("OxiScan")
                    .with_description("Network scanner for IP addresses and ports"),
            )
            .with_instructions(
                "Scans IPv4 networks for hosts and open ports. Call local_network first to \
                 find the subnet to scan, then scan it. Use scan_host for one device in depth. \
                 Saved favorites from the OxiScan app can be scanned by name.",
            )
    }
}

impl Server {
    async fn run_scan(
        &self,
        feeder: Feeder,
        settings: Settings,
        include_dead: bool,
        meta: RequestMetaObject,
        client: Peer<RoleServer>,
    ) -> Result<ScanReport, String> {
        check_allowed(&feeder, &self.options)?;
        let ports_checked = settings.ports.clone();
        let started = Instant::now();
        let handle = scanner::start_scan(feeder, settings, Arc::new(|| {}));
        // Stops the scan threads if the client cancels the call or disconnects.
        let _guard = StopOnDrop(handle.stop.clone());
        let token = meta.get_progress_token();
        let mut results = Vec::new();
        let mut last_reported = u64::MAX;
        loop {
            let finished = drain(&handle, &mut results);
            if finished {
                break;
            }
            if let Some(token) = &token {
                let done = handle.done.load(Ordering::Relaxed);
                if done != last_reported {
                    last_reported = done;
                    let alive = results.iter().filter(|r| r.alive).count();
                    let _ = client
                        .notify_progress(
                            ProgressNotificationParam::new(token.clone(), done as f64)
                                .with_total(handle.total as f64)
                                .with_message(format!(
                                    "{done} of {} addresses scanned, {alive} alive",
                                    handle.total
                                )),
                        )
                        .await;
                }
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        results.sort_by_key(|r| u32::from(r.ip));
        let alive = results.iter().filter(|r| r.alive).count();
        let with_open_ports = results.iter().filter(|r| r.has_ports()).count();
        Ok(ScanReport {
            scanned: handle.description.clone(),
            ports_checked: if ports_checked.is_empty() {
                "none".into()
            } else {
                ports_checked
            },
            addresses: handle.total,
            alive,
            with_open_ports,
            seconds: (started.elapsed().as_secs_f64() * 10.0).round() / 10.0,
            hosts: results
                .iter()
                .filter(|r| include_dead || r.alive)
                .map(host)
                .collect(),
        })
    }
}

struct StopOnDrop(Arc<AtomicBool>);

impl Drop for StopOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

/// Moves finished results out of the channel. Returns true once the scan is over.
fn drain(handle: &ScanHandle, out: &mut Vec<ScanResult>) -> bool {
    use std::sync::mpsc::TryRecvError;
    loop {
        match handle.rx.try_recv() {
            Ok(ScanMsg::Result(r)) => out.push(r),
            Ok(ScanMsg::Finished) | Err(TryRecvError::Disconnected) => return true,
            Err(TryRecvError::Empty) => return false,
        }
    }
}

/// The user's timing settings from the app, with everything the tools decide reset.
fn base_settings() -> Settings {
    let mut s = Config::load().settings;
    s.fetch_hostname = true;
    s.skip_broadcast = true;
    s
}

fn set_details(s: &mut Settings, on: bool) {
    s.fetch_ttl = on;
    s.fetch_mac = on;
    s.fetch_netbios = on;
    s.fetch_web = on;
}

/// Turns a preset name or a port list into a port spec. `default` indexes PRESETS.
fn port_spec(ports: Option<&str>, default: usize) -> Result<String, String> {
    let spec = match ports.map(|p| p.trim().to_lowercase()) {
        None => services::PRESETS[default].1.to_string(),
        Some(p) if p.is_empty() => services::PRESETS[default].1.to_string(),
        Some(p) => match p.as_str() {
            "web" => services::PRESETS[0].1.to_string(),
            "common" | "default" => services::PRESETS[1].1.to_string(),
            "well-known" | "wellknown" | "all-well-known" => services::PRESETS[2].1.to_string(),
            "none" => String::new(),
            _ => p,
        },
    };
    scanner::parse_ports(&spec).map_err(|e| format!("ports: {e}"))?;
    Ok(spec)
}

/// A saved favorite by name, or else a free-form target.
fn resolve_target(target: &str) -> Result<Feeder, String> {
    let wanted = target.trim();
    let favorite: Option<FeederState> = Config::load()
        .favorites
        .into_iter()
        .find(|f| f.name.trim().eq_ignore_ascii_case(wanted))
        .map(|f| f.feeder);
    match favorite {
        Some(f) => f.to_feeder(&mut Vec::new()),
        None => scanner::parse_target(wanted),
    }
}

/// Addresses that stay inside the local network or the machine itself.
pub fn is_local_range(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        // 100.64.0.0/10: carrier-grade NAT, also used by VPNs such as Tailscale.
        || (o[0] == 100 && (64..128).contains(&o[1]))
}

fn check_allowed(feeder: &Feeder, o: &Options) -> Result<(), String> {
    let total = feeder.total();
    if total > o.max_hosts {
        return Err(format!(
            "That target has {total} addresses; this server scans at most {} at a time. \
             Scan a smaller range, or start the server with a larger --max-hosts.",
            o.max_hosts
        ));
    }
    if o.allow_public {
        return Ok(());
    }
    let public = match feeder {
        Feeder::Range { start, end } => (u32::from(*start)..=u32::from(*end))
            .map(Ipv4Addr::from)
            .find(|ip| !is_local_range(*ip)),
        Feeder::Random { base, prefix, .. } => {
            let (start, end) =
                services::parse_cidr(&format!("{base}/{prefix}")).unwrap_or((*base, *base));
            [start, end].into_iter().find(|ip| !is_local_range(*ip))
        }
        Feeder::List(v) => v.iter().copied().find(|ip| !is_local_range(*ip)),
    };
    match public {
        Some(ip) => Err(format!(
            "{ip} is a public internet address. This server only scans private and local \
             networks unless it is started with --allow-public."
        )),
        None => Ok(()),
    }
}

fn host(r: &ScanResult) -> Host {
    let kind = if r.alive {
        services::device_kind(r, r.ip.octets()[3] == 1)
    } else {
        DeviceKind::Unknown
    };
    Host {
        ip: r.ip.to_string(),
        alive: r.alive,
        hostname: r.hostname.clone(),
        ping_ms: r.ping_ms,
        ttl: r.ttl,
        os_hint: r.ttl.map(|t| services::ttl_hint(t).to_string()),
        device: device_name(kind).into(),
        mac: r.mac.clone(),
        netbios: r.netbios.clone(),
        web_server: r.web.clone(),
        open_ports: r
            .ports
            .iter()
            .map(|&port| OpenPort {
                port,
                service: Some(services::service_name(port))
                    .filter(|s| !s.is_empty())
                    .map(String::from),
                url: services::web_url(r.ip, port),
            })
            .collect(),
    }
}

fn device_name(k: DeviceKind) -> &'static str {
    match k {
        DeviceKind::Router => "router",
        DeviceKind::Nas => "nas",
        DeviceKind::Printer => "printer",
        DeviceKind::Tv => "tv",
        DeviceKind::Phone => "phone",
        DeviceKind::Computer => "computer",
        DeviceKind::SingleBoard => "single-board",
        DeviceKind::Unknown => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(allow_public: bool) -> Options {
        Options {
            allow_public,
            max_hosts: DEFAULT_MAX_HOSTS,
        }
    }

    #[test]
    fn options() {
        let a = |v: &[&str]| Options::parse(&v.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert_eq!(a(&[]).unwrap(), opts(false));
        let o = a(&["--allow-public", "--max-hosts", "70000"]).unwrap();
        assert!(o.allow_public);
        assert_eq!(o.max_hosts, 70000);
        assert_eq!(a(&["--max-hosts=10"]).unwrap().max_hosts, 10);
        assert!(a(&["--max-hosts=0"]).is_err());
        assert!(a(&["--bogus"]).is_err());
    }

    #[test]
    fn guardrails() {
        let t = |s: &str| scanner::parse_target(s).unwrap();
        assert!(check_allowed(&t("192.168.1.0/24"), &opts(false)).is_ok());
        assert!(check_allowed(&t("10.1.2.3, 172.16.0.1, 100.100.1.1"), &opts(false)).is_ok());
        assert!(check_allowed(&t("8.8.8.8"), &opts(false)).is_err());
        assert!(check_allowed(&t("8.8.8.8"), &opts(true)).is_ok());
        // A range that runs out of 192.168/16 into public space.
        assert!(check_allowed(&t("192.167.255.250-192.168.0.5"), &opts(false)).is_err());
        assert!(check_allowed(&t("10.0.0.0/16"), &opts(false)).is_err());
    }

    #[test]
    fn port_presets() {
        assert_eq!(port_spec(None, 1).unwrap(), services::PRESETS[1].1);
        assert_eq!(port_spec(Some("Web"), 1).unwrap(), services::PRESETS[0].1);
        assert_eq!(port_spec(Some("none"), 1).unwrap(), "");
        assert_eq!(port_spec(Some("22, 80"), 1).unwrap(), "22, 80");
        assert!(port_spec(Some("http"), 1).is_err());
    }
}
