//! 树的构建与可见行统计；整棵树一次性插入，展开/折叠只切换控件状态。

use native_windows_gui as nwg;

use crate::ui::icons;
use crate::ui::tree_view::{self, VisibleRow};

use super::{App, Document};

impl App {
    /// 按当前筛选词重建整棵树。
    pub fn rebuild_tree(&self) {
        self.ui.tree.clear();

        let document = self.state.borrow_mut().doc.take();
        let Some(document) = document else {
            self.state.borrow_mut().items.clear();
            self.update_info_bar();
            return;
        };

        let filter = self.ui.filter_input.text().trim().to_lowercase();
        let mut items: Vec<Option<nwg::TreeItem>> =
            (0..document.arena.nodes.len()).map(|_| None).collect();

        let root = document.arena.root();
        let top = if document.arena.nodes[root].children.is_empty() {
            vec![root]
        } else {
            document.arena.top_level(root)
        };

        if filter.is_empty() {
            for (index, &node) in top.iter().enumerate() {
                self.insert_subtree(&document, node, None, index == 0, 0, &mut items);
            }
            self.suppress_info.set(true);
            for &node in &top {
                if let Some(item) = items[node].as_ref() {
                    self.ui
                        .tree
                        .set_expand_state(item, nwg::ExpandState::Expand);
                }
            }
            self.suppress_info.set(false);
        } else {
            for (index, &node) in top.iter().enumerate() {
                self.insert_filtered(&document, node, None, index == 0, 0, &filter, &mut items);
            }
        }

        {
            let mut state = self.state.borrow_mut();
            state.items = items;
            state.doc = Some(document);
        }

        self.apply_favicon_indices();
        self.update_scrollbars();
    }

    /// 递归插入一个节点及其全部后代。
    fn insert_subtree(
        &self,
        document: &Document,
        node: usize,
        parent: Option<&nwg::TreeItem>,
        is_top: bool,
        depth: i32,
        items: &mut Vec<Option<nwg::TreeItem>>,
    ) -> nwg::TreeItem {
        let data = &document.arena.nodes[node];
        let text = if is_top {
            document.file_name.clone()
        } else {
            data.title.clone()
        };

        let item = match parent {
            Some(parent) => self.ui.tree.insert_item_with_param(
                &text,
                Some(parent),
                nwg::TreeInsert::Last,
                node as isize,
            ),
            None => self.ui.tree.insert_item_with_param(
                &text,
                None,
                nwg::TreeInsert::Root,
                node as isize,
            ),
        };

        let icon = if is_top {
            icons::ICON_STAR
        } else if data.is_folder {
            icons::ICON_FOLDER
        } else {
            icons::ICON_LINK
        };
        self.set_item_icon(&item, icon);

        items[node] = Some(tree_view::copy_item(&item));

        if data.is_folder {
            for &child in &data.children {
                self.insert_subtree(document, child, Some(&item), false, depth + 1, items);
            }
        }

        item
    }

    /// 筛选模式下的插入：只保留命中节点，文件夹默认展开。
    fn insert_filtered(
        &self,
        document: &Document,
        node: usize,
        parent: Option<&nwg::TreeItem>,
        is_top: bool,
        depth: i32,
        filter: &str,
        items: &mut Vec<Option<nwg::TreeItem>>,
    ) -> Option<nwg::TreeItem> {
        if !node_matches(document, node, filter) {
            return None;
        }

        let data = &document.arena.nodes[node];
        let text = if is_top {
            document.file_name.clone()
        } else {
            data.title.clone()
        };

        let item = match parent {
            Some(parent) => self.ui.tree.insert_item_with_param(
                &text,
                Some(parent),
                nwg::TreeInsert::Last,
                node as isize,
            ),
            None => self.ui.tree.insert_item_with_param(
                &text,
                None,
                nwg::TreeInsert::Root,
                node as isize,
            ),
        };

        let icon = if is_top {
            icons::ICON_STAR
        } else if data.is_folder {
            icons::ICON_FOLDER
        } else {
            icons::ICON_LINK
        };
        self.set_item_icon(&item, icon);
        items[node] = Some(tree_view::copy_item(&item));

        if data.is_folder {
            for &child in &data.children {
                self.insert_filtered(
                    document,
                    child,
                    Some(&item),
                    false,
                    depth + 1,
                    filter,
                    items,
                );
            }
            // 必须先插入子项再展开：对没有子项的行调用展开会被原生控件忽略
            self.ui
                .tree
                .set_expand_state(&item, nwg::ExpandState::Expand);
        }

        Some(item)
    }

/// `on_select = true` 只写选中态图标，普通态需用 `false` 单独设置。
    pub(crate) fn set_item_icon(&self, item: &nwg::TreeItem, icon: i32) {
        self.ui.tree.set_item_image(item, icon, false);
        self.ui.tree.set_item_image(item, icon, true);
    }

    /// 估算一行占用的像素宽度（缩进 + 展开框 + 图标 + 间距 + 文字）。
    pub(crate) fn row_width(&self, text: &str, depth: i32) -> i32 {
        let indent = crate::platform::dpi::scaled(20);
        let chrome = crate::platform::dpi::scaled(56);
        indent * depth + chrome + self.ui.measure_tree_text(text)
    }

    /// 按显示顺序收集当前可见的行（只进入已展开的文件夹）。
    pub fn collect_visible_rows(&self) -> Vec<VisibleRow> {
        let state = self.state.borrow();
        let Some(document) = state.doc.as_ref() else {
            return Vec::new();
        };

        let root = document.arena.root();
        let top = if document.arena.nodes[root].children.is_empty() {
            vec![root]
        } else {
            document.arena.top_level(root)
        };

        let mut rows = Vec::new();
        for node in top {
            if state
                .items
                .get(node)
                .and_then(|item| item.as_ref())
                .is_some()
            {
                collect_rows(document, &state.items, node, 0, &self.ui.tree, &mut rows);
            }
        }

        rows
    }

    /// 树项 -> 节点下标。
    pub fn node_of_item(&self, item: &nwg::TreeItem) -> Option<usize> {
        self.ui
            .tree
            .item_param(item)
            .and_then(|value| usize::try_from(value).ok())
    }

    /// 节点下标 -> 树项。
    pub fn item_of(&self, node: usize) -> Option<nwg::TreeItem> {
        let state = self.state.borrow();
        state
            .items
            .get(node)
            .and_then(|item| item.as_ref())
            .map(tree_view::copy_item)
    }
}

/// 递归收集可见行。
fn collect_rows(
    document: &Document,
    items: &[Option<nwg::TreeItem>],
    node: usize,
    depth: i32,
    tree: &nwg::TreeView,
    rows: &mut Vec<VisibleRow>,
) {
    let Some(item) = items.get(node).and_then(|item| item.as_ref()) else {
        return;
    };

    let data = &document.arena.nodes[node];
    let has_children = !data.children.is_empty();
    // 只有含子项的文件夹才需要向控件询问展开状态；链接节点跳过查询
    let is_open = has_children && tree_view::is_expanded(tree, item);
    // 标题取模型，避免逐行查询控件
    let text = if depth == 0 && data.is_folder {
        document.file_name.clone()
    } else {
        data.title.clone()
    };

    rows.push(VisibleRow {
        node,
        depth,
        text,
        is_folder: data.is_folder,
        is_top: depth == 0 && data.is_folder,
        is_open,
        has_children,
    });

    if is_open {
        for &child in &data.children {
            collect_rows(document, items, child, depth + 1, tree, rows);
        }
    }
}

/// 标题或 URL 命中即视为匹配；文件夹只要有一个后代命中就算命中。
fn node_matches(document: &Document, node: usize, filter: &str) -> bool {
    let data = &document.arena.nodes[node];

    if data.title.to_lowercase().contains(filter) {
        return true;
    }
    if let Some(url) = &data.url {
        if url.to_lowercase().contains(filter) {
            return true;
        }
    }
    if data.is_folder {
        return data
            .children
            .iter()
            .any(|&child| node_matches(document, child, filter));
    }

    false
}
