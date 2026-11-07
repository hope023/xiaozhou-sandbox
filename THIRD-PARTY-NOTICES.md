# 第三方组件与许可声明

本文件由 `node scripts/license-report.mjs` 生成，请勿手工编辑。

生成时间：2026-11-07

小舟沙盒自身以 MIT 许可发布，见 `LICENSE`。以下是随二进制分发的第三方组件。

## 许可分布

| 侧 | 组件数 | 许可证分布 |
|---|---|---|
| Rust（Windows 产物实际编译） | 510 | MIT OR Apache-2.0 × 231、MIT × 105、Apache-2.0 OR MIT × 60、Unicode-3.0 × 18、Apache-2.0 × 15、MIT/Apache-2.0 × 15、Zlib OR Apache-2.0 OR MIT × 11、Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT × 7、MIT OR Apache-2.0 OR Zlib × 6、BSD-3-Clause × 5、Unlicense OR MIT × 4、Zlib × 4、Apache-2.0/MIT × 3、BSL-1.0 × 2、MPL-2.0 × 2、MIT OR Zlib OR Apache-2.0 × 2、BSD-3-Clause OR Apache-2.0 × 2、BSD-3-Clause OR MIT OR Apache-2.0 × 2、MIT OR Apache-2.0 OR LGPL-2.1-or-later × 2、Unlicense/MIT × 2、BSD-2-Clause OR Apache-2.0 OR MIT × 2、0BSD OR MIT OR Apache-2.0 × 1、BSD-2-Clause × 1、MIT / Apache-2.0 × 1、Apache-2.0 AND MIT × 1、(MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0 × 1、MIT OR Apache-2.0 OR BSD-1-Clause × 1、ISC × 1、Apache-2.0 OR GPL-2.0-only × 1、(MIT OR Apache-2.0) AND Unicode-3.0 × 1、(MIT OR Apache-2.0) AND BSD-3-Clause × 1 |

## 需要履行义务的许可证

- **MIT / ISC / BSD / Apache-2.0 / Zlib / 0BSD / Unlicense**：保留版权声明即可。
- **Apache-2.0**：保留声明，若上游附带 NOTICE 文件需一并保留。
- **双许可（`A OR B`）**：任选其一履行即可。本项目一律选宽松的那一侧（如 `self_cell` 的 `Apache-2.0 OR GPL-2.0-only` 按 Apache-2.0 用），不触发 GPL 义务。
- **MPL-2.0**（`colored`、`option-ext`）：
  文件级弱 copyleft。本项目未修改这些文件，仅静态链接使用，因此可以闭源分发；
  对应源码可从 crates.io 获取。**不要 fork 或修改它们**，否则需要公开修改部分。
- **Unicode-3.0**（共 19 个 crate）：保留版权与许可声明。

## 明确未使用的组件

以下组件与本项目功能相似，但因许可证不可闭源商用或存在专利风险而**刻意未引入**：

| 组件 | 许可证 / 风险 | 替代方案 |
|---|---|---|
| Weasel（Rime 的 Windows 前端） | GPL-3.0，传染 | 不使用 |
| rime-ice（雾凇拼音词库） | GPL-3.0-only，传染 | `pinyin-data`（MIT） |
| FFmpeg + libx264 / libfdk-aac | LGPL + H.264/AAC 专利池 | 系统解码器；VP8 用 `vp8-decoder`（纯 Rust，MIT/Apache-2.0 + BSD-3） |

VP8 本来可以用 `libvpx`（BSD-3），但它是 C 代码、要额外的构建链；本项目改用纯 Rust 的
`vp8-decoder`，保持「单 exe、无 C 依赖」。

## Rust 依赖

### MIT OR Apache-2.0（231）

`accesskit@0.25.2`、`accesskit_atspi_common@0.21.1`、`accesskit_consumer@0.39.2`、`accesskit_ios@0.2.2`、`accesskit_macos@0.27.2`、`accesskit_unix@0.24.1`、`accesskit_windows@0.35.2`、`aead@0.5.2`、`ahash@0.8.12`、`allocator-api2@0.2.21`、`android_system_properties@0.1.6`、`android-activity@0.6.1`、`anyhow@1.0.104`、`arboard@3.7.0`、`argon2@0.5.3`、`arrayvec@0.7.8`、`as-raw-xcb-connection@1.0.1`、`ash@0.38.0+1.3.281`、`async-broadcast@0.7.2`、`async-recursion@1.2.0`、`async-trait@0.1.92`、`base64@0.22.1`、`bitflags@2.13.2`、`blake2@0.10.6`、`block-buffer@0.10.4`、`bumpalo@3.20.3`、`cc@1.10.0`、`cfg-if@1.0.5`、`chrono@0.4.45`、`cipher@0.4.4`、`core-foundation@0.9.4`、`core-foundation-sys@0.8.7`、`core-graphics@0.23.2`、`core-graphics-types@0.1.3`、`cpufeatures@0.2.17`、`crc32fast@1.5.3`、`crossbeam-deque@0.8.8`、`crossbeam-epoch@0.9.21`、`crossbeam-utils@0.8.23`、`crypto-common@0.1.7`、`digest@0.10.7`、`dirs@6.0.0`、`dirs-sys@0.5.0`、`displaydoc@0.2.7`、`document-features@0.2.12`、`ecolor@0.37.0`、`eframe@0.37.0`、`egui@0.37.0`、`egui_glow@0.37.0`、`egui_kittest@0.37.0`、`egui_system_font_provider@0.37.0`、`egui-wgpu@0.37.0`、`egui-winit@0.37.0`、`either@1.19.0`、`emath@0.37.0`、`enumflags2@0.7.12`、`enumflags2_derive@0.7.12`、`epaint@0.37.0`、`errno@0.3.14`、`euclid@0.22.14`、`fdeflate@0.3.7`、`find-msvc-tools@0.1.15`、`fixedbitset@0.5.7`、`flate2@1.1.10`、`font-types@0.12.7`、`form_urlencoded@1.2.2`、`futures-channel@0.3.34`、`futures-core@0.3.34`、`futures-io@0.3.34`、`futures-macro@0.3.34`、`futures-task@0.3.34`、`futures-util@0.3.34`、`getopts@0.2.24`、`getrandom@0.2.17`、`getrandom@0.3.4`、`getrandom@0.4.3`、`gif@0.14.2`、`gpu-allocator@0.28.0`、`half@2.7.1`、`hashbrown@0.15.5`、`hashbrown@0.16.1`、`hashbrown@0.17.1`、`hermit-abi@0.5.3`、`hex@0.4.3`、`hkdf@0.12.4`、`hmac@0.12.1`、`iana-time-zone@0.1.65`、`iana-time-zone-haiku@0.1.2`、`idna@1.1.0`、`image@0.25.10`、`image-webp@0.2.4`、`inout@0.1.4`、`itertools@0.15.0`、`itoa@1.0.18`、`jni@0.22.4`、`jni-macros@0.22.4`、`jni-sys@0.3.1`、`jni-sys@0.4.1`、`jni-sys-macros@0.4.1`、`jobserver@0.1.35`、`js-sys@0.3.107`、`kittest@0.5.0`、`lazy_static@1.5.1`、`libc@0.2.191`、`litrs@1.0.1`、`lock_api@0.4.14`、`log@0.4.35`、`memmap2@0.9.12`、`mime@0.3.17`、`naga@30.0.1`、`naga-types@30.0.1`、`ndk@0.9.0`、`ndk-context@0.1.1`、`ndk-sys@0.6.0+11769913`、`num-traits@0.2.19`、`once_cell@1.21.4`、`opaque-debug@0.3.1`、`ordered-stream@0.2.0`、`parking_lot@0.12.5`、`parking_lot_core@0.9.12`、`password-hash@0.5.0`、`percent-encoding@2.3.2`、`petgraph@0.8.3`、`piper@0.2.5`、`pkg-config@0.3.34`、`png@0.18.1`、`poll-promise@0.3.0`、`polycool@0.4.0`、`ppv-lite86@0.2.21`、`presser@0.3.1`、`proc-macro-crate@3.5.0`、`proc-macro2@1.0.107`、`profiling@1.0.18`、`quote@1.0.47`、`rand@0.8.8`、`rand@0.9.5`、`rand_chacha@0.3.1`、`rand_chacha@0.9.0`、`rand_core@0.6.4`、`rand_core@0.9.5`、`range-alloc@0.1.5`、`raw-window-metal@1.1.0`、`rayon@1.12.0`、`rayon-core@1.13.0`、`read-fonts@0.41.0`、`renderdoc-sys@1.1.0`、`roxmltree@0.21.2`、`rustc_version@0.4.1`、`rustversion@1.0.23`、`scopeguard@1.2.0`、`semver@1.0.28`、`serde@1.0.229`、`serde_core@1.0.229`、`serde_derive@1.0.229`、`serde_json@1.0.154`、`serde_repr@0.1.21`、`serde_spanned@1.1.2`、`sha2@0.10.9`、`shlex@2.0.1`、`signal-hook-registry@1.4.8`、`simdutf8@0.1.5`、`siphasher@1.0.4`、`skrifa@0.44.0`、`smallvec@1.16.4`、`smol_str@0.2.2`、`stable_deref_trait@1.2.1`、`static_assertions@1.1.0`、`syn@2.0.119`、`syn@3.0.9`、`sys-locale@0.3.2`、`tempfile@3.27.0`、`thiserror@1.0.69`、`thiserror@2.0.21`、`thiserror-impl@1.0.69`、`thiserror-impl@2.0.21`、`toml@1.1.8+spec-1.1.0`、`toml_datetime@1.1.2+spec-1.1.0`、`toml_edit@0.25.17+spec-1.1.0`、`toml_parser@1.1.5+spec-1.1.0`、`ttf-parser@0.25.1`、`typenum@1.20.1`、`unicase@2.10.0`、`unicode-script@0.5.8`、`unicode-segmentation@1.13.3`、`unicode-width@0.2.3`、`universal-hash@0.5.1`、`url@2.5.8`、`wasm-bindgen@0.2.130`、`wasm-bindgen-futures@0.4.80`、`wasm-bindgen-macro@0.2.130`、`wasm-bindgen-macro-support@0.2.130`、`wasm-bindgen-shared@0.2.130`、`web-sys@0.3.107`、`web-time@1.1.0`、`webbrowser@1.2.6`、`weezl@0.1.12`、`wgpu@30.0.1`、`wgpu-core@30.0.1`、`wgpu-core-deps-apple@30.0.1`、`wgpu-core-deps-emscripten@30.0.1`、`wgpu-core-deps-wasm@30.0.1`、`wgpu-core-deps-windows-linux-android@30.0.1`、`wgpu-hal@30.0.1`、`wgpu-naga-bridge@30.0.1`、`wgpu-types@30.0.1`、`windows@0.62.2`、`windows_aarch64_gnullvm@0.52.6`、`windows_aarch64_msvc@0.52.6`、`windows_i686_gnu@0.52.6`、`windows_i686_gnullvm@0.52.6`、`windows_i686_msvc@0.52.6`、`windows_x86_64_gnu@0.52.6`、`windows_x86_64_gnullvm@0.52.6`、`windows_x86_64_msvc@0.52.6`、`windows-collections@0.3.2`、`windows-core@0.62.2`、`windows-future@0.3.2`、`windows-implement@0.60.2`、`windows-interface@0.59.3`、`windows-link@0.2.1`、`windows-numerics@0.3.1`、`windows-result@0.4.1`、`windows-strings@0.5.1`、`windows-sys@0.52.0`、`windows-sys@0.59.0`、`windows-sys@0.61.2`、`windows-targets@0.52.6`、`windows-threading@0.2.1`、`wl-clipboard-rs@0.9.5`、`x11rb@0.13.2`、`x11rb-protocol@0.13.2`

### MIT（105）

`android-properties@0.2.2`、`ashpd@0.11.1`、`block2@0.5.1`、`block2@0.6.2`、`bytes@1.12.1`、`calloop@0.13.0`、`calloop@0.14.5`、`calloop-wayland-source@0.3.0`、`calloop-wayland-source@0.4.1`、`cfg_aliases@0.2.2`、`color_quant@1.1.0`、`combine@4.6.9`、`crunchy@0.2.4`、`dify@0.8.0`、`dispatch@0.2.0`、`dlib@0.5.3`、`endi@1.1.1`、`fax@0.2.7`、`generic-array@0.14.7`、`glutin-winit@0.5.0`、`harfrust@0.12.0`、`is-docker@0.2.0`、`is-wsl@0.4.0`、`libm@0.2.16`、`libredox@0.1.25`、`memoffset@0.9.1`、`mime_guess@2.0.5`、`nom@8.0.0`、`objc-sys@0.3.5`、`objc2@0.5.3`、`objc2@0.6.5`、`objc2-app-kit@0.2.2`、`objc2-cloud-kit@0.2.2`、`objc2-contacts@0.2.2`、`objc2-core-data@0.2.2`、`objc2-core-image@0.2.2`、`objc2-core-location@0.2.2`、`objc2-encode@4.1.0`、`objc2-foundation@0.2.2`、`objc2-foundation@0.3.2`、`objc2-link-presentation@0.2.2`、`objc2-metal@0.2.2`、`objc2-quartz-core@0.2.2`、`objc2-symbols@0.2.2`、`objc2-ui-kit@0.2.2`、`objc2-uniform-type-identifiers@0.2.2`、`objc2-user-notifications@0.2.2`、`open@5.4.5`、`orbclient@0.3.55`、`ordered-float@5.5.1`、`os_pipe@1.2.3`、`phf@0.14.0`、`phf_generator@0.14.0`、`phf_macros@0.14.0`、`phf_shared@0.14.0`、`quick-xml@0.41.0`、`redox_syscall@0.4.1`、`redox_syscall@0.5.18`、`redox_syscall@0.9.5`、`redox_users@0.5.3`、`rfd@0.15.4`、`sctk-adwaita@0.10.1`、`simd-adler32@0.3.10`、`slab@0.4.12`、`smithay-client-toolkit@0.19.2`、`smithay-client-toolkit@0.20.0`、`smithay-clipboard@0.7.3`、`strict-num@0.1.1`、`synstructure@0.14.0`、`tiff@0.11.3`、`tokio@1.54.1`、`tracing@0.1.44`、`tracing-attributes@0.1.31`、`tracing-core@0.1.36`、`tree_magic_mini@3.2.2`、`uds_windows@1.2.1`、`urlencoding@2.1.3`、`wayland-backend@0.3.17`、`wayland-client@0.31.15`、`wayland-csd-frame@0.3.0`、`wayland-cursor@0.31.14`、`wayland-protocols@0.32.13`、`wayland-protocols-experimental@20250721.0.1`、`wayland-protocols-misc@0.3.12`、`wayland-protocols-plasma@0.3.12`、`wayland-protocols-wlr@0.3.12`、`wayland-scanner@0.31.11`、`wayland-sys@0.31.11`、`winnow@1.0.4`、`x11-dl@2.21.0`、`xcursor@0.3.11`、`xkbcommon-dl@0.4.2`、`xml-rs@0.8.29`、`yeslogic-fontconfig-sys@6.0.1`、`zbus@5.19.0`、`zbus_macros@5.19.0`、`zbus_names@4.3.5`、`zbus_xml@5.2.1`、`zbus-lockstep@0.5.2`、`zbus-lockstep-macros@0.5.2`、`zcheapstr@1.2.0`、`zmij@1.0.23`、`zvariant@5.15.1`、`zvariant_derive@5.15.1`、`zvariant_utils@4.2.0`

### Apache-2.0 OR MIT（60）

`async-channel@2.5.0`、`async-executor@1.14.0`、`async-fs@2.2.0`、`async-io@2.6.0`、`async-lock@3.4.2`、`async-net@2.0.0`、`async-process@2.5.0`、`async-signal@0.2.14`、`async-task@4.7.1`、`atomic-waker@1.1.2`、`atspi@0.29.0`、`atspi-common@0.13.0`、`atspi-proxies@0.13.0`、`autocfg@1.5.1`、`base64ct@1.8.3`、`bit-set@0.10.0`、`bit-vec@0.9.1`、`blocking@1.7.0`、`chacha20@0.9.1`、`chacha20poly1305@0.10.1`、`color@0.3.3`、`concurrent-queue@2.5.0`、`const-oid@0.9.6`、`der@0.7.10`、`ed25519@2.2.3`、`equivalent@1.0.3`、`event-listener@5.4.2`、`event-listener-strategy@0.5.4`、`fastrand@2.5.0`、`fearless_simd@0.4.1`、`fontique@0.11.1`、`futures-lite@2.6.1`、`glifo@0.3.0`、`idna_adapter@1.2.2`、`indexmap@2.14.2`、`kurbo@0.13.1`、`linebender_resource_handle@0.1.1`、`nohash-hasher@0.2.0`、`parking@2.2.1`、`parlance@0.1.1`、`peniko@0.6.1`、`pin-project@1.1.13`、`pin-project-internal@1.1.13`、`pin-project-lite@0.2.17`、`pkcs8@0.10.2`、`polling@3.11.0`、`poly1305@0.8.0`、`portable-atomic@1.15.0`、`portable-atomic-util@0.2.8`、`rustc-hash@2.1.3`、`signature@2.2.0`、`simd_cesu8@1.2.0`、`spki@0.7.3`、`utf_types@1.0.0`、`utf8_iter@1.1.0`、`uuid@1.29.0`、`vello_common@0.2.0`、`vello_cpu@0.2.0`、`zeroize@1.9.1`、`zeroize_derive@1.5.0`

### Unicode-3.0（18）

`icu_collections@2.4.0`、`icu_locale_core@2.4.0`、`icu_normalizer@2.4.0`、`icu_normalizer_data@2.4.0`、`icu_properties@2.4.0`、`icu_properties_data@2.4.0`、`icu_provider@2.4.0`、`litemap@0.8.3`、`potential_utf@0.1.6`、`tinystr@0.8.5`、`writeable@0.6.4`、`yoke@0.8.4`、`yoke-derive@0.8.4`、`zerofrom@0.1.8`、`zerofrom-derive@0.1.8`、`zerotrie@0.2.5`、`zerovec@0.11.9`、`zerovec-derive@0.11.7`

### Apache-2.0（15）

`ab_glyph@0.2.32`、`ab_glyph_rasterizer@0.1.10`、`accesskit_winit@0.34.2`、`codespan-reporting@0.13.1`、`gethostname@1.1.0`、`gl_generator@0.14.0`、`glutin@0.32.3`、`glutin_egl_sys@0.7.1`、`glutin_glx_sys@0.6.1`、`glutin_wgl_sys@0.6.1`、`khronos_api@3.1.0`、`owned_ttf_parser@0.25.1`、`spirv@0.4.0+sdk-1.4.341.0`、`unicode-general-category@1.1.0`、`winit@0.30.13`

### MIT/Apache-2.0（15）

`bitflags@1.3.2`、`curve25519-dalek-derive@0.1.1`、`downcast-rs@1.2.1`、`foreign-types@0.5.0`、`foreign-types-macros@0.2.4`、`foreign-types-shared@0.3.1`、`guillotiere@0.7.0`、`khronos-egl@6.0.0`、`plain@0.2.3`、`quick-error@2.0.1`、`scoped-tls@1.0.1`、`send_wrapper@0.6.0`、`type-map@0.5.1`、`unicode-properties@0.1.4`、`version_check@0.9.5`

### Zlib OR Apache-2.0 OR MIT（11）

`bytemuck@1.25.2`、`bytemuck_derive@1.12.1`、`dispatch2@0.3.1`、`objc2-app-kit@0.3.2`、`objc2-core-foundation@0.3.2`、`objc2-core-graphics@0.3.2`、`objc2-core-text@0.3.2`、`objc2-io-surface@0.3.2`、`objc2-metal@0.3.2`、`objc2-quartz-core@0.3.2`、`objc2-ui-kit@0.3.2`

### Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT（7）

`linux-raw-sys@0.4.15`、`linux-raw-sys@0.12.1`、`rustix@0.38.44`、`rustix@1.1.6`、`wasi@0.11.1+wasi-snapshot-preview1`、`wasip2@1.0.4+wasi-0.2.12`、`wit-bindgen@0.57.1`

### MIT OR Apache-2.0 OR Zlib（6）

`cursor-icon@1.2.0`、`glow@0.17.0`、`raw-window-handle@0.6.2`、`xkeysym@0.2.1`、`zune-core@0.5.3`、`zune-jpeg@0.5.15`

### BSD-3-Clause（5）

`curve25519-dalek@4.1.3`、`ed25519-dalek@2.2.0`、`subtle@2.6.1`、`tiny-skia@0.11.4`、`tiny-skia-path@0.11.4`

### Unlicense OR MIT（4）

`byteorder-lite@0.1.0`、`memchr@2.8.3`、`termcolor@1.4.1`、`winapi-util@0.1.11`

### Zlib（4）

`foldhash@0.1.5`、`foldhash@0.2.0`、`slotmap@1.1.1`、`zlib-rs@0.6.8`

### Apache-2.0/MIT（3）

`pollster@0.4.0`、`pollster@1.0.1`、`rustc-hash@1.1.0`

### BSL-1.0（2）

`clipboard-win@5.4.3`、`error-code@3.4.0`

### MPL-2.0（2）

`colored@2.2.0`、`option-ext@0.2.0`

### MIT OR Zlib OR Apache-2.0（2）

`miniz_oxide@0.8.9`、`miniz_oxide@0.9.1`

### BSD-3-Clause OR Apache-2.0（2）

`moxcms@0.8.1`、`pxfm@0.1.30`

### BSD-3-Clause OR MIT OR Apache-2.0（2）

`num_enum@0.7.6`、`num_enum_derive@0.7.6`

### MIT OR Apache-2.0 OR LGPL-2.1-or-later（2）

`r-efi@5.3.0`、`r-efi@6.0.0`

### Unlicense/MIT（2）

`same-file@1.0.6`、`walkdir@2.5.0`

### BSD-2-Clause OR Apache-2.0 OR MIT（2）

`zerocopy@0.8.63`、`zerocopy-derive@0.8.63`

### 0BSD OR MIT OR Apache-2.0（1）

`adler2@2.0.1`

### BSD-2-Clause（1）

`arrayref@0.3.9`

### MIT / Apache-2.0（1）

`cgl@0.3.2`

### Apache-2.0 AND MIT（1）

`dpi@0.1.2`

### (MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0（1）

`epaint_default_fonts@0.37.0`

### MIT OR Apache-2.0 OR BSD-1-Clause（1）

`fiat-crypto@0.2.9`

### ISC（1）

`libloading@0.8.9`

### Apache-2.0 OR GPL-2.0-only（1）

`self_cell@1.3.0`

### (MIT OR Apache-2.0) AND Unicode-3.0（1）

`unicode-ident@1.0.27`

### (MIT OR Apache-2.0) AND BSD-3-Clause（1）

`vp8-decoder@0.1.1`

## 输入法词库数据源

以下数据集在**构建期**由 `scripts/build-ime.mjs` 读取，生成为 `assets/ime/*.json` 后用 `include_str!` 嵌进 exe。
三者均为 MIT 许可，允许商用与闭源分发，义务是保留版权声明。

| 数据集 | 许可证 | 用途 |
|---|---|---|
| [mozillazg/pinyin-data](https://github.com/mozillazg/pinyin-data) | MIT | 汉字单字读音 |
| [mozillazg/phrase-pinyin-data](https://github.com/mozillazg/phrase-pinyin-data) | MIT | 词组读音（多音字正音） |
| [fxsjy/jieba](https://github.com/fxsjy/jieba) 的 `dict.txt` | MIT | 词表、词频、字频 |

特意**没有**使用 `rime-ice`（GPL-3.0-only）与 Weasel（GPL-3.0）：这两者的传染性会波及整个产品，
而纯拼音数据用上面三个 MIT 数据集已经够用。

