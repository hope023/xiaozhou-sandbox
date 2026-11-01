//! 文本编辑器：等宽 TextEdit + 800ms 防抖自动保存 + 4MB 截断保护 + 隐私输入切换。

use std::time::Instant;

use eframe::egui::{self, Align, Layout};

use super::theme;
use super::widgets::{self, Icon};
use super::{App, EditorState};

impl App {
    /// 在检查器主体中渲染。调用方保证 selected 是文本类文件。
    pub fn show_text_editor(&mut self, ui: &mut egui::Ui) {
        // 加载内容（select() 切换文件时已把旧编辑器落盘并清空）
        let Some(sel) = self.selected.clone() else { return };
        if self.editor.as_ref().map_or(true, |ed| ed.entry_id != sel.id) {
            match crate::service::read_text(&self.state, &sel.id) {
                Ok(p) => {
                    self.editor = Some(EditorState {
                        entry_id: sel.id.clone(),
                        text: p.content,
                        dirty: false,
                        last_edit: None,
                        truncated: p.truncated,
                        phase: "自动保存已开启",
                    });
                }
                Err(e) => {
                    self.notify(format!("读取失败：{e}"));
                    return;
                }
            }
        }

        let truncated = self.editor.as_ref().is_some_and(|ed| ed.truncated);
        if truncated {
            widgets::warn_box(
                ui,
                "文件超过 4 MB，为防止数据丢失只加载了前 4 MB。已切换为只读，可「导出」后编辑。",
            );
            ui.add_space(4.0);
        }

        if self.privacy_on {
            // 隐私模式：打字区 + 候选条 + 状态条，全部由 privacy 模块自己排
            if super::privacy::show(self, ui) {
                self.exit_privacy_mode();
            }
        } else {
            // 普通模式：底部状态条先占位，滚动区拿剩余空间
            egui::Panel::bottom("editor-footer")
                .exact_size(40.0)
                .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(0, 4)))
                .show_inside(ui, |ui| {
                    ui.horizontal_centered(|ui| {
                        if widgets::ghost_button(ui, "隐私输入").clicked() {
                            self.set_privacy_mode(true);
                        }
                        ui.add_space(2.0);
                        // 图标按固定尺寸占位，不再用「光标 + 偏移」画（会和文字挤在一起）
                        let (r, _) =
                            ui.allocate_exact_size(egui::Vec2::splat(12.0), egui::Sense::hover());
                        widgets::icon(ui.painter(), r.center(), Icon::Shield, theme::FAINT, 12.0);
                        ui.add_space(4.0);
                        let (phase, chars) = self
                            .editor
                            .as_ref()
                            .map(|ed| (ed.phase, ed.text.chars().count()))
                            .unwrap_or(("", 0));
                        let phase_text = if truncated { "只读 · 已阻止覆盖原文件" } else { phase };
                        ui.label(egui::RichText::new(phase_text).size(11.5).color(theme::FAINT));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new(format!("{chars} 字符"))
                                    .size(11.5)
                                    .color(theme::FAINT),
                            );
                        });
                    });
                });

            egui::ScrollArea::vertical()
                .id_salt("editor-scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let Some(ed) = self.editor.as_mut() else { return };
                    let te = egui::TextEdit::multiline(&mut ed.text)
                        .code_editor()
                        .desired_width(ui.available_width());
                    let resp = ui.add_enabled(!truncated, te);
                    if resp.changed() {
                        let ed = self.editor.as_mut().unwrap();
                        ed.dirty = true;
                        ed.last_edit = Some(Instant::now());
                        ed.phase = "有改动，稍后自动保存";
                    }
                });
        }
    }
}
