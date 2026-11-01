//! 右侧滑入面板：版本历史 / 设置 / 全文搜索。

use std::time::Instant;

use eframe::egui::{self, Align, Layout, CornerRadius, Stroke};

use super::theme;
use super::widgets::{self, Icon};
use super::{App, Panel};

impl App {
    pub fn show_right_panel(&mut self, ui: &mut egui::Ui) {
        match self.panel {
            Some(Panel::Versions) => self.show_versions_panel(ui),
            Some(Panel::Settings) => self.show_settings_panel(ui),
            Some(Panel::Search) => self.show_search_panel(ui),
            None => {}
        }
    }

    fn close_panel(&mut self) {
        self.panel = None;
        self.settings_draft = None;
    }

    // ─────────────────────────── 版本历史 ───────────────────────────

    fn show_versions_panel(&mut self, ui: &mut egui::Ui) {
        let name = self
            .selected
            .as_ref()
            .filter(|s| s.id == self.versions_for)
            .map(|s| s.name.clone())
            .unwrap_or_default();

        egui::Panel::right("versions-panel")
            .exact_size(460.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(theme::PANEL).inner_margin(egui::Margin::same(14)))
            .show(ui, |ui| {
                panel_header(ui, "版本历史", &name, || self.close_panel());

                ui.label(
                    egui::RichText::new("每次保存都会留一份快照。旧版本按「近密远疏」自动清理，钉住的永远保留。")
                        .size(11.5)
                        .color(theme::FAINT),
                );
                ui.add_space(6.0);

                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    let versions = self.versions.clone();
                    let id = self.versions_for.clone();
                    if versions.is_empty() {
                        ui.add_space(20.0);
                        ui.vertical_centered(|ui| {
                            ui.label(egui::RichText::new("还没有历史版本").size(12.5).color(theme::FAINT));
                        });
                        return;
                    }
                    for v in versions {
                        let row = egui::Frame::new()
                            .fill(if v.current { theme::ACCENT_SOFT } else { theme::PANEL })
                            .stroke(Stroke::new(
                                1.0,
                                if v.current { theme::ACCENT_LINE } else { theme::LINE },
                            ))
                            .corner_radius(CornerRadius::same(10))
                            .inner_margin(egui::Margin::symmetric(10, 8));
                        row.show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(
                                        egui::RichText::new(widgets::fmt_datetime(v.created_at))
                                            .size(13.0)
                                            .color(theme::TEXT),
                                    );
                                    let mut sub = widgets::fmt_size(v.size).to_string();
                                    if v.current {
                                        sub.push_str(" · 当前版本");
                                    } else if v.pinned {
                                        sub.push_str(" · 已钉住");
                                    }
                                    ui.label(egui::RichText::new(sub).size(11.5).color(theme::MUTED));
                                });

                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if !v.current {
                                        if widgets::ghost_button(ui, "恢复").clicked() {
                                            match crate::service::restore_version(&self.state, &id, &v.id) {
                                                Ok(e2) => {
                                                    self.notify("已恢复到该版本（会生成一个新版本，历史不丢）");
                                                    self.versions =
                                                        crate::service::list_versions(&self.state, &id).unwrap_or_default();
                                                    if let Some(s) = self.selected.as_mut() {
                                                        if s.id == e2.id {
                                                            *s = e2;
                                                        }
                                                    }
                                                    // 内容变了，编辑器重新加载
                                                    self.editor = None;
                                                    self.need_list = true;
                                                }
                                                Err(err) => self.notify(format!("恢复失败：{err}")),
                                            }
                                        }
                                    }
                                    let pinned = v.pinned;
                                    let pin_btn = egui::Button::new("").fill(egui::Color32::TRANSPARENT);
                                    let resp = ui.add_sized([28.0, 28.0], pin_btn);
                                    let color = if pinned { theme::ACCENT } else { theme::FAINT };
                                    widgets::icon(ui.painter(), resp.rect.center(), Icon::Pin, color, 15.0);
                                    if resp.clicked() {
                                        if let Err(err) =
                                            crate::service::pin_version(&self.state, &id, &v.id, !pinned)
                                        {
                                            self.notify(format!("操作失败：{err}"));
                                        }
                                        self.versions =
                                            crate::service::list_versions(&self.state, &id).unwrap_or_default();
                                    }
                                    resp.on_hover_text(if pinned { "取消钉住" } else { "钉住（清理时永不删除）" });
                                });
                            });
                        });
                        ui.add_space(6.0);
                    }
                });
            });
    }

    // ─────────────────────────── 设置 ───────────────────────────

    fn show_settings_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::right("settings-panel")
            .exact_size(500.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(theme::PANEL).inner_margin(egui::Margin::same(14)))
            .show(ui, |ui| {
                panel_header(ui, "设置", "", || self.close_panel());

                // 草稿不存在（比如库被锁又解锁）就重新拉
                if self.settings_draft.is_none() {
                    self.settings_draft = crate::service::get_settings(&self.state).ok();
                    self.settings_capture = crate::service::app_prefs(&self.state).capture_guard;
                }
                if self.settings_draft.is_none() {
                    ui.label(egui::RichText::new("设置不可用").color(theme::MUTED));
                    return;
                };

                // 底部操作条先占位，避免上面的滚动区把按钮挤出面板
                egui::Panel::bottom("settings-footer")
                    .exact_size(52.0)
                    .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(0, 6)))
                    .show_inside(ui, |ui| {
                        ui.label(
                            egui::RichText::new("设置保存在加密索引里，别人看不到你开了哪些功能")
                                .size(11.0)
                                .color(theme::FAINT),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if widgets::primary_button(ui, "保存设置").clicked() {
                                if let Some(draft) = self.settings_draft.take() {
                                    match crate::service::update_settings(&self.state, draft) {
                                        Ok(_) => self.notify("设置已保存"),
                                        Err(e) => self.notify(format!("保存失败：{e}")),
                                    }
                                }
                                self.close_panel();
                            }
                            if widgets::ghost_button(ui, "取消").clicked() {
                                self.close_panel();
                            }
                        });
                    });

                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    section(ui, "安全");

                    // 反截屏（应用级偏好，不用解锁）
                    let mut cap = self.settings_capture;
                    if ui.checkbox(&mut cap, "").changed()
                    {
                        match crate::service::set_capture_guard(&self.state, self.hwnd.unwrap_or(0), cap) {
                            Ok(applied) => {
                                self.settings_capture = cap;
                                if cap && !applied {
                                    self.notify("这台系统不支持反截屏（需要 Win10 2004 及以上）");
                                    self.settings_capture = false;
                                }
                            }
                            Err(e) => self.notify(format!("设置失败：{e}")),
                        }
                    }
                    setting_row_rest(ui, "反截屏", "阻止其他程序截取本窗口画面");

                    // 空闲自动锁定
                    let mut d = self.settings_draft.clone().unwrap();
                    let lock_opts: [(u32, &str); 6] = [
                        (0, "不自动锁"),
                        (1, "1 分钟"),
                        (5, "5 分钟"),
                        (15, "15 分钟"),
                        (30, "30 分钟"),
                        (60, "60 分钟"),
                    ];
                    let cur = lock_opts
                        .iter()
                        .find(|(m, _)| *m == d.auto_lock_minutes)
                        .map(|(_, t)| t.to_string())
                        .unwrap_or_else(|| format!("{} 分钟", d.auto_lock_minutes));
                    egui::ComboBox::from_id_salt("auto-lock")
                        .selected_text(cur)
                        .width(140.0)
                        .show_ui(ui, |ui| {
                            for (m, t) in lock_opts {
                                ui.selectable_value(&mut d.auto_lock_minutes, m, t);
                            }
                        });
                    setting_row_rest(ui, "空闲自动锁定", "超过这段时间没有操作就自动锁库");
                    self.settings_draft = Some(d);

                    ui.add_space(10.0);
                    section(ui, "版本历史");

                    let mut d = self.settings_draft.clone().unwrap();
                    let ret_opts: [(&str, &str); 3] = [
                        ("tiered", "近密远疏（推荐）"),
                        ("all", "全部保留"),
                        ("off", "只留最新"),
                    ];
                    let cur = ret_opts
                        .iter()
                        .find(|(k, _)| *k == d.version_retention)
                        .map(|(_, t)| t.to_string())
                        .unwrap_or_default();
                    egui::ComboBox::from_id_salt("retention")
                        .selected_text(cur)
                        .width(170.0)
                        .show_ui(ui, |ui| {
                            for (k, t) in ret_opts {
                                ui.selectable_value(&mut d.version_retention, k.to_string(), t);
                            }
                        });
                    setting_row_rest(ui, "保留策略", "越久远的版本越稀疏，占用很小");
                    ui.add(
                        egui::DragValue::new(&mut d.max_versions)
                            .range(1..=5000)
                            .suffix(" 个"),
                    );
                    setting_row_rest(ui, "单文件版本上限", "超出后从最旧的未钉住版本开始清");
                    self.settings_draft = Some(d);

                    ui.add_space(10.0);
                    section(ui, "备份");

                    let mut d = self.settings_draft.clone().unwrap();
                    if ui.checkbox(&mut d.backup.enabled, "").changed() {
                        // 启用备份是免费能力，直接生效
                    }
                    setting_row_rest(ui, "启用备份", "复制密文，不需要解密");

                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut d.backup.target)
                                .hint_text("例如 D:\\Backup\\XVault")
                                .desired_width(ui.available_width() - 76.0),
                        );
                        if widgets::ghost_button(ui, "浏览").clicked() {
                            if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                                d.backup.target = dir.display().to_string();
                            }
                        }
                    });
                    setting_row_rest(ui, "备份位置", "建议放在另一块物理磁盘上");

                    let mode_opts: [(&str, &str); 2] =
                        [("mirror", "镜像（省空间）"), ("snapshot", "快照（可回滚）")];
                    let cur = mode_opts
                        .iter()
                        .find(|(k, _)| *k == d.backup.mode)
                        .map(|(_, t)| t.to_string())
                        .unwrap_or_default();
                    egui::ComboBox::from_id_salt("bk-mode")
                        .selected_text(cur)
                        .width(170.0)
                        .show_ui(ui, |ui| {
                            for (k, t) in mode_opts {
                                ui.selectable_value(&mut d.backup.mode, k.to_string(), t);
                            }
                        });
                    setting_row_rest(ui, "备份方式", "");

                    if d.backup.mode == "snapshot" {
                        ui.add(egui::DragValue::new(&mut d.backup.keep_copies).range(1..=100).suffix(" 份"));
                        setting_row_rest(ui, "快照保留", "最多留几份带时间戳的快照");
                    }

                    let iv_opts: [(u32, &str); 6] = [
                        (15, "15 分钟"),
                        (30, "30 分钟"),
                        (60, "1 小时"),
                        (180, "3 小时"),
                        (720, "12 小时"),
                        (1440, "1 天"),
                    ];
                    let cur = iv_opts
                        .iter()
                        .find(|(m, _)| *m == d.backup.interval_minutes)
                        .map(|(_, t)| t.to_string())
                        .unwrap_or_else(|| format!("{} 分钟", d.backup.interval_minutes));
                    egui::ComboBox::from_id_salt("bk-interval")
                        .selected_text(cur)
                        .width(140.0)
                        .show_ui(ui, |ui| {
                            for (m, t) in iv_opts {
                                ui.selectable_value(&mut d.backup.interval_minutes, m, t);
                            }
                        });
                    let last = if d.backup.last_backup_at > 0 {
                        format!("上次备份：{}", widgets::fmt_datetime(d.backup.last_backup_at))
                    } else {
                        "还没有备份过".to_string()
                    };
                    setting_row_rest(ui, "自动备份间隔", &last);

                    ui.add_space(4.0);
                    if widgets::ghost_button(ui, "立即备份一次").clicked() {
                        // 先保存设置再备份，保证目标目录是最新的
                        if let Some(draft) = self.settings_draft.take() {
                            match crate::service::update_settings(&self.state, draft.clone()) {
                                Ok(s) => self.settings_draft = Some(s),
                                Err(e) => self.notify(format!("保存设置失败：{e}")),
                            }
                        }
                        match crate::service::backup_now(&self.state) {
                            Ok(rep) => {
                                self.notify(rep.message.clone());
                                self.settings_draft = crate::service::get_settings(&self.state).ok();
                            }
                            Err(e) => self.notify(format!("备份失败：{e}")),
                        }
                    }
                    self.settings_draft = Some(d);

                    ui.add_space(10.0);
                    section(ui, "更换主密码");

                    let (old, next, again) = self.settings_pw.clone();
                    let old_e = ui.add(egui::TextEdit::singleline(&mut self.settings_pw.0).password(true).hint_text("当前密码").desired_width(280.0));
                    ui.add_space(3.0);
                    let next_e = ui.add(egui::TextEdit::singleline(&mut self.settings_pw.1).password(true).hint_text("新密码（至少 8 个字符）").desired_width(280.0));
                    ui.add_space(3.0);
                    let again_e = ui.add(egui::TextEdit::singleline(&mut self.settings_pw.2).password(true).hint_text("再输一遍新密码").desired_width(280.0));
                    let _ = (old_e, next_e, again_e);
                    ui.add_space(4.0);
                    if widgets::ghost_button(ui, "更换密码").clicked() {
                        if next.chars().count() < 8 {
                            self.notify("新密码至少 8 个字符");
                        } else if next != again {
                            self.notify("两次输入的新密码不一致");
                        } else {
                            match crate::service::change_password(&self.state, &old, &next) {
                                Ok(()) => {
                                    self.settings_pw = Default::default();
                                    self.notify("密码已更换（只重新封装密钥，文件没动）");
                                }
                                Err(e) => self.notify(format!("更换失败：{e}")),
                            }
                        }
                    }

                    ui.add_space(10.0);
                    section(ui, "关于");

                    ui.label(egui::RichText::new("小舟沙盒 v0.1.0").size(13.0).color(theme::TEXT));
                    ui.label(
                        egui::RichText::new("本地加密保险库：文件内容、文件名、目录结构、设置全部加密后存本地磁盘，无服务器、无账号、不联网。Argon2id 派生密钥，XChaCha20-Poly1305 加密，逐 1MB 分块防篡改。")
                            .size(11.5)
                            .color(theme::MUTED),
                    );
                    ui.label(
                        egui::RichText::new(format!(
                            "授权状态：{} · 开源协议 MIT",
                            crate::license::GATING_ENABLED.then(|| "已启用门控").unwrap_or("全功能免费")
                        ))
                        .size(11.5)
                        .color(theme::MUTED),
                    );
                });

                // 底部操作条已在 show_inside 的 Panel::bottom 里（见 settings-footer）
            });
    }

    // ─────────────────────────── 全文搜索 ───────────────────────────

    fn show_search_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::right("search-panel")
            .exact_size(560.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(theme::PANEL).inner_margin(egui::Margin::same(14)))
            .show(ui, |ui| {
                panel_header(ui, "全文搜索", "", || self.close_panel());

                let building = self.state.search_building.load(std::sync::atomic::Ordering::SeqCst);

                // 输入框（打开时聚焦）
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut self.search.q)
                        .hint_text("搜索文件内容与文件名，支持中文（二元组切分）")
                        .desired_width(f32::INFINITY),
                );
                resp.request_focus();

                // 防抖 180ms 查询
                if resp.changed() {
                    self.search.typed_at = Some(Instant::now());
                }
                if self
                    .search
                    .typed_at
                    .is_some_and(|t| t.elapsed().as_millis() >= 180)
                {
                    self.search.typed_at = None;
                    let q = self.search.q.clone();
                    match crate::service::search_query(&self.state, &q, None) {
                        Ok(hits) => self.search.hits = hits,
                        Err(_) => self.search.hits.clear(),
                    }
                    ui.ctx().request_repaint();
                }

                ui.add_space(4.0);

                if !self.search.report.built {
                    ui.label(
                        egui::RichText::new("索引只存在内存里，锁库即清空；不解密就搜不到内容。")
                            .size(11.5)
                            .color(theme::FAINT),
                    );
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        let label = if building { "建立中…" } else { "建立索引" };
                        if widgets::primary_button(ui, label).clicked() && !building {
                            if let Err(e) = crate::service::search_build(&self.state, ui.ctx().clone()) {
                                self.notify(format!("建索引失败：{e}"));
                            }
                        }
                    });
                } else {
                    ui.horizontal(|ui| {
                        let mut line = format!("索引 {} 个文件", self.search.report.docs);
                        if self.search.report.truncated {
                            line.push_str("（已达上限）");
                        }
                        ui.label(egui::RichText::new(line).size(11.5).color(theme::FAINT));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            
                            if widgets::ghost_button(ui, if building { "建立中…" } else { "重建" }).clicked()
                                && !building
                            {
                                let _ = crate::service::search_build(&self.state, ui.ctx().clone());
                            }
                        });
                    });

                // 底部说明条先占位
                egui::Panel::bottom("search-footer")
                    .exact_size(30.0)
                    .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(0, 4)))
                    .show_inside(ui, |ui| {
                        ui.label(
                            egui::RichText::new("索引不落盘、不联网。只索引 1 MB 以内的文本类文件，其余按文件名匹配。")
                                .size(11.0)
                                .color(theme::FAINT),
                        );
                    });

                    ui.add_space(6.0);
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        if self.search.q.trim().is_empty() {
                            return;
                        }
                        if self.search.hits.is_empty() {
                            ui.add_space(12.0);
                            ui.vertical_centered(|ui| {
                                ui.label(egui::RichText::new("没有命中").size(12.5).color(theme::FAINT));
                            });
                            return;
                        }
                        let q = self.search.q.to_lowercase();
                        for hit in self.search.hits.clone() {
                            let hit_ui = egui::Frame::new()
                                .fill(theme::PANEL)
                                .inner_margin(egui::Margin::symmetric(6, 6))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        widgets::icon(
                                            ui.painter(),
                                            ui.cursor().left_center() + egui::Vec2::new(8.0, 0.0),
                                            Icon::File,
                                            theme::FAINT,
                                            14.0,
                                        );
                                        ui.add_space(18.0);
                                        ui.vertical(|ui| {
                                            ui.label(
                                                egui::RichText::new(hit.name.as_str())
                                                    .size(13.0)
                                                    .color(theme::TEXT),
                                            );
                                            ui.add(rich_snippet(&hit.snippet, &q));
                                        });
                                    });
                                    let _ = hit_ui_placeholder();
                                })
                                .response;
                            if hit_ui.clicked() {
                                let id = hit.id.clone();
                                self.open_by_id(&id);
                            }
                            ui.add_space(2.0);
                        }
                    });
                }

            });
    }
}


/// 把命中片段里的查询词高亮。
fn rich_snippet(snippet: &str, q: &str) -> egui::Label {
    use egui::text::{LayoutJob, TextFormat};
    use egui::FontId;

    let mut job = LayoutJob::default();
    let normal = TextFormat::simple(FontId::proportional(12.0), theme::MUTED);
    let marked = TextFormat {
        background: egui::Color32::from_rgb(0xfd, 0xf0, 0xc8),
        ..TextFormat::simple(FontId::proportional(12.0), theme::TEXT)
    };

    let lower = snippet.to_lowercase();
    let mut pos = 0usize;
    while let Some(found) = lower[pos..].find(q) {
        let start = pos + found;
        let end = start + q.len();
        if !snippet.is_char_boundary(start) || !snippet.is_char_boundary(end) {
            break;
        }
        if start > pos {
            job.append(&snippet[pos..start], 0.0, normal.clone());
        }
        job.append(&snippet[start..end], 0.0, marked.clone());
        pos = end;
    }
    if pos < snippet.len() {
        job.append(&snippet[pos..], 0.0, normal);
    }
    egui::Label::new(job)
}

fn panel_header(ui: &mut egui::Ui, title: &str, sub: &str, close: impl FnOnce()) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(title).size(15.0).color(theme::TEXT).strong());
        if !sub.is_empty() {
            ui.label(egui::RichText::new(sub).size(11.5).color(theme::FAINT));
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if widgets::icon_button(ui, Icon::X, "关闭").clicked() {
                close();
            }
        });
    });
    ui.separator();
}

fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(2.0);
    ui.label(egui::RichText::new(title).size(12.5).color(theme::ACCENT).strong());
    ui.separator();
}

/// 设置行：右侧控件由调用方先画，这里补左侧标签与灰色说明。
fn setting_row_rest(ui: &mut egui::Ui, label: &str, desc: &str) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(egui::RichText::new(label).size(13.0).color(theme::TEXT));
            if !desc.is_empty() {
                ui.label(egui::RichText::new(desc).size(11.0).color(theme::FAINT));
            }
        });
    });
    ui.add_space(2.0);
}

fn hit_ui_placeholder() {}
