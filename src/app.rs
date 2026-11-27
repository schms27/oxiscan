//! The main window state and logic. Drawing lives in app/ui.rs (sidebar, results list,
//! inspector), app/dialogs.rs (pop-ups) and app/theme.rs (colours and fonts).

mod demo;
mod dialogs;
mod theme;
mod ui;

pub(crate) use crate::config::target_text;
use crate::config::{
    default_feeder, Config, DisplayFilter, FeederKind, FeederState, RecentScan, ThemePref,
};
use crate::export::{self, cell, COLUMNS};
use crate::net;
use crate::opener::Opener;
use crate::scanner::{self, Feeder, ScanHandle, ScanMsg, ScanResult};
use crate::services;
use eframe::egui;
use std::collections::{HashMap, HashSet};
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub use demo::DemoMode;

const MAX_RECENT: usize = 5;

struct Stats {
    seconds: f64,
    stopped: bool,
}

#[derive(PartialEq)]
enum GoTo {
    Alive,
    Dead,
    Ports,
}

/// Something chosen from a host's right-click menu or the inspector's More menu.
enum Action {
    Rescan,
    Delete,
    CopyIp,
    CopyDetails,
    Open(Opener),
}

pub struct ScannerApp {
    config: Config,
    feeder: FeederState,
    list_ips: Vec<Ipv4Addr>,
    local_ip: Option<Ipv4Addr>,

    results: Vec<ScanResult>,
    by_ip: HashMap<Ipv4Addr, usize>,
    /// Indexes into `results` that pass the tab and search filters, sorted by address.
    view: Vec<usize>,
    view_dirty: bool,
    selected: HashSet<usize>,
    anchor: Option<usize>,
    scroll_to: Option<usize>,

    scan: Option<ScanHandle>,
    scan_full: bool,
    pending_start: bool,
    stopping: bool,
    last_desc: String,
    stats: Option<Stats>,

    search: String,
    search_open: bool,
    focus_search: bool,
    show_advanced: bool,
    ports_custom: bool,
    notice: Option<(String, Instant)>,
    error: Option<String>,
    show_settings: bool,
    show_about: bool,
    show_help: bool,
    show_add_fav: bool,
    fav_name: String,
    confirm_new_scan: bool,

    logo: egui::TextureHandle,

    /// Demo and screenshot runs never write the user's config file.
    no_save: bool,
    screenshot: Option<PathBuf>,
    frames: u32,
    shot_requested: bool,
}

impl ScannerApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        screenshot: Option<PathBuf>,
        demo: Option<DemoMode>,
        theme_override: Option<ThemePref>,
    ) -> Self {
        let mut config = if demo.is_some() {
            Config::default()
        } else {
            Config::load()
        };
        if let Some(t) = theme_override {
            config.theme = t;
        }
        theme::install_fonts(&cc.egui_ctx);
        theme::apply_style(&cc.egui_ctx, egui_theme(config.theme));

        let feeder = config.last_feeder.clone().unwrap_or_else(default_feeder);
        let mut list_ips = Vec::new();
        if feeder.kind == FeederKind::List && !feeder.list_path.is_empty() {
            if let Ok(t) = std::fs::read_to_string(&feeder.list_path) {
                list_ips = scanner::parse_ip_list(&t);
            }
        }
        let no_save = screenshot.is_some() || demo.is_some();
        let mut app = ScannerApp {
            config,
            feeder,
            list_ips,
            local_ip: net::local_ipv4(),
            results: Vec::new(),
            by_ip: HashMap::new(),
            view: Vec::new(),
            view_dirty: true,
            selected: HashSet::new(),
            anchor: None,
            scroll_to: None,
            scan: None,
            scan_full: false,
            pending_start: false,
            stopping: false,
            last_desc: String::new(),
            stats: None,
            search: String::new(),
            search_open: false,
            focus_search: false,
            show_advanced: false,
            ports_custom: false,
            notice: None,
            error: None,
            show_settings: false,
            show_about: false,
            show_help: false,
            show_add_fav: false,
            fav_name: String::new(),
            confirm_new_scan: false,
            logo: load_logo(&cc.egui_ctx),
            no_save,
            screenshot,
            frames: 0,
            shot_requested: false,
        };
        if let Some(mode) = demo {
            app.load_demo(mode);
        }
        app
    }

    fn notify(&mut self, msg: impl Into<String>) {
        self.notice = Some((msg.into(), Instant::now()));
    }

    /// The columns that hold data for the current settings (used by export).
    fn visible_columns(&self) -> Vec<usize> {
        let s = &self.config.settings;
        let mut v = vec![0, 1];
        if s.fetch_ttl {
            v.push(2);
        }
        if s.fetch_hostname {
            v.push(3);
        }
        v.push(4);
        if s.fetch_mac {
            v.push(5);
        }
        if s.fetch_netbios {
            v.push(6);
        }
        if s.fetch_web {
            v.push(7);
        }
        v
    }

    fn passes_filter(&self, r: &ScanResult) -> bool {
        let tab = match self.config.display {
            DisplayFilter::All => true,
            DisplayFilter::Alive => r.alive,
            DisplayFilter::OpenPorts => r.has_ports(),
        };
        if !tab {
            return false;
        }
        let needle = self.search.trim().to_lowercase();
        if needle.is_empty() {
            return true;
        }
        (0..COLUMNS.len()).any(|c| cell(r, c).to_lowercase().contains(&needle))
            || r.ports
                .iter()
                .any(|p| services::service_name(*p).to_lowercase().contains(&needle))
    }

    fn rebuild_view(&mut self) {
        let mut v: Vec<usize> = (0..self.results.len())
            .filter(|i| self.passes_filter(&self.results[*i]))
            .collect();
        let res = &self.results;
        v.sort_by_key(|i| u32::from(res[*i].ip));
        self.view = v;
        self.view_dirty = false;
    }

    fn selected_sorted(&self) -> Vec<usize> {
        self.view
            .iter()
            .copied()
            .filter(|i| self.selected.contains(i))
            .collect()
    }

    /// The host shown in the inspector: the last one clicked, if still selected.
    fn focused(&self) -> Option<usize> {
        match self.anchor {
            Some(a) if self.selected.contains(&a) => Some(a),
            _ => self.selected_sorted().first().copied(),
        }
    }

    fn build_feeder(&mut self) -> Result<Feeder, String> {
        self.feeder.to_feeder(&mut self.list_ips)
    }

    fn request_start(&mut self) {
        if self.scan.is_some() {
            return;
        }
        if !self.results.is_empty() && self.config.settings.ask_confirm_new_scan {
            self.confirm_new_scan = true;
        } else {
            self.pending_start = true;
        }
    }

    fn do_start(&mut self, ctx: &egui::Context) {
        if let Err(e) = scanner::parse_ports(&self.config.settings.ports) {
            self.error = Some(format!("Port list: {e}"));
            return;
        }
        match self.build_feeder() {
            Ok(feeder) => {
                self.results.clear();
                self.by_ip.clear();
                self.selected.clear();
                self.anchor = None;
                self.view_dirty = true;
                self.stopping = false;
                self.stats = None;
                self.config.last_feeder = Some(self.feeder.clone());
                self.save_config();
                let handle =
                    scanner::start_scan(feeder, self.config.settings.clone(), repaint(ctx));
                self.last_desc = handle.description.clone();
                self.scan = Some(handle);
                self.scan_full = true;
            }
            Err(e) => self.error = Some(e),
        }
    }

    fn rescan_selected(&mut self, ctx: &egui::Context) {
        if self.scan.is_some() {
            return;
        }
        let ips: Vec<Ipv4Addr> = self
            .selected_sorted()
            .iter()
            .map(|i| self.results[*i].ip)
            .collect();
        if ips.is_empty() {
            return;
        }
        self.stopping = false;
        let handle = scanner::start_rescan(ips, self.config.settings.clone(), repaint(ctx));
        self.scan = Some(handle);
        self.scan_full = false;
    }

    fn stop(&mut self) {
        if let Some(h) = &self.scan {
            h.stop.store(true, std::sync::atomic::Ordering::Relaxed);
            self.stopping = true;
        }
    }

    fn poll_scan(&mut self) {
        let mut finished = false;
        if let Some(h) = &self.scan {
            while let Ok(msg) = h.rx.try_recv() {
                match msg {
                    ScanMsg::Result(r) => {
                        if let Some(i) = self.by_ip.get(&r.ip) {
                            self.results[*i] = r;
                        } else {
                            self.by_ip.insert(r.ip, self.results.len());
                            self.results.push(r);
                        }
                        self.view_dirty = true;
                    }
                    ScanMsg::Finished => finished = true,
                }
            }
        }
        if finished {
            let h = self.scan.take().unwrap();
            if self.scan_full {
                self.stats = Some(Stats {
                    seconds: h.started.elapsed().as_secs_f64(),
                    stopped: self.stopping,
                });
                let online = self.results.iter().filter(|r| r.alive).count();
                self.config.recent.insert(
                    0,
                    RecentScan {
                        at: unix_now(),
                        target: target_text(&self.feeder),
                        feeder: self.feeder.clone(),
                        online,
                    },
                );
                self.config.recent.truncate(MAX_RECENT);
                self.save_config();
            } else {
                self.notify("Rescan finished");
            }
            self.stopping = false;
        }
    }

    fn delete_selected(&mut self) {
        if self.scan.is_some() {
            return;
        }
        let sel = std::mem::take(&mut self.selected);
        let mut i = 0;
        self.results.retain(|_| {
            let keep = !sel.contains(&i);
            i += 1;
            keep
        });
        self.by_ip = self
            .results
            .iter()
            .enumerate()
            .map(|(i, r)| (r.ip, i))
            .collect();
        self.anchor = None;
        self.view_dirty = true;
    }

    fn copy_ips(&mut self, ctx: &egui::Context) {
        let s = self
            .selected_sorted()
            .iter()
            .map(|i| self.results[*i].ip.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        if !s.is_empty() {
            ctx.copy_text(s);
            self.notify("Copied to the clipboard");
        }
    }

    fn details_text(&self, idx: usize) -> String {
        let r = &self.results[idx];
        (0..COLUMNS.len())
            .map(|c| format!("{}: {}", COLUMNS[c], cell(r, c)))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn copy_details(&mut self, ctx: &egui::Context) {
        let s = self
            .selected_sorted()
            .iter()
            .map(|i| self.details_text(*i))
            .collect::<Vec<_>>()
            .join("\n\n");
        if !s.is_empty() {
            ctx.copy_text(s);
            self.notify("Copied to the clipboard");
        }
    }

    fn run_opener(&mut self, o: Opener) {
        if let Err(e) = o.run() {
            self.error = Some(format!("Could not open: {e}"));
        }
    }

    fn apply_action(&mut self, a: Action, ctx: &egui::Context) {
        match a {
            Action::Rescan => self.rescan_selected(ctx),
            Action::Delete => self.delete_selected(),
            Action::CopyIp => self.copy_ips(ctx),
            Action::CopyDetails => self.copy_details(ctx),
            Action::Open(o) => self.run_opener(o),
        }
    }

    fn select_matching(&mut self, f: impl Fn(&ScanResult) -> bool) {
        self.selected = self
            .view
            .iter()
            .copied()
            .filter(|i| f(&self.results[*i]))
            .collect();
    }

    fn next_pos_after_selection(&self) -> usize {
        self.view
            .iter()
            .rposition(|i| self.selected.contains(i))
            .map(|p| p + 1)
            .unwrap_or(0)
    }

    fn go_to(&mut self, what: GoTo) {
        let start_pos = self.next_pos_after_selection();
        let n = self.view.len();
        for k in 0..n {
            let pos = (start_pos + k) % n;
            let i = self.view[pos];
            let r = &self.results[i];
            let ok = match what {
                GoTo::Alive => r.alive,
                GoTo::Dead => !r.alive,
                GoTo::Ports => r.has_ports(),
            };
            if ok {
                self.select_one(i);
                return;
            }
        }
        self.notify("Nothing found");
    }

    fn select_one(&mut self, i: usize) {
        self.selected.clear();
        self.selected.insert(i);
        self.anchor = Some(i);
        self.scroll_to = Some(i);
    }

    fn export(&mut self, only_selected: bool) {
        let rows: Vec<&ScanResult> = if only_selected {
            self.selected_sorted()
                .iter()
                .map(|i| &self.results[*i])
                .collect()
        } else {
            self.view.iter().map(|i| &self.results[*i]).collect()
        };
        if rows.is_empty() {
            self.error = Some("There is nothing to export".into());
            return;
        }
        let Some(path) = rfd::FileDialog::new()
            .set_title(if only_selected {
                "Export selection"
            } else {
                "Export results"
            })
            .add_filter("Text file", &["txt"])
            .add_filter("CSV file", &["csv"])
            .add_filter("XML file", &["xml"])
            .add_filter("IP:Port list", &["lst"])
            .set_file_name("scan.txt")
            .save_file()
        else {
            return;
        };
        let text = export::render(
            &rows,
            &self.visible_columns(),
            export::Format::from_path(&path),
            &self.last_desc,
        );
        let count = rows.len();
        match std::fs::write(&path, text) {
            Ok(_) => self.notify(format!("Exported {count} hosts")),
            Err(e) => self.error = Some(format!("Could not save the file: {e}")),
        }
    }

    fn choose_list_file(&mut self) {
        if let Some(p) = rfd::FileDialog::new()
            .set_title("Load IP addresses from file")
            .add_filter("Text files", &["txt", "csv", "lst", "log"])
            .add_filter("All files", &["*"])
            .pick_file()
        {
            match std::fs::read_to_string(&p) {
                Ok(t) => {
                    self.list_ips = scanner::parse_ip_list(&t);
                    self.feeder.kind = FeederKind::List;
                    self.feeder.list_path = p.display().to_string();
                }
                Err(e) => self.error = Some(format!("Cannot read the file: {e}")),
            }
        }
    }

    /// Switches the sidebar to a saved target (favorite or recent scan).
    fn load_feeder(&mut self, f: FeederState) {
        self.list_ips.clear();
        if f.kind == FeederKind::List && !f.list_path.is_empty() {
            if let Ok(t) = std::fs::read_to_string(&f.list_path) {
                self.list_ips = scanner::parse_ip_list(&t);
            }
        }
        self.feeder = f;
    }

    /// The favorite whose target is the one currently in the sidebar.
    fn current_favorite(&self) -> Option<usize> {
        let t = target_text(&self.feeder);
        self.config
            .favorites
            .iter()
            .position(|f| target_text(&f.feeder) == t)
    }

    fn save_config(&self) {
        if !self.no_save {
            self.config.save();
        }
    }

    fn handle_screenshot(&mut self, ctx: &egui::Context) {
        let Some(path) = self.screenshot.clone() else {
            return;
        };
        self.frames += 1;
        ctx.request_repaint();
        let mut image = None;
        ctx.input(|i| {
            for e in &i.raw.events {
                if let egui::Event::Screenshot { image: img, .. } = e {
                    image = Some(img.clone());
                }
            }
        });
        if let Some(img) = image {
            let [w, h] = img.size;
            let mut buf = Vec::with_capacity(w * h * 4);
            for p in &img.pixels {
                buf.extend_from_slice(&p.to_srgba_unmultiplied());
            }
            if let Some(dir) = path.parent() {
                if !dir.as_os_str().is_empty() {
                    let _ = std::fs::create_dir_all(dir);
                }
            }
            if let Err(e) =
                image::save_buffer(&path, &buf, w as u32, h as u32, image::ColorType::Rgba8)
            {
                eprintln!("Could not save screenshot: {e}");
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if self.frames >= 120 && !self.shot_requested {
            self.shot_requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
    }
}

impl eframe::App for ScannerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_scan();
        if self.pending_start {
            self.pending_start = false;
            self.do_start(ctx);
        }
        self.handle_shortcuts(ctx);
        if self.view_dirty {
            self.rebuild_view();
        }
        if let Some((_, at)) = &self.notice {
            if at.elapsed() > Duration::from_secs(4) {
                self.notice = None;
            } else {
                ctx.request_repaint_after(Duration::from_millis(500));
            }
        }

        theme::sync(ctx);
        let settings_before = self.config.settings.clone();
        let theme_before = self.config.theme;
        self.draw(ctx);
        self.dialogs(ctx);
        if self.config.theme != theme_before {
            ctx.set_theme(egui_theme(self.config.theme));
            ctx.request_repaint();
        }
        if !same_settings(&settings_before, &self.config.settings)
            || self.config.theme != theme_before
        {
            self.save_config();
        }

        if self.scan.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        self.handle_screenshot(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(h) = &self.scan {
            h.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        if !self.no_save {
            self.config.last_feeder = Some(self.feeder.clone());
            self.config.save();
        }
    }
}

fn same_settings(a: &scanner::Settings, b: &scanner::Settings) -> bool {
    serde_json::to_string(a).ok() == serde_json::to_string(b).ok()
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn repaint(ctx: &egui::Context) -> scanner::Notify {
    let ctx = ctx.clone();
    std::sync::Arc::new(move || ctx.request_repaint())
}

fn egui_theme(t: ThemePref) -> egui::ThemePreference {
    match t {
        ThemePref::System => egui::ThemePreference::System,
        ThemePref::Day => egui::ThemePreference::Light,
        ThemePref::Night => egui::ThemePreference::Dark,
    }
}

/// The app icon as a texture for the sidebar header, scaled down once so it stays sharp.
fn load_logo(ctx: &egui::Context) -> egui::TextureHandle {
    let img = image::load_from_memory(include_bytes!("../assets/logo-256.png"))
        .expect("embedded icon is a valid PNG")
        .resize_exact(64, 64, image::imageops::FilterType::Lanczos3)
        .into_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    let pixels = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
    ctx.load_texture("logo", pixels, egui::TextureOptions::LINEAR)
}
