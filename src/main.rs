#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod config;
mod export;
mod net;
mod opener;
mod scanner;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mut screenshot: Option<std::path::PathBuf> = None;
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--screenshot" && i + 1 < args.len() {
            screenshot = Some(args[i + 1].clone().into());
            i += 1;
        }
        i += 1;
    }

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("IP Range - OxiScan")
        .with_inner_size([860.0, 560.0])
        .with_min_inner_size([560.0, 320.0])
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
        Box::new(move |cc| Ok(Box::new(app::ScannerApp::new(cc, screenshot)))),
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
