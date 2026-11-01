//! 隐私输入区（自前端 `ime/PrivacyInput.tsx` 移植）。
//!
//! **为什么不用 egui 的 `TextEdit`：** 只要焦点落在可编辑控件上，
//! Windows 的 TSF 就会激活系统输入法，你打的每个字都会进系统词库、
//! 云候选、用户学习记录——正是我们要避免的。这个组件直接消费 egui 的
//! 原始键盘事件自己拼字，系统输入法不会介入；Rust 侧另有 IME 解绑做双保险。
//!
//! 能力边界（会明确写在界面上，不假装全能）：
//!   - 只支持**在末尾追加/退格**，不支持中间插字与选区
//!   - 支持拼音单字、词组、中英标点、换行
//!   - 不支持简拼、手写、语音
//!   - 防不住键盘钩子和屏幕录制（那是 OS 层，见 SECURITY.md）

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use eframe::egui::{self, Align, CornerRadius, Layout, Stroke, StrokeKind, Vec2};

use super::ime_engine;
use super::theme;
use super::widgets::{self, Icon};
use super::App;

pub struct PrivacyState {
    pub buf: String,
    pub sel: usize,
    pub cands: Vec<ime_engine::Candidate>,
    pub need_focus: bool,
    /// 本帧检测到系统输入法仍在组合/提交输入（压制无效的证据，界面明示）
    pub system_ime_active: bool,
}

impl Default for PrivacyState {
    fn default() -> Self {
        Self {
            buf: String::new(),
            sel: 0,
            cands: Vec::new(),
            need_focus: true,
            system_ime_active: false,
        }
    }
}

const STAGE_ID: &str = "privacy-stage";

/// 渲染隐私输入区并处理按键。返回 true = 用户要求退出隐私输入。
///
/// 版面自上而下：打字区（占满剩余空间）→ 警示行 → 候选条 → 状态条。
/// 底部三行都用 `Panel::bottom` 先占位，打字区拿剩下的高度——
/// 手工算「可用高度 - 固定值」在窗口变高变矮时必然算错，候选项会被挤到面板外看不见。
pub fn show(app: &mut App, ui: &mut egui::Ui) -> bool {
    if app.editor.is_none() {
        return false;
    }
    let mut exit = false;
    app.privacy.system_ime_active = false; // 每帧重置，由本帧事件置位
    let dict_ok = ime_engine::engine().is_ok();
    let stage_id = egui::Id::new(STAGE_ID);
    let (truncated, chars, phase) = app
        .editor
        .as_ref()
        .map(|ed| (ed.truncated, ed.text.chars().count(), ed.phase))
        .unwrap_or((false, 0, ""));

    // ── 1) 状态条（最底）：隐私徽章 + 切回普通输入 + 阶段文案 + 字数 + 退出 ──
    egui::Panel::bottom("privacy-bar")
        .exact_size(40.0)
        .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(0, 4)))
        .show_inside(ui, |ui| {
            ui.horizontal_centered(|ui| {
                badge(ui, dict_ok);
                ui.add_space(2.0);
                if widgets::ghost_button(ui, "切回普通输入").clicked() {
                    exit = true;
                }
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new(if truncated { "只读 · 已阻止覆盖原文件" } else { phase })
                        .size(11.5)
                        .color(theme::FAINT),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!("{chars} 字"))
                            .size(11.5)
                            .color(theme::FAINT),
                    );
                });
            });
        });

    // ── 2) 候选条（紧贴打字区下方，只有组合中才出现）──
    if !app.privacy.cands.is_empty() {
        egui::Panel::bottom("privacy-cands")
            .exact_size(38.0)
            .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(0, 4)))
            .show_inside(ui, |ui| {
                egui::ScrollArea::horizontal()
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            for i in 0..app.privacy.cands.len().min(9) {
                                let on = i == app.privacy.sel;
                                let text = format!("{} {}", i + 1, app.privacy.cands[i].text);
                                let btn = egui::Button::new(
                                    egui::RichText::new(text)
                                        .size(13.0)
                                        .color(if on { theme::ACCENT } else { theme::TEXT }),
                                )
                                .fill(if on { theme::ACCENT_SOFT } else { theme::PANEL })
                                .stroke(Stroke::new(
                                    1.0,
                                    if on { theme::ACCENT_LINE } else { theme::LINE },
                                ))
                                .corner_radius(CornerRadius::same(8))
                                .min_size(Vec2::new(0.0, 28.0));
                                if ui.add(btn).clicked() {
                                    pick(app, i);
                                }
                            }
                        });
                    });
            });
    }

    // ── 3) 警示行：系统输入法没被压住时把后果说清楚 ──
    if app.privacy.system_ime_active {
        egui::Panel::bottom("privacy-warn")
            .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(0, 2)))
            .show_inside(ui, |ui| {
                ui.label(
                    egui::RichText::new(
                        "系统输入法仍在接管按键：请按 Shift 切到英文再打拼音，或「切回普通输入」。",
                    )
                    .size(11.0)
                    .color(theme::WARN),
                );
            });
    } else if app.ime_detach_ok == Some(false) {
        egui::Panel::bottom("privacy-warn")
            .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(0, 2)))
            .show_inside(ui, |ui| {
                ui.label(
                    egui::RichText::new("系统输入法未能压制：若打不出字，请把系统输入法切到英文模式。")
                        .size(11.0)
                        .color(theme::WARN),
                );
            });
    }

    // ── 4) 打字区：吃掉剩余高度 ──
    let avail = ui.available_size();
    let (rect, _) = ui.allocate_exact_size(avail, egui::Sense::hover());
    let resp = ui.interact(rect, stage_id, egui::Sense::click());
    resp.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::Role::TextInput,
            true,
            app.editor.as_ref().map(|e| e.text.as_str()).unwrap_or(""),
        )
    });

    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(10), theme::PANEL);
    painter.rect_stroke(
        rect,
        CornerRadius::same(10),
        Stroke::new(1.0, theme::LINE),
        StrokeKind::Inside,
    );

    let inner = rect.shrink2(Vec2::new(12.0, 10.0));
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(Layout::top_down(Align::Min)),
    );
    let text_empty = app.editor.as_ref().is_none_or(|e| e.text.is_empty());
    if text_empty && app.privacy.buf.is_empty() {
        child.label(
            egui::RichText::new(if dict_ok {
                "在这里打字。中文不会经过系统输入法。"
            } else {
                "词库加载失败，只能粘贴和打英文标点。"
            })
            .size(14.0)
            .color(theme::FAINT),
        );
    } else {
        egui::ScrollArea::vertical()
            .id_salt("privacy-scroll")
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(&mut child, |ui| {
                if let Some(ed) = app.editor.as_ref() {
                    ui.label(
                        egui::RichText::new(ed.text.as_str())
                            .size(14.0)
                            .color(theme::TEXT),
                    );
                }
                if !app.privacy.buf.is_empty() {
                    ui.label(
                        egui::RichText::new(app.privacy.buf.as_str())
                            .size(14.0)
                            .color(theme::MUTED)
                            .underline(),
                    );
                }
                let blink = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .subsec_millis()
                    < 500;
                if blink {
                    ui.painter().rect_filled(
                        egui::Rect::from_min_size(ui.cursor().left_top(), Vec2::new(2.0, 18.0)),
                        0.0,
                        theme::ACCENT,
                    );
                }
            });
    }

    // ── 5) 焦点：自己养回来 ──
    // egui 会在按下 Esc 时清空焦点，Tab / 方向键还会把焦点挪给相邻控件。
    // 这几颗键恰好都是拼音输入要用的（Esc 取消、Tab 翻候选），
    // 旧代码只在一进隐私模式时 request_focus 一次，焦点一丢就再也不回来——
    // 表现就是「打字完全没反应」。这里每帧自愈：只要当前没有别的控件持焦，
    // 就把焦点抢回打字区；用户点开侧边搜索框时不会被打断。
    let cur_focus = ui.ctx().memory(|m| m.focused());
    if cur_focus != Some(stage_id) && (app.privacy.need_focus || cur_focus.is_none()) {
        resp.request_focus();
    }
    let focused = ui.ctx().memory(|m| m.has_focus(stage_id));
    if focused {
        // 光标闪烁 + 持续接收按键
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
        app.privacy.need_focus = false;
    }

    // ── 6) 键盘事件（仅当打字区持焦）──
    if focused {
        let events: Vec<egui::Event> = ui.ctx().input(|i| i.events.clone());

        // 预扫描：本帧 Text 事件带来的字符。真实键盘同一个按键通常同时产生
        // Key 与 Text 两种事件，按帧去重，哪条投递路径先到都只算一次；
        // 反过来，只发 Key 不发 Text 的环境（输入法直通等）由 Key 分支兜底。
        let mut text_chars: std::collections::HashSet<char> = std::collections::HashSet::new();
        for ev in &events {
            if let egui::Event::Text(t) = ev {
                for ch in t.chars() {
                    text_chars.insert(ch);
                }
            }
        }

        for ev in events {
            match ev {
                egui::Event::Ime(ime) => {
                    // 系统输入法还在组合/提交输入 = 压制无效，明示给用户
                    match &ime {
                        egui::ImeEvent::Disabled => {
                            app.privacy.system_ime_active = false;
                        }
                        other => {
                            app.privacy.system_ime_active = true;
                            // 系统输入法提交的内容已经过了它的词库，隐私承诺在这里
                            // 已经破了——不收进文本（保持与原版一致），只提示用户
                            let _ = other;
                        }
                    }
                    // 只记事件类型。ImeEvent::Preedit / Commit 里带的是用户正在打的原文，
                    // 写进 debug.log 就等于把明文落到磁盘上——这里绝不能打印内容。
                    let kind = match &ime {
                        egui::ImeEvent::Preedit { .. } => "preedit",
                        egui::ImeEvent::Commit(_) => "commit",
                        egui::ImeEvent::Disabled => "disabled",
                        _ => "other",
                    };
                    crate::app::debug_log(&format!("[privacy] system ime: {kind}"));
                }
                egui::Event::Key { key, pressed: true, modifiers, .. } => {
                    if modifiers.any() {
                        continue; // 复制粘贴、全选、撤销等组合键放行
                    }
                    match key {
                        egui::Key::Backspace => backspace(app),
                        egui::Key::Escape => {
                            if !app.privacy.buf.is_empty() {
                                app.privacy.buf.clear();
                                recalc(app);
                            } else {
                                exit = true;
                            }
                        }
                        egui::Key::Enter => {
                            if !app.privacy.cands.is_empty() {
                                let sel = app.privacy.sel;
                                pick(app, sel);
                            } else {
                                commit(app, "\n");
                            }
                        }
                        egui::Key::ArrowDown | egui::Key::Tab => cycle_sel(app, 1),
                        egui::Key::ArrowUp => cycle_sel(app, -1),
                        // 字母兜底：只发 Key 不发 Text 的环境下也能打拼音
                        k if matches!(
                            k,
                            egui::Key::A
                                | egui::Key::B
                                | egui::Key::C
                                | egui::Key::D
                                | egui::Key::E
                                | egui::Key::F
                                | egui::Key::G
                                | egui::Key::H
                                | egui::Key::I
                                | egui::Key::J
                                | egui::Key::K
                                | egui::Key::L
                                | egui::Key::M
                                | egui::Key::N
                                | egui::Key::O
                                | egui::Key::P
                                | egui::Key::Q
                                | egui::Key::R
                                | egui::Key::S
                                | egui::Key::T
                                | egui::Key::U
                                | egui::Key::V
                                | egui::Key::W
                                | egui::Key::X
                                | egui::Key::Y
                                | egui::Key::Z
                        ) => {
                            let ch = letter_of(k);
                            if text_chars.contains(&ch) {
                                continue; // 同帧已有 Text 事件，交给 Text 分支
                            }
                            if dict_ok && app.privacy.buf.chars().count() < 24 {
                                app.privacy.buf.push(ch);
                                recalc(app);
                            }
                        }
                        _ => {}
                    }
                }
                egui::Event::Text(t) => {
                    for ch in t.chars() {
                        if (ch as u32) < 0x20 {
                            continue; // 控制字符交给 Key 分支
                        }
                        match ch {
                            '1'..='9' if !app.privacy.cands.is_empty() => {
                                let i = ch as usize - '1' as usize;
                                if i < app.privacy.cands.len() {
                                    pick(app, i);
                                }
                            }
                            c if c.is_ascii_alphabetic() => {
                                if dict_ok && app.privacy.buf.chars().count() < 24 {
                                    app.privacy.buf.push(c.to_ascii_lowercase());
                                    recalc(app);
                                }
                            }
                            ' ' => {
                                if !app.privacy.buf.is_empty() {
                                    if !app.privacy.cands.is_empty() {
                                        let sel = app.privacy.sel;
                                        pick(app, sel);
                                    } else {
                                        // 有拼音但没候选：原样输出字母，等于快速切英文
                                        let b = app.privacy.buf.clone();
                                        commit(app, &b);
                                        app.privacy.buf.clear();
                                        app.privacy.cands.clear();
                                    }
                                } else {
                                    commit(app, " ");
                                }
                            }
                            c => {
                                // 中文标点映射
                                let mapped = ime_engine::PUNCT
                                    .iter()
                                    .find(|(k, _)| *k == c.to_string())
                                    .map(|(_, v)| v.to_string());
                                if let Some(zh) = mapped {
                                    if !app.privacy.buf.is_empty() && !app.privacy.cands.is_empty() {
                                        // 先上屏当前拼音，否则标点会把拼音弄丢
                                        let sel = app.privacy.sel;
                                        pick(app, sel);
                                    } else {
                                        commit(app, &zh);
                                    }
                                }
                                // 其余符号与原版一致：忽略（保持行为可预期）
                            }
                        }
                    }
                }
                egui::Event::Paste(s) => {
                    if !s.is_empty() {
                        commit(app, &s);
                    }
                }
                _ => {}
            }
        }
    }

    exit
}

/// 隐私徽章：图标按固定尺寸占位，不再用「光标 + 魔法偏移」画，
/// 那样在高 DPI 下会和文字、圆角边框叠在一起。
fn badge(ui: &mut egui::Ui, ok: bool) {
    egui::Frame::new()
        .fill(if ok { theme::ACCENT_SOFT } else { theme::WARN_BG })
        .corner_radius(CornerRadius::same(9))
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(13.0), egui::Sense::hover());
                widgets::icon(
                    ui.painter(),
                    r.center(),
                    Icon::Shield,
                    if ok { theme::ACCENT } else { theme::WARN },
                    13.0,
                );
                ui.add_space(3.0);
                ui.label(
                    egui::RichText::new(if ok { "隐私输入" } else { "词库加载失败" })
                        .size(11.5)
                        .color(if ok { theme::ACCENT } else { theme::WARN }),
                );
            });
        });
}

/// egui Key → 小写字母（Key 枚举里 A–Z 不保证连续，显式映射最稳）。
fn letter_of(key: egui::Key) -> char {
    use egui::Key;
    match key {
        Key::A => 'a',
        Key::B => 'b',
        Key::C => 'c',
        Key::D => 'd',
        Key::E => 'e',
        Key::F => 'f',
        Key::G => 'g',
        Key::H => 'h',
        Key::I => 'i',
        Key::J => 'j',
        Key::K => 'k',
        Key::L => 'l',
        Key::M => 'm',
        Key::N => 'n',
        Key::O => 'o',
        Key::P => 'p',
        Key::Q => 'q',
        Key::R => 'r',
        Key::S => 's',
        Key::T => 't',
        Key::U => 'u',
        Key::V => 'v',
        Key::W => 'w',
        Key::X => 'x',
        Key::Y => 'y',
        _ => 'z',
    }
}

fn commit(app: &mut App, text: &str) {
    let Some(ed) = app.editor.as_mut() else { return };
    ed.text.push_str(text);
    ed.dirty = true;
    ed.last_edit = Some(Instant::now());
    ed.phase = "有改动，稍后自动保存";
}

fn backspace(app: &mut App) {
    if !app.privacy.buf.is_empty() {
        app.privacy.buf.pop();
        recalc(app);
        return;
    }
    let Some(ed) = app.editor.as_mut() else { return };
    if let Some(ch) = ed.text.chars().next_back() {
        ed.text.truncate(ed.text.len() - ch.len_utf8());
        ed.dirty = true;
        ed.last_edit = Some(Instant::now());
        ed.phase = "有改动，稍后自动保存";
    }
}

fn pick(app: &mut App, idx: usize) {
    let Some(c) = app.privacy.cands.get(idx).cloned() else { return };
    commit(app, &c.text);
    app.privacy.buf = c.rest;
    app.privacy.sel = 0;
    recalc(app);
}

fn cycle_sel(app: &mut App, dir: i32) {
    let n = app.privacy.cands.len().min(9);
    if n == 0 {
        return;
    }
    let cur = app.privacy.sel as i32;
    app.privacy.sel = ((cur + dir).rem_euclid(n as i32)) as usize;
}

fn recalc(app: &mut App) {
    app.privacy.cands = match ime_engine::engine() {
        Ok(e) => e.lookup(&app.privacy.buf),
        Err(_) => Vec::new(),
    };
    app.privacy.sel = 0;
}
