//! 树的鼠标交互与右键菜单动作。

use native_windows_gui as nwg;

use crate::platform::shell;
use crate::ui::context_menu::{self, ContextAction};
use crate::ui::tree_view;

use super::App;

impl App {
    /// 点击树的空白区域时取消高亮。
    pub fn on_tree_click(&self) {
        let Some(hwnd) = self.ui.tree.handle.hwnd() else {
            return;
        };
        let (x, y) = tree_view::cursor_client_pos(hwnd);

        if tree_view::hit_test(&self.ui.tree, x, y).is_none() {
            self.clear_tree_selection();
        }
    }

    /// 双击书签打开链接；双击文件夹交给原生控件展开/折叠。
    pub fn on_tree_double_click(&self) {
        let Some(hwnd) = self.ui.tree.handle.hwnd() else {
            return;
        };
        let (x, y) = tree_view::cursor_client_pos(hwnd);
        let Some(item) = tree_view::hit_test(&self.ui.tree, x, y) else {
            return;
        };
        let Some(node) = self.node_of_item(&item) else {
            return;
        };

        let url = {
            let state = self.state.borrow();
            state.doc.as_ref().and_then(|document| {
                let data = &document.arena.nodes[node];
                if data.is_folder {
                    None
                } else {
                    data.url.clone()
                }
            })
        };

        if let Some(url) = url {
            self.open_url(&url);
        }
    }

    /// 右键：先选中命中的节点，再弹出菜单。
    pub fn on_tree_right_click(&self) {
        let Some(hwnd) = self.ui.tree.handle.hwnd() else {
            return;
        };
        let (x, y) = tree_view::cursor_client_pos(hwnd);
        let Some(item) = tree_view::hit_test(&self.ui.tree, x, y) else {
            return;
        };

        self.ui.tree.select_item(&item);
        self.ui.tree.set_focus();

        let Some(node) = self.node_of_item(&item) else {
            return;
        };
        self.state.borrow_mut().ctx_node = Some(node);

        self.show_context_menu(node);
    }

    fn show_context_menu(&self, node: usize) {
        let Some(hwnd) = self.ui.window.handle.hwnd() else {
            return;
        };

        let labels: Vec<String> = {
            let i18n = self.i18n.borrow();
            [
                "menu.open_in_browser",
                "menu.copy_url",
                "menu.copy_title",
                "menu.expand_item",
                "menu.collapse_item",
            ]
            .iter()
            .map(|key| i18n.t(key).to_string())
            .collect()
        };
        let labels = [
            labels[0].as_str(),
            labels[1].as_str(),
            labels[2].as_str(),
            labels[3].as_str(),
            labels[4].as_str(),
        ];

        let Some(action) = context_menu::show(hwnd, labels) else {
            return;
        };

        match action {
            ContextAction::Open => self.menu_open(node),
            ContextAction::CopyUrl => self.menu_copy_url(node),
            ContextAction::CopyTitle => self.menu_copy_title(node),
            ContextAction::Expand => self.menu_expand(node),
            ContextAction::Collapse => self.menu_collapse(node),
        }
    }

    /// 「在浏览器中打开」：书签直接打开，文件夹确认后按序打开其下全部书签。
    fn menu_open(&self, node: usize) {
        let (is_folder, url, title, urls) = {
            let state = self.state.borrow();
            let Some(document) = state.doc.as_ref() else {
                return;
            };
            let data = &document.arena.nodes[node];

            let mut urls = Vec::new();
            if data.is_folder {
                document.arena.collect_urls(node, &mut urls);
            }

            (data.is_folder, data.url.clone(), data.title.clone(), urls)
        };

        if !is_folder {
            match url {
                Some(url) => self.open_url(&url),
                None => {
                    let message = self.i18n.borrow().t("status.no_openable_link").to_string();
                    self.set_status(message);
                }
            }
            return;
        }

        if urls.is_empty() {
            let text = self.i18n.borrow().fmt("folder.empty", &[("title", &title)]);
            self.info_box("folder.empty_title", &text);
            return;
        }

        let text = self.i18n.borrow().fmt(
            "folder.confirm",
            &[("title", &title), ("count", &urls.len().to_string())],
        );
        let confirm_title = self.i18n.borrow().t("folder.confirm_title").to_string();

        if shell::confirm(self.ui.window.handle.hwnd(), &confirm_title, &text) {
            self.start_sequential_open(urls);
        }
    }

    fn menu_copy_url(&self, node: usize) {
        let url = {
            let state = self.state.borrow();
            state
                .doc
                .as_ref()
                .and_then(|document| document.arena.nodes[node].url.clone())
        };

        match url {
            Some(url) => {
                shell::copy_text(&url);
                let message = self.i18n.borrow().t("status.copied_url").to_string();
                self.set_status(message);
            }
            None => {
                let message = self.i18n.borrow().t("status.no_link").to_string();
                self.set_status(message);
            }
        }
    }

    fn menu_copy_title(&self, node: usize) {
        let text = {
            let state = self.state.borrow();
            let Some(document) = state.doc.as_ref() else {
                return;
            };

            // 首行显示的是文件名，复制时也以界面文案为准
            if document.arena.nodes[document.arena.root()]
                .children
                .contains(&node)
            {
                document.file_name.clone()
            } else {
                document.arena.nodes[node].title.clone()
            }
        };

        shell::copy_text(&text);
        let message = self.i18n.borrow().t("status.copied_title").to_string();
        self.set_status(message);
    }

    fn menu_expand(&self, node: usize) {
        if let Some(item) = self.item_of(node) {
            self.expand_recursive(&item);
        }
        self.update_info_bar();
    }

    fn menu_collapse(&self, node: usize) {
        if let Some(item) = self.item_of(node) {
            self.collapse_recursive(&item);
        }
        self.update_info_bar();
    }

    /// 递归折叠（子项先折叠），保留节点本身。
    fn collapse_recursive(&self, item: &nwg::TreeItem) {
        let mut child = self.ui.tree.first_child(item);
        while let Some(current) = child {
            self.collapse_recursive(&current);
            child = self.ui.tree.next_sibling(&current);
        }
        self.ui
            .tree
            .set_expand_state(item, nwg::ExpandState::Collapse);
    }

    fn expand_recursive(&self, item: &nwg::TreeItem) {
        self.ui
            .tree
            .set_expand_state(item, nwg::ExpandState::Expand);

        let mut child = self.ui.tree.first_child(item);
        while let Some(current) = child {
            self.expand_recursive(&current);
            child = self.ui.tree.next_sibling(&current);
        }
    }
}
