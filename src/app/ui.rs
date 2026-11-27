//! The three panes of the main window (scan setup sidebar, host list, host inspector),
//! the first-launch screen and the keyboard shortcuts.

use super::theme::{self as t, mono, regular, semibold};
use super::{target_text, Action, GoTo, ScannerApp};
use crate::config::{DisplayFilter, FeederKind};
use crate::opener::{ping_cmd, trace_cmd, Opener};
use crate::scanner::{self, PingMethod, ScanResult, Settings};
use crate::services::{self, DeviceKind, PRESETS};
use eframe::egui::{
    self, pos2, vec2, Align, Align2, Color32, CornerRadius, CursorIcon, FontId, Key,
    KeyboardShortcut, Layout, Margin, Modifiers, Pos2, Rect, Response, RichText, Sense, Stroke, Ui,
};
use egui_phosphor::regular as icon;
use std::net::Ipv4Addr;

const SIDEBAR_WIDTH: f32 = 260.0;
const INSPECTOR_WIDTH: f32 = 420.0;

// ------------------------------------------------------------------ small widgets

fn section_label(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).font(semibold(11.0)).color(t::MUTED));
}

fn small(text: impl Into<String>, color: Color32) -> RichText {
    RichText::new(text).font(regular(11.5)).color(color)
}

fn hline(ui: &mut Ui) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(r, CornerRadius::ZERO, t::BORDER);
}

fn paint_logo(p: &egui::Painter, c: Pos2, radius: f32) {
    p.circle_stroke(c, radius - 2.0, Stroke::new(4.0, t::ACCENT));
    p.circle_filled(c, radius * 0.24, t::SKY);
}

/// Paints a filled button into `rect`; `resp` must be the interaction for that rect.
fn paint_filled_button(
    ui: &Ui,
    rect: Rect,
    resp: &Response,
    text: &str,
    radius: u8,
    enabled: bool,
) {
    let fill = if !enabled {
        t::ACCENT.gamma_multiply(0.45)
    } else if resp.is_pointer_button_down_on() {
        t::ACCENT_DARK
    } else if resp.hovered() {
        Color32::from_rgb(0xD9, 0x4E, 0x08)
    } else {
        t::ACCENT
    };
    ui.painter()
        .rect_filled(rect, CornerRadius::same(radius), fill);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        semibold(13.0),
        Color32::WHITE,
    );
}

fn primary_button(
    ui: &mut Ui,
    text: &str,
    size: egui::Vec2,
    radius: u8,
    enabled: bool,
) -> Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(size, sense);
    paint_filled_button(ui, rect, &resp, text, radius, enabled);
    if enabled {
        resp.on_hover_cursor(CursorIcon::PointingHand)
    } else {
        resp
    }
}

fn stop_button(ui: &mut Ui, text: &str, size: egui::Vec2, enabled: bool) -> Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(size, sense);
    let fill = if resp.hovered() {
        t::RED_SOFT
    } else {
        Color32::WHITE
    };
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(8), fill);
    p.rect_stroke(
        rect,
        CornerRadius::same(8),
        Stroke::new(1.0, t::RED_BORDER),
        egui::StrokeKind::Inside,
    );
    let color = if enabled { t::RED_TEXT } else { t::FAINT };
    p.text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        semibold(13.0),
        color,
    );
    resp
}

/// A row of equal-width segments; returns the index that was clicked.
fn segmented(ui: &mut Ui, labels: &[&str], selected: usize) -> Option<usize> {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, 30.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(8), t::SEGMENT_BG);
    let seg_w = (width - 6.0) / labels.len() as f32;
    let mut clicked = None;
    for (i, label) in labels.iter().enumerate() {
        let r = Rect::from_min_size(
            rect.min + vec2(3.0 + i as f32 * seg_w, 3.0),
            vec2(seg_w, 24.0),
        );
        let resp = ui.interact(r, ui.id().with(("segment", i)), Sense::click());
        let active = i == selected;
        if active {
            ui.painter()
                .rect_filled(r, CornerRadius::same(6), Color32::WHITE);
            ui.painter().rect_stroke(
                r,
                CornerRadius::same(6),
                Stroke::new(1.0, t::BORDER),
                egui::StrokeKind::Inside,
            );
        } else if resp.hovered() && ui.is_enabled() {
            ui.painter()
                .rect_filled(r, CornerRadius::same(6), Color32::from_white_alpha(120));
        }
        let (font, color) = if active {
            (semibold(12.0), t::TEXT)
        } else {
            (regular(12.0), t::TEXT_2)
        };
        let color = if ui.is_enabled() { color } else { t::FAINT };
        ui.painter()
            .text(r.center(), Align2::CENTER_CENTER, *label, font, color);
        if resp.clicked() {
            clicked = Some(i);
        }
    }
    clicked
}

fn toggle(ui: &mut Ui, on: &mut bool) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(vec2(30.0, 18.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t_on = ui.ctx().animate_bool(resp.id, *on);
    let fill = if ui.is_enabled() {
        lerp_color(t::INPUT_BORDER, t::ACCENT, t_on)
    } else {
        t::BORDER
    };
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(9), fill);
    let x = egui::lerp(rect.left() + 9.0..=rect.right() - 9.0, t_on);
    p.circle_filled(pos2(x, rect.center().y), 7.0, Color32::WHITE);
    resp
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

fn labeled_row(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(12.5).color(t::TEXT_2));
        ui.with_layout(Layout::right_to_left(Align::Center), add);
    });
}

fn text_input(ui: &mut Ui, text: &mut String, font: FontId, hint: &str) -> Response {
    ui.add(
        egui::TextEdit::singleline(text)
            .font(font)
            .hint_text(hint)
            .desired_width(ui.available_width())
            .margin(Margin::symmetric(10, 7)),
    )
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

/// How many addresses a range scan will actually try.
fn scannable_count(start: Ipv4Addr, end: Ipv4Addr, skip_broadcast: bool) -> u64 {
    let n = services::range_len(start, end);
    if !skip_broadcast {
        return n;
    }
    if n > 1 << 20 {
        return n / 256 * 254;
    }
    (u32::from(start)..=u32::from(end))
        .filter(|v| !matches!(v & 0xFF, 0 | 255))
        .count() as u64
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("{n} {one}")
    } else {
        format!("{n} {many}")
    }
}

fn relative_time(at: u64) -> String {
    let secs = super::unix_now().saturating_sub(at);
    match secs {
        0..=59 => "Just now".into(),
        60..=3599 => format!("{} min ago", secs / 60),
        3600..=86399 => format!("{} h ago", secs / 3600),
        86400..=172799 => "Yesterday".into(),
        _ => format!("{} days ago", secs / 86400),
    }
}

fn fmt_duration(secs: f32) -> String {
    let s = secs.round() as u64;
    if s < 60 {
        format!("{s} s")
    } else {
        format!("{} min {} s", s / 60, s % 60)
    }
}

fn ping_method_label(m: PingMethod) -> &'static str {
    match m {
        PingMethod::Icmp => "ICMP echo",
        PingMethod::Tcp => "TCP port probe",
        PingMethod::Combined => "ICMP, then TCP",
    }
}

fn device_style(kind: DeviceKind) -> (&'static str, Color32, Color32) {
    match kind {
        DeviceKind::Router => (icon::WIFI_HIGH, rgb(0xE0F2FE), rgb(0x0284C7)),
        DeviceKind::Nas => (icon::HARD_DRIVES, t::ACCENT_SOFT, t::ACCENT),
        DeviceKind::Printer => (icon::PRINTER, rgb(0xEEF2FF), rgb(0x4F46E5)),
        DeviceKind::Tv => (icon::TELEVISION, rgb(0xECFDF5), rgb(0x059669)),
        DeviceKind::Phone => (icon::DEVICE_MOBILE, rgb(0xF5F3FF), rgb(0x7C3AED)),
        DeviceKind::Computer => (icon::DESKTOP, rgb(0xE0F2FE), rgb(0x0284C7)),
        DeviceKind::SingleBoard => (icon::CPU, rgb(0xFDF2F8), rgb(0xDB2777)),
        DeviceKind::Unknown => (icon::QUESTION, t::CARD, t::MUTED),
    }
}

fn rgb(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// Status pill text and colours: (label, text colour, background, dot).
fn status_style(r: &ScanResult) -> (&'static str, Color32, Color32, Color32) {
    if r.has_ports() {
        ("Ports open", t::GREEN_TEXT, t::GREEN_SOFT, t::GREEN)
    } else if r.alive {
        ("Online", t::BLUE_TEXT, t::BLUE_SOFT, t::BLUE)
    } else {
        ("No response", t::RED_TEXT, t::RED_SOFT, t::RED)
    }
}

/// Paints a pill whose right edge is at `right`, vertically centred on `cy`.
fn paint_pill(
    ui: &Ui,
    right: f32,
    cy: f32,
    text: &str,
    fg: Color32,
    bg: Color32,
    dot: Option<Color32>,
) {
    let font = semibold(11.0);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, fg);
    let dot_w = if dot.is_some() { 10.0 } else { 0.0 };
    let w = galley.size().x + 18.0 + dot_w;
    let rect = Rect::from_min_size(pos2(right - w, cy - 10.0), vec2(w, 20.0));
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(10), bg);
    let mut x = rect.left() + 9.0;
    if let Some(d) = dot {
        p.circle_filled(pos2(x + 3.0, cy), 3.0, d);
        x += dot_w;
    }
    p.galley(pos2(x, cy - galley.size().y / 2.0), galley, fg);
}

/// The shared host commands, used by the right-click menu and the inspector's More menu.
fn command_items(ui: &mut Ui, ip: Ipv4Addr, scanning: bool, many: bool) -> Option<Action> {
    let mut act = None;
    let s = if many { "s" } else { "" };
    if ui
        .add_enabled(
            !scanning,
            egui::Button::new(format!("{}  Rescan host{s}", icon::ARROW_CLOCKWISE))
                .shortcut_text("Ctrl+R"),
        )
        .clicked()
    {
        act = Some(Action::Rescan);
    }
    if ui
        .add(
            egui::Button::new(format!(
                "{}  Copy IP address{}",
                icon::COPY,
                if many { "es" } else { "" }
            ))
            .shortcut_text("Ctrl+C"),
        )
        .clicked()
    {
        act = Some(Action::CopyIp);
    }
    if ui
        .button(format!("{}  Copy details", icon::CLIPBOARD_TEXT))
        .clicked()
    {
        act = Some(Action::CopyDetails);
    }
    ui.separator();
    let items: [(&str, &str, Opener); 7] = [
        (
            icon::GLOBE,
            "Web browser",
            Opener::Url(format!("http://{ip}/")),
        ),
        (
            icon::LOCK,
            "Web browser (HTTPS)",
            Opener::Url(format!("https://{ip}/")),
        ),
        (icon::WAVEFORM, "Ping", Opener::Terminal(ping_cmd(ip))),
        (icon::PATH, "Trace route", Opener::Terminal(trace_cmd(ip))),
        (
            icon::TERMINAL_WINDOW,
            "SSH",
            Opener::Terminal(format!("ssh {ip}")),
        ),
        (icon::FOLDER_OPEN, "Windows shares", Opener::Shares(ip)),
        (icon::MONITOR, "Remote desktop", Opener::Rdp(ip)),
    ];
    ui.menu_button(format!("{}  Open in", icon::ARROW_SQUARE_OUT), |ui| {
        for (ic, label, opener) in items {
            if ui.button(format!("{ic}  {label}")).clicked() {
                act = Some(Action::Open(opener));
            }
        }
    });
    ui.separator();
    if ui
        .add_enabled(
            !scanning,
            egui::Button::new(format!("{}  Remove from list", icon::TRASH)).shortcut_text("Del"),
        )
        .clicked()
    {
        act = Some(Action::Delete);
    }
    if act.is_some() {
        ui.close();
    }
    act
}

// ------------------------------------------------------------------ list items

enum Item {
    Header(String),
    Host(usize),
    Dead(usize),
    Scanning(String),
    Note(String),
}

impl Item {
    fn height(&self) -> f32 {
        match self {
            Item::Header(_) => 34.0,
            Item::Host(_) => 48.0,
            Item::Dead(_) => 32.0,
            Item::Scanning(_) | Item::Note(_) => 44.0,
        }
    }
}

struct Counts {
    online: usize,
    ports: usize,
    all: usize,
}

impl ScannerApp {
    pub(super) fn draw(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("sidebar")
            .exact_width(SIDEBAR_WIDTH)
            .resizable(false)
            .frame(egui::Frame::new().fill(t::SIDEBAR).inner_margin(Margin {
                left: 20,
                right: 20,
                top: 18,
                bottom: 12,
            }))
            .show(ctx, |ui| self.sidebar(ui));
        egui::SidePanel::right("inspector")
            .exact_width(INSPECTOR_WIDTH)
            .resizable(false)
            .frame(egui::Frame::new().fill(t::INSPECTOR))
            .show(ctx, |ui| self.inspector(ui, ctx));
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(t::BG))
            .show(ctx, |ui| {
                if self.results.is_empty() && self.scan.is_none() {
                    self.empty_state(ui);
                } else {
                    self.results_panel(ui, ctx);
                }
            });
    }

    // -------------------------------------------------------------- sidebar

    fn sidebar(&mut self, ui: &mut Ui) {
        let scanning = self.scan.is_some();
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
            paint_logo(ui.painter(), r.center(), 11.0);
            ui.label(RichText::new("OxiScan").font(semibold(15.0)).color(t::TEXT));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let w = &mut ui.visuals_mut().widgets;
                w.inactive.bg_stroke = Stroke::NONE;
                w.inactive.weak_bg_fill = Color32::TRANSPARENT;
                ui.menu_button(
                    RichText::new(icon::DOTS_THREE).size(18.0).color(t::MUTED),
                    |ui| {
                        if ui.button(format!("{}  Settings…", icon::GEAR)).clicked() {
                            self.show_settings = true;
                            ui.close();
                        }
                        if ui
                            .button(format!("{}  Getting started", icon::QUESTION))
                            .clicked()
                        {
                            self.show_help = true;
                            ui.close();
                        }
                        if ui
                            .button(format!("{}  About OxiScan", icon::INFO))
                            .clicked()
                        {
                            self.show_about = true;
                            ui.close();
                        }
                    },
                );
            });
        });
        ui.add_space(10.0);

        egui::ScrollArea::vertical()
            .id_salt("sidebar_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                section_label(ui, "NEW SCAN");
                ui.add_enabled_ui(!scanning, |ui| {
                    self.target_editor(ui);
                    ui.add_space(4.0);
                    self.ports_editor(ui);
                });
                ui.add_space(2.0);
                let caret = if self.show_advanced {
                    icon::CARET_DOWN
                } else {
                    icon::CARET_RIGHT
                };
                let adv = ui.add(
                    egui::Label::new(
                        RichText::new(format!("Advanced options  {caret}"))
                            .font(semibold(12.0))
                            .color(if self.show_advanced {
                                t::TEXT
                            } else {
                                t::TEXT_2
                            }),
                    )
                    .sense(Sense::click()),
                );
                if adv.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                    self.show_advanced = !self.show_advanced;
                }
                if self.show_advanced {
                    ui.add_enabled_ui(!scanning, |ui| self.advanced_editor(ui));
                }
                ui.add_space(6.0);
                self.start_stop(ui);

                ui.add_space(12.0);
                hline(ui);
                ui.add_space(10.0);
                self.favorites_list(ui);
                ui.add_space(10.0);
                hline(ui);
                ui.add_space(10.0);
                self.recent_list(ui);
            });
    }

    fn target_editor(&mut self, ui: &mut Ui) {
        const KINDS: [FeederKind; 4] = [
            FeederKind::Subnet,
            FeederKind::Range,
            FeederKind::Random,
            FeederKind::List,
        ];
        let cur = KINDS
            .iter()
            .position(|k| *k == self.feeder.kind)
            .unwrap_or(0);
        if let Some(i) = segmented(ui, &["Subnet", "Range", "Random", "List"], cur) {
            self.feeder.kind = KINDS[i];
        }
        ui.add_space(4.0);
        let skip = self.config.settings.skip_broadcast;
        match self.feeder.kind {
            FeederKind::Subnet => {
                text_input(ui, &mut self.feeder.cidr, mono(13.0), "192.168.1.0/24");
                let text = self.feeder.cidr.trim();
                let (hint, color) = if let Some((a, b)) = services::parse_cidr(text) {
                    let here = self
                        .local_ip
                        .is_some_and(|ip| (u32::from(a)..=u32::from(b)).contains(&u32::from(ip)));
                    let n = scannable_count(a, b, skip);
                    (
                        format!(
                            "{}{}",
                            plural(n as usize, "address", "addresses"),
                            if here { " · your current network" } else { "" }
                        ),
                        t::MUTED,
                    )
                } else if text.is_empty() {
                    ("Enter a subnet such as 192.168.1.0/24".into(), t::MUTED)
                } else if text.chars().any(|c| c.is_ascii_alphabetic()) {
                    ("Host name, looked up when the scan starts".into(), t::MUTED)
                } else {
                    ("Not a valid subnet or address".into(), t::RED_TEXT)
                };
                ui.label(small(hint, color));
            }
            FeederKind::Range => {
                ui.label(RichText::new("From").size(12.0).color(t::TEXT_2));
                text_input(ui, &mut self.feeder.start, mono(13.0), "192.168.1.1");
                ui.label(RichText::new("To").size(12.0).color(t::TEXT_2));
                text_input(ui, &mut self.feeder.end, mono(13.0), "192.168.1.254");
                let a = self.feeder.start.trim().parse::<Ipv4Addr>();
                let b = self.feeder.end.trim().parse::<Ipv4Addr>();
                let (hint, color) = match (a, b) {
                    (Ok(a), Ok(b)) if u32::from(a) <= u32::from(b) => (
                        plural(scannable_count(a, b, skip) as usize, "address", "addresses"),
                        t::MUTED,
                    ),
                    (Ok(_), Ok(_)) => ("The start must come before the end".into(), t::RED_TEXT),
                    _ => ("Enter two IPv4 addresses".into(), t::MUTED),
                };
                ui.label(small(hint, color));
            }
            FeederKind::Random => {
                ui.label(RichText::new("Network").size(12.0).color(t::TEXT_2));
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.feeder.random_base)
                            .font(mono(13.0))
                            .desired_width(150.0)
                            .margin(Margin::symmetric(10, 7)),
                    );
                    ui.label(RichText::new("/").color(t::MUTED));
                    ui.add(egui::DragValue::new(&mut self.feeder.random_prefix).range(0..=32));
                });
                ui.label(
                    RichText::new("Addresses to try")
                        .size(12.0)
                        .color(t::TEXT_2),
                );
                text_input(ui, &mut self.feeder.random_count, mono(13.0), "100");
            }
            FeederKind::List => {
                if ui
                    .add_sized(
                        [ui.available_width(), 32.0],
                        egui::Button::new(format!("{}  Choose file…", icon::FOLDER_OPEN)),
                    )
                    .clicked()
                {
                    self.choose_list_file();
                }
                if self.feeder.list_path.is_empty() {
                    ui.label(small("A text file with one or more IP addresses", t::MUTED));
                } else {
                    ui.label(
                        RichText::new(truncate(&target_text(&self.feeder), 30))
                            .font(semibold(12.5))
                            .color(t::TEXT),
                    )
                    .on_hover_text(&self.feeder.list_path);
                    ui.label(small(
                        format!(
                            "{} loaded",
                            plural(self.list_ips.len(), "address", "addresses")
                        ),
                        t::MUTED,
                    ));
                }
            }
        }
    }

    fn ports_editor(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Ports").size(12.0).color(t::TEXT_2));
        let preset = PRESETS
            .iter()
            .position(|(_, p)| *p == self.config.settings.ports.as_str());
        let custom = self.ports_custom || preset.is_none();
        let selected = match preset {
            Some(i) if !custom => PRESETS[i].0,
            _ => "Custom ports",
        };
        egui::ComboBox::from_id_salt("ports_preset")
            .width(ui.available_width())
            .selected_text(selected)
            .show_ui(ui, |ui| {
                for (i, (label, list)) in PRESETS.iter().enumerate() {
                    if ui
                        .selectable_label(!custom && preset == Some(i), *label)
                        .clicked()
                    {
                        self.config.settings.ports = list.to_string();
                        self.ports_custom = false;
                    }
                }
                ui.separator();
                if ui.selectable_label(custom, "Custom…").clicked() {
                    self.ports_custom = true;
                }
            });
        if custom {
            text_input(
                ui,
                &mut self.config.settings.ports,
                mono(12.5),
                "22,80,443,8000-8100",
            );
        }
        match scanner::parse_ports(&self.config.settings.ports) {
            Ok(v) if v.is_empty() => {
                ui.label(small("No ports: hosts are only pinged", t::MUTED));
            }
            Ok(v) => {
                ui.label(small(plural(v.len(), "port", "ports"), t::MUTED));
            }
            Err(e) => {
                ui.label(small(e, t::RED_TEXT));
            }
        }
    }

    fn advanced_editor(&mut self, ui: &mut Ui) {
        let s = &mut self.config.settings;
        ui.add_space(2.0);
        labeled_row(ui, "Threads", |ui| {
            ui.add(egui::DragValue::new(&mut s.max_threads).range(1..=1000));
        });
        ui.label(RichText::new("Ping method").size(12.5).color(t::TEXT_2));
        egui::ComboBox::from_id_salt("ping_method")
            .width(ui.available_width())
            .selected_text(ping_method_label(s.ping_method))
            .show_ui(ui, |ui| {
                for m in [PingMethod::Combined, PingMethod::Icmp, PingMethod::Tcp] {
                    ui.selectable_value(&mut s.ping_method, m, ping_method_label(m));
                }
            });
        labeled_row(ui, "Ping timeout", |ui| {
            ui.add(
                egui::DragValue::new(&mut s.ping_timeout_ms)
                    .range(50..=30000)
                    .speed(10)
                    .suffix(" ms"),
            );
        });
        labeled_row(ui, "Port timeout", |ui| {
            ui.add(
                egui::DragValue::new(&mut s.port_timeout_ms)
                    .range(50..=30000)
                    .speed(10)
                    .suffix(" ms"),
            );
        });
        ui.add_space(2.0);
        labeled_row(ui, "Adapt port timeout to ping", |ui| {
            toggle(ui, &mut s.adapt_port_timeout);
        });
        labeled_row(ui, "Include hosts that don't ping", |ui| {
            toggle(ui, &mut s.scan_dead_hosts);
        });
        labeled_row(ui, "Skip .0 and .255", |ui| {
            toggle(ui, &mut s.skip_broadcast);
        });
        ui.add_space(4.0);
        section_label(ui, "LOOK UP");
        egui::Grid::new("lookups")
            .num_columns(2)
            .spacing([10.0, 4.0])
            .show(ui, |ui| {
                ui.checkbox(&mut s.fetch_hostname, "Hostname");
                ui.checkbox(&mut s.fetch_mac, "MAC address");
                ui.end_row();
                ui.checkbox(&mut s.fetch_netbios, "NetBIOS name");
                ui.checkbox(&mut s.fetch_web, "Web server");
                ui.end_row();
                ui.checkbox(&mut s.fetch_ttl, "TTL");
                ui.end_row();
            });
        ui.vertical_centered(|ui| {
            let reset = ui
                .add(egui::Label::new(small("Reset to defaults", t::MUTED)).sense(Sense::click()));
            if reset.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                let keep_confirm = s.ask_confirm_new_scan;
                *s = Settings::default();
                s.ask_confirm_new_scan = keep_confirm;
                self.ports_custom = false;
            }
        });
    }

    fn start_stop(&mut self, ui: &mut Ui) {
        let size = vec2(ui.available_width(), 38.0);
        let progress = self
            .scan
            .as_ref()
            .map(|h| (h.progress(), h.started.elapsed().as_secs_f32()));
        match progress {
            Some((p, elapsed)) => {
                let label = if self.stopping {
                    format!("{}  Stopping…", icon::STOP)
                } else {
                    format!("{}  Stop scan", icon::STOP)
                };
                if stop_button(ui, &label, size, !self.stopping).clicked() {
                    self.stop();
                }
                let pct = (p * 100.0).floor();
                let text = if p > 0.02 && p < 1.0 && !self.stopping {
                    format!(
                        "{pct:.0}% · about {} left",
                        fmt_duration(elapsed * (1.0 - p) / p)
                    )
                } else {
                    format!("{pct:.0}%")
                };
                ui.vertical_centered(|ui| ui.label(small(text, t::MUTED)));
            }
            None => {
                if primary_button(ui, &format!("{}  Start scan", icon::PLAY), size, 8, true)
                    .on_hover_text("Ctrl+Enter")
                    .clicked()
                {
                    self.request_start();
                }
            }
        }
    }

    fn favorites_list(&mut self, ui: &mut Ui) {
        section_label(ui, "FAVORITES");
        let current = self.current_favorite();
        let mut load = None;
        let mut remove = None;
        for (i, fav) in self.config.favorites.iter().enumerate() {
            let (rect, resp) =
                ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::click());
            let active = current == Some(i);
            let p = ui.painter();
            if active {
                p.rect_filled(rect, CornerRadius::same(6), t::SEGMENT_BG);
            } else if resp.hovered() {
                p.rect_filled(rect, CornerRadius::same(6), t::BORDER.gamma_multiply(0.6));
            }
            let cy = rect.center().y;
            p.text(
                pos2(rect.left() + 8.0, cy),
                Align2::LEFT_CENTER,
                icon::STAR,
                regular(14.0),
                if active { t::ACCENT } else { t::FAINT },
            );
            p.text(
                pos2(rect.left() + 30.0, cy),
                Align2::LEFT_CENTER,
                truncate(&fav.name, 22),
                if active {
                    semibold(13.0)
                } else {
                    regular(13.0)
                },
                if active { t::TEXT } else { t::TEXT_2 },
            );
            let suffix = match fav.feeder.kind {
                FeederKind::Subnet => fav
                    .feeder
                    .cidr
                    .split_once('/')
                    .map(|(_, p)| format!("/{p}"))
                    .unwrap_or_default(),
                FeederKind::Range => "range".into(),
                FeederKind::Random => "random".into(),
                FeederKind::List => "list".into(),
            };
            p.text(
                pos2(rect.right() - 6.0, cy),
                Align2::RIGHT_CENTER,
                suffix,
                mono(11.0),
                t::MUTED,
            );
            let resp = resp
                .on_hover_text(target_text(&fav.feeder))
                .on_hover_cursor(CursorIcon::PointingHand);
            if resp.clicked() {
                load = Some(fav.feeder.clone());
            }
            resp.context_menu(|ui| {
                if ui
                    .button(format!("{}  Remove from favorites", icon::TRASH))
                    .clicked()
                {
                    remove = Some(i);
                    ui.close();
                }
            });
        }
        if self.config.favorites.is_empty() {
            ui.label(small("Save the targets you scan often", t::FAINT));
        }
        let add = ui.add(
            egui::Label::new(small(
                format!("{}  Save current as favorite", icon::PLUS),
                t::MUTED,
            ))
            .sense(Sense::click()),
        );
        if add.on_hover_cursor(CursorIcon::PointingHand).clicked() {
            self.fav_name = String::new();
            self.show_add_fav = true;
        }
        if let Some(f) = load {
            if self.scan.is_none() {
                self.load_feeder(f);
            }
        }
        if let Some(i) = remove {
            self.config.favorites.remove(i);
            self.save_config();
        }
    }

    fn recent_list(&mut self, ui: &mut Ui) {
        section_label(ui, "RECENT SCANS");
        let mut load = None;
        if self.scan.is_some() && self.scan_full {
            let (rect, _) =
                ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::hover());
            let p = ui.painter();
            p.text(
                pos2(rect.left() + 4.0, rect.center().y),
                Align2::LEFT_CENTER,
                "Now",
                regular(13.0),
                t::TEXT_2,
            );
            p.text(
                pos2(rect.right() - 6.0, rect.center().y),
                Align2::RIGHT_CENTER,
                "scanning…",
                regular(12.0),
                t::ACCENT,
            );
        }
        if self.config.recent.is_empty() && self.scan.is_none() {
            ui.label(small("No scans yet", t::FAINT));
        }
        for r in &self.config.recent {
            let (rect, resp) =
                ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::click());
            if resp.hovered() {
                ui.painter().rect_filled(
                    rect,
                    CornerRadius::same(6),
                    t::BORDER.gamma_multiply(0.6),
                );
            }
            let p = ui.painter();
            p.text(
                pos2(rect.left() + 4.0, rect.center().y),
                Align2::LEFT_CENTER,
                relative_time(r.at),
                regular(13.0),
                t::TEXT_2,
            );
            p.text(
                pos2(rect.right() - 6.0, rect.center().y),
                Align2::RIGHT_CENTER,
                format!("{} online", r.online),
                regular(12.0),
                t::MUTED,
            );
            if resp
                .on_hover_text(&r.target)
                .on_hover_cursor(CursorIcon::PointingHand)
                .clicked()
            {
                load = Some(r.feeder.clone());
            }
        }
        if let Some(f) = load {
            if self.scan.is_none() {
                self.load_feeder(f);
            }
        }
    }

    // -------------------------------------------------------------- results

    fn title_and_target(&self) -> (String, Option<String>) {
        let target = target_text(&self.feeder);
        match self.current_favorite() {
            Some(i) => (self.config.favorites[i].name.clone(), Some(target)),
            None => (target, None),
        }
    }

    fn counts(&self) -> Counts {
        Counts {
            online: self.results.iter().filter(|r| r.alive).count(),
            ports: self.results.iter().filter(|r| r.has_ports()).count(),
            all: self.results.len(),
        }
    }

    fn subtitle(&self, target: Option<&str>) -> String {
        if let Some((msg, _)) = &self.notice {
            return msg.clone();
        }
        let prefix = target.map(|t| format!("{t} · ")).unwrap_or_default();
        if let Some(h) = &self.scan {
            let done = h.done.load(std::sync::atomic::Ordering::Relaxed);
            let active = h.active.load(std::sync::atomic::Ordering::Relaxed);
            if !self.scan_full {
                return format!(
                    "{prefix}rescanning {}",
                    plural(h.total as usize, "host", "hosts")
                );
            }
            if self.stopping {
                return format!("{prefix}stopping…");
            }
            return format!(
                "{prefix}scanning… {done} of {} addresses · {}",
                h.total,
                plural(active, "thread", "threads")
            );
        }
        match &self.stats {
            Some(s) => format!(
                "{prefix}{} in {:.1} s",
                if s.stopped { "stopped" } else { "finished" },
                s.seconds
            ),
            None => format!(
                "{prefix}{}",
                plural(self.results.len(), "address", "addresses")
            ),
        }
    }

    fn results_panel(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let (title, target) = self.title_and_target();
        let subtitle = self.subtitle(target.as_deref());
        let counts = self.counts();
        let has_selection = !self.selected.is_empty();

        egui::Frame::new()
            .inner_margin(Margin {
                left: 23,
                right: 23,
                top: 16,
                bottom: 0,
            })
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        ui.label(RichText::new(&title).font(semibold(18.0)).color(t::TEXT));
                        ui.label(RichText::new(&subtitle).size(12.0).color(t::MUTED));
                    });
                    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                        ui.add_enabled_ui(!self.results.is_empty(), |ui| {
                            ui.menu_button(format!("{}  Export", icon::EXPORT), |ui| {
                                if ui.button("All hosts in this view…").clicked() {
                                    self.export(false);
                                    ui.close();
                                }
                                if ui
                                    .add_enabled(
                                        has_selection,
                                        egui::Button::new("Selected hosts…"),
                                    )
                                    .clicked()
                                {
                                    self.export(true);
                                    ui.close();
                                }
                            });
                        });
                    });
                });
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 24.0;
                    for (filter, label, n) in [
                        (DisplayFilter::Alive, "Online", counts.online),
                        (DisplayFilter::OpenPorts, "Open ports", counts.ports),
                        (DisplayFilter::All, "All", counts.all),
                    ] {
                        if self.tab(ui, filter, label, n) {
                            self.config.display = filter;
                            self.view_dirty = true;
                            self.save_config();
                        }
                    }
                    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        if self.search_open {
                            let close = ui.add(
                                egui::Button::new(RichText::new(icon::X).color(t::MUTED))
                                    .frame(false),
                            );
                            let edit = ui.add(
                                egui::TextEdit::singleline(&mut self.search)
                                    .hint_text("Filter hosts…")
                                    .desired_width(170.0)
                                    .margin(Margin::symmetric(8, 4)),
                            );
                            if self.focus_search {
                                edit.request_focus();
                                self.focus_search = false;
                            }
                            if edit.changed() {
                                self.view_dirty = true;
                            }
                            if close.clicked() {
                                self.search_open = false;
                                self.search.clear();
                                self.view_dirty = true;
                            }
                        } else {
                            let b = ui
                                .add(
                                    egui::Button::new(
                                        RichText::new(icon::MAGNIFYING_GLASS)
                                            .size(15.0)
                                            .color(t::MUTED),
                                    )
                                    .frame(false),
                                )
                                .on_hover_text("Filter hosts (Ctrl+F)");
                            if b.clicked() {
                                self.search_open = true;
                                self.focus_search = true;
                            }
                        }
                    });
                });
            });

        // Divider under the tabs, with the scan progress drawn on top of it.
        let (line, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
        ui.painter()
            .rect_filled(line, CornerRadius::ZERO, t::BORDER);
        if let Some(h) = &self.scan {
            let w = line.width() * h.progress().clamp(0.0, 1.0);
            let bar = Rect::from_min_size(line.min - vec2(0.0, 2.0), vec2(w, 3.0));
            ui.painter().rect_filled(bar, CornerRadius::ZERO, t::ACCENT);
        }

        self.host_list(ui, ctx, &counts);
    }

    /// Draws one tab; returns true when it was clicked.
    fn tab(&self, ui: &mut Ui, filter: DisplayFilter, label: &str, n: usize) -> bool {
        let active = self.config.display == filter;
        let text = format!("{label}  {n}");
        let font = if active {
            semibold(13.0)
        } else {
            regular(13.0)
        };
        let color = if active { t::TEXT } else { t::MUTED };
        let galley = ui.painter().layout_no_wrap(text, font, color);
        let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x, 30.0), Sense::click());
        let color = if resp.hovered() && !active {
            t::TEXT_2
        } else {
            color
        };
        ui.painter()
            .galley(rect.min + vec2(0.0, 4.0), galley, color);
        if active {
            let bar = Rect::from_min_size(
                pos2(rect.left(), rect.bottom() - 2.0),
                vec2(rect.width(), 2.0),
            );
            ui.painter().rect_filled(bar, CornerRadius::ZERO, t::ACCENT);
        }
        resp.on_hover_cursor(CursorIcon::PointingHand).clicked()
    }

    fn list_items(&self, counts: &Counts) -> Vec<Item> {
        let mut ports = Vec::new();
        let mut online = Vec::new();
        let mut dead = Vec::new();
        for &i in &self.view {
            let r = &self.results[i];
            if r.has_ports() {
                ports.push(i);
            } else if r.alive {
                online.push(i);
            } else {
                dead.push(i);
            }
        }
        let mut items = Vec::new();
        if !ports.is_empty() {
            items.push(Item::Header(format!("WITH OPEN PORTS · {}", ports.len())));
            items.extend(ports.into_iter().map(Item::Host));
        }
        if !online.is_empty() {
            items.push(Item::Header(format!(
                "ONLINE, NO OPEN PORTS · {}",
                online.len()
            )));
            items.extend(online.into_iter().map(Item::Host));
        }
        if !dead.is_empty() {
            items.push(Item::Header(format!("NO RESPONSE · {}", dead.len())));
            items.extend(dead.into_iter().map(Item::Dead));
        }
        if let Some(h) = &self.scan {
            let cur = h
                .current
                .lock()
                .ok()
                .and_then(|c| *c)
                .map(|ip| format!(" (at {ip})"))
                .unwrap_or_default();
            items.push(Item::Scanning(if self.scan_full {
                format!("Still scanning{cur}…")
            } else {
                "Rescanning…".into()
            }));
        } else if items.is_empty() {
            items.push(Item::Note(if self.search.trim().is_empty() {
                "No hosts in this view".into()
            } else {
                format!("No hosts match \"{}\"", self.search.trim())
            }));
        } else if self.config.display != DisplayFilter::All && self.search.trim().is_empty() {
            let silent = counts.all - counts.online;
            if silent > 0 {
                items.push(Item::Note(format!(
                    "{} didn't respond",
                    plural(silent, "address", "addresses")
                )));
            }
        }
        items
    }

    fn host_list(&mut self, ui: &mut Ui, ctx: &egui::Context, counts: &Counts) {
        let items = self.list_items(counts);
        let total: f32 = items.iter().map(Item::height).sum::<f32>() + 16.0;
        let scanning = self.scan.is_some();
        let scroll_to = self.scroll_to.take();
        let mut clicked: Option<(usize, Modifiers)> = None;
        let mut double: Option<usize> = None;
        let mut context_row: Option<usize> = None;
        let mut action: Option<Action> = None;
        let results = &self.results;
        let selected = &self.selected;
        let local_ip = self.local_ip;
        let many = selected.len() > 1;

        egui::ScrollArea::vertical()
            .id_salt("hosts")
            .auto_shrink([false, false])
            .show_viewport(ui, |ui, viewport| {
                ui.set_min_height(total);
                let origin = ui.max_rect().min;
                let width = ui.max_rect().width();
                let mut y = 6.0;
                for item in &items {
                    let h = item.height();
                    let rect = Rect::from_min_size(origin + vec2(0.0, y), vec2(width, h));
                    if let Item::Host(i) | Item::Dead(i) = item {
                        if scroll_to == Some(*i) {
                            ui.scroll_to_rect(rect, Some(Align::Center));
                        }
                    }
                    let visible = y + h >= viewport.min.y && y <= viewport.max.y;
                    y += h;
                    if !visible {
                        continue;
                    }
                    match item {
                        Item::Header(text) => {
                            ui.painter().text(
                                pos2(rect.left() + 23.0, rect.bottom() - 8.0),
                                Align2::LEFT_BOTTOM,
                                text,
                                semibold(11.0),
                                t::MUTED,
                            );
                        }
                        Item::Note(text) => {
                            ui.painter().text(
                                pos2(rect.left() + 23.0, rect.center().y),
                                Align2::LEFT_CENTER,
                                text,
                                regular(12.0),
                                t::FAINT,
                            );
                        }
                        Item::Scanning(text) => {
                            let c = pos2(rect.left() + 29.0, rect.center().y);
                            paint_spinner(ui, c, 6.5);
                            ui.painter().text(
                                pos2(rect.left() + 44.0, c.y),
                                Align2::LEFT_CENTER,
                                text,
                                regular(12.0),
                                t::MUTED,
                            );
                            ctx.request_repaint();
                        }
                        Item::Host(i) | Item::Dead(i) => {
                            let r = &results[*i];
                            let resp =
                                ui.interact(rect, ui.id().with(("host", r.ip)), Sense::click());
                            let sel = selected.contains(i);
                            if matches!(item, Item::Host(_)) {
                                paint_host_row(ui, rect, &resp, r, sel, local_ip == Some(r.ip));
                            } else {
                                paint_dead_row(ui, rect, &resp, r, sel);
                            }
                            if resp.double_clicked() {
                                double = Some(*i);
                            } else if resp.clicked() {
                                clicked = Some((*i, ui.input(|inp| inp.modifiers)));
                            }
                            if resp.secondary_clicked() {
                                context_row = Some(*i);
                            }
                            resp.context_menu(|ui| {
                                ui.set_min_width(200.0);
                                ui.label(RichText::new(r.ip.to_string()).font(semibold(13.0)));
                                ui.separator();
                                if let Some(a) = command_items(ui, r.ip, scanning, many) {
                                    action = Some(a);
                                }
                            });
                        }
                    }
                }
            });

        if let Some(i) = context_row {
            if !self.selected.contains(&i) {
                self.selected.clear();
                self.selected.insert(i);
                self.anchor = Some(i);
            }
        }
        if let Some((i, m)) = clicked {
            self.click_host(i, m);
        }
        if let Some(i) = double {
            self.selected.clear();
            self.selected.insert(i);
            self.anchor = Some(i);
            if let Some(url) = services::first_web_url(&self.results[i]) {
                self.run_opener(Opener::Url(url));
            }
        }
        if let Some(a) = action {
            self.apply_action(a, ctx);
        }
    }

    fn click_host(&mut self, i: usize, m: Modifiers) {
        let pos = self.view.iter().position(|x| *x == i);
        if m.shift {
            let anchor_pos = self
                .anchor
                .and_then(|a| self.view.iter().position(|x| *x == a));
            if let (Some(a), Some(p)) = (anchor_pos, pos) {
                let (lo, hi) = if a <= p { (a, p) } else { (p, a) };
                if !m.command {
                    self.selected.clear();
                }
                for k in lo..=hi {
                    self.selected.insert(self.view[k]);
                }
                return;
            }
        }
        if m.command {
            if !self.selected.remove(&i) {
                self.selected.insert(i);
            }
        } else {
            self.selected.clear();
            self.selected.insert(i);
        }
        self.anchor = Some(i);
    }

    // -------------------------------------------------------------- first launch

    fn empty_state(&mut self, ui: &mut Ui) {
        let (title, _) = self.title_and_target();
        let target = target_text(&self.feeder);
        let rect = ui.max_rect();
        let c = rect.center() - vec2(0.0, 70.0);
        let p = ui.painter();
        p.circle_stroke(c, 80.0, Stroke::new(2.0, t::CARD));
        p.circle_stroke(c, 50.0, Stroke::new(2.0, t::DIVIDER));
        p.circle_stroke(c, 27.5, Stroke::new(9.0, t::ACCENT));
        p.circle_filled(c, 6.0, t::SKY);
        p.circle_filled(c + vec2(74.0, -38.0), 4.0, t::BLUE_SOFT);
        p.circle_filled(c + vec2(-64.0, 33.0), 3.5, t::GREEN_SOFT);

        let name = if self.current_favorite().is_some() {
            title
        } else {
            "your network".into()
        };
        p.text(
            c + vec2(0.0, 120.0),
            Align2::CENTER_CENTER,
            format!("Ready to scan {name}"),
            semibold(17.0),
            t::TEXT,
        );
        let line1 = match self.feeder.kind {
            FeederKind::Subnet => match services::parse_cidr(&self.feeder.cidr) {
                Some((a, b)) => format!(
                    "OxiScan will check the {} in {target}",
                    plural(
                        scannable_count(a, b, self.config.settings.skip_broadcast) as usize,
                        "address",
                        "addresses"
                    )
                ),
                None => format!("OxiScan will check {target}"),
            },
            _ => format!("OxiScan will check {target}"),
        };
        p.text(
            c + vec2(0.0, 148.0),
            Align2::CENTER_CENTER,
            line1,
            regular(13.0),
            t::MUTED,
        );
        p.text(
            c + vec2(0.0, 168.0),
            Align2::CENTER_CENTER,
            "and list every device that answers.",
            regular(13.0),
            t::MUTED,
        );
        let btn = Rect::from_center_size(c + vec2(0.0, 214.0), vec2(136.0, 38.0));
        let resp = ui
            .interact(btn, ui.id().with("empty_start"), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        paint_filled_button(
            ui,
            btn,
            &resp,
            &format!("{}  Start scan", icon::PLAY),
            8,
            true,
        );
        if resp.clicked() {
            self.request_start();
        }
        ui.painter().text(
            c + vec2(0.0, 252.0),
            Align2::CENTER_CENTER,
            "or press Ctrl+Enter",
            regular(11.5),
            t::FAINT,
        );
    }

    // -------------------------------------------------------------- inspector

    fn inspector(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let Some(i) = self.focused() else {
            let c = ui.max_rect().center();
            let p = ui.painter();
            p.text(
                c - vec2(0.0, 34.0),
                Align2::CENTER_CENTER,
                icon::CURSOR_CLICK,
                regular(28.0),
                t::INPUT_BORDER,
            );
            p.text(
                c,
                Align2::CENTER_CENTER,
                "Select a host to see its details,",
                regular(13.0),
                t::FAINT,
            );
            p.text(
                c + vec2(0.0, 20.0),
                Align2::CENTER_CENTER,
                "services and actions here.",
                regular(13.0),
                t::FAINT,
            );
            return;
        };
        let r = self.results[i].clone();
        let scanning = self.scan.is_some();
        let many = self.selected.len() > 1;
        let mut action: Option<Action> = None;
        let mut open: Option<Opener> = None;
        let mut rescan_this = false;

        egui::TopBottomPanel::bottom("inspector_actions")
            .frame(
                egui::Frame::new()
                    .fill(t::INSPECTOR)
                    .inner_margin(Margin::symmetric(23, 14)),
            )
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let web = services::first_web_url(&r);
                    let b = primary_button(
                        ui,
                        &format!("{}  Open in browser", icon::GLOBE),
                        vec2(140.0, 32.0),
                        6,
                        web.is_some(),
                    );
                    if b.clicked() {
                        open = web.map(Opener::Url);
                    }
                    if ui
                        .add(egui::Button::new("Ping").min_size(vec2(0.0, 32.0)))
                        .clicked()
                    {
                        open = Some(Opener::Terminal(ping_cmd(r.ip)));
                    }
                    if ui
                        .add_enabled(
                            !scanning,
                            egui::Button::new("Rescan host").min_size(vec2(0.0, 32.0)),
                        )
                        .clicked()
                    {
                        rescan_this = true;
                    }
                    ui.menu_button(format!("More  {}", icon::CARET_DOWN), |ui| {
                        ui.set_min_width(200.0);
                        if let Some(a) = command_items(ui, r.ip, scanning, many) {
                            action = Some(a);
                        }
                    });
                });
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(t::INSPECTOR).inner_margin(Margin {
                left: 23,
                right: 23,
                top: 22,
                bottom: 8,
            }))
            .show_inside(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("inspector_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        self.inspector_header(ui, &r);
                        ui.add_space(18.0);
                        self.inspector_cards(ui, &r);
                        ui.add_space(20.0);
                        if let Some(o) = self.inspector_services(ui, &r) {
                            open = Some(o);
                        }
                    });
            });

        if let Some(o) = open {
            self.run_opener(o);
        }
        if rescan_this {
            self.selected.clear();
            self.selected.insert(i);
            self.anchor = Some(i);
            self.rescan_selected(ctx);
        }
        if let Some(a) = action {
            self.apply_action(a, ctx);
        }
    }

    fn inspector_header(&self, ui: &mut Ui, r: &ScanResult) {
        let width = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(vec2(width, 52.0), Sense::hover());
        let gateway_like = r.ip.octets()[3] == 1;
        let kind = if r.alive {
            services::device_kind(r, gateway_like)
        } else {
            DeviceKind::Unknown
        };
        let (glyph, bg, fg) = device_style(kind);
        let tile = Rect::from_min_size(rect.min, vec2(48.0, 48.0));
        let p = ui.painter();
        p.rect_filled(tile, CornerRadius::same(12), bg);
        p.text(
            tile.center(),
            Align2::CENTER_CENTER,
            glyph,
            regular(24.0),
            fg,
        );

        let (label, fg_pill, bg_pill, dot) = status_style(r);
        paint_pill(
            ui,
            rect.right(),
            rect.top() + 12.0,
            label,
            fg_pill,
            bg_pill,
            Some(dot),
        );

        let name = match (&r.hostname, r.alive) {
            (Some(h), _) => h.clone(),
            (None, true) => "Unknown device".into(),
            (None, false) => "No response".into(),
        };
        let name_color = if r.hostname.is_some() {
            t::TEXT
        } else {
            t::MUTED
        };
        let p = ui.painter();
        p.text(
            rect.min + vec2(62.0, 1.0),
            Align2::LEFT_TOP,
            truncate(&name, 20),
            semibold(20.0),
            name_color,
        );
        let mut sub = r.ip.to_string();
        if let Some(m) = &r.mac {
            sub.push_str(" · ");
            sub.push_str(m);
        }
        if local_is(self.local_ip, r.ip) {
            sub.push_str(" · this device");
        }
        p.text(
            rect.min + vec2(62.0, 31.0),
            Align2::LEFT_TOP,
            sub,
            mono(11.5),
            t::MUTED,
        );
    }

    fn inspector_cards(&self, ui: &mut Ui, r: &ScanResult) {
        let s = &self.config.settings;
        let not_looked = "Not looked up";
        let lookup = |v: &Option<String>, enabled: bool, scanned: bool| -> (String, bool) {
            match v {
                Some(x) => (x.clone(), true),
                None if !r.alive => ("—".into(), false),
                None if !enabled || !scanned => (not_looked.into(), false),
                None => ("None".into(), false),
            }
        };
        let ping = match r.ping_ms {
            Some(ms) => (format!("{ms} ms"), true),
            None => ("No reply".into(), false),
        };
        let ttl = match r.ttl {
            Some(v) => (format!("{v} · {}", services::ttl_hint(v)), true),
            None if !r.alive => ("—".into(), false),
            None if !s.fetch_ttl => (not_looked.into(), false),
            None => ("—".into(), false),
        };
        let cards: [(&str, (String, bool), bool); 6] = [
            ("Ping", ping, false),
            ("TTL", ttl, false),
            (
                "MAC address",
                lookup(&r.mac, s.fetch_mac, r.extra_scanned),
                true,
            ),
            (
                "Hostname",
                lookup(&r.hostname, s.fetch_hostname, r.hostname_scanned),
                false,
            ),
            (
                "NetBIOS",
                lookup(&r.netbios, s.fetch_netbios, r.extra_scanned),
                false,
            ),
            (
                "Web server",
                lookup(&r.web, s.fetch_web, r.extra_scanned),
                false,
            ),
        ];
        let gap = 12.0;
        let w = (ui.available_width() - gap) / 2.0;
        for pair in cards.chunks(2) {
            let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 58.0), Sense::hover());
            for (k, (label, (value, known), is_mono)) in pair.iter().enumerate() {
                let card =
                    Rect::from_min_size(row.min + vec2(k as f32 * (w + gap), 0.0), vec2(w, 58.0));
                let p = ui.painter();
                p.rect_filled(card, CornerRadius::same(8), t::CARD);
                p.text(
                    card.min + vec2(12.0, 10.0),
                    Align2::LEFT_TOP,
                    *label,
                    regular(11.0),
                    t::MUTED,
                );
                let (font, color) = if !known {
                    (regular(13.0), t::FAINT)
                } else if *is_mono {
                    (FontId::new(12.5, egui::FontFamily::Monospace), t::TEXT)
                } else {
                    (semibold(14.0), t::TEXT)
                };
                let max_chars = if *is_mono { 22 } else { 21 };
                let resp_text = truncate(value, max_chars);
                p.text(
                    card.min + vec2(12.0, 28.0),
                    Align2::LEFT_TOP,
                    &resp_text,
                    font,
                    color,
                );
                if resp_text != *value {
                    ui.interact(card, ui.id().with(("card", *label)), Sense::hover())
                        .on_hover_text(value);
                }
            }
            ui.add_space(4.0);
        }
    }

    fn inspector_services(&self, ui: &mut Ui, r: &ScanResult) -> Option<Opener> {
        section_label(ui, &format!("SERVICES · {}", r.ports.len()));
        ui.add_space(4.0);
        if !r.alive {
            ui.label(
                RichText::new("This address didn't answer.")
                    .size(13.0)
                    .color(t::FAINT),
            );
            return None;
        }
        if !r.ports_scanned {
            ui.label(
                RichText::new("Ports were not checked for this host.")
                    .size(13.0)
                    .color(t::FAINT),
            );
            return None;
        }
        if r.ports.is_empty() {
            let checked = scanner::parse_ports(&self.config.settings.ports)
                .map(|v| v.len())
                .unwrap_or(0);
            ui.label(
                RichText::new(format!(
                    "No open ports among the {} checked.",
                    plural(checked, "port", "ports")
                ))
                .size(13.0)
                .color(t::FAINT),
            );
            return None;
        }
        let mut open = None;
        for &port in &r.ports {
            let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::hover());
            let cy = row.center().y;
            let p = ui.painter();
            p.text(
                pos2(row.left(), cy),
                Align2::LEFT_CENTER,
                port.to_string(),
                FontId::new(12.5, egui::FontFamily::Monospace),
                t::TEXT,
            );
            let name = services::service_name(port);
            p.text(
                pos2(row.left() + 52.0, cy),
                Align2::LEFT_CENTER,
                if name.is_empty() {
                    "Unknown service"
                } else {
                    name
                },
                regular(13.0),
                if name.is_empty() { t::FAINT } else { t::TEXT_2 },
            );
            p.rect_filled(
                Rect::from_min_size(pos2(row.left(), row.bottom() - 1.0), vec2(row.width(), 1.0)),
                CornerRadius::ZERO,
                t::DIVIDER,
            );
            if let Some((label, opener)) = services::port_action(r.ip, port) {
                let text = if label == "Open" {
                    format!("Open {}", icon::ARROW_SQUARE_OUT)
                } else {
                    label.to_string()
                };
                let galley = ui.painter().layout_no_wrap(text, semibold(12.0), t::ACCENT);
                let link = Rect::from_min_size(
                    pos2(row.right() - galley.size().x, cy - galley.size().y / 2.0),
                    galley.size(),
                );
                let resp = ui
                    .interact(
                        link.expand(4.0),
                        ui.id().with(("port_action", port)),
                        Sense::click(),
                    )
                    .on_hover_cursor(CursorIcon::PointingHand);
                let color = if resp.hovered() {
                    t::ACCENT_DARK
                } else {
                    t::ACCENT
                };
                ui.painter().galley(link.min, galley, color);
                if resp.clicked() {
                    open = Some(opener);
                }
            }
        }
        open
    }

    // -------------------------------------------------------------- keyboard

    pub(super) fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let cmd = |k| KeyboardShortcut::new(Modifiers::COMMAND, k);
        let pressed = |s: KeyboardShortcut| ctx.input_mut(|i| i.consume_shortcut(&s));
        if pressed(cmd(Key::Enter)) {
            if self.scan.is_some() {
                self.stop();
            } else {
                self.request_start();
            }
        }
        if pressed(cmd(Key::F)) {
            self.search_open = true;
            self.focus_search = true;
        }
        if self.search_open && pressed(KeyboardShortcut::new(Modifiers::NONE, Key::Escape)) {
            self.search_open = false;
            self.search.clear();
            self.view_dirty = true;
        }
        if ctx.wants_keyboard_input() {
            return;
        }
        if pressed(cmd(Key::R)) {
            self.rescan_selected(ctx);
        }
        if pressed(cmd(Key::S)) && !self.results.is_empty() {
            self.export(false);
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
        let up = pressed(KeyboardShortcut::new(Modifiers::NONE, Key::ArrowUp));
        let down = pressed(KeyboardShortcut::new(Modifiers::NONE, Key::ArrowDown));
        if (up || down) && !self.view.is_empty() {
            // Move through the hosts in the order the list shows them (grouped).
            let order = self.display_order();
            let cur = self.anchor.and_then(|a| order.iter().position(|x| *x == a));
            let pos = match (cur, down) {
                (None, _) => 0,
                (Some(p), true) => (p + 1).min(order.len() - 1),
                (Some(p), false) => p.saturating_sub(1),
            };
            self.select_one(order[pos]);
        }
    }

    /// The view in the order the grouped list shows it.
    fn display_order(&self) -> Vec<usize> {
        let rank = |r: &ScanResult| {
            if r.has_ports() {
                0
            } else if r.alive {
                1
            } else {
                2
            }
        };
        let mut v = self.view.clone();
        v.sort_by_key(|i| rank(&self.results[*i]));
        v
    }
}

fn local_is(local: Option<Ipv4Addr>, ip: Ipv4Addr) -> bool {
    local == Some(ip)
}

fn paint_spinner(ui: &Ui, c: Pos2, r: f32) {
    let time = ui.input(|i| i.time) as f32;
    let p = ui.painter();
    p.circle_stroke(c, r, Stroke::new(2.0, t::BORDER));
    let start = time * 5.0;
    let points: Vec<Pos2> = (0..=16)
        .map(|k| {
            let a = start + k as f32 / 16.0 * std::f32::consts::FRAC_PI_2 * 1.5;
            c + r * vec2(a.cos(), a.sin())
        })
        .collect();
    p.add(egui::Shape::line(points, Stroke::new(2.0, t::ACCENT)));
}

fn paint_host_row(
    ui: &Ui,
    rect: Rect,
    resp: &Response,
    r: &ScanResult,
    selected: bool,
    this_device: bool,
) {
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, CornerRadius::ZERO, t::ACCENT_SOFT);
        p.rect_filled(
            Rect::from_min_size(rect.min, vec2(3.0, rect.height())),
            CornerRadius::ZERO,
            t::ACCENT,
        );
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::ZERO, rgb(0xF9FAFB));
    }
    let dot = if r.has_ports() { t::GREEN } else { t::BLUE };
    p.circle_filled(pos2(rect.left() + 27.0, rect.center().y), 4.0, dot);
    let (name, color) = match &r.hostname {
        Some(h) => (truncate(h, 34), t::TEXT),
        None => ("Unknown device".to_string(), t::FAINT),
    };
    let name_rect = p.text(
        rect.min + vec2(41.0, 8.0),
        Align2::LEFT_TOP,
        name,
        semibold(13.0),
        color,
    );
    if this_device {
        let galley = p.layout_no_wrap("This device".into(), semibold(10.0), t::MUTED);
        let tag = Rect::from_min_size(
            pos2(name_rect.right() + 8.0, name_rect.top() + 0.5),
            vec2(galley.size().x + 10.0, 17.0),
        );
        p.rect_filled(
            tag,
            CornerRadius::same(4),
            if selected { Color32::WHITE } else { t::CARD },
        );
        p.galley(
            tag.min + vec2(5.0, (17.0 - galley.size().y) / 2.0),
            galley,
            t::MUTED,
        );
    }
    p.text(
        rect.min + vec2(41.0, 27.0),
        Align2::LEFT_TOP,
        r.ip.to_string(),
        mono(11.5),
        t::MUTED,
    );
    let right = rect.right() - 23.0;
    if let Some(ms) = r.ping_ms {
        p.text(
            pos2(right, rect.center().y),
            Align2::RIGHT_CENTER,
            format!("{ms} ms"),
            regular(12.0),
            t::MUTED,
        );
    }
    if r.has_ports() {
        let text = plural(r.ports.len(), "open port", "open ports");
        paint_pill(
            ui,
            right - 60.0,
            rect.center().y,
            &text,
            t::TEXT_2,
            if selected { Color32::WHITE } else { t::CARD },
            None,
        );
    }
}

fn paint_dead_row(ui: &Ui, rect: Rect, resp: &Response, r: &ScanResult, selected: bool) {
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, CornerRadius::ZERO, t::ACCENT_SOFT);
        p.rect_filled(
            Rect::from_min_size(rect.min, vec2(3.0, rect.height())),
            CornerRadius::ZERO,
            t::ACCENT,
        );
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::ZERO, rgb(0xF9FAFB));
    }
    p.circle_filled(
        pos2(rect.left() + 27.0, rect.center().y),
        3.5,
        t::RED.gamma_multiply(0.7),
    );
    p.text(
        pos2(rect.left() + 41.0, rect.center().y),
        Align2::LEFT_CENTER,
        r.ip.to_string(),
        mono(12.0),
        t::MUTED,
    );
    p.text(
        pos2(rect.right() - 23.0, rect.center().y),
        Align2::RIGHT_CENTER,
        "No response",
        regular(12.0),
        t::FAINT,
    );
}
