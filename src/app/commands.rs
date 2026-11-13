//! 文件打开/关闭、筛选、展开折叠、截图等命令。

use std::path::Path;

use native_windows_gui as nwg;

use crate::config;
use crate::parser::{self, ParseError, ParseOptions};
use crate::platform::{dialogs, encoding, shell};
use crate::screenshot::{self, ShotMode};
use crate::ui::tree_view;

use super::{App, Document};

impl App {
    /// 弹出文件对话框并打开所选收藏夹。
    pub fn browse_open(&self) {
        let title = self.i18n.t("dialog.open_title").to_string();
        let bookmarks = self.i18n.t("dialog.filter_bookmarks").to_string();
        let html = self.i18n.t("dialog.filter_html").to_string();

        // 默认从「下载」目录开始；取不到时退回当前文档目录，再退回系统记忆
        let initial_dir = crate::platform::paths::downloads_dir().or_else(|| {
            let state = self.state.borrow();
            state
                .doc
                .as_ref()
                .and_then(|doc| doc.path.parent().map(|parent| parent.to_path_buf()))
        });

        if let Some(path) = dialogs::open_html(
            self.ui.window.handle.hwnd(),
            &title,
            &bookmarks,
            &html,
            initial_dir.as_deref(),
        ) {
            self.open_file(&path);
        }
    }

    /// 读取并解析收藏夹文件，成功后重建整棵树。
    pub fn open_file(&self, path: &Path) {
        // 与原版一致：先转成绝对路径，路径栏与截图默认目录都基于它
        let path = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());

        if !config::is_html_path(&path) {
            self.error(
                "error.unsupported_format_title",
                "error.unsupported_format",
                &[],
            );
            return;
        }

        if !path.is_file() {
            self.error(
                "error.file_missing_title",
                "error.file_missing",
                &[("path", &path.to_string_lossy())],
            );
            return;
        }

        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) => {
                self.error(
                    "error.read_failed_title",
                    "error.read_failed",
                    &[("error", &error.to_string())],
                );
                return;
            }
        };

        let content = encoding::decode_bookmarks(&bytes);
        let options = ParseOptions {
            virtual_root: self.i18n.t("parser.virtual_root"),
            unnamed_folder: self.i18n.t("parser.unnamed_folder"),
            unnamed_link: self.i18n.t("parser.unnamed_link"),
        };

        let arena = match parser::parse_bookmarks_html(&content, &options) {
            Ok(arena) => arena,
            Err(error) => {
                let message = match error {
                    ParseError::MissingDl => self.i18n.t("parser.missing_dl").to_string(),
                };
                self.error(
                    "error.parse_failed_title",
                    "error.parse_failed",
                    &[("error", &message)],
                );
                return;
            }
        };

        let total_links = arena.count_links(arena.root());
        let total_folders = arena.count_folders_excluding_roots(arena.root());
        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();

        {
            let mut state = self.state.borrow_mut();
            state.doc = Some(Document {
                arena,
                path: path.clone(),
                file_name,
                total_folders,
                total_links,
            });
            let path_text = path.to_string_lossy().to_string();
            state.path_text_width = self.ui.measure_ui_text(&path_text);
            state.path_text = path_text;
            state.path_dim = false;
            state.ctx_node = None;
        }

        // 清空筛选框（不触发筛选重建）
        self.filter_guard.set(true);
        self.ui.filter_input.set_text("");
        self.filter_guard.set(false);

        self.rebuild_tree();
        self.relayout();

        let message = self.i18n.fmt(
            "status.loaded",
            &[
                ("folders", &total_folders.to_string()),
                ("links", &total_links.to_string()),
            ],
        );
        self.set_status(message);
        self.update_info_bar();
    }

    /// 重新加载当前文件。
    pub fn reload(&self) {
        let path = {
            let state = self.state.borrow();
            state.doc.as_ref().map(|doc| doc.path.clone())
        };

        if let Some(path) = path {
            self.open_file(&path);
        }
    }

    /// 关闭当前文件并重置界面。
    pub fn close_file(&self) {
        self.ui.open_timer.stop();

        {
            let mut state = self.state.borrow_mut();
            state.doc = None;
            state.items.clear();
            state.ctx_node = None;
            state.pending_urls.clear();
            state.urls_total = 0;
            state.urls_done = 0;
            state.path_text = self.i18n.t("path.hint").to_string();
            state.path_text_width = 0;
            state.path_dim = true;
            state.info = self.i18n.t("info.ready").to_string();
        }

        self.filter_guard.set(true);
        self.ui.filter_input.set_text("");
        self.filter_guard.set(false);

        self.ui.tree.clear();
        self.relayout();
        self.set_status_idle();
    }

    /// 输入变化后延迟应用筛选，避免每个按键都重建整棵树。
    pub fn schedule_filter(&self) {
        self.ui.filter_timer.stop();
        self.ui.filter_timer.start();
    }

    pub fn apply_filter(&self) {
        self.rebuild_tree();
        self.update_info_bar();
    }

    /// 展开所有节点。
    pub fn expand_all(&self) {
        let items: Vec<nwg::TreeItem> = {
            let state = self.state.borrow();
            state
                .items
                .iter()
                .flatten()
                .map(tree_view::copy_item)
                .collect()
        };

        self.suppress_info.set(true);
        tree_view::set_redraw(&self.ui.tree, false);
        for item in &items {
            self.ui
                .tree
                .set_expand_state(item, nwg::ExpandState::Expand);
        }
        tree_view::set_redraw(&self.ui.tree, true);
        tree_view::refresh(&self.ui.tree);
        self.suppress_info.set(false);

        self.update_scrollbars();
        self.update_info_bar();
    }

    /// 折叠所有节点，但保留顶层展开（与原版一致）。
    pub fn collapse_all(&self) {
        // 注意：不能用 ExpandState::CollapseReset —— 它对应 TVE_COLLAPSERESET，
        // 会直接把子项删掉，之后再也展不开。
        let all: Vec<nwg::TreeItem> = {
            let state = self.state.borrow();
            state
                .items
                .iter()
                .flatten()
                .map(tree_view::copy_item)
                .collect()
        };
        self.suppress_info.set(true);
        tree_view::set_redraw(&self.ui.tree, false);
        for item in &all {
            self.ui
                .tree
                .set_expand_state(item, nwg::ExpandState::Collapse);
        }

        let top: Vec<nwg::TreeItem> = {
            let state = self.state.borrow();
            let Some(doc) = state.doc.as_ref() else {
                return;
            };
            doc.arena
                .top_level(doc.arena.root())
                .into_iter()
                .filter_map(|node| {
                    state
                        .items
                        .get(node)
                        .and_then(|item| item.as_ref())
                        .map(tree_view::copy_item)
                })
                .collect()
        };

        for item in &top {
            self.ui
                .tree
                .set_expand_state(item, nwg::ExpandState::Expand);
        }
        tree_view::set_redraw(&self.ui.tree, true);
        tree_view::refresh(&self.ui.tree);
        self.suppress_info.set(false);

        self.update_scrollbars();
        self.update_info_bar();
    }

    /// 刷新信息栏：已显示 / 总数（不含根文件夹那一行）。
    pub fn update_info_bar(&self) {
        let has_document = self.state.borrow().doc.is_some();

        let text = if !has_document {
            self.i18n.t("info.ready").to_string()
        } else {
            let rows = self.collect_visible_rows();
            let mut shown_folders = 0usize;
            let mut shown_links = 0usize;

            for row in rows.iter().filter(|row| row.depth > 0) {
                if row.is_folder {
                    shown_folders += 1;
                } else {
                    shown_links += 1;
                }
            }

            let (total_folders, total_links) = {
                let state = self.state.borrow();
                state
                    .doc
                    .as_ref()
                    .map(|doc| (doc.total_folders, doc.total_links))
                    .unwrap_or((0, 0))
            };

            let folders = shown_folders.to_string();
            let links = shown_links.to_string();
            let all_folders = total_folders.to_string();
            let all_links = total_links.to_string();

            self.i18n.fmt(
                "info.format",
                &[
                    ("shown_folders", &folders),
                    ("total_folders", &all_folders),
                    ("shown_links", &links),
                    ("total_links", &all_links),
                ],
            )
        };

        self.state.borrow_mut().info = text;
        self.refresh_bars();
    }

    /// 生成并保存长图截图。
    pub fn take_screenshot(&self, mode: ShotMode) {
        let rows = self.collect_visible_rows();

        if rows.is_empty() {
            let text = self.i18n.t("screenshot.empty").to_string();
            self.info_box("screenshot.empty_title", &text);
            return;
        }

        let png = match screenshot::render(&rows, mode) {
            Ok(bytes) => bytes,
            Err(error) => {
                self.error(
                    "error.screenshot_failed_title",
                    "error.screenshot_failed",
                    &[("error", &error)],
                );
                return;
            }
        };

        let (directory, file_name) = self.screenshot_target();
        let title = self.i18n.t("dialog.save_title").to_string();
        let png_filter = self.i18n.t("dialog.filter_png").to_string();
        let all_filter = self.i18n.t("dialog.filter_all").to_string();

        let Some(mut path) = dialogs::save_png(
            self.ui.window.handle.hwnd(),
            &title,
            directory.as_deref(),
            &file_name,
            &png_filter,
            &all_filter,
        ) else {
            let message = self.i18n.t("status.screenshot_cancelled").to_string();
            self.set_status(message);
            return;
        };

        if !path
            .to_string_lossy()
            .to_ascii_lowercase()
            .ends_with(".png")
        {
            path.set_extension("png");
        }

        match screenshot::save(&path, &png) {
            Ok(()) => {
                let message = self.i18n.fmt(
                    "status.screenshot_saved",
                    &[("path", &path.to_string_lossy())],
                );
                self.set_status(message);
            }
            Err(error) => {
                self.error(
                    "error.save_failed_title",
                    "error.save_failed",
                    &[("error", &error)],
                );
            }
        }
    }

    /// 截图默认目录与文件名。
    fn screenshot_target(&self) -> (Option<std::path::PathBuf>, String) {
        let suffix = self.i18n.t("screenshot.default_suffix");
        let fallback = self.i18n.t("screenshot.fallback_stem");

        let state = self.state.borrow();
        match state.doc.as_ref() {
            Some(doc) => {
                let stem = doc
                    .path
                    .file_stem()
                    .map(|stem| stem.to_string_lossy().to_string())
                    .unwrap_or_else(|| fallback.to_string());
                (
                    doc.path.parent().map(|parent| parent.to_path_buf()),
                    format!("{stem}{suffix}"),
                )
            }
            None => (std::env::current_dir().ok(), format!("{fallback}{suffix}")),
        }
    }

    /// 用系统浏览器打开链接。
    pub fn open_url(&self, url: &str) {
        if let Err(error) = shell::open_url(url) {
            self.error(
                "error.open_failed_title",
                "error.open_failed",
                &[("error", &error)],
            );
        }
    }

    /// 依次打开一批链接（间隔 120ms，避免一次性拉起大量窗口）。
    pub fn start_sequential_open(&self, urls: Vec<String>) {
        if urls.is_empty() {
            return;
        }

        let total = urls.len();
        {
            let mut state = self.state.borrow_mut();
            state.pending_urls = urls.into();
            state.urls_total = total;
            state.urls_done = 0;
        }

        let message = self.i18n.fmt(
            "status.opening_progress",
            &[("done", "0"), ("total", &total.to_string())],
        );
        self.set_status(message);

        self.ui.open_timer.stop();
        self.ui.open_timer.start();
    }

    /// 打开队列中的下一个链接。
    pub fn open_next_url(&self) {
        let next = {
            let mut state = self.state.borrow_mut();
            state.pending_urls.pop_front()
        };

        match next {
            Some(url) => {
                self.open_url(&url);
                let (done, total) = {
                    let mut state = self.state.borrow_mut();
                    state.urls_done += 1;
                    (state.urls_done, state.urls_total)
                };
                let message = self.i18n.fmt(
                    "status.opening_progress",
                    &[("done", &done.to_string()), ("total", &total.to_string())],
                );
                self.set_status(message);
            }
            None => {
                self.ui.open_timer.stop();
                let total = self.state.borrow().urls_total;
                let message = self
                    .i18n
                    .fmt("status.opened_all", &[("total", &total.to_string())]);
                self.set_status(message);
            }
        }
    }

    /// 处理拖放进来的文件：只接受第一个 .html。
    pub fn on_files_dropped(&self, files: Vec<String>) {
        let html: Vec<String> = files
            .into_iter()
            .filter(|path| path.to_ascii_lowercase().ends_with(".html"))
            .collect();

        match html.first() {
            Some(path) => self.open_file(Path::new(path)),
            None => {
                let text = self.i18n.t("error.unsupported_drop").to_string();
                let title = self.i18n.t("error.unsupported_format_title").to_string();
                shell::message(
                    self.ui.window.handle.hwnd(),
                    &title,
                    &text,
                    shell::MessageIcon::Error,
                );
            }
        }
    }

    /// 打开当前选中项（回车键）。
    pub fn open_selected(&self) {
        let url = {
            let state = self.state.borrow();
            let Some(doc) = state.doc.as_ref() else {
                return;
            };
            let node = self
                .ui
                .tree
                .selected_item()
                .and_then(|item| self.ui.tree.item_param(&item))
                .and_then(|value| usize::try_from(value).ok());

            match node {
                Some(index) if !doc.arena.nodes[index].is_folder => {
                    doc.arena.nodes[index].url.clone()
                }
                _ => None,
            }
        };

        if let Some(url) = url {
            self.open_url(&url);
        }
    }
}
