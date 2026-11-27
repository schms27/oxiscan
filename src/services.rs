//! What we know about well-known ports and devices: service names, one-click actions,
//! port presets, subnet parsing and a rough guess of the device type.

use crate::opener::Opener;
use crate::scanner::ScanResult;
use std::net::Ipv4Addr;

/// Port presets offered in the sidebar: (label, port list).
pub const PRESETS: [(&str, &str); 3] = [
    ("Web (80, 443, 8080, 8443)", "80,443,8080,8443"),
    (
        "Common services",
        "21,22,23,25,53,80,110,135,139,143,443,445,548,631,993,995,1433,1883,3306,3389,5000,5001,5432,5900,8080,8443,9100",
    ),
    ("Well-known ports (1-1024)", "1-1024"),
];

pub fn service_name(port: u16) -> &'static str {
    match port {
        21 => "FTP",
        22 => "SSH",
        23 => "Telnet",
        25 => "SMTP mail",
        53 => "DNS",
        80 => "HTTP",
        110 => "POP3 mail",
        135 => "Windows RPC",
        139 => "NetBIOS session",
        143 => "IMAP mail",
        443 => "HTTPS",
        445 => "SMB file sharing",
        548 => "AFP file sharing",
        631 => "IPP printing",
        993 => "IMAPS mail",
        995 => "POP3S mail",
        1433 => "SQL Server",
        1883 => "MQTT",
        3306 => "MySQL",
        3389 => "Remote Desktop",
        5000 => "HTTP (web UI)",
        5001 => "HTTPS (web UI)",
        5432 => "PostgreSQL",
        5900 => "VNC",
        8000 | 8008 | 8080 | 8888 => "HTTP (alternate)",
        8443 => "HTTPS (alternate)",
        9100 => "Raw printing",
        _ => "",
    }
}

fn is_http(port: u16) -> bool {
    matches!(port, 80 | 631 | 5000 | 8000 | 8008 | 8080 | 8888)
}

fn is_https(port: u16) -> bool {
    matches!(port, 443 | 5001 | 8443)
}

pub fn web_url(ip: Ipv4Addr, port: u16) -> Option<String> {
    if is_http(port) {
        Some(if port == 80 {
            format!("http://{ip}/")
        } else {
            format!("http://{ip}:{port}/")
        })
    } else if is_https(port) {
        Some(if port == 443 {
            format!("https://{ip}/")
        } else {
            format!("https://{ip}:{port}/")
        })
    } else {
        None
    }
}

/// The one-click action for an open port, if there is one: (button label, what it does).
pub fn port_action(ip: Ipv4Addr, port: u16) -> Option<(&'static str, Opener)> {
    if let Some(url) = web_url(ip, port) {
        return Some(("Open", Opener::Url(url)));
    }
    match port {
        22 => Some(("SSH", Opener::Terminal(format!("ssh {ip}")))),
        445 => Some(("Open share", Opener::Shares(ip))),
        3389 => Some(("Connect", Opener::Rdp(ip))),
        _ => None,
    }
}

/// The first web interface of a host, preferring plain HTTP on port 80.
pub fn first_web_url(r: &ScanResult) -> Option<String> {
    r.ports.iter().find_map(|p| web_url(r.ip, *p))
}

/// A rough operating-system hint from the TTL a host answered with.
pub fn ttl_hint(ttl: u8) -> &'static str {
    match ttl {
        0..=64 => "Linux / macOS",
        65..=128 => "Windows",
        _ => "Network device",
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DeviceKind {
    Router,
    Nas,
    Printer,
    Tv,
    Phone,
    Computer,
    SingleBoard,
    Unknown,
}

/// Guesses what kind of device a host is from its name and open ports.
pub fn device_kind(r: &ScanResult, gateway_like: bool) -> DeviceKind {
    let name = r.hostname.as_deref().unwrap_or("").to_lowercase();
    let has = |p: u16| r.ports.contains(&p);
    let named = |words: &[&str]| words.iter().any(|w| name.contains(w));
    if named(&["raspberry", "pi.", "rpi"]) {
        DeviceKind::SingleBoard
    } else if named(&["printer", "print", "epson", "brother", "canon", "hp-"])
        || has(631)
        || has(9100)
    {
        DeviceKind::Printer
    } else if named(&["nas", "synology", "diskstation", "qnap"]) || (has(445) && has(5000)) {
        DeviceKind::Nas
    } else if named(&["router", "gateway", "fritz", "modem"]) || (gateway_like && has(53)) {
        DeviceKind::Router
    } else if named(&["tv", "chromecast", "roku", "firetv", "appletv"]) {
        DeviceKind::Tv
    } else if named(&["android", "iphone", "ipad", "phone", "galaxy", "pixel"]) {
        DeviceKind::Phone
    } else if named(&["desktop", "laptop", "pc", "macbook", "imac", "workstation"])
        || has(3389)
        || has(135)
    {
        DeviceKind::Computer
    } else if gateway_like {
        DeviceKind::Router
    } else {
        DeviceKind::Unknown
    }
}

/// Parses "192.168.1.0/24" or a single address into an inclusive range.
pub fn parse_cidr(text: &str) -> Option<(Ipv4Addr, Ipv4Addr)> {
    let text = text.trim();
    let (addr, prefix) = match text.split_once('/') {
        Some((a, p)) => (a.trim(), p.trim().parse::<u8>().ok().filter(|p| *p <= 32)?),
        None => (text, 32),
    };
    let ip: Ipv4Addr = addr.parse().ok()?;
    let mask: u32 = if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix as u32)
    };
    let v = u32::from(ip);
    Some((Ipv4Addr::from(v & mask), Ipv4Addr::from(v | !mask)))
}

/// Counts the addresses in an inclusive range.
pub fn range_len(start: Ipv4Addr, end: Ipv4Addr) -> u64 {
    (u32::from(end) as u64).saturating_sub(u32::from(start) as u64) + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cidr_parsing() {
        let (a, b) = parse_cidr("192.168.1.77/24").unwrap();
        assert_eq!(a, Ipv4Addr::new(192, 168, 1, 0));
        assert_eq!(b, Ipv4Addr::new(192, 168, 1, 255));
        assert_eq!(range_len(a, b), 256);
        let (a, b) = parse_cidr("10.0.0.5").unwrap();
        assert_eq!(a, b);
        assert!(parse_cidr("10.0.0.0/33").is_none());
        assert!(parse_cidr("nas.lan").is_none());
    }

    #[test]
    fn presets_are_valid_port_lists() {
        for (_, ports) in PRESETS {
            assert!(!crate::scanner::parse_ports(ports).unwrap().is_empty());
        }
    }

    #[test]
    fn port_actions() {
        let ip = Ipv4Addr::new(10, 0, 0, 2);
        assert_eq!(web_url(ip, 80).as_deref(), Some("http://10.0.0.2/"));
        assert_eq!(web_url(ip, 8443).as_deref(), Some("https://10.0.0.2:8443/"));
        assert!(port_action(ip, 22).is_some());
        assert!(port_action(ip, 139).is_none());
    }
}
