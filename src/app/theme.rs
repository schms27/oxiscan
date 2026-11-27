//! Colours, fonts and widget styling for the split-workspace design.

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle,
};
use std::sync::Arc;

pub const ACCENT: Color32 = Color32::from_rgb(0xEA, 0x58, 0x0C);
pub const ACCENT_DARK: Color32 = Color32::from_rgb(0xC2, 0x41, 0x0C);
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(0xFF, 0xF4, 0xED);
pub const SKY: Color32 = Color32::from_rgb(0x38, 0xBD, 0xF8);

pub const TEXT: Color32 = Color32::from_rgb(0x1F, 0x23, 0x28);
pub const TEXT_2: Color32 = Color32::from_rgb(0x37, 0x41, 0x51);
pub const MUTED: Color32 = Color32::from_rgb(0x6B, 0x72, 0x80);
pub const FAINT: Color32 = Color32::from_rgb(0x9C, 0xA3, 0xAF);

pub const BG: Color32 = Color32::WHITE;
pub const SIDEBAR: Color32 = Color32::from_rgb(0xF5, 0xF6, 0xF8);
pub const INSPECTOR: Color32 = Color32::from_rgb(0xFC, 0xFC, 0xFD);
pub const CARD: Color32 = Color32::from_rgb(0xF3, 0xF4, 0xF6);
pub const SEGMENT_BG: Color32 = Color32::from_rgb(0xE9, 0xEC, 0xF0);
pub const BORDER: Color32 = Color32::from_rgb(0xE5, 0xE7, 0xEB);
pub const INPUT_BORDER: Color32 = Color32::from_rgb(0xD1, 0xD5, 0xDB);
pub const DIVIDER: Color32 = Color32::from_rgb(0xEE, 0xF0, 0xF2);

pub const GREEN: Color32 = Color32::from_rgb(0x16, 0xA3, 0x4A);
pub const GREEN_TEXT: Color32 = Color32::from_rgb(0x15, 0x80, 0x3D);
pub const GREEN_SOFT: Color32 = Color32::from_rgb(0xDC, 0xFC, 0xE7);
pub const BLUE: Color32 = Color32::from_rgb(0x25, 0x63, 0xEB);
pub const BLUE_TEXT: Color32 = Color32::from_rgb(0x1D, 0x4E, 0xD8);
pub const BLUE_SOFT: Color32 = Color32::from_rgb(0xDB, 0xEA, 0xFE);
pub const RED: Color32 = Color32::from_rgb(0xDC, 0x26, 0x26);
pub const RED_TEXT: Color32 = Color32::from_rgb(0xB9, 0x1C, 0x1C);
pub const RED_SOFT: Color32 = Color32::from_rgb(0xFE, 0xE2, 0xE2);
pub const RED_BORDER: Color32 = Color32::from_rgb(0xFC, 0xA5, 0xA5);

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

pub fn apply_style(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Light);
    ctx.style_mut(|style| {
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
        v.panel_fill = BG;
        v.window_fill = BG;
        v.window_stroke = Stroke::new(1.0, BORDER);
        v.window_corner_radius = CornerRadius::same(10);
        v.menu_corner_radius = CornerRadius::same(8);
        v.extreme_bg_color = BG;
        v.faint_bg_color = CARD;
        v.hyperlink_color = ACCENT;
        v.selection.bg_fill = Color32::from_rgb(0xFD, 0xD9, 0xC1);
        v.selection.stroke = Stroke::new(1.0, ACCENT);
        v.text_cursor.stroke = Stroke::new(1.5, ACCENT);
        v.override_text_color = None;

        let r = CornerRadius::same(6);
        let w = &mut v.widgets;
        w.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
        w.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_2);
        w.noninteractive.corner_radius = r;
        for (state, fill) in [
            (&mut w.inactive, BG),
            (&mut w.hovered, Color32::from_rgb(0xF9, 0xFA, 0xFB)),
            (&mut w.active, CARD),
            (&mut w.open, CARD),
        ] {
            state.bg_fill = fill;
            state.weak_bg_fill = fill;
            state.corner_radius = r;
            state.fg_stroke = Stroke::new(1.0, TEXT);
            state.expansion = 0.0;
        }
        w.inactive.bg_stroke = Stroke::new(1.0, INPUT_BORDER);
        w.hovered.bg_stroke = Stroke::new(1.0, FAINT);
        w.active.bg_stroke = Stroke::new(1.0, ACCENT);
        w.open.bg_stroke = Stroke::new(1.0, FAINT);
    });
}
