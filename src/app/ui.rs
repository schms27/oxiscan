//! Menu bar, toolbar, results table, status bar and keyboard shortcuts.

use super::{Action, GoTo, ScannerApp, COLOR_ALIVE, COLOR_DEAD, COLOR_PORTS};
use crate::config::{DisplayFilter, FeederKind};
use crate::export::{cell, COLUMNS};
use crate::opener::{ping_cmd, trace_cmd, Opener};
use eframe::egui::{self, Align, Color32, Key, KeyboardShortcut, Modifiers, RichText};
use egui_extras::{Column, TableBuilder};
use std::net::Ipv4Addr;

const NETMASKS: [(u8, &str); 9] = [
    (24, "/24  255.255.255.0"),
    (25, "/25  255.255.255.128"),
    (26, "/26  255.255.255.192"),
    (27, "/27  255.255.255.224"),
    (28, "/28  255.255.255.240"),
    (23, "/23  255.255.254.0"),
    (22, "/22  255.255.252.0"),
    (20, "/20  255.255.240.0"),
    (16, "/16  255.255.0.0"),
];

/// Draws the command items; shared by the Commands menu and the right-click menu.
fn command_items(ui: &mut egui::Ui, ip: Option<Ipv4Addr>, scanning: bool) -> Option<Action> {
    let has = ip.is_some();
    let mut act = None;
    if ui
        .add_enabled(
            has,
            egui::Button::new("Show details").shortcut_text("Enter"),
        )
        .clicked()
    {
        act = Some(Action::Details);
    }
    ui.separator();
    if ui
        .add_enabled(
            has && !scanning,
            egui::Button::new("Rescan IP(s)").shortcut_text("Ctrl+R"),
        )
        .clicked()
    {
        act = Some(Action::Rescan);
    }
    if ui
        .add_enabled(
            has && !scanning,
            egui::Button::new("Delete IP(s)").shortcut_text("Del"),
        )
        .clicked()
    {
        act = Some(Action::Delete);
    }
    ui.separator();
    if ui
        .add_enabled(has, egui::Button::new("Copy IP").shortcut_text("Ctrl+C"))
        .clicked()
    {
        act = Some(Action::CopyIp);
    }
    if ui
        .add_enabled(has, egui::Button::new("Copy details"))
        .clicked()
    {
        act = Some(Action::CopyDetails);
    }
    ui.separator();
    ui.add_enabled_ui(has, |ui| {
        ui.menu_button("Open", |ui| {
            let Some(ip) = ip else { return };
            let items: [(&str, Opener); 7] = [
                ("Web browser", Opener::Url(format!("http://{ip}/"))),
                ("Web browser (HTTPS)", Opener::Url(format!("https://{ip}/"))),
                ("Ping", Opener::Terminal(ping_cmd(ip))),
                ("Trace route", Opener::Terminal(trace_cmd(ip))),
                ("SSH", Opener::Terminal(format!("ssh {ip}"))),
                ("Windows shares", Opener::Shares(ip)),
                ("Remote desktop", Opener::Rdp(ip)),
            ];
            for (label, opener) in items {
                if ui.button(label).clicked() {
                    act = Some(Action::Open(opener));
                }
            }
        });
    });
    if act.is_some() {
        ui.close();
    }
    act
}

impl ScannerApp {
    pub(super) fn menu_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let scanning = self.scan.is_some();
        let has_sel = !self.selected.is_empty();
        let mut action = None;
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("Scan", |ui| {
                if ui.button("Load from file...").clicked() {
                    ui.close();
                    self.choose_list_file();
                }
                let export_all = egui::Button::new("Export all...").shortcut_text("Ctrl+S");
                if ui
                    .add_enabled(!self.results.is_empty(), export_all)
                    .clicked()
                {
                    ui.close();
                    self.export(false);
                }
                if ui
                    .add_enabled(has_sel, egui::Button::new("Export selection..."))
                    .clicked()
                {
                    ui.close();
                    self.export(true);
                }
                ui.separator();
                if ui.button("Quit").clicked() {
                    ui.close();
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button("Go to", |ui| {
                let any = !self.view.is_empty();
                let items = [
                    ("Next alive host", "Ctrl+H", Some(GoTo::Alive)),
                    ("Next open port", "Ctrl+J", Some(GoTo::Ports)),
                    ("Next dead host", "Ctrl+K", Some(GoTo::Dead)),
                ];
                for (label, keys, what) in items {
                    if ui
                        .add_enabled(any, egui::Button::new(label).shortcut_text(keys))
                        .clicked()
                    {
                        ui.close();
                        if let Some(w) = what {
                            self.go_to(w);
                        }
                    }
                }
                ui.separator();
                if ui
                    .add_enabled(any, egui::Button::new("Find...").shortcut_text("Ctrl+F"))
                    .clicked()
                {
                    ui.close();
                    self.show_find = true;
                }
            });
            ui.menu_button("Commands", |ui| {
                let ip = if has_sel {
                    self.first_selected_ip()
                } else {
                    None
                };
                action = command_items(ui, ip, scanning);
            });
            ui.menu_button("Favorites", |ui| {
                if ui.button("Add current...").clicked() {
                    ui.close();
                    self.fav_name = self.feeder_summary();
                    self.show_add_fav = true;
                }
                if ui.button("Manage favorites...").clicked() {
                    ui.close();
                    self.show_manage_fav = true;
                }
                if !self.config.favorites.is_empty() {
                    ui.separator();
                }
                let mut chosen = None;
                for (i, f) in self.config.favorites.iter().enumerate() {
                    if ui.button(&f.name).clicked() {
                        chosen = Some(i);
                    }
                }
                if let Some(i) = chosen {
                    ui.close();
                    self.feeder = self.config.favorites[i].feeder.clone();
                    self.list_ips.clear();
                }
            });
            ui.menu_button("Tools", |ui| {
                if ui.button("Preferences...").clicked() {
                    ui.close();
                    self.prefs_draft = self.config.settings.clone();
                    self.show_prefs = true;
                }
                if ui.button("Fetchers (columns)...").clicked() {
                    ui.close();
                    self.show_fetchers = true;
                }
                ui.menu_button("Selection", |ui| {
                    if ui.button("Alive hosts").clicked() {
                        ui.close();
                        self.select_matching(|r| r.alive);
                    }
                    if ui.button("Dead hosts").clicked() {
                        ui.close();
                        self.select_matching(|r| !r.alive);
                    }
                    if ui.button("With open ports").clicked() {
                        ui.close();
                        self.select_matching(|r| r.has_ports());
                    }
                    if ui.button("Without open ports").clicked() {
                        ui.close();
                        self.select_matching(|r| r.alive && !r.has_ports());
                    }
                    ui.separator();
                    if ui.button("Select all").clicked() {
                        ui.close();
                        self.select_matching(|_| true);
                    }
                    if ui.button("Invert selection").clicked() {
                        ui.close();
                        let old = std::mem::take(&mut self.selected);
                        self.selected = self
                            .view
                            .iter()
                            .copied()
                            .filter(|i| !old.contains(i))
                            .collect();
                    }
                });
                let stats_btn = egui::Button::new("Scan statistics").shortcut_text("Ctrl+T");
                if ui.add_enabled(self.stats.is_some(), stats_btn).clicked() {
                    ui.close();
                    self.show_stats = true;
                }
            });
            ui.menu_button("Help", |ui| {
                if ui.button("Getting started").clicked() {
                    ui.close();
                    self.show_help = true;
                }
                ui.separator();
                if ui.button("About").clicked() {
                    ui.close();
                    self.show_about = true;
                }
            });
        });
        if let Some(a) = action {
            self.apply_action(a, ctx);
        }
    }

    pub(super) fn toolbar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let scanning = self.scan.is_some();
        let label_w = 72.0;
        ui.add_space(4.0);
        ui.add_enabled_ui(!scanning, |ui| {
            ui.horizontal(|ui| {
                match self.feeder.kind {
                    FeederKind::Range => {
                        ui.add_sized([label_w, 20.0], egui::Label::new("IP Range:"));
                        ui.add(
                            egui::TextEdit::singleline(&mut self.feeder.start).desired_width(130.0),
                        );
                        ui.label("to");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.feeder.end).desired_width(130.0),
                        );
                    }
                    FeederKind::Random => {
                        ui.add_sized([label_w, 20.0], egui::Label::new("Base IP:"));
                        ui.add(
                            egui::TextEdit::singleline(&mut self.feeder.random_base)
                                .desired_width(130.0),
                        );
                        ui.label("Count:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.feeder.random_count)
                                .desired_width(60.0),
                        );
                    }
                    FeederKind::List => {
                        ui.add_sized([label_w, 20.0], egui::Label::new("List file:"));
                        let mut p = self.feeder.list_path.clone();
                        ui.add(
                            egui::TextEdit::singleline(&mut p)
                                .desired_width(222.0)
                                .hint_text("a text file with IP addresses"),
                        );
                        if p != self.feeder.list_path {
                            self.feeder.list_path = p;
                            self.list_ips.clear();
                        }
                        if ui.button("Browse...").clicked() {
                            self.choose_list_file();
                        }
                    }
                }
                let before = self.feeder.kind;
                egui::ComboBox::from_id_salt("feeder_kind")
                    .width(120.0)
                    .selected_text(self.feeder.kind.label())
                    .show_ui(ui, |ui| {
                        for k in [FeederKind::Range, FeederKind::Random, FeederKind::List] {
                            ui.selectable_value(&mut self.feeder.kind, k, k.label());
                        }
                    });
                if before != self.feeder.kind {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
                        "{} - OxiScan",
                        self.feeder.kind.label()
                    )));
                }
            });
        });
        ui.horizontal(|ui| {
            ui.add_enabled_ui(!scanning, |ui| {
                ui.add_sized([label_w, 20.0], egui::Label::new("Hostname:"));
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut self.feeder.hostname).desired_width(130.0),
                );
                if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    self.resolve_hostname();
                }
                if ui
                    .button("IP ⬆")
                    .on_hover_text("Look up this hostname and put its address above")
                    .clicked()
                {
                    self.resolve_hostname();
                }
                if self.feeder.kind == FeederKind::List {
                    ui.add_space(4.0);
                    ui.label(format!("{} addresses", self.list_ips.len()));
                } else {
                    let current = if self.feeder.kind == FeederKind::Random {
                        NETMASKS
                            .iter()
                            .find(|(p, _)| *p == self.feeder.random_prefix)
                            .map(|(_, l)| *l)
                            .unwrap_or("Netmask")
                    } else {
                        "Netmask"
                    };
                    egui::ComboBox::from_id_salt("netmask")
                        .width(140.0)
                        .selected_text(current)
                        .show_ui(ui, |ui| {
                            for (p, l) in NETMASKS {
                                if ui.selectable_label(false, l).clicked() {
                                    if self.feeder.kind == FeederKind::Random {
                                        self.feeder.random_prefix = p;
                                    } else {
                                        self.apply_netmask(p);
                                    }
                                }
                            }
                        });
                }
            });
            let (text, color) = if self.stopping {
                ("■  Stopping", Color32::from_rgb(150, 150, 150))
            } else if scanning {
                ("■  Stop", Color32::from_rgb(200, 70, 60))
            } else {
                ("▶  Start", Color32::from_rgb(40, 140, 60))
            };
            let btn = egui::Button::new(RichText::new(text).strong().color(Color32::WHITE))
                .fill(color)
                .min_size(egui::vec2(96.0, 22.0));
            if ui.add_enabled(!self.stopping, btn).clicked() {
                if scanning {
                    self.stop();
                } else {
                    self.request_start();
                }
            }
            if ui
                .add(egui::Button::new("☰").min_size(egui::vec2(26.0, 22.0)))
                .on_hover_text("Preferences")
                .clicked()
            {
                self.prefs_draft = self.config.settings.clone();
                self.show_prefs = true;
            }
        });
        ui.add_space(4.0);
    }

    pub(super) fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            let status = if let Some(h) = &self.scan {
                if self.stopping {
                    "Stopping...".to_string()
                } else {
                    match *h.current.lock().unwrap() {
                        Some(ip) => format!("Scanning: {ip}"),
                        None => "Starting...".into(),
                    }
                }
            } else {
                self.status.clone()
            };
            ui.allocate_ui_with_layout(
                egui::vec2(230.0, 18.0),
                egui::Layout::left_to_right(Align::Center),
                |ui| {
                    ui.set_min_width(230.0);
                    ui.add(egui::Label::new(status).truncate());
                },
            );
            ui.separator();
            let mut display = self.config.display;
            ui.menu_button(format!("Display: {}", display.label()), |ui| {
                for d in [
                    DisplayFilter::All,
                    DisplayFilter::Alive,
                    DisplayFilter::OpenPorts,
                ] {
                    if ui.selectable_value(&mut display, d, d.label()).clicked() {
                        ui.close();
                    }
                }
            });
            if display != self.config.display {
                self.config.display = display;
                self.view_dirty = true;
                self.save_config();
            }
            ui.separator();
            let threads = self
                .scan
                .as_ref()
                .map(|h| h.active.load(std::sync::atomic::Ordering::Relaxed))
                .unwrap_or(0);
            ui.add_sized(
                [80.0, 18.0],
                egui::Label::new(format!("Threads: {threads}")),
            );
            ui.separator();
            let width = (ui.available_width() - 4.0).max(40.0);
            match &self.scan {
                Some(h) => {
                    let p = h.progress();
                    ui.add(
                        egui::ProgressBar::new(p)
                            .desired_width(width)
                            .desired_height(14.0)
                            .text(format!("{:.0}%", p * 100.0)),
                    );
                }
                None => {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(width, 14.0), egui::Sense::hover());
                    ui.painter()
                        .rect_filled(rect, 7.0, ui.visuals().extreme_bg_color);
                }
            }
        });
    }

    pub(super) fn table(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let cols = self.visible_columns();
        let scanning = self.scan.is_some();
        let mut clicked: Option<(usize, usize, Modifiers)> = None;
        let mut double: Option<usize> = None;
        let mut sort_click: Option<usize> = None;
        let mut context_row: Option<usize> = None;
        let mut action: Option<Action> = None;

        let mut tb = TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .sense(egui::Sense::click())
            .cell_layout(egui::Layout::left_to_right(Align::Center))
            .min_scrolled_height(0.0)
            .auto_shrink([false, false]);
        if let Some(pos) = self.scroll_to.take() {
            tb = tb.scroll_to_row(pos, Some(Align::Center));
        }
        for (k, c) in cols.iter().enumerate() {
            let w = match c {
                0 => 130.0,
                1 => 70.0,
                2 => 50.0,
                3 => 190.0,
                4 => 130.0,
                5 => 140.0,
                6 => 190.0,
                _ => 160.0,
            };
            tb = if k + 1 == cols.len() {
                tb.column(Column::remainder().at_least(60.0).clip(true))
            } else {
                tb.column(Column::initial(w).at_least(36.0).clip(true))
            };
        }

        let sort_col = self.sort_col;
        let sort_asc = self.sort_asc;
        let results = &self.results;
        let view = &self.view;
        let selected = &self.selected;
        tb.header(22.0, |mut header| {
            for c in &cols {
                header.col(|ui| {
                    let bg = ui.max_rect().expand2(egui::vec2(4.0, 1.0));
                    ui.painter().rect_filled(bg, 0.0, Color32::from_gray(236));
                    let mut t = COLUMNS[*c].to_string();
                    if sort_col == Some(*c) {
                        t.push_str(if sort_asc { "  ⏶" } else { "  ⏷" });
                    }
                    let r = ui.add(
                        egui::Label::new(RichText::new(t).strong())
                            .sense(egui::Sense::click())
                            .selectable(false),
                    );
                    if r.clicked() {
                        sort_click = Some(*c);
                    }
                });
            }
        })
        .body(|body| {
            body.rows(19.0, view.len(), |mut row| {
                let pos = row.index();
                let i = view[pos];
                let r = &results[i];
                row.set_selected(selected.contains(&i));
                for c in &cols {
                    row.col(|ui| {
                        if *c == 0 {
                            let color = if r.has_ports() {
                                COLOR_PORTS
                            } else if r.alive {
                                COLOR_ALIVE
                            } else {
                                COLOR_DEAD
                            };
                            let (rect, _) = ui
                                .allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                            ui.painter().circle_filled(rect.center(), 5.0, color);
                        }
                        let text = cell(r, *c);
                        let rt = if text.starts_with('[') {
                            RichText::new(text).color(Color32::from_gray(140))
                        } else {
                            RichText::new(text)
                        };
                        ui.add(egui::Label::new(rt).truncate().selectable(false));
                    });
                }
                let resp = row.response();
                if resp.double_clicked() {
                    double = Some(i);
                } else if resp.clicked() {
                    clicked = Some((i, pos, ctx.input(|inp| inp.modifiers)));
                }
                if resp.secondary_clicked() {
                    context_row = Some(i);
                }
                resp.context_menu(|ui| {
                    ui.set_min_width(180.0);
                    ui.label(RichText::new(r.ip.to_string()).strong());
                    ui.separator();
                    if let Some(a) = command_items(ui, Some(r.ip), scanning) {
                        action = Some(a);
                    }
                });
            });
        });

        if let Some(c) = sort_click {
            if self.sort_col == Some(c) {
                self.sort_asc = !self.sort_asc;
            } else {
                self.sort_col = Some(c);
                self.sort_asc = true;
            }
            self.view_dirty = true;
        }
        if let Some(i) = context_row {
            if !self.selected.contains(&i) {
                self.selected.clear();
                self.selected.insert(i);
                self.anchor = Some(i);
            }
        }
        if let Some((i, pos, m)) = clicked {
            if m.shift {
                if let Some(a) = self
                    .anchor
                    .and_then(|a| self.view.iter().position(|x| *x == a))
                {
                    let (lo, hi) = if a <= pos { (a, pos) } else { (pos, a) };
                    if !m.command {
                        self.selected.clear();
                    }
                    for p in lo..=hi {
                        self.selected.insert(self.view[p]);
                    }
                } else {
                    self.selected.insert(i);
                    self.anchor = Some(i);
                }
            } else if m.command {
                if !self.selected.remove(&i) {
                    self.selected.insert(i);
                }
                self.anchor = Some(i);
            } else {
                self.selected.clear();
                self.selected.insert(i);
                self.anchor = Some(i);
            }
        }
        if let Some(i) = double {
            self.selected.clear();
            self.selected.insert(i);
            self.anchor = Some(i);
            self.details = Some(i);
        }
        if let Some(a) = action {
            self.apply_action(a, ctx);
        }
    }

    pub(super) fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() {
            return;
        }
        let cmd = |k| KeyboardShortcut::new(Modifiers::COMMAND, k);
        let pressed = |s: KeyboardShortcut| ctx.input_mut(|i| i.consume_shortcut(&s));
        if pressed(cmd(Key::R)) {
            self.rescan_selected(ctx);
        }
        if pressed(cmd(Key::S)) && !self.results.is_empty() {
            self.export(false);
        }
        if pressed(cmd(Key::F)) {
            self.show_find = true;
        }
        if pressed(cmd(Key::H)) && !self.view.is_empty() {
            self.go_to(GoTo::Alive);
        }
        if pressed(cmd(Key::J)) && !self.view.is_empty() {
            self.go_to(GoTo::Ports);
        }
        if pressed(cmd(Key::K)) && !self.view.is_empty() {
            self.go_to(GoTo::Dead);
        }
        if pressed(cmd(Key::T)) && self.stats.is_some() {
            self.show_stats = true;
        }
        if pressed(cmd(Key::A)) {
            self.select_matching(|_| true);
        }
        let copy = ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Copy)));
        if copy || pressed(cmd(Key::C)) {
            self.copy_ips(ctx);
        }
        if pressed(KeyboardShortcut::new(Modifiers::NONE, Key::Delete)) {
            self.delete_selected();
        }
        if pressed(KeyboardShortcut::new(Modifiers::NONE, Key::Enter)) {
            self.details = self.selected_sorted().first().copied();
        }
        let up = pressed(KeyboardShortcut::new(Modifiers::NONE, Key::ArrowUp));
        let down = pressed(KeyboardShortcut::new(Modifiers::NONE, Key::ArrowDown));
        if (up || down) && !self.view.is_empty() {
            let cur = self
                .anchor
                .and_then(|a| self.view.iter().position(|x| *x == a));
            let pos = match (cur, down) {
                (None, _) => 0,
                (Some(p), true) => (p + 1).min(self.view.len() - 1),
                (Some(p), false) => p.saturating_sub(1),
            };
            let i = self.view[pos];
            self.select_one(i, pos);
        }
    }
}
