//! 底部状态栏：解锁状态点、库路径、文件数、密文体积、自动锁定倒计时说明。

use eframe::egui::{self, Align, Layout};

use super::theme;
use super::App;

impl App {
    pub fn show_statusbar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("statusbar")
            .exact_size(28.0)
            .frame(egui::Frame::new().fill(theme::BG).inner_margin(egui::Margin::symmetric(12, 0)))
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    let (r, _) = ui.allocate_exact_size(egui::vec2(8.0, 16.0), egui::Sense::hover());
                    ui.painter().circle_filled(r.center(), 3.5, theme::ACCENT);
                    ui.label(egui::RichText::new("已解锁").size(12.0).color(theme::MUTED));

                    if let Some(s) = &self.status {
                        ui.label(
                            egui::RichText::new(s.path.as_str())
                                .size(12.0)
                                .color(theme::FAINT)
                                .weak(),
                        );
                        ui.separator();
                        ui.label(egui::RichText::new(format!("{} 个文件", s.files)).size(12.0).color(theme::MUTED));
                        ui.label(
                            egui::RichText::new(format!("密文 {}", super::widgets::fmt_size(s.vault_bytes)))
                                .size(12.0)
                                .color(theme::MUTED),
                        );
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let mins = self
                            .state
                            .read(|v| Ok(v.index.settings.auto_lock_minutes))
                            .unwrap_or(5);
                        let text = if mins > 0 {
                            format!("空闲 {mins} 分钟后自动锁定")
                        } else {
                            "自动锁定已关闭".to_string()
                        };
                        ui.label(egui::RichText::new(text).size(12.0).color(theme::FAINT));
                    });
                });
            });
    }
}
