#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod config;
mod export;
mod net;
mod opener;
mod scanner;
mod services;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mut screenshot: Option<std::path::PathBuf> = None;
    let mut demo: Option<app::DemoMode> = None;
    let mut theme: Option<config::ThemePref> = None;
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--screenshot" && i + 1 < args.len() {
            screenshot = Some(args[i + 1].clone().into());
            i += 1;
        } else if let Some(t) = args[i].strip_prefix("--theme=") {
            theme = config::ThemePref::parse(t);
        } else if let Some(mode) = args[i].strip_prefix("--demo") {
            demo = app::DemoMode::parse(mode.trim_start_matches('='));
        }
        i += 1;
    }

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("OxiScan")
        .with_inner_size([1200.0, 720.0])
        .with_min_inner_size([960.0, 560.0])
        .with_icon(app_icon());

    if screenshot.is_some() {
        viewport = viewport.with_active(false);
        #[cfg(windows)]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }

    #[allow(unused_mut)]
    let mut options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    #[cfg(target_os = "macos")]
    if screenshot.is_some() {
        options.event_loop_builder = Some(Box::new(|b| {
            use winit::platform::macos::EventLoopBuilderExtMacOS;
            b.with_activate_ignoring_other_apps(false);
        }));
    }

    eframe::run_native(
        "OxiScan",
        options,
        Box::new(move |cc| Ok(Box::new(app::ScannerApp::new(cc, screenshot, demo, theme)))),
    )
}

fn app_icon() -> egui::IconData {
    let img = image::load_from_memory(include_bytes!("../assets/logo-256.png"))
        .expect("embedded icon is a valid PNG")
        .into_rgba8();
    let (width, height) = img.dimensions();
    egui::IconData {
        rgba: img.into_raw(),
        width,
        height,
    }
}
