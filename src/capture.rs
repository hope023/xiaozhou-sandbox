//! 反截屏。
//!
//! `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` 从 Win10 2004 起生效，
//! 让窗口在截图、录屏、以及 Windows Recall 的截屏里变成空白。
//!
//! 为什么手写 FFI 而不引 `windows` crate：整个调用只有一个函数、一个常量，
//! 而 `windows` 会带来数十个 crate 与明显的编译时间开销。
//! 这样写还有一个好处——它完全属于本项目的自有代码，不引入任何新的许可义务。

/// 让窗口对截图隐形。返回是否设置成功。
#[cfg(windows)]
pub fn set_excluded(hwnd: isize, on: bool) -> bool {
    const WDA_NONE: u32 = 0x0000_0000;
    const WDA_EXCLUDEFROMCAPTURE: u32 = 0x0000_0011;

    extern "system" {
        fn SetWindowDisplayAffinity(hwnd: isize, dwaffinity: u32) -> i32;
    }

    if hwnd == 0 {
        return false;
    }
    let val = if on { WDA_EXCLUDEFROMCAPTURE } else { WDA_NONE };
    unsafe { SetWindowDisplayAffinity(hwnd, val) != 0 }
}

#[cfg(not(windows))]
pub fn set_excluded(_hwnd: isize, _on: bool) -> bool {
    false
}
