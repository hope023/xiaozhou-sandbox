//! 中列文件列表：面包屑/标题、筛选框、工具按钮、条目行（类型色块 + 徽标）。

use eframe::egui::{self, Align2, FontId, CornerRadius, Stroke, Vec2};

use super::theme;
use super::{App, Nav, Panel};
use super::widgets::{self, Icon};

const ROW_H: f32 = 46.0;

/// 拖拽时携带的东西：把某一项拖到文件夹 / 面包屑上就是移动。
pub struct DragItem {
    pub id: String,
    pub name: String,
}

impl App {
    pub fn show_file_list(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("filelist")
            .exact_size(348.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(theme::BG).inner_margin(egui::Margin::same(10)))
            .show(ui, |ui| {
                // ── 头部：面包屑或标题 ──
                ui.horizontal(|ui| {
                    if self.nav == Nav::Dir {
                        let all = ui.add(
                            egui::Button::new("全部文件")
                                .fill(egui::Color32::TRANSPARENT)
                                .stroke(Stroke::NONE),
                        );
                        if all.clicked() {
                            self.goto_trail(0);
                        }
                        // 拖到「全部文件」= 移回根目录
                        if let Some(p) = all.dnd_release_payload::<DragItem>() {
                            self.move_entry_to(&p.id, &p.name, "root", "全部文件");
                        }
                        for (i, c) in self.trail.clone().iter().enumerate() {
                            ui.label(egui::RichText::new("›").size(12.0).color(theme::FAINT));
                            let last = i + 1 == self.trail.len();
                            let btn = egui::Button::new(egui::RichText::new(c.name.as_str()).color(
                                if last { theme::TEXT } else { theme::MUTED },
                            ))
                            .fill(egui::Color32::TRANSPARENT)
                            .stroke(Stroke::NONE);
                            let r = ui.add(btn);
                            if r.clicked() {
                                self.goto_crumb(i);
                            }
                            // 拖到面包屑 = 移到该层
                            if let Some(p) = r.dnd_release_payload::<DragItem>() {
                                let (tid, tname) = (c.id.clone(), c.name.clone());
                                self.move_entry_to(&p.id, &p.name, &tid, &tname);
                            }
                        }
                    } else {
                        ui.label(egui::RichText::new(self.nav.title()).size(15.0).color(theme::TEXT).strong());
                        ui.label(
                            egui::RichText::new(format!("{} 项", self.entries.len()))
                                .size(11.5)
                                .color(theme::FAINT),
                        );
                        // 回收站非空时给一个整批销毁的入口；图标按钮说不清「清空」这件事
                        if self.nav == Nav::Trash && !self.entries.is_empty() {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let btn = egui::Button::new(
                                    egui::RichText::new("清空回收站").size(12.0).color(theme::DANGER),
                                )
                                .fill(theme::PANEL)
                                .stroke(Stroke::new(1.0, theme::DANGER))
                                .corner_radius(CornerRadius::same(6));
                                if ui.add(btn).clicked() {
                                    let n = self.entries.len();
                                    self.confirm = Some(widgets::Confirm::purge_trash(n));
                                }
                            });
                        }
                    }
                });

                ui.add_space(2.0);

                // ── 工具行：筛选框 + 图标按钮 ──
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.filter)
                            .hint_text("筛选当前列表（Ctrl+K 搜全文）")
                            .desired_width((ui.available_width() - 154.0).max(120.0)),
                    );
                    if widgets::icon_button(ui, Icon::Search, "全文搜索（Ctrl+K）").clicked() {
                        self.panel = Some(Panel::Search);
                        self.search.focus_pending = true;
                        self.search.report = crate::service::search_status(&self.state);
                    }
                    if self.nav == Nav::Dir && widgets::icon_button(ui, Icon::FolderPlus, "新建文件夹").clicked() {
                        self.new_folder();
                    }
                    if self.nav == Nav::Dir && widgets::icon_button(ui, Icon::Edit, "批量重命名当前文件夹").clicked() {
                        self.open_batch_rename();
                    }
                    if self.nav == Nav::Dir && widgets::icon_button(ui, Icon::Upload, "导入文件（也可拖进来）").clicked() {
                        if let Some(paths) = rfd::FileDialog::new().pick_files() {
                            let v: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
                            self.import_from(v);
                        }
                    }
                });

                ui.add_space(4.0);

                // ── 列表 ──
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let filter = self.filter.to_lowercase();
                        let shown: Vec<usize> = self
                            .entries
                            .iter()
                            .enumerate()
                            .filter(|(_, e)| filter.is_empty() || e.name.to_lowercase().contains(&filter))
                            .map(|(i, _)| i)
                            .collect();

                        if shown.is_empty() {
                            widgets::empty_state(
                                ui,
                                Icon::Folder,
                                if self.nav == Nav::Trash { "回收站是空的" } else { "这里还没有东西" },
                                if self.nav == Nav::Trash {
                                    "删除的文件会先来到这里"
                                } else {
                                    "拖文件进来试试，或者点右上角的导入按钮"
                                },
                            );
                            return;
                        }

                        for idx in shown {
                            let e = self.entries[idx].clone();
                            self.file_row(ui, &e);
                        }
                    });
            });
    }

    fn file_row(&mut self, ui: &mut egui::Ui, e: &crate::service::EntryView) {
        let selected = self.selected.as_ref().is_some_and(|s| s.id == e.id);
        let width = ui.available_width();
        let (rect, resp) =
            ui.allocate_exact_size(Vec2::new(width, ROW_H), egui::Sense::click_and_drag());

        // 拖起这一行：payload 由 egui 托管，松手时落到哪个文件夹就移到哪
        resp.dnd_set_drag_payload(DragItem { id: e.id.clone(), name: e.name.clone() });

        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::ListItem, true, &e.name));

        if selected {
            ui.painter().rect_filled(rect, CornerRadius::same(8), theme::ACCENT_SOFT);
        } else if resp.hovered() {
            ui.painter().rect_filled(rect, CornerRadius::same(8), theme::HOVER);
        }

        // 类型色块 + 图标
        let (bg, fg) = theme::tint(if e.kind == "dir" { "dir" } else { &e.preview });
        let icon_rect = egui::Rect::from_center_size(
            rect.left_center() + Vec2::new(20.0, 0.0),
            Vec2::splat(28.0),
        );
        ui.painter().rect_filled(icon_rect, CornerRadius::same(7), bg);
        widgets::icon(
            ui.painter(),
            icon_rect.center(),
            match (e.kind.as_str(), e.preview.as_str()) {
                ("dir", _) => Icon::Folder,
                (_, "image") => Icon::Image,
                (_, "video") => Icon::Video,
                (_, "audio") => Icon::Audio,
                (_, "text") => Icon::Text,
                _ => Icon::File,
            },
            fg,
            16.0,
        );

        // 名称 + 元信息
        ui.painter().text(
            rect.left_center() + Vec2::new(44.0, -8.0),
            Align2::LEFT_CENTER,
            e.name.as_str(),
            FontId::proportional(13.0),
            theme::TEXT,
        );
        let meta = if e.kind == "dir" {
            "文件夹".to_string()
        } else {
            format!("{} · {}", widgets::fmt_size(e.size), widgets::fmt_time(e.modified_at))
        };
        ui.painter().text(
            rect.left_center() + Vec2::new(44.0, 10.0),
            Align2::LEFT_CENTER,
            meta,
            FontId::proportional(11.0),
            theme::FAINT,
        );

        // 版本徽标
        if e.kind == "file" && e.version_count > 1 {
            ui.painter().text(
                rect.right_center() - Vec2::new(10.0, 0.0),
                Align2::RIGHT_CENTER,
                format!("v{}", e.version_count),
                FontId::proportional(10.5),
                theme::FAINT,
            );
        }

        // 文件夹作为落点：拖动中高亮，松手就移动
        if e.kind == "dir" && self.nav == Nav::Dir {
            let dragging = resp
                .dnd_hover_payload::<DragItem>()
                .is_some_and(|p| p.id != e.id);
            if dragging {
                ui.painter().rect_stroke(
                    rect,
                    CornerRadius::same(8),
                    Stroke::new(1.5, theme::ACCENT),
                    egui::StrokeKind::Inside,
                );
            }
            if let Some(p) = resp.dnd_release_payload::<DragItem>() {
                if p.id != e.id {
                    let (tid, tname) = (e.id.clone(), e.name.clone());
                    self.move_entry_to(&p.id, &p.name, &tid, &tname);
                }
            }
        }

        if resp.clicked() {
            self.select(Some(e.clone()));
        }
        if resp.double_clicked() && e.kind == "dir" && self.nav == Nav::Dir {
            let (id, name) = (e.id.clone(), e.name.clone());
            self.open_dir(&id, &name);
        }
    }

    pub fn goto_crumb(&mut self, idx: usize) {
        if let Some(c) = self.trail.get(idx).cloned() {
            self.trail.truncate(idx + 1);
            self.parent = c.id;
            self.need_list = true;
            self.select(None);
        }
    }
}
