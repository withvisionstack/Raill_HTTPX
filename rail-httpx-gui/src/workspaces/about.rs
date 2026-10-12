//! Aba "Sobre" — informações do app.

use crate::config::AppConfig;
use crate::theme::INFO;
use eframe::egui;
use egui_phosphor::regular as ic;

pub struct AboutWorkspace;

impl AboutWorkspace {
    pub fn new() -> Self { Self }

    pub fn draw(&mut self, ui: &mut egui::Ui, config: &AppConfig) {
        let s = config.language().strings();
        let accent = config.theme().accent_text();

        ui.heading(
            egui::RichText::new(format!("{}  RAIL_HTTPX", ic::ROCKET))
                .color(accent)
                .strong(),
        );
        ui.add_space(4.0);
        ui.label(s.about_subtitle);
        ui.add_space(16.0);

        ui.label(
            egui::RichText::new(format!("{}  {}", ic::INFO, s.about_tabs_header))
                .strong()
                .color(INFO),
        );
        ui.label(format!("  {} {}      — {}", ic::PAPER_PLANE_TILT, s.tab_send, s.about_send_desc));
        ui.label(format!("  {} {}      — {}", ic::SHIELD_CHECK, s.tab_proxy, s.about_proxy_desc));
        ui.label(format!(
            "  {} Scanner   — SQLi, XSS, SSTI, Path Traversal  ({})",
            ic::MAGNIFYING_GLASS, s.about_planned
        ));
        ui.label(format!(
            "  {} Fuzzer    — Intruder                          ({})",
            ic::CROSSHAIR, s.about_planned
        ));
        ui.label(format!(
            "  {} DB        — PostgreSQL / MySQL / SQLite       ({})",
            ic::DATABASE, s.about_planned
        ));

        ui.add_space(16.0);
        ui.label(
            egui::RichText::new(format!(
                "core v{} · GUI egui/eframe · {} / {}",
                env!("CARGO_PKG_VERSION"),
                config.language().label(),
                match config.theme() {
                    crate::theme::Theme::Dark => s.theme_dark,
                    crate::theme::Theme::Light => s.theme_light,
                }
            ))
                .weak(),
        );
    }
}