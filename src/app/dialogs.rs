//! Pop-up windows: problems, confirm, preferences, fetchers, statistics, details, find,
//! favorites, help and about.

use super::{ScannerApp, COLOR_DEAD};
use crate::config::Favorite;
use crate::export::{cell, COLUMNS};
use crate::scanner::{self, PingMethod};
use eframe::egui::{self, Color32, Key, RichText};

impl ScannerApp {
    pub(super) fn dialogs(&mut self, ctx: &egui::Context) {
        self.problem_dialog(ctx);
        self.confirm_dialog(ctx);
        self.prefs_dialog(ctx);
        self.fetchers_dialog(ctx);
        self.stats_dialog(ctx);
        self.details_dialog(ctx);
        self.find_dialog(ctx);
        self.favorites_dialogs(ctx);
        self.info_dialogs(ctx);
    }

    fn problem_dialog(&mut self, ctx: &egui::Context) {
        let Some(msg) = self.error.clone() else {
            return;
        };
        let mut close = false;
        egui::Window::new("Problem")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(msg);
                ui.add_space(6.0);
                if ui.button("OK").clicked() {
                    close = true;
                }
            });
        if close {
            self.error = None;
        }
    }

    fn confirm_dialog(&mut self, ctx: &egui::Context) {
        if !self.confirm_new_scan {
            return;
        }
        let mut answer = None;
        egui::Window::new("Start a new scan?")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("The current results will be cleared.");
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("Start").clicked() {
                        answer = Some(true);
                    }
                    if ui.button("Cancel").clicked() {
                        answer = Some(false);
                    }
                });
            });
        if let Some(a) = answer {
            self.confirm_new_scan = false;
            if a {
                self.do_start(ctx);
            }
        }
    }

    fn prefs_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_prefs {
            return;
        }
        let mut open = true;
        let mut apply = false;
        let mut cancel = false;
        let tab = &mut self.prefs_tab;
        let d = &mut self.prefs_draft;
        egui::Window::new("Preferences")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(420.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for (i, t) in ["Scanning", "Ports", "Display"].iter().enumerate() {
                        ui.selectable_value(tab, i, *t);
                    }
                });
                ui.separator();
                match *tab {
                    0 => {
                        egui::Grid::new("prefs_scan")
                            .num_columns(2)
                            .spacing([12.0, 6.0])
                            .show(ui, |ui| {
                                ui.label("Max. threads:");
                                ui.add(egui::DragValue::new(&mut d.max_threads).range(1..=1000));
                                ui.end_row();
                                ui.label("Delay between starting threads (ms):");
                                ui.add(
                                    egui::DragValue::new(&mut d.thread_delay_ms).range(0..=5000),
                                );
                                ui.end_row();
                                ui.label("Pinging method:");
                                egui::ComboBox::from_id_salt("ping_method")
                                    .selected_text(d.ping_method.label())
                                    .show_ui(ui, |ui| {
                                        for m in [
                                            PingMethod::Icmp,
                                            PingMethod::Tcp,
                                            PingMethod::Combined,
                                        ] {
                                            ui.selectable_value(&mut d.ping_method, m, m.label());
                                        }
                                    });
                                ui.end_row();
                                ui.label("Number of ping probes:");
                                ui.add(egui::DragValue::new(&mut d.ping_count).range(1..=10));
                                ui.end_row();
                                ui.label("Ping timeout (ms):");
                                ui.add(
                                    egui::DragValue::new(&mut d.ping_timeout_ms).range(50..=30000),
                                );
                                ui.end_row();
                            });
                        ui.add_space(4.0);
                        ui.checkbox(
                            &mut d.scan_dead_hosts,
                            "Scan dead hosts, which don't reply to pings",
                        );
                        ui.checkbox(
                            &mut d.skip_broadcast,
                            "Skip likely broadcast addresses (.0 and .255)",
                        );
                    }
                    1 => {
                        ui.label("Ports to scan (e.g. 1-1024,3389,8080):");
                        ui.add(
                            egui::TextEdit::multiline(&mut d.ports)
                                .desired_rows(2)
                                .desired_width(380.0),
                        );
                        match scanner::parse_ports(&d.ports) {
                            Ok(v) => ui.label(
                                RichText::new(format!("{} ports", v.len()))
                                    .color(Color32::from_gray(110)),
                            ),
                            Err(e) => ui.label(RichText::new(e).color(COLOR_DEAD)),
                        };
                        ui.add_space(4.0);
                        egui::Grid::new("prefs_ports")
                            .num_columns(2)
                            .spacing([12.0, 6.0])
                            .show(ui, |ui| {
                                ui.label("Port connect timeout (ms):");
                                ui.add(
                                    egui::DragValue::new(&mut d.port_timeout_ms).range(50..=30000),
                                );
                                ui.end_row();
                            });
                        ui.checkbox(
                            &mut d.adapt_port_timeout,
                            "Adapt timeout to the host's ping time",
                        );
                    }
                    _ => {
                        ui.checkbox(&mut d.show_stats, "Show statistics when a scan finishes");
                        ui.checkbox(
                            &mut d.ask_confirm_new_scan,
                            "Ask before clearing results for a new scan",
                        );
                    }
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        apply = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        if apply {
            if let Err(e) = scanner::parse_ports(&self.prefs_draft.ports) {
                self.error = Some(format!("Port list: {e}"));
            } else {
                self.config.settings = self.prefs_draft.clone();
                self.save_config();
                self.show_prefs = false;
            }
        } else if cancel || !open {
            self.show_prefs = false;
        }
    }

    fn fetchers_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_fetchers {
            return;
        }
        let mut open = true;
        let before = format!("{:?}", self.config.settings);
        let s = &mut self.config.settings;
        egui::Window::new("Fetchers")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Choose what to find out about each host:");
                ui.add_space(4.0);
                ui.add_enabled(
                    false,
                    egui::Checkbox::new(&mut true, "IP and Ping (always)"),
                );
                ui.add_enabled(
                    false,
                    egui::Checkbox::new(&mut true, "Ports (set the list in Preferences)"),
                );
                ui.checkbox(&mut s.fetch_hostname, "Hostname");
                ui.checkbox(&mut s.fetch_ttl, "TTL");
                ui.checkbox(&mut s.fetch_mac, "MAC Address (local network only)");
                ui.checkbox(&mut s.fetch_netbios, "NetBIOS Info");
                ui.checkbox(&mut s.fetch_web, "Web detect (server on port 80)");
                ui.add_space(4.0);
                ui.label(
                    RichText::new("Changes apply to the next scan.").color(Color32::from_gray(110)),
                );
            });
        if before != format!("{:?}", self.config.settings) {
            self.save_config();
        }
        if !open {
            self.show_fetchers = false;
        }
    }

    fn stats_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_stats {
            return;
        }
        let mut open = true;
        if let Some(s) = &self.stats {
            egui::Window::new("Scan statistics")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    egui::Grid::new("stats")
                        .num_columns(2)
                        .spacing([16.0, 4.0])
                        .show(ui, |ui| {
                            let avg = if s.scanned > 0 {
                                s.seconds / s.scanned as f64
                            } else {
                                0.0
                            };
                            let rows = [
                                ("Total time:", format!("{:.2} sec", s.seconds)),
                                ("Average time per host:", format!("{avg:.3} sec")),
                                ("Scanned:", s.description.clone()),
                                ("Hosts scanned:", s.scanned.to_string()),
                                ("Hosts alive:", s.alive.to_string()),
                                ("With open ports:", s.with_ports.to_string()),
                            ];
                            for (k, v) in rows {
                                ui.label(k);
                                ui.label(v);
                                ui.end_row();
                            }
                        });
                });
        }
        if !open || self.stats.is_none() {
            self.show_stats = false;
        }
    }

    fn details_dialog(&mut self, ctx: &egui::Context) {
        let Some(idx) = self.details else { return };
        if idx >= self.results.len() {
            self.details = None;
            return;
        }
        let mut open = true;
        let text = self.details_text(idx);
        let r = &self.results[idx];
        egui::Window::new("IP details")
            .id(egui::Id::new("details_window"))
            .open(&mut open)
            .collapsible(false)
            .default_width(340.0)
            .show(ctx, |ui| {
                ui.label(RichText::new(r.ip.to_string()).strong().size(16.0));
                ui.separator();
                egui::Grid::new("details_grid")
                    .num_columns(2)
                    .spacing([14.0, 4.0])
                    .show(ui, |ui| {
                        ui.label("Status:");
                        ui.label(if r.has_ports() {
                            "Alive, with open ports"
                        } else if r.alive {
                            "Alive"
                        } else {
                            "Dead (no reply)"
                        });
                        ui.end_row();
                        for (c, name) in COLUMNS.iter().enumerate().skip(1) {
                            ui.label(format!("{name}:"));
                            ui.add(egui::Label::new(cell(r, c)).wrap());
                            ui.end_row();
                        }
                    });
                ui.add_space(6.0);
                if ui.button("Copy to clipboard").clicked() {
                    ctx.copy_text(text.clone());
                }
            });
        if !open {
            self.details = None;
        }
    }

    fn find_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_find {
            return;
        }
        let mut open = true;
        let mut go = false;
        egui::Window::new("Find")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let r = ui
                        .add(egui::TextEdit::singleline(&mut self.find_text).desired_width(200.0));
                    if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        go = true;
                        r.request_focus();
                    }
                    if ui.button("Find next").clicked() {
                        go = true;
                    }
                });
            });
        if go {
            self.find_next();
        }
        if !open {
            self.show_find = false;
        }
    }

    fn favorites_dialogs(&mut self, ctx: &egui::Context) {
        if self.show_add_fav {
            let mut open = true;
            let mut add = false;
            egui::Window::new("Add to favorites")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Name:");
                    ui.add(egui::TextEdit::singleline(&mut self.fav_name).desired_width(240.0));
                    if ui.button("Add").clicked() {
                        add = true;
                    }
                });
            if add && !self.fav_name.trim().is_empty() {
                self.config.favorites.push(Favorite {
                    name: self.fav_name.trim().to_string(),
                    feeder: self.feeder.clone(),
                });
                self.save_config();
                self.show_add_fav = false;
            } else if !open {
                self.show_add_fav = false;
            }
        }

        if self.show_manage_fav {
            let mut open = true;
            let mut remove = None;
            let mut up = None;
            egui::Window::new("Manage favorites")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    if self.config.favorites.is_empty() {
                        ui.label("No favorites yet. Use Favorites → Add current.");
                    }
                    for (i, f) in self.config.favorites.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut f.name).desired_width(200.0));
                            if ui.small_button("⏶").on_hover_text("Move up").clicked() && i > 0 {
                                up = Some(i);
                            }
                            if ui.small_button("✖").on_hover_text("Delete").clicked() {
                                remove = Some(i);
                            }
                        });
                    }
                });
            if let Some(i) = remove {
                self.config.favorites.remove(i);
            }
            if let Some(i) = up {
                self.config.favorites.swap(i, i - 1);
            }
            if !open {
                self.show_manage_fav = false;
                self.save_config();
            }
        }
    }

    fn info_dialogs(&mut self, ctx: &egui::Context) {
        if self.show_help {
            let mut open = true;
            egui::Window::new("Getting started")
                .open(&mut open)
                .collapsible(false)
                .default_width(440.0)
                .show(ctx, |ui| {
                    ui.label("1. Enter a range of addresses (or pick a netmask to fill it in from the start address), choose Random, or load a list from a text file.");
                    ui.label("2. Press Start. Each address is pinged; hosts that answer are checked further: name, open ports and whatever else is turned on under Tools → Fetchers.");
                    ui.label("3. Colours: red = no reply, blue = alive, green = alive with open ports.");
                    ui.label("4. Right-click a row (or use the Commands menu) to see details, copy, rescan, delete or open the host in a browser, ping or remote desktop.");
                    ui.label("5. Save results from Scan → Export as text, CSV, XML or an IP:Port list.");
                    ui.add_space(4.0);
                    ui.label(RichText::new("Tip: set the ports to scan under Tools → Preferences → Ports.").italics());
                });
            if !open {
                self.show_help = false;
            }
        }

        if self.show_about {
            let mut open = true;
            egui::Window::new("About")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.heading("OxiScan");
                    ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
                    ui.add_space(4.0);
                    ui.label("A fast network scanner for IP addresses and ports.");
                    ui.label("Native app written in Rust: no external dependencies.");
                });
            if !open {
                self.show_about = false;
            }
        }
    }
}
