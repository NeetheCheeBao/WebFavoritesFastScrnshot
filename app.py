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
    # 用程序化方式赋值为 None，静态分析工具才会继续把这三个名字当作模块
    # （一旦写成字面赋值，它们会被降级为变量，下面那些字符串注解就会失效）。
    # 运行时效果完全相同：三个名字都变成 None。
    globals().update(dict.fromkeys(("Image", "ImageDraw", "ImageFont")))

try:
    import windnd  # 供 tkinter 使用的 Windows 拖放库
except ImportError:  # pragma: no cover
    windnd = None  # type: ignore

# 版本号与窗口标题（统一在此处维护）
_VERSION = "1.0.2"
_WINDOW_TITLE = f"WebFavoritesFastScrnshot v{_VERSION}"

# 状态栏空闲时的版权信息（可点击）
_STATUS_CREDIT = "©2026 NeetheCheeBao"
_REPO_URL = "https://github.com/NeetheCheeBao/WebFavoritesFastScrnshot"
_PATH_HINT = "未打开文件 — 请选择或拖入本地 .html 收藏夹文件"


def _resource_path(*parts: str) -> str:
    """解析资源文件路径，兼容源码运行与 PyInstaller 打包成单文件的情况。"""
    if getattr(sys, "frozen", False) and hasattr(sys, "_MEIPASS"):
        base = sys._MEIPASS  # type: ignore[attr-defined]
    else:
        base = os.path.dirname(os.path.abspath(__file__))
    return os.path.join(base, *parts)


# 共享图标（只创建一次，绝不按书签逐个创建）
# 注意：若改成每个书签都加载 favicon，建树时会严重卡顿

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


# 主应用程序

class FavoritesViewer(tk.Tk):
    def __init__(self) -> None:
        super().__init__()
        self.title(_WINDOW_TITLE)
        self.configure(bg="#ffffff")
        self._apply_window_icon()

        self._root_node: Optional[BookmarkNode] = None
        self._file_path: Optional[str] = None
        self._node_map: Dict[str, BookmarkNode] = {}
        self._filter_job: Optional[str] = None
        self._info_refresh_job: Optional[str] = None
        self._total_folders: int = 0
        self._total_links: int = 0
        # 右键菜单的操作目标（即使选中项被全局点击清掉，也要保住它）
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
        # 等窗口真正映射到屏幕后再挂钩子：windnd 需要拿到有效的窗口句柄
        self.after(100, self._enable_file_drop)

        if len(sys.argv) > 1 and sys.argv[1].lower().endswith(".html"):
            self.after(50, lambda: self.open_file(sys.argv[1]))

    def _apply_window_icon(self) -> None:
        """如果存在 assets/icon.ico，就用它设置标题栏和任务栏图标。"""
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
        # 第一行：操作按钮
        self._toolbar = ttk.Frame(self, style="Toolbar.TFrame", padding=(8, 6, 8, 4))
        self._toolbar.pack(side=tk.TOP, fill=tk.X)

        ttk.Button(self._toolbar, text="打开文件", command=self.browse_open).pack(side=tk.LEFT, padx=(0, 6))
        ttk.Button(self._toolbar, text="关闭文件", command=self.close_file).pack(side=tk.LEFT, padx=(0, 6))
        ttk.Button(self._toolbar, text="完整截图", command=self.screenshot).pack(side=tk.LEFT, padx=(0, 6))
        ttk.Button(self._toolbar, text="中性截图", command=self.screenshot_neutral).pack(side=tk.LEFT, padx=(0, 6))
        ttk.Button(self._toolbar, text="全部展开", command=self.expand_all).pack(side=tk.LEFT, padx=(0, 4))
        ttk.Button(self._toolbar, text="全部折叠", command=self.collapse_all).pack(side=tk.LEFT, padx=(0, 0))

        # 第二行：筛选框（放在按钮下方，避免按钮被挤掉）
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
            text=_PATH_HINT,
            foreground="#888888",
        )
        self._path_label.pack(side=tk.LEFT, fill=tk.X)

        tree_frame = ttk.Frame(self)
        tree_frame.pack(side=tk.TOP, fill=tk.BOTH, expand=True, padx=4, pady=(0, 4))

        # 纯展示用的树：展开与折叠完全交给 ttk 处理，Python 侧不做任何计算
        self._tree = ttk.Treeview(tree_frame, show="tree", selectmode="browse")
        self._tree_vsb = ttk.Scrollbar(tree_frame, orient=tk.VERTICAL, command=self._tree.yview)
        self._tree_hsb = ttk.Scrollbar(tree_frame, orient=tk.HORIZONTAL, command=self._tree.xview)
        self._tree.configure(yscrollcommand=self._tree_vsb.set, xscrollcommand=self._tree_hsb.set)

        self._tree.grid(row=0, column=0, sticky="nsew")
        self._tree_vsb.grid(row=0, column=1, sticky="ns")
        self._tree_hsb.grid(row=1, column=0, sticky="ew")
        tree_frame.rowconfigure(0, weight=1)
        tree_frame.columnconfigure(0, weight=1)

        # 双击只负责打开链接，不在这里切换展开状态（树本身已经处理了）
        self._tree.bind("<Double-1>", self._on_double_click)
        self._tree.bind("<Return>", self._on_activate)
        self._tree.bind("<Button-3>", self._on_right_click)
        self._tree.bind("<Button-2>", self._on_right_click)
        # 点击树内空白处时取消选中高亮
        self._tree.bind("<Button-1>", self._on_tree_click, add="+")
        # 点击按钮、筛选框等树外区域时取消树的高亮
        self.bind_all("<Button-1>", self._on_global_click, add="+")
        # 信息栏：展开或折叠后刷新（部分 Windows 主题下只靠 Open/Close 事件并不可靠）
        self._tree.bind("<<TreeviewOpen>>", self._schedule_info_bar_refresh, add="+")
        self._tree.bind("<<TreeviewClose>>", self._schedule_info_bar_refresh, add="+")
        self._tree.bind("<ButtonRelease-1>", self._schedule_info_bar_refresh, add="+")
        self._tree.bind("<KeyRelease-Left>", self._schedule_info_bar_refresh, add="+")
        self._tree.bind("<KeyRelease-Right>", self._schedule_info_bar_refresh, add="+")

        # 底部两条栏：先打包状态栏（占最底部），再打包信息栏（位于状态栏上方）
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
        """宽度取自工具栏按钮行，并保留适中的默认高度与最小尺寸。"""
        self.update_idletasks()
        # 宽度 = 工具栏内容宽度 + 一点余量（留给窗口边框）
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

    # 文件操作

    def browse_open(self) -> None:
        path = filedialog.askopenfilename(
            title="选择收藏夹 HTML 文件",
            filetypes=[("网页收藏夹", "*.html"), ("HTML 文件", "*.html")],
            defaultextension=".html",
        )
        if path:
            self.open_file(path)

    # 拖放（借助 windnd 库）

    def _enable_file_drop(self) -> None:
        """允许把 .html 文件拖进窗口打开（Windows + windnd）。"""
        if sys.platform != "win32":
            return
        if windnd is None:
            return
        try:
            self.update_idletasks()
            # 只在窗口过程里排队，绝不在其中操作 Tk 控件
            self._pending_drop: Optional[List] = None

            def _on_drop(files) -> None:
                try:
                    self._pending_drop = list(files) if files else []
                except Exception:
                    self._pending_drop = None

            def _poll_drop() -> None:
                pending = self._pending_drop
                if pending is not None:
                    self._pending_drop = None
                    try:
                        self._on_files_dropped(pending)
                    except Exception as e:
                        messagebox.showerror("打开失败", f"无法打开拖入的文件：\n{e}")
                self.after(80, _poll_drop)

            # force_unicode=False：windnd 的 unicode 模式会算错缓冲区大小
            #（按字节数而不是宽字符个数），拖放时可能直接崩溃
            windnd.hook_dropfiles(self, func=_on_drop, force_unicode=False)
            self.after(80, _poll_drop)
        except Exception:
            pass

    def _on_files_dropped(self, paths: List) -> None:
        """处理拖放进来的路径，打开其中第一个 .html 文件。"""
        if not paths:
            return
        normalized: List[str] = []
        for p in paths:
            if isinstance(p, bytes):
                # 优先按文件系统编码解码；遇到异常编码再退回 GBK
                try:
                    p = os.fsdecode(p)
                except Exception:
                    p = p.decode("gbk", errors="replace")
            p = str(p).strip().strip('"')
            if p:
                normalized.append(p)
        html_paths = [p for p in normalized if p.lower().endswith(".html")]
        if not html_paths:
            messagebox.showerror(
                "格式不支持",
                "仅支持拖入 .html 格式的网页收藏夹文件。",
            )
            return
        self.open_file(html_paths[0])

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
        # 排除解析器虚拟根节点与顶层根文件夹（例如「收藏夹栏」）
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
        """清空当前收藏夹文件并重置界面。"""
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
            text=_PATH_HINT,
            foreground="#888888",
        )
        self.title(_WINDOW_TITLE)
        self._set_status_idle()
        self._update_info_bar()

    def _status_is_credit(self) -> bool:
        return str(self._status.cget("text")) == _STATUS_CREDIT

    def _set_status(self, text: str) -> None:
        """更新状态栏文案，并清除链接悬停样式。"""
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
        """树中的显示文案：首行显示完整文件名（含后缀）。"""
        if is_top and self._file_path:
            return os.path.basename(self._file_path)
        return node.title

    @staticmethod
    def _count_folders_excluding_roots(root: BookmarkNode) -> int:
        """统计顶层根文件夹之下的文件夹数量，不含这些根文件夹本身。

        例如：虚拟根节点 → 收藏夹栏 →（EdgeCtrl、WebJS……以及更深的层级）
        返回的只有 EdgeCtrl、WebJS 及其他子文件夹，不含「收藏夹栏」本身。
        """
        items = root.children if root.children else []
        total = 0
        for child in items:
            if child.is_folder:
                # count_folders() 会把节点自身也算进去，这里减掉这一层
                total += max(child.count_folders() - 1, 0)
        return total

    def _schedule_info_bar_refresh(self, event=None) -> None:
        """防抖并延后执行，确保 Treeview 的展开状态已经生效再统计。"""
        if self._info_refresh_job is not None:
            try:
                self.after_cancel(self._info_refresh_job)
            except Exception:
                pass
        # 稍作延迟：展开标记要等点击事件返回之后才会真正生效
        self._info_refresh_job = self.after(10, self._flush_info_bar_refresh)

    def _flush_info_bar_refresh(self) -> None:
        self._info_refresh_job = None
        self._update_info_bar()

    @staticmethod
    def _item_is_open(tree: ttk.Treeview, iid: str) -> bool:
        """更稳健地读取展开状态（Tk 可能返回 int、bool 或 str）。"""
        val = tree.item(iid, "open")
        if isinstance(val, str):
            return val.lower() in ("1", "true", "yes")
        return bool(val)

    def _update_info_bar(self) -> None:
        """显示当前可见（已展开）的节点数，以及文件中的总数。

        根文件夹那一行（深度 0，显示文件名）在「已显示」和「总数」中都不计入。
        只有位于已展开路径上的行才算「已显示」。
        """
        if self._root_node is None or not self._file_path:
            self._info.configure(text="文件夹：就绪，书签：就绪")
            return
        shown_folders = 0
        shown_links = 0
        for row in self._collect_visible_rows():
            # 忽略根文件夹（它显示的是完整文件名）
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

    # 截图（把当前展开状态画成一张长图）

    def screenshot(self) -> None:
        """完整截图：宽度随最长标题伸展。"""
        self._do_screenshot(mode="full")

    def screenshot_neutral(self) -> None:
        """中性截图：固定为树区域宽度，超长标题截断为 …。"""
        self._do_screenshot(mode="neutral")

    def _do_screenshot(self, mode: str = "full") -> None:
        """按当前展开状态截取整棵树并生成一张长图，然后弹出「另存为」对话框。"""
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
        """按显示顺序遍历 Treeview，只进入当前处于展开状态的文件夹。"""
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
        """截断文字并补上省略号，使其宽度不超过 max_width 像素。"""
        if max_width <= 0:
            return ""
        if draw.textlength(text, font=font) <= max_width:
            return text
        ell = "…"
        if draw.textlength(ell, font=font) > max_width:
            return ""
        # 二分查找：在能塞下省略号的前提下取最长的前缀
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

    # 截图绘制倍数：2 表示约 2 倍像素密度（文字和图标更清晰）
    _SHOT_SCALE = 2

    def _neutral_text_cap(self, draw: "ImageDraw.ImageDraw", rows: List[dict], font, scale: int = 1) -> int:
        """按整体名称长度给超长标题定一个上限，但不会窄于根标题。

        根（深度 0，也就是完整文件名）必须完整显示，不能截断。
        一级文件夹名也尽量完整显示，这样整棵树看起来更均衡。
        超出这个上限的极长书签标题会被截断并以 … 结尾。
        """
        all_widths = [self._text_pixel_width(draw, r["text"] or "", font) for r in rows]
        all_widths = [w for w in all_widths if w > 0]
        if not all_widths:
            return 280 * scale

        # 始终让根节点（深度 0）和它的下一级子节点完整显示
        must_fit = 0
        for r in rows:
            if r.get("depth", 0) <= 1:
                must_fit = max(
                    must_fit,
                    self._text_pixel_width(draw, r.get("text") or "", font),
                )

        all_widths.sort()
        # 取所有可见标题宽度的 90 分位（比之前略宽一些）
        idx = min(len(all_widths) - 1, max(0, int(round((len(all_widths) - 1) * 0.90))))
        bulk = int(all_widths[idx])

        # 下限约为 18 个汉字在 2 倍缩放下的宽度，并保证根节点与一级名称一定放得下
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
        """某一层级文字左侧所占的宽度（内边距 + 缩进 + 展开标记 + 图标）。"""
        return pad_x + depth * indent_w + indicator_w + icon_size + gap

    def _render_tree_image(self, rows: List[dict], mode: str = "full") -> "Image.Image":
        """把当前可见的树行绘制成一张长图（与展开状态保持一致，所见即所得）。

        宽度只由书签标题长度和树的层级决定，与 GUI 窗口大小无关。
        按 _SHOT_SCALE 倍数绘制，以获得更高分辨率。

        mode 取值：
          - full：每个标题都完整绘制，图片宽度按最长的一行计算
          - neutral：极长的标题截断为 …，宽度按其余名称计算
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

        # 计算每行显示的文字，宽度只按内容测量
        display_texts: List[str] = []
        line_widths: List[int] = []
        for row in rows:
            text = row["text"] or ""
            # 根文件名（深度 0）绝不截断；其余行在「中性截图」模式下可能被截断
            if text_cap is not None and row.get("depth", 0) > 0:
                text = self._fit_text(probe, text, font, text_cap)
            display_texts.append(text)
            chrome = self._row_chrome_width(
                row["depth"], pad_x, indent_w, indicator_w, icon_size, gap
            )
            line_widths.append(chrome + self._text_pixel_width(probe, text, font) + pad_x)

        # 右侧额外留一点内边距，避免文字紧贴边缘
        width = max(max(line_widths, default=0), 320 * s) + 8 * s
        height = pad_y * 2 + len(rows) * row_h

        img = Image.new("RGB", (width, height), "#ffffff")
        draw = ImageDraw.Draw(img)

        # 一圈很淡的边框
        draw.rectangle([0, 0, width - 1, height - 1], outline="#e0e0e0", width=border)

        folder_icon = self._pil_folder_icon(icon_size)
        star_icon = self._pil_star_icon(icon_size)
        link_icon = self._pil_link_icon(icon_size)

        for i, row in enumerate(rows):
            y0 = pad_y + i * row_h

            x = pad_x + row["depth"] * indent_w

            # 展开/折叠箭头（只有含子节点的行才画）
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

            # 能取到字体度量时，让文字垂直居中
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
        # 坐标随尺寸等比换算（原始设计按 16×16）
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

    # 树：整棵预先建好，所以展开与折叠是瞬时的（像 Edge 那样）

    def _clear_tree(self) -> None:
        items = self._tree.get_children()
        if items:
            self._tree.delete(*items)
        self._node_map.clear()

    def _build_full_tree(self, root: BookmarkNode, filter_text: str = "") -> None:
        """把所有可见节点一次性插入。之后的展开操作只是切换可见性。"""
        self._clear_tree()
        ft = filter_text.strip().lower()
        items = root.children if root.children else [root]

        # 快速路径：用栈迭代插入，之后点击展开就不需要再做任何计算
        if not ft:
            for i, node in enumerate(items):
                # 根的第一行：星标图标 + 完整文件名
                self._insert_all("", node, is_top=(i == 0))
            # 只展开工具栏根节点，子节点早已插入树中
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
        # open=False：默认折叠；数据都已就位，展开时瞬间完成
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

    # 全部展开 / 全部折叠（只改界面，不加载任何数据）

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

    # 交互操作

    def _selected_node(self) -> Optional[BookmarkNode]:
        sel = self._tree.selection()
        if not sel:
            return None
        return self._node_map.get(sel[0])

    def _clear_tree_selection(self) -> None:
        """取消树中的选中高亮。"""
        try:
            sel = self._tree.selection()
            if sel:
                self._tree.selection_remove(*sel)
            self._tree.focus("")
        except Exception:
            pass

    def _on_tree_click(self, event) -> None:
        """点击树的空白区域时取消选中高亮。"""
        if self._tree.identify_row(event.y):
            return
        self._clear_tree_selection()

    def _on_global_click(self, event) -> None:
        """点击树以外的区域（按钮、筛选框等）时取消树的高亮。"""
        w = event.widget
        # 操作树本身或它的滚动条时，保留选中状态
        if w is self._tree or w is self._tree_vsb or w is self._tree_hsb:
            return
        # 使用右键菜单时不取消选中，否则菜单里的操作会失效
        try:
            if isinstance(w, tk.Menu) or w.winfo_class() in ("Menu", "Menubutton"):
                return
        except Exception:
            pass
        # 忽略来自其他 Tk 窗口或对话框的事件（那些不属于本程序）
        try:
            if not str(w).startswith(str(self)):
                return
        except Exception:
            return
        self._clear_tree_selection()

    def _context_node(self) -> Optional[BookmarkNode]:
        """右键菜单操作的目标节点（优先用右键点击到的那一项，而不是当前选中项）。"""
        if self._ctx_node is not None:
            return self._ctx_node
        return self._selected_node()

    def _context_iid(self) -> Optional[str]:
        if self._ctx_iid:
            return self._ctx_iid
        sel = self._tree.selection()
        return sel[0] if sel else None

    def _on_double_click(self, event) -> Optional[str]:
        """双击书签时打开链接；双击文件夹交给 Treeview 自己展开，不做拦截。"""
        row = self._tree.identify_row(event.y)
        if not row:
            return None
        # 点的是树前面的展开箭头区域，就保持默认的展开/折叠行为
        if self._tree.identify_region(event.x, event.y) == "tree":
            element = self._tree.identify_element(event.x, event.y)
            # 某些主题下这个箭头元素的名称是 "Treeitem.indicator"
            if element and "indicator" in element:
                return None

        node = self._node_map.get(row)
        if node is None:
            return None
        if not node.is_folder and node.url:
            self._open_url(node.url)
            return "break"  # 避免打开链接时还触发一次多余的展开/折叠
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
            # 文件夹本身没有链接，提供「打开其下全部书签」的选项
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
            # 逐个依次打开，不并发，避免界面卡死
            self._open_urls_sequential(urls, title=title)
            return
        self._set_status("该项没有可打开的链接")

    def _open_urls_sequential(self, urls: list, title: str = "", delay_ms: int = 120) -> None:
        """按顺序依次打开每个链接，中间留一点间隔；绝不并发批量打开。"""
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
            # 安排下一次打开，让界面有机会在两次打开之间处理事件
            self.after(delay_ms, lambda: open_next(done))

        open_next(0)

    @staticmethod
    def _collect_folder_urls(node: BookmarkNode) -> list:
        """收集某个文件夹下的全部书签链接（深度优先）。"""
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
            # 根文件那一行优先用树中显示的文案
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
