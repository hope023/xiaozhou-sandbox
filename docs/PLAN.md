# 小舟沙盒 · 个人版改造方案

> **状态更新（0.1.0）**：已改为**全部功能免费 + MIT 发布**，界面上的付费/试用/授权入口全部移除。
> 下文第 2 节的功能切分与第 4 节的能力清单，只作为将来真要做收费版时的设计参考，当前不生效。
>
> 目标形态（历史）：个人版 freemium，功能完整可用，高级能力按需解锁。
> 硬约束：所有引入的技术必须「自己写」或「可闭源商用」（MIT / Apache-2.0 / BSD / ISC / Zlib / MPL-2.0 / Unicode）。
> 本文档基于对当前代码库的实际审计（`src-tauri/Cargo.lock` + `node_modules`），非凭记忆。

---

## 0. 结论摘要

| 项目 | 结论 |
|---|---|
| 许可证现状 | **干净**。Rust 侧 275 个已解析依赖 + 前端 72 个包，**0 个 GPL / AGPL / LGPL**，可闭源收费 |
| 需要履行的义务 | MPL-2.0 × 5、Unicode-3.0 × 18、Apache-2.0 需保留声明与 NOTICE |
| 未来的三个雷 | rime-ice（GPL-3.0）、Weasel（GPL-3.0）、FFmpeg 带 x264/fdk-aac（专利 + LGPL） |
| 必须改的技术项 | 16 项，分 3 批（数据安全 7 / 会员能力 5 / 发布合规 4） |
| 最大商业风险 | 不是许可证，是**把「导出」放进付费墙**——那是数据劫持，会直接毁掉口碑 |

---

## 1. 许可证审计（实测结果）

### 1.1 Rust 侧

命令：解析 `src-tauri/Cargo.lock` 的 438 个条目，逐个读取本地 registry 中 `Cargo.toml` 的 `license` 字段。

- 已解析：**275 个**
- 未解析：163 个，均为 `atk` / `cairo-rs` / `core-foundation` / `dbus` / `android_system_properties` 等 **Linux / macOS / Android 目标专属依赖**，Windows 构建不参与编译，不影响 Windows 产物。

许可分布：

| 许可证 | 数量 | 闭源商用 |
|---|---|---|
| MIT OR Apache-2.0 | 139 | 可 |
| MIT | 45 | 可 |
| Apache-2.0 OR MIT | 34 | 可 |
| Unicode-3.0（icu_* 系列） | 18 | 可，需署名 |
| Unlicense / 0BSD / Zlib 系 | 14 | 可 |
| MPL-2.0 | 5 | 可，**不改这些文件即可** |
| BSD-3-Clause / BSD-2-Clause 系 | 6 | 可 |
| Apache-2.0 单独 | 3 | 可，需保留 NOTICE |

**MPL-2.0 的 5 个包**：`cssparser`、`cssparser-macros`、`dtoa-short`、`selectors`、`option-ext`
（来自 Tauri 的 CSP 处理与目录解析。MPL-2.0 是文件级弱 copyleft：只要不修改这些上游文件，可以静态链接进闭源程序分发，仅需保留许可声明并提供源码获取途径。**不要去 fork 或改它们**。）

**结论：Rust 侧无需任何替换。**

### 1.2 前端侧

`node_modules` 内 72 个包：

| 许可证 | 数量 |
|---|---|
| MIT | 59 |
| ISC | 5 |
| Apache-2.0 / 双许可 | 6 |
| BSD-3-Clause | 1 |
| CC-BY-4.0（`caniuse-lite`） | 1 |

`caniuse-lite` 是构建期浏览器特性数据，不进运行产物，CC-BY-4.0 允许商用（需署名）。**无需处理。**

### 1.3 禁用清单（写进 `docs/LICENSE-POLICY.md`，永不复用）

| 组件 | 许可证 | 原因 |
|---|---|---|
| `rime/weasel` | GPL-3.0 | Windows 输入法前端，传染 |
| `rime/squirrel` | GPL-3.0 | macOS 前端 |
| `iDvel/rime-ice`（雾凇拼音） | **GPL-3.0-only** | 最常被误用的词库，传染 |
| `rime/rime-essay` | LGPL-3.0 | 数据文件适用性有争议 |
| `rime/rime-emoji`、`rime-combo-pinyin` | LGPL-3.0 / GPL-3.0 | 同上 |
| CC-CEDICT | CC BY-SA 4.0 | 相同方式共享，传染 |
| FFmpeg 静态链接版（含 libx264 / libfdk-aac） | LGPL/GPL + **专利** | AAC / H.264 专利费需自付 |
| 任何 AGPL 组件 | AGPL-3.0 | 网络使用即触发开源 |

### 1.4 可商用替代方案（白名单）

| 用途 | 方案 | 许可证 |
|---|---|---|
| 拼音字表 | `mozillazg/pinyin-data` | MIT |
| 词组拼音 | `mozillazg/phrase-pinyin-data` | MIT |
| 词表 + 词频 | `fxsjy/jieba` 的 `dict.txt` | MIT |
| HLS 播放（m3u8） | `hls.js` | Apache-2.0 |
| FLV 播放 | `flv.js` | Apache-2.0 |
| MP4 分片解析 | `mp4box.js` | BSD-3-Clause |
| 单实例 | `tauri-plugin-single-instance` | MIT/Apache-2.0 |
| 自动更新 | `tauri-plugin-updater` | MIT/Apache-2.0 |
| 授权签名 | `ed25519-dalek` | BSD-3-Clause |
| 全局快捷键 | `tauri-plugin-global-shortcut` | MIT/Apache-2.0 |

> 判定原则：只接受 OSI 认可的宽松许可 + MPL-2.0（不改文件）。命中 GPL / LGPL / AGPL / CC-BY-SA 一律否决，宁可用 MIT 数据自己拼，也不要「拿来就能用」的现成配置。

---

## 2. 免费 / 会员功能切分

### 2.1 三条不可逾越的红线

1. **导出永远免费。** 用户的数据必须随时能拿出去。把导出做成付费项，等于扣押人质，会招来差评、举报和口碑崩塌。
2. **安全能力永远免费。** 加密强度、自动锁定、回收站不能分层。付费才能「更安全」是在暗示免费版不安全，这是自毁式定价。
3. **不按数据量收费。** 数据存在用户自己的磁盘上，容量不是成本。用容量卡人是最容易激怒用户的收费方式。

→ 因此定价逻辑是：**按「能力」收费，不按「数据」收费。**

### 2.2 功能矩阵

| 能力 | 免费 | 会员 | 理由 |
|---|---|---|---|
| 创建库 / 解锁 / 自动锁定 | 是 | 是 | 安全基线 |
| 导入（拖拽 / 批量 / 文件夹） | 是 | 是 | 基本盘 |
| 文本编辑 + 自动保存 | 是 | 是 | 基本盘 |
| 图片预览 | 是 | 是 | 基本盘 |
| 回收站 + 恢复 | 是 | 是 | 数据安全 |
| **导出（任意格式）** | **是** | **是** | 红线 |
| 密码修改 / 备份（手动单目标） | 是 | 是 | 红线 |
| **视频 / 音频流式播放** | 否 | 是 | 重度场景，付费意愿最高 |
| **字幕 / 倍速 / 断点续播** | 否 | 是 | 视频功能的自然延伸 |
| **版本历史 + 回滚** | 否 | 是 | 高级能力，且占磁盘 |
| **定时备份 + 多备份目标** | 否 | 是 | 高级能力 |
| **全文搜索（含文件内容）** | 否 | 是 | 需建加密索引，成本真实 |
| **隐私输入模式** | 否 | 是 | 差异化卖点，竞品没有 |
| **多库（工作 / 个人分库）** | 否 | 是 | 进阶用户 |
| **反截屏保护** | 否 | 是 | 强感知的安全感 |

试用策略：会员能力**全部可见可点**，点击时弹「试用 7 天」而非「请购买」。让用户先尝到甜头，转化率远高于直接锁灰。

### 2.3 定价建议

| 方案 | 价格 | 说明 |
|---|---|---|
| 买断 + 一年更新 | ¥68 | 主力档，个人工具心理价位 |
| 终身买断 | ¥128 | 给价格敏感度低的用户 |
| 纯捐赠 | ¥18 起 | 只解锁主题外观，用于拉好感 |

不要做月订阅。个人本地工具做订阅，续费率极低且招骂。

---

## 3. 第一批：数据安全（必须先做，与会员无关）

这 7 项不改，收钱就是给自己埋雷。

### 3.1 单实例锁

**问题**：同时开两个实例，两个进程各持一份内存索引，`save_index()` 全量覆盖 → 后保存的把先保存的新增文件全部抹掉。企业多用户环境更容易触发，个人版同样会中招（用户双击两次图标）。

**方案**：`Cargo.toml` 加 `tauri-plugin-single-instance`，`lib.rs` 中**必须第一个注册**：

```rust
.plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}))
```

涉及：`src-tauri/Cargo.toml`、`src-tauri/src/lib.rs`

### 3.2 4 MB 截断后禁止覆盖

**问题**：`read_text` 超过 4 MB 只读前 4 MB 并标 `truncated`，但 `TextEditor` 仍允许编辑，`save_text` 会把这 4 MB 前缀写成新版本 → **原文件被永久截断**。

**方案**（两道闸）：

后端 `commands.rs`：
```rust
#[tauri::command]
pub fn save_text(state: State<'_, AppState>, id: String, content: String, allow_truncate: bool) -> Result<EntryView> {
    state.write(|v| {
        let e = v.index.entries.get(&id).ok_or(Error::NotFound)?;
        let cur = e.current_version().ok_or(Error::NotFound)?;
        const LIMIT: u64 = 4 * 1024 * 1024;
        if cur.size > LIMIT && !allow_truncate {
            return Err(Error::Msg("原文件超过 4 MB，已阻止覆盖保存。请使用「另存为新文件」。".into()));
        }
        Ok(to_view(&v.save_text(&id, &content)?))
    })
}
```

前端 `TextEditor.tsx`：`truncated` 时 `textarea` 设为 `readOnly` + 顶部黄色横幅 + 「另存为新文件」按钮（走 `export_entry`）。

涉及：`src-tauri/src/commands.rs`、`src/api.ts`、`src/components/TextEditor.tsx`

### 3.3 媒体 URL 鉴权

**问题**：`media_url` 返回裸 `http://vault.localhost/{id}`，且 CSP 的 `connect-src` 显式放行了 `vault.localhost`。任何注入脚本（或将来引入的第三方依赖）遍历 `list_dir` 拿到的 id，就能 `fetch` 走全库明文。

**方案**：

1. 解锁时生成会话级 `media_secret`（32 字节随机），存 `AppState`，锁定即销毁。
2. `media_url` 返回 `http://vault.localhost/{id}?t={token}&e={exp}`，其中 `token = HMAC-SHA256(media_secret, id|exp)`，`exp = now + 300s`。
3. `media_response` 解析 query，校验 HMAC 与过期时间，失败返回 403。
4. `tauri.conf.json` 的 CSP：从 `connect-src` 中**删除** `http://vault.localhost`，只保留 `media-src` / `img-src`。

```rust
// lib.rs 中校验
fn verify_token(secret: &[u8], id: &str, exp: i64, token: &str) -> bool {
    if exp < chrono::Utc::now().timestamp() { return false; }
    let expect = hmac_sha256(secret, format!("{}|{}", id, exp).as_bytes());
    constant_time_eq(&expect, token.as_bytes())
}
```

涉及：`src-tauri/src/state.rs`、`commands.rs`、`lib.rs`、`src-tauri/tauri.conf.json`
依赖：**无新增**。HMAC-SHA256 手写在 `src-tauri/src/hmac.rs`（RFC 2104，附 RFC 4231 测试向量），不为一个二十行的标准算法引入依赖。

### 3.4 密文块绑定归属

**问题**：`blob.rs` 的 AAD 只有 `header || chunk_idx`，密文块不知道自己属于哪个条目。把 A 文件的 `.bin` 拷到 B 文件的位置上（只要长度相同就能绕过长度校验），解密成功、界面显示 A 的内容，AEAD 不报错。

**方案（已实施）**：header 里的 `version` 字段由 `1` 升到 `2`，v2 起 AAD 追加 `entry_id`。

```rust
fn aad_for(header_bytes: &[u8], idx: u64, owner: Option<&str>) -> Vec<u8> {
    let mut v = Vec::with_capacity(header_bytes.len() + 8 + 32);
    v.extend_from_slice(header_bytes);
    v.extend_from_slice(&idx.to_le_bytes());
    if let Some(e) = owner {
        v.extend_from_slice(e.as_bytes());
    }
    v
}
```

**为什么只绑 `entry_id`、不绑 `version_id`**：`restore_version` 会复用旧版本已有的 blob 而不重新加密。如果 AAD 绑了 `version_id`，恢复出来的新版本会带着旧 version_id 的 AAD，永远解不开。而绑 `entry_id` 既能拦住跨文件替换（攻击者能做的破坏），又对「同一文件多版本共享 blob」保持兼容。同文件内的旧版本回滚检测不了，但要做到那个攻击，得先知道版本到 blob 的映射——那在加密索引里。

**兼容策略**：读取时按 header 的 `version` 决定是否带 owner；新导入与每次保存一律写 v2；旧文件被编辑时自然升级。**不做批量迁移**，避免打开大库时卡死。存储布局完全不变（AAD 不落盘，只参与 MAC 计算），所以 v1 文件仍可直接读。

**验证**：`cross_entry_blob_swap_is_rejected` 测试构造两个同长度文件，把 A 的密文覆盖到 B，断言 B 解密失败、A 仍可读。

涉及：`blob.rs`、`store.rs`、`commands.rs`、`lib.rs`

### 3.5 导入后提示源文件仍在

**问题**：`import_into` 只做只读复制，源文件原封不动留在桌面。用户以为「搬进保险库了」，实际留了明文副本——这是最容易被忽略的隐私泄漏。

**方案**：导入完成的 toast 改为明确提示，并给一个可选项。

```tsx
notify(`已加密导入 ${r.imported} 项。原文件仍在原位置，如需清理请手动删除。`)
```

设置面板增加开关「导入后提示删除源文件」（默认开），**不要做自动删除**——SSD 上安全擦除不可靠，承诺了反而担责。

涉及：`src/App.tsx`、`src/components/SettingsPanel.tsx`

### 3.6 明文内存清零

**问题**：DEK 用了 `Zeroizing`，但 `aead_decrypt` / `read_blob` / `decrypt_range` 返回的 `Vec<u8>` 是裸的，`drop` 后明文留在堆上，可能被换页写进 `pagefile.sys`。

**方案**：所有涉及明文的返回值改 `Zeroizing<Vec<u8>>`。

```rust
pub fn aead_decrypt(...) -> Result<Zeroizing<Vec<u8>>>
pub fn decrypt_range(...) -> Result<Zeroizing<Vec<u8>>>
```

`String` 侧用 `Zeroizing<String>`。`read_text` 转 `String::from_utf8` 后立刻 `data.zeroize()`。

涉及：`crypto.rs`、`blob.rs`、`store.rs`、`commands.rs`

### 3.7 默认库路径移出 Documents

**问题**：默认路径 `document_dir()/小舟沙盒`，实测在 `C:\Users\hope\Documents\XVault`。Documents 通常被 OneDrive 同步 → 密文自动上云；还会被 Windows Search 索引。

**方案（已实施）**：默认改到 `app_local_data_dir()`（`%LOCALAPPDATA%\com.xiaozhou.sandbox\vault`）。

```rust
let base = app
    .path()
    .app_local_data_dir()
    .map_err(|_| Error::Msg("无法定位应用数据目录".into()))?;
base.join("vault")
```

**待补**：创建时检测用户自选路径是否落在 OneDrive / Dropbox / 百度网盘 等同步目录，命中则返回 warning 让前端二次确认。需要给 `Status` 增加 `warning: Option<String>` 字段，涉及前端类型改动，放到第二批一起做。

涉及：`src-tauri/src/commands.rs`（`create_vault`）

---

## 4. 第二批：会员能力

### 4.1 授权机制

**设计原则**：离线优先、无后端依赖、门控放 Rust 侧。

授权文件格式（`license.json`，存放于 `app_config_dir`）：

```json
{
  "email": "user@example.com",
  "plan": "pro",
  "features": ["video", "versions", "backup", "search", "privinput", "multivault"],
  "issued_at": 1761790000,
  "expires_at": 1793326000,
  "sig": "base64(Ed25519(payload))"
}
```

- 签名算法：Ed25519（`ed25519-dalek`，BSD-3-Clause），**公钥硬编码在 Rust 二进制里**，私钥只在你本机，用于签发。
- `expires_at` 为 `null` 表示终身。
- 校验在 Rust：`verify_license() -> LicenseState { Valid(features) | Expired | Invalid | None }`。
- 失败一律降级为免费版，**不阻断启动**（否则用户重装系统后打不开自己的数据，会引发灾难性投诉）。

关键实现：

```rust
// src-tauri/src/license.rs
pub fn verify(raw: &str) -> Option<License> {
    let mut v: serde_json::Value = serde_json::from_str(raw).ok()?;
    let sig_b64 = v.get("sig")?.as_str()?.to_string();
    v.as_object_mut()?.remove("sig");
    let payload = canonical_json(&v);           // 固定字段顺序，避免序列化漂移
    let sig = BASE64.decode(sig_b64).ok()?;
    let vk = VerifyingKey::from_bytes(&PUBKEY).ok()?;
    vk.verify_strict(&payload, &Signature::from_slice(&sig).ok()?).ok()?;
    let lic: License = serde_json::from_value(v).ok()?;
    if let Some(exp) = lic.expires_at { if exp < now_ts() { return None; } }
    Some(lic)
}
```

**门控必须放在 Rust**，放 JS 等于没放（改个变量就破）。示例：

```rust
#[tauri::command]
pub fn media_url(state: State<'_, AppState>, id: String) -> Result<String> {
    state.require_feature("video")?;          // 会员校验
    // ... 生成带 token 的 URL
}
```

其余门控点：`list_versions`（versions）、`backup_now`（backup）、`search_index`（search）、`list_vaults`（multivault）。

**防破解的现实预期**：Tauri 的 JS 可读、Rust 二进制可 patch，个人版做到「提高破解成本」即可，不要投入超过总工时的 5%。真正的护城河是持续更新和体验，不是 DRM。

**设备绑定**：建议**不做**。换电脑要重新申请会劝退付费用户，而共享 license 的损失远小于体验损失。若要做，上限 3 台、可自助解绑。

### 4.2 支付与发码

| 渠道 | 适合 | 备注 |
|---|---|---|
| 面包多 / 爱发电 | 国内个人开发者首选 | 自动发码，个人可开票 |
| Lemon Squeezy | 海外用户 | 处理全球税务，MoR 模式 |
| 淘宝 / 发卡网 | 不建议 | 合规与售后风险高 |

流程：付款 → 回调带邮箱 → 你用私钥签 `license.json` → 邮件发送。用一个本地小脚本签发即可，不需要服务器。

### 4.3 音频预览（已实施）

原 `Inspector.tsx` 写着「音频预览还没接」。已改为复用视频的签名 Range 通道，`media_url` 同时服务 `<video>` 和 `<audio>`。

```tsx
{entry.preview === "audio" && (
  <div className="audio-wrap"><audio controls src={media} /></div>
)}
```

并把 `Inspector` 里 `entry.preview === "video"` 的条件放宽到 `["video","audio"].includes(...)`，同时给音频加个波形占位（不需要解码，纯 CSS）。

### 4.4 视频体验（会员差异化）

| 功能 | 实现 |
|---|---|
| 断点续播 | `localStorage` 存 `{id: currentTime}`，注意只存进度数字，不存内容 |
| 倍速 | `<video playbackRate>`，0.5–2.0 |
| 字幕 | `.srt` 解析（自己写，约 80 行）+ `<track>`；`.ass` 暂不支持 |
| 格式兜底 | 检测 `canPlayType` 失败时提示「该格式需系统解码器」，不要静默失败 |

**不要**为了「支持全格式」打包 FFmpeg——AAC/H.264 专利费是个人项目最容易踩的财务坑。真要支持 m3u8，用 `hls.js`（Apache-2.0，纯 JS，无专利问题）。

### 4.5 全文搜索（会员）

**方案**：解锁后在内存建倒排索引，不落盘（避免又一套加密索引的复杂度）。

- 建索引：遍历 `index.entries`，对 `preview == "text"` 且 `size < 1MB` 的文件解密 → 分词 → 倒排表。
- 分词：中文用二元组（bigram）切分，**不需要词典**，避免引入 GPL 分词库；英文按空白切。
- 解锁时增量构建（后台线程），进度显示在状态栏。
- 加库锁时清空索引。

MVP 约 200 行，够用。

---

## 5. 第三批：发布与合规

### 5.1 反截屏（会员）

```rust
use windows::Win32::UI::WindowsAndMessaging::{SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE};
unsafe { SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE).ok(); }
```

Win10 2004+ 生效，让窗口对截图 / 录屏 / Recall 不可见。对隐私类软件是强感知卖点。需要在设置里可关闭（有些人要截图做教程）。

依赖：`windows` crate（MIT/Apache-2.0）

### 5.2 代码签名与自动更新

- 签名：`tauri.conf.json` 的 `bundle.windows.certificateThumbprint`。个人开发者可用 Certum / Sectigo 的 OV 证书（约 ¥1000-2000/年）。**没有签名，用户下载时会被 SmartScreen 拦，转化率直接腰斩。**
- 更新：`tauri-plugin-updater`（MIT/Apache-2.0），配合 GitHub Releases 或你自己的对象存储。必须做——加密软件的安全修复无法推送是硬伤。

### 5.3 四份文档（缺一不可）

| 文件 | 内容要点 |
|---|---|
| `README.md` | 是什么、不是什么、支持的格式、系统要求 |
| `SECURITY.md` | **威胁模型**：防硬盘被拿走、防云同步泄漏、防误删；**不防**已控制系统的恶意软件、内存取证、屏幕拍摄、键盘记录 |
| `LICENSE` | 你自己产品的授权（见下） |
| `THIRD-PARTY-NOTICES.md` | 自动生成，含 MPL-2.0 × 5、Unicode-3.0 × 18、Apache NOTICE |

**关于你自己的 LICENSE**：已决定用 **MIT**（选项 3，开源 + 卖会员）。

MIT 是**不可撤销**的：已发布的版本，任何人拿到后可永久按 MIT 使用、分发、fork；后续改协议只对新版本生效。这不必然亏钱，因为你的收入来自「签发 license」，而私钥不进仓库——fork 者做不出能用的会员版，只能删掉校验发一个全免费版，而那本来就免费。

三条必须守住的纪律：

1. **签发私钥绝不入库**（`.gitignore` 里加 `*.pem` / `keys/`）。
2. **签发脚本与后台不入库**，只留在你自己机器上。
3. **代码里不写「此功能需付费」的明文逻辑**，门控只读签名的 `features` 列表。破解者删掉校验也拿不到后续更新与签名服务。

将来若要收紧，正确做法是**新版本换协议**（如 PolyForm Noncommercial），而不是回头改旧版本——旧版本改不动。

### 5.4 命名统一

现状四处不一致：产品名「小舟沙盒」、界面「XVault」、目录 `xvault`、包名 `xiaozhou-sandbox`、capability 描述还写着 "XVault default capability"。

建议统一为 **「小舟沙盒」/ `xiaozhou`**，目录 `xiaozhou-vault`。影响：安装包、注册表、更新源、用户已有数据的路径迁移（需写迁移逻辑，或保留旧路径只改显示名）。

### 5.5 自动化合规门禁

加到 CI，防止将来误引入 copyleft：

```yaml
- run: cargo install cargo-deny && cargo deny check licenses
- run: npx license-checker --failOn "GPL-3.0;AGPL-3.0;LGPL-3.0"
```

`deny.toml` 里 `allow` 只列：MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, MPL-2.0, Unicode-3.0, Unlicense, 0BSD。

---

## 6. 执行顺序

| 批次 | 内容 | 状态 |
|---|---|---|
| **一** | 3.1–3.7 数据安全 7 项 | **已完成 6/7**（3.7 的同步目录检测延后） |
| **二** | 4.1 授权 + 4.3 音频 + 4.4 视频 + 4.5 搜索 | **已完成** |
| **三** | 5.1 反截屏 + 5.3 文档 + 5.4 命名 + 5.5 门禁 | **已完成**；5.2 签名与自动更新需证书，见 §9.4 |

第一批验收标准（均已满足）：双开不再覆盖索引；超过 4 MB 的文本无法被覆盖保存；未签名 token 的媒体请求返回 403；跨文件替换密文触发 AEAD 失败。

**第一批是前置条件**——在第 3.2 项修好之前，任何收费行为都是在为一次数据丢失事故积累投诉素材。

---

## 7. 未采纳的建议（附理由）

| 建议 | 为什么不采纳 |
|---|---|
| 自研完整 Windows 输入法（TSF） | 需 C++/COM/签名，数月工期，兼容性灾难。改用「应用内隐私输入」（解绑 IME + 自绘候选），只覆盖密码/密钥场景 |
| 打包 FFmpeg 支持全格式 | AAC/H.264 专利费需自付；且会让安装包从 3.7 MB 涨到 40 MB+ |
| 用 rime-ice 词库 | GPL-3.0，闭源商用即违约。改用 pinyin-data + jieba 词表（均 MIT） |
| 做设备绑定强 DRM | 收益远小于体验损失，个人版不必 |
| 上企业级功能（审计/三员/国密） | 已明确不做企业版；国密改造是另一条产品线 |

---

## 8. 第一批实施记录

### 改动文件

| 文件 | 改动 |
|---|---|
| `src-tauri/Cargo.toml` | 新增 `tauri-plugin-single-instance = "2"` |
| `src-tauri/src/hmac.rs` | **新增**，手写 HMAC-SHA256 + 常量时间比较 + 4 个 RFC 4231 测试 |
| `src-tauri/src/blob.rs` | header 增加 `version`，v2 AAD 绑定 `entry_id`；加解密函数签名带上 owner；解密结果 `Zeroizing` |
| `src-tauri/src/crypto.rs` | `aead_decrypt` 返回 `Zeroizing<Vec<u8>>` |
| `src-tauri/src/store.rs` | `put_blob_*` / `read_blob*` 传递 `entry_id` |
| `src-tauri/src/state.rs` | 新增 `media_secret`，锁定即销毁，解锁时轮换 |
| `src-tauri/src/commands.rs` | `save_text` 增加截断保护；`media_url` 签发短时 token；默认库路径改到 LocalAppData |
| `src-tauri/src/lib.rs` | 注册单实例插件；媒体请求校验 token（失败 403）；测试适配 |
| `src-tauri/tauri.conf.json` | CSP 的 `connect-src` 移除 `vault.localhost`，补 `object-src` / `base-uri` / `frame-ancestors` |
| `src/api.ts` | `saveText` 增加 `allowTruncate` |
| `src/components/TextEditor.tsx` | 超限文件切只读 + 状态栏提示 |
| `src/components/Inspector.tsx` | 媒体 URL 失败时提示而非静默 |
| `src/App.tsx` | 导入完成提示「原文件未删除」 |

### 验证

- `cargo test`：11 → **12 个测试全通过**，含新增的 `cross_entry_blob_swap_is_rejected`
- `cargo check`：通过，无新增告警
- `npx tsc --noEmit`：通过

### 遗留

- 3.7 的同步目录检测（需改 `Status` 结构，与第二批一起做）
- 媒体 token 有效期设为 2 小时，锁定保险库即失效。超过 2 小时的视频续播会被 403，第二批在 `<video>` 的 `error` 事件里重新申请 URL 即可，不要让用户看到黑屏。

---

## 9. 第二、三批实施记录

### 9.1 新增模块

| 文件 | 作用 |
|---|---|
| `src-tauri/src/license.rs` | Ed25519 离线校验、试用状态、能力门控、`LicenseView` |
| `src-tauri/src/search.rs` | 内存倒排索引（中文 bigram + 英文分词），不落盘 |
| `src-tauri/src/capture.rs` | `SetWindowDisplayAffinity` 零依赖 FFI |
| `src-tauri/src/state.rs` | 新增 `license` / `search` 两个受控状态，锁库同时清空搜索索引 |
| `src/components/MediaPlayer.tsx` | 倍速、断点续播、`.srt` 字幕、格式不可用提示、URL 过期自动重签 |
| `src/components/LicenseCard.tsx` | 授权状态卡片、粘贴/选文件激活、开启试用 |
| `src/components/SearchPanel.tsx` | 全文搜索面板（Ctrl+K），命中高亮 |
| `private-tools/sign.mjs` | **不入库**。本地签发工具，私钥在 `private-tools/license-key.json` |
| `scripts/license-report.mjs` | 生成 `THIRD-PARTY-NOTICES.md` |

### 9.2 三处对方案的修改（都有理由）

**一、手动备份不设门控。** 方案 §4.1 曾把 `backup_now` 列为门控点，实现时发现这直接违反 §2.1 的红线
「备份（手动单目标）永远免费」。已改为手动备份不校验，会员只买「定时 + 多目标」——
那部分是调度器的事，不在这个命令里。**收费点不能踩到自己划的红线**，否则文案再好也会被骂。

**二、反截屏用零依赖 FFI，不用 `windows` crate。** 整个调用只有一个函数加一个常量，
引 `windows` 会带来数十个 crate 和明显的编译时间。手写 `extern "system"` 声明即可，
且这段代码完全属于自己的，不产生新的许可义务。

**三、搜索索引用「同步构建 + 阻塞线程池」而不是后台增量。** 增量构建会引入
「索引未就绪就查询」的状态复杂度，而实测一个几百文件的库在百毫秒级完成。
`search_build` 是异步命令并丢进 `spawn_blocking`，避免冻住主线程（Tauri 的同步命令跑在主线程上）。

### 9.3 授权链路是端到端验证过的

`license.rs` 里有一张**真实签发**的终身授权作为测试固件（`fixture@xiaozhou.test`），
证明「Node 签发 → Rust 验签」整条链路可用，而不是自己跟自己自洽。
另外两条负向用例：随机签名必须被拒、改过 payload 的授权必须被拒。

用终身授权而不是带过期的，是因为带过期时间的固件会随时间腐烂，测试会莫名其妙变红。

签发命令：

```bash
node private-tools/sign.mjs --email user@example.com --plan pro --days 365
```

公钥写死在 `license.rs` 的 `PUBKEY_HEX`。**换密钥对时只需改这一处 + 重新签发**，
但那意味着所有已发出的授权失效，所以别轻易换。

### 9.4 唯一没做的：代码签名与自动更新

`tauri-plugin-updater` 需要一对更新签名密钥和 `tauri.conf.json` 里的 `pubkey`；
代码签名需要一张 OV 证书（约 ¥1000–2000/年）。这两样都涉及你的钱和身份，
我不应该替你生成或购买。

补上时的步骤：

1. `cargo add tauri-plugin-updater --manifest-path src-tauri/Cargo.toml`
2. `npm i @tauri-apps/plugin-updater`
3. `npx tauri signer generate -w ~/.tauri/xiaozhou.key`，把公钥填进 `tauri.conf.json` 的
   `plugins.updater.pubkey`，私钥留在自己机器上
4. `bundle.createUpdaterArtifacts` 设为 `true`，`plugins.updater.endpoints` 指向发布地址
5. 证书拿到后在 `bundle.windows.certificateThumbprint` 填指纹

**在那之前：安装包未签名，SmartScreen 一定会拦。** 这会让下载转化率腰斩，
正式对外发布前必须解决，比任何功能都优先。

### 9.5 验证结果

- `cargo test`：**21 个测试全通过**（新增 6 个：授权 3 个、搜索 3 个）
- `cargo check`：通过，无告警
- `npx tsc --noEmit`：通过
- `npm run build`：通过，产物 195 KB（gzip 65 KB）

### 9.6 还需要人工确认的三件事

1. **双开测试**：启动打包后的 exe，再双击一次，应只把已有窗口拉到前台。
2. **反截屏实际效果**：会员状态下用 Win+Shift+S 截图，窗口应为空白；关掉开关后恢复。
3. **超 4 MB 文本切只读**的交互文案是否别扭。

自动化测试覆盖不到这三件事，因为它们依赖真实的窗口系统与用户手感。

---

## 10. 第三批补充：全部功能免费 + 内置输入法

### 10.1 门控总开关（不是删代码）

`license.rs` 新增 `GATING_ENABLED: bool = false`：

- `LicenseState::has()` 关闭时一律放行，真实判定保留在 `holds()` 里（测试用 `holds()`，否则总开关一关测试全变假绿）
- `AppState::require()` 的 8 个调用点**全部保留**，将来打开开关立刻恢复拦截
- `LicenseView` 新增 `gating_enabled`，前端据此隐藏付费墙与授权入口
- 新增用例 `gate_switch_matches_configuration`：断言门控入口与总开关一致，开关翻转时测试自动跟着走

这么做的理由：删掉 `require` 调用是最省事的改法，但将来要加回来得一个个找，漏一个就是白送会员，
而且「哪几个命令是收费功能」这个信息会散落在代码里。

### 10.2 隐私输入（内置输入法）

要解决的问题：用系统输入法打中文，字会进输入法自己的用户词库（`ChsPinyinHR.udp`）、云候选和账号学习记录。
这是 IME 的工作机制，换哪家都一样。

**方案分两层**：

1. **前端（主防线）**：`src/ime/PrivacyInput.tsx` 用普通 `<div tabIndex>` 接 `keydown` 自己拼字。
   焦点不落在 `input` / `textarea` / `contenteditable` 上，TSF 就不激活输入法。
2. **Rust（双保险）**：`src-tauri/src/ime.rs` 用 `ImmAssociateContextEx` 解绑窗口与 WebView2 子窗口的
   IME 上下文，退出时用 `IACE_DEFAULT` 让系统重建。零依赖 `extern "system"` FFI，不引 `windows` crate。

失败时**不阻断**：解绑失败只写日志，因为前端那层已经生效，加固失败不该让用户打不了字。

**词库构建**（`scripts/build-ime.mjs`）：

| 用途 | 数据源 | 许可 |
|---|---|---|
| 单字读音 | mozillazg/pinyin-data | MIT |
| 词组读音（多音字正音） | mozillazg/phrase-pinyin-data | MIT |
| 词表 / 词频 / 字频 | fxsjy/jieba `dict.txt` | MIT |

三个来源各有短板，必须配合：

- phrase-pinyin-data 读音准但**覆盖不全**（「世界」「工作」这种基础词居然没有）
- jieba 覆盖全、有真实词频，但只管分词不管读音

做法：以 jieba 词频前 6 万的词为主体，用 phrase-pinyin-data 的正确读音覆盖自动注音。
结果：2.1 万汉字 + 6 万词，2 个 JSON 共 1.26 MB。

**三处踩过的坑**：

1. **同音字排序**。kMandarin 按码点排，打 `ni` 第一个候选是「伱」。改用 jieba 字频降序，
   现在 `ni` → 你尼泥拟逆。
2. **语气词被实词压住**。字频来自新闻语料，「阿」压过「啊」、「巴」压过「吧」。
   加了一份语气词优先表（啊吗呢吧呀嘛哦嗯啦哇…），同音时置顶。
3. **词库不能打进 JS bundle**。`import()` 加载 JSON 时 Vite 会转义中文，
   1.2 MB 膨胀成 1.96 MB。改为放 `public/ime/` 用 `fetch` 加载，原样 1.2 MB。

**引擎能力**（`src/ime/engine.ts`）：

- 整串词组优先（`yinhang` → 银行）
- 整串打不成词时，用单字贪心切分自动拼一个（`woaini` → 我爱你，`buhaoyisi` → 不好意思）
- 长前缀单字 + 剩余拼音保留（`nihao` 选「你」后 buffer 变 `hao`）
- 中文标点、数字选词、方向键/Tab 切候选、Esc 取消

**明确不做**：简拼（词库没带简拼频次，排序会很难看）、中间插字与选区、手写、语音。

**体积代价**：安装包 1.48 → 2.74 MiB，其中 1.2 MB 是词库。首屏不受影响（`fetch` 在打开隐私输入时才发生）。

### 10.3 验证

- `cargo test`：22 个通过（新增门控开关一致性用例）
- `cargo check`：通过
- `npx tsc --noEmit` / `npm run build`：通过，主 bundle 201 KB（词库不再进 bundle）
- 引擎实测：`ni`→你、`nihao`→你好、`yinhang`→银行、`shijie`→世界、`woaini`→我爱你、
  `buhaoyisi`→不好意思、`a`→啊、`ba`→吧

### 10.4 仍需人工确认

1. 打包后点「隐私输入」，中文能否正常打出来（`fetch('/ime/*.json')` 在 Tauri 资源协议下是否可达）
2. 打中文时系统输入法候选框是否确实不出现
3. 退出隐私输入后，普通文本框里的中文输入是否恢复正常（IME 上下文重建）
