//! Win32 小工具：单实例互斥锁。
//! 与 `capture.rs` / `ime.rs` 同一哲学：只差一两个常量，不值得为它引 `windows` crate，
//! 手写 `extern "system"` 即可。

/// 尝试持有命名互斥锁。返回 false 表示已有实例在跑，本进程应立即退出。
/// 互斥锁句柄刻意不关闭——进程活着它就得活着。
pub fn acquire_single_instance(name: &str) -> bool {
    #[cfg(windows)]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;

        #[link(name = "kernel32")]
        extern "system" {
            fn CreateMutexW(
                lp_mutex_attributes: *mut std::ffi::c_void,
                b_initial_owner: i32,
                lp_name: *const u16,
            ) -> *mut std::ffi::c_void;
            fn GetLastError() -> u32;
        }
        const ERROR_ALREADY_EXISTS: u32 = 183;

        fn wide(s: &str) -> Vec<u16> {
            OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
        }
        // Local\ 是会话命名空间，不需要管理员权限。
        let full = wide(&format!(r"Local\{}", name));
        let handle = unsafe { CreateMutexW(std::ptr::null_mut(), 0, full.as_ptr()) };
        if handle.is_null() {
            return true; // 创建失败别把用户锁在门外，放行
        }
        unsafe { GetLastError() != ERROR_ALREADY_EXISTS }
    }
    #[cfg(not(windows))]
    {
        let _ = name;
        true
    }
}
