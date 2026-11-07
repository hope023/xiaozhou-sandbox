//! 通用小组件：格式化函数（原 api.ts 的 fmtSize/fmtTime/fmtDateTime）、
//! toast、居中确认弹窗、描边图标、按钮助手。
//!
//! 图标是原 `Icon.tsx` 的 22 个 24×24 stroke SVG 的手绘移植，
//! 用 painter 直线/圆/矩形重现，视觉上保持 1.6px 圆头描边的气质。

use std::time::{Duration, Instant};

use eframe::egui::{self, Align, Color32, FontId, Layout, Response, CornerRadius, Stroke, StrokeKind, Vec2};

use super::theme;

// ─────────────────────────── 格式化（移植自原 api.ts） ───────────────────────────

pub fn fmt_size(n: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < units.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{n} B")
    } else if v < 10.0 {
        format!("{v:.1} {}", units[i])
    } else {
        format!("{v:.0} {}", units[i])
    }
}

/// 毫秒 → 播放器时间文本（mm:ss，超过一小时带小时）。
pub fn fmt_clock(ms: u64) -> String {
    let total = ms / 1000;
    let (h, m, s) = (total / 3600, (total / 60) % 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

pub fn fmt_time(ts: i64) -> String {
    let now = chrono::Utc::now().timestamp();
    let diff = (now - ts).max(0);
    if diff < 60 {
        return "刚刚".into();
    }
    if diff < 3600 {
        return format!("{} 分钟前", diff / 60);
    }
    if diff < 86_400 {
        return format!("{} 小时前", diff / 3600);
    }
    if diff < 7 * 86_400 {
        return format!("{} 天前", diff / 86_400);
    }
    let dt = chrono::DateTime::from_timestamp(ts, 0).unwrap_or_default();
    dt.with_timezone(&chrono::Local).format("%Y-%m-%d").to_string()
}

pub fn fmt_datetime(ts: i64) -> String {
    let dt = chrono::DateTime::from_timestamp(ts, 0).unwrap_or_default();
    dt.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string()
}

// ─────────────────────────── toast ───────────────────────────

pub struct Toast {
    pub text: String,
    pub at: Instant,
}

impl Toast {
    pub const LIFETIME: Duration = Duration::from_millis(3200);

    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), at: Instant::now() }
    }

    /// 深墨底白字，固定在窗口底部居中。存活期间请求重绘以驱动消失动画。
    pub fn show(ctx: &egui::Context, toast: &mut Option<Toast>) {
        let Some(t) = toast else { return };
        let left = Toast::LIFETIME.saturating_sub(t.at.elapsed());
        if left.is_zero() {
            *toast = None;
            return;
        }
        ctx.request_repaint_after(left);
        egui::Area::new(egui::Id::new("toast"))
            .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -44.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(theme::INK)
                    .corner_radius(CornerRadius::same(10))
                    .inner_margin(egui::Margin::symmetric(14, 9))
                    .show(ui, |ui| {
                        // Area 的可宽高很小，不显式禁止换行的话长文案会一字一行
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(&t.text).size(13.0).color(theme::WHITE),
                            )
                            .wrap_mode(egui::TextWrapMode::Extend),
                        );
                    });
            });
    }
}

// ─────────────────────────── 居中确认弹窗 ───────────────────────────

pub enum ConfirmAction {
    DeleteEntry(String), // 软删除 → 回收站
    PurgeEntry(String),  // 彻底删除
    PurgeTrash,          // 清空回收站
}

pub struct Confirm {
    pub title: String,
    pub body: String,
    pub danger: bool,
    pub action: ConfirmAction,
}

impl Confirm {
    pub fn delete_entry(id: impl Into<String>, name: &str) -> Self {
        Self {
            title: "移入回收站".into(),
            body: format!("「{name}」会进入回收站，随时可以恢复。"),
            danger: true,
            action: ConfirmAction::DeleteEntry(id.into()),
        }
    }

    /// 文件夹是连同内部所有内容一起移入回收站的，文案要说清楚。
    pub fn delete_folder(id: impl Into<String>, name: &str) -> Self {
        Self {
            title: "移入回收站".into(),
            body: format!("「{name}」及其内部所有内容会一起进入回收站，随时可以恢复。"),
            danger: true,
            action: ConfirmAction::DeleteEntry(id.into()),
        }
    }

    pub fn purge_entry(id: impl Into<String>, name: &str) -> Self {
        Self {
            title: "彻底删除".into(),
            body: format!("「{name}」及其全部历史版本将被永久销毁，无法恢复。确定吗？"),
            danger: true,
            action: ConfirmAction::PurgeEntry(id.into()),
        }
    }

    /// 清空整个回收站：条目多的时候逐个确认太烦，但破坏性最大，文案要写死。
    pub fn purge_trash(count: usize) -> Self {
        Self {
            title: "清空回收站".into(),
            body: format!("回收站里的 {count} 项及其全部历史版本将被永久销毁，无法恢复。确定吗？"),
            danger: true,
            action: ConfirmAction::PurgeTrash,
        }
    }

    /// 半透明遮罩 + 居中卡片，与原 confirm 弹窗一致。
    pub fn show(ctx: &egui::Context, confirm: &mut Option<Confirm>) -> Option<ConfirmAction> {
        let Some(c) = confirm else { return None };
        let mut fired = None;
        let mut close = false;

        egui::Area::new(egui::Id::new("confirm-mask"))
            .order(egui::Order::Foreground)
            .fixed_pos(egui::Pos2::ZERO)
            .show(ctx, |ui| {
                let screen = ctx.input(|i| i.raw.screen_rect).unwrap_or(egui::Rect::EVERYTHING);
                ui.painter().rect_filled(screen, 0.0, Color32::from_rgba_unmultiplied(27, 27, 25, 56));
            });

        egui::Area::new(egui::Id::new("confirm-dialog"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(theme::PANEL)
                    .corner_radius(CornerRadius::same(12))
                    .stroke(Stroke::new(1.0, theme::LINE))
                    .inner_margin(egui::Margin::same(18))
                    .show(ui, |ui| {
                        ui.set_min_width(340.0);
                        ui.add_space(2.0);
                        ui.label(egui::RichText::new(&c.title).text_style(egui::TextStyle::Name("H2".into())).color(theme::TEXT));
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new(&c.body).size(13.0).color(theme::MUTED));
                        ui.add_space(14.0);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let cancel = egui::Button::new("取消");
                            if ui.add(cancel).clicked() {
                                close = true;
                            }
                            let label = match (&c.action, c.danger) {
                                (ConfirmAction::PurgeTrash, _) => "清空",
                                (_, true) => "彻底删除",
                                (_, false) => "确定",
                            };
                            let btn = egui::Button::new(
                                egui::RichText::new(label).color(theme::WHITE),
                            )
                            .fill(if c.danger { theme::DANGER } else { theme::ACCENT });
                            if ui.add(btn).clicked() {
                                fired = Some(match &c.action {
                                    ConfirmAction::DeleteEntry(id) => ConfirmAction::DeleteEntry(id.clone()),
                                    ConfirmAction::PurgeEntry(id) => ConfirmAction::PurgeEntry(id.clone()),
                                    ConfirmAction::PurgeTrash => ConfirmAction::PurgeTrash,
                                });
                                close = true;
                            }
                        });
                    });
            });

        if close {
            *confirm = None;
        }
        fired
    }
}

// ─────────────────────────── 按钮/输入助手 ───────────────────────────

/// 主操作按钮：墨绿底白字，34px 高。
pub fn primary_button(ui: &mut egui::Ui, text: &str) -> Response {
    ui.add(
        egui::Button::new(egui::RichText::new(text).color(theme::WHITE))
            .fill(theme::ACCENT)
            .corner_radius(CornerRadius::same(8))
            .min_size(Vec2::new(0.0, 34.0)),
    )
}

/// 次级按钮：白底描边。
pub fn ghost_button(ui: &mut egui::Ui, text: &str) -> Response {
    ui.add(
        egui::Button::new(text)
            .fill(theme::PANEL)
            .stroke(Stroke::new(1.0, theme::LINE_STRONG))
            .corner_radius(CornerRadius::same(8))
            .min_size(Vec2::new(0.0, 30.0)),
    )
}

/// 危险按钮：红字白底红描边。
pub fn danger_button(ui: &mut egui::Ui, text: &str) -> Response {
    ui.add(
        egui::Button::new(egui::RichText::new(text).color(theme::DANGER))
            .fill(theme::PANEL)
            .stroke(Stroke::new(1.0, theme::DANGER))
            .corner_radius(CornerRadius::same(8))
            .min_size(Vec2::new(0.0, 30.0)),
    )
}

/// 小方图标按钮（28×28），配 icon() 用。
pub fn icon_button(ui: &mut egui::Ui, kind: Icon, tip: &str) -> Response {
    let btn = ui.add_sized(
        Vec2::new(28.0, 28.0),
        egui::Button::new("").fill(theme::PANEL).corner_radius(CornerRadius::same(7)),
    );
    let rect = btn.rect;
    let color = if btn.hovered() { theme::ACCENT } else { theme::MUTED };
    icon(ui.painter(), rect.center(), kind, color, 16.0);
    btn.on_hover_text(tip)
}

// ─────────────────────────── 描边图标 ───────────────────────────

#[derive(Clone, Copy, PartialEq)]
#[allow(dead_code)] // 部分图标预留给后续界面
pub enum Icon {
    Folder,
    File,
    Image,
    Video,
    Audio,
    Text,
    Trash,
    Lock,
    Unlock,
    Settings,
    Search,
    Plus,
    Upload,
    Download,
    History,
    Pin,
    ChevronRight,
    X,
    Shield,
    Clock,
    Restore,
    FolderPlus,
    Edit,
    Drive,
    Check,
    Play,
    Pause,
}

const SW: f32 = 1.6;

/// 在 `center` 处画一个 `size` 大小的描边图标。
/// 线条数据取自原 Icon.tsx 的 path，做了等价简化。
pub fn icon(p: &egui::Painter, center: egui::Pos2, kind: Icon, color: Color32, size: f32) {
    let s = size / 24.0; // 原始坐标系 24×24
    let st = Stroke::new(SW * s * 1.2, color);
    let cx = center.x - 12.0 * s;
    let cy = center.y - 12.0 * s;
    let s = s as f64;
    let cx = cx as f64;
    let cy = cy as f64;
    let pt = |x: f64, y: f64| egui::Pos2::new((cx + x * s) as f32, (cy + y * s) as f32);
    let line = |a: (f64, f64), b: (f64, f64)| p.line_segment([pt(a.0, a.1), pt(b.0, b.1)], st);

    match kind {
        Icon::Folder => {
            p.add(egui::epaint::PathShape::line(
                vec![
                    pt(3.0, 7.5),
                    pt(3.0, 18.0),
                    pt(21.0, 18.0),
                    pt(21.0, 9.5),
                    pt(11.5, 9.5),
                    pt(9.5, 7.5),
                    pt(3.0, 7.5),
                ],
                st,
            ));
        }
        Icon::File | Icon::Text => {
            p.add(egui::epaint::PathShape::line(
                vec![pt(6.0, 3.0), pt(14.0, 3.0), pt(18.5, 7.5), pt(18.5, 21.0), pt(6.0, 21.0), pt(6.0, 3.0)],
                st,
            ));
            line((14.0, 3.0), (14.0, 7.5));
            line((14.0, 7.5), (18.5, 7.5));
            if kind == Icon::Text {
                line((9.0, 12.0), (15.5, 12.0));
                line((9.0, 15.5), (15.5, 15.5));
                line((9.0, 19.0), (13.0, 19.0));
            }
        }
        Icon::Image => {
            p.rect_stroke(egui::Rect::from_two_pos(pt(3.5, 4.5), pt(20.5, 19.5)), CornerRadius::same(2), st, StrokeKind::Middle);
            p.circle_filled(pt(9.0, 10.0), 1.6 * s as f32, st.color);
            p.add(egui::epaint::PathShape::line(
                vec![pt(6.0, 17.0), pt(11.0, 12.5), pt(14.5, 16.0), pt(17.0, 13.5), pt(20.0, 17.0)],
                st,
            ));
        }
        Icon::Video => {
            p.rect_stroke(egui::Rect::from_two_pos(pt(3.0, 6.0), pt(15.5, 18.0)), CornerRadius::same(3), st, StrokeKind::Middle);
            p.add(egui::epaint::PathShape::line(
                vec![pt(15.5, 10.5), pt(21.0, 7.5), pt(21.0, 16.5), pt(15.5, 13.5)],
                st,
            ));
        }
        Icon::Audio => {
            p.add(egui::epaint::PathShape::line(
                vec![pt(9.0, 18.0), pt(9.0, 5.5), pt(20.0, 3.5), pt(20.0, 16.0)],
                st,
            ));
            p.circle_stroke(pt(6.6, 18.0), 2.4, st);
            p.circle_stroke(pt(17.6, 16.0), 2.4, st);
        }
        Icon::Trash => {
            line((4.5, 6.5), (19.5, 6.5));
            line((9.5, 6.5), (9.5, 4.5));
            line((9.5, 4.5), (14.5, 4.5));
            line((14.5, 4.5), (14.5, 6.5));
            p.add(egui::epaint::PathShape::line(
                vec![pt(6.5, 6.5), pt(7.5, 20.5), pt(16.5, 20.5), pt(17.5, 6.5)],
                st,
            ));
            line((10.0, 10.0), (10.0, 17.0));
            line((14.0, 10.0), (14.0, 17.0));
        }
        Icon::Lock | Icon::Unlock => {
            p.rect_stroke(egui::Rect::from_two_pos(pt(5.5, 11.0), pt(18.5, 20.5)), CornerRadius::same(3), st, StrokeKind::Middle);
            // 锁钩：半圆拱。解锁 = 右侧留缺口（开口的钩）
            let from = if kind == Icon::Lock { 0.0 } else { 0.25 };
            p.add(egui::epaint::PathShape::line(
                (0..=16)
                    .map(|i| {
                        let t = (from as f64 + (1.0 - from as f64) * i as f64 / 16.0)
                            * std::f64::consts::PI;
                        pt(12.0 - 3.5 * t.cos(), 11.0 - 3.5 * t.sin())
                    })
                    .collect(),
                st,
            ));
            p.circle_filled(pt(12.0, 15.5), 1.6 * s as f32, st.color);
            line((12.0, 16.8), (12.0, 18.6));
        }
        Icon::Settings => {
            p.circle_stroke(pt(12.0, 12.0), 3.2, st);
            p.circle_stroke(pt(12.0, 12.0), 8.6, Stroke::new((SW * s as f32 * 1.2), Color32::TRANSPARENT));
            // 齿：8 根短线
            for i in 0..8 {
                let t = i as f64 / 8.0 * std::f64::consts::TAU;
                let (dx, dy) = (t.sin(), -t.cos());
                line(
                    (12.0 + dx * 8.6, 12.0 + dy * 8.6),
                    (12.0 + dx * 11.0, 12.0 + dy * 11.0),
                );
            }
        }
        Icon::Search => {
            p.circle_stroke(pt(10.5, 10.5), 6.2, st);
            line((15.2, 15.2), (20.0, 20.0));
        }
        Icon::Plus => {
            line((12.0, 5.0), (12.0, 19.0));
            line((5.0, 12.0), (19.0, 12.0));
        }
        Icon::Upload => {
            line((12.0, 19.0), (12.0, 6.0));
            line((12.0, 6.0), (7.0, 11.0));
            line((12.0, 6.0), (17.0, 11.0));
            line((5.0, 20.5), (19.0, 20.5));
        }
        Icon::Download => {
            line((12.0, 5.0), (12.0, 18.0));
            line((12.0, 18.0), (7.0, 13.0));
            line((12.0, 18.0), (17.0, 13.0));
            line((5.0, 20.5), (19.0, 20.5));
        }
        Icon::History | Icon::Clock => {
            p.circle_stroke(pt(12.0, 12.0), 8.2, st);
            line((12.0, 7.0), (12.0, 12.5));
            line((12.0, 12.5), (15.5, 14.5));
        }
        Icon::Pin => {
            line((12.0, 14.5), (12.0, 21.0));
            p.add(egui::epaint::PathShape::line(
                vec![
                    pt(8.5, 3.5),
                    pt(15.5, 3.5),
                    pt(15.5, 10.0),
                    pt(17.5, 12.5),
                    pt(17.5, 14.5),
                    pt(6.5, 14.5),
                    pt(6.5, 12.5),
                    pt(8.5, 10.0),
                    pt(8.5, 3.5),
                ],
                st,
            ));
        }
        Icon::ChevronRight => {
            line((9.0, 5.5), (15.5, 12.0));
            line((15.5, 12.0), (9.0, 18.5));
        }
        Icon::X => {
            line((6.0, 6.0), (18.0, 18.0));
            line((18.0, 6.0), (6.0, 18.0));
        }
        Icon::Shield => {
            p.add(egui::epaint::PathShape::line(
                vec![pt(12.0, 3.0), pt(20.0, 6.0), pt(20.0, 11.5), pt(20.0, 12.0)],
                st,
            ));
            p.add(egui::epaint::PathShape::line(
                vec![
                    pt(20.0, 11.5),
                    pt(20.0, 16.0),
                    pt(16.5, 19.5),
                    pt(12.0, 21.0),
                    pt(7.5, 19.5),
                    pt(4.0, 16.0),
                    pt(4.0, 6.0),
                    pt(12.0, 3.0),
                ],
                st,
            ));
            line((8.8, 12.0), (11.0, 14.2));
            line((11.0, 14.2), (15.4, 9.8));
        }
        Icon::Restore => {
            p.add(egui::epaint::PathShape::line(
                vec![pt(4.5, 5.0), pt(4.5, 10.0), pt(9.5, 10.0)],
                st,
            ));
            p.add(egui::epaint::PathShape::line(
                vec![
                    pt(6.2, 14.0),
                    pt(8.4, 17.2),
                    pt(12.0, 18.6),
                    pt(15.6, 17.4),
                    pt(18.0, 14.4),
                    pt(18.7, 10.6),
                    pt(17.4, 7.0),
                    pt(14.8, 4.9),
                    pt(11.4, 4.2),
                    pt(8.0, 5.2),
                    pt(5.8, 7.4),
                    pt(4.5, 10.0),
                ],
                st,
            ));
        }
        Icon::FolderPlus => {
            p.add(egui::epaint::PathShape::line(
                vec![
                    pt(3.0, 7.5),
                    pt(3.0, 18.0),
                    pt(21.0, 18.0),
                    pt(21.0, 9.5),
                    pt(11.5, 9.5),
                    pt(9.5, 7.5),
                    pt(3.0, 7.5),
                ],
                st,
            ));
            line((12.0, 11.5), (12.0, 16.5));
            line((9.5, 14.0), (14.5, 14.0));
        }
        Icon::Edit => {
            line((4.0, 20.0), (8.5, 19.0));
            p.add(egui::epaint::PathShape::line(
                vec![pt(16.8, 3.8), pt(20.2, 7.2), pt(8.0, 19.4), pt(3.8, 20.2), pt(4.6, 16.0), pt(16.8, 3.8)],
                st,
            ));
            line((14.5, 6.2), (17.8, 9.5));
        }
        Icon::Drive => {
            p.rect_stroke(egui::Rect::from_two_pos(pt(3.0, 12.5), pt(21.0, 18.5)), CornerRadius::same(2), st, StrokeKind::Middle);
            p.rect_stroke(egui::Rect::from_two_pos(pt(3.0, 5.5), pt(21.0, 11.5)), CornerRadius::same(2), st, StrokeKind::Middle);
            p.circle_filled(pt(17.5, 15.5), 1.1 * s as f32, st.color);
            p.circle_filled(pt(17.5, 8.5), 1.1 * s as f32, st.color);
        }
        Icon::Check => {
            line((5.0, 12.5), (10.0, 17.5));
            line((10.0, 17.5), (19.0, 6.5));
        }
        Icon::Play => {
            p.add(egui::Shape::convex_polygon(
                vec![pt(7.5, 4.5), pt(19.0, 12.0), pt(7.5, 19.5)],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Pause => {
            p.rect_filled(
                egui::Rect::from_two_pos(pt(7.0, 4.5), pt(10.6, 19.5)),
                CornerRadius::same(1),
                color,
            );
            p.rect_filled(
                egui::Rect::from_two_pos(pt(13.4, 4.5), pt(17.0, 19.5)),
                CornerRadius::same(1),
                color,
            );
        }
    }
}

/// 空状态提示（图标 + 两行文案），用于检查器/列表的空态。
pub fn empty_state(ui: &mut egui::Ui, kind: Icon, title: &str, sub: &str) {
    ui.vertical_centered(|ui| {
        ui.add_space(28.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(44.0), egui::Sense::hover());
        icon(ui.painter(), rect.center(), kind, theme::FAINT, 40.0);
        ui.add_space(8.0);
        ui.label(egui::RichText::new(title).size(13.5).color(theme::TEXT));
        ui.label(egui::RichText::new(sub).size(12.0).color(theme::FAINT));
        ui.add_space(12.0);
    });
}

/// 警示条（黄底圆角框），如 4MB 截断提醒。
pub fn warn_box(ui: &mut egui::Ui, text: &str) {
    egui::Frame::new()
        .fill(theme::WARN_BG)
        .stroke(Stroke::new(1.0, theme::WARN_LINE))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new(text).size(12.0).color(theme::WARN));
        });
}


