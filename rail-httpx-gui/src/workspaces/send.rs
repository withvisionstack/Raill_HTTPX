//! Aba "Enviar" — cliente HTTP único.
//! Estado isolado; comunicação com o runtime via canal `mpsc`.

use crate::config::AppConfig;
use crate::i18n::Strings;
use eframe::egui;
use egui_phosphor::regular as ic;
use rail_httpx_core::environment::Environment;
use rail_httpx_core::http::{Body, HttpClient, HttpRequest, HttpResponse, Method};
use std::sync::mpsc;
use theme_colors::{ERR, INFO, OK, WARN, WHITE};

mod theme_colors {
    pub use crate::theme::{ERR, INFO, OK, WARN, WHITE};
}

enum SendMessage {
    Response(Box<HttpResponse>),
    Error(String),
}

pub struct SendWorkspace {
    method: String,
    url: String,
    headers_text: String,
    body_text: String,
    insecure: bool,
    response: Option<HttpResponse>,
    response_text: String,
    error: Option<String>,
    loading: bool,

    rt: tokio::runtime::Runtime,
    tx: mpsc::Sender<SendMessage>,
    rx: mpsc::Receiver<SendMessage>,
}

impl SendWorkspace {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        Self {
            method: "GET".to_string(),
            url: "https://httpbin.org/get".to_string(),
            headers_text: String::new(),
            body_text: String::new(),
            insecure: false,
            response: None,
            response_text: String::new(),
            error: None,
            loading: false,
            rt,
            tx,
            rx,
        }
    }

    // -------- Acessores usados pela status bar --------
    pub fn is_loading(&self) -> bool { self.loading }
    pub fn error(&self) -> Option<&str> { self.error.as_deref() }
    pub fn response(&self) -> Option<&HttpResponse> { self.response.as_ref() }

    // -------- Ciclo assíncrono --------
    fn fire_request(&mut self) {
        let method = parse_method(&self.method);
        let mut req = HttpRequest { method, ..HttpRequest::get(&self.url) };

        for (k, v) in crate::workspaces::common::parse_headers_text(&self.headers_text) {
            req.headers.insert(k, v);
        }
        
        if !self.body_text.is_empty() {
            req.body = Some(Body::Text(self.body_text.clone()));
        }

        // ---- DEBUG: imprime o que vai ser enviado ----
        eprintln!("=== SEND DEBUG ===");
        eprintln!("method: {:?}", method);
        eprintln!("url:    {}", self.url);
        eprintln!("headers:");
        for (k, v) in &req.headers {
            eprintln!("  {:?}: {:?}", k, v);
        }
        match &req.body {
            Some(Body::Text(t)) => {
                eprintln!("body ({} bytes):", t.len());
                eprintln!("<<<{}>>>", t);          // delimitadores pra ver espaços
                eprintln!("bytes: {:?}", t.as_bytes());
            }
            Some(other) => eprintln!("body: {:?}", other),
            None => eprintln!("body: (nenhum)"),
        }
        eprintln!("==================");
        // ----------------------------------------------

        let insecure = self.insecure;
        let tx = self.tx.clone();
        let env = Environment::new("default");

        self.loading = true;
        self.error = None;
        self.response = None;
        self.response_text.clear();

        self.rt.spawn(async move {
            let client = match if insecure {
                HttpClient::insecure()
            } else {
                HttpClient::new()
            } {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(SendMessage::Error(format!("client: {e}")));
                    return;
                }
            };
            match client.send(&req, &env).await {
                Ok(resp) => { let _ = tx.send(SendMessage::Response(Box::new(resp))); }
                Err(e) => { let _ = tx.send(SendMessage::Error(e.to_string())); }
            }
        });
    }

    pub fn drain(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            self.loading = false;
            match msg {
                SendMessage::Response(resp) => {
                    self.response_text = resp.body_text();
                    self.response = Some(*resp);
                    self.error = None;
                }
                SendMessage::Error(e) => {
                    self.error = Some(e);
                    self.response = None;
                    self.response_text.clear();
                }
            }
        }
    }

    // -------- Render --------
    pub fn draw(&mut self, ui: &mut egui::Ui, config: &AppConfig) {
        let s = config.language().strings();
        let on_accent = config.theme().on_accent();

        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("method")
                .selected_text(&self.method)
                .width(100.0)
                .show_ui(ui, |ui| {
                    for m in ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"] {
                        ui.selectable_value(&mut self.method, m.to_string(), m);
                    }
                });

            let remaining = ui.available_width();
            let button_w = 110.0;
            ui.add_sized(
                [remaining - button_w - 8.0, 24.0],
                egui::TextEdit::singleline(&mut self.url).hint_text(s.url_placeholder),
            );

            let btn = egui::Button::new(
                egui::RichText::new(format!("{}  {}", ic::PAPER_PLANE_TILT, s.send_button))
                    .color(on_accent)
                    .strong(),
            )
                .fill(crate::theme::ACCENT)
                .rounding(4.0);

            if ui.add_enabled(!self.loading, btn).clicked() {
                self.fire_request();
            }
        });

        ui.add_space(4.0);
        ui.checkbox(&mut self.insecure, s.insecure_tls);
        ui.separator();

        self.draw_headers(ui, s);
        self.draw_body(ui, s);
        ui.separator();
        self.draw_response(ui, s);
    }

    fn draw_headers(&mut self, ui: &mut egui::Ui, s: &Strings) {
        ui.label(
            egui::RichText::new(format!("{}  {}", ic::LIST_DASHES, s.headers_label)).color(INFO),
        );
        egui::ScrollArea::vertical()
            .id_salt("headers_scroll")
            .max_height(90.0)
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut self.headers_text)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY)
                        .font(egui::TextStyle::Monospace),
                );
            });
    }

    fn draw_body(&mut self, ui: &mut egui::Ui, s: &Strings) {
        ui.label(egui::RichText::new(format!("{}  {}", ic::CODE, s.body_label)).color(INFO));
        egui::ScrollArea::vertical()
            .id_salt("body_scroll")
            .max_height(150.0)
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut self.body_text)
                        .desired_rows(6)
                        .desired_width(f32::INFINITY)
                        .font(egui::TextStyle::Monospace),
                );
            });
    }

    fn draw_response(&mut self, ui: &mut egui::Ui, s: &Strings) {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!("{}  {}", ic::ARROW_BEND_DOWN_LEFT, s.response_label))
                    .strong()
                    .color(WHITE),
            );
            if let Some(resp) = &self.response {
                let (color, icon) = if resp.status < 300 {
                    (OK, ic::CHECK_CIRCLE)
                } else if resp.status < 400 {
                    (WARN, ic::WARNING_CIRCLE)
                } else {
                    (ERR, ic::X_CIRCLE)
                };
                ui.colored_label(
                    color,
                    format!(
                        "{}  HTTP {}  ·  {}ms  ·  {} bytes",
                        icon,
                        resp.status,
                        resp.duration_ms,
                        resp.body.len()
                    ),
                );
            }
        });

        egui::ScrollArea::vertical()
            .id_salt("response_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let display = if self.response_text.is_empty() {
                    s.no_response_yet.to_string()
                } else {
                    self.response_text.clone()
                };
                ui.add(egui::Label::new(egui::RichText::new(display).monospace()).selectable(true));
            });
    }
}

fn parse_method(s: &str) -> Method {
    match s.to_uppercase().as_str() {
        "POST" => Method::Post,
        "PUT" => Method::Put,
        "PATCH" => Method::Patch,
        "DELETE" => Method::Delete,
        "HEAD" => Method::Head,
        "OPTIONS" => Method::Options,
        _ => Method::Get,
    }
}