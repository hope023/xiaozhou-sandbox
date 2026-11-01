//! 关于：版本、许可、用到的开源组件。全部是本机静态链接的库，程序不联网。

use eframe::egui::{self, Align, CornerRadius, Layout, Margin, Stroke, Vec2};

use super::theme;

struct Dep {
    name: &'static str,
    license: &'static str,
    what: &'static str,
}

/// 直接依赖（Cargo.toml 里的），加上两套词库。传递依赖的完整清单见
/// 仓库根目录的 THIRD-PARTY-NOTICES.md。
const DEPS: &[Dep] = &[
    Dep { name: "egui / eframe", license: "MIT OR Apache-2.0", what: "GUI 框架，纯 Rust 自绘，无 WebView" },
    Dep { name: "winit / glow", license: "Apache-2.0", what: "窗口与 OpenGL 上下文" },
    Dep { name: "rfd", license: "MIT", what: "原生文件选择对话框" },
    Dep { name: "argon2", license: "MIT OR Apache-2.0", what: "主密码 → 密钥（Argon2id）" },
    Dep { name: "chacha20poly1305", license: "Apache-2.0 OR MIT", what: "XChaCha20-Poly1305 分块加密" },
    Dep { name: "hkdf", license: "MIT OR Apache-2.0", what: "从主密钥派生子密钥" },
    Dep { name: "sha2", license: "MIT OR Apache-2.0", what: "SHA-256" },
    Dep { name: "rand", license: "MIT OR Apache-2.0", what: "密码学随机数" },
    Dep { name: "zeroize", license: "Apache-2.0 OR MIT", what: "密钥离开作用域时抹零" },
    Dep { name: "ed25519-dalek", license: "BSD-3-Clause", what: "授权文件签名校验" },
    Dep { name: "image", license: "MIT OR Apache-2.0", what: "图片解码（png/jpeg/gif/webp/bmp）" },
    Dep { name: "mime_guess", license: "MIT", what: "按扩展名判断类型" },
    Dep { name: "base64", license: "MIT OR Apache-2.0", what: "编码" },
    Dep { name: "uuid", license: "Apache-2.0 OR MIT", what: "条目与密文块标识" },
    Dep { name: "chrono", license: "MIT OR Apache-2.0", what: "时间格式化" },
    Dep { name: "dirs", license: "MIT OR Apache-2.0", what: "定位应用数据目录" },
    Dep { name: "raw-window-handle", license: "MIT OR Apache-2.0", what: "取窗口句柄（反截屏 / 输入法解绑）" },
    Dep { name: "windows", license: "MIT OR Apache-2.0", what: "调系统 API（Media Foundation 视频解码、waveOut 音频输出、反截屏、输入法）" },
    Dep { name: "thiserror", license: "MIT OR Apache-2.0", what: "错误类型" },
    Dep { name: "pinyin-data", license: "MIT", what: "内置输入法单字表（mozillazg）" },
    Dep { name: "phrase-pinyin-data", license: "MIT", what: "内置输入法词组表（mozillazg）" },
];

const DIALOG_W: f32 = 520.0;
/// 组件表三列的固定宽度：名称 / 许可证 / 用途。
/// 用固定宽度而不是让内容撑开，列才不会随文字长短跳来跳去。
const COL_NAME: f32 = 150.0;
const COL_LICENSE: f32 = 132.0;

/// 半透明遮罩 + 居中卡片。返回 true 表示用户要求关闭。
pub fn show(ctx: &egui::Context, open: &mut bool) {
    if !*open {
        return;
    }
    let mut close = false;

    egui::Area::new(egui::Id::new("about-mask"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::Pos2::ZERO)
        .show(ctx, |ui| {
            let screen = ctx
                .input(|i| i.raw.screen_rect)
                .unwrap_or(egui::Rect::EVERYTHING);
            ui.painter()
                .rect_filled(screen, 0.0, egui::Color32::from_rgba_unmultiplied(27, 27, 25, 56));
            // 点遮罩关闭
            let resp = ui.interact(screen, egui::Id::new("about-mask-hit"), egui::Sense::click());
            if resp.clicked() {
                close = true;
            }
        });

    egui::Area::new(egui::Id::new("about-dialog"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(theme::PANEL)
                .stroke(Stroke::new(1.0, theme::LINE))
                .corner_radius(CornerRadius::same(14))
                .inner_margin(Margin::same(22))
                .show(ui, |ui| {
                    ui.set_width(DIALOG_W);
                    ui.with_layout(Layout::top_down(Align::Min), |ui| {
                        header(ui, &mut close);
                        ui.add_space(14.0);
                        ui.label(
                            egui::RichText::new(
                                "纯 Rust 单文件程序：没有 WebView、没有运行时依赖、不联网、没有账号。\
                                 文件内容、文件名、目录结构全部在本机加密后写盘。",
                            )
                            .size(12.0)
                            .color(theme::MUTED),
                        );
                        ui.add_space(14.0);
                        ui.separator();
                        ui.add_space(10.0);

                        ui.label(
                            egui::RichText::new("用到的开源方案")
                                .size(12.5)
                                .color(theme::TEXT)
                                .strong(),
                        );
                        ui.add_space(8.0);

                        deps_table(ui);

                        ui.add_space(10.0);
                        ui.separator();
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new(
                                "界面字体使用系统自带的微软雅黑与 Consolas，未随程序分发。\
                                 传递依赖的完整清单与许可证全文见仓库 THIRD-PARTY-NOTICES.md。",
                            )
                            .size(11.0)
                            .color(theme::FAINT),
                        );
                    });
                });
        });

    if close {
        *open = false;
    }
}

fn header(ui: &mut egui::Ui, close: &mut bool) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(Vec2::splat(32.0), egui::Sense::hover());
        ui.painter()
            .rect_filled(r, CornerRadius::same(9), theme::ACCENT);
        super::widgets::icon(
            ui.painter(),
            r.center(),
            super::widgets::Icon::Shield,
            theme::WHITE,
            18.0,
        );
        ui.add_space(10.0);
        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new(format!("小舟沙盒 {}", env!("CARGO_PKG_VERSION")))
                    .size(15.0)
                    .strong()
                    .color(theme::TEXT),
            );
            ui.add_space(2.0);
            ui.label(
                egui::RichText::new("本地加密保险库 · MIT 许可")
                    .size(11.5)
                    .color(theme::MUTED),
            );
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if super::widgets::ghost_button(ui, "关闭").clicked() {
                *close = true;
            }
        });
    });
}

/// 组件清单：三列固定宽度、左对齐，超长文字按列截断或换行。
fn deps_table(ui: &mut egui::Ui) {
    let gap = 8.0;
    let rest = (ui.available_width() - COL_NAME - COL_LICENSE - gap * 2.0).max(120.0);

    // 包一层浅底描边：滚动区最后一行被裁一半时，看起来是"列表还能往下滚"，
    // 而不是"文字被谁切断了"。
    egui::Frame::new()
        .fill(theme::ROW)
        .stroke(Stroke::new(1.0, theme::LINE))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            // egui 默认是"悬浮滚动条"：只在悬停/滚动时画，而且取的是前景色（深墨），
            // 在这块浅底上会变成一根黑条。改成常驻的实心条 + 浅灰滑块。
            ui.style_mut().spacing.scroll = egui::style::ScrollStyle::solid();
            ui.visuals_mut().widgets.inactive.bg_fill = theme::LINE_STRONG;
            ui.visuals_mut().widgets.hovered.bg_fill = theme::FAINT;
            ui.visuals_mut().widgets.active.bg_fill = theme::MUTED;
            egui::ScrollArea::vertical()
                .id_salt("about-deps")
                .max_height(300.0)
                .auto_shrink([false, false])
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                .show(ui, |ui| {
                    for d in DEPS.iter() {
                        ui.horizontal_top(|ui| {
                            cell(ui, COL_NAME, d.name, 12.0, theme::TEXT, false);
                            ui.add_space(gap);
                            cell(ui, COL_LICENSE, d.license, 11.0, theme::ACCENT, false);
                            ui.add_space(gap);
                            cell(ui, rest, d.what, 11.5, theme::MUTED, true);
                        });
                        ui.add_space(7.0);
                    }
                    ui.add_space(2.0);
                });
        });
}

/// 定宽单元格。add_sized 会把 Label 居中，这里用 allocate_ui_with_layout 强制左对齐。
fn cell(ui: &mut egui::Ui, w: f32, text: &str, size: f32, color: egui::Color32, wrap: bool) {
    ui.allocate_ui_with_layout(
        Vec2::new(w, 18.0),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.add(
                egui::Label::new(egui::RichText::new(text).size(size).color(color)).wrap_mode(
                    if wrap {
                        egui::TextWrapMode::Wrap
                    } else {
                        egui::TextWrapMode::Truncate
                    },
                ),
            );
        },
    );
}
