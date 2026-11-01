use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    XChaCha20Poly1305, XNonce,
};
use hkdf::Hkdf;
use rand::{rngs::OsRng, RngCore};
use sha2::Sha256;
use zeroize::Zeroizing;

use crate::error::{Error, Result};

pub const KEY_LEN: usize = 32;
pub const NONCE_LEN: usize = 24;

pub fn random_bytes(n: usize) -> Vec<u8> {
    let mut b = vec![0u8; n];
    OsRng.fill_bytes(&mut b);
    b
}

pub fn random_16() -> [u8; 16] {
    let mut b = [0u8; 16];
    OsRng.fill_bytes(&mut b);
    b
}

pub fn random_key() -> Zeroizing<[u8; KEY_LEN]> {
    let mut k = Zeroizing::new([0u8; KEY_LEN]);
    OsRng.fill_bytes(&mut k[..]);
    k
}

/// Argon2id 派生密钥。参数随库保存，便于以后调参而不破坏兼容性。
pub fn derive_key(
    password: &str,
    salt: &[u8],
    m_cost: u32,
    t_cost: u32,
    p_cost: u32,
) -> Result<Zeroizing<[u8; KEY_LEN]>> {
    let params = Params::new(m_cost, t_cost, p_cost, Some(KEY_LEN)).map_err(|_| Error::Crypto)?;
    let a2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut out = Zeroizing::new([0u8; KEY_LEN]);
    a2.hash_password_into(password.as_bytes(), salt, &mut out[..])
        .map_err(|_| Error::Crypto)?;
    Ok(out)
}

pub fn aead_encrypt(key: &[u8], nonce: &[u8], aad: &[u8], plain: &[u8]) -> Result<Vec<u8>> {
    let c = XChaCha20Poly1305::new_from_slice(key).map_err(|_| Error::Crypto)?;
    c.encrypt(
        XNonce::from_slice(nonce),
        Payload { msg: plain, aad },
    )
    .map_err(|_| Error::Crypto)
}

/// 解密结果用 `Zeroizing` 包裹：离开作用域时自动抹零，
/// 避免明文留在堆上被换页写进 pagefile 或崩溃转储。
pub fn aead_decrypt(
    key: &[u8],
    nonce: &[u8],
    aad: &[u8],
    ct: &[u8],
) -> Result<Zeroizing<Vec<u8>>> {
    let c = XChaCha20Poly1305::new_from_slice(key).map_err(|_| Error::Crypto)?;
    let plain = c
        .decrypt(XNonce::from_slice(nonce), Payload { msg: ct, aad })
        .map_err(|_| Error::Corrupt)?;
    Ok(Zeroizing::new(plain))
}

/// 从主密钥派生用途隔离的子密钥（HKDF-SHA256）。
pub fn subkey(master: &[u8], salt: &[u8], info: &[u8]) -> Zeroizing<[u8; KEY_LEN]> {
    let hk = Hkdf::<Sha256>::new(Some(salt), master);
    let mut okm = Zeroizing::new([0u8; KEY_LEN]);
    hk.expand(info, &mut okm[..]).expect("hkdf expand");
    okm
}
