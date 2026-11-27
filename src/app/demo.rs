//! Made-up example data for `--demo`, used for screenshots and for trying the interface
//! without scanning a real network. Nothing from a demo run is saved.

use super::{ScannerApp, Stats};
use crate::config::{Favorite, FeederKind, FeederState, RecentScan};
use crate::scanner::{ScanHandle, ScanResult};
use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DemoMode {
    /// A finished scan with a host selected.
    Finished,
    /// First launch: nothing scanned yet.
    Empty,
    /// A scan that is 40% done.
    Scanning,
    /// First launch with the advanced options open.
    Advanced,
}

impl DemoMode {
    pub fn parse(s: &str) -> Option<DemoMode> {
        match s {
            "" | "finished" => Some(DemoMode::Finished),
            "empty" => Some(DemoMode::Empty),
            "scanning" => Some(DemoMode::Scanning),
            "advanced" => Some(DemoMode::Advanced),
            _ => None,
        }
    }
}

fn subnet(cidr: &str) -> FeederState {
    FeederState {
        kind: FeederKind::Subnet,
        cidr: cidr.into(),
        random_prefix: 24,
        random_count: "100".into(),
        ..Default::default()
    }
}

struct Host {
    last: u8,
    name: Option<&'static str>,
    ping: u32,
    ttl: u8,
    ports: &'static [u16],
    mac: &'static str,
    netbios: Option<&'static str>,
    web: Option<&'static str>,
}

const HOSTS: [Host; 9] = [
    Host {
        last: 1,
        name: Some("router.lan"),
        ping: 2,
        ttl: 64,
        ports: &[53, 80, 443],
        mac: "02:1A:2B:3C:4D:01",
        netbios: None,
        web: Some("lighttpd"),
    },
    Host {
        last: 4,
        name: Some("nas.lan"),
        ping: 1,
        ttl: 64,
        ports: &[22, 80, 139, 445, 5000],
        mac: "00:11:32:4A:9C:E1",
        netbios: Some("WORKGROUP\\NAS"),
        web: Some("nginx"),
    },
    Host {
        last: 6,
        name: Some("printer.lan"),
        ping: 4,
        ttl: 255,
        ports: &[80, 443, 631, 9100],
        mac: "3C:2A:F4:10:22:6B",
        netbios: None,
        web: Some("HTTP server"),
    },
    Host {
        last: 9,
        name: Some("living-room-tv.lan"),
        ping: 3,
        ttl: 64,
        ports: &[],
        mac: "A8:23:FE:51:0C:9D",
        netbios: None,
        web: None,
    },
    Host {
        last: 10,
        name: Some("android-7f3c.lan"),
        ping: 6,
        ttl: 64,
        ports: &[],
        mac: "DA:A1:19:7F:3C:02",
        netbios: None,
        web: None,
    },
    Host {
        last: 12,
        name: Some("DESKTOP-PC"),
        ping: 1,
        ttl: 128,
        ports: &[135, 139, 445, 3389],
        mac: "B4:2E:99:61:7A:10",
        netbios: Some("WORKGROUP\\DESKTOP-PC"),
        web: None,
    },
    Host {
        last: 15,
        name: None,
        ping: 12,
        ttl: 64,
        ports: &[],
        mac: "F0:9F:C2:3E:11:88",
        netbios: None,
        web: None,
    },
    Host {
        last: 17,
        name: Some("raspberrypi.lan"),
        ping: 2,
        ttl: 64,
        ports: &[22, 80],
        mac: "DC:A6:32:8B:44:5E",
        netbios: None,
        web: Some("Apache/2.4"),
    },
    Host {
        last: 20,
        name: None,
        ping: 2,
        ttl: 64,
        ports: &[],
        mac: "52:54:00:12:34:56",
        netbios: None,
        web: None,
    },
];

fn result(last: u8) -> ScanResult {
    let ip = Ipv4Addr::new(192, 168, 1, last);
    match HOSTS.iter().find(|h| h.last == last) {
        Some(h) => ScanResult {
            ip,
            alive: true,
            ping_ms: Some(h.ping),
            ttl: Some(h.ttl),
            hostname: h.name.map(String::from),
            hostname_scanned: true,
            ports: h.ports.to_vec(),
            ports_scanned: true,
            mac: Some(h.mac.into()),
            netbios: h.netbios.map(String::from),
            web: h.web.map(String::from),
            extra_scanned: true,
        },
        None => ScanResult {
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
        },
    }
}

impl ScannerApp {
    pub(super) fn load_demo(&mut self, mode: DemoMode) {
        let home = subnet("192.168.1.0/24");
        self.config.favorites = vec![
            Favorite {
                name: "Home network".into(),
                feeder: home.clone(),
            },
            Favorite {
                name: "Office VLAN".into(),
                feeder: subnet("10.0.10.0/24"),
            },
            Favorite {
                name: "Lab".into(),
                feeder: subnet("172.16.0.0/22"),
            },
        ];
        let s = &mut self.config.settings;
        s.fetch_ttl = true;
        s.fetch_mac = true;
        s.fetch_netbios = true;
        s.fetch_web = true;
        self.feeder = home.clone();
        self.local_ip = Some(Ipv4Addr::new(192, 168, 1, 12));
        self.last_desc = "192.168.1.0 - 192.168.1.255".into();
        if mode == DemoMode::Advanced {
            self.show_advanced = true;
        }
        if matches!(mode, DemoMode::Empty | DemoMode::Advanced) {
            return;
        }
        let now = super::unix_now();
        self.config.recent = vec![
            RecentScan {
                at: now - 120,
                target: "192.168.1.0/24".into(),
                feeder: home.clone(),
                online: 9,
            },
            RecentScan {
                at: now - 26 * 3600,
                target: "192.168.1.0/24".into(),
                feeder: home.clone(),
                online: 8,
            },
            RecentScan {
                at: now - 4 * 86400,
                target: "10.0.10.0/24".into(),
                feeder: subnet("10.0.10.0/24"),
                online: 7,
            },
        ];
        let upto = if mode == DemoMode::Scanning { 102 } else { 254 };
        self.results = (1..=upto).map(result).collect();
        self.by_ip = self
            .results
            .iter()
            .enumerate()
            .map(|(i, r)| (r.ip, i))
            .collect();
        self.view_dirty = true;
        let nas = self.by_ip[&Ipv4Addr::new(192, 168, 1, 4)];
        self.selected.insert(nas);
        self.anchor = Some(nas);

        if mode == DemoMode::Finished {
            self.stats = Some(Stats {
                seconds: 41.3,
                stopped: false,
            });
        } else {
            self.config.recent.remove(0);
            let (_tx, rx) = mpsc::channel();
            self.scan = Some(ScanHandle {
                rx,
                stop: Arc::new(AtomicBool::new(false)),
                done: Arc::new(AtomicU64::new(102)),
                active: Arc::new(AtomicUsize::new(100)),
                current: Arc::new(Mutex::new(Some(Ipv4Addr::new(192, 168, 1, 103)))),
                total: 254,
                started: Instant::now()
                    .checked_sub(Duration::from_secs(17))
                    .unwrap_or_else(Instant::now),
                description: self.last_desc.clone(),
            });
            self.scan_full = true;
        }
    }
}
