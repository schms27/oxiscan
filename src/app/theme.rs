//! Colours, fonts and widget styling for the split-workspace design.

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Every colour the interface uses, in one set per theme.
pub struct Palette {
    pub dark: bool,
    pub accent: Color32,
    pub accent_dark: Color32,
    pub accent_hover: Color32,
    pub accent_soft: Color32,
    pub selection: Color32,
    pub sky: Color32,
    /// Text and icons drawn on the accent colour or on the toggle knob.
    pub on_accent: Color32,

    pub text: Color32,
    pub text_2: Color32,
    pub muted: Color32,
    pub faint: Color32,

    pub bg: Color32,
    pub sidebar: Color32,
    pub inspector: Color32,
    pub card: Color32,
    /// Raised surfaces: the active segment, the stop button, selected chips.
    pub raised: Color32,
    pub hover: Color32,
    pub segment_bg: Color32,
    pub border: Color32,
    pub input_border: Color32,
    pub divider: Color32,

    pub green: Color32,
    pub green_text: Color32,
    pub green_soft: Color32,
    pub blue: Color32,
    pub blue_text: Color32,
    pub blue_soft: Color32,
    pub red: Color32,
    pub red_text: Color32,
    pub red_soft: Color32,
    pub red_border: Color32,
}

const fn rgb(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

pub static LIGHT: Palette = Palette {
    dark: false,
    accent: rgb(0xEA580C),
    accent_dark: rgb(0xC2410C),
    accent_hover: rgb(0xD94E08),
    accent_soft: rgb(0xFFF4ED),
    selection: rgb(0xFDD9C1),
    sky: rgb(0x38BDF8),
    on_accent: Color32::WHITE,
    text: rgb(0x1F2328),
    text_2: rgb(0x374151),
    muted: rgb(0x6B7280),
    faint: rgb(0x9CA3AF),
    bg: Color32::WHITE,
    sidebar: rgb(0xF5F6F8),
    inspector: rgb(0xFCFCFD),
    card: rgb(0xF3F4F6),
    raised: Color32::WHITE,
    hover: rgb(0xF9FAFB),
    segment_bg: rgb(0xE9ECF0),
    border: rgb(0xE5E7EB),
    input_border: rgb(0xD1D5DB),
    divider: rgb(0xEEF0F2),
    green: rgb(0x16A34A),
    green_text: rgb(0x15803D),
    green_soft: rgb(0xDCFCE7),
    blue: rgb(0x2563EB),
    blue_text: rgb(0x1D4ED8),
    blue_soft: rgb(0xDBEAFE),
    red: rgb(0xDC2626),
    red_text: rgb(0xB91C1C),
    red_soft: rgb(0xFEE2E2),
    red_border: rgb(0xFCA5A5),
};

/// Night colours, matched to the dark navy of the app icon.
pub static DARK: Palette = Palette {
    dark: true,
    accent: rgb(0xF97316),
    accent_dark: rgb(0xEA580C),
    accent_hover: rgb(0xFB8A3C),
    accent_soft: rgb(0x3A2418),
    selection: rgb(0x7C3A12),
    sky: rgb(0x38BDF8),
    on_accent: Color32::WHITE,
    text: rgb(0xE6EAF0),
    text_2: rgb(0xC3CAD5),
    muted: rgb(0x8B95A5),
    faint: rgb(0x5F6B7C),
    bg: rgb(0x12161D),
    sidebar: rgb(0x181D26),
    inspector: rgb(0x151A22),
    card: rgb(0x212834),
    raised: rgb(0x2B3341),
    hover: rgb(0x1E2530),
    segment_bg: rgb(0x0E1218),
    border: rgb(0x2A313D),
    input_border: rgb(0x3A4351),
    divider: rgb(0x222934),
    green: rgb(0x22C55E),
    green_text: rgb(0x4ADE80),
    green_soft: rgb(0x14321F),
    blue: rgb(0x3B82F6),
    blue_text: rgb(0x93C5FD),
    blue_soft: rgb(0x172A4A),
    red: rgb(0xEF4444),
    red_text: rgb(0xFCA5A5),
    red_soft: rgb(0x3B1A1C),
    red_border: rgb(0x7F2A2A),
};

static IS_DARK: AtomicBool = AtomicBool::new(false);

/// The palette of the theme shown right now.
pub fn p() -> &'static Palette {
    if IS_DARK.load(Ordering::Relaxed) {
        &DARK
    } else {
        &LIGHT
    }
}

/// Picks the palette for this frame from the theme egui resolved (it follows the
/// system setting when the preference is System).
pub fn sync(ctx: &egui::Context) {
    IS_DARK.store(ctx.theme() == egui::Theme::Dark, Ordering::Relaxed);
}

pub const SEMIBOLD: &str = "semibold";

pub fn regular(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(SEMIBOLD.into()))
}

pub fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

/// Reads the first font file that exists from a list of candidates.
fn load_first(paths: &[&str]) -> Option<Vec<u8>> {
    paths.iter().find_map(|p| std::fs::read(p).ok())
}

/// Font file contents for (regular, semibold, monospace), each if found.
type FontFiles = (Option<Vec<u8>>, Option<Vec<u8>>, Option<Vec<u8>>);

fn system_fonts() -> FontFiles {
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    let win = |f: &str| format!("{windir}\\Fonts\\{f}");
    let (r1, s1, m1, m2) = (
        win("segoeui.ttf"),
        win("seguisb.ttf"),
        win("CascadiaMono.ttf"),
        win("consola.ttf"),
    );
    let regular = load_first(&[
        &r1,
        "/System/Library/Fonts/SFNS.ttf",
        "/usr/share/fonts/truetype/inter/Inter-Regular.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    ]);
    let semibold = load_first(&[
        &s1,
        "/usr/share/fonts/truetype/inter/Inter-SemiBold.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-SemiBold.ttf",
        "/usr/share/fonts/noto/NotoSans-SemiBold.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
    ]);
    let mono = load_first(&[
        &m1,
        &m2,
        "/System/Library/Fonts/SFNSMono.ttf",
        "/System/Library/Fonts/Menlo.ttc",
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
    ]);
    (regular, semibold, mono)
}

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    let (regular, semibold, mono) = system_fonts();

    // Everything egui ships (Ubuntu, emoji, phosphor icons) stays behind our fonts as a
    // fallback for glyphs the system font lacks.
    let fallback = fonts.families[&FontFamily::Proportional].clone();

    let mut prop = fallback.clone();
    if let Some(bytes) = regular {
        fonts
            .font_data
            .insert("ui-regular".into(), Arc::new(FontData::from_owned(bytes)));
        prop.insert(0, "ui-regular".into());
    }
    let mut bold = fallback.clone();
    if let Some(bytes) = semibold {
        fonts
            .font_data
            .insert("ui-semibold".into(), Arc::new(FontData::from_owned(bytes)));
        bold.insert(0, "ui-semibold".into());
    } else if prop.first().map(|s| s.as_str()) == Some("ui-regular") {
        bold.insert(0, "ui-regular".into());
    }
    if let Some(bytes) = mono {
        fonts
            .font_data
            .insert("ui-mono".into(), Arc::new(FontData::from_owned(bytes)));
        if let Some(m) = fonts.families.get_mut(&FontFamily::Monospace) {
            m.insert(0, "ui-mono".into());
        }
    }
    fonts.families.insert(FontFamily::Proportional, prop);
    fonts
        .families
        .insert(FontFamily::Name(SEMIBOLD.into()), bold);
    ctx.set_fonts(fonts);
}

/// Installs the day and night styles; egui switches between them with the theme.
pub fn apply_style(ctx: &egui::Context, pref: egui::ThemePreference) {
    ctx.set_style_of(egui::Theme::Light, style(ctx, &LIGHT));
    ctx.set_style_of(egui::Theme::Dark, style(ctx, &DARK));
    ctx.set_theme(pref);
    sync(ctx);
}

fn style(ctx: &egui::Context, c: &Palette) -> egui::Style {
    let mut style = (*ctx.style()).clone();
    style.visuals = if c.dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    style.text_styles = [
        (TextStyle::Small, regular(11.0)),
        (TextStyle::Body, regular(13.0)),
        (TextStyle::Button, regular(13.0)),
        (TextStyle::Heading, semibold(18.0)),
        (TextStyle::Monospace, mono(12.5)),
    ]
    .into();
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    style.spacing.interact_size.y = 28.0;
    style.spacing.combo_width = 220.0;
    style.spacing.text_edit_width = 220.0;

    let v = &mut style.visuals;
    v.panel_fill = c.bg;
    v.window_fill = c.bg;
    v.window_stroke = Stroke::new(1.0_f32, c.border);
    v.window_corner_radius = CornerRadius::same(10);
    v.menu_corner_radius = CornerRadius::same(8);
    v.extreme_bg_color = if c.dark { c.segment_bg } else { c.bg };
    v.faint_bg_color = c.card;
    v.code_bg_color = c.card;
    v.hyperlink_color = c.accent;
    v.selection.bg_fill = c.selection;
    v.selection.stroke = Stroke::new(1.0_f32, c.accent);
    v.text_cursor.stroke = Stroke::new(1.5_f32, c.accent);
    v.override_text_color = None;

    let r = CornerRadius::same(6);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = c.bg;
    w.noninteractive.weak_bg_fill = c.bg;
    w.noninteractive.bg_stroke = Stroke::new(1.0_f32, c.border);
    w.noninteractive.fg_stroke = Stroke::new(1.0_f32, c.text_2);
    w.noninteractive.corner_radius = r;
    for (state, fill) in [
        (&mut w.inactive, if c.dark { c.card } else { c.bg }),
        (&mut w.hovered, if c.dark { c.raised } else { c.hover }),
        (&mut w.active, if c.dark { c.raised } else { c.card }),
        (&mut w.open, if c.dark { c.raised } else { c.card }),
    ] {
        state.bg_fill = fill;
        state.weak_bg_fill = fill;
        state.corner_radius = r;
        state.fg_stroke = Stroke::new(1.0_f32, c.text);
        state.expansion = 0.0;
    }
    w.inactive.bg_stroke = Stroke::new(1.0_f32, c.input_border);
    w.hovered.bg_stroke = Stroke::new(1.0_f32, c.faint);
    w.active.bg_stroke = Stroke::new(1.0_f32, c.accent);
    w.open.bg_stroke = Stroke::new(1.0_f32, c.faint);
    style
}
