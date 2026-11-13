//! 「加载图标 / 卸载图标」：解析收藏夹内嵌的 favicon 并替换树的 ImageList。

use std::collections::HashMap;

use winapi::um::winuser::{PostMessageW, RedrawWindow, RDW_INVALIDATE, RDW_UPDATENOW};

use crate::ui::{favicons, icons};

use super::{App, Favicons};

impl App {
    pub fn toggle_favicons(&self) {
        if self.favicons.borrow().is_some() {
            self.unload_favicons();
            return;
        }

        if self.state.borrow().doc.is_none() {
            return;
        }

        let loading = self.i18n.borrow().t("toolbar.loading_icons").to_string();
        self.ui.set_icons_button(&loading, false);
        self.paint_icons_button();

        if let Some(hwnd) = self.ui.window.handle.hwnd() {
            unsafe {
                PostMessageW(hwnd, super::WM_APP_LOAD_ICONS, 0, 0);
            }
        }
    }

    /// 由 `WM_APP_LOAD_ICONS` 触发：按钮已完成重绘，这里才做解码。
    pub(crate) fn load_favicons(&self) {
        let entries: Vec<Option<String>> = {
            let state = self.state.borrow();
            let Some(doc) = state.doc.as_ref() else {
                return;
            };
            doc.arena.nodes.iter().map(|node| node.icon.clone()).collect()
        };

        let loading = self.i18n.borrow().t("toolbar.loading_icons").to_string();
        self.ui.set_icons_button(&loading, false);
        self.paint_icons_button();

        let mut unique: HashMap<String, usize> = HashMap::new();
        let mut pixels: Vec<Vec<u8>> = Vec::new();
        let mut indices: Vec<Option<i32>> = vec![None; entries.len()];

        for (node, entry) in entries.iter().enumerate() {
            let Some(uri) = entry.as_deref() else {
                continue;
            };
            if uri.is_empty() {
                continue;
            }

            if let Some(&slot) = unique.get(uri) {
                indices[node] = Some(icons::FAVICON_BASE + slot as i32);
                continue;
            }

            let Some(rgba) = favicons::decode_data_uri(uri) else {
                continue;
            };
            let slot = pixels.len();
            pixels.push(rgba);
            unique.insert(uri.to_string(), slot);
            indices[node] = Some(icons::FAVICON_BASE + slot as i32);
        }

        let list = match icons::build_image_list_with_favicons(&pixels) {
            Ok(list) => list,
            Err(_) => {
                let text = self.i18n.borrow().t("toolbar.load_icons").to_string();
                self.ui.set_icons_button(&text, true);
                return;
            }
        };

        self.ui.tree.set_image_list(Some(&list));
        *self.favicons.borrow_mut() = Some(Favicons {
            list,
            indices,
            pixels,
        });
        self.apply_favicon_indices();

        let text = self.i18n.borrow().t("toolbar.unload_icons").to_string();
        self.ui.set_icons_button(&text, true);
    }

    pub(crate) fn unload_favicons(&self) {
        if self.favicons.borrow().is_none() {
            return;
        }

        self.ui.tree.set_image_list(Some(&self.ui.image_list));
        *self.favicons.borrow_mut() = None;
        self.apply_favicon_indices();

        let text = self.i18n.borrow().t("toolbar.load_icons").to_string();
        let enabled = self.state.borrow().doc.is_some();
        self.ui.set_icons_button(&text, enabled);
    }

    /// 按当前 favicon 表刷新每个树项的图像索引。
    pub fn apply_favicon_indices(&self) {
        let favicons = self.favicons.borrow();
        let state = self.state.borrow();
        let Some(doc) = state.doc.as_ref() else {
            return;
        };
        let root = doc.arena.root();

        for (node, item) in state.items.iter().enumerate() {
            let Some(item) = item else {
                continue;
            };
            let index = if node == root {
                icons::ICON_STAR
            } else if doc.arena.nodes[node].is_folder {
                icons::ICON_FOLDER
            } else {
                match favicons.as_ref() {
                    Some(fav) => fav
                        .indices
                        .get(node)
                        .copied()
                        .flatten()
                        .unwrap_or(icons::ICON_GLOBE),
                    None => icons::ICON_LINK,
                }
            };
            self.set_item_icon(item, index);
        }
    }

    /// 同步绘制按钮的文案与禁用态，否则加载期间界面不会刷新。
    fn paint_icons_button(&self) {
        if let Some(hwnd) = self.ui.btn_icons.handle.hwnd() {
            unsafe {
                RedrawWindow(
                    hwnd,
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    RDW_INVALIDATE | RDW_UPDATENOW,
                );
            }
        }
    }
}
