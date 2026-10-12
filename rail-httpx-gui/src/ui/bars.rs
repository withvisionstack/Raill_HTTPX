//! Componentes reutilizáveis da UI: barra superior e barra de status.

use crate::i18n::{Language, Strings};
use crate::theme::{Theme, ERR, OK};
use crate::workspaces::proxy::ProxyWorkspace;
use crate::workspaces::send::SendWorkspace;
use eframe::egui;
use egui_phosphor::regular as ic;

// ==================== Top bar ====================

/// Dropdown de tema. Chama `on_change` quando o usuário escolhe.
pub fn theme_switcher(
    ui: &mut egui::Ui,
    s: &'static Strings,
    current: Theme,
    mut on_change: impl FnMut(Theme),
) {
    let (icon, label) = theme_icon_label(current, s);
    egui::ComboBox::from_id_salt("theme_combo")
        .selected_text(format!("{}  {}", icon, label))
        .width(110.0)
        .show_ui(ui, |ui| {
            for t in [Theme::Dark, Theme::Light] {
                let (icon, lbl) = theme_icon_label(t, s);
                if ui
                    .selectable_label(current == t, format!("{}  {}", icon, lbl))
                    .clicked()
                {
                    on_change(t);
                }
            }
        });
}

fn theme_icon_label(t: Theme, s: &Strings) -> (&'static str, &'static str) {
    match t {
        Theme::Dark => (ic::MOON, s.theme_dark),
        Theme::Light => (ic::SUN, s.theme_light),
    }
}

/// Dropdown de idioma.
pub fn language_switcher(
    ui: &mut egui::Ui,
    _s: &'static Strings,
    current: &Language,
    mut on_change: impl FnMut(Language),
) {
    egui::ComboBox::from_id_salt("lang_combo")
        .selected_text(format!("{}  {}", ic::TRANSLATE, current.label()))
        .width(180.0)
        .show_ui(ui, |ui| {
            for l in Language::all() {
                if ui
                    .selectable_label(*current == l, format!("{}  {}", ic::TRANSLATE, l.label()))
                    .clicked()
                {
                    on_change(l);
                }
            }
        });
}

// ==================== Status bar ====================

pub fn status_bar(
    ui: &mut egui::Ui,
    s: &'static Strings,
    send: &SendWorkspace,
    proxy: &ProxyWorkspace,
) {
    ui.horizontal(|ui| {
        if proxy.is_running() {
            ui.colored_label(OK, format!("{}  {}", ic::CIRCLE, s.status_proxy));
        }

        if send.is_loading() {
            ui.spinner();
            ui.label(s.status_sending);
        } else if let Some(err) = send.error() {
            ui.colored_label(ERR, format!("{}  {} {}", ic::X_CIRCLE, s.status_error, err));
        } else if let Some(resp) = send.response() {
            let icon = if resp.status < 300 {
                ic::CHECK_CIRCLE
            } else {
                ic::WARNING_CIRCLE
            };
            ui.label(format!(
                "{}  {} HTTP {} em {}ms",
                icon, s.status_last_response, resp.status, resp.duration_ms
            ));
        } else {
            ui.label(format!("{}  {}", ic::CHECK, s.status_ready));
        }
    });
}