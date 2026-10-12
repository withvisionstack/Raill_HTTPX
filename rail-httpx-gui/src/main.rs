//! RAIL_HTTPX — aplicação nativa.
//!
//! Entry point mínimo. A lógica vive em `app.rs` (top-level),
//! `workspaces/*` (cada aba) e `ui/*` (componentes compartilhados).
//!
//!
//!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]


mod app;
mod config;
mod i18n;
mod theme;
mod ui;
mod workspaces;

use eframe::egui;

fn load_icon() -> Option<egui::IconData> {
    let bytes = include_bytes!("../assets/icon.png");
    let img = image::load_from_memory(bytes).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    Some(egui::IconData {
        rgba: img.into_raw(),
        width: w,
        height: h,
    })
}

fn main() -> eframe::Result<()> {
    let cfg = config::load();

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1200.0, 800.0])
        .with_min_inner_size([700.0, 500.0])
        .with_title("RAIL_HTTPX");

    if let Some(icon) = load_icon() {
        viewport = viewport.with_icon(std::sync::Arc::new(icon));
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "RAIL_HTTPX",
        options,
        Box::new(move |cc| {
            let mut fonts = egui::FontDefinitions::default();
            egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
            cc.egui_ctx.set_fonts(fonts);
            cfg.theme().apply(&cc.egui_ctx);
            Ok(Box::new(app::RailHttpxApp::new(cfg)))
        }),
    )
}