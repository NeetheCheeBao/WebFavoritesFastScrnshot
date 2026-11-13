//! 树节点右键菜单（原生 Win32 弹出菜单）。

use std::ptr;

use winapi::shared::windef::{HWND, POINT};
use winapi::um::winuser::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, PostMessageW, SetForegroundWindow,
    TrackPopupMenu, MF_SEPARATOR, MF_STRING, TPM_LEFTALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON,
    TPM_TOPALIGN, WM_NULL,
};

use crate::platform::shell::to_wide;

/// 右键菜单可执行的动作。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContextAction {
    Open,
    CopyUrl,
    CopyTitle,
    Expand,
    Collapse,
}

/// 弹出菜单并返回用户选择；取消时返回 `None`。
pub fn show(hwnd: HWND, labels: [&str; 5]) -> Option<ContextAction> {
    unsafe {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return None;
        }

        for (index, label) in labels.iter().enumerate() {
            if index == 3 {
                AppendMenuW(menu, MF_SEPARATOR, 0, ptr::null());
            }
            let wide = to_wide(label);
            AppendMenuW(menu, MF_STRING, index + 1, wide.as_ptr());
        }

        let mut point = POINT { x: 0, y: 0 };
        GetCursorPos(&mut point);
        SetForegroundWindow(hwnd);

        let command = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_LEFTALIGN | TPM_TOPALIGN,
            point.x,
            point.y,
            0,
            hwnd,
            ptr::null(),
        );

        // 让菜单在点击别处时能正常消失
        PostMessageW(hwnd, WM_NULL, 0, 0);
        DestroyMenu(menu);

        match command {
            1 => Some(ContextAction::Open),
            2 => Some(ContextAction::CopyUrl),
            3 => Some(ContextAction::CopyTitle),
            4 => Some(ContextAction::Expand),
            5 => Some(ContextAction::Collapse),
            _ => None,
        }
    }
}
