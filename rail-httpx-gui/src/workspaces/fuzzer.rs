//! Aba "Fuzzer" — dispara uma wordlist contra uma URL com marcador FUZZ.
//! Reaproveita `rail_httpx_core::security::Fuzzer`.

use crate::config::AppConfig;
use crate::i18n::Strings;
use crate::theme::{ACCENT, ERR, INFO, OK, WARN, WHITE};
use eframe::egui;
use egui_phosphor::regular as ic;
use rail_httpx_core::environment::Environment;
use rail_httpx_core::http::{HttpClient, HttpRequest, Method};
use rail_httpx_core::limits::Limits;
use rail_httpx_core::security::{FuzzResult, Fuzzer};
use std::sync::mpsc;

const MARKER: &str = "FUZZ";

enum FuzzerMessage {
    Done(Vec<FuzzResult>),
    Error(String),
}

pub struct FuzzerWorkspace {
    // Inputs
    url: String,
    method: String,
    insecure: bool,
    concurrency: usize,
    wordlist_text: String,
    wordlist_path: String,

    // Estado
    running: bool,
    error: Option<String>,
    results: Vec<FuzzResult>,

    // Infra
    tx: mpsc::Sender<FuzzerMessage>,
    rx: mpsc::Receiver<FuzzerMessage>,
}

impl FuzzerWorkspace {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            url: "http://127.0.0.1:8080/FUZZ".to_string(),
            method: "GET".to_string(),
            insecure: false,
            concurrency: 20,
            wordlist_text: "admin\ntest\nuser\napi\nlogin\n'".to_string(),
            wordlist_path: String::new(),
            running: false,
            error: None,
            results: Vec::new(),
            tx,
            rx,
        }
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    // -------- Ações --------
    fn load_wordlist_from_file(&mut self) {
        if self.wordlist_path.trim().is_empty() {
            return;
        }
        match std::fs::read_to_string(self.wordlist_path.trim()) {
            Ok(s) => self.wordlist_text = s,
            Err(e) => self.error = Some(format!("{e}")),
        }
    }

    fn fire(&mut self) {
        if self.running {
            return;
        }

        if !self.url.contains(MARKER) {
            self.error = Some("fuzzer_requires_marker".into());
            return;
        }

        let payloads: Vec<String> = self
            .wordlist_text
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();
        if payloads.is_empty() {
            self.error = Some("fuzzer_requires_payloads".into());
            return;
        }

        let method = parse_method(&self.method);
        let template = HttpRequest { method, ..HttpRequest::get(&self.url) };
        let insecure = self.insecure;
        let concurrency = self.concurrency;
        let tx = self.tx.clone();
        let env = Environment::new("default");

        self.running = true;
        self.error = None;
        self.results.clear();

        // std::thread + block_on em vez de tokio::spawn.
        // Motivo: o future de `Fuzzer::fuzz` segura empréstimos internos
        // (aos `payloads`/`template`/`env`). Isso é seguro em uso normal
        // — o CLI faz igual — mas `tokio::spawn` exige `Send + 'static`
        // e o compilador não consegue provar. Num thread dedicado com
        // runtime próprio, o future nasce e morre no mesmo escopo.
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime");

            rt.block_on(async move {
                let client = match if insecure {
                    HttpClient::insecure()
                } else {
                    HttpClient::new()
                } {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = tx.send(FuzzerMessage::Error(format!("client: {e}")));
                        return;
                    }
                };

                let fuzzer = Fuzzer::with_limits(client, concurrency, Limits::default());
                match fuzzer.fuzz(&template, &env, MARKER, &payloads).await {
                    Ok(results) => {
                        let _ = tx.send(FuzzerMessage::Done(results));
                    }
                    Err(e) => {
                        let _ = tx.send(FuzzerMessage::Error(e.to_string()));
                    }
                }
            });
        });
    }

    pub fn drain(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            self.running = false;
            match msg {
                FuzzerMessage::Done(results) => {
                    self.results = results;
                    self.error = None;
                }
                FuzzerMessage::Error(e) => {
                    self.error = Some(e);
                    self.results.clear();
                }
            }
        }
    }

    // -------- Render --------
    pub fn draw(&mut self, ui: &mut egui::Ui, config: &AppConfig) {
        let s = config.language().strings();
        self.draw_inputs(ui, s, config);
        ui.separator();
        self.draw_results(ui, s, config);
    }

    fn draw_inputs(&mut self, ui: &mut egui::Ui, s: &Strings, config: &AppConfig) {
        let on_accent = config.theme().on_accent();

        ui.label(egui::RichText::new(s.fuzzer_url_label).color(INFO));
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("fuzzer_method")
                .selected_text(&self.method)
                .width(100.0)
                .show_ui(ui, |ui| {
                    for m in ["GET", "POST", "PUT", "PATCH", "DELETE"] {
                        ui.selectable_value(&mut self.method, m.to_string(), m);
                    }
                });

            let remaining = ui.available_width();
            let button_w = 120.0;
            ui.add_sized(
                [remaining - button_w - 8.0, 24.0],
                egui::TextEdit::singleline(&mut self.url).hint_text(s.fuzzer_marker_hint),
            );

            let btn = egui::Button::new(
                egui::RichText::new(format!("{}  {}", ic::CROSSHAIR, s.fuzzer_run))
                    .color(on_accent)
                    .strong(),
            )
                .fill(ACCENT)
                .rounding(4.0);

            if ui.add_enabled(!self.running, btn).clicked() {
                self.fire();
            }
        });

        ui.horizontal(|ui| {
            ui.label(format!("{}  {}", ic::LIGHTNING, s.fuzzer_concurrency));
            ui.add_sized(
                [80.0, 22.0],
                egui::DragValue::new(&mut self.concurrency).range(1..=200),
            );
            ui.separator();
            ui.checkbox(&mut self.insecure, s.fuzzer_insecure);
        });

        ui.separator();

        ui.label(egui::RichText::new(s.fuzzer_wordlist_label).color(INFO));
        egui::ScrollArea::vertical()
            .id_salt("fuzzer_wordlist_scroll")
            .max_height(120.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut self.wordlist_text)
                        .desired_rows(5)
                        .desired_width(f32::INFINITY)
                        .font(egui::TextStyle::Monospace),
                );
            });

        ui.horizontal(|ui| {
            ui.label(format!("{}  {}", ic::FOLDER_OPEN, s.fuzzer_wordlist_file));
            ui.add_sized(
                [300.0, 22.0],
                egui::TextEdit::singleline(&mut self.wordlist_path)
                    .hint_text("C:\\caminho\\words.txt"),
            );
            if ui.button(s.fuzzer_load_file).clicked() {
                self.load_wordlist_from_file();
            }
        });

        if let Some(err) = &self.error {
            let text = match err.as_str() {
                "fuzzer_requires_marker" => s.fuzzer_requires_marker,
                "fuzzer_requires_payloads" => s.fuzzer_requires_payloads,
                other => other,
            };
            ui.colored_label(ERR, format!("{}  {}", ic::X_CIRCLE, text));
        }
    }

    fn draw_results(&self, ui: &mut egui::Ui, s: &Strings, _config: &AppConfig) {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!(
                    "{}  {} {}",
                    ic::LIST_MAGNIFYING_GLASS,
                    s.fuzzer_results,
                    self.results.len()
                ))
                    .strong()
                    .color(WHITE),
            );
            if self.running {
                ui.spinner();
                ui.label(s.fuzzer_running);
            }
        });

        if self.results.is_empty() && !self.running {
            ui.label(egui::RichText::new(s.fuzzer_no_results).weak());
            return;
        }

        egui::ScrollArea::vertical()
            .id_salt("fuzzer_results_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("fuzzer_results_grid")
                    .striped(true)
                    .min_col_width(60.0)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(s.fuzzer_col_payload).strong());
                        ui.label(egui::RichText::new(s.fuzzer_col_status).strong());
                        ui.label(egui::RichText::new(s.fuzzer_col_length).strong());
                        ui.label(egui::RichText::new(s.fuzzer_col_time).strong());
                        ui.end_row();

                        for r in &self.results {
                            let color = status_color(r.status);
                            ui.label(
                                egui::RichText::new(&r.payload).monospace().color(WHITE),
                            );
                            ui.label(
                                egui::RichText::new(format!("{}", r.status))
                                    .color(color)
                                    .strong(),
                            );
                            ui.label(egui::RichText::new(format!("{}", r.length)).color(WHITE));
                            ui.label(
                                egui::RichText::new(format!("{}ms", r.duration_ms))
                                    .color(WHITE),
                            );
                            ui.end_row();
                        }
                    });
            });
    }
}

// -------- Helpers --------

fn parse_method(s: &str) -> Method {
    match s.to_uppercase().as_str() {
        "POST" => Method::Post,
        "PUT" => Method::Put,
        "PATCH" => Method::Patch,
        "DELETE" => Method::Delete,
        _ => Method::Get,
    }
}

fn status_color(status: u16) -> egui::Color32 {
    match status {
        0 => ERR,
        200..=299 => OK,
        300..=399 => WARN,
        400..=499 => INFO,
        500..=599 => ERR,
        _ => WHITE,
    }
}