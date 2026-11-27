//! Pop-up windows: problems, confirm a new scan, save a favorite, settings, help and about.

use super::theme::{self as t, regular, semibold};
use super::{target_text, ScannerApp};
use crate::config::Favorite;
use eframe::egui::{self, Align2, Key, RichText};

fn dialog(title: &str) -> egui::Window<'_> {
    egui::Window::new(RichText::new(title).font(semibold(14.0)).color(t::TEXT))
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .frame(
            egui::Frame::window(&egui::Style::default())
                .fill(t::BG)
                .stroke(egui::Stroke::new(1.0, t::BORDER))
                .corner_radius(10)
                .inner_margin(16),
        )
}

impl ScannerApp {
    pub(super) fn dialogs(&mut self, ctx: &egui::Context) {
        self.problem_dialog(ctx);
        self.confirm_dialog(ctx);
        self.favorite_dialog(ctx);
        self.settings_dialog(ctx);
        self.info_dialogs(ctx);
    }

    fn problem_dialog(&mut self, ctx: &egui::Context) {
        let Some(msg) = self.error.clone() else {
            return;
        };
        let mut close = false;
        dialog("Something went wrong").show(ctx, |ui| {
            ui.set_max_width(360.0);
            ui.label(RichText::new(msg).color(t::TEXT_2));
            ui.add_space(8.0);
            if ui.button("OK").clicked() || ui.input(|i| i.key_pressed(Key::Enter)) {
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
        dialog("Start a new scan?").show(ctx, |ui| {
            ui.label(RichText::new("The current results will be cleared.").color(t::TEXT_2));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Start scan").clicked() || ui.input(|i| i.key_pressed(Key::Enter)) {
                    answer = Some(true);
                }
                if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
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

    fn favorite_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_add_fav {
            return;
        }
        let mut save = false;
        let mut cancel = false;
        let target = target_text(&self.feeder);
        dialog("Save as favorite").show(ctx, |ui| {
            ui.label(
                RichText::new(format!("Target: {target}"))
                    .font(regular(12.0))
                    .color(t::MUTED),
            );
            ui.add_space(4.0);
            let edit = ui.add(
                egui::TextEdit::singleline(&mut self.fav_name)
                    .hint_text("Name, e.g. Home network")
                    .desired_width(280.0)
                    .margin(egui::Margin::symmetric(10, 7)),
            );
            edit.request_focus();
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() || ui.input(|i| i.key_pressed(Key::Enter)) {
                    save = true;
                }
                if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                    cancel = true;
                }
            });
        });
        if save && !self.fav_name.trim().is_empty() {
            let fav = Favorite {
                name: self.fav_name.trim().to_string(),
                feeder: self.feeder.clone(),
            };
            match self.current_favorite() {
                Some(i) => self.config.favorites[i] = fav,
                None => self.config.favorites.push(fav),
            }
            self.save_config();
            self.show_add_fav = false;
        } else if cancel {
            self.show_add_fav = false;
        }
    }

    fn settings_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_settings {
            return;
        }
        let mut close = false;
        let s = &mut self.config.settings;
        dialog("Settings").show(ctx, |ui| {
            ui.label(
                RichText::new("Scan options are in the sidebar under Advanced options.")
                    .font(regular(12.0))
                    .color(t::MUTED),
            );
            ui.add_space(8.0);
            egui::Grid::new("settings_grid")
                .num_columns(2)
                .spacing([16.0, 8.0])
                .show(ui, |ui| {
                    ui.label("Delay between starting threads");
                    ui.add(
                        egui::DragValue::new(&mut s.thread_delay_ms)
                            .range(0..=5000)
                            .suffix(" ms"),
                    );
                    ui.end_row();
                    ui.label("Ping probes per host");
                    ui.add(egui::DragValue::new(&mut s.ping_count).range(1..=10));
                    ui.end_row();
                });
            ui.add_space(4.0);
            ui.checkbox(
                &mut s.ask_confirm_new_scan,
                "Ask before clearing results for a new scan",
            );
            ui.add_space(8.0);
            if ui.button("Done").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                close = true;
            }
        });
        if close {
            self.show_settings = false;
        }
    }

    fn info_dialogs(&mut self, ctx: &egui::Context) {
        if self.show_help {
            let mut close = false;
            dialog("Getting started").show(ctx, |ui| {
                ui.set_max_width(420.0);
                for line in [
                    "1. On the left, choose what to scan: a subnet such as 192.168.1.0/24, a range of addresses, random addresses or a list from a text file. Pick which ports to check.",
                    "2. Press Start scan (or Ctrl+Enter). Hosts appear in the middle as they answer, grouped into those with open ports and those without.",
                    "3. Click a host to see its details and services on the right. Web interfaces, file shares, SSH and Remote Desktop open with one click.",
                    "4. Right-click hosts to copy, rescan or remove them. Ctrl+F filters the list, Export saves it as text, CSV, XML or an IP:Port list.",
                    "5. Save targets you scan often as favorites. Recent scans are listed at the bottom of the sidebar.",
                ] {
                    ui.label(RichText::new(line).color(t::TEXT_2));
                }
                ui.add_space(8.0);
                if ui.button("Close").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                    close = true;
                }
            });
            if close {
                self.show_help = false;
            }
        }

        if self.show_about {
            let mut close = false;
            dialog("About OxiScan").show(ctx, |ui| {
                ui.label(RichText::new("OxiScan").font(semibold(18.0)).color(t::TEXT));
                ui.label(
                    RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION"))).color(t::MUTED),
                );
                ui.add_space(6.0);
                ui.label("A fast network scanner for IP addresses and ports.");
                ui.label("Native app written in Rust: no external dependencies.");
                ui.hyperlink_to(
                    "github.com/schms27/oxiscan",
                    "https://github.com/schms27/oxiscan",
                );
                ui.add_space(8.0);
                if ui.button("Close").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                    close = true;
                }
            });
            if close {
                self.show_about = false;
            }
        }
    }
}
