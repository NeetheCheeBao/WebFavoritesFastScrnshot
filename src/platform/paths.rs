//! 常用系统文件夹定位。

use std::path::PathBuf;
use std::ptr;

use winapi::um::combaseapi::CoTaskMemFree;
use winapi::um::knownfolders::FOLDERID_Downloads;
use winapi::um::shlobj::SHGetKnownFolderPath;

/// 当前用户的「下载」目录；取不到时返回 `None`。
///
/// 使用 `SHGetKnownFolderPath` 而不是拼 `%USERPROFILE%\Downloads`，
/// 这样用户把「下载」重定向到其它盘符时也能正确定位。
pub fn downloads_dir() -> Option<PathBuf> {
    unsafe {
        let mut raw: *mut u16 = ptr::null_mut();
        let result = SHGetKnownFolderPath(&FOLDERID_Downloads, 0, ptr::null_mut(), &mut raw);
        if result < 0 || raw.is_null() {
            return None;
        }

        let mut length = 0usize;
        while *raw.add(length) != 0 {
            length += 1;
        }
        let text = String::from_utf16_lossy(std::slice::from_raw_parts(raw, length));
        CoTaskMemFree(raw as *mut _);

        let path = PathBuf::from(text);
        if path.is_dir() {
            Some(path)
        } else {
            None
        }
    }
}
