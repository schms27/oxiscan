//! Saved preferences and favorites (a small JSON file in the user's settings folder).

use crate::scanner::Settings;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FeederKind {
    #[default]
    Range,
    Random,
    List,
}

impl FeederKind {
    pub fn label(&self) -> &'static str {
        match self {
            FeederKind::Range => "IP Range",
            FeederKind::Random => "Random",
            FeederKind::List => "IP List File",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DisplayFilter {
    #[default]
    All,
    Alive,
    OpenPorts,
}

impl DisplayFilter {
    pub fn label(&self) -> &'static str {
        match self {
            DisplayFilter::All => "All",
            DisplayFilter::Alive => "Alive only",
            DisplayFilter::OpenPorts => "Open ports",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct FeederState {
    pub kind: FeederKind,
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

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub settings: Settings,
    pub favorites: Vec<Favorite>,
    pub display: DisplayFilter,
    pub last_feeder: Option<FeederState>,
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
