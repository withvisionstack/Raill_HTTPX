//! Preferências persistentes em `~/.rail_httpx/config.toml`.

use crate::i18n::Language;
use crate::theme::Theme;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_theme")]
    pub theme: String,
}

fn default_language() -> String { "ptbr".to_string() }
fn default_theme() -> String { "dark".to_string() }

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            language: default_language(),
            theme: default_theme(),
        }
    }
}

impl AppConfig {
    pub fn language(&self) -> Language {
        Language::from_code(&self.language)
    }

    pub fn set_language(&mut self, l: Language) {
        self.language = l.code().to_string();
    }

    pub fn theme(&self) -> Theme {
        Theme::from_code(&self.theme)
    }

    pub fn set_theme(&mut self, t: Theme) {
        self.theme = t.code().to_string();
    }
}

pub fn config_path() -> PathBuf {
    let base = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join(".rail_httpx").join("config.toml")
}

pub fn load() -> AppConfig {
    let path = config_path();
    if !path.exists() {
        return AppConfig::default();
    }
    match std::fs::read_to_string(&path) {
        Ok(s) => toml::from_str(&s).unwrap_or_default(),
        Err(_) => AppConfig::default(),
    }
}

pub fn save(cfg: &AppConfig) -> std::io::Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let s = toml::to_string_pretty(cfg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(path, s)
}