//! Windows 输入法（IME）压制。
//!
//! 隐私输入的核心防线其实是 UI 层：`PrivacyInput` 组件直接消费原始键盘事件，
//! 焦点不落在任何可编辑控件上，TSF 就不会激活输入法。这里是**双保险**，
//! 而且是两道不同的锁：
//!
//! 1. **英文直通**：`ImmSetOpenStatus(FALSE)` 把输入法切到「关闭」状态，
//!    等效于用户按了 Shift 切英文。微软拼音等 TSF 输入法对这一招买账——
//!    即使它的候选窗还挂着，关闭状态下按键也会以 WM_CHAR 直达应用。
//! 2. **上下文解绑**：`ImmAssociateContextEx(hwnd, NULL, 0)` 把 IMM32 上下文
//!    从窗口解绑，退出时用 `IACE_DEFAULT` 要求系统重建。
//!
//! 为什么两道都要：窗口重新获得焦点时，系统会发 `WM_IME_SETCONTEXT` 并把
//! 输入法重新挂回来，所以 UI 层要在进入隐私模式、窗口重获焦点时反复施加，
//! 隐私模式期间还要每帧保持（见 `keep_ime_off`，只是两次 FFI 调用，很便宜）。
//!
//! 为什么不用 `windows` crate：整件事只有几个常量和函数，引那个 crate
//! 会带进几十个依赖和明显的编译时间，不划算。手写 `extern "system"` 就够了。
//!
//! 失败时的行为：返回错误，但 UI 层**不阻断**隐私输入（前端那层已生效），
//! 只是把结果亮在界面上提示用户手动切英文。

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicU8, Ordering};

    type Handle = *mut c_void;

    #[link(name = "imm32")]
    extern "system" {
        /// 关联/解绑窗口的 IME 上下文。`flags` 传 0 表示解绑。
        fn ImmAssociateContextEx(hwnd: Handle, himc: Handle, flags: u32) -> i32;
        fn ImmGetContext(hwnd: Handle) -> Handle;
        fn ImmReleaseContext(hwnd: Handle, himc: Handle) -> i32;
        fn ImmGetOpenStatus(himc: Handle) -> i32;
        fn ImmSetOpenStatus(himc: Handle, open: i32) -> i32;
    }

    #[link(name = "user32")]
    extern "system" {
        fn GetWindow(hwnd: Handle, cmd: u32) -> Handle;
    }

    const GW_CHILD: u32 = 5;
    const GW_HWNDNEXT: u32 = 2;
    /// 让系统为窗口重建一个默认 IME 上下文——这是"恢复"最省事也最可靠的方式，
    /// 比保存旧 HIMC 再还回去稳（子窗口可能在我们解绑后被重建，旧句柄就失效了）。
    const IACE_DEFAULT: u32 = 0x0010;

    /// 进入隐私输入前系统输入法的开关状态：0=未知 1=原本开启 2=原本关闭。
    /// 退出隐私输入时按它恢复，不替用户做「恢复后该不该开输入法」的决定。
    pub static PREV_OPEN: AtomicU8 = AtomicU8::new(0);

    /// 对窗口本身 + 它的同级子窗口逐个执行。egui/glow 只有一个顶层窗口，
    /// 保留子窗口遍历是为了万一以后出现多层窗口结构时依然正确。
    unsafe fn each(h: Handle, mut f: impl FnMut(Handle)) {
        if h.is_null() {
            return;
        }
        f(h);
        let mut c = GetWindow(h, GW_CHILD);
        let mut guard = 0;
        while !c.is_null() && guard < 32 {
            f(c);
            c = GetWindow(c, GW_HWNDNEXT);
            guard += 1;
        }
    }

    /// 返回处理的窗口数；0 表示系统没接受请求。
    pub unsafe fn apply(h: Handle, detach: bool) -> usize {
        let flags = if detach { 0 } else { IACE_DEFAULT };
        let mut n = 0usize;
        each(h, |w| {
            if ImmAssociateContextEx(w, std::ptr::null_mut(), flags) != 0 {
                n += 1;
            }
        });
        n
    }

    /// 当前输入法开关状态。窗口没有 IME 上下文（已解绑）时返回 None。
    pub unsafe fn open_status(h: Handle) -> Option<bool> {
        let himc = ImmGetContext(h);
        if himc.is_null() {
            return None;
        }
        let st = ImmGetOpenStatus(himc) != 0;
        ImmReleaseContext(h, himc);
        Some(st)
    }

    /// 把输入法切到「关闭（英文直通）」或重新打开。窗口无上下文时返回 false。
    pub unsafe fn set_open_status(h: Handle, open: bool) -> bool {
        let himc = ImmGetContext(h);
        if himc.is_null() {
            return false;
        }
        let ok = ImmSetOpenStatus(himc, open as i32) != 0;
        ImmReleaseContext(h, himc);
        ok
    }
}

/// 隐私模式期间每帧调用：把输入法摁在「关闭」状态。
/// 用户若手动按 Shift 重新打开输入法，下一帧会被摁回去——
/// 这正是隐私输入的契约：中文绝不经过系统输入法。
pub fn keep_ime_off(hwnd: isize) {
    #[cfg(windows)]
    {
        if hwnd == 0 {
            return;
        }
        let ptr = hwnd as *mut std::ffi::c_void;
        unsafe {
            if imp::open_status(ptr) == Some(true) {
                let _ = imp::set_open_status(ptr, false);
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = hwnd;
    }
}

/// 打开/关闭隐私输入时调用。`on = true` 压制输入法，`false` 恢复系统默认。
/// `hwnd` 为主窗口句柄（egui 侧从 `Frame::window_handle` 取得）。
pub fn set_isolated(hwnd: isize, on: bool) -> crate::error::Result<()> {
    #[cfg(windows)]
    {
        use std::sync::atomic::Ordering;
        if hwnd == 0 {
            return Err(crate::error::Error::Msg("取不到窗口句柄".into()));
        }
        let ptr = hwnd as *mut std::ffi::c_void;
        let mut any_ok = false;

        unsafe {
            if on {
                // 第一道锁：先把输入法切到英文直通（对 TSF 输入法最有效），
                // 并记住原本的开关状态，退出时好还原。
                match imp::open_status(ptr) {
                    Some(was_open) => {
                        imp::PREV_OPEN.store(if was_open { 1 } else { 2 }, Ordering::SeqCst);
                        if was_open && imp::set_open_status(ptr, false) {
                            any_ok = true;
                        }
                    }
                    None => {}
                }
                // 第二道锁：解绑 IMM32 上下文
                if imp::apply(ptr, true) > 0 {
                    any_ok = true;
                }
            } else {
                // 恢复：重建默认上下文 + 还原本来的开关状态
                if imp::apply(ptr, false) > 0 {
                    any_ok = true;
                }
                if imp::PREV_OPEN.swap(0, Ordering::SeqCst) == 1 {
                    let _ = imp::set_open_status(ptr, true);
                }
            }
        }

        if !any_ok {
            return Err(crate::error::Error::Msg(
                "系统没有接受输入法压制请求".into(),
            ));
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = (hwnd, on);
        Err(crate::error::Error::Msg(
            "当前系统不支持解绑输入法；隐私输入仍可使用（由组件保证按键不进入输入法）".into(),
        ))
    }
}
