//! 列表排序：自然排序（A-2 排在 A-10、A-11 前面）+ 几种常用排序键。
//!
//! 排序只改变显示顺序，不动库里的条目。文件夹始终排在文件前面
//! （与资源管理器一致），倒序只作用于组内。

use std::cmp::Ordering;

use crate::service::EntryView;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Name,
    Modified,
    Size,
    Kind,
}

impl SortKey {
    pub const ALL: [SortKey; 4] = [
        SortKey::Name,
        SortKey::Modified,
        SortKey::Size,
        SortKey::Kind,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SortKey::Name => "名称",
            SortKey::Modified => "修改时间",
            SortKey::Size => "大小",
            SortKey::Kind => "类型",
        }
    }

    /// 切到这个键时的默认方向：时间/大小习惯从大到小，名称/类型从小到大。
    fn default_desc(self) -> bool {
        matches!(self, SortKey::Modified | SortKey::Size)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct SortSpec {
    pub key: SortKey,
    pub desc: bool,
}

impl Default for SortSpec {
    fn default() -> Self {
        Self {
            key: SortKey::Name,
            desc: false,
        }
    }
}

impl SortSpec {
    /// 切换排序键；方向回到这个键习惯的默认值。
    pub fn set_key(&mut self, key: SortKey) {
        if self.key != key {
            self.key = key;
            self.desc = key.default_desc();
        }
    }

    /// 按钮提示里显示的当前状态。
    pub fn label(self) -> String {
        format!("{} {}", self.key.label(), if self.desc { "↓" } else { "↑" })
    }
}

/// 按 `spec` 排序。文件夹永远在前；同一组内按所选键排序，
/// 键相同再按名称自然排序，保证顺序稳定可预期。
pub fn sort_entries(entries: &mut [EntryView], spec: SortSpec) {
    entries.sort_by(|a, b| {
        let group = (a.kind != "dir").cmp(&(b.kind != "dir"));
        if group != Ordering::Equal {
            return group;
        }
        let ord = key_cmp(a, b, spec.key).then_with(|| natural_cmp(&a.name, &b.name));
        if spec.desc {
            ord.reverse()
        } else {
            ord
        }
    });
}

fn key_cmp(a: &EntryView, b: &EntryView, key: SortKey) -> Ordering {
    match key {
        SortKey::Name => natural_cmp(&a.name, &b.name),
        SortKey::Modified => a.modified_at.cmp(&b.modified_at),
        SortKey::Size => a.size.cmp(&b.size),
        SortKey::Kind => natural_cmp(ext(&a.name), ext(&b.name)),
    }
}

/// 扩展名（不含点，大小写保留）；没有扩展名时返回空串。
fn ext(name: &str) -> &str {
    match name.rfind('.') {
        Some(i) if i > 0 && i + 1 < name.len() => &name[i + 1..],
        _ => "",
    }
}

/// 自然排序比较：连续数字按数值大小比，其余按忽略大小写的字符比。
/// "A-2" < "A-10" < "A-11"，"图 9" < "图 10"。
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let mut ai = a.chars().peekable();
    let mut bi = b.chars().peekable();
    loop {
        match (ai.peek().copied(), bi.peek().copied()) {
            // 大小写/前导零不同才会走到这里，用原串定序，结果稳定
            (None, None) => return a.cmp(b),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(ca), Some(cb)) if ca.is_ascii_digit() && cb.is_ascii_digit() => {
                let ra = take_digits(&mut ai);
                let rb = take_digits(&mut bi);
                match cmp_digits(&ra, &rb) {
                    Ordering::Equal => {}
                    ord => return ord,
                }
            }
            (Some(ca), Some(cb)) => {
                ai.next();
                bi.next();
                match lower(ca).cmp(&lower(cb)) {
                    Ordering::Equal => {}
                    ord => return ord,
                }
            }
        }
    }
}

fn take_digits(it: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut s = String::new();
    while let Some(&c) = it.peek() {
        if !c.is_ascii_digit() {
            break;
        }
        s.push(c);
        it.next();
    }
    s
}

/// 数字串按数值比较：先去掉前导零，再比位数，最后逐位比。
/// 数值相同时短串在前（"2" 在 "02" 前），保证全序。
fn cmp_digits(a: &str, b: &str) -> Ordering {
    let sa = a.trim_start_matches('0');
    let sb = b.trim_start_matches('0');
    let sa = if sa.is_empty() { "0" } else { sa };
    let sb = if sb.is_empty() { "0" } else { sb };
    sa.len()
        .cmp(&sb.len())
        .then_with(|| sa.cmp(sb))
        .then_with(|| a.len().cmp(&b.len()))
}

fn lower(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, kind: &str, size: u64, modified_at: i64) -> EntryView {
        EntryView {
            id: name.to_string(),
            name: name.to_string(),
            kind: kind.to_string(),
            mime: String::new(),
            size,
            created_at: 0,
            modified_at,
            version_count: 1,
            deleted_at: None,
            preview: String::new(),
        }
    }

    fn names(v: &[EntryView]) -> Vec<&str> {
        v.iter().map(|e| e.name.as_str()).collect()
    }

    #[test]
    fn natural_order_puts_2_before_10() {
        let mut v = vec![
            entry("A-11", "file", 0, 0),
            entry("A-2", "file", 0, 0),
            entry("A-10", "file", 0, 0),
            entry("A-1", "file", 0, 0),
        ];
        sort_entries(&mut v, SortSpec::default());
        assert_eq!(names(&v), ["A-1", "A-2", "A-10", "A-11"]);
    }

    #[test]
    fn natural_order_ignores_case_and_pads() {
        assert_eq!(natural_cmp("a-2.txt", "A-10.txt"), Ordering::Less);
        assert_eq!(natural_cmp("图 9.png", "图 10.png"), Ordering::Less);
        assert_eq!(natural_cmp("img_2.png", "img_02.png"), Ordering::Less);
        assert_eq!(natural_cmp("ABC", "abc"), Ordering::Less);
    }

    #[test]
    fn folders_stay_first_even_descending() {
        let mut v = vec![
            entry("a.txt", "file", 0, 0),
            entry("z", "dir", 0, 0),
            entry("b.txt", "file", 0, 0),
        ];
        sort_entries(
            &mut v,
            SortSpec {
                key: SortKey::Name,
                desc: true,
            },
        );
        assert_eq!(names(&v), ["z", "b.txt", "a.txt"]);
    }

    #[test]
    fn size_sort_desc_largest_first() {
        let mut v = vec![
            entry("small.txt", "file", 10, 0),
            entry("big.txt", "file", 900, 0),
            entry("mid.txt", "file", 100, 0),
        ];
        sort_entries(
            &mut v,
            SortSpec {
                key: SortKey::Size,
                desc: true,
            },
        );
        assert_eq!(names(&v), ["big.txt", "mid.txt", "small.txt"]);
    }

    #[test]
    fn switching_key_resets_direction() {
        let mut s = SortSpec::default();
        s.set_key(SortKey::Modified);
        assert!(s.desc);
        s.set_key(SortKey::Name);
        assert!(!s.desc);
    }
}
