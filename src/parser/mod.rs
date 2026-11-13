//! Netscape 收藏夹 HTML 解析：从 `<H1>` 后的第一个 `<DL>` 开始递归下降。

pub mod entities;

use crate::model::Arena;

/// 嵌套层级上限，避免异常文件把调用栈打穿。
const MAX_DEPTH: usize = 256;

/// 解析时可本地化的文案。
pub struct ParseOptions<'a> {
    /// 虚拟根节点标题（不参与界面统计，仅在空文件时可能显示）
    pub virtual_root: &'a str,
    pub unnamed_folder: &'a str,
    pub unnamed_link: &'a str,
}

/// 解析失败的原因。文案由界面层按语言包渲染。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    /// 没有找到收藏夹结构（缺少 `<DL>`）
    MissingDl,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::MissingDl => write!(f, "missing <DL>"),
        }
    }
}

impl std::error::Error for ParseError {}

/// 解析收藏夹 HTML 文本，返回以虚拟根为 0 号节点的 arena。
pub fn parse_bookmarks_html(content: &str, opts: &ParseOptions) -> Result<Arena, ParseError> {
    let mut arena = Arena::with_root(opts.virtual_root.to_string());

    let search_from = h1_end(content).unwrap_or(0);
    let dl = find_ci(content, b"<dl", search_from).ok_or(ParseError::MissingDl)?;
    let body = after_gt(content.as_bytes(), dl).unwrap_or(content.len());

    let (children, _) = parse_children(content, body, &mut arena, opts, 0)?;
    let root = arena.root();
    arena.nodes[root].children = children;

    Ok(arena)
}

/// 解析一个 `<DL>` 内部的项目，直到配对的 `</DL>`。返回（子节点, 新位置）。
fn parse_children(
    content: &str,
    mut pos: usize,
    arena: &mut Arena,
    opts: &ParseOptions,
    depth: usize,
) -> Result<(Vec<usize>, usize), ParseError> {
    let bytes = content.as_bytes();
    let mut nodes = Vec::new();

    while pos < bytes.len() {
        pos = skip_ws(bytes, pos);
        if pos >= bytes.len() {
            break;
        }

        // 当前层级结束
        if starts_with_ci(bytes, pos, b"</dl") {
            pos = after_gt(bytes, pos).unwrap_or(bytes.len());
            return Ok((nodes, pos));
        }

        // 没有 H3 的孤立 <DL>：把其中的节点当作同级处理
        if starts_with_ci(bytes, pos, b"<dl") {
            let inner = after_gt(bytes, pos).unwrap_or(bytes.len());
            let (nested, next) = if depth >= MAX_DEPTH {
                (Vec::new(), inner)
            } else {
                parse_children(content, inner, arena, opts, depth + 1)?
            };
            nodes.extend(nested);
            pos = next;
            continue;
        }

        if starts_with_ci(bytes, pos, b"<dt") {
            pos = after_gt(bytes, pos).unwrap_or(bytes.len());
            pos = skip_ws(bytes, pos);

            if starts_with_ci(bytes, pos, b"<h3") {
                pos = parse_folder(content, pos, arena, opts, depth, &mut nodes)?;
                continue;
            }

            if starts_with_ci(bytes, pos, b"<a") {
                pos = parse_link(content, pos, arena, opts, &mut nodes);
                continue;
            }

            continue;
        }

        // 无法识别的标记：跳到下一个 <DT 或 </DL，避免死循环
        let next_dt = find_ci(content, b"<dt", pos);
        let next_close = find_ci(content, b"</dl", pos);
        match (next_dt, next_close) {
            (Some(a), Some(b)) => pos = a.min(b),
            (Some(a), None) => pos = a,
            (None, Some(b)) => pos = b,
            (None, None) => break,
        }
    }

    Ok((nodes, pos))
}

/// 解析 `<DT><H3 ...>标题</H3>`（可能紧跟一个 `<DL>` 子层）。
fn parse_folder(
    content: &str,
    pos: usize,
    arena: &mut Arena,
    opts: &ParseOptions,
    depth: usize,
    nodes: &mut Vec<usize>,
) -> Result<usize, ParseError> {
    let bytes = content.as_bytes();
    let len = bytes.len();

    let tag_end = after_gt(bytes, pos).unwrap_or(len);
    let attrs = attribute_slice(content, pos, 3, tag_end);
    let close = find_ci(content, b"</h3", tag_end).unwrap_or(len);
    let raw_title = content.get(tag_end..close).unwrap_or("");
    let title = clean_text(raw_title);
    let title = if title.is_empty() {
        opts.unnamed_folder.to_string()
    } else {
        title
    };

    let index = arena.add_folder(title);
    if let Some(value) = get_attr(attrs, "ADD_DATE") {
        arena.nodes[index].add_date = Some(value);
    }

    let mut next = after_gt(bytes, close).unwrap_or(len);
    next = skip_ws(bytes, next);

    if starts_with_ci(bytes, next, b"<dl") {
        let inner = after_gt(bytes, next).unwrap_or(len);
        if depth < MAX_DEPTH {
            let (children, after) = parse_children(content, inner, arena, opts, depth + 1)?;
            arena.nodes[index].children = children;
            next = after;
        } else {
            next = inner;
        }
    }

    nodes.push(index);
    Ok(next)
}

/// 解析 `<DT><A ...>标题</A>`。
fn parse_link(
    content: &str,
    pos: usize,
    arena: &mut Arena,
    opts: &ParseOptions,
    nodes: &mut Vec<usize>,
) -> usize {
    let bytes = content.as_bytes();
    let len = bytes.len();

    let tag_end = after_gt(bytes, pos).unwrap_or(len);
    let attrs = attribute_slice(content, pos, 2, tag_end);
    let close = find_ci(content, b"</a", tag_end).unwrap_or(len);
    let raw_title = content.get(tag_end..close).unwrap_or("");
    let title = clean_text(raw_title);
    let title = if title.is_empty() {
        opts.unnamed_link.to_string()
    } else {
        title
    };

    let url = get_attr(attrs, "HREF")
        .map(|href| entities::unescape(&href))
        .filter(|href| !href.is_empty());
    let icon = get_attr(attrs, "ICON");
    let add_date = get_attr(attrs, "ADD_DATE");

    nodes.push(arena.add_link(title, url, icon, add_date));
    after_gt(bytes, close).unwrap_or(len)
}

/// 取标签内属性文本：`pos` 指向 `<`，`name_len` 是标签名的字节数。
fn attribute_slice<'a>(content: &'a str, pos: usize, name_len: usize, tag_end: usize) -> &'a str {
    let start = (pos + 1 + name_len).min(content.len());
    let end = tag_end.saturating_sub(1).clamp(start, content.len());
    content.get(start..end).unwrap_or("")
}

/// 解析属性列表，键名统一转大写。
fn parse_attrs(attrs: &str) -> Vec<(String, String)> {
    let bytes = attrs.as_bytes();
    let mut result = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        // 跳过空白与残留符号
        while i < bytes.len() && !is_name_start(bytes[i]) {
            i += 1;
        }
        let name_start = i;
        while i < bytes.len() && is_name_char(bytes[i]) {
            i += 1;
        }
        if i == name_start {
            i += 1;
            continue;
        }
        let name = attrs[name_start..i].to_ascii_uppercase();

        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] != b'=' {
            continue;
        }
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }

        let value = if bytes[i] == b'"' || bytes[i] == b'\'' {
            let quote = bytes[i];
            i += 1;
            let value_start = i;
            while i < bytes.len() && bytes[i] != quote {
                i += 1;
            }
            let value = attrs[value_start..i].to_string();
            i = (i + 1).min(bytes.len());
            value
        } else {
            let value_start = i;
            while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            attrs[value_start..i].to_string()
        };

        result.push((name, value));
    }

    result
}

fn get_attr(attrs: &str, name: &str) -> Option<String> {
    parse_attrs(attrs)
        .into_iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value)
}

/// 去掉标签、还原实体并裁剪空白。
fn clean_text(raw: &str) -> String {
    entities::unescape(&strip_tags(raw)).trim().to_string()
}

fn strip_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_tag = false;

    for ch in text.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }

    out
}

/// 找到 `<H1>...</H1>` 之后的位置。
fn h1_end(content: &str) -> Option<usize> {
    let bytes = content.as_bytes();
    let open = find_ci(content, b"<h1", 0)?;
    let after_open = after_gt(bytes, open)?;
    let close = find_ci(content, b"</h1", after_open)?;
    after_gt(bytes, close)
}

fn find_ci(haystack: &str, needle: &[u8], from: usize) -> Option<usize> {
    let bytes = haystack.as_bytes();
    if needle.is_empty() || from >= bytes.len() || bytes.len() - from < needle.len() {
        return None;
    }

    let mut i = from;
    while i + needle.len() <= bytes.len() {
        if bytes[i..i + needle.len()].eq_ignore_ascii_case(needle) {
            return Some(i);
        }
        i += 1;
    }

    None
}

fn starts_with_ci(bytes: &[u8], pos: usize, needle: &[u8]) -> bool {
    pos + needle.len() <= bytes.len() && bytes[pos..pos + needle.len()].eq_ignore_ascii_case(needle)
}

/// 返回 `pos` 之后第一个 `>` 的下一个字节位置。
fn after_gt(bytes: &[u8], pos: usize) -> Option<usize> {
    let mut i = pos;
    while i < bytes.len() {
        if bytes[i] == b'>' {
            return Some(i + 1);
        }
        i += 1;
    }
    None
}

fn skip_ws(bytes: &[u8], mut pos: usize) -> usize {
    while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
        pos += 1;
    }
    pos
}

fn is_name_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_' || byte == b':'
}

fn is_name_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b':' | b'-' | b'.')
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<META HTTP-EQUIV="Content-Type" CONTENT="text/html; charset=UTF-8">
<TITLE>Bookmarks</TITLE>
<H1>Bookmarks</H1>
<DL><p>
    <DT><H3 ADD_DATE="1700000000" PERSONAL_TOOLBAR_FOLDER="true">收藏夹栏</H3>
    <DL><p>
        <DT><H3>工具</H3>
        <DL><p>
            <DT><A HREF="https://example.com/a?x=1&amp;y=2" ADD_DATE="1700000001" ICON="data:image/png;base64,AAA">示例 &amp; 站点</A>
        </DL><p>
        <DT><A HREF="https://rust-lang.org">Rust</A>
    </DL><p>
</DL><p>
"#;

    fn opts() -> ParseOptions<'static> {
        ParseOptions {
            virtual_root: "收藏夹",
            unnamed_folder: "(未命名文件夹)",
            unnamed_link: "(未命名书签)",
        }
    }

    #[test]
    fn parses_nested_structure() {
        let arena = parse_bookmarks_html(SAMPLE, &opts()).unwrap();
        assert_eq!(arena.count_links(0), 2);
        assert_eq!(arena.count_folders_excluding_roots(0), 1);

        let bar = arena.nodes[0].children[0];
        assert_eq!(arena.nodes[bar].title, "收藏夹栏");

        let tool = arena.nodes[bar].children[0];
        assert_eq!(arena.nodes[tool].title, "工具");

        let link = arena.nodes[tool].children[0];
        assert_eq!(arena.nodes[link].title, "示例 & 站点");
        assert_eq!(
            arena.nodes[link].url.as_deref(),
            Some("https://example.com/a?x=1&y=2")
        );
        assert_eq!(arena.nodes[link].add_date.as_deref(), Some("1700000001"));
    }

    #[test]
    fn rejects_file_without_dl() {
        let err = parse_bookmarks_html("<html><body>hi</body></html>", &opts()).unwrap_err();
        assert_eq!(err, ParseError::MissingDl);
    }
}
