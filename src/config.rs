//! Saved preferences and favorites (a small JSON file in the user's settings folder).

use crate::net;
use crate::scanner::{self, Feeder, Settings};
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FeederKind {
    #[default]
    Subnet,
    Range,
    Random,
    List,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DisplayFilter {
    All,
    #[default]
    Alive,
    OpenPorts,
}

/// Day or night colours, or whatever the operating system uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemePref {
    #[default]
    System,
    Day,
    Night,
}

impl ThemePref {
    pub fn parse(s: &str) -> Option<ThemePref> {
        match s.to_ascii_lowercase().as_str() {
            "system" => Some(ThemePref::System),
            "day" | "light" => Some(ThemePref::Day),
            "night" | "dark" => Some(ThemePref::Night),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct FeederState {
    pub kind: FeederKind,
    /// Subnet mode: "192.168.1.0/24", a single address or a host name.
    pub cidr: String,
    pub start: String,
    pub end: String,
    pub hostname: String,
    pub random_base: String,
    pub random_prefix: u8,
    pub random_count: String,
    pub list_path: String,
}

impl FeederState {
    /// Turns the target into addresses to scan. `list` caches the addresses of a list
    /// file; it is filled from `list_path` when empty.
    pub fn to_feeder(&self, list: &mut Vec<Ipv4Addr>) -> Result<Feeder, String> {
        match self.kind {
            FeederKind::Subnet => scanner::parse_target(&self.cidr),
            FeederKind::Range => {
                let start: Ipv4Addr = self
                    .start
                    .trim()
                    .parse()
                    .map_err(|_| format!("\"{}\" is not a valid start address", self.start))?;
                let end: Ipv4Addr = self
                    .end
                    .trim()
                    .parse()
                    .map_err(|_| format!("\"{}\" is not a valid end address", self.end))?;
                if u32::from(start) > u32::from(end) {
                    return Err("The start address must not be greater than the end address".into());
                }
                Ok(Feeder::Range { start, end })
            }
            FeederKind::Random => {
                let base: Ipv4Addr =
                    self.random_base.trim().parse().map_err(|_| {
                        format!("\"{}\" is not a valid base address", self.random_base)
                    })?;
                let count: u32 = self
                    .random_count
                    .trim()
                    .parse()
                    .ok()
                    .filter(|c| *c > 0)
                    .ok_or("The number of addresses must be a positive number")?;
                Ok(Feeder::Random {
                    base,
                    prefix: self.random_prefix.min(32),
                    count,
                })
            }
            FeederKind::List => {
                if list.is_empty() {
                    if self.list_path.is_empty() {
                        return Err("Choose a file with IP addresses first".into());
                    }
                    let text = std::fs::read_to_string(&self.list_path)
                        .map_err(|e| format!("Cannot read {}: {}", self.list_path, e))?;
                    *list = scanner::parse_ip_list(&text);
                }
                if list.is_empty() {
                    return Err("No IP addresses were found in the file".into());
                }
                Ok(Feeder::List(list.clone()))
            }
        }
    }
}

/// A short description of what a feeder will scan, e.g. "192.168.1.0/24".
pub fn target_text(f: &FeederState) -> String {
    match f.kind {
        FeederKind::Subnet => f.cidr.trim().to_string(),
        FeederKind::Range => format!("{} – {}", f.start.trim(), f.end.trim()),
        FeederKind::Random => format!(
            "{} random in {}/{}",
            f.random_count.trim(),
            f.random_base.trim(),
            f.random_prefix
        ),
        FeederKind::List => std::path::Path::new(&f.list_path)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "IP list".into()),
    }
}

/// The /24 around this machine's address.
pub fn local_subnet() -> (Option<Ipv4Addr>, String) {
    let ip = net::local_ipv4();
    let o = ip.unwrap_or(Ipv4Addr::new(192, 168, 1, 1)).octets();
    (ip, format!("{}.{}.{}.0/24", o[0], o[1], o[2]))
}

pub fn default_feeder() -> FeederState {
    let ip = net::local_ipv4().unwrap_or(Ipv4Addr::new(192, 168, 1, 1));
    let o = ip.octets();
    FeederState {
        kind: FeederKind::Subnet,
        cidr: local_subnet().1,
        start: format!("{}.{}.{}.0", o[0], o[1], o[2]),
        end: format!("{}.{}.{}.255", o[0], o[1], o[2]),
        hostname: String::new(),
        random_base: ip.to_string(),
        random_prefix: 24,
        random_count: "100".into(),
        list_path: String::new(),
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Favorite {
    pub name: String,
    pub feeder: FeederState,
}

/// One finished scan, shown under "Recent scans".
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecentScan {
    /// Unix time in seconds when the scan finished.
    pub at: u64,
    pub target: String,
    pub feeder: FeederState,
    pub online: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub settings: Settings,
    pub favorites: Vec<Favorite>,
    pub display: DisplayFilter,
    pub last_feeder: Option<FeederState>,
    pub recent: Vec<RecentScan>,
    pub theme: ThemePref,
}

fn path() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("OxiScan").join("config.json"))
}

impl Config {
    pub fn load() -> Config {
        path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Some(p) = path() {
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Ok(s) = serde_json::to_string_pretty(self) {
                let _ = std::fs::write(p, s);
            }
        }
    }
}
