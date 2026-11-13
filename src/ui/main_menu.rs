//! 工具栏的汉堡菜单：语言包与检查更新。

use std::ptr;

use winapi::shared::windef::{HWND, POINT};
use winapi::um::winuser::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, PostMessageW, SetForegroundWindow, TrackPopupMenu,
    MF_CHECKED, MF_POPUP, MF_SEPARATOR, MF_STRING, TPM_LEFTALIGN, TPM_RETURNCMD, TPM_TOPALIGN,
    WM_NULL,
};

use crate::platform::shell::to_wide;

pub enum MainMenuAction {
    Language(String),
    CheckUpdates,
}

const ID_CHECK_UPDATES: usize = 1;
const ID_LANGUAGE_BASE: usize = 100;

pub fn show(
    hwnd: HWND,
    anchor: POINT,
    language_label: &str,
    update_label: &str,
    languages: &[(String, String)],
    current: &str,
) -> Option<MainMenuAction> {
    unsafe {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return None;
        }

        let submenu = CreatePopupMenu();
        for (index, (code, name)) in languages.iter().enumerate() {
            let wide = to_wide(name);
            let flags = MF_STRING | if code == current { MF_CHECKED } else { 0 };
            AppendMenuW(submenu, flags, ID_LANGUAGE_BASE + index, wide.as_ptr());
        }

        let language = to_wide(language_label);
        AppendMenuW(menu, MF_POPUP, submenu as usize, language.as_ptr());
        AppendMenuW(menu, MF_SEPARATOR, 0, ptr::null());

        let update = to_wide(update_label);
        AppendMenuW(menu, MF_STRING, ID_CHECK_UPDATES, update.as_ptr());

        SetForegroundWindow(hwnd);
        let command = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_LEFTALIGN | TPM_TOPALIGN,
            anchor.x,
            anchor.y,
            0,
            hwnd,
            ptr::null(),
        );

        // 让菜单在点击别处时能正常消失
        PostMessageW(hwnd, WM_NULL, 0, 0);
        DestroyMenu(menu);

        if command <= 0 {
            return None;
        }
        if command as usize == ID_CHECK_UPDATES {
            return Some(MainMenuAction::CheckUpdates);
        }

        let index = command as usize - ID_LANGUAGE_BASE;
        languages
            .get(index)
            .map(|(code, _)| MainMenuAction::Language(code.clone()))
    }
}
