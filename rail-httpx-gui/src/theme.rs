//! Paleta de cores do RAIL_HTTPX.
//!
//! Conceito:
//!   - Verde neon (#D6FF5C) é a cor de "ação positiva": botões primários,
//!     foco, seleção, status ok. Três variantes para hover/pressed.
//!   - Azul claro (#7BBBFF) é a cor de "informação": labels, títulos,
//!     links, cabeçalhos de seção.
//!   - Fundo escuro usa navy (#050F2A) e cinza-escuro (#1E1E1E) em vez de
//!     preto puro, para reduzir fadiga visual em sessões longas.
//!   - No tema claro, todos os fundos sobem pra quase-branco e o
//!     texto desce pra quase-preto, mantendo os acentos.
//!
//!

#![allow(dead_code)] // cores de hover/pressed reservadas para uso futuro

use eframe::egui::{self, Color32, Rounding, Stroke, Visuals};

// ==================== Accent (verde) ====================

/// Verde neon — variante clara (hover).
pub const ACCENT_LIGHT: Color32 = Color32::from_rgb(0xE5, 0xFF, 0x85);
/// Verde neon — base.
pub const ACCENT: Color32 = Color32::from_rgb(0xD6, 0xFF, 0x5C);
/// Verde neon — variante escura (pressed).
pub const ACCENT_DARK: Color32 = Color32::from_rgb(0xA8, 0xD4, 0x2C);

// ==================== Info (azul) ====================

/// Azul claro — variante clara.
pub const INFO_LIGHT: Color32 = Color32::from_rgb(0x9F, 0xCB, 0xFF);
/// Azul claro — base.
pub const INFO: Color32 = Color32::from_rgb(0x7B, 0xBB, 0xFF);

// ==================== Neutros ====================

/// Texto primário.
pub const WHITE: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
/// Texto secundário/desabilitado.
pub const GRAY_LIGHT: Color32 = Color32::from_rgb(0xB0, 0xB0, 0xB0);
/// Divisores e bordas.
pub const GRAY_MID: Color32 = Color32::from_rgb(0x2A, 0x2A, 0x2A);
/// Painéis escuros (era #141414).
pub const GRAY_DARK: Color32 = Color32::from_rgb(0x1E, 0x1E, 0x1E);
/// Fundo principal do tema escuro.
pub const NAVY: Color32 = Color32::from_rgb(0x05, 0x0F, 0x2A);



// ==================== Estados ====================

pub const OK: Color32 = ACCENT;
pub const WARN: Color32 = Color32::from_rgb(0xF0, 0xC0, 0x50);
pub const ERR: Color32 = Color32::from_rgb(0xE0, 0x60, 0x60);

// ==================== Theme ====================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Dark,
    Light,
}

impl Default for Theme {
    fn default() -> Self {
        Theme::Dark
    }
}

impl Theme {
    pub fn code(&self) -> &'static str {
        match self {
            Theme::Dark => "dark",
            Theme::Light => "light",
        }
    }

    pub fn from_code(s: &str) -> Self {
        match s {
            "light" => Theme::Light,
            _ => Theme::Dark,
        }
    }

    /// Fundo do painel central (o "miolo" da tela).
    pub fn central_bg(&self) -> Color32 {
        match self {
            Theme::Dark => NAVY,
            Theme::Light => Color32::from_rgb(0xF5, 0xF7, 0xFB),
        }
    }

    /// Fundo das barras (topo e rodapé).
    pub fn bar_bg(&self) -> Color32 {
        match self {
            Theme::Dark => GRAY_DARK,
            Theme::Light => WHITE,
        }
    }

    /// Texto sobre o accent (botão verde primário).
    pub fn on_accent(&self) -> Color32 {
        // Sempre preto sobre verde — funciona nos dois temas.
        Color32::from_rgb(0x14, 0x14, 0x14)
    }

    /// Cor de destaque textual (títulos, "RAIL_HTTPX" na barra).
    /// No escuro usa o neon; no claro precisa escurecer, senão some no branco.
    pub fn accent_text(&self) -> Color32 {
        match self {
            Theme::Dark => ACCENT,
            Theme::Light => Color32::from_rgb(0x4A, 0x6B, 0x00), // verde escuro legível
        }
    }

    /// Aplica as `Visuals` no contexto do egui.
    pub fn apply(&self, ctx: &egui::Context) {
        match self {
            Theme::Dark => ctx.set_visuals(dark_visuals()),
            Theme::Light => ctx.set_visuals(light_visuals()),
        }
    }
}

fn dark_visuals() -> Visuals {
    let mut v = Visuals::dark();

    v.override_text_color = Some(WHITE);
    v.window_fill = NAVY;
    v.panel_fill = GRAY_DARK;
    v.extreme_bg_color = NAVY;
    v.faint_bg_color = Color32::from_rgb(0x0A, 0x18, 0x38);
    v.window_stroke = Stroke::new(1.0_f32, GRAY_MID);

    // Widgets base
    v.widgets.noninteractive.bg_fill = GRAY_DARK;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, GRAY_MID);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, WHITE);

    v.widgets.inactive.bg_fill = Color32::from_rgb(0x14, 0x1E, 0x38);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, WHITE);

    v.widgets.hovered.bg_fill = Color32::from_rgb(0x1E, 0x2A, 0x4A);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, INFO);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, WHITE);

    v.widgets.active.bg_fill = ACCENT;
    v.widgets.active.bg_stroke = Stroke::new(1.0_f32, ACCENT_LIGHT);
    v.widgets.active.fg_stroke = Stroke::new(1.5_f32, Color32::from_rgb(0x14, 0x14, 0x14));

    // Seleção (linha selecionada, texto selecionado)
    v.selection.bg_fill = Color32::from_rgb(0x2A, 0x40, 0x70);
    v.selection.stroke = Stroke::new(1.0_f32, ACCENT);

    // Cantos
    v.window_rounding = Rounding::same(6.0);
    v.menu_rounding = Rounding::same(4.0);

    // Links
    v.hyperlink_color = INFO;

    v
}

fn light_visuals() -> Visuals {
    let mut v = Visuals::light();

    v.override_text_color = Some(Color32::from_rgb(0x1A, 0x1A, 0x1A));
    v.window_fill = Color32::from_rgb(0xFA, 0xFA, 0xFA);
    v.panel_fill = WHITE;
    v.extreme_bg_color = Color32::from_rgb(0xEF, 0xF3, 0xFA);
    v.faint_bg_color = Color32::from_rgb(0xE7, 0xEC, 0xF5);

    v.widgets.noninteractive.bg_fill = WHITE;
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(0x1A, 0x1A, 0x1A));

    v.widgets.inactive.bg_fill = Color32::from_rgb(0xE5, 0xEB, 0xF5);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(0x1A, 0x1A, 0x1A));

    v.widgets.hovered.bg_fill = Color32::from_rgb(0xD0, 0xDD, 0xF0);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, NAVY);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(0x1A, 0x1A, 0x1A));

    v.widgets.active.bg_fill = NAVY;
    v.widgets.active.bg_stroke = Stroke::new(1.0_f32, NAVY);
    v.widgets.active.fg_stroke = Stroke::new(1.5_f32, WHITE);

    v.selection.bg_fill = Color32::from_rgb(0xB8, 0xD8, 0xFF);
    v.selection.stroke = Stroke::new(1.0_f32, NAVY);

    v.hyperlink_color = NAVY;

    v
}