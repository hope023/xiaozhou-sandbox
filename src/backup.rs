use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Serialize;

use crate::error::{Error, Result};
use crate::store::{now_ts, BackupSettings};

#[derive(Serialize, Clone, Debug, Default)]
pub struct BackupReport {
    pub at: i64,
    pub copied: u64,
    pub skipped: u64,
    pub bytes: u64,
    pub target: String,
    pub message: String,
}

fn mtime(md: &fs::Metadata) -> u64 {
    md.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn human(bytes: u64) -> String {
    const U: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = bytes as f64;
    let mut i = 0;
    while v >= 1024.0 && i < U.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    format!("{:.1} {}", v, U[i])
}

fn copy_tree(src: &Path, dst: &Path, skip: &Path, rep: &mut BackupReport) -> Result<()> {
    for ent in fs::read_dir(src)? {
        let ent = ent?;
        let p = ent.path();
        if p == skip || p.starts_with(skip) {
            continue;
        }
        let d = dst.join(ent.file_name());
        let md = ent.metadata()?;
        if md.is_dir() {
            fs::create_dir_all(&d)?;
            copy_tree(&p, &d, skip, rep)?;
        } else {
            let need = match fs::metadata(&d) {
                Ok(dm) => dm.len() != md.len() || mtime(&md) > mtime(&dm),
                Err(_) => true,
            };
            if need {
                fs::copy(&p, &d)?;
                rep.copied += 1;
                rep.bytes += md.len();
            } else {
                rep.skipped += 1;
            }
        }
    }
    Ok(())
}

fn prune_snapshots(root: &Path, keep: u32) {
    let keep = keep.max(1) as usize;
    let mut dirs: Vec<PathBuf> = match fs::read_dir(root) {
        Ok(rd) => rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.is_dir()
                    && p.file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n.starts_with("xvault-"))
                        .unwrap_or(false)
            })
            .collect(),
        Err(_) => return,
    };
    dirs.sort();
    if dirs.len() > keep {
        for p in dirs.iter().take(dirs.len() - keep) {
            let _ = fs::remove_dir_all(p);
        }
    }
}

/// 保险库本身就是密文，所以备份 = 复制文件，不需要解密。
pub fn run(root: &Path, s: &BackupSettings) -> Result<BackupReport> {
    let target = s.target.trim();
    if target.is_empty() {
        return Err(Error::Msg("还没有设置备份位置".into()));
    }
    let target_root = PathBuf::from(target);
    fs::create_dir_all(&target_root)?;

    let dest = if s.mode == "snapshot" {
        let d = target_root.join(format!("xvault-{}", chrono::Local::now().format("%Y%m%d-%H%M%S")));
        fs::create_dir_all(&d)?;
        d
    } else {
        let d = target_root.join("xvault");
        fs::create_dir_all(&d)?;
        d
    };

    let mut rep = BackupReport {
        at: now_ts(),
        target: dest.display().to_string(),
        ..Default::default()
    };

    let canon_root = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let canon_dest = fs::canonicalize(&dest).unwrap_or_else(|_| dest.clone());
    if canon_dest.starts_with(&canon_root) {
        return Err(Error::Msg("备份位置不能放在保险库内部".into()));
    }

    copy_tree(root, &dest, &canon_dest, &mut rep)?;
    if s.mode == "snapshot" {
        prune_snapshots(&target_root, s.keep_copies);
    }

    rep.message = format!("同步 {} 个文件 · {}", rep.copied, human(rep.bytes));
    Ok(rep)
}

pub fn dir_size(p: &Path) -> u64 {
    let mut total = 0u64;
    if let Ok(rd) = fs::read_dir(p) {
        for e in rd.flatten() {
            if let Ok(md) = e.metadata() {
                if md.is_dir() {
                    total += dir_size(&e.path());
                } else {
                    total += md.len();
                }
            }
        }
    }
    total
}
