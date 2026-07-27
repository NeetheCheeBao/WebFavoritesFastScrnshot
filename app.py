from __future__ import annotations

import math
import os
import sys
import webbrowser
from typing import Dict, List, Optional

import tkinter as tk
from tkinter import filedialog, messagebox, ttk

from parser import BookmarkNode, load_bookmarks_file

try:
    from PIL import Image, ImageDraw, ImageFont
except ImportError:  # pragma: no cover
    Image = ImageDraw = ImageFont = None  # type: ignore

# Status bar idle credit (clickable)
_STATUS_CREDIT = "©2026 NeetheCheeBao"
_REPO_URL = "https://github.com/NeetheCheeBao/WebFavoritesFastScrnshot"


def _resource_path(*parts: str) -> str:
    """Resolve asset paths for source run and PyInstaller onefile."""
    if getattr(sys, "frozen", False) and hasattr(sys, "_MEIPASS"):
        base = sys._MEIPASS  # type: ignore[attr-defined]
    else:
        base = os.path.dirname(os.path.abspath(__file__))
    return os.path.join(base, *parts)


# ---------------------------------------------------------------------------
# Shared icons (created once; never per-bookmark)
# ---------------------------------------------------------------------------

def _px(img: tk.PhotoImage, x: int, y: int, color: str) -> None:
    img.put(color, to=(x, y, x + 1, y + 1))


def _make_folder_icon() -> tk.PhotoImage:
    img = tk.PhotoImage(width=16, height=16)
    img.put("#ffffff", to=(0, 0, 16, 16))
    for y in range(3, 6):
        for x in range(2, 8):
            _px(img, x, y, "#e6b800")
    for y in range(6, 13):
        for x in range(2, 14):
            _px(img, x, y, "#ffcc33")
    for x in range(2, 14):
        _px(img, x, 6, "#c9a000")
        _px(img, x, 12, "#c9a000")
    for y in range(6, 13):
        _px(img, 2, y, "#c9a000")
        _px(img, 13, y, "#c9a000")
    return img


def _make_star_icon() -> tk.PhotoImage:
    img = tk.PhotoImage(width=16, height=16)
    img.put("#ffffff", to=(0, 0, 16, 16))
    body = {
        (8, 2), (8, 3), (7, 3), (9, 3),
        (7, 4), (8, 4), (9, 4),
        (6, 5), (7, 5), (8, 5), (9, 5), (10, 5),
        (3, 6), (4, 6), (5, 6), (6, 6), (7, 6), (8, 6), (9, 6), (10, 6), (11, 6), (12, 6), (13, 6),
        (5, 7), (6, 7), (7, 7), (8, 7), (9, 7), (10, 7), (11, 7),
        (6, 8), (7, 8), (8, 8), (9, 8), (10, 8),
        (6, 9), (7, 9), (8, 9), (9, 9),
        (5, 10), (6, 10), (7, 10), (8, 10), (9, 10), (10, 10),
        (5, 11), (6, 11), (7, 11), (9, 11), (10, 11),
        (4, 12), (5, 12), (10, 12), (11, 12),
    }
    for x, y in body:
        _px(img, x, y, "#f5c518")
    return img


def _make_link_icon() -> tk.PhotoImage:
    img = tk.PhotoImage(width=16, height=16)
    img.put("#ffffff", to=(0, 0, 16, 16))
    for y in range(2, 14):
        for x in range(4, 12):
            _px(img, x, y, "#f0f0f0")
    for x in range(4, 12):
        _px(img, x, 2, "#6b6b6b")
        _px(img, x, 13, "#6b6b6b")
    for y in range(2, 14):
        _px(img, 4, y, "#6b6b6b")
        _px(img, 11, y, "#6b6b6b")
    for x in range(6, 10):
        _px(img, x, 6, "#888888")
        _px(img, x, 8, "#888888")
        _px(img, x, 10, "#888888")
    return img


# ---------------------------------------------------------------------------
# Main application
# ---------------------------------------------------------------------------

class FavoritesViewer(tk.Tk):
    def __init__(self) -> None:
        super().__init__()
        self.title("WebFavoritesFastScrnshot v1.0.0")
        self.configure(bg="#ffffff")
        self._apply_window_icon()

        self._root_node: Optional[BookmarkNode] = None
        self._file_path: Optional[str] = None
        self._node_map: Dict[str, BookmarkNode] = {}
        self._filter_job: Optional[str] = None
        self._info_refresh_job: Optional[str] = None
        self._total_folders: int = 0
        self._total_links: int = 0
        # Context-menu target (survives selection being cleared by global click)
        self._ctx_iid: Optional[str] = None
        self._ctx_node: Optional[BookmarkNode] = None

        self._folder_icon = _make_folder_icon()
        self._star_icon = _make_star_icon()
        self._link_icon = _make_link_icon()
        self._icons = (self._folder_icon, self._star_icon, self._link_icon)

        self._build_style()
        self._build_ui()
        self._bind_shortcuts()
        self._fit_window_to_toolbar()

        if len(sys.argv) > 1 and sys.argv[1].lower().endswith(".html"):
            self.after(50, lambda: self.open_file(sys.argv[1]))

    def _apply_window_icon(self) -> None:
        """Set title-bar / taskbar icon from assets/icon.ico when available."""
        ico = _resource_path("assets", "icon.ico")
        if not os.path.isfile(ico):
            return
        try:
            self.iconbitmap(ico)
        except tk.TclError:
            pass

    def _build_style(self) -> None:
        style = ttk.Style(self)
        for theme in ("vista", "xpnative", "clam", "default"):
            if theme in style.theme_names():
                style.theme_use(theme)
                break
        style.configure(
            "Treeview",
            rowheight=22,
            font=("Microsoft YaHei UI", 10),
            background="#ffffff",
            fieldbackground="#ffffff",
        )
        style.configure("TButton", font=("Microsoft YaHei UI", 9))
        style.configure("TLabel", font=("Microsoft YaHei UI", 9), background="#f5f5f5")
        style.configure("Toolbar.TFrame", background="#f5f5f5")
        style.configure(
            "Status.TLabel",
            font=("Microsoft YaHei UI", 9),
            background="#f0f0f0",
            foreground="#555555",
        )
        style.configure(
            "StatusLink.TLabel",
            font=("Microsoft YaHei UI", 9, "underline"),
            background="#f0f0f0",
            foreground="#0563c1",
        )
        style.configure(
            "Info.TLabel",
            font=("Microsoft YaHei UI", 9),
            background="#f5f5f5",
            foreground="#333333",
        )

    def _build_ui(self) -> None:
        # Row 1: action buttons
        self._toolbar = ttk.Frame(self, style="Toolbar.TFrame", padding=(8, 6, 8, 4))
        self._toolbar.pack(side=tk.TOP, fill=tk.X)

        ttk.Button(self._toolbar, text="打开文件", command=self.browse_open).pack(side=tk.LEFT, padx=(0, 6))
        ttk.Button(self._toolbar, text="关闭文件", command=self.close_file).pack(side=tk.LEFT, padx=(0, 6))
        ttk.Button(self._toolbar, text="完整截图", command=self.screenshot).pack(side=tk.LEFT, padx=(0, 6))
        ttk.Button(self._toolbar, text="中性截图", command=self.screenshot_neutral).pack(side=tk.LEFT, padx=(0, 6))
        ttk.Button(self._toolbar, text="全部展开", command=self.expand_all).pack(side=tk.LEFT, padx=(0, 4))
        ttk.Button(self._toolbar, text="全部折叠", command=self.collapse_all).pack(side=tk.LEFT, padx=(0, 0))

        # Row 2: filter (below buttons so they are never clipped)
        filter_bar = ttk.Frame(self, style="Toolbar.TFrame", padding=(8, 0, 8, 6))
        filter_bar.pack(side=tk.TOP, fill=tk.X)
        ttk.Label(filter_bar, text="筛选:").pack(side=tk.LEFT)
        self._filter_var = tk.StringVar()
        self._filter_var.trace_add("write", lambda *_: self._schedule_filter())
        ttk.Entry(filter_bar, textvariable=self._filter_var).pack(
            side=tk.LEFT, padx=(6, 0), fill=tk.X, expand=True
        )

        path_frame = ttk.Frame(self, style="Toolbar.TFrame", padding=(8, 0, 8, 6))
        path_frame.pack(side=tk.TOP, fill=tk.X)
        self._path_label = ttk.Label(
            path_frame,
            text="未打开文件 — 请选择本地 .html 收藏夹文件",
            foreground="#888888",
        )
        self._path_label.pack(side=tk.LEFT, fill=tk.X)

        tree_frame = ttk.Frame(self)
        tree_frame.pack(side=tk.TOP, fill=tk.BOTH, expand=True, padx=4, pady=(0, 4))

        # Pure display tree: expand/collapse is handled entirely by ttk (no Python work)
        self._tree = ttk.Treeview(tree_frame, show="tree", selectmode="browse")
        self._tree_vsb = ttk.Scrollbar(tree_frame, orient=tk.VERTICAL, command=self._tree.yview)
        self._tree_hsb = ttk.Scrollbar(tree_frame, orient=tk.HORIZONTAL, command=self._tree.xview)
        self._tree.configure(yscrollcommand=self._tree_vsb.set, xscrollcommand=self._tree_hsb.set)

        self._tree.grid(row=0, column=0, sticky="nsew")
        self._tree_vsb.grid(row=0, column=1, sticky="ns")
        self._tree_hsb.grid(row=1, column=0, sticky="ew")
        tree_frame.rowconfigure(0, weight=1)
        tree_frame.columnconfigure(0, weight=1)

        # Only open URLs on double-click — do NOT toggle open here (tree already does it)
        self._tree.bind("<Double-1>", self._on_double_click)
        self._tree.bind("<Return>", self._on_activate)
        self._tree.bind("<Button-3>", self._on_right_click)
        self._tree.bind("<Button-2>", self._on_right_click)
        # Click empty area inside tree to clear selection
        self._tree.bind("<Button-1>", self._on_tree_click, add="+")
        # Click buttons / filter / elsewhere → clear tree highlight
        self.bind_all("<Button-1>", self._on_global_click, add="+")
        # Info bar: refresh after expand/collapse (Open/Close alone is unreliable on some Windows themes)
        self._tree.bind("<<TreeviewOpen>>", self._schedule_info_bar_refresh, add="+")
        self._tree.bind("<<TreeviewClose>>", self._schedule_info_bar_refresh, add="+")
        self._tree.bind("<ButtonRelease-1>", self._schedule_info_bar_refresh, add="+")
        self._tree.bind("<KeyRelease-Left>", self._schedule_info_bar_refresh, add="+")
        self._tree.bind("<KeyRelease-Right>", self._schedule_info_bar_refresh, add="+")

        # Bottom bars: pack status first (lowest), then info (above status)
        self._status = ttk.Label(
            self, text=_STATUS_CREDIT, style="Status.TLabel", padding=(8, 4), anchor=tk.W
        )
        self._status.pack(side=tk.BOTTOM, fill=tk.X)
        self._status.bind("<Enter>", self._on_status_enter)
        self._status.bind("<Leave>", self._on_status_leave)
        self._status.bind("<Button-1>", self._on_status_click)
        self._info = ttk.Label(
            self,
            text="文件夹：就绪，书签：就绪",
            style="Info.TLabel",
            padding=(8, 4),
            anchor=tk.W,
        )
        self._info.pack(side=tk.BOTTOM, fill=tk.X)

        self._menu = tk.Menu(self, tearoff=0)
        self._menu.add_command(label="在浏览器中打开", command=self._ctx_open)
        self._menu.add_command(label="复制链接", command=self._ctx_copy_url)
        self._menu.add_command(label="复制标题", command=self._ctx_copy_title)
        self._menu.add_separator()
        self._menu.add_command(label="展开此项", command=self._ctx_expand)
        self._menu.add_command(label="折叠此项", command=self._ctx_collapse)

    def _fit_window_to_toolbar(self) -> None:
        """Set default/min width from toolbar button row; keep a modest height."""
        self.update_idletasks()
        # Width: toolbar content + small margin (window chrome)
        btn_w = max(int(self._toolbar.winfo_reqwidth()), 1)
        width = btn_w + 24
        height = 640
        self.minsize(max(btn_w, 400), 360)
        self.geometry(f"{width}x{height}")

    def _bind_shortcuts(self) -> None:
        self.bind("<Control-o>", lambda e: self.browse_open())
        self.bind("<Control-O>", lambda e: self.browse_open())
        self.bind("<Control-f>", lambda e: self.focus_filter())
        self.bind("<F5>", lambda e: self._reload())

    def focus_filter(self) -> None:
        def _find(widget):
            for c in widget.winfo_children():
                if isinstance(c, ttk.Entry):
                    c.focus_set()
                    c.selection_range(0, tk.END)
                    return True
                if _find(c):
                    return True
            return False

        _find(self)

    # ----- File -----

    def browse_open(self) -> None:
        path = filedialog.askopenfilename(
            title="选择收藏夹 HTML 文件",
            filetypes=[("网页收藏夹", "*.html"), ("HTML 文件", "*.html")],
            defaultextension=".html",
        )
        if path:
            self.open_file(path)

    def open_file(self, path: str) -> None:
        path = os.path.abspath(path)
        if not path.lower().endswith(".html"):
            messagebox.showerror("格式不支持", "仅支持 .html 格式的网页收藏夹文件。")
            return
        if not os.path.isfile(path):
            messagebox.showerror("文件不存在", f"找不到文件：\n{path}")
            return
        try:
            root = load_bookmarks_file(path)
        except Exception as e:
            messagebox.showerror("解析失败", f"无法解析收藏夹文件：\n{e}")
            return

        self._root_node = root
        self._file_path = path
        self._total_links = root.count_links()
        # Exclude virtual parser root and top-level root folder(s) (e.g. 收藏夹栏)
        self._total_folders = self._count_folders_excluding_roots(root)
        self._filter_var.set("")
        self._path_label.configure(text=path, foreground="#333333")

        self._build_full_tree(root)

        self._set_status(
            f"已加载  {self._total_folders} 个文件夹  ·  {self._total_links} 个书签"
        )
        self._update_info_bar()

    def _reload(self) -> None:
        if self._file_path:
            self.open_file(self._file_path)

    def close_file(self) -> None:
        """Clear the current bookmark file and reset the UI."""
        if self._filter_job is not None:
            try:
                self.after_cancel(self._filter_job)
            except Exception:
                pass
            self._filter_job = None

        self._root_node = None
        self._file_path = None
        self._total_folders = 0
        self._total_links = 0
        self._filter_var.set("")
        self._clear_tree()
        self._path_label.configure(
            text="未打开文件 — 请选择本地 .html 收藏夹文件",
            foreground="#888888",
        )
        self.title("WebFavoritesFastScrnshot v1.0.0")
        self._set_status_idle()
        self._update_info_bar()

    def _status_is_credit(self) -> bool:
        return str(self._status.cget("text")) == _STATUS_CREDIT

    def _set_status(self, text: str) -> None:
        """Update status text and clear link hover styling."""
        self._status.configure(text=text, style="Status.TLabel", cursor="")

    def _set_status_idle(self) -> None:
        self._set_status(_STATUS_CREDIT)

    def _on_status_enter(self, _event=None) -> None:
        if self._status_is_credit():
            self._status.configure(style="StatusLink.TLabel", cursor="hand2")

    def _on_status_leave(self, _event=None) -> None:
        self._status.configure(style="Status.TLabel", cursor="")

    def _on_status_click(self, _event=None) -> None:
        if self._status_is_credit():
            webbrowser.open(_REPO_URL)

    def _display_title(self, node: BookmarkNode, is_top: bool) -> str:
        """Tree label: top row shows full filename (with extension)."""
        if is_top and self._file_path:
            return os.path.basename(self._file_path)
        return node.title

    @staticmethod
    def _count_folders_excluding_roots(root: BookmarkNode) -> int:
        """Count folders under top-level root folder(s), not counting those roots.

        Example: virtual root → 收藏夹栏 → (EdgeCtrl, WebJS, … nested…)
        returns only EdgeCtrl / WebJS / nested, not 收藏夹栏 itself.
        """
        items = root.children if root.children else []
        total = 0
        for child in items:
            if child.is_folder:
                # count_folders() includes the node itself; drop that one level
                total += max(child.count_folders() - 1, 0)
        return total

    def _schedule_info_bar_refresh(self, event=None) -> None:
        """Debounce + defer so Treeview open state is committed before counting."""
        if self._info_refresh_job is not None:
            try:
                self.after_cancel(self._info_refresh_job)
            except Exception:
                pass
        # Short delay: open flag is applied after the click event returns
        self._info_refresh_job = self.after(10, self._flush_info_bar_refresh)

    def _flush_info_bar_refresh(self) -> None:
        self._info_refresh_job = None
        self._update_info_bar()

    @staticmethod
    def _item_is_open(tree: ttk.Treeview, iid: str) -> bool:
        """Robust open-state check (Tk may return int/bool/str)."""
        val = tree.item(iid, "open")
        if isinstance(val, str):
            return val.lower() in ("1", "true", "yes")
        return bool(val)

    def _update_info_bar(self) -> None:
        """Show currently visible (expanded) counts vs file totals.

        Root folder row (depth 0, file name label) is ignored in both shown and
        total folder counts. Only rows on an open path count as “shown”.
        """
        if self._root_node is None or not self._file_path:
            self._info.configure(text="文件夹：就绪，书签：就绪")
            return
        shown_folders = 0
        shown_links = 0
        for row in self._collect_visible_rows():
            # Ignore root folder (displayed as full filename)
            if row["depth"] == 0:
                continue
            node = self._node_map.get(row["iid"])
            is_folder = node.is_folder if node is not None else row.get("is_folder", False)
            if is_folder:
                shown_folders += 1
            else:
                shown_links += 1
        self._info.configure(
            text=(
                f"文件夹：{shown_folders}/{self._total_folders}，"
                f"书签：{shown_links}/{self._total_links}"
            )
        )

    # ----- Screenshot (long capture of current expanded view) -----

    def screenshot(self) -> None:
        """完整截图：宽度随最长标题伸展。"""
        self._do_screenshot(mode="full")

    def screenshot_neutral(self) -> None:
        """中性截图：固定为树区域宽度，超长标题截断为 …。"""
        self._do_screenshot(mode="neutral")

    def _do_screenshot(self, mode: str = "full") -> None:
        """Capture the full tree as currently expanded (long image), then Save As."""
        if Image is None:
            messagebox.showerror(
                "缺少依赖",
                "截图功能需要 Pillow 库。\n请先安装：py -3 -m pip install Pillow",
            )
            return
        if not self._tree.get_children():
            messagebox.showinfo("截图", "当前没有可截图的收藏夹内容。")
            return

        try:
            rows = self._collect_visible_rows()
            if not rows:
                messagebox.showinfo("截图", "当前没有可截图的收藏夹内容。")
                return
            image = self._render_tree_image(rows, mode=mode)
        except Exception as e:
            messagebox.showerror("截图失败", f"生成截图时出错：\n{e}")
            return

        default_name = self._default_screenshot_name(mode=mode)
        initial_dir = os.path.dirname(self._file_path) if self._file_path else os.getcwd()
        save_path = filedialog.asksaveasfilename(
            title="保存到",
            defaultextension=".png",
            initialdir=initial_dir,
            initialfile=default_name,
            filetypes=[
                ("PNG 图片", "*.png"),
                ("所有文件", "*.*"),
            ],
        )
        if not save_path:
            self._set_status("已取消截图保存")
            return

        if not save_path.lower().endswith(".png"):
            save_path += ".png"

        try:
            image.save(save_path, format="PNG")
        except Exception as e:
            messagebox.showerror("保存失败", f"无法保存截图：\n{e}")
            return

        self._set_status(f"截图已保存：{save_path}")

    def _default_screenshot_name(self, mode: str = "full") -> str:
        if self._file_path:
            stem = os.path.splitext(os.path.basename(self._file_path))[0]
        else:
            stem = "Favorites"
        return f"{stem}-Screenshot.png"

    def _collect_visible_rows(self) -> List[dict]:
        """Walk Treeview in display order, only into currently open folders."""
        rows: List[dict] = []

        def walk(parent: str, depth: int) -> None:
            for iid in self._tree.get_children(parent):
                node = self._node_map.get(iid)
                children = self._tree.get_children(iid)
                is_open = self._item_is_open(self._tree, iid)
                is_folder = bool(node.is_folder) if node else bool(children)
                is_top = depth == 0 and is_folder
                rows.append(
                    {
                        "iid": iid,
                        "depth": depth,
                        "text": self._tree.item(iid, "text") or "",
                        "is_folder": is_folder,
                        "is_top": is_top,
                        "is_open": is_open,
                        "has_children": bool(children),
                    }
                )
                if is_open and children:
                    walk(iid, depth + 1)

        walk("", 0)
        return rows

    def _load_ui_font(self, size: int = 14) -> "ImageFont.ImageFont":
        candidates = [
            r"C:\Windows\Fonts\msyh.ttc",
            r"C:\Windows\Fonts\msyh.ttf",
            r"C:\Windows\Fonts\msyhbd.ttc",
            r"C:\Windows\Fonts\simhei.ttf",
            r"C:\Windows\Fonts\simsun.ttc",
            "/System/Library/Fonts/PingFang.ttc",
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        ]
        for path in candidates:
            if os.path.isfile(path):
                try:
                    return ImageFont.truetype(path, size=size)
                except Exception:
                    continue
        return ImageFont.load_default()

    @staticmethod
    def _fit_text(draw: "ImageDraw.ImageDraw", text: str, font, max_width: int) -> str:
        """Truncate text with ellipsis so it fits within max_width pixels."""
        if max_width <= 0:
            return ""
        if draw.textlength(text, font=font) <= max_width:
            return text
        ell = "…"
        if draw.textlength(ell, font=font) > max_width:
            return ""
        # Binary search for longest prefix that fits with ellipsis
        lo, hi = 0, len(text)
        best = ell
        while lo <= hi:
            mid = (lo + hi) // 2
            candidate = text[:mid] + ell
            if draw.textlength(candidate, font=font) <= max_width:
                best = candidate
                lo = mid + 1
            else:
                hi = mid - 1
        return best

    def _text_pixel_width(self, draw: "ImageDraw.ImageDraw", text: str, font) -> int:
        try:
            return int(draw.textlength(text, font=font))
        except Exception:
            bbox = draw.textbbox((0, 0), text, font=font)
            return int(bbox[2] - bbox[0])

    # Screenshot render scale: 2 = ~2× pixel density (sharper text/icons)
    _SHOT_SCALE = 2

    def _neutral_text_cap(self, draw: "ImageDraw.ImageDraw", rows: List[dict], font, scale: int = 1) -> int:
        """Cap long titles using bulk name lengths, but never narrower than the root title.

        Root / depth-0 label (full filename) must always fit completely.
        First-level folder names also prefer full width so the tree looks balanced.
        Extreme long bookmark titles beyond this cap are truncated with ….
        """
        all_widths = [self._text_pixel_width(draw, r["text"] or "", font) for r in rows]
        all_widths = [w for w in all_widths if w > 0]
        if not all_widths:
            return 280 * scale

        # Always keep root (depth 0) and top-level children fully visible
        must_fit = 0
        for r in rows:
            if r.get("depth", 0) <= 1:
                must_fit = max(
                    must_fit,
                    self._text_pixel_width(draw, r.get("text") or "", font),
                )

        all_widths.sort()
        # 90th percentile of all visible titles (slightly wider than before)
        idx = min(len(all_widths) - 1, max(0, int(round((len(all_widths) - 1) * 0.90))))
        bulk = int(all_widths[idx])

        # Floor ~ 18 CJK chars at 2× scale, plus must-fit root/first-level names
        return max(bulk, must_fit, 280 * scale)

    def _row_chrome_width(
        self,
        depth: int,
        pad_x: int,
        indent_w: int,
        indicator_w: int,
        icon_size: int,
        gap: int,
    ) -> int:
        """Left chrome before text for a given depth (padding + indent + indicator + icon)."""
        return pad_x + depth * indent_w + indicator_w + icon_size + gap

    def _render_tree_image(self, rows: List[dict], mode: str = "full") -> "Image.Image":
        """Paint currently visible tree rows into one long PNG (WYSIWYG expand state).

        Width is derived only from bookmark title lengths (and tree depth), never from
        the GUI window size. Drawn at _SHOT_SCALE for higher resolution.

        mode:
          - full: every title drawn in full; image as wide as the longest line needs
          - neutral: extreme long titles truncated (…); width follows remaining names
        """
        assert Image is not None and ImageDraw is not None

        s = max(int(self._SHOT_SCALE), 1)
        row_h = 28 * s
        pad_x = 12 * s
        pad_y = 10 * s
        indent_w = 20 * s
        icon_size = 20 * s
        indicator_w = 16 * s
        gap = 8 * s
        font_size = 16 * s
        font = self._load_ui_font(font_size)
        border = max(1, s)

        probe = ImageDraw.Draw(Image.new("RGB", (1, 1)))
        text_cap = (
            self._neutral_text_cap(probe, rows, font, scale=s) if mode == "neutral" else None
        )

        # Resolve display text and measure width from content only
        display_texts: List[str] = []
        line_widths: List[int] = []
        for row in rows:
            text = row["text"] or ""
            # Never truncate root file name (depth 0); other rows may be capped in neutral mode
            if text_cap is not None and row.get("depth", 0) > 0:
                text = self._fit_text(probe, text, font, text_cap)
            display_texts.append(text)
            chrome = self._row_chrome_width(
                row["depth"], pad_x, indent_w, indicator_w, icon_size, gap
            )
            line_widths.append(chrome + self._text_pixel_width(probe, text, font) + pad_x)

        # Extra horizontal padding so the right edge is not tight against text
        width = max(max(line_widths, default=0), 320 * s) + 8 * s
        height = pad_y * 2 + len(rows) * row_h

        img = Image.new("RGB", (width, height), "#ffffff")
        draw = ImageDraw.Draw(img)

        # Subtle border
        draw.rectangle([0, 0, width - 1, height - 1], outline="#e0e0e0", width=border)

        folder_icon = self._pil_folder_icon(icon_size)
        star_icon = self._pil_star_icon(icon_size)
        link_icon = self._pil_link_icon(icon_size)

        for i, row in enumerate(rows):
            y0 = pad_y + i * row_h

            x = pad_x + row["depth"] * indent_w

            # Expand / collapse indicator (only for nodes with children)
            if row["has_children"]:
                cx = x + indicator_w // 2
                cy = y0 + row_h // 2
                if row["is_open"]:
                    draw.polygon(
                        [
                            (cx - 5 * s, cy - 3 * s),
                            (cx + 5 * s, cy - 3 * s),
                            (cx, cy + 4 * s),
                        ],
                        fill="#666666",
                    )
                else:
                    draw.polygon(
                        [
                            (cx - 3 * s, cy - 5 * s),
                            (cx - 3 * s, cy + 5 * s),
                            (cx + 4 * s, cy),
                        ],
                        fill="#666666",
                    )
            x += indicator_w

            if row["is_top"]:
                icon = star_icon
            elif row["is_folder"]:
                icon = folder_icon
            else:
                icon = link_icon
            iy = y0 + (row_h - icon_size) // 2
            img.paste(icon, (x, iy), icon if icon.mode == "RGBA" else None)
            x += icon_size + gap

            # Vertical center text using font metrics when available
            try:
                ascent, descent = font.getmetrics()
                text_h = ascent + descent
            except Exception:
                text_h = font_size
            text_y = y0 + (row_h - text_h) // 2
            draw.text((x, text_y), display_texts[i], fill="#1a1a1a", font=font)

        return img

    @staticmethod
    def _pil_folder_icon(size: int = 16) -> "Image.Image":
        img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
        d = ImageDraw.Draw(img)
        # Coordinates scale with size (base design is 16×16)
        def sc(v: float) -> int:
            return int(round(v * size / 16))

        d.rectangle([sc(2), sc(6), sc(13), sc(13)], fill="#ffcc33", outline="#c9a000")
        d.rectangle([sc(2), sc(3), sc(7), sc(6)], fill="#e6b800", outline="#c9a000")
        return img

    @staticmethod
    def _pil_star_icon(size: int = 16) -> "Image.Image":
        img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
        d = ImageDraw.Draw(img)
        cx = cy = size / 2
        r = size * 0.40
        pts = []
        for i in range(10):
            ang = math.radians(-90 + i * 36)
            rad = r if i % 2 == 0 else r * 0.45
            pts.append((cx + rad * math.cos(ang), cy + rad * math.sin(ang)))
        d.polygon(pts, fill="#f5c518", outline="#d4a017")
        return img

    @staticmethod
    def _pil_link_icon(size: int = 16) -> "Image.Image":
        img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
        d = ImageDraw.Draw(img)

        def sc(v: float) -> int:
            return int(round(v * size / 16))

        d.rectangle([sc(4), sc(2), sc(11), sc(13)], fill="#f0f0f0", outline="#6b6b6b")
        for y in (6, 8, 10):
            d.line([sc(6), sc(y), sc(9), sc(y)], fill="#888888", width=max(1, size // 16))
        return img

    # ----- Tree: fully prebuilt so expand/collapse is instant (Edge-like) -----

    def _clear_tree(self) -> None:
        items = self._tree.get_children()
        if items:
            self._tree.delete(*items)
        self._node_map.clear()

    def _build_full_tree(self, root: BookmarkNode, filter_text: str = "") -> None:
        """Insert every visible node once. Expand only toggles visibility afterward."""
        self._clear_tree()
        ft = filter_text.strip().lower()
        items = root.children if root.children else [root]

        # Fast path: iterative stack insert (no work on later expand clicks)
        if not ft:
            for i, node in enumerate(items):
                # First root row: star icon + full filename
                self._insert_all("", node, is_top=(i == 0))
            # Open toolbar root only — children already in tree
            for iid in self._tree.get_children():
                self._tree.item(iid, open=True)
            self._update_info_bar()
            return

        for i, node in enumerate(items):
            self._insert_filtered("", node, ft, is_top=(i == 0))
        self._update_info_bar()

    def _insert_all(self, parent: str, node: BookmarkNode, is_top: bool = False) -> str:
        if node.is_folder:
            icon = self._star_icon if is_top else self._folder_icon
        else:
            icon = self._link_icon

        title = self._display_title(node, is_top)
        # open=False: collapsed by default; data is already present for instant expand
        iid = self._tree.insert(parent, tk.END, text=title, image=icon, open=False)
        self._node_map[iid] = node

        if node.is_folder:
            for child in node.children:
                self._insert_all(iid, child, is_top=False)
        return iid

    def _insert_filtered(
        self, parent: str, node: BookmarkNode, ft: str, is_top: bool = False
    ) -> Optional[str]:
        if not self._node_matches(node, ft):
            return None

        title = self._display_title(node, is_top)
        if node.is_folder:
            icon = self._star_icon if is_top else self._folder_icon
            iid = self._tree.insert(parent, tk.END, text=title, image=icon, open=True)
            self._node_map[iid] = node
            for child in node.children:
                self._insert_filtered(iid, child, ft, is_top=False)
            return iid

        iid = self._tree.insert(parent, tk.END, text=title, image=self._link_icon, open=False)
        self._node_map[iid] = node
        return iid

    def _node_matches(self, node: BookmarkNode, ft: str) -> bool:
        if ft in (node.title or "").lower():
            return True
        if node.url and ft in node.url.lower():
            return True
        if node.is_folder:
            return any(self._node_matches(c, ft) for c in node.children)
        return False

    def _schedule_filter(self) -> None:
        if self._filter_job is not None:
            try:
                self.after_cancel(self._filter_job)
            except Exception:
                pass
        self._filter_job = self.after(200, self._apply_filter)

    def _apply_filter(self) -> None:
        self._filter_job = None
        if self._root_node is None:
            return
        self._build_full_tree(self._root_node, self._filter_var.get())

    # ----- Expand / collapse (UI only — no data loading) -----

    def expand_all(self) -> None:
        def open_rec(iid: str) -> None:
            self._tree.item(iid, open=True)
            for c in self._tree.get_children(iid):
                open_rec(c)

        for c in self._tree.get_children():
            open_rec(c)
        self._update_info_bar()

    def collapse_all(self) -> None:
        def close_rec(iid: str) -> None:
            for c in self._tree.get_children(iid):
                close_rec(c)
            self._tree.item(iid, open=False)

        for c in self._tree.get_children():
            close_rec(c)
        for c in self._tree.get_children():
            self._tree.item(c, open=True)
        self._update_info_bar()

    # ----- Interactions -----

    def _selected_node(self) -> Optional[BookmarkNode]:
        sel = self._tree.selection()
        if not sel:
            return None
        return self._node_map.get(sel[0])

    def _clear_tree_selection(self) -> None:
        """Remove tree selection highlight."""
        try:
            sel = self._tree.selection()
            if sel:
                self._tree.selection_remove(*sel)
            self._tree.focus("")
        except Exception:
            pass

    def _on_tree_click(self, event) -> None:
        """Click blank area of the tree to clear selection highlight."""
        if self._tree.identify_row(event.y):
            return
        self._clear_tree_selection()

    def _on_global_click(self, event) -> None:
        """Clear tree highlight when clicking outside the tree (buttons, filter, etc.)."""
        w = event.widget
        # Keep selection when interacting with the tree itself or its scrollbars
        if w is self._tree or w is self._tree_vsb or w is self._tree_hsb:
            return
        # Do not clear when using the context menu (would break menu actions)
        try:
            if isinstance(w, tk.Menu) or w.winfo_class() in ("Menu", "Menubutton"):
                return
        except Exception:
            pass
        # Ignore events from other Tk windows / dialogs not owned by us
        try:
            if not str(w).startswith(str(self)):
                return
        except Exception:
            return
        self._clear_tree_selection()

    def _context_node(self) -> Optional[BookmarkNode]:
        """Node for context-menu actions (prefer right-click target over selection)."""
        if self._ctx_node is not None:
            return self._ctx_node
        return self._selected_node()

    def _context_iid(self) -> Optional[str]:
        if self._ctx_iid:
            return self._ctx_iid
        sel = self._tree.selection()
        return sel[0] if sel else None

    def _on_double_click(self, event) -> Optional[str]:
        """Double-click bookmark → open URL. Folders: let Treeview handle expand (don't block)."""
        row = self._tree.identify_row(event.y)
        if not row:
            return None
        # Clicked the tree indicator region — leave default expand/collapse alone
        if self._tree.identify_region(event.x, event.y) == "tree":
            element = self._tree.identify_element(event.x, event.y)
            # On some themes the indicator is "Treeitem.indicator"
            if element and "indicator" in element:
                return None

        node = self._node_map.get(row)
        if node is None:
            return None
        if not node.is_folder and node.url:
            self._open_url(node.url)
            return "break"  # prevent extra toggle noise on links
        return None

    def _on_activate(self, event=None) -> None:
        node = self._selected_node()
        if node and not node.is_folder and node.url:
            self._open_url(node.url)

    def _open_url(self, url: str) -> None:
        try:
            webbrowser.open(url)
        except Exception as e:
            messagebox.showerror("打开失败", f"无法打开链接：\n{e}")

    def _on_right_click(self, event) -> None:
        row = self._tree.identify_row(event.y)
        if not row:
            return
        self._tree.selection_set(row)
        self._tree.focus(row)
        self._ctx_iid = row
        self._ctx_node = self._node_map.get(row)
        try:
            self._menu.tk_popup(event.x_root, event.y_root)
        finally:
            self._menu.grab_release()

    def _ctx_open(self) -> None:
        node = self._context_node()
        if node is None:
            self._set_status("未选中任何项")
            return
        if not node.is_folder and node.url:
            self._open_url(node.url)
            return
        if node.is_folder:
            # Folder has no URL; offer to open all links under it
            urls = self._collect_folder_urls(node)
            title = node.title
            if not urls:
                messagebox.showinfo(
                    "文件夹",
                    f"「{title}」\n没有可打开的书签链接。",
                )
                return
            if not messagebox.askyesno(
                "在浏览器中打开",
                f"「{title}」下共有 {len(urls)} 个书签。\n是否全部在浏览器中打开？",
            ):
                return
            # Open all URLs one-by-one (not concurrent) to avoid UI freeze
            self._open_urls_sequential(urls, title=title)
            return
        self._set_status("该项没有可打开的链接")

    def _open_urls_sequential(self, urls: list, title: str = "", delay_ms: int = 120) -> None:
        """Open every URL in order with a short gap; never batch-open concurrently."""
        total = len(urls)
        if total == 0:
            return
        self._set_status(f"正在打开链接 0/{total}…")

        def open_next(index: int) -> None:
            if index >= total:
                self._set_status(f"已在浏览器中打开全部 {total} 个链接")
                return
            self._open_url(urls[index])
            done = index + 1
            self._set_status(f"正在打开链接 {done}/{total}…")
            # Schedule next open so the UI can process events between tabs
            self.after(delay_ms, lambda: open_next(done))

        open_next(0)

    @staticmethod
    def _collect_folder_urls(node: BookmarkNode) -> list:
        """Collect all bookmark URLs under a folder (depth-first)."""
        urls: list = []

        def walk(n: BookmarkNode) -> None:
            if n.is_folder:
                for c in n.children:
                    walk(c)
            elif n.url:
                urls.append(n.url)

        walk(node)
        return urls

    def _ctx_copy_url(self) -> None:
        node = self._context_node()
        if node and node.url:
            self.clipboard_clear()
            self.clipboard_append(node.url)
            self._set_status("已复制链接")
        else:
            self._set_status("该项没有链接")

    def _ctx_copy_title(self) -> None:
        node = self._context_node()
        if node:
            # Prefer the label shown in the tree for the root file row
            title = node.title
            if self._ctx_iid and self._ctx_iid in self._tree.get_children(""):
                title = self._tree.item(self._ctx_iid, "text") or title
            self.clipboard_clear()
            self.clipboard_append(title)
            self._set_status("已复制标题")

    def _ctx_expand(self) -> None:
        iid = self._context_iid()
        if not iid:
            return

        def open_rec(item: str) -> None:
            self._tree.item(item, open=True)
            for c in self._tree.get_children(item):
                open_rec(c)

        open_rec(iid)
        self._update_info_bar()

    def _ctx_collapse(self) -> None:
        iid = self._context_iid()
        if not iid:
            return

        def close_rec(item: str) -> None:
            for c in self._tree.get_children(item):
                close_rec(c)
            self._tree.item(item, open=False)

        close_rec(iid)
        self._update_info_bar()


def main() -> None:
    try:
        from ctypes import windll

        windll.shcore.SetProcessDpiAwareness(1)
    except Exception:
        pass
    app = FavoritesViewer()
    app.mainloop()


if __name__ == "__main__":
    main()
