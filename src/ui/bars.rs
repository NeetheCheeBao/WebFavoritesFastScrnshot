//! 路径行与底部两条栏自绘（原生 Label 无法满足文字色/下划线/手型光标）。

use std::mem::zeroed;

use winapi::shared::windef::{HDC, HFONT, HGDIOBJ, HWND, RECT};
use winapi::um::wingdi::{
    CreateFontW, CreateSolidBrush, DeleteObject, SelectObject, SetBkColor, SetBkMode, SetTextColor,
    CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, DEFAULT_PITCH, FF_DONTCARE, FW_NORMAL,
    OPAQUE, OUT_TT_PRECIS,
};
use winapi::um::winuser::{
    BeginPaint, DrawTextW, EndPaint, FillRect, GetClientRect, GetDC, ReleaseDC, DT_END_ELLIPSIS,
    DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, PAINTSTRUCT,
};

use crate::platform::shell::to_wide;
use crate::screenshot::gdi::rgb;

/// 每条栏的高度（96dpi 逻辑像素）。
const STRIP_HEIGHT: i32 = 24;

/// 按当前 DPI 换算后的栏高。
pub fn strip_height() -> i32 {
    crate::platform::dpi::scaled(STRIP_HEIGHT)
}

const TOOLBAR_BG: u32 = rgb(245, 245, 245);
const STATUS_BG: u32 = rgb(236, 236, 236);
const SEPARATOR: u32 = rgb(220, 220, 220);
const INFO_TEXT: u32 = rgb(51, 51, 51);
const STATUS_TEXT: u32 = rgb(85, 85, 85);
const PATH_TEXT: u32 = rgb(51, 51, 51);
const PATH_HINT: u32 = rgb(136, 136, 136);
const LINK_TEXT: u32 = rgb(5, 99, 193);

/// 文字右侧留白，避免贴边。
const TEXT_PADDING: i32 = 8;

/// 栏使用的字体：常规体 + 下划线体（用于链接悬停）。
pub struct BarFonts {
    normal: HFONT,
    underline: HFONT,
}

impl BarFonts {
    pub fn new(pixel_height: i32, family: &str) -> BarFonts {
        BarFonts {
            normal: create_font(pixel_height, family, false),
            underline: create_font(pixel_height, family, true),
        }
    }
}

impl Drop for BarFonts {
    fn drop(&mut self) {
        unsafe {
            DeleteObject(self.normal as HGDIOBJ);
            DeleteObject(self.underline as HGDIOBJ);
        }
    }
}

/// 需要自绘的区域与内容。
pub struct BarContent<'a> {
    pub info: &'a str,
    pub status: &'a str,
    pub status_is_credit: bool,
    pub hover_credit: bool,
    pub path: &'a str,
    /// 未打开文件时路径行使用浅灰提示色
    pub path_dim: bool,
    /// 鼠标悬停在路径行上（可点击定位文件）
    pub path_hover: bool,
    /// 路径行矩形（客户区坐标）
    pub path_rect: RECT,
}

/// 擦除背景（同时把栏内容画上，避免只收到擦除消息时留下空白）。
pub fn erase_background(hwnd: HWND, content: &BarContent, fonts: &BarFonts) {
    unsafe {
        let dc = GetDC(hwnd);
        if dc.is_null() {
            return;
        }
        render(dc, hwnd, content, fonts);
        ReleaseDC(hwnd, dc);
    }
}

pub fn paint(hwnd: HWND, content: &BarContent, fonts: &BarFonts) {
    unsafe {
        let mut paint_struct: PAINTSTRUCT = zeroed();
        let dc = BeginPaint(hwnd, &mut paint_struct);
        if dc.is_null() {
            return;
        }
        render(dc, hwnd, content, fonts);
        EndPaint(hwnd, &mut paint_struct);
    }
}

pub fn is_in_status_bar(hwnd: HWND, y: i32) -> bool {
    unsafe {
        let mut client: RECT = zeroed();
        GetClientRect(hwnd, &mut client);
        y >= (client.bottom - client.top) - strip_height()
    }
}

fn render(dc: HDC, hwnd: HWND, content: &BarContent, fonts: &BarFonts) {
    unsafe {
        let mut client: RECT = zeroed();
        GetClientRect(hwnd, &mut client);
        let width = client.right - client.left;
        let height = client.bottom - client.top;
        if width <= 0 || height <= 0 {
            return;
        }

        let right = width - crate::platform::dpi::scaled(TEXT_PADDING);
        let strip = strip_height();

        // 整块底色（与工具栏同色）
        fill_rect(
            dc,
            &RECT {
                left: 0,
                top: 0,
                right: width,
                bottom: height,
            },
            TOOLBAR_BG,
        );

        // 路径行（悬停时按链接样式显示，提示可点击）
        let (path_font, path_color) = if content.path_hover {
            (fonts.underline, LINK_TEXT)
        } else if content.path_dim {
            (fonts.normal, PATH_HINT)
        } else {
            (fonts.normal, PATH_TEXT)
        };
        draw_text(
            dc,
            8,
            content.path_rect.top,
            right,
            content.path_rect.bottom - content.path_rect.top,
            content.path,
            path_font,
            path_color,
            TOOLBAR_BG,
        );

        // 信息栏
        let info_top = height - strip * 2;
        draw_text(
            dc,
            8,
            info_top,
            right,
            strip,
            content.info,
            fonts.normal,
            INFO_TEXT,
            TOOLBAR_BG,
        );

        // 状态栏
        let status_top = height - strip;
        fill_rect(
            dc,
            &RECT {
                left: 0,
                top: status_top,
                right: width,
                bottom: height,
            },
            STATUS_BG,
        );

        let (font, color) = if content.status_is_credit && content.hover_credit {
            (fonts.underline, LINK_TEXT)
        } else {
            (fonts.normal, STATUS_TEXT)
        };
        draw_text(
            dc,
            8,
            status_top,
            right,
            strip,
            content.status,
            font,
            color,
            STATUS_BG,
        );

        // 分隔线最后画：文字使用不透明底色，先画会被覆盖
        fill_rect(
            dc,
            &RECT {
                left: 0,
                top: info_top,
                right: width,
                bottom: info_top + 1,
            },
            SEPARATOR,
        );
        fill_rect(
            dc,
            &RECT {
                left: 0,
                top: status_top,
                right: width,
                bottom: status_top + 1,
            },
            SEPARATOR,
        );
    }
}

fn create_font(pixel_height: i32, family: &str, underline: bool) -> HFONT {
    let name = to_wide(family);
    unsafe {
        CreateFontW(
            -pixel_height.max(1),
            0,
            0,
            0,
            FW_NORMAL,
            0,
            if underline { 1 } else { 0 },
            0,
            DEFAULT_CHARSET as u32,
            OUT_TT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            CLEARTYPE_QUALITY as u32,
            (DEFAULT_PITCH | FF_DONTCARE) as u32,
            name.as_ptr(),
        )
    }
}

unsafe fn fill_rect(dc: HDC, rect: &RECT, color: u32) {
    let brush = CreateSolidBrush(color);
    let mut rect = *rect;
    FillRect(dc, &mut rect, brush);
    DeleteObject(brush as HGDIOBJ);
}

/// 单行绘制，超宽时自动加省略号（而不是硬裁切）。
#[allow(clippy::too_many_arguments)]
unsafe fn draw_text(
    dc: HDC,
    left: i32,
    top: i32,
    right: i32,
    height: i32,
    text: &str,
    font: HFONT,
    color: u32,
    background: u32,
) {
    if text.is_empty() || right <= left || height <= 0 {
        return;
    }

    let previous = SelectObject(dc, font as HGDIOBJ);
// ClearType 需要已知底色，否则小字发虚、出现彩边
    SetBkMode(dc, OPAQUE as i32);
    SetBkColor(dc, background);
    SetTextColor(dc, color);

    let mut rect = RECT {
        left,
        top,
        right,
        bottom: top + height,
    };
    let wide: Vec<u16> = text.encode_utf16().collect();

    DrawTextW(
        dc,
        wide.as_ptr(),
        wide.len() as i32,
        &mut rect,
        DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
    );

    SelectObject(dc, previous);
}
