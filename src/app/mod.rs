//! eframe 界面层：App 状态机 + 帧调度。
//!
//! 帧流程（每帧）：取 HWND / touch 心跳 → 处理自动锁定信号 → 轮询图片解码结果
//! → 编辑器 800ms 自动保存 → 处理拖拽导入 / Ctrl+K → 刷新状态与列表 → 渲染。

mod about;
mod file_list;
mod ime_engine;
mod inspector;
mod lock_screen;
mod panels;
mod sidebar;
mod statusbar;
mod text_editor;
mod theme;
mod widgets;

pub mod privacy;

#[cfg(test)]
mod kittest_tests;

use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::Instant;

use eframe::egui;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

use crate::service::{self, Crumb, EntryView, SearchReport, Status, VersionView};
use crate::state::AppState;
use crate::win32;
use widgets::{Confirm, ConfirmAction, Toast};

/// 左侧导航类别（对应原 NavKind）。
#[derive(Clone, Copy, PartialEq)]
pub enum Nav {
    Dir,
    Image,
    Video,
    Text,
    Trash,
}

impl Nav {
    fn title(&self) -> &'static str {
        match self {
            Nav::Dir => "全部文件",
            Nav::Image => "图片",
            Nav::Video => "视频",
            Nav::Text => "文档",
            Nav::Trash => "回收站",
        }
    }
    fn kind_param(&self) -> &'static str {
        match self {
            Nav::Image => "image",
            Nav::Video => "video",
            Nav::Text => "text",
            _ => "",
        }
    }
}

/// 右侧滑入面板（一次只开一个，与原交互一致）。
#[derive(Clone, Copy, PartialEq)]
pub enum Panel {
    Versions,
    Settings,
    Search,
}

/// 编辑器状态：内容 + 脏标记 + 800ms 防抖保存。
pub struct EditorState {
    pub entry_id: String,
    pub text: String,
    pub dirty: bool,
    pub last_edit: Option<Instant>,
    pub truncated: bool,
    pub phase: &'static str, // 状态条文案
}

/// 解码好的一张图。静态图只有一帧；GIF 是多帧，各带自己的停留时长。
pub struct DecodedImage {
    /// (画面, 停留秒数)。只有一帧时秒数为 0。
    pub frames: Vec<(Arc<egui::ColorImage>, f32)>,
}

pub struct ImageState {
    pub for_id: String,
    pub rx: Option<Receiver<Result<DecodedImage, String>>>,
    pub texture: Option<(String, egui::TextureHandle)>,
    /// GIF 的全部帧。空表示还没解码完。
    pub frames: Vec<(Arc<egui::ColorImage>, f32)>,
    pub frame_idx: usize,
    pub frame_at: std::time::Instant,
}

/// 视频播放状态：读字节 → 开解码器 → 每帧换纹理。
pub struct VideoState {
    pub for_id: String,
    pub rx: Option<Receiver<Result<std::sync::Arc<dyn crate::media::Source>, String>>>,
    pub player: Option<crate::media::Player>,
    pub texture: Option<(String, egui::TextureHandle)>,
    /// 拖动进度条时的临时位置，松手才真正 seek。
    pub scrub: Option<u64>,
    pub err: Option<String>,
}

impl Default for VideoState {
    fn default() -> Self {
        Self {
            for_id: String::new(),
            rx: None,
            player: None,
            texture: None,
            scrub: None,
            err: None,
        }
    }
}

pub struct SearchUi {
    pub q: String,
    pub typed_at: Option<Instant>,
    pub hits: Vec<crate::search::Hit>,
    pub report: SearchReport,
    /// 刚打开面板，下一帧把焦点交给搜索框（只抢一次，见 panels.rs）。
    pub focus_pending: bool,
}

pub struct App {
    pub state: Arc<AppState>,
    /// egui Context：随时可发 ViewportCommand（如 IMEAllowed）
    pub ctx: egui::Context,
    pub hwnd: Option<isize>,
    pub capture_applied: bool,
    pub prev_building: bool,

    pub status: Option<Status>,
    pub need_status: bool,

    pub nav: Nav,
    pub parent: String,
    pub trail: Vec<Crumb>,
    pub entries: Vec<EntryView>,
    pub need_list: bool,
    pub filter: String,
    pub selected: Option<EntryView>,

    pub toast: Option<Toast>,
    pub confirm: Option<Confirm>,

    pub panel: Option<Panel>,
    pub settings_draft: Option<crate::store::Settings>,
    pub settings_capture: bool,
    pub settings_pw: (String, String, String),
    /// (主口令, 新的第二口令, 再输一遍)
    pub settings_second: (String, String, String),
    pub versions: Vec<VersionView>,
    pub versions_for: String,

    pub editor: Option<EditorState>,
    pub image: ImageState,
    pub video: VideoState,
    pub search: SearchUi,

    /// 隐私输入模式（编辑器内切换）。
    pub privacy_on: bool,
    pub privacy: privacy::PrivacyState,
    /// 进入隐私输入时 Win32 IME 压制是否成功（None=尚未尝试）。
    /// 失败不打扰使用（UI 层本身不触发输入法），但要在界面上明示。
    pub ime_detach_ok: Option<bool>,
    /// 上一帧窗口是否有焦点（重获焦点时系统会重新挂输入法，需要重新压制）。
    pub was_window_focused: bool,
    /// 无头/自动化测试钩子帧计数（仅 debug 构建生效）
    #[cfg(debug_assertions)]
    pub e2e_frame: u32,
    #[cfg(debug_assertions)]
    pub e2e_unlocked_done: bool,
    #[cfg(debug_assertions)]
    pub e2e_selected_done: bool,
    #[cfg(debug_assertions)]
    pub e2e_privacy_on: bool,

    // 锁屏状态
    pub lock_pw: String,
    pub lock_pw2: String,
    pub lock_path: String,
    pub lock_err: String,
    /// 首帧把焦点给密码框（只给一次，之后交给用户自己点）
    pub lock_focus_pending: bool,
    /// 菜单里选了「添加新目录…」：即使已有库也显示创建表单
    pub lock_force_create: bool,
    /// 创建表单里的第二口令（可选）
    pub lock_second: String,
    /// 第二口令输入框是否展开
    pub lock_second_open: bool,
    /// 关于窗口是否打开
    pub about_open: bool,

    // 检查器改名状态
    pub renaming: bool,
    pub rename_draft: String,
    /// 刚进入改名，下一帧把焦点交给输入框（只抢一次）。
    ///
    /// 不能每帧都 request_focus()：它会顺带 interrupt_ime()，而
    /// egui-winit 收到后每帧都会 set_ime_allowed(false/true) 重置输入法，
    /// 结果是拼音永远拼不完——英文能打、中文打不进去。
    pub rename_focus_pending: bool,
}

impl App {
    fn new(state: Arc<AppState>, ctx: egui::Context) -> Self {
        Self {
            state,
            ctx,
            hwnd: None,
            capture_applied: false,
            prev_building: false,
            status: None,
            need_status: true,
            nav: Nav::Dir,
            parent: "root".into(),
            trail: Vec::new(),
            entries: Vec::new(),
            need_list: true,
            filter: String::new(),
            selected: None,
            toast: None,
            confirm: None,
            panel: None,
            settings_draft: None,
            settings_capture: false,
            settings_pw: Default::default(),
            settings_second: Default::default(),
            versions: Vec::new(),
            versions_for: String::new(),
            editor: None,
            image: ImageState {
                for_id: String::new(),
                rx: None,
                texture: None,
                frames: Vec::new(),
                frame_idx: 0,
                frame_at: std::time::Instant::now(),
            },
            video: VideoState::default(),
            search: SearchUi {
                q: String::new(),
                typed_at: None,
                hits: Vec::new(),
                report: SearchReport { docs: 0, truncated: false, ms: 0, built: false, built_at: 0 },
                focus_pending: true,
            },
            privacy_on: false,
            privacy: privacy::PrivacyState::default(),
            ime_detach_ok: None,
            was_window_focused: false,
            #[cfg(debug_assertions)]
            e2e_frame: 0,
            #[cfg(debug_assertions)]
            e2e_unlocked_done: false,
            #[cfg(debug_assertions)]
            e2e_selected_done: false,
            #[cfg(debug_assertions)]
            e2e_privacy_on: false,
            lock_pw: String::new(),
            lock_pw2: String::new(),
            lock_path: String::new(),
            lock_err: String::new(),
            lock_focus_pending: true,
            lock_force_create: false,
            lock_second: String::new(),
            lock_second_open: false,
            about_open: false,
            renaming: false,
            rename_draft: String::new(),
            rename_focus_pending: false,
        }
    }

    pub fn unlocked(&self) -> bool {
        self.state.is_unlocked()
    }

    pub fn notify(&mut self, text: impl Into<String>) {
        self.toast = Some(Toast::new(text));
    }

    /// 选中条目（切换时先把旧编辑器的未保存内容落盘）。
    pub fn select(&mut self, e: Option<EntryView>) {
        if let Some(ed) = &self.editor {
            let switching = match &e {
                Some(n) => n.id != ed.entry_id,
                None => true,
            };
            if switching && ed.dirty {
                self.flush_editor();
            }
        }
        if self.editor.as_ref().is_some_and(|ed| {
            !e.as_ref().is_some_and(|n| n.id == ed.entry_id)
        }) {
            // 换文件了：编辑器缓存作废，隐私输入也要退出
            if self.privacy_on {
                self.exit_privacy_mode();
            }
            self.editor = None;
        }
        self.selected = e;
        self.image.for_id.clear();
        self.image.rx = None;
        self.image.frames.clear();
    }

    /// 打开目录（面包屑下钻）。
    pub fn open_dir(&mut self, id: &str, name: &str) {
        self.parent = id.to_string();
        self.trail.push(Crumb { id: id.to_string(), name: name.to_string() });
        self.nav = Nav::Dir;
        self.need_list = true;
        self.select(None);
    }

    pub fn goto_trail(&mut self, idx: usize) {
        self.trail.truncate(idx);
        self.parent = if idx == 0 {
            "root".into()
        } else {
            self.trail.get(idx - 1).map(|c| c.id.clone()).unwrap_or_else(|| "root".into())
        };
        self.nav = Nav::Dir;
        self.need_list = true;
        self.select(None);
    }

    /// 全文搜索跳转：定位 + 展开面包屑 + 选中。
    pub fn open_by_id(&mut self, id: &str) {
        match service::locate_entry(&self.state, id) {
            Ok(loc) => {
                self.nav = Nav::Dir;
                self.trail = loc.trail;
                self.parent = self.trail.last().map(|c| c.id.clone()).unwrap_or_else(|| "root".into());
                self.need_list = true;
                let entry = loc.entry.clone();
                self.select(Some(entry));
            }
            Err(e) => self.notify(format!("定位失败：{e}")),
        }
    }

    /// 把未保存的编辑器内容立刻写盘（尽力而为）。
    pub fn flush_editor(&mut self) {
        let Some(ed) = self.editor.as_mut() else { return };
        if !ed.dirty {
            return;
        }
        let id = ed.entry_id.clone();
        let text = ed.text.clone();
        match service::save_text(&self.state, &id, &text, false) {
            Ok(v) => {
                ed.dirty = false;
                ed.phase = "已加密写入";
                if let Some(s) = self.selected.as_mut() {
                    if s.id == v.id {
                        *s = v;
                    }
                }
            }
            Err(e) => {
                ed.phase = "保存失败";
                self.notify(format!("自动保存失败：{e}"));
            }
        }
    }

    /// 锁库（手动）。先尽力保存编辑器，再清 UI。
    pub fn do_lock(&mut self) {
        self.flush_editor();
        service::lock(&self.state);
        self.on_vault_locked();
        self.notify("已锁定");
    }

    /// 锁库后的 UI 清理（手动锁定与空闲自动锁定共用）。
    fn on_vault_locked(&mut self) {
        if let Some(ed) = self.editor.as_mut() {
            ed.dirty = false; // 锁了就保存不了了，别让它反复尝试
        }
        self.editor = None;
        self.selected = None;
        self.entries.clear();
        self.panel = None;
        self.confirm = None;
        self.privacy_on = false;
        self.image.texture = None;
        self.image.rx = None;
        self.image.frames.clear();
        self.need_status = true;
        // 回到锁屏：重新把焦点交给密码框
        self.lock_focus_pending = true;
        self.lock_pw.clear();
        self.lock_err.clear();
    }

    /// 导入文件/目录到当前目录。
    pub fn import_from(&mut self, paths: Vec<String>) {
        let parent = self.parent.clone();
        match service::import_paths(&self.state, &paths, &parent) {
            Ok(r) => {
                if r.errors.is_empty() {
                    self.notify(format!("已加密导入 {} 项 · 原文件未删除", r.imported));
                } else {
                    self.notify(format!(
                        "已加密导入 {} 项 · {} 个失败 · 原文件未删除",
                        r.imported,
                        r.errors.len()
                    ));
                }
                self.need_list = true;
                self.need_status = true;
            }
            Err(e) => self.notify(format!("导入失败：{e}")),
        }
    }

    /// 进入改名：把原名填进输入框，并标记下一帧抢一次焦点。
    /// 焦点只能抢一次（见 `rename_focus_pending` 的注释）。
    pub fn begin_rename(&mut self, name: &str) {
        self.renaming = true;
        self.rename_draft = name.to_string();
        self.rename_focus_pending = true;
    }

    pub fn new_folder(&mut self) {
        match service::create_folder(&self.state, &self.parent, "新建文件夹") {
            Ok(e) => {
                self.need_list = true;
                self.notify("已创建文件夹");
                self.select(Some(e));
            }
            Err(e) => self.notify(format!("创建失败：{e}")),
        }
    }

    /// 把条目移动到目标文件夹（拖拽落点 / 面包屑）。
    pub fn move_entry_to(&mut self, id: &str, item_name: &str, new_parent: &str, target_name: &str) {
        match service::move_entry(&self.state, id, new_parent) {
            Ok(v) => {
                self.notify(format!("已把「{item_name}」移动到「{target_name}」"));
                if self.selected.as_ref().is_some_and(|s| s.id == v.id) {
                    self.selected = Some(v);
                }
                self.need_list = true;
            }
            Err(e) => self.notify(format!("移动失败：{e}")),
        }
    }

    pub fn delete_selected(&mut self, id: &str) {
        match service::delete_entry(&self.state, id) {
            Ok(()) => {
                self.notify("已移入回收站");
                if self.selected.as_ref().is_some_and(|s| s.id == id) {
                    self.editor = None;
                    self.selected = None;
                }
                self.need_list = true;
                self.need_status = true;
            }
            Err(e) => self.notify(format!("删除失败：{e}")),
        }
    }

    pub fn purge_selected(&mut self, id: &str) {
        match service::purge_entry(&self.state, id) {
            Ok(()) => {
                self.notify("已彻底删除");
                if self.selected.as_ref().is_some_and(|s| s.id == id) {
                    self.editor = None;
                    self.selected = None;
                }
                self.need_list = true;
                self.need_status = true;
            }
            Err(e) => self.notify(format!("删除失败：{e}")),
        }
    }

    pub fn restore_selected(&mut self, id: &str) {
        match service::restore_entry(&self.state, id) {
            Ok(()) => {
                self.notify("已恢复");
                self.need_list = true;
                self.need_status = true;
            }
            Err(e) => self.notify(format!("恢复失败：{e}")),
        }
    }

    /// 导出当前选中文件到用户选择的位置。
    pub fn export_selected(&mut self) {
        let Some(e) = self.selected.clone() else { return };
        if e.kind != "file" {
            self.notify("暂不支持导出文件夹");
            return;
        }
        let Some(dest) = rfd::FileDialog::new().set_file_name(&e.name).save_file() else {
            return;
        };
        match service::export_entry(&self.state, &e.id, &dest.display().to_string()) {
            Ok(()) => self.notify(format!("已解密导出到 {}", dest.display())),
            Err(err) => self.notify(format!("导出失败：{err}")),
        }
    }

    fn refresh_status(&mut self) {
        self.status = Some(service::vault_status(&self.state));
        self.need_status = false;
    }

    fn refresh_list(&mut self) {
        self.need_list = false;
        self.entries = match self.nav {
            Nav::Dir => service::list_dir(&self.state, &self.parent).unwrap_or_default(),
            Nav::Trash => service::list_trash(&self.state).unwrap_or_default(),
            other => service::list_by_kind(&self.state, other.kind_param()).unwrap_or_default(),
        };
        // 保持选中（如果它还在列表里）
        if let Some(sel) = &self.selected {
            if !self.entries.iter().any(|e| e.id == sel.id) {
                self.editor = None;
                self.selected = None;
            }
        }
    }

    /// 每帧前置处理。
    fn frame_start(&mut self, ctx: &egui::Context) {
        // 1. HWND：首帧从 egui 拿到后应用反截屏
        if let Some(h) = self.hwnd {
            if !self.capture_applied && service::startup_capture_wanted(&self.state) {
                service::apply_capture_guard(h, true);
                self.capture_applied = true;
            }
            if self.state.capture_apply_requested.swap(false, std::sync::atomic::Ordering::SeqCst) {
                let on = self.state.config.lock().unwrap().capture_guard;
                let ok = service::apply_capture_guard(h, on);
                self.capture_applied = on && ok;
            }
        }

        // 2. 心跳：有交互（即有重绘）就算活动
        self.state.touch();

        // 2.5 隐私输入期间保持输入法压制：
        //     窗口重获焦点时系统会重新挂 IME，必须立刻摁回去
        let focused_now = ctx.input(|i| i.viewport().focused.unwrap_or(false));
        if focused_now && !self.was_window_focused && self.privacy_on {
            if let Some(h) = self.hwnd {
                service::set_ime_isolated(h, true);
            }
        }
        self.was_window_focused = focused_now;
        if self.privacy_on {
            if let Some(h) = self.hwnd {
                crate::ime::keep_ime_off(h);
            }
        }

        // 3. 空闲自动锁定信号
        if self.state.lock_signal.swap(false, std::sync::atomic::Ordering::SeqCst) {
            self.on_vault_locked();
            self.notify("空闲超时，已自动锁定");
        }

        // 4. 图片解码结果
        if let Some(rx) = &self.image.rx {
            if let Ok(res) = rx.try_recv() {
                match res {
                    Ok(img) => {
                        if let Some((first, _)) = img.frames.first() {
                            let tex = ctx.load_texture(
                                "vault-image",
                                Arc::clone(first),
                                egui::TextureOptions::default(),
                            );
                            self.image.texture = Some((self.image.for_id.clone(), tex));
                        }
                        self.image.frames = img.frames;
                        self.image.frame_idx = 0;
                        self.image.frame_at = std::time::Instant::now();
                    }
                    Err(e) => self.notify(e),
                }
                self.image.rx = None;
            }
        }

        // 5. 建索引后台线程结果
        let building = self.state.search_building.load(std::sync::atomic::Ordering::SeqCst);
        if self.prev_building && !building {
            let rep = self.state.last_search_report.lock().unwrap().take();
            if let Some(rep) = rep {
                if rep.built {
                    let mut msg = format!("索引完成：{} 个文件，用时 {} ms", rep.docs, rep.ms);
                    if rep.truncated {
                        msg.push_str("（已达上限，超出部分未索引）");
                    }
                    self.notify(msg);
                    self.search.report = rep;
                }
            }
            let err = self.state.last_error.lock().unwrap().take();
            if let Some(e) = err {
                self.notify(format!("索引失败：{e}"));
            }
        }
        self.prev_building = building;

        // 6. 编辑器 800ms 防抖自动保存
        let due = self.editor.as_ref().is_some_and(|ed| {
            ed.dirty && ed.last_edit.is_some_and(|t| t.elapsed().as_millis() >= 800)
        });
        if due {
            self.flush_editor();
        }

        // 7. 拖拽导入
        let dropped: Vec<String> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path().display().to_string().into())
                .collect()
        });
        if !dropped.is_empty() && self.unlocked() && self.nav == Nav::Dir {
            self.import_from(dropped);
        }

        // 8. Ctrl+K 打开全文搜索
        let ctrl_k = ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::K));
        if ctrl_k && self.unlocked() {
            self.panel = Some(Panel::Search);
            self.search.focus_pending = true;
            self.search.report = service::search_status(&self.state);
        }

        // 9. 状态/列表刷新
        if self.need_status {
            self.refresh_status();
        }
        if self.need_list && self.unlocked() {
            self.refresh_list();
        }

        // 10. 自动化测试钩子（仅 debug）：自动解锁 → 选中首个文件 → 开隐私模式。
        //     之后由外部注入真实按键，测试 OS 输入法层。
        //     注意 egui 只在有事件时重绘，钩子要主动请求重绘推进帧。
        #[cfg(debug_assertions)]
        if std::env::var("XIAOZHOU_E2E").is_ok() {
            self.e2e_frame = self.e2e_frame.wrapping_add(1);
            if !self.e2e_privacy_on {
                self.ctx
                    .request_repaint_after(std::time::Duration::from_millis(50));
            }
            let f = self.e2e_frame;
            if f % 40 == 0 {
                debug_log(&format!(
                    "[e2e] frame={f} unlocked={} sel={} privacy_on={}",
                    self.e2e_unlocked_done, self.e2e_selected_done, self.privacy_on
                ));
            }
            if !self.e2e_unlocked_done && f >= 15 {
                if !self.unlocked() {
                    self.lock_pw = "test12345678".into();
                    self.try_unlock();
                }
                if self.unlocked() {
                    self.e2e_unlocked_done = true;
                    self.need_list = true;
                    debug_log("[e2e] unlocked");
                }
            } else if self.e2e_unlocked_done && !self.e2e_selected_done && f >= 40 {
                match self.entries.first().cloned() {
                    Some(e) => {
                        self.select(Some(e));
                        self.e2e_selected_done = true;
                        debug_log("[e2e] selected first entry");
                    }
                    None => self.need_list = true,
                }
            } else if self.e2e_selected_done && !self.e2e_privacy_on && f >= 60 {
                // 窗口定位/尺寸走官方命令（外部 MoveWindow 受 DPI 虚拟化干扰不可靠）
                self.ctx
                    .send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::Pos2::new(0.0, 0.0)));
                self.ctx
                    .send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::Vec2::new(1280.0, 800.0)));
                self.set_privacy_mode(true);
                self.e2e_privacy_on = true;
                debug_log("[e2e] privacy mode on");
            }
        }
    }

    fn show_overlays(&mut self, ctx: &egui::Context) {
        Toast::show(ctx, &mut self.toast);

        if let Some(action) = Confirm::show(ctx, &mut self.confirm) {
            match action {
                ConfirmAction::DeleteEntry(id) => self.delete_selected(&id),
                ConfirmAction::PurgeEntry(id) => self.purge_selected(&id),
            }
        }
    }

    /// 进入/退出隐私输入模式（系统 IME 解绑联动）。
    pub fn set_privacy_mode(&mut self, on: bool) {
        if self.privacy_on == on {
            return;
        }
        // 双保险：压制失败不阻断（UI 层本身不触发输入法），但把结果亮在界面上
        self.ime_detach_ok = self.hwnd.map(|h| service::set_ime_isolated(h, on));
        if self.hwnd.is_none() {
            self.ime_detach_ok = Some(false);
        }
        self.privacy_on = on;
        // 重新进入时把焦点交还打字区（旧代码只在 Default 里置 true，
        // 退出再进来就再也拿不到焦点，表现是「打字没反应」）
        self.privacy.need_focus = on;
        if !on {
            self.privacy.buf.clear();
            self.privacy.cands.clear();
            self.privacy.sel = 0;
        }
        // 官方通道：egui-winit 会调 winit 的 set_ime_allowed（TSF 兼容路径）；
        // 隐私模式时 egui 本身也会因无 TextEdit 聚焦而关闭系统输入法，这里是显式兜底
        self.ctx
            .send_viewport_cmd(egui::ViewportCommand::IMEAllowed(!on));
        debug_log(&format!(
            "[ime] privacy_on={on} hwnd={:?} ok={:?}",
            self.hwnd, self.ime_detach_ok
        ));
    }

    pub fn exit_privacy_mode(&mut self) {
        self.set_privacy_mode(false);
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.hwnd.is_none() {
            self.hwnd = current_hwnd(frame);
        }
        self.frame_start(&ctx);

        if self.unlocked() {
            self.show_statusbar(ui);
            self.show_sidebar(ui);
            self.show_file_list(ui);
            self.show_right_panel(ui);
            self.show_inspector(ui);
        } else {
            self.show_lock_menu(ui);
            egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(theme::BG))
                .show(ui, |ui| self.show_lock_screen(ui));
            about::show(&ctx, &mut self.about_open);
        }

        self.show_overlays(&ctx);
    }

    fn on_exit(&mut self) {
        self.flush_editor();
    }
}

/// 追加一行诊断日志到应用数据目录（GUI 无控制台，stderr 看不见）。
pub(crate) fn debug_log(msg: &str) {
    let mut p = service::app_local_data_dir();
    p.push("debug.log");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&p) {
        use std::io::Write;
        let _ = writeln!(
            f,
            "{} {}",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
            msg
        );
    }
}

/// 窗口/任务栏图标。图片编进 exe，不依赖外部文件。
fn load_icon() -> Option<egui::IconData> {
    let bytes = include_bytes!("../../assets/icon.png");
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    Some(egui::IconData {
        rgba: img.into_raw(),
        width: w,
        height: h,
    })
}

/// 从 eframe 的 Frame 拿 Win32 HWND（反截屏 / IME 解绑需要）。
fn current_hwnd(frame: &eframe::Frame) -> Option<isize> {
    let handle = frame.window_handle().ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(w) => Some(w.hwnd.get()),
        _ => None,
    }
}

/// 入口：单实例 → 初始化 → run_native。
pub fn run() -> eframe::Result {
    if !win32::acquire_single_instance(service::APP_ID) {
        eprintln!("小舟沙盒已在运行");
        return Ok(());
    }
    let state = Arc::new(AppState::default());
    service::init(&state);

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("小舟沙盒")
        .with_inner_size([1200.0, 780.0])
        .with_min_inner_size([940.0, 620.0]);
    if let Some(icon) = load_icon() {
        viewport = viewport.with_icon(icon);
    }

    let native = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "小舟沙盒",
        native,
        Box::new(move |cc| {
            theme::install_fonts(&cc.egui_ctx);
            cc.egui_ctx.all_styles_mut(theme::apply);
            service::spawn_idle_locker(Arc::clone(&state), cc.egui_ctx.clone());
            Ok(Box::new(App::new(state, cc.egui_ctx.clone())))
        }),
    )
}
