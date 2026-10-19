// Melbourne Fire Brigade — native Windows app (Rust).
// No console window behind the app on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod auth;
mod chime;
mod core;
mod display;
#[allow(dead_code)] // the ambulance guide, kept until the fire one is written
mod fieldguide;
mod postals;
mod relay;
mod settings;
mod single;
mod storage;
mod streets;
mod timeclock;
mod ui;
mod update;

fn main() -> eframe::Result {
    // Already open? Bring that window forward and stop here.
    if !single::claim() {
        return Ok(());
    }
    let icon = image::load_from_memory(include_bytes!("../assets/icon.png")).ok().map(|img| {
        let img = img.to_rgba8();
        let (w, h) = img.dimensions();
        eframe::egui::IconData { rgba: img.into_raw(), width: w, height: h }
    });
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_title("Melbourne Fire Brigade")
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([400.0, 480.0]);
    // Not `.with_maximized(true)`: on Windows that can mark the window as maximised
    // without actually resizing it. The app maximises itself once it's on screen (app.rs).
    // Start in the saved window mode (the app finishes the job once it's on screen).
    if std::env::var("MFB_WINDOWED").is_err() {
        match display::mode() {
            display::Mode::Borderless => viewport = viewport.with_decorations(false),
            display::Mode::Fullscreen => {}
            display::Mode::Windowed => {}
        }
    }
    if let Some(icon) = icon {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    let core = core::Core::new();
    eframe::run_native("Melbourne Fire Brigade", options, Box::new(move |cc| Ok(Box::new(app::App::new(cc, core)))))
}
