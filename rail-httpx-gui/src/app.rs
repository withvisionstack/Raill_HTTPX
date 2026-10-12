//! Struct top-level do app.

use crate::config::{self, AppConfig};
use crate::i18n::Language;
use crate::theme::Theme;
use crate::ui::bars;
use crate::workspaces::{
    about::AboutWorkspace, db::DbWorkspace, fuzzer::FuzzerWorkspace, proxy::ProxyWorkspace,
    scanner::ScannerWorkspace, send::SendWorkspace, webhook::WebhookWorkspace, Workspace,
};
use eframe::egui;
use std::time::Duration;

pub struct RailHttpxApp {
    pub config: AppConfig,
    pub workspace: Workspace,
    pub send: SendWorkspace,
    pub proxy: ProxyWorkspace,
    pub fuzzer: FuzzerWorkspace,
    pub scanner: ScannerWorkspace,
    pub webhook: WebhookWorkspace,
    pub db: DbWorkspace,
    pub about: AboutWorkspace,
}

impl RailHttpxApp {
    pub fn new(config: AppConfig) -> Self {
        Self {
            config,
            workspace: Workspace::Send,
            send: SendWorkspace::new(),
            proxy: ProxyWorkspace::new(),
            fuzzer: FuzzerWorkspace::new(),
            scanner: ScannerWorkspace::new(),
            webhook: WebhookWorkspace::new(),
            db: DbWorkspace::new(),
            about: AboutWorkspace::new(),
        }
    }

    fn set_language(&mut self, l: Language) {
        self.config.set_language(l);
        let _ = config::save(&self.config);
    }

    fn set_theme(&mut self, t: Theme, ctx: &egui::Context) {
        self.config.set_theme(t);
        t.apply(ctx);
        let _ = config::save(&self.config);
    }
}

impl eframe::App for RailHttpxApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.config.theme().apply(ctx);

        self.send.drain();
        self.proxy.drain_events();
        self.fuzzer.drain();
        self.scanner.drain();
        self.webhook.drain_events();
        self.db.drain();

        if self.send.is_loading()
            || self.proxy.is_running()
            || self.fuzzer.is_running()
            || self.scanner.is_running()
            || self.db.is_running()
        {
            ctx.request_repaint_after(Duration::from_millis(200));
        }

        let s = self.config.language().strings();
        let theme = self.config.theme();

        egui::TopBottomPanel::top("tabs")
            .frame(egui::Frame::default().fill(theme.bar_bg()).inner_margin(6.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for w in Workspace::all() {
                        ui.selectable_value(
                            &mut self.workspace,
                            *w,
                            format!("{}  {}", w.icon(), w.label(s)),
                        );
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new("RAIL_HTTPX")
                                .color(theme.accent_text())
                                .strong(),
                        );
                        bars::language_switcher(ui, s, &self.config.language(), |l| {
                            self.set_language(l);
                        });
                        bars::theme_switcher(ui, s, theme, |t| {
                            self.set_theme(t, ctx);
                        });
                    });
                });
            });

        egui::TopBottomPanel::bottom("status")
            .frame(egui::Frame::default().fill(theme.bar_bg()).inner_margin(4.0))
            .show(ctx, |ui| {
                bars::status_bar(ui, s, &self.send, &self.proxy);
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(theme.central_bg()).inner_margin(8.0))
            .show(ctx, |ui| match self.workspace {
                Workspace::Send => self.send.draw(ui, &self.config),
                Workspace::Proxy => self.proxy.draw(ui, &self.config),
                Workspace::Fuzzer => self.fuzzer.draw(ui, &self.config),
                Workspace::Scanner => self.scanner.draw(ui, &self.config),
                Workspace::Webhook => self.webhook.draw(ui, &self.config),
                Workspace::Db => self.db.draw(ui, &self.config),
                Workspace::About => self.about.draw(ui, &self.config),
            });
    }
}