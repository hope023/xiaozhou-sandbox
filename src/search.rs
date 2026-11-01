//! 全文本地搜索。
//!
//! 三条约束决定了这个模块的形状：
//!
//! 1. **索引只存在内存里**，不落盘。落盘就得再来一套「加密索引 + 迁移 + 损坏恢复」，
//!    为一个搜索功能不值当；而且索引本身是明文摘要，落盘等于在磁盘上留了一份
//!    可读的内容指纹，正好破坏本产品的基本承诺。
//! 2. **不引第三方分词库**。中文用二元组（bigram）切分不需要词典，
//!    英文按空白与标点切。这也顺带避开了 GPL 词典的传染风险。
//! 3. **锁库即清空**。索引在内存里是明文，锁定后必须立刻丢。

use std::collections::HashMap;

use serde::Serialize;

use crate::store::{now_ts, Vault};

/// 单文件参与索引的上限。超过这个体积基本是日志或数据文件，全文搜索价值低，
/// 而解密开销会随库变大线性上升。
const MAX_DOC_BYTES: u64 = 1024 * 1024;
/// 每篇文档保留的正文长度，用于生成摘要片段。
const DOC_TEXT_CAP: usize = 2000;
/// 一次构建最多读入的明文字节总量，防止超大库把内存打满。
const BUILD_BUDGET: u64 = 96 * 1024 * 1024;
/// 最多索引的文档数。
const MAX_DOCS: usize = 4000;

#[derive(Serialize, Clone, Debug)]
pub struct Hit {
    pub id: String,
    pub name: String,
    pub snippet: String,
    pub score: u32,
}

#[derive(Default)]
pub struct SearchIndex {
    ids: Vec<String>,
    names: Vec<String>,
    texts: Vec<String>,
    postings: HashMap<String, Vec<u32>>,
    pub built_at: i64,
    pub docs: usize,
    pub truncated: bool,
}

impl SearchIndex {
    pub fn query(&self, q: &str, limit: usize) -> Vec<Hit> {
        let tokens = tokenize(q);
        if tokens.is_empty() {
            return Vec::new();
        }

        // 单字中文查询没有对应的 bigram，退化为在正文里直接找。
        if tokens.len() == 1 && tokens[0].chars().count() == 1 {
            let t = &tokens[0];
            return self
                .texts
                .iter()
                .enumerate()
                .filter_map(|(i, text)| {
                    if text.contains(t.as_str()) || self.names[i].to_lowercase().contains(t.as_str()) {
                        Some(Hit {
                            id: self.ids[i].clone(),
                            name: self.names[i].clone(),
                            snippet: snippet(text, t),
                            score: 1,
                        })
                    } else {
                        None
                    }
                })
                .take(limit)
                .collect();
        }

        let mut score: HashMap<u32, u32> = HashMap::new();
        for t in &tokens {
            if let Some(list) = self.postings.get(t) {
                for id in list {
                    *score.entry(*id).or_insert(0) += 1;
                }
            }
        }

        let mut hits: Vec<(u32, u32)> = score.into_iter().collect();
        // 命中词数优先，其次按体量小的文档优先（短文档命中通常更精准）
        hits.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        hits.truncate(limit);

        hits.into_iter()
            .map(|(i, s)| {
                let i = i as usize;
                Hit {
                    id: self.ids[i].clone(),
                    name: self.names[i].clone(),
                    snippet: snippet(&self.texts[i], q),
                    score: s,
                }
            })
            .collect()
    }
}

/// 把内容切成可检索的 token：英文/数字按词，中文按相邻二字组。
fn tokenize(s: &str) -> Vec<String> {
    let lower = s.to_lowercase();
    let chars: Vec<char> = lower.chars().collect();
    let mut out: Vec<String> = Vec::new();
    let mut word = String::new();
    let mut cjk: Vec<char> = Vec::new();

    let flush_word = |word: &mut String, out: &mut Vec<String>| {
        if word.chars().count() >= 2 {
            out.push(word.clone());
        }
        word.clear();
    };
    let flush_cjk = |cjk: &mut Vec<char>, out: &mut Vec<String>| {
        if cjk.len() == 1 {
            out.push(cjk[0].to_string());
        }
        for w in cjk.windows(2) {
            out.push(w.iter().collect());
        }
        cjk.clear();
    };

    for c in chars {
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
            flush_cjk(&mut cjk, &mut out);
            word.push(c);
        } else if is_cjk(c) {
            flush_word(&mut word, &mut out);
            cjk.push(c);
        } else {
            flush_word(&mut word, &mut out);
            flush_cjk(&mut cjk, &mut out);
        }
    }
    flush_word(&mut word, &mut out);
    flush_cjk(&mut cjk, &mut out);
    out
}

fn is_cjk(c: char) -> bool {
    let u = c as u32;
    (0x4E00..=0x9FFF).contains(&u)      // 基本区
        || (0x3400..=0x4DBF).contains(&u) // 扩展 A
        || (0x3040..=0x30FF).contains(&u) // 日文假名
        || (0xAC00..=0xD7AF).contains(&u) // 韩文
}

/// 在正文里截取命中位置附近的片段。找不到就返回开头。
fn snippet(text: &str, q: &str) -> String {
    let flat = text.replace(['\n', '\r', '\t'], " ");
    let flat = flat.split_whitespace().collect::<Vec<_>>().join(" ");
    let probe = q.trim().to_lowercase();
    let lower = flat.to_lowercase();

    let byte_pos = if probe.is_empty() {
        None
    } else {
        lower.find(&probe)
    };

    let start = match byte_pos {
        Some(p) => {
            let mut s = p.saturating_sub(24);
            while s > 0 && !flat.is_char_boundary(s) {
                s -= 1;
            }
            s
        }
        None => 0,
    };

    let mut end = (start + 90).min(flat.len());
    while end < flat.len() && !flat.is_char_boundary(end) {
        end += 1;
    }
    let mut out = flat[start..end].to_string();
    if start > 0 {
        out.insert_str(0, "…");
    }
    if end < flat.len() {
        out.push('…');
    }
    out
}

/// 构建索引。调用方需持有已解锁的 `Vault`。
///
/// 这里刻意做成同步函数：AEAD 解密速度在 GB/s 量级，一个几百文件的库
/// 通常在百毫秒内完成，加线程反而引入「索引还没好就查询」的状态复杂度。
pub fn build(v: &Vault) -> SearchIndex {
    let mut idx = SearchIndex {
        built_at: now_ts(),
        ..Default::default()
    };
    let mut budget = BUILD_BUDGET;

    let mut targets: Vec<&crate::store::Entry> = v
        .index
        .entries
        .values()
        .filter(|e| e.kind == "file" && e.deleted_at.is_none())
        .collect();
    // 小文件优先：命中的可能性更高，也更容易在预算内多覆盖几篇
    targets.sort_by_key(|e| e.size);

    for e in targets {
        if idx.ids.len() >= MAX_DOCS {
            idx.truncated = true;
            break;
        }
        let ver = match e.current_version() {
            Some(v) => v,
            None => continue,
        };
        if ver.size == 0 {
            continue;
        }

        let mut text = String::new();
        // 正文只对文本类文件解密；图片/视频只索引文件名。
        let is_text = crate::store::classify(&e.mime, &e.name) == "text" || is_doc_ext(&e.name);
        if is_text && ver.size <= MAX_DOC_BYTES && ver.size <= budget {
            if let Ok(raw) = v.read_blob(&e.id, &ver.blob) {
                budget = budget.saturating_sub(raw.len() as u64);
                // 含 NUL 认为不是文本，别把二进制噪音塞进索引
                if !raw.iter().take(4096).any(|b| *b == 0) {
                    let s = String::from_utf8_lossy(&raw).to_string();
                    text = s.chars().take(DOC_TEXT_CAP).collect();
                }
            }
        }

        let doc_id = idx.ids.len() as u32;
        let hay = format!("{} {}", e.name, text);
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for t in tokenize(&hay) {
            // 同一 token 在一篇文档里只记一次，避免长文档靠重复刷分
            if !seen.insert(t.clone()) {
                continue;
            }
            idx.postings.entry(t).or_default().push(doc_id);
        }

        idx.ids.push(e.id.clone());
        idx.names.push(e.name.clone());
        idx.texts.push(text);
    }

    idx.docs = idx.ids.len();
    idx
}

fn is_doc_ext(name: &str) -> bool {
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    matches!(
        ext.as_str(),
        "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "rtf" | "epub"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chinese_bigram_split() {
        let t = tokenize("加密沙盒");
        assert!(t.contains(&"加密".to_string()));
        assert!(t.contains(&"沙盒".to_string()));
        assert!(!t.contains(&"加".to_string()), "多字查询不应产生单字 token");
    }

    #[test]
    fn english_words() {
        let t = tokenize("Hello, world_2 -- Foo");
        assert!(t.contains(&"hello".to_string()));
        assert!(t.contains(&"world_2".to_string()));
        assert!(t.contains(&"foo".to_string()));
    }

    #[test]
    fn snippet_centers_on_match() {
        let s = snippet("前面一堆废话然后是关键词在这里后面还有内容", "关键词");
        assert!(s.contains("关键词"));
        assert!(s.starts_with('…'));
    }
}
