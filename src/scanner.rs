//! Address feeders, scan settings and the multi-threaded scanning engine.

use crate::net;
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PingMethod {
    Icmp,
    Tcp,
    Combined,
}

impl PingMethod {
    pub fn label(&self) -> &'static str {
        match self {
            PingMethod::Icmp => "ICMP echo",
            PingMethod::Tcp => "TCP port probe",
            PingMethod::Combined => "Combined (ICMP, then TCP)",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub max_threads: usize,
    pub thread_delay_ms: u64,
    pub ping_method: PingMethod,
    pub ping_count: u32,
    pub ping_timeout_ms: u64,
    pub scan_dead_hosts: bool,
    pub skip_broadcast: bool,
    pub ports: String,
    pub port_timeout_ms: u64,
    pub adapt_port_timeout: bool,
    pub fetch_hostname: bool,
    pub fetch_ttl: bool,
    pub fetch_mac: bool,
    pub fetch_netbios: bool,
    pub fetch_web: bool,
    pub show_stats: bool,
    pub ask_confirm_new_scan: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            max_threads: 100,
            thread_delay_ms: 20,
            ping_method: PingMethod::Combined,
            ping_count: 3,
            ping_timeout_ms: 2000,
            scan_dead_hosts: false,
            skip_broadcast: true,
            ports: "80,443,8080".to_string(),
            port_timeout_ms: 2000,
            adapt_port_timeout: true,
            fetch_hostname: true,
            fetch_ttl: false,
            fetch_mac: false,
            fetch_netbios: false,
            fetch_web: false,
            show_stats: true,
            ask_confirm_new_scan: true,
        }
    }
}

/// Parses "1-1024, 3389,8080" into a sorted, unique list.
pub fn parse_ports(spec: &str) -> Result<Vec<u16>, String> {
    let mut v = Vec::new();
    for part in spec.split(|c: char| c == ',' || c.is_whitespace()) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((a, b)) = part.split_once('-') {
            let a: u16 = a.trim().parse().map_err(|_| format!("Bad port: {part}"))?;
            let b: u16 = b.trim().parse().map_err(|_| format!("Bad port: {part}"))?;
            if a == 0 || a > b {
                return Err(format!("Bad port range: {part}"));
            }
            v.extend(a..=b);
        } else {
            let p: u16 = part.parse().map_err(|_| format!("Bad port: {part}"))?;
            if p == 0 {
                return Err("Port 0 is not valid".into());
            }
            v.push(p);
        }
    }
    v.sort_unstable();
    v.dedup();
    Ok(v)
}

/// Turns [20,21,22,80] into "20-22,80".
pub fn format_ports(ports: &[u16]) -> String {
    let mut out = Vec::new();
    let mut i = 0;
    while i < ports.len() {
        let start = ports[i];
        let mut end = start;
        while i + 1 < ports.len() && ports[i + 1] == end + 1 {
            i += 1;
            end = ports[i];
        }
        if start == end {
            out.push(start.to_string());
        } else {
            out.push(format!("{start}-{end}"));
        }
        i += 1;
    }
    out.join(",")
}

// ---------------------------------------------------------------- Feeders

#[derive(Clone, Debug)]
pub enum Feeder {
    Range {
        start: Ipv4Addr,
        end: Ipv4Addr,
    },
    Random {
        base: Ipv4Addr,
        prefix: u8,
        count: u32,
    },
    List(Vec<Ipv4Addr>),
}

impl Feeder {
    pub fn total(&self) -> u64 {
        match self {
            Feeder::Range { start, end } => u32::from(*end) as u64 - u32::from(*start) as u64 + 1,
            Feeder::Random { count, .. } => *count as u64,
            Feeder::List(v) => v.len() as u64,
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Feeder::Range { start, end } => format!("{start} - {end}"),
            Feeder::Random {
                base,
                prefix,
                count,
            } => {
                format!("{count} random in {base}/{prefix}")
            }
            Feeder::List(v) => format!("{} addresses from list", v.len()),
        }
    }

    fn into_iter(self) -> Box<dyn Iterator<Item = Ipv4Addr> + Send> {
        match self {
            Feeder::Range { start, end } => {
                Box::new((u32::from(start)..=u32::from(end)).map(Ipv4Addr::from))
            }
            Feeder::Random {
                base,
                prefix,
                count,
            } => {
                let mask: u32 = if prefix == 0 {
                    0
                } else {
                    u32::MAX << (32 - prefix as u32)
                };
                let net = u32::from(base) & mask;
                let mut seed = Instant::now().elapsed().as_nanos() as u64
                    ^ std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_nanos() as u64)
                        .unwrap_or(12345)
                    | 1;
                Box::new((0..count).map(move |_| {
                    seed ^= seed << 13;
                    seed ^= seed >> 7;
                    seed ^= seed << 17;
                    Ipv4Addr::from(net | ((seed as u32) & !mask))
                }))
            }
            Feeder::List(v) => Box::new(v.into_iter()),
        }
    }
}

/// Extracts every IPv4 address found in a text file (one per line or mixed with other text).
pub fn parse_ip_list(text: &str) -> Vec<Ipv4Addr> {
    let mut out = Vec::new();
    for token in text.split(|c: char| !(c.is_ascii_digit() || c == '.')) {
        if let Ok(ip) = token.trim_matches('.').parse::<Ipv4Addr>() {
            out.push(ip);
        }
    }
    out
}

// ---------------------------------------------------------------- Results

#[derive(Clone, Debug)]
pub struct ScanResult {
    pub ip: Ipv4Addr,
    pub alive: bool,
    pub ping_ms: Option<u32>,
    pub ttl: Option<u8>,
    pub hostname: Option<String>,
    pub hostname_scanned: bool,
    pub ports: Vec<u16>,
    pub ports_scanned: bool,
    pub mac: Option<String>,
    pub netbios: Option<String>,
    pub web: Option<String>,
    pub extra_scanned: bool,
}

impl ScanResult {
    fn new(ip: Ipv4Addr) -> Self {
        ScanResult {
            ip,
            alive: false,
            ping_ms: None,
            ttl: None,
            hostname: None,
            hostname_scanned: false,
            ports: Vec::new(),
            ports_scanned: false,
            mac: None,
            netbios: None,
            web: None,
            extra_scanned: false,
        }
    }

    pub fn has_ports(&self) -> bool {
        !self.ports.is_empty()
    }
}

pub enum ScanMsg {
    Result(ScanResult),
    Finished,
}

pub struct ScanHandle {
    pub rx: Receiver<ScanMsg>,
    pub stop: Arc<AtomicBool>,
    pub done: Arc<AtomicU64>,
    pub active: Arc<AtomicUsize>,
    pub current: Arc<Mutex<Option<Ipv4Addr>>>,
    pub total: u64,
    pub started: Instant,
    pub description: String,
}

impl ScanHandle {
    pub fn progress(&self) -> f32 {
        if self.total == 0 {
            1.0
        } else {
            (self.done.load(Ordering::Relaxed) as f64 / self.total as f64) as f32
        }
    }
}

fn is_broadcast_like(ip: Ipv4Addr) -> bool {
    let last = ip.octets()[3];
    last == 0 || last == 255
}

pub fn start_scan(feeder: Feeder, settings: Settings, ctx: eframe::egui::Context) -> ScanHandle {
    let (tx, rx) = channel();
    let stop = Arc::new(AtomicBool::new(false));
    let done = Arc::new(AtomicU64::new(0));
    let active = Arc::new(AtomicUsize::new(0));
    let current = Arc::new(Mutex::new(None));
    let total = feeder.total();
    let description = feeder.describe();
    let skip_bcast = settings.skip_broadcast && matches!(feeder, Feeder::Range { .. });
    let source = Arc::new(Mutex::new(feeder.into_iter()));
    let ports = Arc::new(parse_ports(&settings.ports).unwrap_or_default());
    let settings = Arc::new(settings);

    {
        let stop = stop.clone();
        let done = done.clone();
        let active = active.clone();
        let current = current.clone();
        std::thread::spawn(move || {
            let mut workers = Vec::new();
            let n = settings.max_threads.clamp(1, 1000);
            for i in 0..n {
                let source = source.clone();
                let tx: Sender<ScanMsg> = tx.clone();
                let stop = stop.clone();
                let done = done.clone();
                let active = active.clone();
                let current = current.clone();
                let settings = settings.clone();
                let ports = ports.clone();
                let ctx = ctx.clone();
                // Stagger thread start-up so the network isn't flooded at once.
                if settings.thread_delay_ms > 0 && i > 0 {
                    std::thread::sleep(Duration::from_millis(settings.thread_delay_ms));
                }
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                workers.push(std::thread::spawn(move || loop {
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    let next = source.lock().unwrap().next();
                    let Some(ip) = next else { break };
                    if skip_bcast && is_broadcast_like(ip) {
                        done.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }
                    active.fetch_add(1, Ordering::Relaxed);
                    *current.lock().unwrap() = Some(ip);
                    let r = scan_host(ip, &settings, &ports, &stop);
                    active.fetch_sub(1, Ordering::Relaxed);
                    done.fetch_add(1, Ordering::Relaxed);
                    let _ = tx.send(ScanMsg::Result(r));
                    ctx.request_repaint();
                }));
            }
            for w in workers {
                let _ = w.join();
            }
            let _ = tx.send(ScanMsg::Finished);
            ctx.request_repaint();
        });
    }

    ScanHandle {
        rx,
        stop,
        done,
        active,
        current,
        total,
        started: Instant::now(),
        description,
    }
}

pub fn scan_host(ip: Ipv4Addr, s: &Settings, ports: &[u16], stop: &AtomicBool) -> ScanResult {
    let mut r = ScanResult::new(ip);
    let timeout = Duration::from_millis(s.ping_timeout_ms.max(50));

    // 1. Ping
    let mut rtts = Vec::new();
    let mut ttl = None;
    let use_icmp = matches!(s.ping_method, PingMethod::Icmp | PingMethod::Combined);
    if use_icmp {
        for _ in 0..s.ping_count.max(1) {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            if let Some(p) = net::icmp_ping(ip, timeout) {
                rtts.push(p.rtt_ms);
                ttl = ttl.or(p.ttl);
            }
        }
    }
    let use_tcp = s.ping_method == PingMethod::Tcp
        || (s.ping_method == PingMethod::Combined && rtts.is_empty());
    if use_tcp && !stop.load(Ordering::Relaxed) {
        if let Some(p) = net::tcp_ping(ip, timeout) {
            rtts.push(p.rtt_ms);
        }
    }
    if !rtts.is_empty() {
        r.alive = true;
        r.ping_ms = Some(rtts.iter().sum::<u32>() / rtts.len() as u32);
        if s.fetch_ttl {
            r.ttl = ttl;
        }
    }

    if stop.load(Ordering::Relaxed) || (!r.alive && !s.scan_dead_hosts) {
        return r;
    }

    // 2. Hostname
    if s.fetch_hostname {
        r.hostname = net::reverse_dns(ip);
        r.hostname_scanned = true;
    }

    // 3. Ports
    if !ports.is_empty() {
        let mut pt = Duration::from_millis(s.port_timeout_ms.max(50));
        if s.adapt_port_timeout {
            if let Some(ms) = r.ping_ms {
                // Fast hosts don't need the full timeout.
                let adapted = Duration::from_millis((ms as u64 * 3).max(100));
                pt = pt.min(adapted.max(Duration::from_millis(150)));
            }
        }
        r.ports = scan_ports(ip, ports, pt, stop);
        r.ports_scanned = true;
        if !r.ports.is_empty() {
            r.alive = true;
        }
    }

    if !r.alive {
        return r;
    }

    // 4. Extras
    if s.fetch_mac {
        r.mac = net::mac_address(ip);
    }
    if s.fetch_netbios {
        r.netbios = net::netbios_info(ip, timeout.min(Duration::from_millis(1500)));
    }
    if s.fetch_web {
        r.web = net::web_detect(ip, Duration::from_millis(s.port_timeout_ms.max(200)));
    }
    r.extra_scanned = true;
    r
}

fn scan_ports(ip: Ipv4Addr, ports: &[u16], timeout: Duration, stop: &AtomicBool) -> Vec<u16> {
    let parallel = ports.len().clamp(1, 32);
    let idx = AtomicUsize::new(0);
    let found = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..parallel {
            scope.spawn(|| loop {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                let i = idx.fetch_add(1, Ordering::Relaxed);
                if i >= ports.len() {
                    break;
                }
                if net::port_open(ip, ports[i], timeout) {
                    found.lock().unwrap().push(ports[i]);
                }
            });
        }
    });
    let mut v = found.into_inner().unwrap();
    v.sort_unstable();
    v
}

/// Re-scans a list of single hosts (used by "Rescan selected").
pub fn start_rescan(
    ips: Vec<Ipv4Addr>,
    settings: Settings,
    ctx: eframe::egui::Context,
) -> ScanHandle {
    let mut s = settings;
    s.skip_broadcast = false;
    start_scan(Feeder::List(ips), s, ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ports_parse_and_format() {
        let v = parse_ports("80, 20-22,443 22").unwrap();
        assert_eq!(v, vec![20, 21, 22, 80, 443]);
        assert_eq!(format_ports(&v), "20-22,80,443");
        assert!(parse_ports("0").is_err());
        assert!(parse_ports("90-80").is_err());
    }

    #[test]
    fn feeders() {
        let f = Feeder::Range {
            start: "10.0.0.250".parse().unwrap(),
            end: "10.0.1.2".parse().unwrap(),
        };
        assert_eq!(f.total(), 9);
        let ips: Vec<_> = f.into_iter().collect();
        assert_eq!(ips.last().unwrap().to_string(), "10.0.1.2");
        let r = Feeder::Random {
            base: "192.168.5.7".parse().unwrap(),
            prefix: 24,
            count: 50,
        };
        assert!(r.into_iter().all(|ip| ip.octets()[..3] == [192, 168, 5]));
        assert_eq!(parse_ip_list("a 1.2.3.4\nx,5.6.7.8;bad 999.1.1.1").len(), 2);
    }

    #[test]
    fn scan_localhost() {
        // Open a listening port so the port scan has something to find.
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let s = Settings {
            ports: format!("{port},1"),
            ping_count: 1,
            fetch_ttl: true,
            ..Settings::default()
        };
        let ports = parse_ports(&s.ports).unwrap();
        let r = scan_host(
            "127.0.0.1".parse().unwrap(),
            &s,
            &ports,
            &AtomicBool::new(false),
        );
        assert!(r.alive, "localhost should answer");
        assert!(r.ping_ms.is_some());
        assert_eq!(r.ports, vec![port]);
        println!("{r:?}");
    }
}
