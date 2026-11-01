//! 小舟沙盒 —— 本地加密保险库（egui 桌面版）。
//!
//! 文件、文件名、目录结构、设置全部加密后存本地磁盘；打开文件只在内存里解密。
//! 原 Tauri 2 + React 架构，2026-10 改造为 eframe/egui 纯 Rust 单 exe。
//!
//! 模块布局：
//! - 核心库（自 `src-tauri/src` 原样平移，零 Tauri 依赖）：
//!   `crypto error vault blob store backup search license state capture ime`
//! - `service`：原 38 个 Tauri 命令改为普通函数，UI 层直接调用
//! - `app`：eframe 界面（锁屏/侧栏/文件列表/检查器/面板/隐私输入）

// release 版用 Windows 图形子系统，双击时不弹黑色控制台窗口；
// debug 版保留控制台，方便直接看日志。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod backup;
mod blob;
mod capture;
mod crypto;
mod error;
mod ime;
mod license;
mod media;
mod search;
mod service;
mod state;
mod store;
mod vault;
mod win32;

fn main() -> eframe::Result {
    install_panic_logger();
    app::run()
}

/// GUI 程序双击启动时没有控制台，panic 会无声无息地消失。
/// 把它连同调用栈写进日志，出问题才有得查。
fn install_panic_logger() {
    let mut log_path = service::app_local_data_dir();
    log_path.push("panic.log");
    if log_path.parent().is_some_and(|p| p.as_os_str().is_empty()) {
        // dirs 失败的兜底：写到临时目录
        log_path = std::env::temp_dir().join("xiaozhou-panic.log");
    }
    std::panic::set_hook(Box::new(move |info| {
        use std::io::Write;
        let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        if let Ok(mut f) =
            std::fs::OpenOptions::new().create(true).append(true).open(&log_path)
        {
            let _ = writeln!(
                f,
                "\n==== {ts} ====\n{info}\n--- backtrace ---\n{}",
                std::backtrace::Backtrace::force_capture()
            );
        }
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::BackupSettings;

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("xvault-{}-{}", name, uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// 手动跑：`cargo test fixture_test_vault -- --ignored`
    /// 在 D:\software\temp\xv-test 造一个带两个文件的测试库，供 UI 自动化测试用。
    #[test]
    #[ignore]
    fn fixture_test_vault() {
        let dir = std::path::PathBuf::from(r"D:\software\temp\xv-test");
        let _ = std::fs::remove_dir_all(&dir);
        vault::create(&dir, "test12345678", None).unwrap();
        let dek = vault::unlock(&dir, "test12345678").unwrap();
        let mut v = store::Vault::open(dir.clone(), dek).unwrap();
        let a = dir.join("a.txt");
        std::fs::write(&a, "你好，这是测试文本内容 hello world").unwrap();
        v.import_file("root", "a.txt", &a).unwrap();
        let b = dir.join("b.md");
        std::fs::write(&b, "# 会议记录\n今天讨论了加密沙盒的边界与威胁模型").unwrap();
        v.import_file("root", "b.md", &b).unwrap();
        v.save_index().unwrap();
    }

    /// 重装 / 换机后找回旧库：配置指向空路径时，能通过「打开已有保险库」接上旧目录。
    #[test]
    fn open_existing_vault_reattaches_after_config_loss() {
        let dir = tmp("reattach");
        vault::create(&dir, "correct horse battery", None).unwrap();

        // 模拟重装后 config.json 丢失：配置是空的
        let state = std::sync::Arc::new(state::AppState::default());
        assert!(!service::vault_status(&state).exists);

        // 指错了目录要报错，且不能污染配置
        assert!(service::open_vault_at(&state, &tmp("empty").display().to_string()).is_err());
        assert!(state.config.lock().unwrap().vault_path.is_empty());

        let st = service::open_vault_at(&state, &dir.display().to_string()).unwrap();
        assert!(st.exists, "打开后应识别为已有保险库");
        assert_eq!(
            std::path::PathBuf::from(&state.config.lock().unwrap().vault_path),
            dir
        );

        // 接上之后用原密码就能解开——密码只在 keyslot 里，与程序无关
        service::unlock(&state, "correct horse battery").unwrap();
        assert!(state.is_unlocked());
    }


    /// 双口令：两个口令各自封装**不同的** DEK，因此看到的是两套完全独立的内容。
    #[test]
    fn second_password_opens_a_separate_volume() {
        let dir = tmp("hidden");
        vault::create(&dir, "outer-pass-123", Some("inner-pass-456")).unwrap();

        // 主口令 → 主索引
        let a = vault::unlock_full(&dir, "outer-pass-123").unwrap();
        assert_eq!(a.index_file, "index.enc");
        let mut va = store::Vault::open_named(dir.clone(), a.dek, &a.index_file).unwrap();
        let src = dir.join("outer.txt");
        std::fs::write(&src, b"outer only").unwrap();
        va.import_file("root", "outer.txt", &src).unwrap();
        va.save_index().unwrap();

        // 第二口令 → 另一套索引，看不见主库里的文件
        let b = vault::unlock_full(&dir, "inner-pass-456").unwrap();
        assert_ne!(b.index_file, "index.enc");
        let mut vb = store::Vault::open_named(dir.clone(), b.dek, &b.index_file).unwrap();
        assert!(vb.index.entries.is_empty(), "第二口令不该看到主库的文件");
        let src2 = dir.join("inner.txt");
        std::fs::write(&src2, b"inner only").unwrap();
        vb.import_file("root", "inner.txt", &src2).unwrap();
        vb.save_index().unwrap();

        // 回到主口令：仍然只有自己那一个文件
        let a2 = vault::unlock_full(&dir, "outer-pass-123").unwrap();
        let va2 = store::Vault::open_named(dir.clone(), a2.dek, &a2.index_file).unwrap();
        assert_eq!(va2.index.entries.len(), 1);
        assert_eq!(
            va2.index.entries.values().next().unwrap().name,
            "outer.txt"
        );

        // 改第二口令的密码：不能把主口令顶掉，且索引文件名要保持不变
        vault::change_password(&dir, "inner-pass-456", "inner-pass-789").unwrap();
        assert!(vault::unlock_full(&dir, "outer-pass-123").is_ok());
        assert!(vault::unlock_full(&dir, "inner-pass-456").is_err());
        let c = vault::unlock_full(&dir, "inner-pass-789").unwrap();
        assert_eq!(c.index_file, b.index_file, "改密码后索引文件名必须不变");
        let vc = store::Vault::open_named(dir.clone(), c.dek, &c.index_file).unwrap();
        assert_eq!(vc.index.entries.len(), 1);
        assert_eq!(vc.index.entries.values().next().unwrap().name, "inner.txt");

        assert!(vault::unlock(&dir, "no-such-pass").is_err());
    }

    /// 视频按需解密：任意偏移读出来的明文必须和原文件逐字节一致。
    /// 这条覆盖换窗、跨窗、末尾不足一个窗口、越界读四种情况。
    #[test]
    fn media_source_reads_any_range() {
        use std::io::Write;

        let dir = tmp("mediasrc");
        vault::create(&dir, "pw12345678", None).unwrap();
        let dek = vault::unlock(&dir, "pw12345678").unwrap();
        let mut v = store::Vault::open(dir.clone(), dek).unwrap();

        // 9 MB + 一点尾巴：跨 4 MB 窗口边界，且末窗不满
        let mut data = vec![0u8; (9 << 20) + 12345];
        let mut x: u32 = 0x9E37_79B9;
        for b in data.iter_mut() {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            *b = (x & 0xFF) as u8;
        }
        let src_path = dir.join("clip.bin");
        std::fs::File::create(&src_path).unwrap().write_all(&data).unwrap();
        let e = v.import_file("root", "clip.bin", &src_path).unwrap();
        v.save_index().unwrap();

        let state = std::sync::Arc::new(state::AppState::default());
        service::open_vault_at(&state, &dir.display().to_string()).unwrap();
        service::unlock(&state, "pw12345678").unwrap();

        let (_mime, src) = service::open_media(&state, &e.id).unwrap();
        assert_eq!(src.size(), data.len() as u64, "长度不对");

        let cases: [(usize, usize); 6] = [
            (0, 100),
            ((4 << 20) - 10, 20),   // 跨第一个窗口边界
            (4 << 20, 1),           // 正好落在边界上
            (7_000_000, 3 << 20),   // 一次要跨两个窗口
            (data.len() - 5, 5),    // 末尾
            (1_234_567, 65_536),    // 普通中段
        ];
        for (off, len) in cases {
            let mut buf = vec![0u8; len];
            let n = src.read_at(off as u64, &mut buf).unwrap();
            let expect = len.min(data.len() - off);
            assert_eq!(n, expect, "偏移 {off} 读到 {n} 字节，应为 {expect}");
            assert_eq!(&buf[..n], &data[off..off + n], "偏移 {off} 内容不一致");
        }

        // 越界读返回 0，不 panic
        let mut tail = [0u8; 8];
        assert_eq!(src.read_at(data.len() as u64, &mut tail).unwrap(), 0);
        assert_eq!(src.read_at(u64::MAX / 2, &mut tail).unwrap(), 0);
    }

    /// 文件夹：改名、删除（连同内部内容）、恢复、彻底删除。
    #[test]
    fn folder_rename_delete_and_restore() {
        let dir = tmp("folders");
        vault::create(&dir, "pw12345678", None).unwrap();
        let dek = vault::unlock(&dir, "pw12345678").unwrap();
        let mut v = store::Vault::open(dir.clone(), dek).unwrap();

        let folder = v.create_folder("root", "文档").unwrap();
        let src = dir.join("a.txt");
        std::fs::write(&src, b"hello folder").unwrap();
        let child = v.import_file(&folder.id, "a.txt", &src).unwrap();
        v.save_index().unwrap();

        let state = std::sync::Arc::new(state::AppState::default());
        service::open_vault_at(&state, &dir.display().to_string()).unwrap();
        service::unlock(&state, "pw12345678").unwrap();

        // 改名
        let renamed = service::rename_entry(&state, &folder.id, "资料").unwrap();
        assert_eq!(renamed.name, "资料");
        assert_eq!(renamed.kind, "dir");

        // 删除文件夹：子项要跟着进回收站，否则会在「全部文件」里变成孤儿
        service::delete_entry(&state, &folder.id).unwrap();
        assert!(service::list_dir(&state, "root").unwrap().is_empty());
        let trash = service::list_trash(&state).unwrap();
        assert_eq!(trash.len(), 2, "文件夹和内部文件都该在回收站");
        assert!(trash.iter().any(|e| e.id == child.id && e.deleted_at.is_some()));

        // 恢复：两个一起回来
        service::restore_entry(&state, &folder.id).unwrap();
        assert_eq!(service::list_dir(&state, "root").unwrap().len(), 1);
        assert_eq!(service::list_dir(&state, &folder.id).unwrap().len(), 1);
        assert!(service::list_trash(&state).unwrap().is_empty());

        // 彻底删除：文件夹和子项都消失，blob 也不再占空间
        service::delete_entry(&state, &folder.id).unwrap();
        service::purge_entry(&state, &folder.id).unwrap();
        assert!(service::list_trash(&state).unwrap().is_empty());
        assert!(service::list_dir(&state, &folder.id).unwrap().is_empty());
    }

    #[test]
    fn vault_create_unlock_and_wrong_password() {
        let dir = tmp("vault");
        vault::create(&dir, "correct horse battery", None).unwrap();
        assert!(vault::exists(&dir));
        assert!(vault::unlock(&dir, "wrong password").is_err());
        assert!(vault::unlock(&dir, "correct horse battery").is_ok());
    }

    #[test]
    fn import_encrypts_and_never_leaks_plaintext() {
        let dir = tmp("import");
        vault::create(&dir, "pw12345678", None).unwrap();
        let dek = vault::unlock(&dir, "pw12345678").unwrap();
        let mut v = store::Vault::open(dir.clone(), dek).unwrap();

        let src = dir.join("plain.txt");
        std::fs::write(&src, b"SECRET-MARKER-42").unwrap();
        let e = v.import_file("root", "a.txt", &src).unwrap();
        assert_eq!(e.size, 16);

        let blob = v.blob_path(&e.versions[0].blob);
        let raw = std::fs::read(&blob).unwrap();
        assert!(
            !raw.windows(10).any(|w| w == b"SECRET-MARK"),
            "密文里出现了明文"
        );
        assert_eq!(
            v.read_blob(&e.id, &e.versions[0].blob).unwrap().as_slice(),
            &b"SECRET-MARKER-42"[..]
        );
    }

    #[test]
    fn chunked_range_across_boundary() {
        let dir = tmp("range");
        vault::create(&dir, "pw12345678", None).unwrap();
        let dek = vault::unlock(&dir, "pw12345678").unwrap();
        let mut v = store::Vault::open(dir.clone(), dek).unwrap();

        // 2.5 MiB，跨 3 个分块
        let data: Vec<u8> = (0..(2 * 1024 * 1024 + 512 * 1024))
            .map(|i| (i % 251) as u8)
            .collect();
        let src = dir.join("big.bin");
        std::fs::write(&src, &data).unwrap();
        let e = v.import_file("root", "big.bin", &src).unwrap();
        let blob = e.versions[0].blob.clone();

        let head = v.read_blob_range(&e.id, &blob, 0, 16).unwrap();
        assert_eq!(head.as_slice(), &data[..16]);

        // 跨 1 MiB 边界读
        let s = 1024 * 1024 - 8;
        let part = v.read_blob_range(&e.id, &blob, s, s + 16).unwrap();
        assert_eq!(part.as_slice(), &data[s as usize..(s + 16) as usize]);

        let tail = v
            .read_blob_range(&e.id, &blob, data.len() as u64 - 32, data.len() as u64)
            .unwrap();
        assert_eq!(tail.as_slice(), &data[data.len() - 32..]);
    }

    #[test]
    fn versions_and_restore() {
        let dir = tmp("ver");
        vault::create(&dir, "pw12345678", None).unwrap();
        let dek = vault::unlock(&dir, "pw12345678").unwrap();
        let mut v = store::Vault::open(dir.clone(), dek).unwrap();

        let src = dir.join("n.md");
        std::fs::write(&src, b"v0").unwrap();
        let e = v.import_file("root", "n.md", &src).unwrap();
        for i in 1..4 {
            v.save_text(&e.id, &format!("v{i}")).unwrap();
        }
        let entry = v.index.entries[&e.id].clone();
        assert_eq!(entry.versions.len(), 4);
        let cur = entry.current_version().unwrap().clone();
        assert_eq!(v.read_blob(&e.id, &cur.blob).unwrap().as_slice(), &b"v3"[..]);

        let first = entry.versions[0].clone();
        v.restore_version(&e.id, &first.id).unwrap();
        let after = v.index.entries[&e.id].clone();
        assert_eq!(after.versions.len(), 5, "恢复应生成新版本而不是删除历史");
        let cur2 = after.current_version().unwrap().clone();
        assert_eq!(v.read_blob(&e.id, &cur2.blob).unwrap().as_slice(), &b"v0"[..]);
    }

    #[test]
    fn password_change_keeps_files_readable() {
        let dir = tmp("pwchange");
        vault::create(&dir, "oldpassword", None).unwrap();
        let dek = vault::unlock(&dir, "oldpassword").unwrap();
        let mut v = store::Vault::open(dir.clone(), dek).unwrap();
        let src = dir.join("x.txt");
        std::fs::write(&src, b"payload").unwrap();
        let e = v.import_file("root", "x.txt", &src).unwrap();
        let blob = e.versions[0].blob.clone();
        let eid = e.id.clone();
        drop(v);

        vault::change_password(&dir, "oldpassword", "newpassword").unwrap();
        assert!(vault::unlock(&dir, "oldpassword").is_err());
        let dek2 = vault::unlock(&dir, "newpassword").unwrap();
        let v2 = store::Vault::open(dir.clone(), dek2).unwrap();
        assert_eq!(v2.read_blob(&eid, &blob).unwrap().as_slice(), &b"payload"[..]);
    }

    #[test]
    fn backup_copies_ciphertext() {
        let dir = tmp("bk");
        vault::create(&dir, "pw12345678", None).unwrap();
        let dek = vault::unlock(&dir, "pw12345678").unwrap();
        let mut v = store::Vault::open(dir.clone(), dek).unwrap();
        let src = dir.join("y.txt");
        std::fs::write(&src, b"backup me").unwrap();
        v.import_file("root", "y.txt", &src).unwrap();
        v.save_index().unwrap();

        let target = tmp("bk-target");
        let s = BackupSettings {
            enabled: true,
            target: target.display().to_string(),
            mode: "mirror".into(),
            interval_minutes: 60,
            keep_copies: 3,
            last_backup_at: 0,
            last_result: String::new(),
        };
        let rep = backup::run(&dir, &s).unwrap();
        assert!(rep.copied > 0, "应该至少复制了 vault.json");
        assert!(target.join("xvault").join("vault.json").exists());
        assert!(target.join("xvault").join("index.enc").exists());

        // 二次备份应跳过未变化的文件
        let rep2 = backup::run(&dir, &s).unwrap();
        assert_eq!(rep2.copied, 0);
    }

    #[test]
    fn backup_refuses_target_inside_vault() {
        let dir = tmp("bk-inside");
        vault::create(&dir, "pw12345678", None).unwrap();
        let s = BackupSettings {
            enabled: true,
            target: dir.join("inner").display().to_string(),
            mode: "mirror".into(),
            interval_minutes: 60,
            keep_copies: 3,
            last_backup_at: 0,
            last_result: String::new(),
        };
        assert!(backup::run(&dir, &s).is_err());
    }

    /// v2 格式的核心保证：把 A 文件的密文块挪到 B 文件的位置上，必须解不出来，
    /// 而不是静默返回 A 的内容。两个文件长度相同，所以能绕过长度校验直接打到 AEAD。
    #[test]
    fn cross_entry_blob_swap_is_rejected() {
        let dir = tmp("swap");
        vault::create(&dir, "pw12345678", None).unwrap();
        let dek = vault::unlock(&dir, "pw12345678").unwrap();
        let mut v = store::Vault::open(dir.clone(), dek).unwrap();

        let a_src = dir.join("a.txt");
        let b_src = dir.join("b.txt");
        std::fs::write(&a_src, b"AAAAAAAAAAAAAAAA").unwrap();
        std::fs::write(&b_src, b"BBBBBBBBBBBBBBBB").unwrap();
        let a = v.import_file("root", "a.txt", &a_src).unwrap();
        let b = v.import_file("root", "b.txt", &b_src).unwrap();

        let a_blob = v.blob_path(&a.versions[0].blob);
        let b_blob = v.blob_path(&b.versions[0].blob);
        std::fs::copy(&a_blob, &b_blob).unwrap();

        assert!(
            v.read_blob(&b.id, &b.versions[0].blob).is_err(),
            "跨条目替换密文必须解密失败"
        );
        assert_eq!(
            v.read_blob(&a.id, &a.versions[0].blob).unwrap().as_slice(),
            &b"AAAAAAAAAAAAAAAA"[..]
        );
    }

    /// 全文搜索必须能搜到中文正文（二元组切分），并且不要误命中无关文件。
    #[test]
    fn fulltext_search_hits_chinese_content() {
        let dir = tmp("search");
        vault::create(&dir, "pw12345678", None).unwrap();
        let dek = vault::unlock(&dir, "pw12345678").unwrap();
        let mut v = store::Vault::open(dir.clone(), dek).unwrap();

        let a = dir.join("a.md");
        let b = dir.join("b.md");
        std::fs::write(&a, "今天开会讨论了加密沙盒的边界与威胁模型").unwrap();
        std::fs::write(&b, "完全无关的一份购物清单").unwrap();
        v.import_file("root", "a.md", &a).unwrap();
        v.import_file("root", "b.md", &b).unwrap();

        let idx = search::build(&v);
        assert_eq!(idx.docs, 2);

        let hits = idx.query("加密沙盒", 10);
        assert!(hits.iter().any(|h| h.name == "a.md"), "应命中 a.md，实际：{hits:?}");
        assert!(!hits.iter().any(|h| h.name == "b.md"), "不应命中 b.md");

        // 文件名也要能被搜到
        let by_name = idx.query("b.md", 10);
        assert!(by_name.iter().any(|h| h.name == "b.md"));
    }
}
