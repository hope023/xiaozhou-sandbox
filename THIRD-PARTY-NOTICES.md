# 第三方组件与许可声明

本文件由 `node scripts/license-report.mjs` 生成，请勿手工编辑。

生成时间：2026-10-30

小舟沙盒自身以 MIT 许可发布，见 `LICENSE`。以下是随二进制分发的第三方组件。

## 许可分布

| 侧 | 组件数 | 许可证分布 |
|---|---|---|
| Rust（Windows 产物实际编译） | 286 | MIT OR Apache-2.0 × 139、MIT × 46、Apache-2.0 OR MIT × 41、Unicode-3.0 × 18、Unlicense OR MIT × 8、MIT/Apache-2.0 × 6、BSD-3-Clause × 5、MPL-2.0 × 5、Zlib × 2、MIT OR Zlib OR Apache-2.0 × 2、MIT OR Apache-2.0 OR Zlib × 2、Unlicense/MIT × 2、0BSD OR MIT OR Apache-2.0 × 1、BSD-3-Clause AND MIT × 1、BSD-3-Clause/MIT × 1、Apache-2.0 AND MIT × 1、CC0-1.0 OR MIT-0 OR Apache-2.0 × 1、Apache-2.0 / MIT × 1、Apache-2.0 × 1、Zlib OR Apache-2.0 OR MIT × 1、(MIT OR Apache-2.0) AND Unicode-3.0 × 1、BSD-2-Clause OR Apache-2.0 OR MIT × 1 |
| 前端（随产物打包） | 7 | MIT × 5、Apache-2.0 OR MIT × 1、MIT OR Apache-2.0 × 1 |

另有 200 个 Rust 依赖与 122 个前端开发依赖：前者只服务于 Linux / macOS / Android 构建目标，Windows 产物不编译它们；后者只在构建期运行，不进安装包。两者都不构成随二进制分发的义务，清单见文末。

## 需要履行义务的许可证

- **MIT / ISC / BSD / Apache-2.0 / Zlib / 0BSD / Unlicense**：保留版权声明即可。
- **Apache-2.0**：保留声明，若上游附带 NOTICE 文件需一并保留。
- **MPL-2.0**（`cssparser`、`cssparser-macros`、`dtoa-short`、`selectors`、`option-ext`）：
  文件级弱 copyleft。本项目未修改这些文件，仅静态链接使用，因此可以闭源分发；
  对应源码可从 crates.io 获取。**不要 fork 或修改它们**，否则需要公开修改部分。
- **Unicode-3.0**（`icu_*` 系列）：保留版权与许可声明。

## 明确未使用的组件

以下组件与本项目功能相似，但因许可证不可闭源商用或存在专利风险而**刻意未引入**：

| 组件 | 许可证 / 风险 | 替代方案 |
|---|---|---|
| Weasel（Rime 的 Windows 前端） | GPL-3.0，传染 | 不使用 |
| rime-ice（雾凇拼音词库） | GPL-3.0-only，传染 | `pinyin-data`（MIT） |
| FFmpeg + libx264 / libfdk-aac | LGPL + H.264/AAC 专利池 | 使用系统解码器 / `hls.js`（Apache-2.0） |

## Rust 依赖

### MIT OR Apache-2.0（139）

`aead@0.5.2`、`anyhow@1.0.104`、`argon2@0.5.3`、`base64@0.22.1`、`base64@0.23.1`、`bitflags@2.13.2`、`blake2@0.10.6`、`block-buffer@0.10.4`、`camino@1.2.6`、`cargo-platform@0.1.9`、`cc@1.10.0`、`cfg-if@1.0.5`、`chrono@0.4.45`、`cipher@0.4.4`、`cookie@0.18.2`、`cpufeatures@0.2.17`、`crc32fast@1.5.3`、`crossbeam-channel@0.5.17`、`crossbeam-utils@0.8.23`、`crypto-common@0.1.7`、`defmt@1.2.0`、`defmt-macros@1.2.0`、`defmt-parser@1.1.0`、`deranged@0.5.9`、`digest@0.10.7`、`dirs@7.0.0`、`dirs-sys@0.5.0`、`displaydoc@0.2.7`、`dtoa@1.0.11`、`dyn-clone@1.0.20`、`erased-serde@0.4.11`、`fdeflate@0.3.7`、`find-msvc-tools@0.1.15`、`flate2@1.1.10`、`form_urlencoded@1.2.2`、`getrandom@0.2.17`、`getrandom@0.3.4`、`getrandom@0.4.3`、`glob@0.3.4`、`hashbrown@0.12.3`、`hashbrown@0.17.1`、`heck@0.5.0`、`hex@0.4.3`、`hkdf@0.12.4`、`hmac@0.12.1`、`html5ever@0.39.0`、`http@1.5.0`、`idna@1.1.0`、`inout@0.1.4`、`itoa@1.0.18`、`jsonptr@0.7.1`、`keyboard-types@0.8.3`、`libc@0.2.191`、`lock_api@0.4.14`、`log@0.4.34`、`markup5ever@0.39.0`、`mime@0.3.17`、`num-conv@0.2.2`、`num-traits@0.2.19`、`once_cell@1.21.4`、`opaque-debug@0.3.1`、`parking_lot@0.12.5`、`parking_lot_core@0.9.12`、`password-hash@0.5.0`、`percent-encoding@2.3.2`、`png@0.18.1`、`powerfmt@0.2.1`、`ppv-lite86@0.2.21`、`proc-macro2@1.0.107`、`quote@1.0.47`、`rand@0.8.8`、`rand_chacha@0.3.1`、`rand_core@0.6.4`、`ref-cast@1.0.27`、`ref-cast-impl@1.0.27`、`regex@1.13.1`、`regex-automata@0.4.18`、`regex-syntax@0.8.11`、`rustc_version@0.4.1`、`scopeguard@1.2.0`、`semver@1.0.28`、`serde@1.0.229`、`serde_core@1.0.229`、`serde_derive@1.0.229`、`serde_derive_internals@0.29.1`、`serde_json@1.0.154`、`serde_repr@0.1.21`、`serde_spanned@1.1.2`、`serde_with@3.24.0`、`serde_with_macros@3.24.0`、`serde-untagged@0.1.9`、`serialize-to-javascript@0.1.2`、`serialize-to-javascript-impl@0.1.2`、`servo_arc@0.4.3`、`sha2@0.10.9`、`shlex@2.0.1`、`siphasher@1.0.4`、`smallvec@1.16.4`、`socket2@0.6.5`、`softbuffer@0.4.8`、`stable_deref_trait@1.2.1`、`string_cache@0.9.0`、`string_cache_codegen@0.6.1`、`syn@2.0.119`、`syn@3.0.9`、`tendril@0.5.1`、`thiserror@1.0.69`、`thiserror@2.0.21`、`thiserror-impl@1.0.69`、`thiserror-impl@2.0.21`、`time@0.3.55`、`time-core@0.1.9`、`time-macros@0.2.32`、`toml@1.1.8+spec-1.1.0`、`toml_datetime@1.1.2+spec-1.1.0`、`toml_parser@1.1.5+spec-1.1.0`、`toml_writer@1.1.3+spec-1.1.0`、`tray-icon@0.25.1`、`typeid@1.0.3`、`typenum@1.20.1`、`unicase@2.10.0`、`unicode-segmentation@1.13.3`、`universal-hash@0.5.1`、`url@2.5.8`、`web_atoms@0.2.6`、`web-time@1.1.0`、`windows@0.62.2`、`windows-collections@0.3.2`、`windows-core@0.62.2`、`windows-future@0.3.2`、`windows-implement@0.60.2`、`windows-interface@0.59.3`、`windows-link@0.2.1`、`windows-numerics@0.3.1`、`windows-result@0.4.1`、`windows-strings@0.5.1`、`windows-sys@0.61.2`、`windows-threading@0.2.1`、`windows-version@0.1.7`

### MIT（46）

`bytes@1.12.1`、`cargo_metadata@0.19.2`、`cfb@0.14.0`、`darling@0.24.1`、`darling_core@0.24.1`、`darling_macro@0.24.1`、`derive_more@2.1.1`、`derive_more-impl@2.1.1`、`dom_query@0.28.0`、`embed-resource@3.0.12`、`generic-array@0.14.7`、`ico@0.6.0`、`infer@0.22.0`、`mime_guess@2.0.5`、`mio@1.2.4`、`new_debug_unreachable@1.0.6`、`phf@0.13.1`、`phf_codegen@0.13.1`、`phf_generator@0.13.1`、`phf_macros@0.13.1`、`phf_shared@0.13.1`、`plist@1.10.3`、`precomputed-hash@0.1.1`、`quick-xml@0.42.0`、`rfd@0.17.2`、`schemars@0.8.22`、`schemars@0.9.0`、`schemars@1.2.3`、`schemars_derive@0.8.22`、`simd-adler32@0.3.10`、`strsim@0.11.1`、`synstructure@0.14.0`、`tauri-winres@0.3.7`、`tokio@1.54.1`、`tracing@0.1.44`、`tracing-attributes@0.1.31`、`tracing-core@0.1.36`、`urlpattern@0.6.0`、`vswhom@0.1.0`、`vswhom-sys@0.1.3`、`webview2-com@0.39.1`、`webview2-com-macros@0.8.1`、`webview2-com-sys@0.39.1`、`winnow@1.0.4`、`winreg@0.56.0`、`zmij@1.0.23`

### Apache-2.0 OR MIT（41）

`autocfg@1.5.1`、`base64ct@1.8.3`、`bit-set@0.8.0`、`bit-vec@0.8.0`、`cargo_toml@1.0.1`、`chacha20@0.9.1`、`chacha20poly1305@0.10.1`、`const-oid@0.9.6`、`ctor@1.0.13`、`der@0.7.10`、`ed25519@2.2.3`、`equivalent@1.0.3`、`fastrand@2.5.0`、`idna_adapter@1.2.2`、`indexmap@1.9.3`、`indexmap@2.14.2`、`muda@0.20.0`、`pin-project-lite@0.2.17`、`pkcs8@0.10.2`、`poly1305@0.8.0`、`rustc-hash@2.1.3`、`signature@2.2.0`、`spki@0.7.3`、`tauri@2.12.5`、`tauri-build@2.7.4`、`tauri-codegen@2.7.4`、`tauri-macros@2.7.4`、`tauri-plugin@2.7.2`、`tauri-plugin-dialog@2.8.4`、`tauri-plugin-fs@2.7.1`、`tauri-plugin-single-instance@2.5.5`、`tauri-runtime@2.12.2`、`tauri-runtime-wry@2.12.3`、`tauri-utils@2.10.2`、`utf_types@1.0.0`、`utf8_iter@1.1.0`、`uuid@1.29.0`、`window-vibrancy@0.8.1`、`wry@0.57.1`、`zeroize@1.9.1`、`zeroize_derive@1.5.0`

### Unicode-3.0（18）

`icu_collections@2.4.0`、`icu_locale_core@2.4.0`、`icu_normalizer@2.4.0`、`icu_normalizer_data@2.4.0`、`icu_properties@2.4.0`、`icu_properties_data@2.4.0`、`icu_provider@2.4.0`、`litemap@0.8.3`、`potential_utf@0.1.6`、`tinystr@0.8.5`、`writeable@0.6.4`、`yoke@0.8.4`、`yoke-derive@0.8.4`、`zerofrom@0.1.8`、`zerofrom-derive@0.1.8`、`zerotrie@0.2.5`、`zerovec@0.11.9`、`zerovec-derive@0.11.7`

### Unlicense OR MIT（8）

`aho-corasick@1.1.5`、`byteorder@1.5.0`、`jiff@0.2.38`、`jiff-core@0.1.1`、`jiff-tzdb@0.1.9`、`jiff-tzdb-platform@0.1.3`、`memchr@2.8.3`、`winapi-util@0.1.11`

### MIT/Apache-2.0（6）

`bitflags@1.3.2`、`bs58@0.5.1`、`curve25519-dalek-derive@0.1.1`、`ident_case@1.0.1`、`json-patch@4.2.0`、`version_check@0.9.5`

### BSD-3-Clause（5）

`alloc-no-stdlib@3.0.0`、`alloc-stdlib@0.3.0`、`curve25519-dalek@4.1.3`、`ed25519-dalek@2.2.0`、`subtle@2.6.1`

### MPL-2.0（5）

`cssparser@0.37.0`、`cssparser-macros@0.7.1`、`dtoa-short@0.3.5`、`option-ext@0.2.0`、`selectors@0.38.0`

### Zlib（2）

`foldhash@0.2.0`、`zlib-rs@0.6.8`

### MIT OR Zlib OR Apache-2.0（2）

`miniz_oxide@0.8.9`、`miniz_oxide@0.9.1`

### MIT OR Apache-2.0 OR Zlib（2）

`raw-window-handle@0.6.2`、`static_vcruntime@3.0.0`

### Unlicense/MIT（2）

`same-file@1.0.6`、`walkdir@2.5.0`

### 0BSD OR MIT OR Apache-2.0（1）

`adler2@2.0.1`

### BSD-3-Clause AND MIT（1）

`brotli@9.1.1`

### BSD-3-Clause/MIT（1）

`brotli-decompressor@6.2.0`

### Apache-2.0 AND MIT（1）

`dpi@0.1.2`

### CC0-1.0 OR MIT-0 OR Apache-2.0（1）

`dunce@1.0.5`

### Apache-2.0 / MIT（1）

`fnv@1.0.7`

### Apache-2.0（1）

`tao@0.37.2`

### Zlib OR Apache-2.0 OR MIT（1）

`tinyvec@1.13.3`

### (MIT OR Apache-2.0) AND Unicode-3.0（1）

`unicode-ident@1.0.27`

### BSD-2-Clause OR Apache-2.0 OR MIT（1）

`zerocopy@0.8.63`

## 前端依赖（随产物打包）

### MIT（5）

`js-tokens@4.0.0`、`loose-envify@1.4.0`、`react@18.3.1`、`react-dom@18.3.1`、`scheduler@0.23.2`

### Apache-2.0 OR MIT（1）

`@tauri-apps/api@2.12.4`

### MIT OR Apache-2.0（1）

`@tauri-apps/plugin-dialog@2.8.4`

## 输入法词库数据源

以下数据集在**构建期**由 `scripts/build-ime.mjs` 读取，生成为 `public/ime/*.json` 后随应用分发。
三者均为 MIT 许可，允许商用与闭源分发，义务是保留版权声明。

| 数据集 | 许可证 | 用途 |
|---|---|---|
| [mozillazg/pinyin-data](https://github.com/mozillazg/pinyin-data) | MIT | 汉字单字读音 |
| [mozillazg/phrase-pinyin-data](https://github.com/mozillazg/phrase-pinyin-data) | MIT | 词组读音（多音字正音） |
| [fxsjy/jieba](https://github.com/fxsjy/jieba) 的 `dict.txt` | MIT | 词表、词频、字频 |

特意**没有**使用 `rime-ice`（GPL-3.0-only）与 Weasel（GPL-3.0）：这两者的传染性会波及整个产品，
而纯拼音数据用上面三个 MIT 数据集已经够用。

## 附：非 Windows 目标的 Rust 依赖

以下 crate 出现在 `Cargo.lock` 中，但因平台条件编译不会被编进 Windows 产物：

`android_system_properties@0.1.6`、`async-broadcast@0.7.2`、`async-channel@2.5.0`、`async-executor@1.14.0`、`async-io@2.6.0`、`async-lock@3.4.2`、`async-process@2.5.0`、`async-recursion@1.2.0`、`async-signal@0.2.14`、`async-task@4.7.1`、`async-trait@0.1.92`、`atk@0.18.2`、`atk-sys@0.18.2`、`atomic-waker@1.1.2`、`base64@0.21.7`、`block2@0.6.2`、`blocking@1.7.0`、`bumpalo@3.20.3`、`bytemuck@1.25.2`、`cairo-rs@0.18.5`、`cairo-sys-rs@0.18.2`、`cesu8@1.1.0`、`cfg-expr@0.15.8`、`combine@4.6.9`、`concurrent-queue@2.5.0`、`core-foundation@0.10.1`、`core-foundation-sys@0.8.7`、`core-graphics@0.25.0`、`core-graphics-types@0.2.0`、`dbus@0.9.12`、`dispatch2@0.3.1`、`dlopen2@0.8.2`、`dlopen2_derive@0.4.3`、`embed_plist@1.2.2`、`endi@1.1.1`、`enumflags2@0.7.12`、`enumflags2_derive@0.7.12`、`errno@0.3.14`、`event-listener@5.4.2`、`event-listener-strategy@0.5.4`、`fiat-crypto@0.2.9`、`field-offset@0.3.6`、`foreign-types@0.5.0`、`foreign-types-macros@0.2.4`、`foreign-types-shared@0.3.1`、`futures-channel@0.3.34`、`futures-core@0.3.34`、`futures-executor@0.3.34`、`futures-io@0.3.34`、`futures-lite@2.6.1`、`futures-macro@0.3.34`、`futures-sink@0.3.34`、`futures-task@0.3.34`、`futures-util@0.3.34`、`gdk@0.18.2`、`gdk-pixbuf@0.18.5`、`gdk-pixbuf-sys@0.18.0`、`gdk-sys@0.18.2`、`gdkwayland-sys@0.18.2`、`gdkx11@0.18.2`、`gdkx11-sys@0.18.2`、`gio@0.18.4`、`gio-sys@0.18.1`、`glib@0.18.5`、`glib-macros@0.18.5`、`glib-sys@0.18.1`、`gobject-sys@0.18.0`、`gtk@0.18.2`、`gtk-sys@0.18.2`、`gtk3-macros@0.18.2`、`heck@0.4.1`、`hermit-abi@0.5.3`、`http-body@1.1.0`、`http-body-util@0.1.5`、`httparse@1.10.1`、`hyper@1.12.0`、`hyper-util@0.1.21`、`iana-time-zone@0.1.65`、`iana-time-zone-haiku@0.1.2`、`ipnet@2.12.2`、`javascriptcore-rs@1.1.2`、`javascriptcore-rs-sys@1.1.1`、`jiff-static@0.2.38`、`jni@0.21.1`、`jni-sys@0.3.1`、`jni-sys@0.4.1`、`jni-sys-macros@0.4.1`、`js-sys@0.3.106`、`libappindicator@0.9.0`、`libappindicator-sys@0.9.0`、`libdbus-sys@0.2.7`、`libloading@0.7.4`、`libredox@0.1.25`、`linux-raw-sys@0.12.1`、`memoffset@0.9.1`、`ndk@0.9.0`、`ndk-context@0.1.1`、`ndk-sys@0.6.0+11769913`、`num_enum@0.7.6`、`num_enum_derive@0.7.6`、`objc2@0.6.5`、`objc2-app-kit@0.3.2`、`objc2-cloud-kit@0.3.2`、`objc2-core-data@0.3.2`、`objc2-core-foundation@0.3.2`、`objc2-core-graphics@0.3.2`、`objc2-core-image@0.3.2`、`objc2-core-location@0.3.2`、`objc2-core-text@0.3.2`、`objc2-encode@4.1.0`、`objc2-exception-helper@0.1.1`、`objc2-foundation@0.3.2`、`objc2-io-surface@0.3.2`、`objc2-quartz-core@0.3.2`、`objc2-ui-kit@0.3.2`、`objc2-user-notifications@0.3.2`、`objc2-web-kit@0.3.2`、`ordered-stream@0.2.0`、`pango@0.18.3`、`pango-sys@0.18.0`、`parking@2.2.1`、`piper@0.2.5`、`pkg-config@0.3.34`、`polling@3.11.0`、`portable-atomic@1.15.0`、`portable-atomic-util@0.2.8`、`proc-macro-crate@1.3.1`、`proc-macro-crate@2.0.2`、`proc-macro-crate@3.5.0`、`proc-macro-error@1.0.4`、`proc-macro-error-attr@1.0.4`、`r-efi@5.3.0`、`r-efi@6.0.0`、`redox_syscall@0.5.18`、`redox_users@0.5.3`、`reqwest@0.13.5`、`rustix@1.1.6`、`rustversion@1.0.23`、`serde_spanned@0.6.9`、`signal-hook-registry@1.4.8`、`slab@0.4.12`、`soup3@0.5.0`、`soup3-sys@0.5.0`、`swift-rs@1.0.9`、`syn@1.0.109`、`sync_wrapper@1.0.2`、`system-deps@6.2.2`、`tao-macros@0.1.4`、`target-lexicon@0.12.16`、`tempfile@3.27.0`、`tokio-util@0.7.20`、`toml@0.8.2`、`toml_datetime@0.6.3`、`toml_edit@0.19.15`、`toml_edit@0.20.2`、`toml_edit@0.25.17+spec-1.1.0`、`tower@0.5.3`、`tower-http@0.6.11`、`tower-layer@0.3.3`、`tower-service@0.3.3`、`try-lock@0.2.5`、`uds_windows@1.2.1`、`version-compare@0.2.1`、`want@0.3.2`、`wasi@0.11.1+wasi-snapshot-preview1`、`wasip2@1.0.4+wasi-0.2.12`、`wasm-bindgen@0.2.129`、`wasm-bindgen-futures@0.4.79`、`wasm-bindgen-macro@0.2.129`、`wasm-bindgen-macro-support@0.2.129`、`wasm-bindgen-shared@0.2.129`、`wasm-streams@0.5.0`、`web-sys@0.3.106`、`webkit2gtk@2.0.2`、`webkit2gtk-sys@2.0.2`、`winapi@0.3.9`、`winapi-i686-pc-windows-gnu@0.4.0`、`winapi-x86_64-pc-windows-gnu@0.4.0`、`windows-sys@0.45.0`、`windows-targets@0.42.2`、`windows_aarch64_gnullvm@0.42.2`、`windows_aarch64_msvc@0.42.2`、`windows_i686_gnu@0.42.2`、`windows_i686_msvc@0.42.2`、`windows_x86_64_gnu@0.42.2`、`windows_x86_64_gnullvm@0.42.2`、`windows_x86_64_msvc@0.42.2`、`winnow@0.5.40`、`wit-bindgen@0.57.1`、`x11@2.21.0`、`x11-dl@2.21.0`、`xiaozhou-sandbox@0.1.0`、`zbus@5.19.0`、`zbus_macros@5.19.0`、`zbus_names@4.3.5`、`zcheapstr@1.2.0`、`zerocopy-derive@0.8.63`、`zvariant@5.15.1`、`zvariant_derive@5.15.1`、`zvariant_utils@4.2.0`

## 附：构建期前端依赖

以下 npm 包只在 `vite build` 时运行，不进入安装包：

`@babel/code-frame@7.29.7`、`@babel/compat-data@7.29.7`、`@babel/core@7.29.7`、`@babel/generator@7.29.12`、`@babel/helper-compilation-targets@7.29.7`、`@babel/helper-globals@7.29.7`、`@babel/helper-module-imports@7.29.7`、`@babel/helper-module-transforms@7.29.7`、`@babel/helper-plugin-utils@7.29.7`、`@babel/helper-string-parser@7.29.11`、`@babel/helper-validator-identifier@7.29.7`、`@babel/helper-validator-option@7.29.7`、`@babel/helpers@7.29.10`、`@babel/parser@7.29.12`、`@babel/plugin-transform-react-jsx-self@7.29.7`、`@babel/plugin-transform-react-jsx-source@7.29.7`、`@babel/template@7.29.7`、`@babel/traverse@7.29.12`、`@babel/types@7.29.11`、`@esbuild/aix-ppc64@0.21.5`、`@esbuild/android-arm@0.21.5`、`@esbuild/android-arm64@0.21.5`、`@esbuild/android-x64@0.21.5`、`@esbuild/darwin-arm64@0.21.5`、`@esbuild/darwin-x64@0.21.5`、`@esbuild/freebsd-arm64@0.21.5`、`@esbuild/freebsd-x64@0.21.5`、`@esbuild/linux-arm@0.21.5`、`@esbuild/linux-arm64@0.21.5`、`@esbuild/linux-ia32@0.21.5`、`@esbuild/linux-loong64@0.21.5`、`@esbuild/linux-mips64el@0.21.5`、`@esbuild/linux-ppc64@0.21.5`、`@esbuild/linux-riscv64@0.21.5`、`@esbuild/linux-s390x@0.21.5`、`@esbuild/linux-x64@0.21.5`、`@esbuild/netbsd-x64@0.21.5`、`@esbuild/openbsd-x64@0.21.5`、`@esbuild/sunos-x64@0.21.5`、`@esbuild/win32-arm64@0.21.5`、`@esbuild/win32-ia32@0.21.5`、`@esbuild/win32-x64@0.21.5`、`@jridgewell/gen-mapping@0.3.13`、`@jridgewell/remapping@2.3.5`、`@jridgewell/resolve-uri@3.1.2`、`@jridgewell/sourcemap-codec@1.6.0`、`@jridgewell/trace-mapping@0.3.31`、`@napi-rs/lzma-linux-x64-gnu@1.5.1`、`@rolldown/pluginutils@1.0.0-beta.27`、`@rollup/rollup-android-arm-eabi@4.66.0`、`@rollup/rollup-android-arm64@4.66.0`、`@rollup/rollup-darwin-arm64@4.66.0`、`@rollup/rollup-darwin-x64@4.66.0`、`@rollup/rollup-freebsd-arm64@4.66.0`、`@rollup/rollup-freebsd-x64@4.66.0`、`@rollup/rollup-linux-arm-gnueabihf@4.66.0`、`@rollup/rollup-linux-arm-musleabihf@4.66.0`、`@rollup/rollup-linux-arm64-gnu@4.66.0`、`@rollup/rollup-linux-arm64-musl@4.66.0`、`@rollup/rollup-linux-loong64-gnu@4.66.0`、`@rollup/rollup-linux-loong64-musl@4.66.0`、`@rollup/rollup-linux-ppc64-gnu@4.66.0`、`@rollup/rollup-linux-ppc64-musl@4.66.0`、`@rollup/rollup-linux-riscv64-gnu@4.66.0`、`@rollup/rollup-linux-riscv64-musl@4.66.0`、`@rollup/rollup-linux-s390x-gnu@4.66.0`、`@rollup/rollup-linux-x64-gnu@4.66.0`、`@rollup/rollup-linux-x64-musl@4.66.0`、`@rollup/rollup-openbsd-x64@4.66.0`、`@rollup/rollup-openharmony-arm64@4.66.0`、`@rollup/rollup-win32-arm64-msvc@4.66.0`、`@rollup/rollup-win32-ia32-msvc@4.66.0`、`@rollup/rollup-win32-x64-gnu@4.66.0`、`@rollup/rollup-win32-x64-msvc@4.66.0`、`@tauri-apps/cli@2.12.4`、`@tauri-apps/cli-darwin-arm64@2.12.4`、`@tauri-apps/cli-darwin-x64@2.12.4`、`@tauri-apps/cli-linux-arm-gnueabihf@2.12.4`、`@tauri-apps/cli-linux-arm64-gnu@2.12.4`、`@tauri-apps/cli-linux-arm64-musl@2.12.4`、`@tauri-apps/cli-linux-riscv64-gnu@2.12.4`、`@tauri-apps/cli-linux-x64-gnu@2.12.4`、`@tauri-apps/cli-linux-x64-musl@2.12.4`、`@tauri-apps/cli-win32-arm64-msvc@2.12.4`、`@tauri-apps/cli-win32-ia32-msvc@2.12.4`、`@tauri-apps/cli-win32-x64-msvc@2.12.4`、`@types/babel__core@7.20.5`、`@types/babel__generator@7.27.0`、`@types/babel__template@7.4.4`、`@types/babel__traverse@7.28.0`、`@types/estree@1.0.9`、`@types/prop-types@15.7.15`、`@types/react@18.3.31`、`@types/react-dom@18.3.7`、`@vitejs/plugin-react@4.7.0`、`baseline-browser-mapping@2.11.34`、`browserslist@4.29.5`、`caniuse-lite@1.0.30001820`、`convert-source-map@2.0.0`、`csstype@3.2.3`、`debug@4.4.3`、`electron-to-chromium@1.5.468`、`esbuild@0.21.5`、`escalade@3.2.0`、`fsevents@2.3.3`、`gensync@1.0.0-beta.2`、`jsesc@3.1.0`、`json5@2.2.3`、`lru-cache@5.1.1`、`ms@2.1.3`、`nanoid@3.3.20`、`node-releases@2.0.61`、`picocolors@1.1.1`、`postcss@8.5.32`、`react-refresh@0.17.0`、`rollup@4.66.0`、`semver@6.3.1`、`source-map-js@1.2.2`、`typescript@5.9.3`、`update-browserslist-db@1.3.4`、`vite@5.4.21`、`yallist@3.1.1`

