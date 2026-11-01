//! 右侧检查器：标题/改名、操作按钮（版本/导出/删除/恢复）、各类预览、元信息条。

use eframe::egui::{self, Align, Layout, CornerRadius, Stroke, Vec2};

use super::theme;
use super::widgets::{self, Icon};
use super::{App, Panel, VideoState};

impl App {
    pub fn show_inspector(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::PANEL).inner_margin(egui::Margin::same(12)))
            .show(ui, |ui| {
                let Some(e) = self.selected.clone() else {
                    widgets::empty_state(
                        ui,
                        Icon::Shield,
                        "选一个文件，内容会即时解密显示",
                        "解密只在内存里进行，磁盘上永远只有密文。",
                    );
                    return;
                };

                // ── 底部元信息条（先占位）+ CentralPanel 收缩剩余空间 ──
                // 只画 Panel 不收缩 Ui 的话，available_height 会把元信息条也算进去，
                // 下面的隐私条/警示行就会溢出并与元信息条重叠。
                egui::Panel::bottom("insp-meta")
                    .exact_size(26.0)
                    .frame(egui::Frame::new().fill(theme::BG).inner_margin(egui::Margin::symmetric(4, 0)))
                    .show_inside(ui, |ui| {
                        ui.horizontal_centered(|ui| {
                            ui.label(
                                egui::RichText::new(format!(
                                    "大小 {} · 修改 {} · 版本 {}",
                                    widgets::fmt_size(e.size),
                                    widgets::fmt_datetime(e.modified_at),
                                    e.version_count
                                ))
                                .size(11.5)
                                .color(theme::FAINT),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if e.kind == "file" {
                                    let (r, _) =
                                        ui.allocate_exact_size(egui::vec2(8.0, 16.0), egui::Sense::hover());
                                    ui.painter().circle_filled(r.center(), 3.0, theme::ACCENT);
                                    ui.label(
                                        egui::RichText::new("内存解密渲染").size(11.5).color(theme::ACCENT),
                                    );
                                }
                            });
                        });
                    });

                egui::CentralPanel::default()
                    .frame(egui::Frame::new())
                    .show_inside(ui, |ui| {
                        Self::inspector_body(self, ui, &e);
                    });
            });
    }

    fn inspector_body(&mut self, ui: &mut egui::Ui, e: &crate::service::EntryView) {
        {
            // ── 头部：标题/改名 + 操作按钮 ──
            ui.horizontal(|ui| {
                    if self.renaming {
                        let resp = ui.add(
                            egui::TextEdit::singleline(&mut self.rename_draft)
                                .desired_width(260.0)
                                .font(egui::TextStyle::Name("H2".into())),
                        );
                        // 只抢一次焦点：每帧抢会顺带 interrupt_ime()，
                        // 输入法组合被逐帧打断，中文（拼音）根本拼不完。
                        if self.rename_focus_pending {
                            resp.request_focus();
                            self.rename_focus_pending = false;
                        }
                        if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            self.commit_rename(&e.id);
                        }
                        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                            self.renaming = false;
                        }
                        if widgets::ghost_button(ui, "确定").clicked() {
                            self.commit_rename(&e.id);
                        }
                    } else {
                        ui.label(egui::RichText::new(e.name.as_str()).size(17.0).color(theme::TEXT).strong());
                        if e.kind == "file" {
                            chip(ui, &e.mime);
                        }
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if e.deleted_at.is_some() {
                            // 回收站里：恢复 / 彻底删除
                            if widgets::danger_button(ui, "彻底删除").clicked() {
                                self.confirm = Some(widgets::Confirm::purge_entry(e.id.clone(), &e.name));
                            }
                            if widgets::ghost_button(ui, "恢复").clicked() {
                                self.restore_selected(&e.id);
                            }
                            return;
                        }
                        if e.kind == "dir" {
                            if widgets::ghost_button(ui, "打开").clicked() {
                                let (id, name) = (e.id.clone(), e.name.clone());
                                self.open_dir(&id, &name);
                            }
                            if widgets::icon_button(ui, Icon::Trash, "移入回收站").clicked() {
                                self.confirm =
                                    Some(widgets::Confirm::delete_folder(e.id.clone(), &e.name));
                            }
                            if widgets::icon_button(ui, Icon::Edit, "重命名").clicked() {
                                self.begin_rename(&e.name);
                            }
                            return;
                        }
                        if widgets::icon_button(ui, Icon::Trash, "移入回收站").clicked() {
                            self.confirm = Some(widgets::Confirm::delete_entry(e.id.clone(), &e.name));
                        }
                        if widgets::icon_button(ui, Icon::Edit, "重命名").clicked() {
                            self.begin_rename(&e.name);
                        }
                        if widgets::icon_button(ui, Icon::Download, "解密导出到本地").clicked() {
                            self.export_selected();
                        }
                        let tip = format!("版本历史（{} 个版本）", e.version_count);
                        if widgets::icon_button(ui, Icon::History, &tip).clicked() {
                            self.open_versions_panel(&e.id);
                        }
                    });
                });

                ui.separator();
                ui.add_space(4.0);

                // ── 预览主体 ──
                if e.kind == "dir" {
                    self.show_folder(ui, e);
                    return;
                }
                match e.preview.as_str() {
                    "text" => self.show_text_editor(ui),
                    "image" => self.show_image(ui, &e),
                    "video" => self.show_video(ui, &e),
                    _ => self.show_media_stub(ui, &e),
                }
        }
    }

    /// 图片：后台解密 + 解码，首帧显示「解密中…」，完成后上纹理。
    fn show_image(&mut self, ui: &mut egui::Ui, e: &crate::service::EntryView) {
        if self.image.for_id != e.id && self.image.rx.is_none() {
            self.image.for_id = e.id.clone();
            self.image.texture = None;
            self.image.frames.clear();
            self.image.frame_idx = 0;
            let st = Arc::clone(&self.state);
            let id = e.id.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let res = crate::service::read_image(&st, &id)
                    .map_err(|e| e.to_string())
                    .and_then(|(_mime, bytes)| decode_image(&bytes));
                let _ = tx.send(res);
            });
            self.image.rx = Some(rx);
        }

        // GIF：按各帧时长推进。只在换帧时重传纹理，其余时间只安排一次重绘。
        if self.image.frames.len() > 1 {
            let total: f32 = self.image.frames.iter().map(|(_, d)| *d).sum();
            if total > 0.0 {
                let mut t = self.image.frame_at.elapsed().as_secs_f32() % total;
                let mut idx = 0;
                while idx + 1 < self.image.frames.len() && t >= self.image.frames[idx].1 {
                    t -= self.image.frames[idx].1;
                    idx += 1;
                }
                if idx != self.image.frame_idx {
                    let pic = Arc::clone(&self.image.frames[idx].0);
                    if let Some((_, tex)) = &mut self.image.texture {
                        tex.set(pic, egui::TextureOptions::default());
                    }
                    self.image.frame_idx = idx;
                }
                let wait = (self.image.frames[idx].1 - t).max(0.001);
                ui.ctx().request_repaint_after(std::time::Duration::from_secs_f32(wait));
            }
        }

        match &self.image.texture {
            Some((tid, tex)) if *tid == e.id => {
                let avail = ui.available_size();
                ui.with_layout(Layout::top_down_justified(Align::LEFT), |ui| {
                    ui.add(
                        egui::Image::from_texture(tex)
                            .max_size(avail)
                            .corner_radius(CornerRadius::same(6)),
                    );
                });
            }
            _ => {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.spinner();
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("解密中…").size(12.0).color(theme::MUTED));
                });
            }
        }
    }

    /// 视频：读完整字节 → Media Foundation 内存解码 → 逐帧上纹理。
    /// 明文全程只在内存里，不写临时文件。
    fn show_video(&mut self, ui: &mut egui::Ui, e: &crate::service::EntryView) {
        if self.video.for_id != e.id {
            self.video = VideoState { for_id: e.id.clone(), ..Default::default() };
            let st = Arc::clone(&self.state);
            let id = e.id.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let res = crate::service::open_media(&st, &id)
                    .map(|(_mime, src)| src)
                    .map_err(|err| err.to_string());
                let _ = tx.send(res);
            });
            self.video.rx = Some(rx);
        }

        // 字节到了就开解码器
        if self.video.player.is_none() && self.video.err.is_none() {
            let mut got = None;
            if let Some(rx) = &self.video.rx {
                match rx.try_recv() {
                    Ok(r) => got = Some(r),
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        got = Some(Err("读取中断".to_string()))
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {}
                }
            }
            if let Some(r) = got {
                self.video.rx = None;
                match r {
                    Ok(src) => self.video.player = Some(crate::media::Player::open(src)),
                    Err(err) => self.video.err = Some(err),
                }
            }
        }

        // 出错：说清楚原因，并留一条导出的退路
        if let Some(err) = self.video.err.clone() {
            ui.vertical_centered(|ui| {
                ui.add_space(30.0);
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(40.0), egui::Sense::hover());
                widgets::icon(ui.painter(), rect.center(), Icon::Video, theme::FAINT, 36.0);
                ui.add_space(8.0);
                ui.label(egui::RichText::new("这段视频没法内嵌播放").size(13.5).color(theme::TEXT));
                ui.add_space(4.0);
                ui.label(egui::RichText::new(err).size(12.0).color(theme::MUTED));
                ui.add_space(12.0);
                if widgets::primary_button(ui, "解密导出到本地…").clicked() {
                    self.export_selected();
                }
            });
            return;
        }

        let Some(player) = &self.video.player else {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.spinner();
                ui.add_space(6.0);
                ui.label(egui::RichText::new("解密中…").size(12.0).color(theme::MUTED));
            });
            return;
        };

        // 解码线程可能报了错（缺解码器、没有视频轨等）
        if let Some(err) = player.error() {
            self.video.err = Some(err);
            return;
        }

        // ── 底部控制条先占位 ──
        let ended = player.is_ended();
        // 播完也算「暂停态」：按钮要显示成播放，点一下从头再来
        let paused = player.is_paused() || ended;
        let dur = player.duration_ms();
        let mut want_toggle = false;
        let mut want_seek: Option<u64> = None;

        egui::Panel::bottom("video-ctrl")
            .exact_size(40.0)
            .frame(egui::Frame::new().fill(theme::BG).inner_margin(egui::Margin::symmetric(8, 4)))
            .show_inside(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    let kind = if paused { Icon::Play } else { Icon::Pause };
                    let tip = if paused { "播放（空格）" } else { "暂停（空格）" };
                    if widgets::icon_button(ui, kind, tip).clicked() {
                        want_toggle = true;
                    }

                    let cur = self.video.scrub.unwrap_or_else(|| player.position_ms());
                    ui.label(
                        egui::RichText::new(widgets::fmt_clock(cur))
                            .size(11.5)
                            .color(theme::MUTED)
                            .monospace(),
                    );

                    // Slider 的长度看 spacing.slider_width，add_sized 给多宽都不理会。
                    // 右边还要留出总时长标签的位置。
                    let avail = (ui.available_width() - 56.0).max(80.0);
                    let mut pos = cur as f64;
                    let max = dur.max(1) as f64;
                    let resp = ui
                        .scope(|ui| {
                            ui.spacing_mut().slider_width = avail;
                            ui.add(egui::Slider::new(&mut pos, 0.0..=max).show_value(false))
                        })
                        .inner;
                    if resp.dragged() {
                        self.video.scrub = Some(pos as u64);
                    } else if resp.changed() || resp.drag_stopped() {
                        // 拖完松手，或直接点轨道任意位置
                        want_seek = Some(self.video.scrub.take().unwrap_or(pos as u64));
                    }

                    ui.label(
                        egui::RichText::new(widgets::fmt_clock(dur))
                            .size(11.5)
                            .color(theme::FAINT)
                            .monospace(),
                    );
                });
            });

        // ── 快捷键：空格 播放/暂停，←/→ 后退/前进 10 秒 ──
        // 放在控制条之后：万一进度条此刻有焦点，它自己会处理方向键，
        // 这里覆盖掉，保证一次按键就是 ±10 秒。
        // 正文/搜索/改名框在收键盘时一律不抢，否则空格会变成「暂停 + 一个空格」。
        if !ui.ctx().text_edit_focused() {
            const STEP: u64 = 10_000;
            let cur = self.video.scrub.unwrap_or_else(|| player.position_ms());
            ui.input(|i| {
                if i.key_pressed(egui::Key::Space) {
                    want_toggle = true;
                }
                if i.key_pressed(egui::Key::ArrowLeft) {
                    want_seek = Some(cur.saturating_sub(STEP));
                }
                if i.key_pressed(egui::Key::ArrowRight) {
                    want_seek = Some((cur + STEP).min(dur.max(cur)));
                }
            });
        }

        if want_toggle {
            if ended {
                player.seek_ms(0);
                player.set_paused(false);
            } else {
                player.toggle_paused();
            }
        }
        if let Some(ms) = want_seek {
            player.seek_ms(ms);
        }

        // ── 画面 ──
        if let Some(frame) = player.take_frame() {
            let img = egui::ColorImage::from_rgba_unmultiplied([frame.w, frame.h], &frame.rgba);
            let tex = ui.ctx().load_texture(
                "video-frame",
                img,
                egui::TextureOptions::LINEAR,
            );
            self.video.texture = Some((e.id.clone(), tex));
        }

        match &self.video.texture {
            Some((tid, tex)) if *tid == e.id => {
                let (vw, vh) = player.size();
                let avail = ui.available_size();
                let scale = if vw > 0 && vh > 0 {
                    (avail.x / vw as f32).min(avail.y / vh as f32).max(0.01)
                } else {
                    1.0
                };
                let size = Vec2::new(vw as f32 * scale, vh as f32 * scale);
                ui.centered_and_justified(|ui| {
                    ui.add(
                        egui::Image::from_texture(tex)
                            .fit_to_exact_size(size)
                            .corner_radius(CornerRadius::same(6)),
                    );
                });
            }
            _ => {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.spinner();
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("首帧解码中…").size(12.0).color(theme::MUTED));
                });
            }
        }

        // 解码线程按帧推进，界面要持续刷新
        if !paused {
            ui.ctx().request_repaint();
        }
    }

    /// 文件夹没有可预览的内容：只说明它能做什么，
    /// 不要落到「导出到本地」那种对目录没有意义的按钮上。
    fn show_folder(&mut self, ui: &mut egui::Ui, e: &crate::service::EntryView) {
        ui.vertical_centered(|ui| {
            ui.add_space(36.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(44.0), egui::Sense::hover());
            widgets::icon(ui.painter(), rect.center(), Icon::Folder, theme::FAINT, 40.0);
            ui.add_space(6.0);
            ui.label(egui::RichText::new(e.name.as_str()).size(14.0).color(theme::TEXT));
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new("双击左侧列表进入文件夹；删除会连同内部内容一起进入回收站。")
                    .size(12.0)
                    .color(theme::MUTED),
            );
            ui.add_space(24.0);
        });
    }

    /// 音频/其他格式：暂不内嵌播放，提供「导出后打开」。
    fn show_media_stub(&mut self, ui: &mut egui::Ui, e: &crate::service::EntryView) {
        ui.vertical_centered(|ui| {
            ui.add_space(36.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(44.0), egui::Sense::hover());
            let icon = match e.preview.as_str() {
                "video" => Icon::Video,
                "audio" => Icon::Audio,
                _ => Icon::File,
            };
            widgets::icon(ui.painter(), rect.center(), icon, theme::FAINT, 40.0);
            ui.add_space(6.0);
            ui.label(egui::RichText::new(e.name.as_str()).size(14.0).color(theme::TEXT));
            ui.label(
                egui::RichText::new(format!(
                    "{} · {}",
                    if e.mime.is_empty() { "未知格式" } else { e.mime.as_str() },
                    widgets::fmt_size(e.size)
                ))
                .size(11.5)
                .color(theme::FAINT),
            );
            ui.add_space(10.0);
            ui.label(
                egui::RichText::new("这一版不做内嵌播放。导出是你主动把明文写到磁盘，其余时间磁盘上只有密文。")
                    .size(12.0)
                    .color(theme::MUTED),
            );
            ui.add_space(8.0);
            if widgets::primary_button(ui, "解密导出到本地…").clicked() {
                self.export_selected();
            }
            ui.add_space(24.0);
        });
    }

    fn commit_rename(&mut self, id: &str) {
        let name = self.rename_draft.trim().to_string();
        self.renaming = false;
        if name.is_empty() {
            return;
        }
        match crate::service::rename_entry(&self.state, id, &name) {
            Ok(v) => {
                if let Some(s) = self.selected.as_mut() {
                    if s.id == v.id {
                        *s = v;
                    }
                }
                self.need_list = true;
            }
            Err(e) => self.notify(format!("重命名失败：{e}")),
        }
    }

    pub fn open_versions_panel(&mut self, id: &str) {
        self.panel = Some(Panel::Versions);
        self.versions_for = id.to_string();
        self.versions = crate::service::list_versions(&self.state, id).unwrap_or_default();
    }
}


/// 解码图片。GIF 会把所有帧都解出来（有动画才多帧），其余格式只解一帧。
fn decode_image(bytes: &[u8]) -> Result<super::DecodedImage, String> {
    if bytes.len() > 6 && &bytes[..3] == b"GIF" {
        if let Some(frames) = decode_gif(bytes) {
            return Ok(super::DecodedImage { frames });
        }
    }
    let img = image::load_from_memory(bytes).map_err(|err| format!("图片解码失败：{err}"))?;
    let rgba = img.to_rgba8();
    let ci = egui::ColorImage::from_rgba_unmultiplied(
        [rgba.width() as usize, rgba.height() as usize],
        rgba.as_raw(),
    );
    Ok(super::DecodedImage {
        frames: vec![(Arc::new(ci), 0.0)],
    })
}

/// 解 GIF 的全部帧。超过上限就返回 None，调用方退回静态首帧——
/// 一个几百帧的大 GIF 解出来能吃掉几个 GB，不能无脑全收。
fn decode_gif(bytes: &[u8]) -> Option<Vec<(Arc<egui::ColorImage>, f32)>> {
    use image::AnimationDecoder;

    const MAX_FRAMES: usize = 600;
    const MAX_BYTES: usize = 256 << 20;

    let dec = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(bytes)).ok()?;
    let mut out: Vec<(Arc<egui::ColorImage>, f32)> = Vec::new();
    let mut used = 0usize;
    for frame in dec.into_frames() {
        let frame = frame.ok()?;
        let (num, den) = frame.delay().numer_denom_ms();
        let mut secs = if den == 0 {
            0.0
        } else {
            num as f32 / den as f32 / 1000.0
        };
        // 很多老 GIF 的延时时长写的是 0，浏览器按 100ms 处理，这里跟随
        if !(secs > 0.0) {
            secs = 0.1;
        }
        let buf = frame.into_buffer();
        let (w, h) = buf.dimensions();
        used += (w as usize) * (h as usize) * 4;
        if out.len() >= MAX_FRAMES || used > MAX_BYTES {
            return None;
        }
        out.push((
            Arc::new(egui::ColorImage::from_rgba_unmultiplied(
                [w as usize, h as usize],
                buf.as_raw(),
            )),
            secs,
        ));
    }
    if out.len() > 1 {
        Some(out)
    } else {
        None
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use image::codecs::gif::{GifEncoder, Repeat};
    use image::{Delay, Frame, Rgba, RgbaImage};

    fn make_gif(frames: usize, delay_ms: u32) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut enc = GifEncoder::new(&mut out);
            enc.set_repeat(Repeat::Infinite).unwrap();
            for i in 0..frames {
                let img = RgbaImage::from_pixel(8, 8, Rgba([(i * 40) as u8, 20, 30, 255]));
                let f = Frame::from_parts(img, 0, 0, Delay::from_numer_denom_ms(delay_ms, 1));
                enc.encode_frame(f).unwrap();
            }
        }
        out
    }

    /// 动图要解出全部帧，静态图只解一帧。
    #[test]
    fn gif_decodes_all_frames() {
        let gif = make_gif(4, 100);
        let d = decode_image(&gif).unwrap();
        assert_eq!(d.frames.len(), 4, "应该解出 4 帧");
        for (ci, delay) in &d.frames {
            assert_eq!((ci.width(), ci.height()), (8, 8));
            assert!((delay - 0.1).abs() < 0.005, "帧延时应该是 0.1 秒，实际 {delay}");
        }
        // 帧内容确实不一样，不是同一个画面重复
        assert_ne!(d.frames[0].0.pixels, d.frames[3].0.pixels);
    }

    /// 单帧 GIF 和普通图片一样处理，不进入动画循环。
    #[test]
    fn single_frame_gif_is_static() {
        let gif = make_gif(1, 100);
        let d = decode_image(&gif).unwrap();
        assert_eq!(d.frames.len(), 1);
        assert_eq!(d.frames[0].1, 0.0, "静态图不该有动画时长");
    }

    #[test]
    fn png_still_decodes() {
        let img = RgbaImage::from_pixel(5, 7, Rgba([1, 2, 3, 255]));
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        let d = decode_image(&buf).unwrap();
        assert_eq!(d.frames.len(), 1);
        assert_eq!((d.frames[0].0.width(), d.frames[0].0.height()), (5, 7));
    }
}

fn chip(ui: &mut egui::Ui, text: &str) {
    egui::Frame::new()
        .fill(theme::ROW)
        .stroke(Stroke::new(1.0, theme::LINE))
        .corner_radius(CornerRadius::same(9))
        .inner_margin(egui::Margin::symmetric(7, 2))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).size(11.0).color(theme::MUTED));
        });
}

use std::sync::Arc;
