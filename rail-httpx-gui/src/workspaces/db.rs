//! Aba "DB" — conecta em PostgreSQL/MySQL/SQLite e executa SQL.

use crate::config::AppConfig;
use crate::i18n::Strings;
use crate::theme::{ACCENT, ERR, INFO, OK, WHITE};
use eframe::egui;
use egui_phosphor::regular as ic;
use rail_httpx_core::db::{Database, QueryOutput};
use std::sync::mpsc;
use std::sync::Arc;

enum DbMessage {
    ConnectedWithDb(Arc<Database>),
    ConnectError(String),
    Result(QueryOutput),
    Error(String),
}

pub struct DbWorkspace {
    url: String,
    sql_text: String,

    db: Option<Arc<Database>>,
    connecting: bool,
    running: bool,
    error: Option<String>,
    last_output: Option<QueryOutput>,

    tx: mpsc::Sender<DbMessage>,
    rx: mpsc::Receiver<DbMessage>,
}

impl DbWorkspace {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            url: "sqlite::memory:".to_string(),
            sql_text: "SELECT 1 AS one;".to_string(),
            db: None,
            connecting: false,
            running: false,
            error: None,
            last_output: None,
            tx,
            rx,
        }
    }

    pub fn is_running(&self) -> bool {
        self.connecting || self.running
    }

    // -------- Ações --------
    fn connect(&mut self) {
        if self.connecting {
            return;
        }
        let url = self.url.clone();
        let tx = self.tx.clone();
        self.connecting = true;
        self.error = None;

        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime");
            match rt.block_on(Database::connect(&url)) {
                Ok(db) => {
                    let _ = tx.send(DbMessage::ConnectedWithDb(Arc::new(db)));
                }
                Err(e) => {
                    let _ = tx.send(DbMessage::ConnectError(e.to_string()));
                }
            }
        });
    }

    fn disconnect(&mut self) {
        self.db = None;
        self.last_output = None;
        self.error = None;
    }

    fn run_query(&mut self) {
        let Some(db) = self.db.clone() else {
            self.error = Some("db_requires_connection".into());
            return;
        };
        if self.running {
            return;
        }
        let sql = self.sql_text.clone();
        let tx = self.tx.clone();
        self.running = true;
        self.error = None;
        self.last_output = None;

        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime");
            match rt.block_on(db.query(&sql)) {
                Ok(out) => {
                    let _ = tx.send(DbMessage::Result(out));
                }
                Err(e) => {
                    let _ = tx.send(DbMessage::Error(e.to_string()));
                }
            }
        });
    }

    pub fn drain(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            self.connecting = false;
            self.running = false;
            match msg {
                DbMessage::ConnectedWithDb(db) => {
                    self.db = Some(db);
                    self.error = None;
                }
                DbMessage::ConnectError(e) => {
                    self.error = Some(format!("connect: {e}"));
                    self.db = None;
                }
                DbMessage::Result(out) => {
                    self.last_output = Some(out);
                    self.error = None;
                }
                DbMessage::Error(e) => {
                    self.error = Some(e);
                    self.last_output = None;
                }
            }
        }
    }

    // -------- Render --------
    pub fn draw(&mut self, ui: &mut egui::Ui, config: &AppConfig) {
        let s = config.language().strings();
        self.draw_toolbar(ui, s, config);
        ui.separator();
        self.draw_editor(ui, s, config);
        ui.separator();
        self.draw_result(ui, s);
    }

    fn draw_toolbar(&mut self, ui: &mut egui::Ui, s: &Strings, config: &AppConfig) {
        let on_accent = config.theme().on_accent();

        ui.horizontal(|ui| {
            ui.label(format!("{}  {}", ic::PLUGS, s.db_url_label));
            let remaining = ui.available_width();
            let btn_w = 130.0;
            ui.add_sized(
                [remaining - btn_w - 8.0, 24.0],
                egui::TextEdit::singleline(&mut self.url).hint_text(s.db_url_hint),
            );

            if self.db.is_some() {
                let btn = egui::Button::new(
                    egui::RichText::new(format!("{}  {}", ic::X, s.db_disconnect))
                        .color(WHITE)
                        .strong(),
                )
                    .fill(ERR)
                    .rounding(4.0);
                if ui.add_sized([btn_w, 24.0], btn).clicked() {
                    self.disconnect();
                }
                ui.colored_label(OK, s.db_connected);
            } else {
                let btn = egui::Button::new(
                    egui::RichText::new(format!("{}  {}", ic::LINK, s.db_connect))
                        .color(on_accent)
                        .strong(),
                )
                    .fill(ACCENT)
                    .rounding(4.0);
                if ui.add_enabled(!self.connecting, btn).clicked() {
                    self.connect();
                }
                if self.connecting {
                    ui.spinner();
                    ui.label(s.db_connecting);
                }
            }
        });

        if let Some(err) = &self.error {
            let text = match err.as_str() {
                "db_requires_connection" => s.db_requires_connection,
                other => other,
            };
            ui.colored_label(ERR, format!("{}  {}", ic::X_CIRCLE, text));
        }
    }

    fn draw_editor(&mut self, ui: &mut egui::Ui, s: &Strings, config: &AppConfig) {
        let on_accent = config.theme().on_accent();

        ui.label(
            egui::RichText::new(format!("{}  {}", ic::CODE, s.db_sql_label))
                .strong()
                .color(INFO),
        );

        egui::ScrollArea::vertical()
            .id_salt("db_sql_scroll")
            .max_height(140.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut self.sql_text)
                        .desired_rows(6)
                        .desired_width(f32::INFINITY)
                        .font(egui::TextStyle::Monospace),
                );
            });

        // Barra de ações em linha separada — botão Executar à esquerda.
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let connected = self.db.is_some();
            let can_run = connected && !self.running;

            // Botão colorido quando habilitado; cinza quando não.
            let btn = if can_run {
                egui::Button::new(
                    egui::RichText::new(format!("{}  {}", ic::PLAY, s.db_run))
                        .color(on_accent)
                        .strong(),
                )
                    .fill(ACCENT)
                    .rounding(4.0)
            } else {
                egui::Button::new(format!("{}  {}", ic::PLAY, s.db_run))
            };

            if ui.add_enabled(can_run, btn).clicked() {
                self.run_query();
            }

            if self.running {
                ui.spinner();
                ui.label(s.db_running);
            } else if !connected {
                ui.colored_label(ERR, s.db_requires_connection);
            }
        });
    }

    fn draw_result(&self, ui: &mut egui::Ui, s: &Strings) {
        let Some(out) = &self.last_output else {
            ui.label(egui::RichText::new(s.db_empty).weak());
            return;
        };

        ui.horizontal(|ui| {
            if let Some(n) = out.affected {
                ui.colored_label(
                    OK,
                    format!("{} {} {}ms", n, s.db_affected_info, out.duration_ms),
                );
            } else {
                ui.colored_label(
                    OK,
                    format!("{} {} {}ms", out.rows.len(), s.db_rows_info, out.duration_ms),
                );
            }
        });

        if out.affected.is_some() || out.rows.is_empty() {
            return;
        }

        egui::ScrollArea::both()
            .id_salt("db_result_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("db_result_grid")
                    .striped(true)
                    .min_col_width(80.0)
                    .show(ui, |ui| {
                        for col in &out.columns {
                            ui.label(egui::RichText::new(col).strong().color(ACCENT));
                        }
                        ui.end_row();

                        for row in &out.rows {
                            if let Some(obj) = row.as_object() {
                                for col in &out.columns {
                                    let val = obj
                                        .get(col)
                                        .map(json_cell_to_string)
                                        .unwrap_or_else(|| "—".into());
                                    ui.label(
                                        egui::RichText::new(val).monospace().color(WHITE),
                                    );
                                }
                                ui.end_row();
                            }
                        }
                    });
            });
    }
}

fn json_cell_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "NULL".into(),
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}