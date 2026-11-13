//! 打开/保存文件对话框（`GetOpenFileNameW` / `GetSaveFileNameW`）。

use std::mem::{size_of, zeroed};
use std::path::{Path, PathBuf};
use std::ptr;

use winapi::shared::windef::HWND;
use winapi::um::commdlg::{
    GetOpenFileNameW, GetSaveFileNameW, OFN_ENABLESIZING, OFN_EXPLORER, OFN_FILEMUSTEXIST,
    OFN_NOCHANGEDIR, OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST, OPENFILENAMEW,
};

use crate::platform::shell::to_wide;

/// 文件名缓冲区长度（字符数）。
const BUFFER_LEN: usize = 1024;

/// 选择收藏夹 HTML 文件。
pub fn open_html(
    parent: Option<HWND>,
    title: &str,
    desc_bookmarks: &str,
    desc_html: &str,
    initial_dir: Option<&Path>,
) -> Option<PathBuf> {
    let filter = build_filter(&[(desc_bookmarks, "*.html"), (desc_html, "*.html")]);
    let title = to_wide(title);
    let default_ext = to_wide("html");
    let initial_dir = initial_dir.map(|dir| to_wide(&dir.to_string_lossy()));
    let mut buffer = vec![0u16; BUFFER_LEN];

    let mut dialog: OPENFILENAMEW = unsafe { zeroed() };
    dialog.lStructSize = size_of::<OPENFILENAMEW>() as u32;
    dialog.hwndOwner = parent.unwrap_or(ptr::null_mut());
    dialog.lpstrFilter = filter.as_ptr();
    dialog.nFilterIndex = 1;
    dialog.lpstrFile = buffer.as_mut_ptr();
    dialog.nMaxFile = BUFFER_LEN as u32;
    dialog.lpstrTitle = title.as_ptr();
    dialog.lpstrDefExt = default_ext.as_ptr();
    dialog.lpstrInitialDir = initial_dir
        .as_ref()
        .map(|dir| dir.as_ptr())
        .unwrap_or(ptr::null());
    dialog.Flags =
        OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR | OFN_ENABLESIZING;

    if unsafe { GetOpenFileNameW(&mut dialog) } == 0 {
        return None;
    }

    Some(buffer_to_path(&buffer))
}

/// 选择 PNG 保存路径，默认文件名与初始目录可指定。
pub fn save_png(
    parent: Option<HWND>,
    title: &str,
    initial_dir: Option<&Path>,
    initial_name: &str,
    desc_png: &str,
    desc_all: &str,
) -> Option<PathBuf> {
    let filter = build_filter(&[(desc_png, "*.png"), (desc_all, "*.*")]);
    let title = to_wide(title);
    let default_ext = to_wide("png");
    let initial_dir_wide = initial_dir.map(|dir| to_wide(&dir.to_string_lossy()));

    // 完整路径写进 lpstrFile：部分系统会忽略 lpstrInitialDir
    let initial_path = match initial_dir.as_deref() {
        Some(dir) => dir.join(initial_name),
        None => PathBuf::from(initial_name),
    };

    let initial_text = initial_path.to_string_lossy().into_owned();
    let mut buffer = vec![0u16; BUFFER_LEN];
    for (slot, unit) in buffer
        .iter_mut()
        .zip(initial_text.encode_utf16().take(BUFFER_LEN - 1))
    {
        *slot = unit;
    }

    let mut dialog: OPENFILENAMEW = unsafe { zeroed() };
    dialog.lStructSize = size_of::<OPENFILENAMEW>() as u32;
    dialog.hwndOwner = parent.unwrap_or(ptr::null_mut());
    dialog.lpstrFilter = filter.as_ptr();
    dialog.nFilterIndex = 1;
    dialog.lpstrFile = buffer.as_mut_ptr();
    dialog.nMaxFile = BUFFER_LEN as u32;
    dialog.lpstrTitle = title.as_ptr();
    dialog.lpstrDefExt = default_ext.as_ptr();
    dialog.lpstrInitialDir = initial_dir_wide
        .as_ref()
        .map(|dir| dir.as_ptr())
        .unwrap_or(ptr::null());
    dialog.Flags = OFN_EXPLORER | OFN_PATHMUSTEXIST | OFN_OVERWRITEPROMPT | OFN_NOCHANGEDIR;

    if unsafe { GetSaveFileNameW(&mut dialog) } == 0 {
        return None;
    }

    Some(buffer_to_path(&buffer))
}

/// 构造 Win32 过滤串：`描述 (*.ext)\0*.ext\0...\0\0`。
fn build_filter(pairs: &[(&str, &str)]) -> Vec<u16> {
    let mut filter = Vec::new();

    for (desc, pattern) in pairs {
        let label = format!("{desc} ({pattern})");
        filter.extend(label.encode_utf16());
        filter.push(0);
        filter.extend(pattern.encode_utf16());
        filter.push(0);
    }

    filter.push(0);
    filter
}

fn buffer_to_path(buffer: &[u16]) -> PathBuf {
    let end = buffer.iter().position(|&unit| unit == 0).unwrap_or(0);
    PathBuf::from(String::from_utf16_lossy(&buffer[..end]))
}
