//! 批量重命名：对当前文件夹（或筛选命中的项）一次改一批名字。
//!
//! 两条改名策略，二选一：
//! - **规则模式**：查找替换（作用于全名，含扩展名）＋ 前缀 ＋ 后缀 ＋ 序号，
//!   后缀和序号都插在扩展名之前；
//! - **模板模式**：整个名字用模板生成，`{name}` 是原名去扩展名、`{n}` 是序号，
//!   扩展名自动保留，规则输入框一并停用。
//!
//! 每帧对每个条目算出「旧名 → 新名」的预览：重名、空名、带路径分隔符的行标红，
//! 有红行或没有可改项时「应用」不可点。预演与 `service::batch_rename` 的校验一一对应
//! （整批统一先让出旧名、再按顺序占新名），预览全绿时应用一定成功。

use std::collections::HashSet;

use eframe::egui::{self, Align, Align2, Color32, CornerRadius, Label, Layout, Stroke, TextEdit, Vec2};

use super::theme;
use super::widgets::{self, Icon};
use super::{App, Nav};

/// 打开弹窗时的一份条目快照（id + 名字），改名过程不受列表刷新影响。
pub struct BatchItem {
    pub id: String,
    pub name: String,
}

pub struct BatchRenameState {
    pub parent: String,
    /// 打开时的筛选词快照。非空时提供「只改筛选命中的项」开关。
    pub filter_snapshot: String,
    pub only_filtered: bool,
    pub items: Vec<BatchItem>,

    // 规则模式
    pub find: String,
    pub replace: String,
    pub prefix: String,
    pub suffix: String,

    // 模板模式
    pub template_on: bool,
    pub template: String,

    // 序号（规则模式插在扩展名前；模板模式由 {n} 承担）
    pub numbering: bool,
    pub num_start: i64,
    pub num_digits: usize,
    pub num_sep: String,
}

impl BatchRenameState {
    pub fn open(parent: String, filter: &str, entries: &[crate::service::EntryView]) -> Self {
        Self {
            parent,
            filter_snapshot: filter.to_string(),
            only_filtered: false,
            items: entries
                .iter()
                .map(|e| BatchItem { id: e.id.clone(), name: e.name.clone() })
                .collect(),
            find: String::new(),
            replace: String::new(),
            prefix: String::new(),
            suffix: String::new(),
            template_on: false,
            template: "{name}-{n}".into(),
            numbering: false,
            num_start: 1,
            num_digits: 2,
            num_sep: "_".into(),
        }
    }

    /// 这一项参与这批改名吗：没开筛选就全部参与；开了就按快照词匹配。
    fn included(&self, it: &BatchItem) -> bool {
        self.filter_snapshot.is_empty()
            || !self.only_filtered
            || it.name.to_lowercase().contains(&self.filter_snapshot.to_lowercase())
    }

    /// 把对话框状态收敛成一套纯数据规则，供预演使用。
    fn rules(&self) -> Rules {
        Rules {
            find: self.find.clone(),
            replace: self.replace.clone(),
            prefix: self.prefix.clone(),
            suffix: self.suffix.clone(),
            template: self.template_on.then(|| self.template.clone()),
            numbering: self.numbering,
            num_start: self.num_start,
            num_digits: self.num_digits,
            num_sep: self.num_sep.clone(),
        }
    }

    /// 预演一次：逐个算新名，再做同文件夹查重。
    /// 返回 (预览行, 可应用的改名数, 有问题的行数)。
    ///
    /// 与 `service::batch_rename` 同一套占用规则：未入选条目的名字照常占位；
    /// 要改名的行**统一先让出旧名**，再按顺序占新名（A↔B 互换因此不标红）；
    /// 同批不变的条目名字始终占着，改成的名字撞上它们会被标红。
    pub fn plan(&self) -> Plan {
        let rules = self.rules();
        let mut taken: HashSet<(String, String)> = HashSet::new();
        // 所有条目先占住自己的旧名：未入选的名字照常占位；入选的等第二遍统一让出
        for it in &self.items {
            taken.insert((self.parent.clone(), it.name.clone()));
        }

        // 第一遍：算出每行的新名（序号按参与顺序连续），只收真正要改的
        struct Draft {
            idx: usize,
            old: String,
            new: String,
            changed: bool,
            problem: Option<String>,
        }
        let mut drafts: Vec<Draft> = Vec::new();
        let mut seq = rules.num_start;
        for (idx, it) in self.items.iter().enumerate() {
            if !self.included(it) {
                continue;
            }
            let cur_seq = seq;
            seq += 1;
            match plan_name(&rules, &it.name, cur_seq) {
                Ok(new) => {
                    let changed = new != it.name;
                    drafts.push(Draft { idx, old: it.name.clone(), new, changed, problem: None });
                }
                Err(problem) => {
                    drafts.push(Draft {
                        idx,
                        old: it.name.clone(),
                        new: String::new(),
                        changed: false,
                        problem: Some(problem),
                    });
                }
            }
        }
        // 第二遍：统一让出旧名，再按顺序占新名
        for d in &drafts {
            if d.changed {
                taken.remove(&(self.parent.clone(), d.old.clone()));
            }
        }
        let mut rows = Vec::with_capacity(drafts.len());
        let mut applicable = 0usize;
        let mut problems = 0usize;
        for d in drafts {
            let mut problem = d.problem;
            if problem.is_none() && d.changed {
                let key = (self.parent.clone(), d.new.clone());
                if taken.contains(&key) {
                    problem = Some("这个名字已被占用".into());
                } else {
                    taken.insert(key);
                }
            }
            if problem.is_some() {
                problems += 1;
            } else if d.changed {
                applicable += 1;
            }
            rows.push(PlannedRow { idx: d.idx, old: d.old, new: d.new, problem, changed: d.changed });
        }
        Plan { rows, applicable, problems }
    }
}

pub struct Plan {
    pub rows: Vec<PlannedRow>,
    /// 预览没有问题、且名字确实变了、可以应用的行数
    pub applicable: usize,
    pub problems: usize,
}

pub struct PlannedRow {
    /// 对应 `items` 的下标，应用时用它换回条目 id
    pub idx: usize,
    pub old: String,
    pub new: String,
    /// 该行为什么不能改。None = 可以改。
    pub problem: Option<String>,
    pub changed: bool,
}

/// 一套改名规则（从对话框状态收敛而来）。
#[derive(Clone)]
pub struct Rules {
    pub find: String,
    pub replace: String,
    pub prefix: String,
    pub suffix: String,
    /// Some = 模板模式：整个名字由模板生成
    pub template: Option<String>,
    pub numbering: bool,
    pub num_start: i64,
    pub num_digits: usize,
    pub num_sep: String,
}

/// 拆扩展名：最后一个「.」且不在开头才算（`.gitignore` 没有扩展名），
/// 与 `store::unique_child_name` 的约定一致。
fn split_ext(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    }
}

fn padded(n: i64, digits: usize) -> String {
    format!("{:0width$}", n, width = digits)
}

/// 按规则算一个条目的新名字。Err(原因) 表示这一行不合法，预览里标红。
pub fn plan_name(rules: &Rules, old: &str, seq: i64) -> Result<String, String> {
    let (stem, ext) = split_ext(old);
    let new = if let Some(t) = &rules.template {
        if !t.contains("{n}") {
            return Err("模板里缺少 {n}，所有条目会改成同一个名字".into());
        }
        let body = t.replace("{name}", stem).replace("{n}", &padded(seq, rules.num_digits));
        // 模板里已经写了相同扩展名就不重复追加
        if !ext.is_empty() && body.to_lowercase().ends_with(&ext.to_lowercase()) {
            body
        } else {
            format!("{}{}", body, ext)
        }
    } else {
        // 查找替换作用于全名（含扩展名），与常见批量改名工具一致
        let mut s = old.to_string();
        if !rules.find.is_empty() {
            s = s.replace(&rules.find, &rules.replace);
        }
        let (st, ex) = split_ext(&s);
        let mut core = format!("{}{}", rules.prefix, st);
        core.push_str(&rules.suffix);
        if rules.numbering {
            core.push_str(&rules.num_sep);
            core.push_str(&padded(seq, rules.num_digits));
        }
        format!("{}{}", core, ex)
    };
    let new = new.trim();
    if new.is_empty() {
        return Err("新名字是空的".into());
    }
    if new.contains('\\') || new.contains('/') {
        return Err("新名字不能包含 \\ 或 /".into());
    }
    Ok(new.to_string())
}

impl App {
    /// 从文件列表工具栏进入：拿当前文件夹的条目快照开弹窗。
    pub fn open_batch_rename(&mut self) {
        if !self.unlocked() || self.nav != Nav::Dir {
            return;
        }
        // 弹窗里全是系统输入法的输入框：隐私输入开着就先退出，
        // 否则 keep_ime_off 会把拼音摁成英文（与 begin_rename 同一处理）。
        if self.privacy_on {
            self.exit_privacy_mode();
        }
        if let Some(h) = self.hwnd {
            crate::service::ensure_ime_available(h);
        }
        self.ctx.send_viewport_cmd(egui::ViewportCommand::IMEAllowed(true));
        let st = BatchRenameState::open(self.parent.clone(), &self.filter, &self.entries);
        self.batch = Some(st);
    }

    /// 应用：把预演通过的行交给 service 一次写盘。
    fn apply_batch_rename(&mut self) {
        let Some(b) = self.batch.as_ref() else { return };
        let plan = b.plan();
        if plan.applicable == 0 || plan.problems > 0 {
            return;
        }
        let updates: Vec<(String, String)> = plan
            .rows
            .iter()
            .filter(|r| r.problem.is_none() && r.changed)
            .map(|r| (b.items[r.idx].id.clone(), r.new.clone()))
            .collect();
        match crate::service::batch_rename(&self.state, &updates) {
            Ok(rep) => {
                // 页签和选中项里的名字要跟着改，否则页签栏一直显示旧名
                for v in &rep.entries {
                    if let Some(s) = self.selected.as_mut() {
                        if s.id == v.id {
                            *s = v.clone();
                        }
                    }
                    for t in self.tabs.iter_mut() {
                        if t.entry.id == v.id {
                            t.entry.name = v.name.clone();
                            t.entry.modified_at = v.modified_at;
                        }
                    }
                }
                let mut msg = format!("已批量重命名 {} 项", rep.renamed);
                if rep.skipped > 0 {
                    msg.push_str(&format!("（{} 项无变化）", rep.skipped));
                }
                self.notify(msg);
                self.need_list = true;
                self.batch = None;
            }
            Err(e) => self.notify(format!("批量重命名失败：{e}")),
        }
    }

    /// 居中弹窗：规则输入 + 逐行预览 + 应用/取消。每帧重算预览。
    pub fn show_batch_rename(&mut self, ctx: &egui::Context) {
        if self.batch.is_none() {
            return;
        }
        if !self.unlocked() {
            // 锁库等场景 on_vault_locked 已清理，这里兜个底
            self.batch = None;
            return;
        }

        let (included_count, included_matched) = {
            let b = self.batch.as_ref().unwrap();
            (
                b.items.len(),
                b.items.iter().filter(|it| b.included(it)).count(),
            )
        };
        let plan = self.batch.as_ref().unwrap().plan();

        let mut close = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let mut apply = false;

        // 遮罩（与确认弹窗一致）
        egui::Area::new(egui::Id::new("batch-rename-mask"))
            .order(egui::Order::Foreground)
            .fixed_pos(egui::Pos2::ZERO)
            .show(ctx, |ui| {
                let screen = ctx.input(|i| i.raw.screen_rect).unwrap_or(egui::Rect::EVERYTHING);
                ui.painter().rect_filled(screen, 0.0, Color32::from_rgba_unmultiplied(27, 27, 25, 56));
            });

        egui::Area::new(egui::Id::new("batch-rename-dialog"))
            .order(egui::Order::Foreground)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                let Some(b) = self.batch.as_mut() else { return };
                egui::Frame::new()
                    .fill(theme::PANEL)
                    .corner_radius(CornerRadius::same(12))
                    .stroke(Stroke::new(1.0, theme::LINE))
                    .inner_margin(egui::Margin::same(18))
                    .show(ui, |ui| {
                        ui.set_min_width(560.0);

                        // ── 标题行 ──
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("批量重命名")
                                    .text_style(egui::TextStyle::Name("H2".into()))
                                    .color(theme::TEXT),
                            );
                            ui.label(
                                egui::RichText::new(if included_count == included_matched {
                                    format!("当前文件夹 {included_count} 项")
                                } else {
                                    format!("当前文件夹 {included_count} 项 · 筛选命中 {included_matched} 项")
                                })
                                .size(11.5)
                                .color(theme::FAINT),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if widgets::icon_button(ui, Icon::X, "关闭（Esc）").clicked() {
                                    close = true;
                                }
                            });
                        });

                        // 筛选快照非空才给「只改命中项」的开关
                        if !b.filter_snapshot.is_empty() {
                            ui.checkbox(
                                &mut b.only_filtered,
                                format!("只改筛选命中的项（「{}」）", b.filter_snapshot),
                            );
                        }

                        ui.add_space(8.0);

                        // ── 规则 / 模板 ──
                        ui.checkbox(&mut b.template_on, "用模板生成整个名字");
                        if b.template_on {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("模板").size(12.5).color(theme::MUTED));
                                ui.add(TextEdit::singleline(&mut b.template).desired_width(240.0));
                            });
                            ui.label(
                                egui::RichText::new(
                                    "{name}＝原名（不含扩展名），{n}＝序号；扩展名自动保留，例如 {name}-{n}",
                                )
                                .size(11.5)
                                .color(theme::FAINT),
                            );
                            if !b.template.contains("{n}") {
                                ui.label(
                                    egui::RichText::new("模板里要有 {n}，否则所有条目会改成同一个名字")
                                        .size(11.5)
                                        .color(theme::DANGER),
                                );
                            }
                        } else {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("查找").size(12.5).color(theme::MUTED));
                                ui.add(
                                    TextEdit::singleline(&mut b.find)
                                        .desired_width(130.0)
                                        .hint_text("要替换的内容"),
                                );
                                ui.label(egui::RichText::new("替换为").size(12.5).color(theme::MUTED));
                                ui.add(
                                    TextEdit::singleline(&mut b.replace)
                                        .desired_width(130.0)
                                        .hint_text("留空＝删掉这段"),
                                );
                            });
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("前缀").size(12.5).color(theme::MUTED));
                                ui.add(TextEdit::singleline(&mut b.prefix).desired_width(120.0));
                                ui.label(egui::RichText::new("后缀").size(12.5).color(theme::MUTED));
                                ui.add(
                                    TextEdit::singleline(&mut b.suffix)
                                        .desired_width(120.0)
                                        .hint_text("加在扩展名前"),
                                );
                            });
                        }
                        ui.horizontal(|ui| {
                            if b.template_on {
                                ui.label(
                                    egui::RichText::new("模板模式下序号由 {n} 提供")
                                        .size(11.5)
                                        .color(theme::FAINT),
                                );
                            } else {
                                ui.add_enabled(
                                    true,
                                    egui::Checkbox::new(&mut b.numbering, "加序号"),
                                );
                                if b.numbering {
                                    ui.label(egui::RichText::new("起始").size(12.5).color(theme::MUTED));
                                    ui.add(egui::DragValue::new(&mut b.num_start).range(0..=999_999));
                                    ui.label(egui::RichText::new("位数").size(12.5).color(theme::MUTED));
                                    ui.add(egui::DragValue::new(&mut b.num_digits).range(1..=6));
                                    ui.label(egui::RichText::new("分隔符").size(12.5).color(theme::MUTED));
                                    ui.add(TextEdit::singleline(&mut b.num_sep).desired_width(36.0));
                                } else {
                                    ui.label(
                                        egui::RichText::new("查找替换作用于全名；后缀和序号都插在扩展名之前")
                                            .size(11.5)
                                            .color(theme::FAINT),
                                    );
                                }
                            }
                        });

                        // ── 预览 ──
                        ui.add_space(10.0);
                        ui.label(egui::RichText::new("预览").size(11.5).color(theme::FAINT));
                        egui::ScrollArea::vertical()
                            .max_height(240.0)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                if plan.rows.is_empty() {
                                    ui.label(
                                        egui::RichText::new("没有命中的项")
                                            .size(12.5)
                                            .color(theme::FAINT),
                                    );
                                }
                                for r in &plan.rows {
                                    ui.horizontal(|ui| {
                                        match &r.problem {
                                            Some(p) => {
                                                ui.add_sized(
                                                    Vec2::new(200.0, 18.0),
                                                    Label::new(
                                                        egui::RichText::new(&r.old).size(12.5).color(theme::FAINT),
                                                    )
                                                    .truncate(),
                                                );
                                                ui.label(egui::RichText::new("→").size(12.0).color(theme::FAINT));
                                                ui.add(
                                                    Label::new(
                                                        egui::RichText::new(p).size(12.5).color(theme::DANGER),
                                                    )
                                                    .truncate(),
                                                );
                                            }
                                            None if !r.changed => {
                                                ui.add_sized(
                                                    Vec2::new(200.0, 18.0),
                                                    Label::new(
                                                        egui::RichText::new(&r.old).size(12.5).color(theme::MUTED),
                                                    )
                                                    .truncate(),
                                                );
                                                ui.label(egui::RichText::new("（不变）").size(11.5).color(theme::FAINT));
                                            }
                                            None => {
                                                ui.add_sized(
                                                    Vec2::new(200.0, 18.0),
                                                    Label::new(
                                                        egui::RichText::new(&r.old).size(12.5).color(theme::MUTED),
                                                    )
                                                    .truncate(),
                                                );
                                                ui.label(egui::RichText::new("→").size(12.0).color(theme::FAINT));
                                                ui.add(
                                                    Label::new(
                                                        egui::RichText::new(&r.new).size(12.5).color(theme::ACCENT),
                                                    )
                                                    .truncate(),
                                                );
                                            }
                                        }
                                    });
                                }
                            });

                        // ── 汇总 + 按钮 ──
                        ui.add_space(8.0);
                        if plan.problems > 0 {
                            ui.label(
                                egui::RichText::new(format!(
                                    "有 {} 行标红不能改，请调整规则后重试",
                                    plan.problems
                                ))
                                .size(12.0)
                                .color(theme::DANGER),
                            );
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if widgets::ghost_button(ui, "取消").clicked() {
                                close = true;
                            }
                            let enabled = plan.applicable > 0 && plan.problems == 0;
                            let btn = egui::Button::new(
                                egui::RichText::new(format!("应用（改名 {} 项）", plan.applicable))
                                    .color(theme::WHITE),
                            )
                            .fill(theme::ACCENT)
                            .corner_radius(CornerRadius::same(8))
                            .min_size(Vec2::new(0.0, 34.0));
                            if ui.add_enabled(enabled, btn).clicked() {
                                apply = true;
                            }
                        });
                    });
            });

        if close {
            self.batch = None;
        }
        if apply {
            self.apply_batch_rename();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_rules() -> Rules {
        Rules {
            find: String::new(),
            replace: String::new(),
            prefix: String::new(),
            suffix: String::new(),
            template: None,
            numbering: false,
            num_start: 1,
            num_digits: 2,
            num_sep: "_".into(),
        }
    }

    fn st(items: &[&str]) -> BatchRenameState {
        BatchRenameState {
            parent: "root".into(),
            filter_snapshot: String::new(),
            only_filtered: false,
            items: items
                .iter()
                .map(|n| BatchItem { id: (*n).to_string(), name: (*n).to_string() })
                .collect(),
            find: String::new(),
            replace: String::new(),
            prefix: String::new(),
            suffix: String::new(),
            template_on: false,
            template: String::new(),
            numbering: false,
            num_start: 1,
            num_digits: 2,
            num_sep: "_".into(),
        }
    }

    #[test]
    fn find_replace_and_suffix_goes_before_ext() {
        let mut r = base_rules();
        r.find = "IMG".into();
        r.replace = "照片".into();
        r.suffix = "-备注".into();
        assert_eq!(plan_name(&r, "IMG_0001.jpg", 1).unwrap(), "照片_0001-备注.jpg");
        // 查找替换也作用于扩展名（与常见工具一致）
        let mut r2 = base_rules();
        r2.find = "txt".into();
        r2.replace = "md".into();
        assert_eq!(plan_name(&r2, "a.txt", 1).unwrap(), "a.md");
    }

    #[test]
    fn affixes_on_extensionless_names_and_hidden_files() {
        let mut r = base_rules();
        r.prefix = "2024-".into();
        r.suffix = "-月份".into();
        // 文件夹没有扩展名：前后缀直接拼接
        assert_eq!(plan_name(&r, "归档", 1).unwrap(), "2024-归档-月份");
        // 点在开头不是扩展名
        let mut r2 = base_rules();
        r2.numbering = true;
        assert_eq!(plan_name(&r2, ".gitignore", 1).unwrap(), ".gitignore_01");
    }

    #[test]
    fn numbering_pads_and_advances() {
        let mut r = base_rules();
        r.numbering = true;
        r.num_start = 7;
        r.num_digits = 3;
        r.num_sep = "-".into();
        assert_eq!(plan_name(&r, "a.jpg", 7).unwrap(), "a-007.jpg");
        assert_eq!(plan_name(&r, "归档", 8).unwrap(), "归档-008");
        // 序号超过位数就自然变长，不截断
        assert_eq!(plan_name(&r, "a.jpg", 1234).unwrap(), "a-1234.jpg");
    }

    #[test]
    fn template_mode_replaces_whole_name_keeps_ext() {
        let mut r = base_rules();
        r.template = Some("{name}-{n}".into());
        assert_eq!(plan_name(&r, "IMG_0001.jpg", 1).unwrap(), "IMG_0001-01.jpg");
        // 不含 {name}＝整批按序号重命名
        r.template = Some("旅行-{n}".into());
        r.num_digits = 3;
        assert_eq!(plan_name(&r, "a.jpg", 2).unwrap(), "旅行-002.jpg");
        // 模板里自带相同扩展名时不重复追加
        r.template = Some("{name}-{n}.jpg".into());
        r.num_digits = 2;
        assert_eq!(plan_name(&r, "a.jpg", 1).unwrap(), "a-01.jpg");
    }

    #[test]
    fn template_without_n_and_illegal_names_are_rejected() {
        let mut r = base_rules();
        r.template = Some("固定名字".into());
        assert!(plan_name(&r, "a.jpg", 1).is_err(), "模板缺 {{n}} 必须报错");
        // 替换结果为空
        let mut r2 = base_rules();
        r2.find = "a".into();
        r2.replace = "".into();
        assert!(plan_name(&r2, "a", 1).is_err());
        // 名字里带路径分隔符
        let mut r3 = base_rules();
        r3.find = "a".into();
        r3.replace = "b/c".into();
        assert!(plan_name(&r3, "a.txt", 1).is_err());
    }

    #[test]
    fn plan_flags_collision_with_unchanged_sibling() {
        // b.txt 要改成 a.txt，而 a.txt 在同批里不变（名字被占着）→ 标红
        let mut s = st(&["a.txt", "b.txt"]);
        s.find = "b".into();
        s.replace = "a".into();
        let plan = s.plan();
        assert_eq!(plan.rows.len(), 2);
        assert!(plan.rows[0].problem.is_none() && !plan.rows[0].changed);
        assert!(plan.rows[1].problem.is_some());
        assert_eq!(plan.applicable, 0);
        assert_eq!(plan.problems, 1);
    }

    #[test]
    fn plan_respects_filter_and_numbering_sequence() {
        let mut s = st(&["a.jpg", "b.png", "c.jpg"]);
        s.filter_snapshot = "jpg".into();
        s.only_filtered = true;
        s.numbering = true;
        s.num_digits = 2;
        let plan = s.plan();
        // 只有筛选命中的两行参与，序号按参与顺序连续（插在扩展名前）
        assert_eq!(plan.rows.len(), 2);
        assert_eq!(plan.rows[0].new, "a_01.jpg");
        assert_eq!(plan.rows[1].new, "c_02.jpg");
        assert_eq!(plan.applicable, 2);
        assert_eq!(plan.problems, 0);
    }

    #[test]
    fn plan_no_change_means_nothing_applicable() {
        let s = st(&["a.txt", "b.txt"]);
        let plan = s.plan();
        assert_eq!(plan.applicable, 0);
        assert_eq!(plan.problems, 0);
        assert!(plan.rows.iter().all(|r| !r.changed));
    }
}
