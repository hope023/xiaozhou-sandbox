//! 内置隐私输入法的查表引擎（自前端 `ime/engine.ts` 1:1 移植）。
//!
//! 目的：让中文输入**不经过系统输入法**，从而不在系统 IME 的词库、
//! 云候选、学习记录里留下痕迹。
//!
//! 词库来源（均可商用，见 THIRD-PARTY-NOTICES.md）：
//!   - mozillazg/pinyin-data        MIT   → 单字表
//!   - mozillazg/phrase-pinyin-data MIT   → 词组表（带正确多音字读音）
//!
//! 两张表 `include_str!` 直接嵌进 exe（共约 1.3MB），首次打开隐私输入时解析一次。

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

const SINGLE_JSON: &str = include_str!("../../assets/ime/single.json");
const PHRASE_JSON: &str = include_str!("../../assets/ime/phrase.json");

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    Phrase,
    Char,
}

#[derive(Clone, Debug)]
pub struct Candidate {
    /// 要插入的文本。
    pub text: String,
    /// 选完之后还剩多少拼音没处理（连续输入用）。
    pub rest: String,
    /// 与原前端结构对齐保留（phrase / char），当前 UI 不区分。
    #[allow(dead_code)]
    pub kind: Kind,
}

pub struct Engine {
    single: HashMap<String, String>,
    phrase: HashMap<String, Vec<String>>,
    /// 所有合法拼音音节的前缀集合，用来判断剩余串还可能拼出什么。
    prefixes: HashSet<String>,
}

static ENGINE: OnceLock<Engine> = OnceLock::new();

/// 词库加载失败（JSON 损坏）返回 Err，UI 会提示但不妨碍英文输入。
pub fn engine() -> Result<&'static Engine, String> {
    if let Some(e) = ENGINE.get() {
        return Ok(e);
    }
    let single: HashMap<String, String> =
        serde_json::from_str(SINGLE_JSON).map_err(|e| format!("单字表损坏：{e}"))?;
    let phrase: HashMap<String, Vec<String>> =
        serde_json::from_str(PHRASE_JSON).map_err(|e| format!("词组表损坏：{e}"))?;
    let mut prefixes = HashSet::new();
    for k in single.keys() {
        for i in 1..=k.len() {
            prefixes.insert(k[..i].to_string());
        }
    }
    let _ = ENGINE.set(Engine { single, phrase, prefixes });
    Ok(ENGINE.get().unwrap())
}

impl Engine {
    /// 这段字母还可能是一个拼音的开头吗。用来避免 `nih` 这种半截串乱出候选。
    /// 允许「切分到一半」的情况：`niha` = `ni` + `ha`。
    fn could_be_pinyin(&self, s: &str) -> bool {
        if s.is_empty() {
            return true;
        }
        if self.prefixes.contains(s) {
            return true;
        }
        for i in 1..s.len() {
            if !s.is_char_boundary(i) {
                continue;
            }
            if self.prefixes.contains(&s[..i]) && self.could_be_pinyin(&s[i..]) {
                return true;
            }
        }
        false
    }

    /// 把整串切成一个个合法音节，每段取最高频的那个字拼起来。
    /// `woshi` 既不是词也不是单音节时，第一个候选直接是「我是」。
    /// 最长优先的贪心切分，够用且不会出错。
    fn greedy_join(&self, s: &str) -> Option<String> {
        let mut parts: Vec<&str> = Vec::new();
        let mut i = 0usize;
        while i < s.len() {
            let mut hit = "";
            let max = 6.min(s.len() - i);
            for len in (1..=max).rev() {
                if !s.is_char_boundary(i) || !s.is_char_boundary(i + len) {
                    continue;
                }
                let p = &s[i..i + len];
                if self.single.contains_key(p) {
                    hit = p;
                    break;
                }
            }
            if hit.is_empty() {
                return None;
            }
            parts.push(hit);
            i += hit.len();
        }
        if parts.len() < 2 {
            return None;
        }
        Some(
            parts
                .iter()
                .map(|p| self.single[*p].chars().next().unwrap_or('?'))
                .collect(),
        )
    }

    /// 按拼音串查候选。
    ///
    /// 排序原则（也是所有拼音输入法的直觉）：
    ///   1. 整串命中的**词组**最靠前——用户打的字母越长，越可能想要一个词
    ///   2. 打不成词时，单字自动拼出的组合（`woshi` → 我是）
    ///   3. 能切掉一个较长前缀的单字（剩余拼音留着继续打）
    ///
    /// 不做简拼（首字母缩写），因为词库没带词频，简拼排序会很难看。
    pub fn lookup(&self, raw: &str) -> Vec<Candidate> {
        let s: String = raw
            .to_lowercase()
            .chars()
            .filter(|c| c.is_ascii_lowercase())
            .collect();
        if s.is_empty() {
            return Vec::new();
        }

        let mut out: Vec<Candidate> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        let mut push = |c: Candidate, out: &mut Vec<Candidate>, seen: &mut HashSet<String>| {
            let k = format!("{}\u{0}{}", c.text, c.rest);
            if seen.insert(k) {
                out.push(c);
            }
        };

        // 1. 整串
        if let Some(list) = self.phrase.get(&s) {
            for w in list {
                push(Candidate { text: w.clone(), rest: String::new(), kind: Kind::Phrase }, &mut out, &mut seen);
            }
        }
        if let Some(chars) = self.single.get(&s) {
            for ch in chars.chars().take(24) {
                push(Candidate { text: ch.to_string(), rest: String::new(), kind: Kind::Char }, &mut out, &mut seen);
            }
        }

        // 2. 再给一个「单字自动拼」的候选。
        // 总是给，不只是打不成词的时候：`woshi` 词库里命中「卧室」，但用户多半想打
        // 「我是」。两者都给，多数情况会重合，被 dedupe 掉，不会多出噪声。
        if let Some(joined) = self.greedy_join(&s) {
            push(Candidate { text: joined, rest: String::new(), kind: Kind::Phrase }, &mut out, &mut seen);
        }

        // 3. 前缀单字（长前缀优先，最多看 3 层，再多就是噪声）
        let mut depth = 0;
        let mut len = s.len();
        while len >= 1 && depth < 3 {
            let head = &s[..len];
            if let Some(chars) = self.single.get(head) {
                let rest = s[len..].to_string();
                if self.could_be_pinyin(&rest) {
                    depth += 1;
                    for ch in chars.chars().take(12) {
                        push(
                            Candidate { text: ch.to_string(), rest: rest.clone(), kind: Kind::Char },
                            &mut out,
                            &mut seen,
                        );
                    }
                }
            }
            len -= 1;
        }

        out.truncate(60);
        out
    }
}

/// 中文标点：输入法自带，省得用户切回英文键位打逗号句号。（物理键 → 中文标点）
pub const PUNCT: &[(&str, &str)] = &[
    (",", "，"),
    (".", "。"),
    ("?", "？"),
    ("!", "！"),
    (";", "；"),
    (":", "："),
    ("'", "’"),
    ("\"", "“"),
    ("(", "（"),
    (")", "）"),
    ("[", "【"),
    ("]", "】"),
    ("/", "、"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn woshi_gives_both_phrase_and_auto_join() {
        let e = engine().unwrap();
        let cands = e.lookup("woshi");
        let texts: Vec<&str> = cands.iter().map(|c| c.text.as_str()).collect();
        assert!(texts.contains(&"我是"), "贪心拼字应给「我是」");
        assert!(texts.contains(&"卧室"), "词库应给「卧室」");
    }

    #[test]
    fn ni_first_candidate_is_ni_char() {
        let e = engine().unwrap();
        let cands = e.lookup("ni");
        assert!(!cands.is_empty());
        assert_eq!(cands[0].text, "你");
        assert_eq!(cands[0].rest, "");
    }

    #[test]
    fn partial_syllable_keeps_rest() {
        let e = engine().unwrap();
        let cands = e.lookup("nih");
        assert!(cands.iter().any(|c| c.text == "你" && c.rest == "h"), "nih 应给「你」+ 剩余 h");
    }

    #[test]
    fn non_pinyin_is_empty() {
        let e = engine().unwrap();
        assert!(e.lookup("qqq").is_empty());
        assert!(e.lookup("123").is_empty());
        assert!(e.lookup("").is_empty());
    }

    #[test]
    fn uppercase_is_lowercased() {
        let e = engine().unwrap();
        let a = e.lookup("NI");
        assert!(!a.is_empty());
    }
}
