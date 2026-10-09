"""把 Netscape Bookmark File Format（.html）解析成树形结构。"""

from __future__ import annotations

import re
from dataclasses import dataclass, field
from html import unescape
from typing import List, Optional


@dataclass
class BookmarkNode:
    """一个文件夹，或者一条书签链接。"""

    title: str
    is_folder: bool
    url: Optional[str] = None
    icon: Optional[str] = None
    add_date: Optional[str] = None
    children: List["BookmarkNode"] = field(default_factory=list)

    def count_links(self) -> int:
        if not self.is_folder:
            return 1
        return sum(c.count_links() for c in self.children)

    def count_folders(self) -> int:
        n = 1 if self.is_folder else 0
        return n + sum(c.count_folders() for c in self.children)


_TAG_RE = re.compile(
    r"<DT\s*>\s*"
    r"(?:"
    r"<H3(?P<h3attrs>[^>]*)>(?P<h3text>.*?)</H3>"
    r"|"
    r"<A(?P<aattrs>[^>]*)>(?P<atext>.*?)</A>"
    r")",
    re.IGNORECASE | re.DOTALL,
)

_DL_OPEN_RE = re.compile(r"<DL\s*>\s*(?:<p\s*/?>)?", re.IGNORECASE)
_DL_CLOSE_RE = re.compile(r"</DL\s*>\s*(?:<p\s*/?>)?", re.IGNORECASE)
_ATTR_RE = re.compile(
    r'([A-Za-z_:][-A-Za-z0-9_:.]*)\s*=\s*"([^"]*)"',
    re.IGNORECASE,
)
_STRIP_TAGS_RE = re.compile(r"<[^>]+>")


def _parse_attrs(attr_str: str) -> dict[str, str]:
    return {m.group(1).upper(): m.group(2) for m in _ATTR_RE.finditer(attr_str or "")}


def _clean_text(raw: str) -> str:
    text = _STRIP_TAGS_RE.sub("", raw or "")
    return unescape(text).strip()


def parse_bookmarks_html(content: str) -> BookmarkNode:
    """
    解析 Netscape 格式的收藏夹 HTML 字符串。

    返回一个虚拟根节点，它的子节点就是顶层项目
    （通常只有一个工具栏文件夹，例如「收藏夹栏」）。
    """
    # 统一换行符；正文保持为一个整串，方便按下标扫描
    text = content

    # 找到 <H1> 之后的第一个顶层 <DL>（若没有 <H1> 则取第一个 DL）
    h1 = re.search(r"<H1[^>]*>.*?</H1>", text, re.IGNORECASE | re.DOTALL)
    start = h1.end() if h1 else 0
    m = _DL_OPEN_RE.search(text, start)
    if not m:
        raise ValueError("未找到收藏夹结构（缺少 <DL>），请确认这是浏览器导出的 .html 收藏夹文件。")

    children, _ = _parse_dl_children(text, m.end())
    root = BookmarkNode(title="收藏夹", is_folder=True, children=children)
    return root


def _parse_dl_children(text: str, pos: int) -> tuple[List[BookmarkNode], int]:
    """解析 <DL> 内部的项目，直到遇到配对的 </DL>。返回 (节点列表, 新位置)。"""
    nodes: List[BookmarkNode] = []
    n = len(text)

    while pos < n:
        # 跳过空白字符，以及不是 DT/DL 的杂散标签
        while pos < n and text[pos] in " \t\r\n":
            pos += 1
        if pos >= n:
            break

        close = _DL_CLOSE_RE.match(text, pos)
        if close:
            return nodes, close.end()

        open_dl = _DL_OPEN_RE.match(text, pos)
        if open_dl:
            # 前面没有 H3 的孤立嵌套 DL——把它的子节点当作同级节点处理
            nested, pos = _parse_dl_children(text, open_dl.end())
            nodes.extend(nested)
            continue

        tag = _TAG_RE.match(text, pos)
        if not tag:
            # 跳过无法识别的标记，避免陷入死循环
            next_dt = re.search(r"<DT\s*>", text[pos:], re.IGNORECASE)
            next_close = re.search(r"</DL\s*>", text[pos:], re.IGNORECASE)
            candidates = [x.start() for x in (next_dt, next_close) if x]
            if not candidates:
                break
            pos += min(candidates)
            continue

        pos = tag.end()

        if tag.group("h3text") is not None:
            attrs = _parse_attrs(tag.group("h3attrs"))
            title = _clean_text(tag.group("h3text")) or "(未命名文件夹)"
            folder = BookmarkNode(
                title=title,
                is_folder=True,
                add_date=attrs.get("ADD_DATE"),
            )
            # 后面可能紧跟一个 <DL>，用来存放子节点
            while pos < n and text[pos] in " \t\r\n":
                pos += 1
            dl = _DL_OPEN_RE.match(text, pos)
            if dl:
                folder.children, pos = _parse_dl_children(text, dl.end())
            nodes.append(folder)
        else:
            attrs = _parse_attrs(tag.group("aattrs"))
            title = _clean_text(tag.group("atext")) or "(未命名书签)"
            href = attrs.get("HREF", "")
            icon = attrs.get("ICON")
            nodes.append(
                BookmarkNode(
                    title=title,
                    is_folder=False,
                    url=unescape(href) if href else None,
                    icon=icon,
                    add_date=attrs.get("ADD_DATE"),
                )
            )

    return nodes, pos


def load_bookmarks_file(path: str, encoding: str = "utf-8") -> BookmarkNode:
    """从磁盘读取并解析收藏夹 .html 文件。"""
    # 先试 utf-8，再依次尝试常见的中文编码
    encodings = [encoding, "utf-8-sig", "gbk", "gb18030", "latin-1"]
    last_err: Optional[Exception] = None
    raw: Optional[str] = None
    for enc in encodings:
        try:
            with open(path, "r", encoding=enc) as f:
                raw = f.read()
            break
        except UnicodeDecodeError as e:
            last_err = e
            continue
    if raw is None:
        raise ValueError(f"无法解码文件编码: {last_err}")
    return parse_bookmarks_html(raw)
