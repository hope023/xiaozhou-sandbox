//! 离线授权。
//!
//! 设计要点：
//!
//! 1. **签名覆盖的是原始字节，不是「重新序列化后的结构体」。**
//!    授权文件形如 `{ "payload": "<base64url>", "sig": "<base64url>" }`，
//!    签名直接针对 `payload` 解码出的那串字节。这样就不存在
//!    「两边 JSON 字段顺序 / 空格不同导致验签失败」的问题——签发端
//!    只负责把一段字节签掉，客户端只负责校验同一段字节。
//!
//! 2. **公钥硬编码在二进制里，私钥永远不进来。**
//!    私钥在 `private-tools/license-key.json`（已 gitignore）。这意味着
//!    代码即使是 MIT、即使被 fork，别人也做不出能通过校验的新授权。
//!
//! 3. **校验失败一律降级为免费版，绝不阻断启动。**
//!    否则用户重装系统后打不开自己的数据，那是灾难性投诉，比盗版严重得多。

use std::path::{Path, PathBuf};

use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::store::now_ts;

/// 签发公钥（Ed25519，32 字节 hex）。私钥在 `private-tools/license-key.json`。
const PUBKEY_HEX: &str = "50fbda84b715bacf3f7f715e6c6561ff9c105b464232c65d3ef4a9d378384319";

/// 试用天数。
pub const TRIAL_DAYS: i64 = 7;

/// 所有受门控的能力。门控只认这个列表，代码里不出现散装的能力判断。
pub const ALL_FEATURES: [&str; 7] = [
    "video",     // 视频 / 音频流式播放、倍速、字幕、断点续播
    "versions",  // 版本历史与回滚
    "backup",    // 定时备份 / 多目标
    "search",    // 全文搜索
    "privinput", // 隐私输入模式（后续批次）
    "multivault", // 多库
    "noscreen",  // 反截屏
];

/// **门控总开关。**
///
/// 当前阶段所有功能免费，置 `false`：`LicenseState::has()` 一律放行，
/// `AppState::require()` 恒为 `Ok`，界面也不出现任何授权入口。
///
/// 将来真要按能力区分授权时把它改回 `true` 即可——所有门控点立刻恢复拦截，
/// 不需要改其它任何一行。授权、签发、验签这套设施全部保持完好，
/// 只是暂时不参与决策。
///
/// 之所以做成「总开关」而不是「删掉 require 调用」：删掉之后再加回来，
/// 得把 8 个命令的门控点一个个找回来，漏一个就等于门控失效；而且
/// 「哪几个命令受门控」这个信息会散落在代码里，迟早对不上。
pub const GATING_ENABLED: bool = false;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct License {
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub plan: String,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub issued_at: i64,
    /// `None` 表示终身授权。
    #[serde(default)]
    pub expires_at: Option<i64>,
}

#[derive(Serialize, Clone, Debug)]
pub struct LicenseView {
    /// `none` | `trial` | `valid` | `expired` | `invalid`
    pub status: String,
    pub plan: String,
    pub email: String,
    pub features: Vec<String>,
    pub expires_at: Option<i64>,
    /// 试用结束时间；0 表示从未开始试用。
    pub trial_ends_at: i64,
    pub trial_available: bool,
    /// 给用户看的一句话，比如签名不匹配的原因。
    pub detail: String,
    /// 门控总开关的当前值。`false` 表示全部功能免费。
    /// 当前界面不读这个字段——前端已经完全没有任何授权界面，保留它只是为了
    /// 将来开回 `true` 时能一次性接上。
    pub gating_enabled: bool,
}

#[derive(Default)]
pub struct LicenseState {
    lic: Option<License>,
    /// 解析/验签失败时的原因，用于在界面上解释，不上报、不联网。
    detail: String,
    trial_started: i64,
}

fn b64decode(s: &str) -> Option<Vec<u8>> {
    let t = s.trim();
    URL_SAFE_NO_PAD
        .decode(t)
        .or_else(|_| STANDARD.decode(t))
        .ok()
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    let b = s.as_bytes();
    if b.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(b.len() / 2);
    for i in (0..b.len()).step_by(2) {
        let hi = (b[i] as char).to_digit(16)?;
        let lo = (b[i + 1] as char).to_digit(16)?;
        out.push((hi * 16 + lo) as u8);
    }
    Some(out)
}

/// 验签并解析。失败返回 `None`，原因写进第二个返回值。
pub fn verify(raw: &str) -> std::result::Result<License, String> {
    #[derive(Deserialize)]
    struct Envelope {
        payload: String,
        sig: String,
    }

    let env: Envelope =
        serde_json::from_str(raw).map_err(|_| "授权文件格式不对，应为 license.json".to_string())?;
    let payload = b64decode(&env.payload).ok_or("授权文件里的 payload 不是合法 base64")?;
    let sig_bytes = b64decode(&env.sig).ok_or("授权文件里的 sig 不是合法 base64")?;
    if sig_bytes.len() != 64 {
        return Err("签名长度不对".into());
    }

    let pk = unhex(PUBKEY_HEX).ok_or("内置公钥损坏")?;
    let vk = VerifyingKey::from_bytes(
        <&[u8; 32]>::try_from(pk.as_slice()).map_err(|_| "内置公钥长度不对")?,
    )
    .map_err(|_| "内置公钥无效")?;

    let sig = Signature::from_slice(&sig_bytes).map_err(|_| "签名格式不对")?;
    vk.verify_strict(&payload, &sig)
        .map_err(|_| "签名校验失败：这份授权不是本产品签发的，或内容被改过".to_string())?;

    let lic: License = serde_json::from_slice(&payload).map_err(|_| "授权内容解析失败")?;
    Ok(lic)
}

impl LicenseState {
    pub fn load(config_dir: &Path) -> Self {
        let mut st = Self::default();
        let p = license_path(config_dir);
        if let Ok(raw) = std::fs::read_to_string(&p) {
            match verify(&raw) {
                Ok(l) => st.lic = Some(l),
                Err(e) => st.detail = e,
            }
        }
        st
    }

    pub fn set_trial_started(&mut self, ts: i64) {
        self.trial_started = ts;
    }

    pub fn activate(&mut self, raw: &str) -> crate::error::Result<LicenseView> {
        let lic = verify(raw).map_err(crate::error::Error::Msg)?;
        self.lic = Some(lic);
        self.detail.clear();
        Ok(self.view())
    }

    pub fn clear(&mut self) {
        self.lic = None;
        self.detail.clear();
    }

    fn trial_ends_at(&self) -> i64 {
        if self.trial_started > 0 {
            self.trial_started + TRIAL_DAYS * 86400
        } else {
            0
        }
    }

    pub fn trial_active(&self) -> bool {
        let end = self.trial_ends_at();
        end > 0 && now_ts() < end
    }

    pub fn status(&self) -> &'static str {
        if let Some(l) = &self.lic {
            if let Some(exp) = l.expires_at {
                if exp <= now_ts() {
                    return "expired";
                }
            }
            return "valid";
        }
        if !self.detail.is_empty() {
            return "invalid";
        }
        if self.trial_active() {
            return "trial";
        }
        "none"
    }

    /// 真实的能力判定，**不受 [`GATING_ENABLED`] 影响**。
    ///
    /// 门控关闭时 `has()` 会直接返回 true，所以判断「这份授权到底覆盖了什么」
    /// 只能看这里。测试断言也用它，否则总开关一关所有用例就变成假绿。
    pub fn holds(&self, feature: &str) -> bool {
        // 开发构建可用环境变量全解锁，便于调试；release 构建里这段代码不存在。
        #[cfg(debug_assertions)]
        if std::env::var("XIAOZHOU_DEV_UNLOCK").ok().as_deref() == Some("1") {
            return true;
        }

        match self.status() {
            "valid" => self
                .lic
                .as_ref()
                .map(|l| l.features.iter().any(|f| f == feature))
                .unwrap_or(false),
            "trial" => true,
            _ => false,
        }
    }

    /// 门控入口。所有命令都必须走这个函数，别自己拼判断。
    pub fn has(&self, feature: &str) -> bool {
        if !GATING_ENABLED {
            return true;
        }
        self.holds(feature)
    }

    pub fn view(&self) -> LicenseView {
        let status = self.status().to_string();
        let (plan, email, features, expires_at, detail) = match &self.lic {
            Some(l) => {
                if status == "expired" {
                    (
                        l.plan.clone(),
                        l.email.clone(),
                        Vec::new(),
                        l.expires_at,
                        "授权已过期，续费后重新导入授权文件即可".to_string(),
                    )
                } else {
                    (
                        l.plan.clone(),
                        l.email.clone(),
                        l.features.clone(),
                        l.expires_at,
                        String::new(),
                    )
                }
            }
            None => {
                let d = if self.detail.is_empty() {
                    if self.trial_active() {
                        "试用中".to_string()
                    } else if self.trial_started > 0 {
                        "试用已结束".to_string()
                    } else {
                        "未激活".to_string()
                    }
                } else {
                    self.detail.clone()
                };
                (
                    if self.trial_active() { "试用".to_string() } else { String::new() },
                    String::new(),
                    if self.trial_active() {
                        ALL_FEATURES.iter().map(|s| s.to_string()).collect()
                    } else {
                        Vec::new()
                    },
                    None,
                    d,
                )
            }
        };

        // 门控关闭（当前阶段）时把能力铺满、改写提示语，界面上不再出现任何
        // 「这项功能要单独授权」的痕迹。`status` 仍保留真实值，方便排查授权文件本身的问题。
        let (features, detail) = if GATING_ENABLED {
            (features, detail)
        } else {
            (
                ALL_FEATURES.iter().map(|s| s.to_string()).collect(),
                "当前版本全部功能免费，无需授权".to_string(),
            )
        };

        LicenseView {
            status,
            plan,
            email,
            features,
            expires_at,
            trial_ends_at: self.trial_ends_at(),
            trial_available: self.trial_started == 0,
            detail,
            gating_enabled: GATING_ENABLED,
        }
    }
}

pub fn license_path(config_dir: &Path) -> PathBuf {
    config_dir.join("license.json")
}

/// 把用户粘贴的授权文件规范化后落盘。允许用户直接粘贴带换行的 JSON。
pub fn save(config_dir: &Path, raw: &str) -> crate::error::Result<()> {
    std::fs::create_dir_all(config_dir)?;
    let v: serde_json::Value = serde_json::from_str(raw)
        .map_err(|_| crate::error::Error::Msg("粘贴的内容不是合法 JSON".into()))?;
    std::fs::write(license_path(config_dir), serde_json::to_vec_pretty(&v)?)?;
    Ok(())
}

/// 中文 + 英文混排的能力名，用于报错文案。
pub fn feature_label(f: &str) -> &'static str {
    match f {
        "video" => "视频 / 音频播放",
        "versions" => "版本历史",
        "backup" => "备份",
        "search" => "全文搜索",
        "privinput" => "隐私输入",
        "multivault" => "多库",
        "noscreen" => "反截屏保护",
        _ => "该功能",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_garbage() {
        assert!(verify("not json").is_err());
        assert!(verify(r#"{"payload":"AAAA","sig":"AAAA"}"#).is_err());
    }

    #[test]
    fn unsigned_license_is_rejected() {
        // 构造一个内容合法但签名是随机字节的授权，必须被拒。
        let payload = URL_SAFE_NO_PAD.encode(
            br#"{"email":"a@b.c","plan":"pro","features":["video"],"issued_at":1}"#,
        );
        let sig = URL_SAFE_NO_PAD.encode([7u8; 64]);
        let raw = format!(r#"{{"payload":"{}","sig":"{}"}}"#, payload, sig);
        let e = verify(&raw).unwrap_err();
        assert!(e.contains("签名校验失败"), "实际错误：{e}");
    }

    /// 真实签发的终身授权（fixture@xiaozhou.test），用来证明
    /// 「Node 签发 → Rust 验签」这条链路是通的，而不是自己跟自己自洽。
    /// 用终身授权是因为带过期的固件会随时间腐烂，测试会莫名其妙变红。
    const FIXTURE: &str = r#"{
      "payload": "eyJlbWFpbCI6ImZpeHR1cmVAeGlhb3pob3UudGVzdCIsInBsYW4iOiJwcm8iLCJmZWF0dXJlcyI6WyJ2aWRlbyIsInNlYXJjaCJdLCJpc3N1ZWRfYXQiOjE3OTMzNzAwNjgsImV4cGlyZXNfYXQiOm51bGx9",
      "sig": "uRmAQ5OouQtOeXHc9ZzG3F2iD9yLRpw94GBEiS4hPyyM8X2eLOxrNYKs1C_uCvMEFyJX5Hg_pWj6UP_LO3K3Ag"
    }"#;

    #[test]
    fn valid_fixture_unlocks_only_its_features() {
        let lic = verify(FIXTURE).expect("签名应当通过");
        assert_eq!(lic.email, "fixture@xiaozhou.test");
        assert!(lic.expires_at.is_none(), "终身授权不应带过期时间");

        let mut st = LicenseState::default();
        st.lic = Some(lic);
        assert_eq!(st.status(), "valid");
        // 断言用 holds()，不用 has()：后者受 GATING_ENABLED 影响，
        // 总开关一关就恒真，测试会变成假绿，等于什么都没测。
        assert!(st.holds("video"));
        assert!(st.holds("search"));
        assert!(!st.holds("versions"), "授权里没写的能力不能给");
    }

    #[test]
    fn tampered_payload_is_rejected() {
        // 把 payload 换掉但保留原签名：内容与签名不再匹配
        let forged = r#"{
          "payload": "eyJlbWFpbCI6ImZpeHR1cmVAeGlhb3pob3UudGVzdCIsInBsYW4iOiJwcm8iLCJmZWF0dXJlcyI6WyJ2aWRlbyIsInNlYXJjaCIsInZlcnNpb25zIl0sImlzc3VlZF9hdCI6MTc5MzM3MDA2OCwiZXhwaXJlc19hdCI6bnVsbH0",
          "sig": "uRmAQ5OouQtOeXHc9ZzG3F2iD9yLRpw94GBEiS4hPyyM8X2eLOxrNYKs1C_uCvMEFyJX5Hg_pWj6UP_LO3K3Ag"
        }"#;
        assert!(verify(forged).is_err(), "改过 payload 的授权必须被拒");
    }

    #[test]
    fn none_state_has_no_features() {
        let st = LicenseState::default();
        assert_eq!(st.status(), "none");
        assert!(!st.holds("video"));
        assert!(st.view().trial_available);
    }

    #[test]
    fn trial_grants_everything_for_seven_days() {
        let mut st = LicenseState::default();
        st.set_trial_started(now_ts() - 86400);
        assert_eq!(st.status(), "trial");
        assert!(st.holds("search"));

        let mut expired = LicenseState::default();
        expired.set_trial_started(now_ts() - (TRIAL_DAYS + 1) * 86400);
        assert_eq!(expired.status(), "none");
        assert!(!expired.holds("search"));
        assert!(!expired.view().trial_available, "试用只能用一次");
    }

    /// 总开关和门控入口必须始终一致。将来把 `GATING_ENABLED` 改回 `true`，
    /// 这条用例会自动跟着翻转，不会留下一句「注释说全免费、代码其实在收费」
    /// 或者「以为在收费、其实全放开」的错位。
    #[test]
    fn gate_switch_matches_configuration() {
        let st = LicenseState::default(); // 未授权
        assert!(!st.holds("video"), "未授权时真实能力应为零");

        assert_eq!(st.has("video"), !GATING_ENABLED, "门控入口必须跟随总开关");

        let v = st.view();
        assert_eq!(v.gating_enabled, GATING_ENABLED);
        if GATING_ENABLED {
            assert!(v.features.is_empty(), "门控开着时不该白送能力");
        } else {
            assert_eq!(v.features.len(), ALL_FEATURES.len(), "门控关着时应铺满全部能力");
            assert!(v.detail.contains("免费"), "提示语要说明当前是免费版");
        }
    }
}
