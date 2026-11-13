//! 打开外部链接、剪贴板、消息框。

use std::ptr;

use winapi::shared::windef::HWND;
use winapi::um::shellapi::ShellExecuteW;
use winapi::um::winbase::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use winapi::um::winuser::{
    CloseClipboard, EmptyClipboard, MessageBoxW, OpenClipboard, SetClipboardData, CF_UNICODETEXT,
    IDYES, MB_ICONERROR, MB_ICONINFORMATION, MB_ICONQUESTION, MB_OK, MB_YESNO, SW_SHOWNORMAL,
};

pub fn to_wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn open_url(url: &str) -> Result<(), String> {
    let operation = to_wide("open");
    let target = to_wide(url);

    let result = unsafe {
        ShellExecuteW(
            ptr::null_mut(),
            operation.as_ptr(),
            target.as_ptr(),
            ptr::null(),
            ptr::null(),
            SW_SHOWNORMAL,
        )
    };

    let code = result as isize;
    if code > 32 {
        Ok(())
    } else {
        Err(format!("ShellExecuteW 返回 {code}"))
    }
}

pub fn reveal_in_explorer(path: &std::path::Path) -> Result<(), String> {
    let program = to_wide("explorer.exe");
    let parameters = to_wide(&format!("/select,\"{}\"", path.display()));

    let result = unsafe {
        ShellExecuteW(
            ptr::null_mut(),
            ptr::null(),
            program.as_ptr(),
            parameters.as_ptr(),
            ptr::null(),
            SW_SHOWNORMAL,
        )
    };

    let code = result as isize;
    if code > 32 {
        Ok(())
    } else {
        Err(format!("ShellExecuteW 返回 {code}"))
    }
}

#[derive(Clone, Copy)]
pub enum MessageIcon {
    Error,
    Info,
}

pub fn message(parent: Option<HWND>, title: &str, text: &str, icon: MessageIcon) {
    let icon_flag = match icon {
        MessageIcon::Error => MB_ICONERROR,
        MessageIcon::Info => MB_ICONINFORMATION,
    };
    show(parent, title, text, MB_OK | icon_flag);
}

pub fn confirm(parent: Option<HWND>, title: &str, text: &str) -> bool {
    show(parent, title, text, MB_YESNO | MB_ICONQUESTION) == IDYES
}

fn show(parent: Option<HWND>, title: &str, text: &str, flags: u32) -> i32 {
    let title = to_wide(title);
    let text = to_wide(text);

    unsafe {
        MessageBoxW(
            parent.unwrap_or(ptr::null_mut()),
            text.as_ptr(),
            title.as_ptr(),
            flags,
        )
    }
}

/// 把文本写入剪贴板（失败时静默忽略，不影响主流程）。
pub fn copy_text(text: &str) {
    let wide = to_wide(text);

    unsafe {
        if OpenClipboard(ptr::null_mut()) == 0 {
            return;
        }
        EmptyClipboard();

        let bytes = wide.len() * std::mem::size_of::<u16>();
        let handle = GlobalAlloc(GMEM_MOVEABLE, bytes);
        if !handle.is_null() {
            let buffer = GlobalLock(handle) as *mut u16;
            if !buffer.is_null() {
                std::ptr::copy_nonoverlapping(wide.as_ptr(), buffer, wide.len());
                GlobalUnlock(handle);
                SetClipboardData(CF_UNICODETEXT, handle);
            }
        }

        CloseClipboard();
    }
}
