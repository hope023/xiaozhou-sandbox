//! 左侧导航栏：品牌块、五个导航项、用量条、设置/锁定。

use eframe::egui::{self, CornerRadius, Stroke, Vec2};

use super::theme;
use super::{App, Nav, Panel};
use super::widgets::{self, Icon};

impl App {
    pub fn show_sidebar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("sidebar")
            .exact_size(214.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(theme::PANEL).inner_margin(egui::Margin::same(10)))
            .show(ui, |ui| {
                ui.add_space(4.0);

                // 品牌块
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(30.0), egui::Sense::hover());
                    ui.painter().rect_filled(r, CornerRadius::same(8), theme::ACCENT);
                    widgets::icon(ui.painter(), r.center(), Icon::Lock, theme::WHITE, 18.0);
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("小舟沙盒").size(15.0).color(theme::TEXT).strong());
                        ui.label(egui::RichText::new("已解锁").size(11.0).color(theme::ACCENT));
                    });
                });

                ui.add_space(12.0);

                // 导航项
                let file_count = self.status.as_ref().map(|s| s.files).unwrap_or(0);
                for (nav, icon, label, count) in [
                    (Nav::Dir, Icon::Folder, "全部文件", Some(file_count)),
                    (Nav::Image, Icon::Image, "图片", None),
                    (Nav::Video, Icon::Video, "视频", None),
                    (Nav::Text, Icon::Text, "文档", None),
                    (Nav::Trash, Icon::Trash, "回收站", None),
                ] {
                    let active = self.nav == nav;
                    let (rect, resp) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), 32.0),
                        egui::Sense::click(),
                    );
                    resp.widget_info(|| egui::WidgetInfo::selected(
                        egui::Role::Button,
                        true,
                        active,
                        label,
                    ));
                    if active {
                        ui.painter().rect_filled(rect, CornerRadius::same(8), theme::ACCENT_SOFT);
                    } else if resp.hovered() {
                        ui.painter().rect_filled(rect, CornerRadius::same(8), theme::HOVER);
                    }
                    let color = if active { theme::ACCENT } else { theme::TEXT };
                    widgets::icon(ui.painter(), rect.left_center() + Vec2::new(14.0, 0.0), icon, color, 16.0);
                    ui.painter().text(
                        rect.left_center() + Vec2::new(34.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        label,
                        egui::FontId::proportional(13.0),
                        color,
                    );
                    if let Some(n) = count {
                        ui.painter().text(
                            rect.right_center() - Vec2::new(10.0, 0.0),
                            egui::Align2::RIGHT_CENTER,
                            n.to_string(),
                            egui::FontId::proportional(11.5),
                            theme::FAINT,
                        );
                    }
                    if resp.clicked() && self.nav != nav {
                        self.nav = nav;
                        self.need_list = true;
                        self.select(None);
                    }
                }

                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.add_space(8.0);

                    // 设置 / 立即锁定（bottom_up：先添加的在最底下）
                    if nav_row(ui, Icon::Lock, "立即锁定").clicked() {
                        self.do_lock();
                    }
                    if nav_row(ui, Icon::Settings, "设置").clicked() {
                        if self.panel != Some(Panel::Settings) {
                            self.panel = Some(Panel::Settings);
                            self.settings_draft = crate::service::get_settings(&self.state).ok();
                            self.settings_capture =
                                crate::service::app_prefs(&self.state).capture_guard;
                            self.settings_pw = Default::default();
                        }
                    }

                    ui.add_space(8.0);

                    // 用量条
                    let (used, cipher) = self
                        .status
                        .as_ref()
                        .map(|s| (s.bytes, s.vault_bytes))
                        .unwrap_or((0, 0));
                    ui.label(
                        egui::RichText::new(format!(
                            "已用 {} · 密文 {}",
                            widgets::fmt_size(used),
                            widgets::fmt_size(cipher)
                        ))
                        .size(11.5)
                        .color(theme::MUTED),
                    );
                    let quota = 10.0 * 1024.0 * 1024.0 * 1024.0; // 10 GB 展示用配额
                    let frac = (used as f32 / quota).clamp(0.0, 1.0);
                    let (bar, _) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), 4.0),
                        egui::Sense::hover(),
                    );
                    ui.painter().rect_filled(bar, CornerRadius::same(2), theme::LINE);
                    if frac > 0.0 {
                        ui.painter().rect_filled(
                            egui::Rect::from_min_size(bar.left_top(), Vec2::new(bar.width() * frac, 4.0)),
                            CornerRadius::same(2),
                            theme::ACCENT,
                        );
                    }
                    ui.add_space(6.0);
                    ui.separator();
                });
            });
    }
}

/// 底部整行按钮（设置/锁定），样式与导航项一致。
fn nav_row(ui: &mut egui::Ui, icon: Icon, label: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), 30.0),
        egui::Sense::click(),
    );
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(8), theme::HOVER);
    }
    widgets::icon(ui.painter(), rect.left_center() + Vec2::new(14.0, 0.0), icon, theme::TEXT, 15.0);
    ui.painter().text(
        rect.left_center() + Vec2::new(34.0, 0.0),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(13.0),
        theme::TEXT,
    );
    resp
}

// Stroke 未用到的占位引用
#[allow(unused)]
fn _t(_: Stroke) {}
