use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use zeroize::Zeroizing;

use crate::crypto;
use crate::error::{Error, Result};

pub const MAGIC: [u8; 4] = *b"XVB1";
pub const CHUNK: usize = 1 << 20;
pub const TAG: usize = 16;
pub const HEADER_LEN: usize = 4 + 1 + 1 + 4 + 16 + 8 + 16;
const KEY_INFO: &[u8] = b"xvault-blob-v1";

/// 密文格式版本。
///
/// - v1：AAD = header || chunk_idx
/// - v2：AAD = header || chunk_idx || entry_id
///
/// v2 把密文块绑定到它所属的条目。这样把 A 文件的 `.bin` 拷到 B 文件的位置上，
/// 解密会直接失败，而不是静默解出另一个文件的内容。旧的 v1 文件继续可读，
/// 并在下次保存时自然升级为 v2，不需要批量迁移。
pub const FMT_V2: u8 = 2;

#[derive(Clone)]
pub struct Header {
    pub version: u8,
    pub file_id: [u8; 16],
    pub plain_len: u64,
    pub chunk_size: u32,
    pub nonce_prefix: [u8; 16],
}

impl Header {
    pub fn to_bytes(&self) -> [u8; HEADER_LEN] {
        let mut b = [0u8; HEADER_LEN];
        b[0..4].copy_from_slice(&MAGIC);
        b[4] = self.version;
        b[5] = 0;
        b[6..10].copy_from_slice(&self.chunk_size.to_le_bytes());
        b[10..26].copy_from_slice(&self.file_id);
        b[26..34].copy_from_slice(&self.plain_len.to_le_bytes());
        b[34..50].copy_from_slice(&self.nonce_prefix);
        b
    }

    pub fn parse(b: &[u8]) -> Result<Header> {
        if b.len() < HEADER_LEN || b[0..4] != MAGIC {
            return Err(Error::Corrupt);
        }
        let version = b[4];
        if version == 0 || version > FMT_V2 {
            return Err(Error::Corrupt);
        }
        let mut file_id = [0u8; 16];
        file_id.copy_from_slice(&b[10..26]);
        let mut nonce_prefix = [0u8; 16];
        nonce_prefix.copy_from_slice(&b[34..50]);
        let chunk_size = u32::from_le_bytes(b[6..10].try_into().unwrap());
        if chunk_size == 0 || chunk_size as usize > 64 * 1024 * 1024 {
            return Err(Error::Corrupt);
        }
        let mut len_b = [0u8; 8];
        len_b.copy_from_slice(&b[26..34]);
        Ok(Header {
            version,
            file_id,
            plain_len: u64::from_le_bytes(len_b),
            chunk_size,
            nonce_prefix,
        })
    }

    pub fn chunk_count(&self) -> u64 {
        let cs = self.chunk_size as u64;
        if self.plain_len == 0 {
            1
        } else {
            (self.plain_len + cs - 1) / cs
        }
    }

    pub fn last_chunk_index(&self) -> u64 {
        self.chunk_count() - 1
    }

    pub fn chunk_plain_len(&self, idx: u64) -> usize {
        let cs = self.chunk_size as u64;
        if idx < self.last_chunk_index() {
            cs as usize
        } else {
            (self.plain_len - idx * cs) as usize
        }
    }

    pub fn chunk_ct_len(&self, idx: u64) -> usize {
        self.chunk_plain_len(idx) + TAG
    }

    pub fn chunk_offset(&self, idx: u64) -> u64 {
        HEADER_LEN as u64 + idx * (self.chunk_size as u64 + TAG as u64)
    }
}

fn nonce_for(prefix: &[u8; 16], idx: u64) -> [u8; 24] {
    let mut n = [0u8; 24];
    n[0..16].copy_from_slice(prefix);
    n[16..24].copy_from_slice(&idx.to_le_bytes());
    n
}

fn aad_for(header_bytes: &[u8], idx: u64, owner: Option<&str>) -> Vec<u8> {
    let mut v = Vec::with_capacity(header_bytes.len() + 8 + 32);
    v.extend_from_slice(header_bytes);
    v.extend_from_slice(&idx.to_le_bytes());
    if let Some(e) = owner {
        v.extend_from_slice(e.as_bytes());
    }
    v
}

fn read_up_to(f: &mut File, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut n = 0usize;
    while n < buf.len() {
        match f.read(&mut buf[n..]) {
            Ok(0) => break,
            Ok(k) => n += k,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(n)
}

/// 流式加密：不把整个文件读进内存。新写入一律使用 v2 格式。
pub fn encrypt_file(
    dek: &[u8],
    file_id: [u8; 16],
    entry_id: &str,
    src: &Path,
    dst: &Path,
) -> Result<u64> {
    let plain_len = std::fs::metadata(src)?.len();
    let header = Header {
        version: FMT_V2,
        file_id,
        plain_len,
        chunk_size: CHUNK as u32,
        nonce_prefix: crypto::random_16(),
    };
    let key = crypto::subkey(dek, &file_id, KEY_INFO);
    let hb = header.to_bytes();
    let owner = Some(entry_id);

    let mut fin = File::open(src)?;
    let mut fout = File::create(dst)?;
    fout.write_all(&hb)?;

    let mut buf = vec![0u8; CHUNK];
    let mut idx: u64 = 0;
    loop {
        let n = read_up_to(&mut fin, &mut buf)?;
        let nonce = nonce_for(&header.nonce_prefix, idx);
        let aad = aad_for(&hb, idx, owner);
        let ct = crypto::aead_encrypt(&key[..], &nonce, &aad, &buf[..n])?;
        fout.write_all(&ct)?;
        idx += 1;
        if n < CHUNK {
            break;
        }
    }
    fout.sync_all()?;
    Ok(plain_len)
}

pub fn encrypt_bytes(
    dek: &[u8],
    file_id: [u8; 16],
    entry_id: &str,
    plain: &[u8],
) -> Result<Vec<u8>> {
    let header = Header {
        version: FMT_V2,
        file_id,
        plain_len: plain.len() as u64,
        chunk_size: CHUNK as u32,
        nonce_prefix: crypto::random_16(),
    };
    let key = crypto::subkey(dek, &file_id, KEY_INFO);
    let hb = header.to_bytes();
    let owner = Some(entry_id);
    let mut out = Vec::with_capacity(HEADER_LEN + plain.len() + TAG);
    out.extend_from_slice(&hb);
    let cs = CHUNK;
    let mut idx: u64 = 0;
    let mut pos = 0usize;
    loop {
        let end = (pos + cs).min(plain.len());
        let nonce = nonce_for(&header.nonce_prefix, idx);
        let aad = aad_for(&hb, idx, owner);
        let ct = crypto::aead_encrypt(&key[..], &nonce, &aad, &plain[pos..end])?;
        out.extend_from_slice(&ct);
        idx += 1;
        pos = end;
        if pos >= plain.len() {
            break;
        }
    }
    Ok(out)
}

fn open_and_check(path: &Path) -> Result<(File, Header)> {
    let mut f = File::open(path)?;
    let mut hb = [0u8; HEADER_LEN];
    if read_up_to(&mut f, &mut hb)? < HEADER_LEN {
        return Err(Error::Corrupt);
    }
    let header = Header::parse(&hb)?;
    let expect = header.chunk_offset(header.last_chunk_index())
        + header.chunk_ct_len(header.last_chunk_index()) as u64;
    let actual = f.metadata()?.len();
    if actual != expect {
        return Err(Error::Corrupt);
    }
    Ok((f, header))
}

/// 按明文偏移解密一段，供视频 Range 请求使用。
pub fn decrypt_range(
    dek: &[u8],
    path: &Path,
    entry_id: &str,
    start: u64,
    end: u64,
) -> Result<Zeroizing<Vec<u8>>> {
    let (mut f, header) = open_and_check(path)?;
    let owner = if header.version >= FMT_V2 {
        Some(entry_id)
    } else {
        None
    };
    let total = header.plain_len;
    let start = start.min(total);
    let end = end.min(total);
    if end <= start {
        return Ok(Zeroizing::new(Vec::new()));
    }
    let key = crypto::subkey(dek, &header.file_id, KEY_INFO);
    let hb = header.to_bytes();
    let cs = header.chunk_size as u64;
    let first = start / cs;
    let last = (end - 1) / cs;
    let mut out = Zeroizing::new(Vec::with_capacity((end - start) as usize + TAG));
    let mut ct = vec![0u8; header.chunk_size as usize + TAG];

    for idx in first..=last {
        let offset = header.chunk_offset(idx);
        let clen = header.chunk_ct_len(idx);
        f.seek(SeekFrom::Start(offset))?;
        let n = read_up_to(&mut f, &mut ct[..clen])?;
        if n != clen {
            return Err(Error::Corrupt);
        }
        let nonce = nonce_for(&header.nonce_prefix, idx);
        let aad = aad_for(&hb, idx, owner);
        let plain = crypto::aead_decrypt(&key[..], &nonce, &aad, &ct[..clen])?;
        let chunk_start = idx * cs;
        let lo = start.saturating_sub(chunk_start) as usize;
        let hi = ((end - chunk_start) as usize).min(plain.len());
        if lo < hi {
            out.extend_from_slice(&plain[lo..hi]);
        }
    }
    Ok(out)
}

pub fn decrypt_all(dek: &[u8], path: &Path, entry_id: &str) -> Result<Zeroizing<Vec<u8>>> {
    let (_, header) = open_and_check(path)?;
    decrypt_range(dek, path, entry_id, 0, header.plain_len)
}
