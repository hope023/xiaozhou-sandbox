/**
 * 构建内置输入法的词库。
 *
 * 数据源（均可商用）：
 *   - kMandarin.txt        mozillazg/pinyin-data       MIT
 *   - pinyin.txt（词组）   mozillazg/phrase-pinyin-data MIT
 *
 * 输出（进 `src/data/`，随应用打包，离线可用）：
 *   - single.json   单字：拼音（无调）→ 同音字串，按常用度排序
 *   - phrase.json   词组：拼音 → 候选数组
 *
 * 体积控制思路：拼音用无声调形式聚合（`ni` 而不是 `nǐ`），
 * 同音字拼成一个字符串而不是数组，JSON 里不存多余字段。
 *
 * 用法：node scripts/build-ime.mjs
 */

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const cacheDir = path.join(root, "scripts", ".ime-cache");
// 放 public/ 而不是 src/：走 fetch 加载，避免 Vite 把 JSON 转成 JS 模块时
// 中文被转义、体积从 1.2 MB 膨胀到 2 MB。这俩文件不参与打包，原样拷贝进产物。
const outDir = path.join(root, "public", "ime");

const SRC = {
  single: "https://fastly.jsdelivr.net/gh/mozillazg/pinyin-data@master/kMandarin.txt",
  phrase:
    "https://fastly.jsdelivr.net/gh/mozillazg/phrase-pinyin-data@master/pinyin.txt",
  freq: "https://fastly.jsdelivr.net/gh/fxsjy/jieba@master/jieba/dict.txt",
};

/** 声调元音 → 无声调，用于把 `nǐ` 归一成 `ni`。 */
const TONE = {
  ā: "a", á: "a", ǎ: "a", à: "a",
  ē: "e", é: "e", ě: "e", è: "e", ê: "e",
  ī: "i", í: "i", ǐ: "i", ì: "i",
  ō: "o", ó: "o", ǒ: "o", ò: "o",
  ū: "u", ú: "u", ǔ: "u", ù: "u",
  ǖ: "v", ǘ: "v", ǚ: "v", ǜ: "v", ü: "v",
  ń: "n", ň: "n", ǹ: "n",
  ḿ: "m", m̀: "m",
};

function stripTone(s) {
  let out = "";
  for (const ch of s.toLowerCase()) {
    if (TONE[ch]) out += TONE[ch];
    else if (/[a-z]/.test(ch)) out += ch;
  }
  return out;
}

/** 只保留「一个音节的合法拼音」：纯字母、长度 1–6、以元音或 n/ng/r 结尾。 */
function isSyllable(p) {
  return /^[a-z]{1,6}$/.test(p) && /[aeiouv]/.test(p);
}

/**
 * 常用度排序表。
 *
 * 没有字频数据，但 GB2312 本身就是一份现成的常用度分级：
 * 一级汉字区（0xB0A1–0xD7F9，3755 字）是常用字，二级（0xD8A1–0xF7FE，3008 字）
 * 是次常用字，**不在 GB2312 里的基本都是生僻字**。
 *
 * 不做这一步会很难用：kMandarin 按码点排序，打 `ni` 第一个候选会是「伱」
 * 而不是「你」——生僻字码点小，全排在前面。
 *
 * Node 只支持解码 GBK，不支持编码，所以反过来遍历码表建「字 → 位次」。
 */
function buildRank() {
  const dec = new TextDecoder("gbk", { fatal: false });
  const rank = new Map();
  let i = 0;
  for (let hi = 0xb0; hi <= 0xf7; hi++) {
    for (let lo = 0xa1; lo <= 0xfe; lo++) {
      const ch = dec.decode(Buffer.from([hi, lo]));
      if (ch.length === 1 && !rank.has(ch)) rank.set(ch, i);
      i++;
    }
  }
  return rank;
}

const RANK = buildRank();
/** 不在 GB2312 里的字给一个很大的值，让它们沉到候选末尾。 */
const rankOf = (ch) => RANK.get(ch) ?? 1_000_000;

/**
 * 真正的常用度：jieba 词频词典（MIT）。
 *
 * 只用 GB2312 位次不够——它的一级字库按拼音排、同音按笔画排，
 * 打 `ni` 时「你」会被「妮霓倪泥尼拟」压在后面。字频才是输入法该用的排序依据。
 */
function buildFreq(text) {
  const char = new Map();
  const word = new Map();
  let lines = 0;
  for (const line of text.split("\n")) {
    const sp = line.indexOf(" ");
    if (sp < 0) continue;
    const w = line.slice(0, sp);
    const f = Number(line.slice(sp + 1, line.indexOf(" ", sp + 1)));
    if (!Number.isFinite(f) || f <= 0) continue;
    lines++;
    if (!word.has(w)) word.set(w, f);
    for (const ch of w) {
      if (ch >= "\u4e00" && ch <= "\u9fff") char.set(ch, (char.get(ch) ?? 0) + f);
    }
  }
  return { char, word, lines };
}

/**
 * 语气词和极高频虚词：同音时永远排第一。
 *
 * 不加这个的话，打 `a` 第一个候选是「阿」而不是「啊」，打 `ba` 是「巴」而不是「吧」——
 * 字频表来自新闻语料，人名地名占的比重比口语语气词大，但输入法里正好反过来。
 */
const TOP = new Set([..."啊吗呢吧呀嘛哦嗯啦哇哈嘞额欸"]);

/** 字频降序；没有词频记录的用 GB2312 位次兜底，再不行按码点。 */
function makeCharCmp(FREQ) {
  return (a, b) => {
    const pa = TOP.has(a) ? 1 : 0;
    const pb = TOP.has(b) ? 1 : 0;
    if (pa !== pb) return pb - pa;
    const fa = FREQ.get(a) ?? 0;
    const fb = FREQ.get(b) ?? 0;
    if (fa !== fb) return fb - fa;
    return rankOf(a) - rankOf(b);
  };
}

async function fetchCached(name, url) {
  fs.mkdirSync(cacheDir, { recursive: true });
  const p = path.join(cacheDir, name);
  if (fs.existsSync(p) && fs.statSync(p).size > 1024) return fs.readFileSync(p, "utf8");
  const res = await fetch(url);
  if (!res.ok) throw new Error(`${name} 下载失败：HTTP ${res.status}`);
  const text = await res.text();
  if (text.length < 1024) throw new Error(`${name} 内容异常，只有 ${text.length} 字节`);
  fs.writeFileSync(p, text);
  return text;
}

// ── 单字 ────────────────────────────────────────────────────────────────
// 格式：`U+4E00: yī  # 一`，注释里是字本身，比从码点反推更省事。
function buildSingle(text, cmp) {
  const byPinyin = new Map();
  const byChar = new Map(); // 字 → 主音，用来给 jieba 的词自动注音
  const order = []; // 保持码点顺序，同音字里常用的（编码靠前的）自然排前面
  for (const line of text.split("\n")) {
    const m = line.match(/^U\+([0-9A-Fa-f]+):\s*([^#]+?)\s*#\s*(.+)$/);
    if (!m) continue;
    const cp = parseInt(m[1], 16);
    // 只管基本汉字区，甲骨文扩展区除了拖大体积没别的用
    if (cp < 0x4e00 || cp > 0x9fff) continue;
    const ch = m[3].trim();
    if (!/^[\u4e00-\u9fff]$/.test(ch)) continue;
    // 多音字在 kMandarin 里可能写成 `háng xíng`，取第一个
    const py = stripTone(m[2].trim().split(/\s+/)[0]);
    if (!isSyllable(py)) continue;
    if (!byChar.has(ch)) byChar.set(ch, py);
    if (byPinyin.has(py)) {
      const arr = byPinyin.get(py);
      if (!arr.includes(ch)) arr.push(ch);
    } else {
      const arr = [ch];
      byPinyin.set(py, arr);
      order.push(py);
    }
  }
  const obj = {};
  for (const py of order.sort()) {
    // 同音字按字频排，「你」才会排在「伱」前面
    obj[py] = byPinyin.get(py).sort(cmp).join("");
  }
  return { obj, byChar };
}

// ── 词组 ────────────────────────────────────────────────────────────────
/**
 * 词组表有两个来源，各有短板，所以配合着用：
 *
 *   - `phrase-pinyin-data`（4.7 万条）读音准确，连多音字都对，但**覆盖不全**：
 *     「世界」「工作」这种最基础的词反而没收进去。
 *   - `jieba/dict.txt`（38 万条带词频）覆盖全、有真实词频，但只管分词不管读音。
 *
 * 做法：以 jieba 的高频词为**主体**（保证能打出日常用词），
 * 用 phrase-pinyin-data 的正确读音**覆盖**自动注音（保证「银行」是 háng 不是 xíng）。
 */
function buildCorrect(text) {
  const map = new Map();
  for (const line of text.split("\n")) {
    if (!line || line.startsWith("#")) continue;
    const i = line.indexOf(":");
    if (i < 0) continue;
    const word = line.slice(0, i).trim();
    if (!/^[\u4e00-\u9fff]{2,4}$/.test(word)) continue;
    const pys = line
      .slice(i + 1)
      .trim()
      .split(/\s+/)
      .map(stripTone);
    if (pys.length !== [...word].length) continue;
    if (!pys.every(isSyllable)) continue;
    map.set(word, pys);
  }
  return map;
}

function buildPhrase(correct, wordFreq, byChar, limit) {
  // 按词频降序取前 N 个纯汉字词；候选的排列顺序就是这里的顺序
  const picked = [...wordFreq.entries()]
    .filter(([w]) => /^[\u4e00-\u9fff]{2,4}$/.test(w))
    .sort((a, b) => b[1] - a[1])
    .slice(0, limit);

  const byPinyin = new Map();
  let auto = 0;
  let fixed = 0;
  let unknown = 0;

  for (const [w] of picked) {
    let pys = correct.get(w);
    if (pys) {
      fixed++;
    } else {
      // 逐字取主音。多音字这里会是错的，但覆盖它的 correct 表已经尽力了，
      // 剩下的错读音只会让候选靠后（词频排序仍然正确），不会让词消失。
      const guess = [...w].map((ch) => byChar.get(ch));
      if (guess.some((p) => !p)) {
        unknown++;
        continue;
      }
      pys = guess;
      auto++;
    }
    const key = pys.join("");
    const arr = byPinyin.get(key);
    if (arr) {
      if (arr.length < 12 && !arr.includes(w)) arr.push(w);
    } else {
      byPinyin.set(key, [w]);
    }
  }

  const obj = {};
  for (const k of [...byPinyin.keys()].sort()) obj[k] = byPinyin.get(k);
  return { obj, picked: picked.length, auto, fixed, unknown };
}

const singleText = await fetchCached("kMandarin.txt", SRC.single);
const phraseText = await fetchCached("pinyin.txt", SRC.phrase);
const freqText = await fetchCached("jieba-dict.txt", SRC.freq);

const { char: FREQ, word: WORD_FREQ, lines } = buildFreq(freqText);
console.log(`词频表：${lines} 条词，${FREQ.size} 个字有字频`);

const { obj: single, byChar } = buildSingle(singleText, makeCharCmp(FREQ));
const correct = buildCorrect(phraseText);
const phrase = buildPhrase(correct, WORD_FREQ, byChar, 60000);

fs.mkdirSync(outDir, { recursive: true });

const w = (name, data) => {
  const p = path.join(outDir, name);
  const s = JSON.stringify(data);
  fs.writeFileSync(p, s);
  return { p, kb: (Buffer.byteLength(s) / 1024).toFixed(0) };
};

const a = w("single.json", single);
const b = w("phrase.json", phrase.obj);

const chars = Object.values(single).reduce((n, s) => n + s.length, 0);
const words = Object.values(phrase.obj).reduce((n, v) => n + v.length, 0);
console.log(`单字：${Object.keys(single).length} 个音节 / ${chars} 个汉字 → ${a.kb} KB`);
console.log(
  `词组：${phrase.picked} 个候选词（正确读音 ${phrase.fixed} / 自动注音 ${phrase.auto} / 缺拼音跳过 ${phrase.unknown}）` +
    ` → ${Object.keys(phrase.obj).length} 个拼音串 / ${words} 个词，${b.kb} KB`,
);
console.log(`合计 ${(Number(a.kb) + Number(b.kb)).toFixed(0)} KB`);
