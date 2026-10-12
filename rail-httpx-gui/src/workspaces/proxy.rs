//! Aba "Proxy" — proxy MITM com log em tempo real.

use crate::config::AppConfig;
use crate::i18n::Strings;
use crate::theme::{ACCENT, ERR, INFO, OK, WHITE};
use eframe::egui;
use egui_phosphor::regular as ic;
use rail_httpx_core::security::{
    default_ca_dir, CertificateAuthority, EventSink, ProxyEvent, ProxyServer,
};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::task::JoinHandle;

pub struct ProxyWorkspace {
    port: u16,
    running: bool,
    addr: Option<String>,
    events: Vec<ProxyEvent>,
    selected: Option<usize>,
    error: Option<String>,
    ca_export_path: String,
    ca_export_msg: Option<String>,

    rt: tokio::runtime::Runtime,
    handle: Option<JoinHandle<()>>,
    events_rx: Option<tokio::sync::mpsc::UnboundedReceiver<ProxyEvent>>,
}

impl ProxyWorkspace {
    pub fn new() -> Self {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        Self {
            port: 8080,
            running: false,
            addr: None,
            events: Vec::new(),
            selected: None,
            error: None,
            ca_export_path: "rail-httpx-ca.pem".to_string(),
            ca_export_msg: None,
            rt,
            handle: None,
            events_rx: None,
        }
    }

    pub fn is_running(&self) -> bool { self.running }

    // -------- Ciclo de vida --------
    fn start(&mut self) {
        if self.running { return; }

        let ca = match CertificateAuthority::new_or_load(default_ca_dir()) {
            Ok(c) => Arc::new(c),
            Err(e) => { self.error = Some(format!("CA: {e}")); return; }
        };

        let addr = format!("127.0.0.1:{}", self.port);
        let (sink, rx) = EventSink::new();

        let server = match self.rt.block_on(ProxyServer::bind(&addr, ca)) {
            Ok(s) => s.with_events(sink),
            Err(e) => { self.error = Some(format!("bind {addr}: {e}")); return; }
        };

        let local = match server.local_addr() {
            Ok(a) => a,
            Err(e) => { self.error = Some(format!("local_addr: {e}")); return; }
        };

        let handle = self.rt.spawn(async move {
            let _ = server.run().await;
        });

        self.addr = Some(local.to_string());
        self.running = true;
        self.events.clear();
        self.selected = None;
        self.error = None;
        self.handle = Some(handle);
        self.events_rx = Some(rx);
    }

    fn stop(&mut self) {
        if let Some(h) = self.handle.take() { h.abort(); }
        self.running = false;
        self.addr = None;
        self.events_rx = None;
    }

    pub fn drain_events(&mut self) {
        let mut incoming = Vec::new();
        if let Some(rx) = &mut self.events_rx {
            while let Ok(ev) = rx.try_recv() { incoming.push(ev); }
        }
        if !incoming.is_empty() {
            incoming.reverse();
            self.events.splice(0..0, incoming);
            if self.events.len() > 1000 {
                self.events.truncate(1000);
            }
        }
    }

    fn export_ca(&mut self) {
        match CertificateAuthority::new_or_load(default_ca_dir()) {
            Ok(ca) => match std::fs::write(&self.ca_export_path, ca.ca_cert_pem()) {
                Ok(_) => self.ca_export_msg = Some(format!("{} {}", ic::CHECK, self.ca_export_path)),
                Err(e) => self.ca_export_msg = Some(format!("{} {e}", ic::X)),
            },
            Err(e) => self.ca_export_msg = Some(format!("{} {e}", ic::X)),
        }
    }

    // -------- Render --------
    pub fn draw(&mut self, ui: &mut egui::Ui, config: &AppConfig) {
        let s = config.language().strings();
        self.draw_toolbar(ui, s, config);
        ui.separator();
        self.draw_table(ui, s);
        ui.separator();
        self.draw_detail(ui, s);
    }

    fn draw_toolbar(&mut self, ui: &mut egui::Ui, s: &Strings, config: &AppConfig) {
        let on_accent = config.theme().on_accent();

        ui.horizontal(|ui| {
            ui.label(format!("{}  {}", ic::PLUGS, s.proxy_port));
            ui.add_sized(
                [80.0, 22.0],
                egui::DragValue::new(&mut self.port).range(1024..=65535),
            );

            if self.running {
                let stop = egui::Button::new(
                    egui::RichText::new(format!("{}  {}", ic::STOP, s.proxy_stop))
                        .color(WHITE)
                        .strong(),
                )
                    .fill(ERR)
                    .rounding(4.0);
                if ui.add(stop).clicked() { self.stop(); }

                if let Some(addr) = &self.addr {
                    ui.colored_label(OK, format!("{}  {} {}", ic::CIRCLE, s.proxy_running_at, addr));
                }
            } else {
                let start = egui::Button::new(
                    egui::RichText::new(format!("{}  {}", ic::PLAY, s.proxy_start))
                        .color(on_accent)
                        .strong(),
                )
                    .fill(ACCENT)
                    .rounding(4.0);
                if ui.add(start).clicked() { self.start(); }
            }

            ui.separator();

            ui.label(format!("{}  {}", ic::DOWNLOAD_SIMPLE, s.proxy_export_ca));
            ui.add_sized(
                [200.0, 22.0],
                egui::TextEdit::singleline(&mut self.ca_export_path),
            );
            if ui.button(s.proxy_export_button).clicked() { self.export_ca(); }
        });

        if let Some(msg) = &self.ca_export_msg {
            ui.colored_label(INFO, msg);
        }
        if let Some(err) = &self.error {
            ui.colored_label(ERR, format!("{}  {} {}", ic::X_CIRCLE, s.status_error, err));
        }
    }

    fn draw_table(&mut self, ui: &mut egui::Ui, s: &Strings) {
        let available = ui.available_height();
        let table_height = (available * 0.45).max(120.0);

        ui.label(
            egui::RichText::new(format!(
                "{}  {} {}",
                ic::LIST_MAGNIFYING_GLASS,
                s.proxy_captured,
                self.events.len()
            ))
                .strong()
                .color(WHITE),
        );

        egui::ScrollArea::vertical()
            .id_salt("events_scroll")
            .max_height(table_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("events_grid")
                    .striped(true)
                    .min_col_width(60.0)
                    .show(ui, |ui| {
                        self.draw_header_row(ui, s);
                        self.draw_body_rows(ui);
                    });
            });
    }

    fn draw_header_row(&self, ui: &mut egui::Ui, s: &Strings) {
        for col in [
            s.proxy_col_time,
            s.proxy_col_method,
            s.proxy_col_host,
            s.proxy_col_status,
            s.proxy_col_req_b,
            s.proxy_col_resp_b,
        ] {
            ui.label(egui::RichText::new(col).strong());
        }
        ui.end_row();
    }

    fn draw_body_rows(&mut self, ui: &mut egui::Ui) {
        let mut clicked: Option<usize> = None;

        for (i, ev) in self.events.iter().enumerate() {
            let selected = self.selected == Some(i);
            let color = if selected { ACCENT } else { WHITE };

            ui.label(egui::RichText::new(format_time(ev.timestamp)).color(color));
            ui.label(egui::RichText::new(extract_method(&ev.request)).color(color).strong());
            ui.label(egui::RichText::new(&ev.host).color(color));
            ui.label(egui::RichText::new(extract_status(ev.response.as_deref())).color(color));
            ui.label(egui::RichText::new(format!("{}", ev.request.len())).color(color));
            ui.label(
                egui::RichText::new(match &ev.response {
                    Some(r) => format!("{}", r.len()),
                    None => "—".into(),
                })
                    .color(color),
            );

            let resp = ui.interact(
                ui.min_rect(),
                egui::Id::new(("row", i)),
                egui::Sense::click(),
            );
            if resp.clicked() { clicked = Some(i); }
            ui.end_row();
        }

        if let Some(i) = clicked { self.selected = Some(i); }
    }

    fn draw_detail(&self, ui: &mut egui::Ui, s: &Strings) {
        let Some(i) = self.selected else {
            ui.label(egui::RichText::new(s.proxy_select_row).weak());
            return;
        };
        let Some(ev) = self.events.get(i) else { return; };

        ui.label(
            egui::RichText::new(format!(
                "{}  {}:{}  ·  {}  ·  {}",
                ic::CARET_RIGHT,
                ev.host,
                ev.port,
                extract_method(&ev.request),
                if ev.dropped { s.proxy_dropped } else { s.proxy_forwarded }
            ))
                .strong()
                .color(ACCENT),
        );

        egui::ScrollArea::vertical()
            .id_salt("event_detail_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(format!("{}  {}", ic::ARROW_RIGHT, s.proxy_request_header))
                        .strong()
                        .color(INFO),
                );
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(String::from_utf8_lossy(&ev.request)).monospace(),
                    )
                        .selectable(true),
                );
                ui.add_space(8.0);
                if let Some(resp) = &ev.response {
                    ui.label(
                        egui::RichText::new(format!(
                            "{}  {}",
                            ic::ARROW_LEFT,
                            s.proxy_response_header
                        ))
                            .strong()
                            .color(INFO),
                    );
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(String::from_utf8_lossy(resp)).monospace(),
                        )
                            .selectable(true),
                    );
                }
            });
    }
}

// -------- Helpers puros --------

fn extract_method(request: &[u8]) -> String {
    String::from_utf8_lossy(request)
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().next())
        .unwrap_or("?")
        .to_string()
}

fn extract_status(response: Option<&[u8]>) -> String {
    let Some(r) = response else { return "—".into() };
    String::from_utf8_lossy(r)
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .unwrap_or("?")
        .to_string()
}

fn format_time(t: SystemTime) -> String {
    match t.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(d) => {
            let secs = d.as_secs() % 86_400;
            format!("{:02}:{:02}:{:02}", secs / 3600, (secs % 3600) / 60, secs % 60)
        }
        Err(_) => "??:??:??".into(),
    }
}