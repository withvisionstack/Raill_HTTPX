//! Cada aba é um "workspace" — tem seu próprio estado e sua própria
//! função `draw`. A enum abaixo define quais existem.

pub mod about;
pub mod common;
pub mod db;
pub mod fuzzer;
pub mod proxy;
pub mod scanner;
pub mod send;
pub mod webhook;

use crate::i18n::Strings;
use egui_phosphor::regular as ic;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Workspace {
    Send,
    Proxy,
    Fuzzer,
    Scanner,
    Webhook,
    Db,
    About,
}

impl Workspace {
    pub fn all() -> &'static [Workspace] {
        &[
            Workspace::Send,
            Workspace::Proxy,
            Workspace::Fuzzer,
            Workspace::Scanner,
            Workspace::Webhook,
            Workspace::Db,
            Workspace::About,
        ]
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Workspace::Send => ic::PAPER_PLANE_TILT,
            Workspace::Proxy => ic::SHIELD_CHECK,
            Workspace::Fuzzer => ic::CROSSHAIR,
            Workspace::Scanner => ic::MAGNIFYING_GLASS,
            Workspace::Webhook => ic::RSS,
            Workspace::Db => ic::DATABASE,
            Workspace::About => ic::INFO,
        }
    }

    pub fn label(&self, s: &'static Strings) -> &'static str {
        match self {
            Workspace::Send => s.tab_send,
            Workspace::Proxy => s.tab_proxy,
            Workspace::Fuzzer => s.tab_fuzzer,
            Workspace::Scanner => s.tab_scanner,
            Workspace::Webhook => s.tab_webhook,
            Workspace::Db => s.tab_db,
            Workspace::About => s.tab_about,
        }
    }
}