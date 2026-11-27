//! The main window state and logic. Drawing lives in app/ui.rs and app/dialogs.rs.

mod dialogs;
mod ui;

use crate::config::{Config, DisplayFilter, FeederKind, FeederState};
use crate::export::{self, cell, COLUMNS};
use crate::net;
use crate::opener::Opener;
use crate::scanner::{self, Feeder, ScanHandle, ScanMsg, ScanResult, Settings};
use eframe::egui::{self, Color32};
use std::collections::{HashMap, HashSet};
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::time::Duration;

pub(crate) const COLOR_DEAD: Color32 = Color32::from_rgb(214, 60, 52);
pub(crate) const COLOR_ALIVE: Color32 = Color32::from_rgb(52, 120, 220);
pub(crate) const COLOR_PORTS: Color32 = Color32::from_rgb(40, 170, 70);

struct Stats {
    seconds: f64,
    description: String,
    scanned: usize,
    alive: usize,
    with_ports: usize,
}

#[derive(PartialEq)]
enum GoTo {
    Alive,
    Dead,
    Ports,
}

/// Something chosen from the Commands menu or a row's right-click menu.
enum Action {
    Details,
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

    results: Vec<ScanResult>,
    by_ip: HashMap<Ipv4Addr, usize>,
    view: Vec<usize>,
    view_dirty: bool,
    selected: HashSet<usize>,
    anchor: Option<usize>,
    sort_col: Option<usize>,
    sort_asc: bool,
    scroll_to: Option<usize>,

    scan: Option<ScanHandle>,
    pending_start: bool,
    stopping: bool,
    last_desc: String,
    stats: Option<Stats>,

    status: String,
    error: Option<String>,
    show_prefs: bool,
    prefs_draft: Settings,
    prefs_tab: usize,
    show_fetchers: bool,
    show_stats: bool,
    show_about: bool,
    show_help: bool,
    show_find: bool,
    find_text: String,
    show_add_fav: bool,
    fav_name: String,
    show_manage_fav: bool,
    details: Option<usize>,
    confirm_new_scan: bool,

    screenshot: Option<PathBuf>,
    frames: u32,
    shot_requested: bool,
}

impl ScannerApp {
    pub fn new(cc: &eframe::CreationContext<'_>, screenshot: Option<PathBuf>) -> Self {
        cc.egui_ctx.set_theme(egui::Theme::Light);
        cc.egui_ctx.style_mut(|style| {
            style.spacing.item_spacing = egui::vec2(6.0, 4.0);
            style.spacing.button_padding = egui::vec2(8.0, 3.0);
        });

        let config = Config::load();
        let feeder = config.last_feeder.clone().unwrap_or_else(default_feeder);
        let mut list_ips = Vec::new();
        if feeder.kind == FeederKind::List && !feeder.list_path.is_empty() {
            if let Ok(t) = std::fs::read_to_string(&feeder.list_path) {
                list_ips = scanner::parse_ip_list(&t);
            }
        }
        if feeder.kind != FeederKind::Range {
            cc.egui_ctx
                .send_viewport_cmd(egui::ViewportCommand::Title(format!(
                    "{} - OxiScan",
                    feeder.kind.label()
                )));
        }
        let prefs_draft = config.settings.clone();
        ScannerApp {
            config,
            feeder,
            list_ips,
            results: Vec::new(),
            by_ip: HashMap::new(),
            view: Vec::new(),
            view_dirty: true,
            selected: HashSet::new(),
            anchor: None,
            sort_col: None,
            sort_asc: true,
            scroll_to: None,
            scan: None,
            pending_start: false,
            stopping: false,
            last_desc: String::new(),
            stats: None,
            status: "Ready".into(),
            error: None,
            show_prefs: false,
            prefs_draft,
            prefs_tab: 0,
            show_fetchers: false,
            show_stats: false,
            show_about: false,
            show_help: false,
            show_find: false,
            find_text: String::new(),
            show_add_fav: false,
            fav_name: String::new(),
            show_manage_fav: false,
            details: None,
            confirm_new_scan: false,
            screenshot,
            frames: 0,
            shot_requested: false,
        }
    }

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
        match self.config.display {
            DisplayFilter::All => true,
            DisplayFilter::Alive => r.alive,
            DisplayFilter::OpenPorts => r.has_ports(),
        }
    }

    fn rebuild_view(&mut self) {
        let mut v: Vec<usize> = (0..self.results.len())
            .filter(|i| self.passes_filter(&self.results[*i]))
            .collect();
        if let Some(col) = self.sort_col {
            let res = &self.results;
            let asc = self.sort_asc;
            v.sort_by(|a, b| {
                let (ra, rb) = (&res[*a], &res[*b]);
                let o = match col {
                    0 => u32::from(ra.ip).cmp(&u32::from(rb.ip)),
                    1 => ra
                        .ping_ms
                        .unwrap_or(u32::MAX)
                        .cmp(&rb.ping_ms.unwrap_or(u32::MAX)),
                    2 => ra.ttl.unwrap_or(0).cmp(&rb.ttl.unwrap_or(0)),
                    4 => rb
                        .ports
                        .len()
                        .cmp(&ra.ports.len())
                        .then(ra.ports.cmp(&rb.ports)),
                    c => cell(ra, c).to_lowercase().cmp(&cell(rb, c).to_lowercase()),
                };
                if asc {
                    o
                } else {
                    o.reverse()
                }
            });
        }
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

    fn first_selected_ip(&self) -> Option<Ipv4Addr> {
        self.selected_sorted().first().map(|i| self.results[*i].ip)
    }

    fn build_feeder(&mut self) -> Result<Feeder, String> {
        let f = &self.feeder;
        match f.kind {
            FeederKind::Range => {
                let start: Ipv4Addr = f
                    .start
                    .trim()
                    .parse()
                    .map_err(|_| format!("\"{}\" is not a valid start address", f.start))?;
                let end: Ipv4Addr = f
                    .end
                    .trim()
                    .parse()
                    .map_err(|_| format!("\"{}\" is not a valid end address", f.end))?;
                if u32::from(start) > u32::from(end) {
                    return Err("The start address must not be greater than the end address".into());
                }
                Ok(Feeder::Range { start, end })
            }
            FeederKind::Random => {
                let base: Ipv4Addr =
                    f.random_base.trim().parse().map_err(|_| {
                        format!("\"{}\" is not a valid base address", f.random_base)
                    })?;
                let count: u32 = f
                    .random_count
                    .trim()
                    .parse()
                    .ok()
                    .filter(|c| *c > 0)
                    .ok_or("Count must be a positive number")?;
                Ok(Feeder::Random {
                    base,
                    prefix: f.random_prefix.min(32),
                    count,
                })
            }
            FeederKind::List => {
                if self.list_ips.is_empty() {
                    if f.list_path.is_empty() {
                        return Err("Choose a file with IP addresses first".into());
                    }
                    let text = std::fs::read_to_string(&f.list_path)
                        .map_err(|e| format!("Cannot read {}: {}", f.list_path, e))?;
                    self.list_ips = scanner::parse_ip_list(&text);
                }
                if self.list_ips.is_empty() {
                    return Err("No IP addresses were found in the file".into());
                }
                Ok(Feeder::List(self.list_ips.clone()))
            }
        }
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
                self.details = None;
                self.view_dirty = true;
                self.stopping = false;
                self.stats = None;
                self.config.last_feeder = Some(self.feeder.clone());
                self.save_config();
                let handle = scanner::start_scan(feeder, self.config.settings.clone(), ctx.clone());
                self.last_desc = handle.description.clone();
                self.status = "Scanning...".into();
                self.scan = Some(handle);
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
        let handle = scanner::start_rescan(ips, self.config.settings.clone(), ctx.clone());
        self.status = "Rescanning...".into();
        self.scan = Some(handle);
    }

    fn stop(&mut self) {
        if let Some(h) = &self.scan {
            h.stop.store(true, std::sync::atomic::Ordering::Relaxed);
            self.stopping = true;
            self.status = "Stopping...".into();
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
            let secs = h.started.elapsed().as_secs_f64();
            self.stats = Some(Stats {
                seconds: secs,
                description: h.description.clone(),
                scanned: self.results.len(),
                alive: self.results.iter().filter(|r| r.alive).count(),
                with_ports: self.results.iter().filter(|r| r.has_ports()).count(),
            });
            self.status = if self.stopping {
                "Stopped".into()
            } else {
                "Ready".into()
            };
            self.stopping = false;
            if self.config.settings.show_stats && self.screenshot.is_none() {
                self.show_stats = true;
            }
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
        self.details = None;
        self.anchor = None;
        self.view_dirty = true;
    }

    fn copy_ips(&self, ctx: &egui::Context) {
        let s = self
            .selected_sorted()
            .iter()
            .map(|i| self.results[*i].ip.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        if !s.is_empty() {
            ctx.copy_text(s);
        }
    }

    fn details_text(&self, idx: usize) -> String {
        let r = &self.results[idx];
        (0..COLUMNS.len())
            .map(|c| format!("{}: {}", COLUMNS[c], cell(r, c)))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn copy_details(&self, ctx: &egui::Context) {
        let s = self
            .selected_sorted()
            .iter()
            .map(|i| self.details_text(*i))
            .collect::<Vec<_>>()
            .join("\n\n");
        if !s.is_empty() {
            ctx.copy_text(s);
        }
    }

    fn apply_action(&mut self, a: Action, ctx: &egui::Context) {
        match a {
            Action::Details => self.details = self.selected_sorted().first().copied(),
            Action::Rescan => self.rescan_selected(ctx),
            Action::Delete => self.delete_selected(),
            Action::CopyIp => self.copy_ips(ctx),
            Action::CopyDetails => self.copy_details(ctx),
            Action::Open(o) => {
                if let Err(e) = o.run() {
                    self.error = Some(format!("Could not open: {e}"));
                }
            }
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
                self.select_one(i, pos);
                return;
            }
        }
        self.status = "Nothing found".into();
    }

    fn find_next(&mut self) {
        let needle = self.find_text.trim().to_lowercase();
        if needle.is_empty() {
            return;
        }
        let cols = self.visible_columns();
        let start_pos = self.next_pos_after_selection();
        let n = self.view.len();
        for k in 0..n {
            let pos = (start_pos + k) % n;
            let i = self.view[pos];
            if cols
                .iter()
                .any(|c| cell(&self.results[i], *c).to_lowercase().contains(&needle))
            {
                self.select_one(i, pos);
                return;
            }
        }
        self.status = format!("\"{}\" was not found", self.find_text.trim());
    }

    fn select_one(&mut self, i: usize, pos: usize) {
        self.selected.clear();
        self.selected.insert(i);
        self.anchor = Some(i);
        self.scroll_to = Some(pos);
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
                "Export all"
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
            Ok(_) => self.status = format!("Exported {count} rows"),
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
                    self.status = format!("{} addresses loaded", self.list_ips.len());
                }
                Err(e) => self.error = Some(format!("Cannot read the file: {e}")),
            }
        }
    }

    fn apply_netmask(&mut self, prefix: u8) {
        let base = self
            .feeder
            .start
            .trim()
            .parse::<Ipv4Addr>()
            .ok()
            .or_else(net::local_ipv4);
        if let Some(ip) = base {
            let mask: u32 = if prefix == 0 {
                0
            } else {
                u32::MAX << (32 - prefix as u32)
            };
            let v = u32::from(ip);
            self.feeder.start = Ipv4Addr::from(v & mask).to_string();
            self.feeder.end = Ipv4Addr::from(v | !mask).to_string();
        }
    }

    fn resolve_hostname(&mut self) {
        let name = self.feeder.hostname.trim().to_string();
        if name.is_empty() {
            return;
        }
        match net::resolve_host(&name) {
            Some(ip) => {
                let s = ip.to_string();
                match self.feeder.kind {
                    FeederKind::Random => self.feeder.random_base = s,
                    _ => {
                        self.feeder.kind = FeederKind::Range;
                        self.feeder.start = s.clone();
                        self.feeder.end = s;
                    }
                }
            }
            None => self.error = Some(format!("Could not resolve \"{name}\"")),
        }
    }

    fn feeder_summary(&self) -> String {
        match self.feeder.kind {
            FeederKind::Range => format!("{} - {}", self.feeder.start, self.feeder.end),
            FeederKind::Random => format!(
                "{} random in {}/{}",
                self.feeder.random_count, self.feeder.random_base, self.feeder.random_prefix
            ),
            FeederKind::List => std::path::Path::new(&self.feeder.list_path)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "IP list".into()),
        }
    }

    fn save_config(&self) {
        if self.screenshot.is_none() {
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

        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            self.menu_bar(ui, ctx);
        });
        egui::TopBottomPanel::top("toolbar")
            .frame(
                egui::Frame::side_top_panel(&ctx.style())
                    .inner_margin(egui::Margin::symmetric(8, 2)),
            )
            .show(ctx, |ui| {
                self.toolbar(ui, ctx);
            });
        egui::TopBottomPanel::bottom("status")
            .exact_height(26.0)
            .show(ctx, |ui| {
                self.status_bar(ui);
            });
        egui::CentralPanel::default()
            .frame(
                egui::Frame::central_panel(&ctx.style())
                    .inner_margin(egui::Margin::same(2))
                    .fill(Color32::WHITE),
            )
            .show(ctx, |ui| {
                self.table(ui, ctx);
            });
        self.dialogs(ctx);

        if self.scan.is_some() {
            ctx.request_repaint_after(Duration::from_millis(150));
        }
        self.handle_screenshot(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(h) = &self.scan {
            h.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        if self.screenshot.is_none() {
            self.config.last_feeder = Some(self.feeder.clone());
            self.config.save();
        }
    }
}

fn default_feeder() -> FeederState {
    let ip = net::local_ipv4().unwrap_or(Ipv4Addr::new(192, 168, 1, 1));
    let o = ip.octets();
    FeederState {
        kind: FeederKind::Range,
        start: format!("{}.{}.{}.0", o[0], o[1], o[2]),
        end: format!("{}.{}.{}.255", o[0], o[1], o[2]),
        hostname: net::local_hostname(),
        random_base: ip.to_string(),
        random_prefix: 24,
        random_count: "100".into(),
        list_path: String::new(),
    }
}
