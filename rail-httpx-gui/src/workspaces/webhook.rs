//! Aba "Webhook" — servidor HTTP local que loga requests recebidas.

use crate::config::AppConfig;
use crate::i18n::Strings;
use crate::theme::{ACCENT, ERR, INFO, OK, WHITE};
use eframe::egui;
use egui_phosphor::regular as ic;
use rail_httpx_core::webhook::{WebhookEvent, WebhookServer, WebhookSink};
use std::time::SystemTime;
use tokio::task::JoinHandle;

pub struct WebhookWorkspace {
    port: u16,
    running: bool,
    addr: Option<String>,
    events: Vec<WebhookEvent>,
    selected: Option<usize>,
    error: Option<String>,
    copied_notice_until: Option<std::time::Instant>,

    rt: tokio::runtime::Runtime,
    handle: Option<JoinHandle<()>>,
    events_rx: Option<tokio::sync::mpsc::UnboundedReceiver<WebhookEvent>>,
}

impl WebhookWorkspace {
    pub fn new() -> Self {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        Self {
            port: 9000,
            running: false,
            addr: None,
            events: Vec::new(),
            selected: None,
            error: None,
            copied_notice_until: None,
            rt,
            handle: None,
            events_rx: None,
        }
    }

    // -------- Ciclo de vida --------
    fn start(&mut self) {
        if self.running {
            return;
        }

        let addr = format!("127.0.0.1:{}", self.port);
        let (sink, rx) = WebhookSink::new();

        let server = match self.rt.block_on(WebhookServer::bind(&addr)) {
            Ok(s) => s.with_events(sink),
            Err(e) => {
                self.error = Some(format!("bind {addr}: {e}"));
                return;
            }
        };

        let local = match server.local_addr() {
            Ok(a) => a,
            Err(e) => {
                self.error = Some(format!("local_addr: {e}"));
                return;
            }
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
        if let Some(h) = self.handle.take() {
            h.abort();
        }
        self.running = false;
        self.addr = None;
        self.events_rx = None;
    }

    pub fn drain_events(&mut self) {
        let mut incoming = Vec::new();
        if let Some(rx) = &mut self.events_rx {
            while let Ok(ev) = rx.try_recv() {
                incoming.push(ev);
            }
        }
        if !incoming.is_empty() {
            incoming.reverse();
            self.events.splice(0..0, incoming);
            if self.events.len() > 1000 {
                self.events.truncate(1000);
            }
        }
    }

    fn endpoint_url(&self) -> Option<String> {
        self.addr.as_ref().map(|a| format!("http://{}/", a))
    }

    fn copy_endpoint(&mut self, ctx: &egui::Context) {
        if let Some(url) = self.endpoint_url() {
            ctx.output_mut(|o| o.copied_text = url);
            self.copied_notice_until =
                Some(std::time::Instant::now() + std::time::Duration::from_secs(2));
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
            ui.label(format!("{}  {}", ic::PLUGS, s.webhook_port));
            ui.add_sized(
                [80.0, 22.0],
                egui::DragValue::new(&mut self.port).range(1024..=65535),
            );

            if self.running {
                let stop = egui::Button::new(
                    egui::RichText::new(format!("{}  {}", ic::STOP, s.webhook_stop))
                        .color(WHITE)
                        .strong(),
                )
                    .fill(ERR)
                    .rounding(4.0);
                if ui.add(stop).clicked() {
                    self.stop();
                }
                if let Some(addr) = &self.addr {
                    ui.colored_label(
                        OK,
                        format!("{}  {} {}", ic::CIRCLE, s.webhook_running_at, addr),
                    );
                }
            } else {
                let start = egui::Button::new(
                    egui::RichText::new(format!("{}  {}", ic::PLAY, s.webhook_start))
                        .color(on_accent)
                        .strong(),
                )
                    .fill(ACCENT)
                    .rounding(4.0);
                if ui.add(start).clicked() {
                    self.start();
                }
            }
        });

        if self.running {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(s.webhook_endpoint).strong().color(INFO));
                if let Some(url) = self.endpoint_url() {
                    ui.label(egui::RichText::new(&url).monospace().color(WHITE));
                    if ui.button(format!("{}  {}", ic::COPY, s.webhook_copy)).clicked() {
                        // ctx é obtido via ui.ctx()
                        let ctx = ui.ctx().clone();
                        self.copy_endpoint(&ctx);
                    }
                    if let Some(t) = self.copied_notice_until {
                        if std::time::Instant::now() < t {
                            ui.colored_label(OK, s.webhook_copied);
                        }
                    }
                }
            });
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
                s.webhook_captured,
                self.events.len()
            ))
                .strong()
                .color(WHITE),
        );

        egui::ScrollArea::vertical()
            .id_salt("webhook_events_scroll")
            .max_height(table_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("webhook_events_grid")
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
            s.webhook_col_time,
            s.webhook_col_method,
            s.webhook_col_path,
            s.webhook_col_remote,
            s.webhook_col_body_b,
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
            ui.label(egui::RichText::new(&ev.method).color(color).strong());
            ui.label(egui::RichText::new(&ev.path).color(color));
            ui.label(egui::RichText::new(ev.remote_addr.ip().to_string()).color(color));
            ui.label(egui::RichText::new(format!("{}", ev.body.len())).color(color));

            let resp = ui.interact(
                ui.min_rect(),
                egui::Id::new(("webhook_row", i)),
                egui::Sense::click(),
            );
            if resp.clicked() {
                clicked = Some(i);
            }
            ui.end_row();
        }

        if let Some(i) = clicked {
            self.selected = Some(i);
        }
    }

    fn draw_detail(&self, ui: &mut egui::Ui, s: &Strings) {
        let Some(i) = self.selected else {
            ui.label(egui::RichText::new(s.webhook_select_row).weak());
            return;
        };
        let Some(ev) = self.events.get(i) else {
            return;
        };

        ui.label(
            egui::RichText::new(format!(
                "{}  {}  {}  ·  {}",
                ic::CARET_RIGHT,
                ev.method,
                ev.path,
                ev.remote_addr
            ))
                .strong()
                .color(ACCENT),
        );

        egui::ScrollArea::vertical()
            .id_salt("webhook_detail_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Headers
                ui.label(
                    egui::RichText::new(format!("{}  {}", ic::LIST_DASHES, s.webhook_headers_header))
                        .strong()
                        .color(INFO),
                );
                for (k, v) in &ev.headers {
                    ui.label(
                        egui::RichText::new(format!("{k}: {v}")).monospace().color(WHITE),
                    );
                }

                ui.add_space(8.0);

                // Body
                ui.label(
                    egui::RichText::new(format!("{}  {}", ic::CODE, s.webhook_body_header))
                        .strong()
                        .color(INFO),
                );
                if ev.body.is_empty() {
                    ui.label(egui::RichText::new("(vazio)").weak());
                } else if let Ok(text) = std::str::from_utf8(&ev.body) {
                    ui.add(
                        egui::Label::new(egui::RichText::new(text).monospace().color(WHITE))
                            .selectable(true),
                    );
                } else {
                    ui.label(
                        egui::RichText::new(format!("<{} bytes binários>", ev.body.len()))
                            .monospace()
                            .color(WHITE),
                    );
                }
            });
    }
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