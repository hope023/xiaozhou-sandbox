//! 锁屏：顶部菜单栏（切换/新增库、关于）+ 创建 / 解锁双模式卡片。

use eframe::egui::{self, Align, CornerRadius, Layout, Margin, Stroke, Vec2};

use super::theme;
use super::widgets::{self, Icon};
use super::App;

/// 卡片内容宽度（不含内边距）。
const CARD_W: f32 = 360.0;
/// 输入框统一高度，和按钮同高，视觉上不再「一条细线配一个大按钮」。
const FIELD_H: f32 = 36.0;

impl App {
    /// 登录页顶部菜单栏：换库、加库、关于。
    pub fn show_lock_menu(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("lock-menu")
            .frame(
                egui::Frame::new()
                    .fill(theme::PANEL)
                    .inner_margin(Margin::symmetric(10, 3)),
            )
            .show_inside(ui, |ui| {
                egui::MenuBar::new().ui(ui, |ui| {
                    ui.menu_button("库", |ui| self.vault_menu(ui));
                    ui.menu_button("关于", |ui| {
                        if ui.button("关于小舟沙盒…").clicked() {
                            self.about_open = true;
                            ui.close();
                        }
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let cur = self.state.config.lock().unwrap().vault_path.clone();
                        if !cur.is_empty() {
                            ui.label(
                                egui::RichText::new(format!("当前库：{cur}"))
                                    .size(11.0)
                                    .color(theme::FAINT),
                            );
                        }
                    });
                });
            });
    }

    fn vault_menu(&mut self, ui: &mut egui::Ui) {
        let (recent, cur) = {
            let cfg = self.state.config.lock().unwrap();
            (cfg.recent_vaults.clone(), cfg.vault_path.clone())
        };

        if recent.is_empty() {
            ui.label(
                egui::RichText::new("还没有打开过别的库")
                    .size(11.5)
                    .color(theme::FAINT),
            );
        } else {
            for p in recent {
                let active = p.eq_ignore_ascii_case(&cur);
                let mark = if active { "● " } else { "   " };
                let shown = if p.chars().count() > 46 {
                    let head: String = p.chars().take(44).collect();
                    format!("{mark}{head}…")
                } else {
                    format!("{mark}{p}")
                };
                let text = egui::RichText::new(shown).size(11.5);
                if ui.button(text).clicked() {
                    self.switch_vault(&p);
                    ui.close();
                }
            }
        }

        ui.separator();
        if ui.button("选择别的目录…").clicked() {
            self.pick_existing_vault();
            ui.close();
        }
        if ui.button("添加新目录…").clicked() {
            self.begin_create_new();
            ui.close();
        }
    }

    pub fn show_lock_screen(&mut self, ui: &mut egui::Ui) {
        let exists = self.status.as_ref().map(|s| s.exists).unwrap_or(false);
        let creating = self.lock_force_create || !exists;
        let path_hint = self
            .status
            .as_ref()
            .map(|s| s.path.clone())
            .unwrap_or_default();

        // 用 Area 锚在正中间：CentralPanel 的「居中 + 撑满」布局会把卡片拉成整窗宽，
        // 旧版登录页看起来就是一条横贯屏幕的白色长条。
        egui::Area::new(egui::Id::new("lock-card"))
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .order(egui::Order::Middle)
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(theme::PANEL)
                    .stroke(Stroke::new(1.0, theme::LINE))
                    .corner_radius(CornerRadius::same(16))
                    .inner_margin(Margin::same(24))
                    .show(ui, |ui| {
                        ui.set_width(CARD_W);
                        ui.with_layout(Layout::top_down(Align::Min), |ui| {
                            self.brand_row(ui);
                            ui.add_space(18.0);

                            if creating {
                                self.create_form(ui, exists);
                            } else {
                                self.unlock_form(ui, &path_hint);
                            }
                        });
                    });
            });
    }

    fn brand_row(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(34.0), egui::Sense::hover());
            ui.painter()
                .rect_filled(r, CornerRadius::same(9), theme::ACCENT);
            widgets::icon(ui.painter(), r.center(), Icon::Lock, theme::WHITE, 20.0);
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new("小舟沙盒")
                        .size(15.0)
                        .strong()
                        .color(theme::TEXT),
                );
                ui.label(
                    egui::RichText::new("本地加密保险库")
                        .size(11.5)
                        .color(theme::MUTED),
                );
            });
        });
    }

    fn unlock_form(&mut self, ui: &mut egui::Ui, path_hint: &str) {
        ui.label(
            egui::RichText::new("解锁保险库")
                .size(17.0)
                .color(theme::TEXT),
        );
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(path_hint)
                .size(11.5)
                .color(theme::FAINT),
        );
        ui.add_space(12.0);

        let pw = ui.add_sized(
            Vec2::new(CARD_W, FIELD_H),
            egui::TextEdit::singleline(&mut self.lock_pw)
                .password(true)
                .hint_text("输入口令")
                .margin(Margin::symmetric(10, 9)),
        );
        // 只在首帧抢一次焦点。旧代码每帧都 request_focus，
        // 结果点「浏览」或想切到别的输入框都会被立刻拽回来。
        if self.lock_focus_pending {
            pw.request_focus();
            self.lock_focus_pending = false;
        }
        if pw.changed() {
            self.lock_err.clear();
        }
        let submit = pw.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

        self.error_line(ui);
        ui.add_space(6.0);

        if self.wide_primary(ui, "解锁").clicked() || submit {
            self.try_unlock();
        }
        ui.add_space(10.0);
        ui.label(
            egui::RichText::new("密码在内存中派生，落盘的一切都是密文。空闲会自动锁定。")
                .size(11.5)
                .color(theme::FAINT),
        );
    }

    fn create_form(&mut self, ui: &mut egui::Ui, can_go_back: bool) {
        ui.label(
            egui::RichText::new("创建保险库")
                .size(17.0)
                .color(theme::TEXT),
        );
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            let browse_w = 60.0;
            let resp = ui.add_sized(
                Vec2::new(CARD_W - browse_w - 8.0, FIELD_H),
                egui::TextEdit::singleline(&mut self.lock_path)
                    .hint_text("新库的位置（留空用默认位置）")
                    .margin(Margin::symmetric(10, 9)),
            );
            if self.lock_focus_pending {
                resp.request_focus();
                self.lock_focus_pending = false;
            }
            let browse = ui.add_sized(
                Vec2::new(browse_w, FIELD_H),
                egui::Button::new("浏览")
                    .fill(theme::PANEL)
                    .stroke(Stroke::new(1.0, theme::LINE_STRONG))
                    .corner_radius(CornerRadius::same(8)),
            );
            if browse.clicked() {
                if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                    self.lock_path = dir.display().to_string();
                }
            }
        });

        ui.add_space(8.0);
        let pw1 = ui.add_sized(
            Vec2::new(CARD_W, FIELD_H),
            egui::TextEdit::singleline(&mut self.lock_pw)
                .password(true)
                .hint_text("设置主口令（至少 8 个字符）")
                .margin(Margin::symmetric(10, 9)),
        );
        ui.add_space(8.0);
        let pw2 = ui.add_sized(
            Vec2::new(CARD_W, FIELD_H),
            egui::TextEdit::singleline(&mut self.lock_pw2)
                .password(true)
                .hint_text("再输一遍")
                .margin(Margin::symmetric(10, 9)),
        );
        if pw1.changed() || pw2.changed() {
            self.lock_err.clear();
        }

        // ── 第二口令：平时只是一个不起眼的链接，点开才多一个输入框 ──
        ui.add_space(8.0);
        if self.lock_second_open {
            ui.add_sized(
                Vec2::new(CARD_W, FIELD_H),
                egui::TextEdit::singleline(&mut self.lock_second)
                    .password(true)
                    .hint_text("第二口令（可选，至少 8 个字符）")
                    .margin(Margin::symmetric(10, 9)),
            );
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(
                    "同一目录里再开一套独立内容：用主口令看到第一套，用第二口令看到第二套，两边互相看不见。",
                )
                .size(11.0)
                .color(theme::FAINT),
            );
        } else if ui
            .link(egui::RichText::new("＋ 第二口令（可选）").size(11.5))
            .clicked()
        {
            self.lock_second_open = true;
        }

        let submit = pw2.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

        self.error_line(ui);
        ui.add_space(6.0);

        if self.wide_primary(ui, "创建并进入").clicked() || submit {
            self.try_create();
        }
        ui.add_space(10.0);
        widgets::warn_box(
            ui,
            "密码无法找回。忘记密码 = 里面的所有文件永远读不出来，没有任何后门。",
        );

        if can_go_back {
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("只是想打开已有的库？")
                        .size(11.5)
                        .color(theme::FAINT),
                );
                if ui
                    .link(egui::RichText::new("返回解锁").size(11.5))
                    .clicked()
                {
                    self.lock_force_create = false;
                    self.lock_focus_pending = true;
                    self.lock_err.clear();
                }
            });
        } else {
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("已经有库了？")
                        .size(11.5)
                        .color(theme::FAINT),
                );
                if ui
                    .link(egui::RichText::new("打开已有保险库…").size(11.5))
                    .clicked()
                {
                    self.pick_existing_vault();
                }
            });
        }
    }

    /// 错误行固定占一行高度，出现/消失不会把下面的按钮顶来顶去。
    fn error_line(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.allocate_ui(Vec2::new(CARD_W, 18.0), |ui| {
            if !self.lock_err.is_empty() {
                ui.label(
                    egui::RichText::new(&self.lock_err)
                        .size(12.0)
                        .color(theme::DANGER),
                );
            }
        });
    }

    fn wide_primary(&self, ui: &mut egui::Ui, text: &str) -> egui::Response {
        ui.add_sized(
            Vec2::new(CARD_W, FIELD_H),
            egui::Button::new(egui::RichText::new(text).size(13.5).color(theme::WHITE))
                .fill(theme::ACCENT)
                .corner_radius(CornerRadius::same(8)),
        )
    }

    /// 选一个已有库目录 → 写进配置 → 切到解锁模式。
    fn pick_existing_vault(&mut self) {
        let Some(dir) = rfd::FileDialog::new()
            .set_title("选择保险库目录（含 vault.json 的那个文件夹）")
            .pick_folder()
        else {
            return;
        };
        match crate::service::open_vault_at(&self.state, &dir.display().to_string()) {
            Ok(_) => self.enter_unlock_mode("已找到保险库，输入口令即可打开"),
            Err(e) => self.lock_err = e.to_string(),
        }
    }

    /// 切到最近打开过的库。
    fn switch_vault(&mut self, path: &str) {
        match crate::service::open_vault_at(&self.state, path) {
            Ok(_) => self.enter_unlock_mode(format!("已切换到 {path}")),
            Err(e) => {
                self.lock_err = e.to_string();
                // 目录已经没了 / 不是库：从最近列表里摘掉，别一直挂着
                let mut cfg = self.state.config.lock().unwrap();
                cfg.recent_vaults.retain(|p| !p.eq_ignore_ascii_case(path));
                drop(cfg);
                let _ = self.state.save_config();
            }
        }
    }

    fn enter_unlock_mode(&mut self, toast: impl Into<String>) {
        self.lock_err.clear();
        self.lock_pw.clear();
        self.lock_force_create = false;
        self.lock_focus_pending = true;
        self.need_status = true;
        self.notify(toast);
    }

    /// 菜单里的「添加新目录…」：即使已有库也进入创建表单。
    fn begin_create_new(&mut self) {
        self.lock_force_create = true;
        self.lock_path.clear();
        self.lock_pw.clear();
        self.lock_pw2.clear();
        self.lock_second.clear();
        self.lock_second_open = false;
        self.lock_err.clear();
        self.lock_focus_pending = true;
    }

    pub(crate) fn try_unlock(&mut self) {
        match crate::service::unlock(&self.state, &self.lock_pw) {
            Ok(_) => {
                self.lock_pw.clear();
                self.lock_err.clear();
                self.need_status = true;
                self.need_list = true;
                self.notify("已解锁");
            }
            Err(e) => self.lock_err = e.to_string(),
        }
    }

    fn try_create(&mut self) {
        if self.lock_pw.chars().count() < 8 {
            self.lock_err = "密码至少 8 个字符".into();
            return;
        }
        if self.lock_pw != self.lock_pw2 {
            self.lock_err = "两次输入的密码不一致".into();
            return;
        }
        let second = if self.lock_second_open && !self.lock_second.trim().is_empty() {
            Some(self.lock_second.clone())
        } else {
            None
        };
        match crate::service::create_vault(
            &self.state,
            &self.lock_path,
            &self.lock_pw,
            second.as_deref(),
        ) {
            Ok(_) => {
                let msg = if second.is_some() {
                    "保险库已创建（含第二口令），落盘的都是密文"
                } else {
                    "保险库已创建，落盘的都是密文"
                };
                self.lock_pw.clear();
                self.lock_pw2.clear();
                self.lock_second.clear();
                self.lock_second_open = false;
                self.lock_err.clear();
                self.lock_force_create = false;
                self.need_status = true;
                self.need_list = true;
                self.notify(msg);
            }
            Err(e) => self.lock_err = e.to_string(),
        }
    }
}
