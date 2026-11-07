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
                self.show_tab_bar(ui);
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
        // 选中文件就开始查重（后台线程，结论到了角标自己会变）
        if e.kind == "file" && e.deleted_at.is_none() {
            self.ensure_dup_check(&e.id);
        }
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
                            // 焦点真正落到输入框的这一帧，再确认一次输入法可用：
                            // 点「重命名」时 hwnd 可能还没拿到，begin_rename 里那次
                            // 补救不一定生效；这里补上就不会出现「点开了却打不了字」。
                            if let Some(h) = self.hwnd {
                                crate::service::ensure_ime_available(h);
                            }
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
                        // 右上角：这份内容在库里还有没有别的副本
                        self.dup_chip(ui, e);
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

    /// 页签栏：打开过的文件/文件夹。点切换，点 × 关闭。
    ///
    /// 页签是跨目录的——浏览到别的文件夹、点侧栏分类都不会把它们关掉，
    /// 只有锁库、删除对应条目、或者手动点 × 才关。
    fn show_tab_bar(&mut self, ui: &mut egui::Ui) {
        self.sync_active_entry();
        if self.tabs.is_empty() {
            return;
        }

        let mut activate: Option<usize> = None;
        let mut close: Option<usize> = None;

        egui::ScrollArea::horizontal()
            .id_salt("insp-tabs")
            .max_height(28.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for i in 0..self.tabs.len() {
                        let active = i == self.active;
                        let name = short_name(&self.tabs[i].entry.name, 10);
                        let kind = self.tabs[i].entry.kind.clone();
                        let preview = self.tabs[i].entry.preview.clone();

                        let (rect, resp) =
                            ui.allocate_exact_size(Vec2::new(132.0, 26.0), egui::Sense::click());
                        let bg = if active { theme::PANEL } else { theme::BG };
                        ui.painter().rect_filled(rect, CornerRadius::same(7), bg);
                        if active {
                            ui.painter().rect_stroke(
                                rect,
                                CornerRadius::same(7),
                                Stroke::new(1.0, theme::LINE),
                                egui::StrokeKind::Inside,
                            );
                        }

                        let icon = tab_icon(&kind, &preview);
                        let (_, ink) = theme::tint(if kind == "dir" { "dir" } else { preview.as_str() });
                        widgets::icon(
                            ui.painter(),
                            egui::pos2(rect.left() + 13.0, rect.center().y),
                            icon,
                            ink,
                            13.0,
                        );
                        // 名字裁在页签里，别让它横着长出去
                        let clip = ui.painter().with_clip_rect(rect.shrink2(Vec2::new(8.0, 0.0)));
                        clip.text(
                            egui::pos2(rect.left() + 25.0, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            &name,
                            egui::FontId::proportional(12.0),
                            if active { theme::TEXT } else { theme::MUTED },
                        );

                        // × 只在当前页签或悬停时出现，平时不抢视觉
                        let close_rect = egui::Rect::from_center_size(
                            egui::pos2(rect.right() - 13.0, rect.center().y),
                            Vec2::splat(18.0),
                        );
                        let close_resp = ui.interact(
                            close_rect,
                            ui.id().with(("tab-close", i)),
                            egui::Sense::click(),
                        );
                        if active || resp.hovered() || close_resp.hovered() {
                            if close_resp.hovered() {
                                ui.painter().circle_filled(close_rect.center(), 8.0, theme::LINE);
                            }
                            widgets::icon(
                                ui.painter(),
                                close_rect.center(),
                                Icon::X,
                                theme::MUTED,
                                11.0,
                            );
                        }
                        if close_resp.clicked() {
                            close = Some(i);
                        } else if resp.clicked() {
                            activate = Some(i);
                        }
                    }
                });
            });

        if let Some(i) = close {
            // 关掉正在编辑且有未保存内容的页签前，先落盘——关页签不等于放弃修改
            if i == self.active && self.editor.as_ref().is_some_and(|ed| ed.dirty) {
                self.flush_editor();
            }
            self.close_tab(i);
        } else if let Some(i) = activate {
            self.activate_tab(i);
        }
        ui.add_space(6.0);
    }

    /// 右上角的查重角标：这同一份内容在库里还有没有别的位置。
    ///
    /// 三态：算着呢 / 只有这一份 / 另有 N 份（点开列出去处，点条目跳过去）。
    /// 只在文件上出现——文件夹没有内容可比。结论只按内容指纹（SHA-256）算，
    /// 不猜名字：同名不同内容、同内容不同名都能分得清。
    fn dup_chip(&mut self, ui: &mut egui::Ui, e: &crate::service::EntryView) {
        if e.kind != "file" || e.deleted_at.is_some() {
            return;
        }

        let pending = self.dup.for_id != e.id || self.dup.pending;
        let big = self.dup.skipped_size;
        let (label, fg, bg, line) = if pending {
            ("比对中…".to_string(), theme::FAINT, theme::PANEL, theme::LINE)
        } else if self.dup.error.is_some() {
            ("查重失败".to_string(), theme::WARN, theme::WARN_BG, theme::WARN_LINE)
        } else if big.is_some() {
            // 大文件不查：与其读十几秒磁盘，不如老实说「没查」
            ("未查重".to_string(), theme::FAINT, theme::PANEL, theme::LINE)
        } else if self.dup.hits.is_empty() {
            // 有同大小的旧文件没能比对时不能说「无重复」
            let t = if self.dup.truncated > 0 { "未查全" } else { "无重复" };
            (t.to_string(), theme::FAINT, theme::PANEL, theme::LINE)
        } else {
            (
                format!("另有 {} 份", self.dup.hits.len()),
                theme::ACCENT,
                theme::ACCENT_SOFT,
                theme::ACCENT_LINE,
            )
        };

        let btn = ui.add(
            egui::Button::new(egui::RichText::new(label).size(11.5).color(fg))
                .fill(bg)
                .stroke(Stroke::new(1.0, line))
                .corner_radius(CornerRadius::same(7))
                .min_size(Vec2::new(0.0, 22.0)),
        );

        if pending {
            btn.on_hover_text("正在按内容指纹比对库里的其他文件（旧库里的文件要先解密算一次指纹）");
            return;
        }
        if let Some(err) = self.dup.error.clone() {
            btn.on_hover_text(format!("查重没成功：{err}"));
            return;
        }
        if let Some(size) = big {
            btn.on_hover_text(format!(
                "这份有 {}，还没有内容指纹（旧库里的超大文件），所以没自动比对。\n现算指纹要解密整份文件，大文件是十几秒级别。",
                widgets::fmt_size(size)
            ));
            return;
        }
        if self.dup.hits.is_empty() {
            if self.dup.truncated > 0 {
                btn.on_hover_text(format!(
                    "另有 {} 份同大小的文件没能比对：旧库里没有指纹的超大文件，或一次要算的份数太多。",
                    self.dup.truncated
                ));
            } else {
                btn.on_hover_text("按内容指纹（SHA-256）比对过：库里只有这一份。");
            }
            return;
        }

        // 有副本：点开一份清单，逐条能跳过去。
        // 用 Popup 而不是 tooltip——提示框里点不了，而且一移开鼠标就没了。
        let hits = self.dup.hits.clone();
        let truncated = self.dup.truncated;
        let popup_id = egui::Id::new(("dup-popup", e.id.as_str()));
        let mut goto: Option<String> = None;
        egui::Popup::from_toggle_button_response(&btn)
            .id(popup_id)
            .align(egui::RectAlign::BOTTOM_END)
            .gap(6.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::PANEL)
                    .stroke(Stroke::new(1.0, theme::LINE_STRONG))
                    .corner_radius(CornerRadius::same(10))
                    .inner_margin(egui::Margin::same(8))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 4],
                        blur: 12,
                        spread: 0,
                        color: egui::Color32::from_black_alpha(20),
                    }),
            )
            .show(|ui| {
                ui.set_width(300.0);
                ui.label(
                    egui::RichText::new(format!("内容完全相同，还有 {} 份", hits.len()))
                        .size(11.5)
                        .color(theme::FAINT),
                );
                ui.add_space(2.0);
                for h in &hits {
                    if dup_row(ui, h) {
                        goto = Some(h.id.clone());
                    }
                }
                if truncated > 0 {
                    ui.add_space(2.0);
                    ui.label(
                        egui::RichText::new(format!("另有 {truncated} 份同大小的文件没能比对"))
                            .size(11.0)
                            .color(theme::FAINT),
                    );
                }
            });
        if let Some(id) = goto {
            self.open_by_id(&id);
        }
        if !egui::Popup::is_id_open(ui.ctx(), popup_id) {
            btn.on_hover_text("点开看这些副本都在哪个目录");
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
                // 提示里带文件名，改名后要刷一下
                self.invalidate_duplicates();
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


/// 页签上的类型图标。
fn tab_icon(kind: &str, preview: &str) -> Icon {
    if kind == "dir" {
        return Icon::Folder;
    }
    match preview {
        "image" => Icon::Image,
        "video" => Icon::Video,
        "audio" => Icon::Audio,
        "text" => Icon::Text,
        _ => Icon::File,
    }
}

/// 页签宽度有限，名字按字符数截断（中文按一个字算）。
fn short_name(name: &str, max: usize) -> String {
    let mut s: String = name.chars().take(max).collect();
    if name.chars().count() > max {
        s.push('…');
    }
    s
}

/// 弹层里的一行副本：文件名 + 所在目录，整行可点。
/// 手绘是为了两行用不同字号/颜色——egui 的 Button 装不下两段样式。
fn dup_row(ui: &mut egui::Ui, h: &crate::service::DuplicateHit) -> bool {
    let width = ui.available_width().max(240.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 38.0), egui::Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(7), theme::HOVER);
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    // 名字太长就裁掉，不要让它在弹层里横着长出去
    let painter = ui.painter().with_clip_rect(rect.shrink2(Vec2::new(6.0, 0.0)));
    painter.text(
        rect.left_top() + Vec2::new(8.0, 4.0),
        egui::Align2::LEFT_TOP,
        &h.name,
        egui::FontId::proportional(12.5),
        theme::TEXT,
    );
    painter.text(
        rect.left_top() + Vec2::new(8.0, 21.0),
        egui::Align2::LEFT_TOP,
        &h.folder,
        egui::FontId::proportional(11.0),
        theme::FAINT,
    );
    resp.on_hover_text(format!("{}
{}", h.name, h.folder)).clicked()
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
