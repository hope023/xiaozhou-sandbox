use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::{blob, crypto};
use crate::error::{Error, Result};

const INDEX_INFO: &[u8] = b"xvault-index-v1";

pub fn now_ts() -> i64 {
    chrono::Utc::now().timestamp()
}

/// 按 MIME / 扩展名把文件归到 image/video/audio/text/other。
/// 决定列表图标、检查器用哪种预览，以及搜索时是否解密正文。
/// （原在 commands.rs，搜索索引也要用，挪到核心库。）
pub fn classify(mime: &str, name: &str) -> &'static str {
    if mime.starts_with("image/") {
        return "image";
    }
    if mime.starts_with("video/") {
        return "video";
    }
    if mime.starts_with("audio/") {
        return "audio";
    }
    if mime.starts_with("text/") {
        return "text";
    }
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    const TEXT: [&str; 26] = [
        "md", "markdown", "json", "toml", "yaml", "yml", "rs", "ts", "tsx", "js", "jsx", "py",
        "go", "c", "h", "cpp", "hpp", "html", "css", "csv", "log", "ini", "conf", "sh", "bat", "sql",
    ];
    if TEXT.contains(&ext.as_str()) {
        return "text";
    }
    "other"
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Version {
    pub id: String,
    pub blob: String,
    pub size: u64,
    pub created_at: i64,
    #[serde(default)]
    pub pinned: bool,
    /// 明文内容的 SHA-256（十六进制）。入库时顺路算好，查重直接比它，
    /// 不用再解密整份文件——大文件也能瞬时判断。
    /// 旧库里的版本没有这个字段，第一次查到时补算并写回。
    #[serde(default)]
    pub hash: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Entry {
    pub id: String,
    pub parent: String,
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub mime: String,
    pub size: u64,
    pub created_at: i64,
    pub modified_at: i64,
    #[serde(default)]
    pub current: Option<String>,
    #[serde(default)]
    pub versions: Vec<Version>,
    #[serde(default)]
    pub deleted_at: Option<i64>,
}

impl Entry {
    pub fn current_version(&self) -> Option<&Version> {
        let cur = self.current.as_ref()?;
        self.versions.iter().find(|v| &v.id == cur)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BackupSettings {
    pub enabled: bool,
    pub target: String,
    pub mode: String,
    pub interval_minutes: u32,
    pub keep_copies: u32,
    pub last_backup_at: i64,
    #[serde(default)]
    pub last_result: String,
}

impl Default for BackupSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            target: String::new(),
            mode: "mirror".into(),
            interval_minutes: 60,
            keep_copies: 5,
            last_backup_at: 0,
            last_result: String::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Settings {
    pub auto_lock_minutes: u32,
    pub version_retention: String,
    pub max_versions: usize,
    pub backup: BackupSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_lock_minutes: 5,
            version_retention: "tiered".into(),
            max_versions: 200,
            backup: BackupSettings::default(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Index {
    pub entries: HashMap<String, Entry>,
    pub settings: Settings,
}

pub struct Vault {
    pub root: PathBuf,
    pub dek: Zeroizing<[u8; 32]>,
    pub index: Index,
    /// 本次解锁对应的索引文件名。主口令是 `index.enc`，
    /// 第二口令是 `index-<slot>.enc`——同一个目录里两套索引互不干扰。
    pub index_file: String,
}

pub fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    let tmp = path.with_extension("tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(data)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

impl Vault {
    pub fn blobs_dir(&self) -> PathBuf {
        self.root.join("blobs")
    }

    pub fn index_path(&self) -> PathBuf {
        self.root.join(&self.index_file)
    }

    /// 打开主索引（`index.enc`）。
    pub fn open(root: PathBuf, dek: Zeroizing<[u8; 32]>) -> Result<Vault> {
        Self::open_named(root, dek, "index.enc")
    }

    /// 打开指定索引文件。文件不存在时是空库（第二口令第一次进来就是这种情况）。
    pub fn open_named(root: PathBuf, dek: Zeroizing<[u8; 32]>, file: &str) -> Result<Vault> {
        fs::create_dir_all(root.join("blobs"))?;
        let index_file = file.to_string();
        let index_path = root.join(&index_file);
        let index = if index_path.exists() {
            let raw = fs::read(&index_path)?;
            if raw.len() < 4 + crypto::NONCE_LEN || &raw[0..4] != b"XVI1" {
                return Err(Error::Corrupt);
            }
            let nonce = &raw[4..4 + crypto::NONCE_LEN];
            let key = crypto::subkey(&dek[..], INDEX_INFO, INDEX_INFO);
            let plain = crypto::aead_decrypt(&key[..], nonce, INDEX_INFO, &raw[4 + crypto::NONCE_LEN..])?;
            serde_json::from_slice::<Index>(&plain[..])?
        } else {
            Index::default()
        };
        Ok(Vault { root, dek, index, index_file })
    }

    pub fn save_index(&self) -> Result<()> {
        let plain = serde_json::to_vec(&self.index)?;
        let key = crypto::subkey(&self.dek[..], INDEX_INFO, INDEX_INFO);
        let nonce = crypto::random_bytes(crypto::NONCE_LEN);
        let ct = crypto::aead_encrypt(&key[..], &nonce, INDEX_INFO, &plain)?;
        let mut out = Vec::with_capacity(4 + crypto::NONCE_LEN + ct.len());
        out.extend_from_slice(b"XVI1");
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&ct);
        atomic_write(&self.index_path(), &out)
    }

    pub fn blob_path(&self, blob_id: &str) -> PathBuf {
        let shard = &blob_id[0..2.min(blob_id.len())];
        self.blobs_dir().join(shard).join(format!("{blob_id}.bin"))
    }

    pub fn read_blob(&self, entry_id: &str, blob_id: &str) -> Result<Zeroizing<Vec<u8>>> {
        let p = self.blob_path(blob_id);
        if !p.exists() {
            return Err(Error::NotFound);
        }
        blob::decrypt_all(&self.dek[..], &p, entry_id)
    }

    pub fn read_blob_range(
        &self,
        entry_id: &str,
        blob_id: &str,
        start: u64,
        end: u64,
    ) -> Result<Zeroizing<Vec<u8>>> {
        let p = self.blob_path(blob_id);
        if !p.exists() {
            return Err(Error::NotFound);
        }
        blob::decrypt_range(&self.dek[..], &p, entry_id, start, end)
    }

    /// 返回 (blob id, 明文大小, 明文指纹)。
    fn put_blob_file(
        &self,
        file_id: [u8; 16],
        entry_id: &str,
        src: &Path,
    ) -> Result<(String, u64, String)> {
        let blob_id = uuid::Uuid::new_v4().simple().to_string();
        let dst = self.blob_path(&blob_id);
        fs::create_dir_all(dst.parent().unwrap())?;
        let tmp = dst.with_extension("tmp");
        let (size, digest) = blob::encrypt_file(&self.dek[..], file_id, entry_id, src, &tmp)?;
        fs::rename(&tmp, &dst)?;
        Ok((blob_id, size, crypto::hex(&digest)))
    }

    /// 返回 (blob id, 明文大小, 明文指纹)。
    fn put_blob_bytes(
        &self,
        file_id: [u8; 16],
        entry_id: &str,
        plain: &[u8],
    ) -> Result<(String, u64, String)> {
        let blob_id = uuid::Uuid::new_v4().simple().to_string();
        let dst = self.blob_path(&blob_id);
        fs::create_dir_all(dst.parent().unwrap())?;
        let ct = blob::encrypt_bytes(&self.dek[..], file_id, entry_id, plain)?;
        atomic_write(&dst, &ct)?;
        Ok((blob_id, plain.len() as u64, crypto::sha256_hex(plain)))
    }

    pub fn remove_blob(&self, blob_id: &str) {
        let p = self.blob_path(blob_id);
        let _ = fs::remove_file(p);
    }

    pub fn import_file(&mut self, parent: &str, name: &str, src: &Path) -> Result<Entry> {
        let id = uuid::Uuid::new_v4().simple().to_string();
        let fid = crypto::random_16();
        let (blob_id, size, hash) = self.put_blob_file(fid, &id, src)?;
        let ts = now_ts();
        let ver = Version {
            id: uuid::Uuid::new_v4().simple().to_string(),
            blob: blob_id,
            size,
            created_at: ts,
            pinned: false,
            hash,
        };
        let entry = Entry {
            id: id.clone(),
            parent: parent.to_string(),
            name: name.to_string(),
            kind: "file".into(),
            mime: mime_guess::from_path(name).first_or_octet_stream().to_string(),
            size,
            created_at: ts,
            modified_at: ts,
            current: Some(ver.id.clone()),
            versions: vec![ver],
            deleted_at: None,
        };
        self.index.entries.insert(id, entry.clone());
        Ok(entry)
    }

    pub fn add_version(
        &mut self,
        entry_id: &str,
        blob_id: String,
        size: u64,
        hash: String,
    ) -> Result<()> {
        let ts = now_ts();
        let settings = self.index.settings.clone();
        let entry = self.index.entries.get_mut(entry_id).ok_or(Error::NotFound)?;
        let ver = Version {
            id: uuid::Uuid::new_v4().simple().to_string(),
            blob: blob_id,
            size,
            created_at: ts,
            pinned: false,
            hash,
        };
        entry.versions.push(ver.clone());
        entry.current = Some(ver.id);
        entry.size = size;
        entry.modified_at = ts;
        prune_versions(entry, &settings);
        Ok(())
    }

    pub fn save_text(&mut self, entry_id: &str, content: &str) -> Result<Entry> {
        let fid = crypto::random_16();
        let (blob_id, size, hash) = self.put_blob_bytes(fid, entry_id, content.as_bytes())?;
        self.add_version(entry_id, blob_id, size, hash)?;
        self.save_index()?;
        self.index.entries.get(entry_id).cloned().ok_or(Error::NotFound)
    }

    pub fn create_folder(&mut self, parent: &str, name: &str) -> Result<Entry> {
        let ts = now_ts();
        let id = uuid::Uuid::new_v4().simple().to_string();
        let entry = Entry {
            id: id.clone(),
            parent: parent.to_string(),
            name: name.to_string(),
            kind: "dir".into(),
            mime: String::new(),
            size: 0,
            created_at: ts,
            modified_at: ts,
            current: None,
            versions: Vec::new(),
            deleted_at: None,
        };
        self.index.entries.insert(id, entry.clone());
        Ok(entry)
    }

    pub fn children(&self, parent: &str, trash: bool) -> Vec<Entry> {
        let mut v: Vec<Entry> = self
            .index
            .entries
            .values()
            .filter(|e| e.parent == parent && e.deleted_at.is_some() == trash)
            .cloned()
            .collect();
        v.sort_by(|a, b| {
            let ak = if a.kind == "dir" { 0 } else { 1 };
            let bk = if b.kind == "dir" { 0 } else { 1 };
            ak.cmp(&bk)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        v
    }


    /// 同名冲突时生成 "名字 (2).ext"。`except_id` 用来排除自己
    /// （在同一个文件夹里移动时，自己的名字不该被算作占用）。
    pub fn unique_child_name(&self, parent: &str, name: &str, except_id: &str) -> String {
        let taken: Vec<&str> = self
            .index
            .entries
            .values()
            .filter(|e| e.parent == parent && e.deleted_at.is_none() && e.id != except_id)
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

    /// 把条目移到另一个文件夹。文件夹不能移进自己或自己的子孙里。
    pub fn move_entry(&mut self, id: &str, new_parent: &str) -> Result<Entry> {
        if id == new_parent {
            return Err(Error::Msg("不能移动到它自己里面".into()));
        }
        // "root" 是虚拟根目录，没有对应条目
        if new_parent != "root" {
            let target = self
                .index
                .entries
                .get(new_parent)
                .ok_or(Error::NotFound)?
                .clone();
            if target.kind != "dir" || target.deleted_at.is_some() {
                return Err(Error::Msg("目标不是可用的文件夹".into()));
            }
        }

        // 沿新父目录往上走，撞到自己就说明要移进自己的子孙里
        let mut cur = new_parent.to_string();
        while cur != "root" {
            if cur == id {
                return Err(Error::Msg("不能把文件夹移动到它自己的子目录里".into()));
            }
            match self.index.entries.get(&cur) {
                Some(e) => cur = e.parent.clone(),
                None => break,
            }
        }

        let name = self.index.entries.get(id).ok_or(Error::NotFound)?.name.clone();
        let unique = self.unique_child_name(new_parent, &name, id);
        let e = self.index.entries.get_mut(id).ok_or(Error::NotFound)?;
        e.parent = new_parent.to_string();
        e.name = unique;
        e.modified_at = now_ts();
        let out = e.clone();
        self.save_index()?;
        Ok(out)
    }

    pub fn delete_entry(&mut self, id: &str) -> Result<()> {
        let ts = now_ts();
        let ids = self.subtree_ids(id);
        for i in ids {
            if let Some(e) = self.index.entries.get_mut(&i) {
                e.deleted_at = Some(ts);
            }
        }
        self.save_index()
    }

    pub fn restore_entry(&mut self, id: &str) -> Result<()> {
        let ids = self.subtree_ids(id);
        for i in ids {
            if let Some(e) = self.index.entries.get_mut(&i) {
                e.deleted_at = None;
            }
        }
        self.save_index()
    }

    pub fn purge_entry(&mut self, id: &str) -> Result<()> {
        let ids = self.subtree_ids(id);
        for i in &ids {
            if let Some(e) = self.index.entries.get(i).cloned() {
                for v in &e.versions {
                    self.remove_blob(&v.blob);
                }
            }
        }
        for i in ids {
            self.index.entries.remove(&i);
        }
        self.save_index()
    }

    /// 清空回收站：把所有 `deleted_at` 有值的条目连同历史版本一起销毁。
    /// 返回销毁的条目数。
    pub fn purge_trash(&mut self) -> Result<usize> {
        let ids: Vec<String> = self
            .index
            .entries
            .values()
            .filter(|e| e.deleted_at.is_some())
            .map(|e| e.id.clone())
            .collect();
        if ids.is_empty() {
            return Ok(0);
        }
        for id in &ids {
            if let Some(e) = self.index.entries.get(id) {
                for v in &e.versions {
                    self.remove_blob(&v.blob);
                }
            }
        }
        for id in &ids {
            self.index.entries.remove(id);
        }
        self.save_index()?;
        Ok(ids.len())
    }

    fn subtree_ids(&self, id: &str) -> Vec<String> {
        let mut out = vec![id.to_string()];
        let mut i = 0;
        while i < out.len() {
            let cur = out[i].clone();
            for e in self.index.entries.values() {
                if e.parent == cur {
                    out.push(e.id.clone());
                }
            }
            i += 1;
        }
        out
    }

    pub fn restore_version(&mut self, entry_id: &str, version_id: &str) -> Result<Entry> {
        let (blob_id, size, hash) = {
            let e = self.index.entries.get(entry_id).ok_or(Error::NotFound)?;
            let v = e.versions.iter().find(|v| v.id == version_id).ok_or(Error::NotFound)?;
            (v.blob.clone(), v.size, v.hash.clone())
        };
        self.add_version(entry_id, blob_id, size, hash)?;
        self.save_index()?;
        self.index.entries.get(entry_id).cloned().ok_or(Error::NotFound)
    }

    /// 给旧版本补上内容指纹。旧库（本字段出现之前写的）第一次被查到时
    /// 现算一次，写回索引，之后就只剩一次哈希比较。调用方负责存索引。
    pub fn fill_version_hash(&mut self, entry_id: &str, version_id: &str, hash: &str) {
        if let Some(e) = self.index.entries.get_mut(entry_id) {
            if let Some(v) = e.versions.iter_mut().find(|v| v.id == version_id) {
                v.hash = hash.to_string();
            }
        }
    }

    pub fn pin_version(&mut self, entry_id: &str, version_id: &str, pinned: bool) -> Result<()> {
        if let Some(e) = self.index.entries.get_mut(entry_id) {
            if let Some(v) = e.versions.iter_mut().find(|v| v.id == version_id) {
                v.pinned = pinned;
            }
        }
        self.save_index()
    }

    pub fn stats(&self) -> (usize, u64) {
        let files = self.index.entries.values().filter(|e| e.kind == "file" && e.deleted_at.is_none()).count();
        let bytes = self.index.entries.values().filter(|e| e.deleted_at.is_none()).map(|e| e.size).sum();
        (files, bytes)
    }
}

fn bucket_key(age: i64, ts: i64) -> i64 {
    if age < 86_400 {
        ts / 3600
    } else if age < 7 * 86_400 {
        ts / 86_400
    } else if age < 30 * 86_400 {
        ts / (7 * 86_400)
    } else {
        ts / (30 * 86_400)
    }
}

/// 时间分层保留：越久远越稀疏，用户感觉"历史一直在"，实际占用很小。
fn prune_versions(entry: &mut Entry, settings: &Settings) {
    if entry.versions.len() < 2 {
        return;
    }
    let mode = settings.version_retention.as_str();
    if mode == "all" {
        return;
    }

    let now = now_ts();
    let mut order: Vec<usize> = (0..entry.versions.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(entry.versions[i].created_at));

    let mut keep: HashSet<usize> = HashSet::new();
    let mut seen: HashSet<i64> = HashSet::new();
    for (rank, &i) in order.iter().enumerate() {
        let v = &entry.versions[i];
        if v.pinned || rank == 0 {
            keep.insert(i);
            continue;
        }
        if mode == "off" {
            continue;
        }
        let age = (now - v.created_at).max(0);
        // 一小时内每次都留，避免连续自动保存被折叠
        if age < 3600 {
            keep.insert(i);
            continue;
        }
        if seen.insert(bucket_key(age, v.created_at)) {
            keep.insert(i);
        }
    }

    // 上限保护：钉住的永远保留，其余按新旧截断
    if keep.len() > settings.max_versions {
        let mut kept: Vec<usize> = keep.iter().copied().collect();
        kept.sort_by_key(|&i| std::cmp::Reverse(entry.versions[i].created_at));
        let mut selected: HashSet<usize> = HashSet::new();
        for &i in &kept {
            if entry.versions[i].pinned {
                selected.insert(i);
            }
        }
        let budget = settings.max_versions.saturating_sub(selected.len());
        let mut added = 0usize;
        for &i in &kept {
            if selected.contains(&i) {
                continue;
            }
            if added >= budget {
                break;
            }
            selected.insert(i);
            added += 1;
        }
        keep = selected;
    }

    let mut idx = 0;
    entry.versions.retain(|_| {
        let k = keep.contains(&idx);
        idx += 1;
        k
    });
    if entry.current.is_none() || !entry.versions.iter().any(|v| Some(&v.id) == entry.current.as_ref()) {
        entry.current = entry.versions.first().map(|v| v.id.clone());
    }
}
