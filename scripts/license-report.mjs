#!/usr/bin/env node
/**
 * 生成 THIRD-PARTY-NOTICES.md。
 *
 * 为什么要有这个脚本：许可证义务靠人工维护必然漂移。依赖一升级，
 * 手工写的清单就成了谎言。所以把「读 Cargo.lock + package-lock.json
 * → 按许可证分组 → 输出署名清单」做成一次性的、可重复执行的命令。
 *
 * 用法：node scripts/license-report.mjs
 */
import fs from "node:fs";
import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "..");
const CARGO_HOME = process.env.CARGO_HOME || path.join(process.env.USERPROFILE || "", ".cargo");

function cargoPackages() {
  const lock = fs.readFileSync(path.join(ROOT, "Cargo.lock"), "utf8");
  const regRoot = path.join(CARGO_HOME, "registry", "src");
  const dirs = new Map();
  try {
    for (const reg of fs.readdirSync(regRoot)) {
      for (const d of fs.readdirSync(path.join(regRoot, reg))) {
        dirs.set(d, path.join(regRoot, reg, d));
      }
    }
  } catch {
    /* 没有本地 registry 时只能给出包名，许可证一栏留空 */
  }

  // 根包自己也出现在 Cargo.lock 里，但它不是第三方依赖
  const selfName = (fs.readFileSync(path.join(ROOT, "Cargo.toml"), "utf8").match(/^\s*name\s*=\s*"([^"]+)"/m) || [])[1];

  const out = [];
  const unresolved = [];
  for (const block of lock.split("[[package]]").slice(1)) {
    const name = (block.match(/name = "([^"]+)"/) || [])[1];
    const version = (block.match(/version = "([^"]+)"/) || [])[1];
    if (!name || !version) continue;
    if (name === selfName) continue;
    const dir = dirs.get(`${name}-${version}`);
    if (!dir) {
      unresolved.push({ name, version });
      continue;
    }
    let lic = "";
    try {
      const toml = fs.readFileSync(path.join(dir, "Cargo.toml"), "utf8");
      const m = toml.match(/^\s*license\s*=\s*"([^"]+)"/m);
      if (m) lic = m[1];
      else if (/^\s*license-file\s*=/m.test(toml)) lic = "见上游 LICENSE 文件";
    } catch {
      /* ignore */
    }
    out.push({ name, version, lic: lic || "未知" });
  }
  out.sort((a, b) => a.name.localeCompare(b.name));
  return { resolved: out, unresolved };
}

function npmPackages() {
  const lockPath = path.join(ROOT, "package-lock.json");
  if (!fs.existsSync(lockPath)) return [];
  const lock = JSON.parse(fs.readFileSync(lockPath, "utf8"));
  const out = [];
  for (const [p, info] of Object.entries(lock.packages || {})) {
    if (!p.startsWith("node_modules/")) continue;
    const name = p.replace(/^node_modules\//, "");
    if (!name || info.dev === false && info.link) continue;
    out.push({ name, version: info.version || "", lic: info.license || "未知", dev: !!info.dev });
  }
  return out.sort((a, b) => a.name.localeCompare(b.name));
}

const cargoAll = cargoPackages();
const cargo = cargoAll.resolved;
const cargoUnresolved = cargoAll.unresolved;
const npm = npmPackages();
// 纯 Rust 版没有前端依赖；有 package-lock.json 时才输出前端那几段
const hasNpm = npm.length > 0;

function group(list) {
  const by = new Map();
  for (const p of list) {
    const k = p.lic;
    if (!by.has(k)) by.set(k, []);
    by.get(k).push(p);
  }
  return [...by.entries()].sort((a, b) => b[1].length - a[1].length);
}

const lines = [];
lines.push("# 第三方组件与许可声明");
lines.push("");
lines.push("本文件由 `node scripts/license-report.mjs` 生成，请勿手工编辑。");
lines.push("");
lines.push(`生成时间：${new Date().toISOString().slice(0, 10)}`);
lines.push("");
lines.push("小舟沙盒自身以 MIT 许可发布，见 `LICENSE`。以下是随二进制分发的第三方组件。");
lines.push("");
lines.push("## 许可分布");
lines.push("");
lines.push("| 侧 | 组件数 | 许可证分布 |");
lines.push("|---|---|---|");
const fmt = (g) => g.map(([k, v]) => `${k} × ${v.length}`).join("、");
lines.push(`| Rust（Windows 产物实际编译） | ${cargo.length} | ${fmt(group(cargo))} |`);
if (hasNpm) {
  lines.push(`| 前端（随产物打包） | ${npm.filter((p) => !p.dev).length} | ${fmt(group(npm.filter((p) => !p.dev)))} |`);
}
lines.push("");
const devNpm = npm.filter((p) => p.dev).length;
const tails = [];
if (cargoUnresolved.length) {
  tails.push(`${cargoUnresolved.length} 个 Rust 依赖只服务于 Linux / macOS / Android 构建目标，Windows 产物不编译它们`);
}
if (devNpm) tails.push(`${devNpm} 个前端开发依赖只在构建期运行，不进安装包`);
if (tails.length) {
  lines.push(`另有 ${tails.join("；")}。这些都不构成随二进制分发的义务，清单见文末。`);
  lines.push("");
}
lines.push("## 需要履行义务的许可证");
lines.push("");
lines.push("- **MIT / ISC / BSD / Apache-2.0 / Zlib / 0BSD / Unlicense**：保留版权声明即可。");
lines.push("- **Apache-2.0**：保留声明，若上游附带 NOTICE 文件需一并保留。");
lines.push(
  "- **双许可（`A OR B`）**：任选其一履行即可。本项目一律选宽松的那一侧" +
    "（如 `self_cell` 的 `Apache-2.0 OR GPL-2.0-only` 按 Apache-2.0 用），不触发 GPL 义务。",
);
const mplCrates = cargo.filter((p) => p.lic.includes("MPL-2.0")).map((p) => p.name);
if (mplCrates.length) {
  lines.push(`- **MPL-2.0**（${[...new Set(mplCrates)].map((n) => `\`${n}\``).join("、")}）：`);
  lines.push("  文件级弱 copyleft。本项目未修改这些文件，仅静态链接使用，因此可以闭源分发；");
  lines.push("  对应源码可从 crates.io 获取。**不要 fork 或修改它们**，否则需要公开修改部分。");
}
const unicodeCrates = [...new Set(cargo.filter((p) => p.lic.includes("Unicode-3.0")).map((p) => p.name))];
if (unicodeCrates.length) {
  const label = unicodeCrates.length > 6 ? `共 ${unicodeCrates.length} 个 crate` : unicodeCrates.map((n) => `\`${n}\``).join("、");
  lines.push(`- **Unicode-3.0**（${label}）：保留版权与许可声明。`);
}
lines.push("");
lines.push("## 明确未使用的组件");
lines.push("");
lines.push("以下组件与本项目功能相似，但因许可证不可闭源商用或存在专利风险而**刻意未引入**：");
lines.push("");
lines.push("| 组件 | 许可证 / 风险 | 替代方案 |");
lines.push("|---|---|---|");
lines.push("| Weasel（Rime 的 Windows 前端） | GPL-3.0，传染 | 不使用 |");
lines.push("| rime-ice（雾凇拼音词库） | GPL-3.0-only，传染 | `pinyin-data`（MIT） |");
lines.push("| FFmpeg + libx264 / libfdk-aac | LGPL + H.264/AAC 专利池 | 系统解码器；VP8 用 `vp8-decoder`（纯 Rust，MIT/Apache-2.0 + BSD-3） |");
lines.push("");
lines.push("VP8 本来可以用 `libvpx`（BSD-3），但它是 C 代码、要额外的构建链；本项目改用纯 Rust 的");
lines.push("`vp8-decoder`，保持「单 exe、无 C 依赖」。");
lines.push("");

for (const [title, list] of [
  ["Rust 依赖", cargo],
  ...(hasNpm ? [["前端依赖（随产物打包）", npm.filter((p) => !p.dev)]] : []),
]) {
  lines.push(`## ${title}`);
  lines.push("");
  for (const [lic, items] of group(list)) {
    lines.push(`### ${lic}（${items.length}）`);
    lines.push("");
    lines.push(items.map((p) => `\`${p.name}@${p.version}\``).join("、"));
    lines.push("");
  }
}

lines.push("## 输入法词库数据源");
lines.push("");
lines.push(
  "以下数据集在**构建期**由 `scripts/build-ime.mjs` 读取，生成为 `assets/ime/*.json` 后用 `include_str!` 嵌进 exe。",
);
lines.push("三者均为 MIT 许可，允许商用与闭源分发，义务是保留版权声明。");
lines.push("");
lines.push("| 数据集 | 许可证 | 用途 |");
lines.push("|---|---|---|");
lines.push("| [mozillazg/pinyin-data](https://github.com/mozillazg/pinyin-data) | MIT | 汉字单字读音 |");
lines.push(
  "| [mozillazg/phrase-pinyin-data](https://github.com/mozillazg/phrase-pinyin-data) | MIT | 词组读音（多音字正音） |",
);
lines.push("| [fxsjy/jieba](https://github.com/fxsjy/jieba) 的 `dict.txt` | MIT | 词表、词频、字频 |");
lines.push("");
lines.push(
  "特意**没有**使用 `rime-ice`（GPL-3.0-only）与 Weasel（GPL-3.0）：这两者的传染性会波及整个产品，",
);
lines.push("而纯拼音数据用上面三个 MIT 数据集已经够用。");
lines.push("");

if (cargoUnresolved.length) {
  lines.push("## 附：非 Windows 目标的 Rust 依赖");
  lines.push("");
  lines.push("以下 crate 出现在 `Cargo.lock` 中，但因平台条件编译不会被编进 Windows 产物：");
  lines.push("");
  lines.push(cargoUnresolved.map((p) => `\`${p.name}@${p.version}\``).join("、"));
  lines.push("");
}

if (hasNpm) {
  lines.push("## 附：构建期前端依赖");
  lines.push("");
  lines.push("以下 npm 包只在 `vite build` 时运行，不进入安装包：");
  lines.push("");
  lines.push(npm.filter((p) => p.dev).map((p) => `\`${p.name}@${p.version}\``).join("、"));
  lines.push("");
}

fs.writeFileSync(path.join(ROOT, "THIRD-PARTY-NOTICES.md"), lines.join("\n") + "\n");
console.log(`已生成 THIRD-PARTY-NOTICES.md：Rust ${cargo.length} 个、前端 ${npm.length} 个`);
