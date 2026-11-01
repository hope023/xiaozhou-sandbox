//! 视觉主题：把原网页版的暖白纸感 + 墨绿主色 1:1 搬进 egui。
//! 色值来自原 `styles.css` 的 CSS 变量，不要随手改——它们是设计的一部分。

use eframe::egui;
use eframe::egui::{Color32, FontDefinitions, FontFamily, Style, Visuals};

pub const BG: Color32 = Color32::from_rgb(0xf6, 0xf6, 0xf3); // --bg 暖白纸面
pub const PANEL: Color32 = Color32::from_rgb(0xff, 0xff, 0xff); // --panel 白面板
pub const LINE: Color32 = Color32::from_rgb(0xe7, 0xe6, 0xe1); // --line 发丝线
pub const LINE_STRONG: Color32 = Color32::from_rgb(0xd8, 0xd7, 0xd0); // --line-strong
pub const TEXT: Color32 = Color32::from_rgb(0x1b, 0x1b, 0x19); // --text
pub const MUTED: Color32 = Color32::from_rgb(0x75, 0x74, 0x6c); // --muted
pub const FAINT: Color32 = Color32::from_rgb(0xa3, 0xa2, 0x9a); // --faint
pub const ACCENT: Color32 = Color32::from_rgb(0x0f, 0x6e, 0x56); // --accent 墨绿
#[allow(dead_code)] // hover 加深色，预留给按钮按下态
pub const ACCENT_DARK: Color32 = Color32::from_rgb(0x0d, 0x5f, 0x4a); // hover 加深
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(0xe1, 0xf5, 0xee); // --accent-soft 选中底
pub const ACCENT_LINE: Color32 = Color32::from_rgb(0x9f, 0xe1, 0xcb); // --accent-line
pub const DANGER: Color32 = Color32::from_rgb(0xa3, 0x2d, 0x2d); // --danger
#[allow(dead_code)] // 预留给删除确认态
pub const DANGER_SOFT: Color32 = Color32::from_rgb(0xfc, 0xeb, 0xeb); // --danger-soft
pub const WARN: Color32 = Color32::from_rgb(0x85, 0x4f, 0x0b); // --warn
pub const WARN_BG: Color32 = Color32::from_rgb(0xfd, 0xf6, 0xe8); // 警示条底
pub const WARN_LINE: Color32 = Color32::from_rgb(0xf0, 0xdf, 0xb8); // 警示条边
pub const INK: Color32 = Color32::from_rgb(0x26, 0x26, 0x1f); // 深墨（toast/激活 chip）
pub const HOVER: Color32 = Color32::from_rgb(0xf1, 0xf1, 0xec); // 列表行悬停
pub const ROW: Color32 = Color32::from_rgb(0xfb, 0xfb, 0xf9); // 列表底
pub const WHITE: Color32 = Color32::from_rgb(0xff, 0xff, 0xff);

/// 文件类型的色块配色（dir/image/video/audio/text/other），来自原 FileList 的 TINT。
pub fn tint(kind: &str) -> (Color32, Color32) {
    // (底色, 图标色)
    match kind {
        "dir" => (Color32::from_rgb(0xfa, 0xee, 0xda), Color32::from_rgb(0x85, 0x4f, 0x0b)),
        "image" => (Color32::from_rgb(0xe3, 0xef, 0xfb), Color32::from_rgb(0x1d, 0x54, 0x93)),
        "video" => (Color32::from_rgb(0xf0, 0xe8, 0xfa), Color32::from_rgb(0x5b, 0x2d, 0x86)),
        "audio" => (Color32::from_rgb(0xfa, 0xe8, 0xf1), Color32::from_rgb(0x93, 0x25, 0x5c)),
        "text" => (Color32::from_rgb(0xe1, 0xf5, 0xee), Color32::from_rgb(0x0f, 0x6e, 0x56)),
        _ => (Color32::from_rgb(0xef, 0xef, 0xeb), Color32::from_rgb(0x5c, 0x5b, 0x54)),
    }
}

/// 全局 Style/Visuals。圆角 8px（控件）/10px（面板），发丝分隔线，无阴影渐变。
pub fn apply(style: &mut Style) {
    let v = &mut style.visuals;
    *v = Visuals::light();
    v.window_fill = PANEL;
    v.panel_fill = PANEL;
    v.extreme_bg_color = ROW; // 文本框内部底色
    v.faint_bg_color = ROW; // 斑马纹
    v.window_stroke = egui::Stroke::new(1.0, LINE);
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, LINE);
    v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, TEXT);
    for w in [
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = egui::CornerRadius::same(8);
        w.fg_stroke = egui::Stroke::new(1.2, TEXT);
    }
    v.widgets.inactive.bg_fill = PANEL;
    v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, LINE_STRONG);
    v.widgets.hovered.bg_fill = HOVER;
    v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, LINE_STRONG);
    v.widgets.active.bg_fill = ACCENT_SOFT;
    v.widgets.active.bg_stroke = egui::Stroke::new(1.0, ACCENT);
    v.selection.bg_fill = ACCENT_SOFT;
    v.selection.stroke = egui::Stroke::new(1.0, ACCENT);
    v.hyperlink_color = ACCENT;
    v.window_corner_radius = egui::CornerRadius::same(10);
    v.menu_corner_radius = egui::CornerRadius::same(10);
    v.override_text_color = Some(TEXT);

    use egui::FontId;
    let ts = &mut style.text_styles;
    ts.insert(egui::TextStyle::Body, FontId::proportional(13.0));
    ts.insert(egui::TextStyle::Button, FontId::proportional(13.0));
    ts.insert(egui::TextStyle::Small, FontId::proportional(11.0));
    ts.insert(egui::TextStyle::Heading, FontId::proportional(17.0));
    ts.insert(egui::TextStyle::Name("H2".into()), FontId::proportional(14.5));
    ts.insert(egui::TextStyle::Name("Meta".into()), FontId::proportional(11.5));
    ts.insert(egui::TextStyle::Monospace, FontId::monospace(13.0));

    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    style.spacing.menu_margin = egui::Margin::same(8);
}

/// 运行时加载系统字体：微软雅黑（界面/正文）+ Consolas（编辑器等宽）。
/// 都不打进 exe——系统自带，保持绿色小体积。加载失败就退回 egui 默认拉丁字体。
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    // 中文主字体
    if let Ok(data) = std::fs::read(r"C:\Windows\Fonts\msyh.ttc") {
        fonts.font_data.insert("msyh".into(), egui::FontData::from_owned(data).into());
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .insert(0, "msyh".into());
        // 等宽场景（编辑器）也挂雅黑做中文回退
        fonts
            .families
            .entry(FontFamily::Monospace)
            .or_default()
            .push("msyh".into());
    }
    // 等宽：Consolas 打头（拉丁），雅黑兜底中文
    if let Ok(data) = std::fs::read(r"C:\Windows\Fonts\consola.ttf") {
        fonts.font_data.insert("consola".into(), egui::FontData::from_owned(data).into());
        fonts
            .families
            .entry(FontFamily::Monospace)
            .or_default()
            .insert(0, "consola".into());
    }
    ctx.set_fonts(fonts);
}
