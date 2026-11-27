//! Saved preferences and favorites (a small JSON file in the user's settings folder).

use crate::scanner::Settings;
use serde::{Deserialize, Serialize};
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
