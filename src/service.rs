//! 服务层 —— 原 Tauri 的 38 个命令改为普通函数，UI 直接调用。
//!
//! 逻辑体与原 `commands.rs` 保持一致（`state.read/write` 闭包原样搬运）；
//! Tauri 事件（vault-unlocked/locked）改成 `AppState::lock_signal` 标志位，
//! 由 UI 每帧轮询；媒体流播管线（hmac token / vault:// 协议）随 WebView 一起废弃，
//! 视频改由 `media` 模块从内存解码播放（不落盘），音频仍走「导出到本地打开」。

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use zeroize::Zeroizing;

use eframe::egui;
use serde::Serialize;

use crate::backup::{self, BackupReport};
use crate::capture;
use crate::error::{Error, Result};
use crate::license::{self, LicenseView};
use crate::search;
use crate::state::AppState;
use crate::store::{classify, now_ts, Entry, Settings, Vault};
use crate::vault;

// ─────────────────────────── 数据视图（与原前端拿到的 JSON 同构） ───────────────────────────

#[derive(Serialize, Clone)]
pub struct Status {
    pub exists: bool,
    pub unlocked: bool,
    pub path: String,
    pub files: usize,
    pub bytes: u64,
    pub vault_bytes: u64,
}

#[derive(Serialize, Clone)]
pub struct EntryView {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub mime: String,
    pub size: u64,
    pub created_at: i64,
    pub modified_at: i64,
    pub version_count: usize,
    pub deleted_at: Option<i64>,
    pub preview: String,
}

#[derive(Serialize, Clone)]
pub struct TextPayload {
    pub content: String,
    pub size: u64,
    pub truncated: bool,
}

#[derive(Serialize, Clone)]
pub struct VersionView {
    pub id: String,
    pub size: u64,
    pub created_at: i64,
    pub pinned: bool,
    pub current: bool,
}

#[derive(Serialize, Default, Clone)]
pub struct ImportResult {
    pub imported: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
}

#[derive(Serialize)]
pub struct AppPrefs {
    pub capture_guard: bool,
}

#[derive(Serialize, Clone)]
pub struct SearchReport {
    pub docs: usize,
    pub truncated: bool,
    pub ms: u128,
    pub built: bool,
    pub built_at: i64,
}

#[derive(Serialize, Clone)]
pub struct Crumb {
    pub id: String,
    pub name: String,
}

#[derive(Serialize, Clone)]
pub struct Locate {
    pub entry: EntryView,
    pub trail: Vec<Crumb>,
}

// ─────────────────────────── 应用数据目录（与 Tauri 版落盘位置兼容） ───────────────────────────

pub const APP_ID: &str = "com.xiaozhou.sandbox";

pub fn app_config_dir() -> PathBuf {
    dirs::config_dir().unwrap_or_default().join(APP_ID)
}

pub fn app_local_data_dir() -> PathBuf {
    dirs::data_local_dir().unwrap_or_default().join(APP_ID)
}

// ─────────────────────────── 启动初始化与后台线程 ───────────────────────────

/// 读取 config.json 与 license.json。签名校验失败只当作未授权，不拦启动。
pub fn init(state: &AppState) {
    let dir = app_config_dir();
    let _ = std::fs::create_dir_all(&dir);
    let cfg_path = dir.join("config.json");
    let cfg: crate::state::Config = std::fs::read_to_string(&cfg_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    let mut lic = license::LicenseState::load(&dir);
    lic.set_trial_started(cfg.trial_started);

    *state.config.lock().unwrap() = cfg;
    *state.config_path.lock().unwrap() = cfg_path;
    *state.license.lock().unwrap() = lic;
}

/// 启动时是否应该打开反截屏（需要「用户没关开关」且「授权含 noscreen 能力」）。
/// 真正应用要等拿到窗口 HWND 之后，由 UI 在首帧执行。
pub fn startup_capture_wanted(state: &AppState) -> bool {
    let guard_on = state.config.lock().unwrap().capture_guard;
    guard_on && state.license.lock().unwrap().has("noscreen")
}

/// 空闲自动锁定线程：每 5 秒轮询，超时则锁库并通知 UI 重绘。
pub fn spawn_idle_locker(state: Arc<AppState>, ctx: egui::Context) {
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(5));
        if !state.is_unlocked() {
            continue;
        }
        let mins = state
            .read(|v| Ok(v.index.settings.auto_lock_minutes))
            .unwrap_or(5);
        if mins > 0 && state.idle_secs() > (mins as i64) * 60 {
            state.lock_now();
            state.lock_signal.store(true, Ordering::SeqCst);
            ctx.request_repaint();
        }
    });
}

// ─────────────────────────── 保险库生命周期 ───────────────────────────

fn status_of(state: &AppState) -> Status {
    let cfg = state.config.lock().unwrap().clone();
    let root = PathBuf::from(&cfg.vault_path);
    let unlocked = state.is_unlocked();
    let (files, bytes) = if unlocked {
        state.read(|v| Ok(v.stats())).unwrap_or((0, 0))
    } else {
        (0, 0)
    };
    let vault_bytes = if root.as_os_str().is_empty() || !root.exists() {
        0
    } else {
        backup::dir_size(&root)
    };
    Status {
        exists: vault::exists(&root),
        unlocked,
        path: cfg.vault_path,
        files,
        bytes,
        vault_bytes,
    }
}

pub fn vault_status(state: &AppState) -> Status {
    status_of(state)
}

/// 建库。`second` 是可选第二口令：填了就在同一目录里再开一套完全独立的内容，
/// 两个口令各自打开自己那一套（互相看不见）。
pub fn create_vault(
    state: &AppState,
    path: &str,
    password: &str,
    second: Option<&str>,
) -> Result<Status> {
    if password.chars().count() < 8 {
        return Err(Error::Msg("密码至少 8 个字符".into()));
    }
    if let Some(s) = second.map(str::trim).filter(|s| !s.is_empty()) {
        if s.chars().count() < 8 {
            return Err(Error::Msg("第二口令至少 8 个字符".into()));
        }
    }
    let root = if path.trim().is_empty() {
        // 默认放在 LocalAppData 而不是 Documents：
        // Documents 常被 OneDrive / 坚果云 同步，密文会被自动上云；
        // 也会被 Windows Search 索引，无谓地暴露文件数量与访问时间。
        app_local_data_dir().join("vault")
    } else {
        PathBuf::from(path.trim())
    };
    vault::create(&root, password, second)?;
    {
        let path = root.display().to_string();
        let mut cfg = state.config.lock().unwrap();
        cfg.vault_path = path.clone();
        remember_vault(&mut cfg, &path);
    }
    state.save_config()?;
    let unlocked = vault::unlock_full(&root, password)?;
    let v = Vault::open_named(root, unlocked.dek, &unlocked.index_file)?;
    *state.vault.lock().unwrap() = Some(v);
    state.touch();
    Ok(status_of(state))
}

/// 打开一个**已存在**的保险库：把配置指向该目录，之后走正常解锁流程。
///
/// 重装系统 / 换电脑 / 配置文件丢失时，这是找回旧数据的唯一入口——
/// 加密只依赖库目录里的 `vault.json` 和 `blobs/`，与程序本身无关。
pub fn open_vault_at(state: &AppState, path: &str) -> Result<Status> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(Error::Msg("请选择库所在的目录".into()));
    }
    let root = PathBuf::from(trimmed);
    if !vault::exists(&root) {
        return Err(Error::Msg(
            "这个目录里没有 vault.json，不像是小舟沙盒的库".into(),
        ));
    }
    {
        let mut cfg = state.config.lock().unwrap();
        cfg.vault_path = root.display().to_string();
        let p = cfg.vault_path.clone();
        remember_vault(&mut cfg, &p);
    }
    state.save_config()?;
    Ok(status_of(state))
}

pub fn unlock(state: &AppState, password: &str) -> Result<Status> {
    let cfg = state.config.lock().unwrap().clone();
    if cfg.vault_path.is_empty() {
        return Err(Error::Msg("还没有创建保险库".into()));
    }
    let root = PathBuf::from(&cfg.vault_path);
    let unlocked = vault::unlock_full(&root, password)?;
    let v = Vault::open_named(root, unlocked.dek, &unlocked.index_file)?;
    *state.vault.lock().unwrap() = Some(v);
    state.touch();
    Ok(status_of(state))
}

/// 记住最近打开过的库（登录页菜单栏用），最新的排最前，最多 8 个。
fn remember_vault(cfg: &mut crate::state::Config, path: &str) {
    if path.is_empty() {
        return;
    }
    cfg.recent_vaults.retain(|p| !p.eq_ignore_ascii_case(path));
    cfg.recent_vaults.insert(0, path.to_string());
    cfg.recent_vaults.truncate(8);
}

pub fn lock(state: &AppState) {
    state.lock_now();
}

#[allow(dead_code)] // UI 直接调 state.touch()，接口保留对齐
pub fn touch(state: &AppState) {
    state.touch();
}

// ─────────────────────────── 列表与条目 ───────────────────────────

fn to_view(e: &Entry) -> EntryView {
    EntryView {
        id: e.id.clone(),
        name: e.name.clone(),
        kind: e.kind.clone(),
        mime: e.mime.clone(),
        size: e.size,
        created_at: e.created_at,
        modified_at: e.modified_at,
        version_count: e.versions.len(),
        deleted_at: e.deleted_at,
        preview: if e.kind == "dir" {
            "dir".into()
        } else {
            classify(&e.mime, &e.name).to_string()
        },
    }
}

pub fn list_dir(state: &AppState, parent: &str) -> Result<Vec<EntryView>> {
    state.read(|v| Ok(v.children(parent, false).iter().map(to_view).collect()))
}

pub fn list_trash(state: &AppState) -> Result<Vec<EntryView>> {
    state.read(|v| {
        let mut all: Vec<Entry> = v
            .index
            .entries
            .values()
            .filter(|e| e.deleted_at.is_some())
            .cloned()
            .collect();
        all.sort_by_key(|e| std::cmp::Reverse(e.deleted_at.unwrap_or(0)));
        Ok(all.iter().map(to_view).collect())
    })
}

pub fn list_by_kind(state: &AppState, kind: &str) -> Result<Vec<EntryView>> {
    state.read(|v| {
        let mut all: Vec<Entry> = v
            .index
            .entries
            .values()
            .filter(|e| e.kind == "file" && e.deleted_at.is_none())
            .filter(|e| classify(&e.mime, &e.name) == kind)
            .cloned()
            .collect();
        all.sort_by_key(|e| std::cmp::Reverse(e.modified_at));
        Ok(all.iter().map(to_view).collect())
    })
}

fn unique_name(v: &Vault, parent: &str, name: &str) -> String {
    let taken: Vec<&str> = v
        .index
        .entries
        .values()
        .filter(|e| e.parent == parent && e.deleted_at.is_none())
        .map(|e| e.name.as_str())
        .collect();
    if !taken.contains(&name) {
        return name.to_string();
    }
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    for n in 2..5000 {
        let cand = format!("{} ({}){}", stem, n, ext);
        if !taken.contains(&cand.as_str()) {
            return cand;
        }
    }
    format!("{}-{}", stem, now_ts())
}

fn import_into(v: &mut Vault, src: &Path, parent: &str, skip: &Path, out: &mut ImportResult) {
    let canon = std::fs::canonicalize(src).unwrap_or_else(|_| src.to_path_buf());
    if canon.starts_with(skip) {
        out.skipped += 1;
        return;
    }
    let raw_name = src
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "未命名".into());

    if src.is_dir() {
        let name = unique_name(v, parent, &raw_name);
        match v.create_folder(parent, &name) {
            Ok(dir) => {
                out.imported += 1;
                if let Ok(rd) = std::fs::read_dir(src) {
                    for e in rd.flatten() {
                        import_into(v, &e.path(), &dir.id, skip, out);
                    }
                }
            }
            Err(e) => out.errors.push(format!("{}: {}", name, e)),
        }
    } else {
        let name = unique_name(v, parent, &raw_name);
        match v.import_file(parent, &name, src) {
            Ok(_) => out.imported += 1,
            Err(e) => out.errors.push(format!("{}: {}", name, e)),
        }
    }
}

pub fn import_paths(state: &AppState, paths: &[String], parent: &str) -> Result<ImportResult> {
    state.write(|v| {
        let skip = std::fs::canonicalize(&v.root).unwrap_or_else(|_| v.root.clone());
        let mut out = ImportResult::default();
        for p in paths {
            import_into(v, Path::new(p), parent, &skip, &mut out);
        }
        v.save_index()?;
        Ok(out)
    })
}

pub fn create_folder(state: &AppState, parent: &str, name: &str) -> Result<EntryView> {
    state.write(|v| {
        let n = unique_name(v, parent, name.trim());
        let e = v.create_folder(parent, &n)?;
        v.save_index()?;
        Ok(to_view(&e))
    })
}

pub fn rename_entry(state: &AppState, id: &str, name: &str) -> Result<EntryView> {
    state.write(|v| {
        let parent = v.index.entries.get(id).ok_or(Error::NotFound)?.parent.clone();
        let n = unique_name(v, &parent, name.trim());
        let e = v.index.entries.get_mut(id).ok_or(Error::NotFound)?;
        e.name = n;
        e.modified_at = now_ts();
        let out = e.clone();
        v.save_index()?;
        Ok(to_view(&out))
    })
}

// ─────────────────────────── 内容读写 ───────────────────────────

pub fn read_text(state: &AppState, id: &str) -> Result<TextPayload> {
    const LIMIT: u64 = 4 * 1024 * 1024;
    state.read(|v| {
        let e = v.index.entries.get(id).ok_or(Error::NotFound)?;
        let ver = e.current_version().ok_or(Error::NotFound)?;
        let size = ver.size;
        let data = if size > LIMIT {
            v.read_blob_range(&e.id, &ver.blob, 0, LIMIT)?
        } else {
            v.read_blob(&e.id, &ver.blob)?
        };
        Ok(TextPayload {
            content: String::from_utf8_lossy(&data[..]).to_string(),
            size,
            truncated: size > LIMIT,
        })
    })
}

pub fn save_text(state: &AppState, id: &str, content: &str, allow_truncate: bool) -> Result<EntryView> {
    state.write(|v| {
        const LIMIT: u64 = 4 * 1024 * 1024;
        // 借用必须在可变借用之前结束
        let cur_size = {
            let e = v.index.entries.get(id).ok_or(Error::NotFound)?;
            e.current_version().ok_or(Error::NotFound)?.size
        };
        if cur_size > LIMIT && !allow_truncate {
            return Err(Error::Msg(
                "原文件超过 4 MB，编辑器只加载了前 4 MB。已阻止覆盖保存，请用「导出」另存后编辑。"
                    .into(),
            ));
        }
        let e = v.save_text(id, content)?;
        Ok(to_view(&e))
    })
}

/// 解密图片，返回 (mime, 原始字节)。UI 用 `image` crate 解码后上纹理。
pub fn read_image(state: &AppState, id: &str) -> Result<(String, Vec<u8>)> {
    const LIMIT: u64 = 24 * 1024 * 1024;
    state.read(|v| {
        let e = v.index.entries.get(id).ok_or(Error::NotFound)?;
        let ver = e.current_version().ok_or(Error::NotFound)?;
        if ver.size > LIMIT {
            return Err(Error::Msg("图片太大，超过 24 MB".into()));
        }
        let data = v.read_blob(&e.id, &ver.blob)?;
        let mime = if e.mime.is_empty() {
            "image/png".to_string()
        } else {
            e.mime.clone()
        };
        Ok((mime, data.to_vec()))
    })
}

/// 打开视频/音频的按需解密数据源。
///
/// 不再把整个文件解密进内存：解码器读到哪，就从保险库解出那一段（4 MB 一个窗口），
/// 内存占用与文件大小无关，几个 GB 的视频也能直接播。
pub fn open_media(state: &AppState, id: &str) -> Result<(String, Arc<dyn crate::media::Source>)> {
    state.read(|v| {
        let e = v.index.entries.get(id).ok_or(Error::NotFound)?;
        let ver = e.current_version().ok_or(Error::NotFound)?;
        let src = VaultSource {
            dek: Zeroizing::new(*v.dek),
            path: v.blob_path(&ver.blob),
            entry_id: e.id.clone(),
            size: ver.size,
            cache: Mutex::new(Cache::default()),
        };
        Ok((e.mime.clone(), Arc::new(src) as Arc<dyn crate::media::Source>))
    })
}

/// 解密窗口大小：每次换窗都要重开一次密文文件并解 1 MB 的块，太小会频繁 IO，
/// 太大浪费内存。4 MB 足够覆盖 Media Foundation 的常规读取量。
const MEDIA_WINDOW: u64 = 4 << 20;

/// 从保险库按偏移解密的只读数据源。明文只在这个窗口里存在。
struct VaultSource {
    dek: Zeroizing<[u8; 32]>,
    path: PathBuf,
    entry_id: String,
    size: u64,
    cache: Mutex<Cache>,
}

#[derive(Default)]
struct Cache {
    start: u64,
    data: Zeroizing<Vec<u8>>,
}

impl crate::media::Source for VaultSource {
    fn size(&self) -> u64 {
        self.size
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> std::result::Result<usize, String> {
        if buf.is_empty() || offset >= self.size {
            return Ok(0);
        }
        let want = buf.len().min((self.size - offset) as usize);
        let mut done = 0usize;
        while done < want {
            let pos = offset + done as u64;
            let mut c = self.cache.lock().unwrap_or_else(|e| e.into_inner());
            let wstart = pos / MEDIA_WINDOW * MEDIA_WINDOW;
            if c.start != wstart || c.data.is_empty() {
                let wend = (wstart + MEDIA_WINDOW).min(self.size);
                let data =
                    crate::blob::decrypt_range(&self.dek[..], &self.path, &self.entry_id, wstart, wend)
                        .map_err(|e| e.to_string())?;
                *c = Cache { start: wstart, data };
            }
            let lo = (pos - c.start) as usize;
            if lo >= c.data.len() {
                break;
            }
            let n = (c.data.len() - lo).min(want - done);
            buf[done..done + n].copy_from_slice(&c.data[lo..lo + n]);
            done += n;
        }
        Ok(done)
    }
}

pub fn export_entry(state: &AppState, id: &str, dest: &str) -> Result<()> {
    use std::io::Write;
    state.read(|v| {
        let e = v.index.entries.get(id).ok_or(Error::NotFound)?;
        if e.kind != "file" {
            return Err(Error::Msg("暂不支持导出文件夹".into()));
        }
        let ver = e.current_version().ok_or(Error::NotFound)?;
        let total = ver.size;
        let mut out = std::fs::File::create(dest)?;
        let step = 4 * 1024 * 1024u64;
        let mut pos = 0u64;
        while pos < total {
            let end = (pos + step).min(total);
            let chunk = v.read_blob_range(&e.id, &ver.blob, pos, end)?;
            out.write_all(&chunk[..])?;
            pos = end;
        }
        out.sync_all()?;
        Ok(())
    })
}

pub fn delete_entry(state: &AppState, id: &str) -> Result<()> {
    state.write(|v| v.delete_entry(id))
}

pub fn restore_entry(state: &AppState, id: &str) -> Result<()> {
    state.write(|v| v.restore_entry(id))
}

pub fn purge_entry(state: &AppState, id: &str) -> Result<()> {
    state.write(|v| v.purge_entry(id))
}

// ─────────────────────────── 版本历史 ───────────────────────────

pub fn list_versions(state: &AppState, id: &str) -> Result<Vec<VersionView>> {
    state.require("versions")?;
    state.read(|v| {
        let e = v.index.entries.get(id).ok_or(Error::NotFound)?;
        let mut list: Vec<VersionView> = e
            .versions
            .iter()
            .map(|ver| VersionView {
                id: ver.id.clone(),
                size: ver.size,
                created_at: ver.created_at,
                pinned: ver.pinned,
                current: e.current.as_deref() == Some(ver.id.as_str()),
            })
            .collect();
        list.sort_by_key(|x| std::cmp::Reverse(x.created_at));
        Ok(list)
    })
}

pub fn restore_version(state: &AppState, id: &str, version_id: &str) -> Result<EntryView> {
    state.require("versions")?;
    state.write(|v| {
        let e = v.restore_version(id, version_id)?;
        Ok(to_view(&e))
    })
}

pub fn pin_version(state: &AppState, id: &str, version_id: &str, pinned: bool) -> Result<()> {
    state.require("versions")?;
    state.write(|v| v.pin_version(id, version_id, pinned))
}

// ─────────────────────────── 设置与备份 ───────────────────────────

pub fn get_settings(state: &AppState) -> Result<Settings> {
    state.read(|v| Ok(v.index.settings.clone()))
}

pub fn update_settings(state: &AppState, settings: Settings) -> Result<Settings> {
    state.write(|v| {
        v.index.settings = settings;
        v.save_index()?;
        Ok(v.index.settings.clone())
    })
}

pub fn backup_now(state: &AppState) -> Result<BackupReport> {
    // 注意：手动备份**不设门控**。红线之一是「备份（手动单目标）永远免费」——
    // 用户必须永远有办法把自己的密文复制出去。门控只覆盖「定时 + 多目标」，
    // 那部分由调度器控制，不在这个入口里。
    let (root, cfg) = state.read(|v| Ok((v.root.clone(), v.index.settings.backup.clone())))?;
    let rep = backup::run(&root, &cfg)?;
    state.write(|v| {
        v.index.settings.backup.last_backup_at = rep.at;
        v.index.settings.backup.last_result = rep.message.clone();
        v.save_index()
    })?;
    Ok(rep)
}

pub fn change_password(state: &AppState, old: &str, new: &str) -> Result<()> {
    if new.chars().count() < 8 {
        return Err(Error::Msg("新密码至少 8 个字符".into()));
    }
    let root = state.read(|v| Ok(v.root.clone()))?;
    vault::change_password(&root, old, new)
}

// ─────────────────────────── 授权 ───────────────────────────

fn config_dir(state: &AppState) -> PathBuf {
    state
        .config_path
        .lock()
        .unwrap()
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_default()
}

/// 把「反截屏」应用到主窗口。返回是否真的生效（旧系统上会失败）。
pub fn apply_capture_guard(hwnd: isize, on: bool) -> bool {
    #[cfg(windows)]
    {
        capture::set_excluded(hwnd, on)
    }
    #[cfg(not(windows))]
    {
        let _ = (hwnd, on);
        false
    }
}

#[allow(dead_code)] // 授权 UI 尚未开放，接口保留
pub fn license_state(state: &AppState) -> LicenseView {
    state.license.lock().unwrap().view()
}

/// 应用级偏好。与保险库设置分开：这些是「这台机器怎么跑」的开关，
/// 不需要解锁也能读。
pub fn app_prefs(state: &AppState) -> AppPrefs {
    AppPrefs {
        capture_guard: state.config.lock().unwrap().capture_guard,
    }
}

#[allow(dead_code)] // 授权 UI 尚未开放，接口保留（后端能力与 Tauri 版对齐）
pub fn activate_license(state: &AppState, raw: &str) -> Result<LicenseView> {
    let dir = config_dir(state);
    let text = raw.trim().to_string();
    if text.is_empty() {
        return Err(Error::Msg("请粘贴授权文件的内容".into()));
    }
    let view = {
        let mut lic = state.license.lock().unwrap();
        lic.activate(&text)?
    };
    license::save(&dir, &text)?;

    // 刚激活的授权如果含反截屏能力，且用户没关掉这个开关，立刻生效。
    let guard_on = state.config.lock().unwrap().capture_guard;
    let has_noscreen = state.license.lock().unwrap().has("noscreen");
    if guard_on && has_noscreen {
        state.capture_apply_requested.store(true, Ordering::SeqCst);
    }
    Ok(view)
}

#[allow(dead_code)] // 授权 UI 尚未开放，接口保留
pub fn start_trial(state: &AppState) -> Result<LicenseView> {
    let started = {
        let mut cfg = state.config.lock().unwrap();
        if cfg.trial_started == 0 {
            cfg.trial_started = now_ts();
        }
        cfg.trial_started
    };
    state.save_config()?;
    {
        let mut lic = state.license.lock().unwrap();
        lic.set_trial_started(started);
    }

    let guard_on = state.config.lock().unwrap().capture_guard;
    if guard_on {
        state.capture_apply_requested.store(true, Ordering::SeqCst);
    }
    Ok(state.license.lock().unwrap().view())
}

#[allow(dead_code)] // 授权 UI 尚未开放，接口保留
pub fn remove_license(state: &AppState) -> Result<LicenseView> {
    state.license.lock().unwrap().clear();
    let dir = config_dir(state);
    let _ = std::fs::remove_file(license::license_path(&dir));
    // 授权没了，反截屏也随之关闭，避免「功能已失效但窗口还是黑的」这种困惑。
    if !state.license.lock().unwrap().has("noscreen") {
        state.capture_apply_requested.store(true, Ordering::SeqCst);
    }
    Ok(state.license.lock().unwrap().view())
}

pub fn set_capture_guard(state: &AppState, hwnd: isize, enabled: bool) -> Result<bool> {
    if enabled {
        state.require("noscreen")?;
    }
    {
        let mut cfg = state.config.lock().unwrap();
        cfg.capture_guard = enabled;
    }
    state.save_config()?;
    Ok(apply_capture_guard(hwnd, enabled))
}

// ─────────────────────────── 全文搜索 ───────────────────────────

fn search_report_of(idx: Option<&search::SearchIndex>) -> SearchReport {
    match idx {
        Some(i) => SearchReport {
            docs: i.docs,
            truncated: i.truncated,
            ms: 0,
            built: true,
            built_at: i.built_at,
        },
        None => SearchReport {
            docs: 0,
            truncated: false,
            ms: 0,
            built: false,
            built_at: 0,
        },
    }
}

pub fn search_status(state: &AppState) -> SearchReport {
    let guard = state.search.lock().unwrap();
    search_report_of(guard.as_ref())
}

/// 后台建索引：解密几十 MB 不能冻 UI。完成或失败都通过 repaint 通知 UI。
pub fn search_build(state: &Arc<AppState>, ctx: egui::Context) -> Result<()> {
    state.require("search")?;
    // 先确认库还开着，给同步的错误反馈
    state.read(|_| Ok(()))?;
    if state.search_building.swap(true, Ordering::SeqCst) {
        return Ok(()); // 已经在建了
    }
    let st = Arc::clone(state);
    std::thread::spawn(move || {
        let t0 = std::time::Instant::now();
        let built = st.read(|v| Ok(search::build(v)));
        match built {
            Ok(idx) => {
                let rep = SearchReport {
                    docs: idx.docs,
                    truncated: idx.truncated,
                    ms: t0.elapsed().as_millis(),
                    built: true,
                    built_at: idx.built_at,
                };
                *st.search.lock().unwrap() = Some(idx);
                *st.last_search_report.lock().unwrap() = Some(rep);
            }
            Err(e) => {
                *st.last_error.lock().unwrap() = Some(e.to_string());
            }
        }
        st.search_building.store(false, Ordering::SeqCst);
        ctx.request_repaint();
    });
    Ok(())
}

pub fn search_query(state: &AppState, q: &str, limit: Option<usize>) -> Result<Vec<search::Hit>> {
    state.require("search")?;
    if q.trim().is_empty() {
        return Ok(Vec::new());
    }
    let guard = state.search.lock().unwrap();
    let idx = guard
        .as_ref()
        .ok_or_else(|| Error::Msg("搜索索引还没建好，请先点「建立索引」".into()))?;
    Ok(idx.query(q, limit.unwrap_or(60).clamp(1, 200)))
}

// ─────────────────────────── 定位 ───────────────────────────

/// 按 id 定位一条记录，并返回它到根目录的完整路径。
/// 全文搜索的命中项可能在任何子目录里，UI 需要这个才能「跳过去并选中」。
pub fn locate_entry(state: &AppState, id: &str) -> Result<Locate> {
    state.read(|v| {
        let e = v.index.entries.get(id).ok_or(Error::NotFound)?;
        let mut trail: Vec<Crumb> = Vec::new();
        let mut cur = e.parent.clone();
        // 防御环形父子关系：最多向上走 64 层
        for _ in 0..64 {
            if cur == "root" || cur.is_empty() {
                break;
            }
            let p = match v.index.entries.get(&cur) {
                Some(p) => p,
                None => break,
            };
            trail.push(Crumb {
                id: p.id.clone(),
                name: p.name.clone(),
            });
            cur = p.parent.clone();
        }
        trail.reverse();
        Ok(Locate {
            entry: to_view(e),
            trail,
        })
    })
}

// ─────────────────────── 隐私输入的 IME 解绑 ───────────────────────

/// 进入隐私输入时解绑系统输入法，退出时恢复。
///
/// **失败不阻断**：UI 层（焦点不在可编辑控件上）本身就不触发输入法，
/// 这里只是加固。加固失败就让用户打不了字，那是本末倒置。
pub fn set_ime_isolated(hwnd: isize, on: bool) -> bool {
    if let Err(e) = crate::ime::set_isolated(hwnd, on) {
        eprintln!("[ime] 解绑系统输入法失败：{e}");
        return false;
    }
    true
}
