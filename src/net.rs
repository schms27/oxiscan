//! Low-level network probes: ICMP ping, TCP probes, ARP (MAC), NetBIOS, HTTP banner.

use std::io::{ErrorKind, Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, UdpSocket};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug)]
pub struct PingReply {
    pub rtt_ms: u32,
    pub ttl: Option<u8>,
}

// ---------------------------------------------------------------- ICMP

#[cfg(windows)]
mod win {
    use std::ffi::c_void;

    #[repr(C)]
    pub struct IpOptionInformation {
        pub ttl: u8,
        pub tos: u8,
        pub flags: u8,
        pub options_size: u8,
        pub options_data: *mut u8,
    }

    #[repr(C)]
    pub struct IcmpEchoReply {
        pub address: u32,
        pub status: u32,
        pub round_trip_time: u32,
        pub data_size: u16,
        pub reserved: u16,
        pub data: *mut c_void,
        pub options: IpOptionInformation,
    }

    #[link(name = "iphlpapi")]
    extern "system" {
        pub fn IcmpCreateFile() -> *mut c_void;
        pub fn IcmpCloseHandle(handle: *mut c_void) -> i32;
        pub fn IcmpSendEcho(
            handle: *mut c_void,
            dest: u32,
            request_data: *const c_void,
            request_size: u16,
            request_options: *const IpOptionInformation,
            reply_buffer: *mut c_void,
            reply_size: u32,
            timeout: u32,
        ) -> u32;
        pub fn SendARP(dest_ip: u32, src_ip: u32, mac: *mut c_void, len: *mut u32) -> u32;
    }
}

/// Sends one ICMP echo request. Uses the Windows ICMP API (no admin rights needed);
/// on other systems uses the system `ping` command.
#[cfg(windows)]
pub fn icmp_ping(ip: Ipv4Addr, timeout: Duration) -> Option<PingReply> {
    use std::ffi::c_void;
    unsafe {
        let handle = win::IcmpCreateFile();
        if handle.is_null() || handle as isize == -1 {
            return None;
        }
        let payload = *b"abcdefghijklmnopqrstuvwabcdefghi";
        let mut buf = vec![0u64; 64]; // 512 bytes, 8-byte aligned
        let n = win::IcmpSendEcho(
            handle,
            u32::from_ne_bytes(ip.octets()),
            payload.as_ptr() as *const c_void,
            payload.len() as u16,
            std::ptr::null(),
            buf.as_mut_ptr() as *mut c_void,
            (buf.len() * 8) as u32,
            timeout.as_millis().max(1) as u32,
        );
        let result = if n > 0 {
            let reply = &*(buf.as_ptr() as *const win::IcmpEchoReply);
            if reply.status == 0 {
                Some(PingReply {
                    rtt_ms: reply.round_trip_time,
                    ttl: Some(reply.options.ttl),
                })
            } else {
                None
            }
        } else {
            None
        };
        win::IcmpCloseHandle(handle);
        result
    }
}

#[cfg(not(windows))]
pub fn icmp_ping(ip: Ipv4Addr, timeout: Duration) -> Option<PingReply> {
    let secs = timeout.as_secs().max(1).to_string();
    let start = Instant::now();
    let out = std::process::Command::new("ping")
        .args(["-c", "1", "-W", &secs, &ip.to_string()])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).to_lowercase();
    let ttl = text
        .split("ttl=")
        .nth(1)
        .and_then(|s| s.split_whitespace().next())
        .and_then(|s| s.parse::<u8>().ok());
    let rtt = text
        .split("time=")
        .nth(1)
        .and_then(|s| s.split(|c: char| !(c.is_ascii_digit() || c == '.')).next())
        .and_then(|s| s.parse::<f64>().ok())
        .map(|f| f.round() as u32)
        .unwrap_or(start.elapsed().as_millis() as u32);
    Some(PingReply { rtt_ms: rtt, ttl })
}

// ---------------------------------------------------------------- TCP

pub enum TcpProbe {
    Open(Duration),
    Refused(Duration),
    Nothing,
}

pub fn tcp_probe(ip: Ipv4Addr, port: u16, timeout: Duration) -> TcpProbe {
    let addr = SocketAddr::new(IpAddr::V4(ip), port);
    let start = Instant::now();
    match TcpStream::connect_timeout(&addr, timeout) {
        Ok(_) => TcpProbe::Open(start.elapsed()),
        Err(e) if e.kind() == ErrorKind::ConnectionRefused => {
            // Windows reports a refused connection only after retries; the host answered anyway.
            TcpProbe::Refused(start.elapsed())
        }
        Err(_) => TcpProbe::Nothing,
    }
}

/// "Ping" by trying common TCP ports. A refusal also proves the host is up.
pub fn tcp_ping(ip: Ipv4Addr, timeout: Duration) -> Option<PingReply> {
    for port in [80u16, 443, 22, 445, 139, 3389] {
        match tcp_probe(ip, port, timeout) {
            TcpProbe::Open(d) | TcpProbe::Refused(d) => {
                return Some(PingReply {
                    rtt_ms: d.as_millis() as u32,
                    ttl: None,
                })
            }
            TcpProbe::Nothing => {}
        }
    }
    None
}

pub fn port_open(ip: Ipv4Addr, port: u16, timeout: Duration) -> bool {
    matches!(tcp_probe(ip, port, timeout), TcpProbe::Open(_))
}

// ---------------------------------------------------------------- MAC address

#[cfg(windows)]
pub fn mac_address(ip: Ipv4Addr) -> Option<String> {
    let mut mac = [0u8; 8];
    let mut len: u32 = 6;
    let r = unsafe {
        win::SendARP(
            u32::from_ne_bytes(ip.octets()),
            0,
            mac.as_mut_ptr() as *mut std::ffi::c_void,
            &mut len,
        )
    };
    if r == 0 && len >= 6 && mac[..6].iter().any(|b| *b != 0) {
        Some(format_mac(&mac[..6]))
    } else {
        None
    }
}

#[cfg(not(windows))]
pub fn mac_address(ip: Ipv4Addr) -> Option<String> {
    let table = std::fs::read_to_string("/proc/net/arp").ok()?;
    let ip_s = ip.to_string();
    for line in table.lines().skip(1) {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() >= 4 && cols[0] == ip_s && cols[3] != "00:00:00:00:00:00" {
            return Some(cols[3].to_uppercase());
        }
    }
    None
}

pub fn format_mac(b: &[u8]) -> String {
    b.iter()
        .map(|x| format!("{:02X}", x))
        .collect::<Vec<_>>()
        .join(":")
}

// ---------------------------------------------------------------- Hostname

pub fn reverse_dns(ip: Ipv4Addr) -> Option<String> {
    match dns_lookup::lookup_addr(&IpAddr::V4(ip)) {
        Ok(name) if !name.is_empty() && name != ip.to_string() => Some(name),
        _ => None,
    }
}

pub fn resolve_host(name: &str) -> Option<Ipv4Addr> {
    if let Ok(ip) = name.trim().parse::<Ipv4Addr>() {
        return Some(ip);
    }
    dns_lookup::lookup_host(name.trim())
        .ok()?
        .into_iter()
        .find_map(|a| match a {
            IpAddr::V4(v4) => Some(v4),
            _ => None,
        })
}

// ---------------------------------------------------------------- NetBIOS

/// Queries the NetBIOS name table (UDP 137). Returns e.g. "WORKGROUP\PC-NAME [user]".
pub fn netbios_info(ip: Ipv4Addr, timeout: Duration) -> Option<String> {
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.set_read_timeout(Some(timeout)).ok()?;
    let mut pkt: Vec<u8> = vec![0x13, 0x37, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0x20];
    // Encoded wildcard name "*" padded with zeros
    pkt.push(b'C');
    pkt.push(b'K');
    for _ in 0..15 {
        pkt.push(b'A');
        pkt.push(b'A');
    }
    pkt.extend_from_slice(&[0, 0, 0x21, 0, 1]);
    sock.send_to(&pkt, SocketAddr::new(IpAddr::V4(ip), 137)).ok()?;
    let mut buf = [0u8; 1024];
    let (n, _) = sock.recv_from(&mut buf).ok()?;
    let data = &buf[..n];
    if n < 57 {
        return None;
    }
    let count = data[56] as usize;
    let mut computer = None;
    let mut group = None;
    let mut user = None;
    for i in 0..count {
        let off = 57 + i * 18;
        if off + 18 > n {
            break;
        }
        let name = String::from_utf8_lossy(&data[off..off + 15]).trim().to_string();
        let suffix = data[off + 15];
        let flags = u16::from_be_bytes([data[off + 16], data[off + 17]]);
        let is_group = flags & 0x8000 != 0;
        match (suffix, is_group) {
            (0x00, false) if computer.is_none() => computer = Some(name),
            (0x00, true) if group.is_none() => group = Some(name),
            (0x03, false) => {
                if computer.as_deref() != Some(name.as_str()) && user.is_none() {
                    user = Some(name)
                }
            }
            _ => {}
        }
    }
    let computer = computer?;
    let mut s = match group {
        Some(g) => format!("{}\\{}", g, computer),
        None => computer,
    };
    if let Some(u) = user {
        s.push_str(&format!(" [{}]", u));
    }
    Some(s)
}

// ---------------------------------------------------------------- Web detect

/// Asks the web server on port 80 for its "Server" header.
pub fn web_detect(ip: Ipv4Addr, timeout: Duration) -> Option<String> {
    let addr = SocketAddr::new(IpAddr::V4(ip), 80);
    let mut s = TcpStream::connect_timeout(&addr, timeout).ok()?;
    s.set_read_timeout(Some(timeout.max(Duration::from_millis(800)))).ok()?;
    s.set_write_timeout(Some(timeout)).ok()?;
    let req = format!("HEAD / HTTP/1.0\r\nHost: {}\r\nUser-Agent: NetScanner\r\n\r\n", ip);
    s.write_all(req.as_bytes()).ok()?;
    let mut buf = vec![0u8; 4096];
    let mut total = 0;
    while total < buf.len() {
        match s.read(&mut buf[total..]) {
            Ok(0) => break,
            Ok(k) => {
                total += k;
                if buf[..total].windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    let text = String::from_utf8_lossy(&buf[..total]);
    if !text.starts_with("HTTP/") {
        return None;
    }
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("Server:").or_else(|| line.strip_prefix("server:")) {
            return Some(rest.trim().to_string());
        }
    }
    Some("HTTP server".to_string())
}

// ---------------------------------------------------------------- Local info

pub fn local_ipv4() -> Option<Ipv4Addr> {
    // No packet is sent: connecting a UDP socket only picks the outgoing interface.
    let s = UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("8.8.8.8:80").ok()?;
    match s.local_addr().ok()?.ip() {
        IpAddr::V4(v) if !v.is_unspecified() => Some(v),
        _ => None,
    }
}

pub fn local_hostname() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .or_else(|| {
            std::fs::read_to_string("/etc/hostname")
                .ok()
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_else(|| "localhost".to_string())
}
