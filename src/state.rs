use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::license::LicenseState;
use crate::search::SearchIndex;
use crate::store::{now_ts, Vault};

fn default_true() -> bool {
    true
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    pub vault_path: String,
    /// 最近打开过的库目录（登录页菜单栏用），最新的在前。
    #[serde(default)]
    pub recent_vaults: Vec<String>,
    /// 试用开始的 Unix 时间戳，0 表示从未开始试用。放在这里而不是保险库里，
    /// 是为了「先试用、后建库」也能正常工作。
    #[serde(default)]
    pub trial_started: i64,
    /// 反截屏开关。默认开启——这是隐私类软件的应有默认。
    #[serde(default = "default_true")]
    pub capture_guard: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            vault_path: String::new(),
            recent_vaults: Vec::new(),
            trial_started: 0,
            capture_guard: true,
        }
    }
}

pub struct AppState {
    pub vault: Mutex<Option<Vault>>,
    pub config: Mutex<Config>,
    pub config_path: Mutex<PathBuf>,
    pub last_activity: AtomicI64,
    /// 授权状态。启动时从 `license.json` 读入，之后只由 `activate_license` 改写。
    pub license: Mutex<LicenseState>,
    /// 全文搜索索引。内存明文，锁库即清空。
    pub search: Mutex<Option<SearchIndex>>,
    /// 空闲自动锁定线程把「刚才锁库了」通知 UI 用的信号。
    /// UI 每帧检查，读到 true 后清空选中、刷新状态并弹提示。
    pub lock_signal: AtomicBool,
    /// 后台线程正在建全文索引（建索引耗时，UI 显示进度状态用）。
    pub search_building: AtomicBool,
    /// 后台建索引线程写给 UI 的结果报告，UI 读走后置回 None。
    pub last_search_report: Mutex<Option<crate::service::SearchReport>>,
    /// 后台线程写给 UI 的错误信息（如建索引失败），UI 读走后置回 None。
    pub last_error: Mutex<Option<String>>,
    /// 「请把反截屏应用到窗口」的请求标志。由授权/设置路径发起，
    /// UI 在拿到 HWND 后执行并清零（HWND 只有 UI 层拿得到）。
    pub capture_apply_requested: AtomicBool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            vault: Mutex::new(None),
            config: Mutex::new(Config::default()),
            config_path: Mutex::new(PathBuf::new()),
            last_activity: AtomicI64::new(now_ts()),
            license: Mutex::new(LicenseState::default()),
            search: Mutex::new(None),
            lock_signal: AtomicBool::new(false),
            search_building: AtomicBool::new(false),
            last_search_report: Mutex::new(None),
            last_error: Mutex::new(None),
            capture_apply_requested: AtomicBool::new(false),
        }
    }
}

impl AppState {
    pub fn touch(&self) {
        self.last_activity.store(now_ts(), Ordering::SeqCst);
    }

    pub fn idle_secs(&self) -> i64 {
        now_ts() - self.last_activity.load(Ordering::SeqCst)
    }

    pub fn is_unlocked(&self) -> bool {
        self.vault.lock().unwrap().is_some()
    }

    /// 能力门控。位置必须在 Rust 侧——放在前端等于没有，
    /// 改个变量就能绕过。
    ///
    /// 当前阶段 [`crate::license::GATING_ENABLED`] 为 `false`，所以这里恒为 `Ok`。
    /// 命令里的调用点**刻意保留**：将来真需要区分能力时打开总开关，拦截立刻生效，不用回来补。
    pub fn require(&self, feature: &str) -> Result<()> {
        if self.license.lock().unwrap().has(feature) {
            Ok(())
        } else {
            Err(Error::Msg(format!(
                "「{}」在当前版本不可用",
                crate::license::feature_label(feature),
            )))
        }
    }

    pub fn read<T>(&self, f: impl FnOnce(&Vault) -> Result<T>) -> Result<T> {
        let guard = self.vault.lock().unwrap();
        let v = guard.as_ref().ok_or(Error::Locked)?;
        f(v)
    }

    pub fn write<T>(&self, f: impl FnOnce(&mut Vault) -> Result<T>) -> Result<T> {
        let mut guard = self.vault.lock().unwrap();
        let v = guard.as_mut().ok_or(Error::Locked)?;
        f(v)
    }

    pub fn lock_now(&self) {
        *self.vault.lock().unwrap() = None;
        // 搜索索引是明文摘要，必须跟着一起丢。
        *self.search.lock().unwrap() = None;
    }

    pub fn save_config(&self) -> Result<()> {
        let cfg = self.config.lock().unwrap().clone();
        let path = self.config_path.lock().unwrap().clone();
        if path.as_os_str().is_empty() {
            return Ok(());
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, serde_json::to_vec_pretty(&cfg)?)?;
        Ok(())
    }
}
