//! 编码探测：UTF-8 → GBK(936) → GB18030(54936) → Latin-1 兜底。

use std::ptr;
use winapi::um::stringapiset::MultiByteToWideChar;
use winapi::um::winnls::MB_ERR_INVALID_CHARS;

const CP_GBK: u32 = 936;
const CP_GB18030: u32 = 54936;

/// 把收藏夹文件字节解码为字符串。
pub fn decode_bookmarks(bytes: &[u8]) -> String {
    let bytes = strip_bom(bytes);

    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.to_string();
    }
    if let Some(text) = decode_codepage(bytes, CP_GBK) {
        return text;
    }
    if let Some(text) = decode_codepage(bytes, CP_GB18030) {
        return text;
    }

    // 最后的兜底：按字节映射到 Latin-1，保证不因个别脏字节整份失败
    bytes.iter().map(|&b| b as char).collect()
}

fn strip_bom(bytes: &[u8]) -> &[u8] {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        &bytes[3..]
    } else {
        bytes
    }
}

/// 用系统代码页解码；遇到非法字节返回 None。
fn decode_codepage(bytes: &[u8], codepage: u32) -> Option<String> {
    if bytes.is_empty() {
        return Some(String::new());
    }

    let length = i32::try_from(bytes.len()).ok()?;

    let needed = unsafe {
        MultiByteToWideChar(
            codepage,
            MB_ERR_INVALID_CHARS,
            bytes.as_ptr() as *const i8,
            length,
            ptr::null_mut(),
            0,
        )
    };
    if needed <= 0 {
        return None;
    }

    let mut buffer = vec![0u16; needed as usize];
    let written = unsafe {
        MultiByteToWideChar(
            codepage,
            MB_ERR_INVALID_CHARS,
            bytes.as_ptr() as *const i8,
            length,
            buffer.as_mut_ptr(),
            needed,
        )
    };
    if written <= 0 {
        return None;
    }

    String::from_utf16(&buffer[..written as usize]).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_utf8_with_and_without_bom() {
        let text = "收藏夹栏";
        assert_eq!(decode_bookmarks(text.as_bytes()), text);

        let mut with_bom = vec![0xEF, 0xBB, 0xBF];
        with_bom.extend_from_slice(text.as_bytes());
        assert_eq!(decode_bookmarks(&with_bom), text);
    }
}
