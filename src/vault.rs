use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::crypto;
use crate::error::{Error, Result};
use crate::store::{atomic_write, now_ts};

pub const DEFAULT_M: u32 = 65536;
pub const DEFAULT_T: u32 = 3;
pub const DEFAULT_P: u32 = 4;
const SLOT_AAD: &[u8] = b"xvault-keyslot-v1";

/// 主口令的索引文件名（历史名，保持兼容，不做迁移）。
pub const PRIMARY_INDEX: &str = "index.enc";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct KdfParams {
    pub algo: String,
    pub m_cost: u32,
    pub t_cost: u32,
    pub p_cost: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct KeySlot {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub kdf: KdfParams,
    pub salt: String,
    pub nonce: String,
    pub wrapped: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VaultFile {
    pub format: String,
    pub version: u32,
    pub vault_id: String,
    pub created_at: i64,
    pub keyslots: Vec<KeySlot>,
}

/// 解开一个口令之后拿到的东西：数据密钥 + 它对应的索引文件。
///
/// 两个口令各自封装**不同的** DEK，因此各自的索引（文件名、目录树、内容）互不可见：
/// 主口令看到 `index.enc`，第二口令看到 `index-<slot>.enc`。
pub struct Unlocked {
    pub dek: Zeroizing<[u8; 32]>,
    /// 该口令对应的索引文件名（不含目录）。
    pub index_file: String,
}

fn b64e(b: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(b)
}

fn b64d(s: &str) -> Result<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(s)
        .map_err(|_| Error::Corrupt)
}

fn slot_aad(id: &str) -> Vec<u8> {
    let mut v = SLOT_AAD.to_vec();
    v.extend_from_slice(id.as_bytes());
    v
}

pub fn exists(root: &Path) -> bool {
    root.join("vault.json").exists()
}

pub fn read_header(root: &Path) -> Result<VaultFile> {
    let raw = fs::read(root.join("vault.json")).map_err(|_| Error::Msg("找不到保险库".into()))?;
    let vf: VaultFile = serde_json::from_slice(&raw)?;
    if vf.format != "xvault" {
        return Err(Error::Msg("不是有效的保险库文件".into()));
    }
    Ok(vf)
}

/// 用指定 id 包一个 keyslot。改密码时复用原 id，索引文件名才不会漂移。
fn make_slot(id: Option<String>, kind: &str, label: &str, secret: &str, dek: &[u8]) -> Result<KeySlot> {
    let id = id.unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string());
    let salt = crypto::random_bytes(16);
    let kek = crypto::derive_key(secret, &salt, DEFAULT_M, DEFAULT_T, DEFAULT_P)?;
    let nonce = crypto::random_bytes(crypto::NONCE_LEN);
    let wrapped = crypto::aead_encrypt(&kek[..], &nonce, &slot_aad(&id), dek)?;
    Ok(KeySlot {
        id,
        kind: kind.into(),
        label: label.into(),
        kdf: KdfParams {
            algo: "argon2id".into(),
            m_cost: DEFAULT_M,
            t_cost: DEFAULT_T,
            p_cost: DEFAULT_P,
        },
        salt: b64e(&salt),
        nonce: b64e(&nonce),
        wrapped: b64e(&wrapped),
    })
}

/// 试一个 slot，成功返回它封装的 DEK。
fn try_slot(slot: &KeySlot, password: &str) -> Option<Zeroizing<[u8; 32]>> {
    let salt = b64d(&slot.salt).ok()?;
    let kek = crypto::derive_key(
        password,
        &salt,
        slot.kdf.m_cost,
        slot.kdf.t_cost,
        slot.kdf.p_cost,
    )
    .ok()?;
    let nonce = b64d(&slot.nonce).ok()?;
    let wrapped = b64d(&slot.wrapped).ok()?;
    let plain = crypto::aead_decrypt(&kek[..], &nonce, &slot_aad(&slot.id), &wrapped).ok()?;
    if plain.len() != 32 {
        return None;
    }
    let mut k = Zeroizing::new([0u8; 32]);
    k.copy_from_slice(&plain[..]);
    Some(k)
}

/// 建库。`second` 非空时额外建一个**独立**的隐藏库：另一套 DEK、另一套索引，
/// 两个口令互相看不到对方的内容。
pub fn create(root: &Path, password: &str, second: Option<&str>) -> Result<()> {
    if exists(root) {
        return Err(Error::Msg(
            "该位置已经有一个保险库了。要打开它，请点下面的「打开已有保险库…」".into(),
        ));
    }
    fs::create_dir_all(root.join("blobs"))?;

    let dek = crypto::random_key();
    let mut keyslots = vec![make_slot(None, "password", "主密码", password, &dek[..])?];

    if let Some(second) = second.map(str::trim).filter(|s| !s.is_empty()) {
        if second == password {
            return Err(Error::Msg("第二口令不能和主口令相同".into()));
        }
        // 关键：第二个 slot 封装的是**另一个** DEK，而不是同一个。
        // 共用 DEK 就变成"两个密码进同一个库"，跟隐藏库完全不是一回事。
        let dek2 = crypto::random_key();
        keyslots.push(make_slot(None, "password", "第二口令", second, &dek2[..])?);
    }

    let vf = VaultFile {
        format: "xvault".into(),
        version: 2,
        vault_id: uuid::Uuid::new_v4().simple().to_string(),
        created_at: now_ts(),
        keyslots,
    };
    atomic_write(&root.join("vault.json"), &serde_json::to_vec_pretty(&vf)?)?;
    Ok(())
}

/// 只用主口令拿 DEK（测试与内部使用）。
pub fn unlock(root: &Path, password: &str) -> Result<Zeroizing<[u8; 32]>> {
    Ok(unlock_full(root, password)?.dek)
}

/// 解开口令，并告诉调用方该用哪个索引文件。
///
/// 第一个 keyslot 恒为 `index.enc`（兼容老库），其余各用自己的 `index-<id>.enc`。
pub fn unlock_full(root: &Path, password: &str) -> Result<Unlocked> {
    let vf = read_header(root)?;
    for (idx, slot) in vf.keyslots.iter().enumerate() {
        if slot.kind != "password" {
            continue;
        }
        if let Some(dek) = try_slot(slot, password) {
            let index_file = if idx == 0 {
                PRIMARY_INDEX.to_string()
            } else {
                format!("index-{}.enc", slot.id)
            };
            return Ok(Unlocked { dek, index_file });
        }
    }
    Err(Error::BadPassword)
}


/// 给已有保险库补一个第二口令。
///
/// 第二口令封装的是**一把全新的 DEK**，所以用它打开是一个空库，
/// 与主口令的内容完全隔离——主库看不到它，它也看不到主库。
/// 这是「隐藏空间」，不是「第二个能进同一空间的密码」。
pub fn add_second(root: &Path, main_password: &str, second: &str) -> Result<()> {
    let mut vf = read_header(root)?;

    // 必须能用主口令打开，才能给它加第二个入口
    let ok = vf
        .keyslots
        .iter()
        .any(|s| s.kind == "password" && try_slot(s, main_password).is_some());
    if !ok {
        return Err(Error::BadPassword);
    }

    let second = second.trim();
    if second.chars().count() < 8 {
        return Err(Error::Msg("第二口令至少 8 个字符".into()));
    }
    // 已经能打开本库的口令不能再加一遍，否则同一个入口出现两个 slot
    if vf
        .keyslots
        .iter()
        .any(|s| s.kind == "password" && try_slot(s, second).is_some())
    {
        return Err(Error::Msg("这个口令已经能打开本库了，换一个".into()));
    }

    let dek2 = crypto::random_key();
    vf.keyslots
        .push(make_slot(None, "password", "第二口令", second, &dek2[..])?);
    atomic_write(&root.join("vault.json"), &serde_json::to_vec_pretty(&vf)?)?;
    Ok(())
}

/// 改密码只需重包裹 DEK，不触碰任何文件内容。
///
/// 必须按"哪个口令能打开哪个 slot"来定位：多口令的库里，
/// 直接改第一个 password slot 会把另一个口令的入口一起顶掉。
pub fn change_password(root: &Path, old: &str, new: &str) -> Result<()> {
    let mut vf = read_header(root)?;
    let mut target: Option<(usize, Zeroizing<[u8; 32]>)> = None;
    for (idx, slot) in vf.keyslots.iter().enumerate() {
        if slot.kind != "password" {
            continue;
        }
        if let Some(dek) = try_slot(slot, old) {
            target = Some((idx, dek));
            break;
        }
    }
    let Some((idx, dek)) = target else {
        return Err(Error::BadPassword);
    };
    // 复用原 slot id：非主口令的索引文件名由 id 决定，换了 id 就找不到自己的内容了。
    let id = vf.keyslots[idx].id.clone();
    let label = vf.keyslots[idx].label.clone();
    vf.keyslots[idx] = make_slot(Some(id), "password", &label, new, &dek[..])?;
    atomic_write(&root.join("vault.json"), &serde_json::to_vec_pretty(&vf)?)?;
    Ok(())
}
