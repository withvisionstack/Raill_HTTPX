//! Aba "Scanner" — injeta payloads nos query params e mostra os findings
//! (SQLi, XSS, SSTI, Path Traversal, Cmd Injection).
//!
//! Diferente do Fuzzer (que mostra status/tamanho/tempo brutos), aqui
//! o core `Scanner` já aplica as regras de detecção e devolve `Finding`s.

use crate::config::AppConfig;
use crate::i18n::Strings;
use crate::theme::{ACCENT, ERR, INFO, WARN, WHITE};
use eframe::egui;
use egui_phosphor::regular as ic;
use rail_httpx_core::environment::Environment;
use rail_httpx_core::http::{HttpClient, HttpRequest, Method};
use rail_httpx_core::limits::Limits;
use rail_httpx_core::security::{Finding, Scanner, Severity};
use std::sync::mpsc;

enum ScannerMessage {
    Done(Vec<Finding>),
    Error(String),
    TestResult(String),
}

pub struct ScannerWorkspace {
    // Inputs
    url: String,
    method: String,
    headers_text: String,
    params_input: String,
    insecure: bool,
    max_payloads_per_category: usize,

    // Estado
    running: bool,
    error: Option<String>,
    findings: Vec<Finding>,
    test_result: Option<String>,

    // Infra
    tx: mpsc::Sender<ScannerMessage>,
    rx: mpsc::Receiver<ScannerMessage>,
}

impl ScannerWorkspace {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            url: "http://localhost/vulnerabilities/sqli/?id=1&Submit=Submit".to_string(),
            method: "GET".to_string(),
            headers_text: String::new(),
            params_input: "id".to_string(),
            insecure: false,
            max_payloads_per_category: 30,
            running: false,
            error: None,
            findings: Vec::new(),
            test_result: None,
            tx,
            rx,
        }
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    // -------- Ações --------
    fn fire(&mut self) {
        if self.running {
            return;
        }

        // Extrai params da URL.
        let parsed = match url::Url::parse(&self.url) {
            Ok(u) => u,
            Err(e) => {
                self.error = Some(format!("URL inválida: {e}"));
                return;
            }
        };
        let all_params: Vec<String> = parsed
            .query_pairs()
            .map(|(k, _)| k.to_string())
            .collect();
        if all_params.is_empty() {
            self.error = Some("scanner_requires_query".into());
            return;
        }

        let selected: Vec<String> = if self.params_input.trim().is_empty() {
            all_params
        } else {
            self.params_input
                .split(',')
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect()
        };

        // Monta o base SEM mexer na URL. O reqwest faz merge de query —
        // se a URL tem `?id=1&Submit=Submit` e adicionamos `id=<payload>`,
        // chega como `?id=1&Submit=Submit&id=<payload>` e o PHP pega o
        // último valor.
        let method = parse_method(&self.method);
        let mut base = HttpRequest { method, ..HttpRequest::get(&self.url) };

        // Headers — parser centralizado que aceita tanto `Nome: valor`
        // quanto o formato colado do DevTools (`"Nome": "valor"`).
        for (k, v) in crate::workspaces::common::parse_headers_text(&self.headers_text) {
            base.headers.insert(k, v);
        }

        // Só os params selecionados vão pro `base.query`, com valor
        // placeholder. O Scanner substitui por payload em cada request.
        for p in &selected {
            base.query.insert(p.clone(), "1".to_string());
        }

        let insecure = self.insecure;
        let max_payloads = self.max_payloads_per_category;
        let tx = self.tx.clone();
        let env = Environment::new("default");

        self.running = true;
        self.error = None;
        self.findings.clear();

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
                        let _ = tx.send(ScannerMessage::Error(format!("client: {e}")));
                        return;
                    }
                };

                let scanner = Scanner::with_limits(client, max_payloads, Limits::default());
                match scanner.scan(&base, &env).await {
                    Ok(findings) => {
                        let _ = tx.send(ScannerMessage::Done(findings));
                    }
                    Err(e) => {
                        let _ = tx.send(ScannerMessage::Error(e.to_string()));
                    }
                }
            });
        });
    }

    /// Dispara 1 GET na URL (com os headers) e devolve status + tempo.
    /// Serve pra validar cookie/sessão antes de rodar o scan inteiro.
    fn test_connection(&mut self) {
        let method = parse_method(&self.method);
        let mut req = HttpRequest { method, ..HttpRequest::get(&self.url) };
        for (k, v) in crate::workspaces::common::parse_headers_text(&self.headers_text) {
            req.headers.insert(k, v);
        }

        let insecure = self.insecure;
        let tx = self.tx.clone();
        let env = Environment::new("default");

        self.test_result = Some("...".into());

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
                        let _ = tx.send(ScannerMessage::TestResult(format!("client: {e}")));
                        return;
                    }
                };
                match client.send(&req, &env).await {
                    Ok(resp) => {
                        let snippet = String::from_utf8_lossy(&resp.body)
                            .chars()
                            .take(80)
                            .collect::<String>()
                            .replace('\n', " ");
                        let msg = format!(
                            "HTTP {} · {}ms · {} bytes\n  {}",
                            resp.status, resp.duration_ms, resp.body.len(), snippet
                        );
                        let _ = tx.send(ScannerMessage::TestResult(msg));
                    }
                    Err(e) => {
                        let _ = tx.send(ScannerMessage::TestResult(format!("erro: {e}")));
                    }
                }
            });
        });
    }

    pub fn drain(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            self.running = false;
            match msg {
                ScannerMessage::Done(findings) => {
                    self.findings = findings;
                    self.error = None;
                }
                ScannerMessage::Error(e) => {
                    self.error = Some(e);
                    self.findings.clear();
                }
                ScannerMessage::TestResult(msg) => {
                    self.test_result = Some(msg);
                }
            }
        }
    }

    // -------- Render --------
    pub fn draw(&mut self, ui: &mut egui::Ui, config: &AppConfig) {
        let s = config.language().strings();
        self.draw_inputs(ui, s, config);
        ui.separator();
        self.draw_findings(ui, s);
    }

    fn draw_inputs(&mut self, ui: &mut egui::Ui, s: &Strings, config: &AppConfig) {
        let on_accent = config.theme().on_accent();

        ui.label(egui::RichText::new(s.scanner_url_label).color(INFO));
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("scanner_method")
                .selected_text(&self.method)
                .width(100.0)
                .show_ui(ui, |ui| {
                    for m in ["GET", "POST", "PUT", "PATCH", "DELETE"] {
                        ui.selectable_value(&mut self.method, m.to_string(), m);
                    }
                });

            let remaining = ui.available_width();
            let run_w = 130.0;
            let test_w = 110.0;
            ui.add_sized(
                [remaining - run_w - test_w - 16.0, 24.0],
                egui::TextEdit::singleline(&mut self.url)
                    .hint_text("http://alvo/item?id=1&cat=2"),
            );

            if ui
                .add_sized([test_w, 24.0], egui::Button::new("🔌  Testar"))
                .clicked()
            {
                self.test_connection();
            }

            let btn = egui::Button::new(
                egui::RichText::new(format!("{}  {}", ic::MAGNIFYING_GLASS, s.scanner_run))
                    .color(on_accent)
                    .strong(),
            )
                .fill(ACCENT)
                .rounding(4.0);

            if ui.add_enabled(!self.running, btn).clicked() {
                self.fire();
            }
        });

        if let Some(msg) = &self.test_result {
            ui.colored_label(INFO, format!("{}  {}", ic::PLUGS, msg));
        }

        ui.horizontal(|ui| {
            ui.label(format!("{}  {}", ic::FUNNEL, s.scanner_params_label));
            ui.add_sized(
                [220.0, 22.0],
                egui::TextEdit::singleline(&mut self.params_input)
                    .hint_text(s.scanner_params_hint),
            );

            ui.separator();

            ui.label(format!("{}  {}", ic::LIGHTNING, s.scanner_max_payloads));
            ui.add_sized(
                [80.0, 22.0],
                egui::DragValue::new(&mut self.max_payloads_per_category).range(1..=500),
            );

            ui.separator();
            ui.checkbox(&mut self.insecure, s.scanner_insecure);
        });

        // Headers
        ui.label(
            egui::RichText::new(format!(
                "{}  Headers (uma por linha, `Nome: valor`):",
                ic::LIST_DASHES
            ))
                .color(INFO),
        );
        egui::ScrollArea::vertical()
            .id_salt("scanner_headers_scroll")
            .max_height(90.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut self.headers_text)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY)
                        .font(egui::TextStyle::Monospace),
                );
            });

        if let Some(err) = &self.error {
            let text = match err.as_str() {
                "scanner_requires_query" => s.scanner_requires_query,
                other => other,
            };
            ui.colored_label(ERR, format!("{}  {}", ic::X_CIRCLE, text));
        }
    }

    fn draw_findings(&self, ui: &mut egui::Ui, s: &Strings) {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!(
                    "{}  {} {}",
                    ic::WARNING_OCTAGON,
                    s.scanner_findings,
                    self.findings.len()
                ))
                    .strong()
                    .color(WHITE),
            );
            if self.running {
                ui.spinner();
                ui.label(s.scanner_running);
            }
        });

        if self.findings.is_empty() && !self.running {
            ui.label(egui::RichText::new(s.scanner_no_findings).weak());
            return;
        }

        egui::ScrollArea::vertical()
            .id_salt("scanner_findings_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("scanner_findings_grid")
                    .striped(true)
                    .min_col_width(70.0)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(s.scanner_col_severity).strong());
                        ui.label(egui::RichText::new(s.scanner_col_category).strong());
                        ui.label(egui::RichText::new(s.scanner_col_param).strong());
                        ui.label(egui::RichText::new(s.scanner_col_payload).strong());
                        ui.label(egui::RichText::new(s.scanner_col_status).strong());
                        ui.end_row();

                        for f in &self.findings {
                            let sev_color = severity_color(f.severity);
                            let sev_label = format!("{:?}", f.severity).to_uppercase();

                            ui.label(
                                egui::RichText::new(sev_label)
                                    .color(sev_color)
                                    .strong(),
                            );
                            ui.label(egui::RichText::new(f.category.as_str()).color(WHITE));
                            ui.label(
                                egui::RichText::new(&f.parameter).monospace().color(WHITE),
                            );
                            ui.label(
                                egui::RichText::new(truncate(&f.payload, 40))
                                    .monospace()
                                    .color(WHITE),
                            );
                            ui.label(
                                egui::RichText::new(format!("{}", f.status)).color(WHITE),
                            );
                            ui.end_row();
                        }
                    });

                if !self.findings.is_empty() {
                    ui.add_space(10.0);
                    ui.separator();
                    ui.label(egui::RichText::new("Evidências:").strong().color(INFO));
                    for (i, f) in self.findings.iter().enumerate() {
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(format!(
                                "#{} {} [{}] {} ({})",
                                i + 1,
                                format!("{:?}", f.severity).to_uppercase(),
                                f.category.as_str(),
                                f.url,
                                f.parameter
                            ))
                                .strong()
                                .color(severity_color(f.severity)),
                        );
                        ui.label(
                            egui::RichText::new(format!("  payload:   {}", f.payload))
                                .monospace()
                                .color(WHITE),
                        );
                        ui.label(
                            egui::RichText::new(format!("  evidência: {}", f.evidence))
                                .monospace()
                                .color(INFO),
                        );
                    }
                }
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

fn severity_color(sev: Severity) -> egui::Color32 {
    match sev {
        Severity::Critical => egui::Color32::from_rgb(0xE0, 0x50, 0x50),
        Severity::High => ERR,
        Severity::Medium => WARN,
        Severity::Low => INFO,
        Severity::Info => WHITE,
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        format!("{}…", &s[..n - 1])
    }
}