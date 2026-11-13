//! 基于 GDI 的离屏绘图画布。
//!
//! 用一个 32 位自上而下的 DIB 作为绘制目标，先画到内存里，再取回像素编码 PNG。
//! 这样文字由系统字体渲染，中文与字体外观与原版（Pillow 绘制）基本一致。

use std::mem::{size_of, zeroed};
use std::ptr;

use winapi::shared::minwindef::DWORD;
use winapi::shared::windef::{HDC, HFONT, HGDIOBJ, SIZE};
use winapi::um::wingdi::{
    CreateCompatibleDC, CreateDIBSection, CreateFontW, CreatePen, CreateSolidBrush, DeleteDC,
    DeleteObject, GetTextExtentPoint32W, GetTextMetricsW, Polygon, Rectangle, SelectObject,
    SetBkMode, SetTextColor, TextOutW, ANTIALIASED_QUALITY, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, DEFAULT_PITCH, DIB_RGB_COLORS, FF_DONTCARE, FW_NORMAL,
    OUT_TT_PRECIS, PS_SOLID, RGBQUAD, TEXTMETRICW, TRANSPARENT,
};
use winapi::um::winuser::FillRect;

/// 由 RGB 分量合成 Win32 的 COLORREF。
pub const fn rgb(red: u8, green: u8, blue: u8) -> u32 {
    (red as u32) | ((green as u32) << 8) | ((blue as u32) << 16)
}

/// 离屏画布。像素格式为 BGRA（Win32 原生顺序）。
pub struct Canvas {
    dc: HDC,
    bitmap: winapi::shared::windef::HBITMAP,
    previous: HGDIOBJ,
    bits: *mut u32,
    width: i32,
    height: i32,
}

impl Canvas {
    pub fn new(width: i32, height: i32) -> Option<Canvas> {
        if width <= 0 || height <= 0 {
            return None;
        }

        unsafe {
            let dc = CreateCompatibleDC(ptr::null_mut());
            if dc.is_null() {
                return None;
            }

            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as DWORD,
                    biWidth: width,
                    biHeight: -height, // 负高度表示自上而下
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB,
                    biSizeImage: (width * height * 4) as DWORD,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [RGBQUAD {
                    rgbBlue: 0,
                    rgbGreen: 0,
                    rgbRed: 0,
                    rgbReserved: 0,
                }],
            };

            let mut bits: *mut winapi::ctypes::c_void = ptr::null_mut();
            let bitmap = CreateDIBSection(
                dc,
                &info,
                DIB_RGB_COLORS,
                &mut bits as *mut *mut winapi::ctypes::c_void,
                ptr::null_mut(),
                0,
            );

            if bitmap.is_null() || bits.is_null() {
                DeleteDC(dc);
                return None;
            }

            let previous = SelectObject(dc, bitmap as HGDIOBJ);

            Some(Canvas {
                dc,
                bitmap,
                previous,
                bits: bits as *mut u32,
                width,
                height,
            })
        }
    }

    /// 用纯色填充矩形。
    pub fn fill(&self, left: i32, top: i32, right: i32, bottom: i32, color: u32) {
        unsafe {
            let brush = CreateSolidBrush(color);
            let mut rect = winapi::shared::windef::RECT {
                left,
                top,
                right,
                bottom,
            };
            FillRect(self.dc, &mut rect, brush);
            DeleteObject(brush as HGDIOBJ);
        }
    }

    /// 画一条横线（以矩形填充实现，可指定粗细）。
    pub fn hline(&self, left: i32, right: i32, top: i32, color: u32, thickness: i32) {
        self.fill(left, top, right + 1, top + thickness.max(1), color);
    }

    /// 画填充 + 描边的矩形，边界包含右下角（与 Pillow 的语义一致）。
    pub fn rect(&self, left: i32, top: i32, right: i32, bottom: i32, fill: u32, outline: u32) {
        unsafe {
            let brush = CreateSolidBrush(fill);
            let pen = CreatePen(PS_SOLID as i32, 1, outline);
            let old_brush = SelectObject(self.dc, brush as HGDIOBJ);
            let old_pen = SelectObject(self.dc, pen as HGDIOBJ);

            Rectangle(self.dc, left, top, right + 1, bottom + 1);

            SelectObject(self.dc, old_pen);
            SelectObject(self.dc, old_brush);
            DeleteObject(pen as HGDIOBJ);
            DeleteObject(brush as HGDIOBJ);
        }
    }

    /// 只画描边矩形，线宽由 `thickness` 控制。
    pub fn stroke_rect(
        &self,
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
        color: u32,
        thickness: i32,
    ) {
        let t = thickness.max(1);
        self.fill(left, top, right + 1, top + t, color);
        self.fill(left, bottom - t + 1, right + 1, bottom + 1, color);
        self.fill(left, top, left + t, bottom + 1, color);
        self.fill(right - t + 1, top, right + 1, bottom + 1, color);
    }

    /// 画多边形（填充 + 描边）。
    pub fn polygon(&self, points: &[(i32, i32)], fill: u32, outline: u32) {
        if points.len() < 3 {
            return;
        }

        let mut win_points: Vec<winapi::shared::windef::POINT> = points
            .iter()
            .map(|&(x, y)| winapi::shared::windef::POINT { x, y })
            .collect();

        unsafe {
            let brush = CreateSolidBrush(fill);
            let pen = CreatePen(PS_SOLID as i32, 1, outline);
            let old_brush = SelectObject(self.dc, brush as HGDIOBJ);
            let old_pen = SelectObject(self.dc, pen as HGDIOBJ);

            Polygon(self.dc, win_points.as_mut_ptr(), win_points.len() as i32);

            SelectObject(self.dc, old_pen);
            SelectObject(self.dc, old_brush);
            DeleteObject(pen as HGDIOBJ);
            DeleteObject(brush as HGDIOBJ);
        }
    }

    /// 绘制一行文字，坐标是文字单元格左上角。
    pub fn text(&self, x: i32, y: i32, text: &str, font: &Font, color: u32) {
        let wide: Vec<u16> = text.encode_utf16().collect();
        if wide.is_empty() {
            return;
        }

        unsafe {
            let previous = SelectObject(self.dc, font.handle as HGDIOBJ);
            SetBkMode(self.dc, TRANSPARENT as i32);
            SetTextColor(self.dc, color);
            TextOutW(self.dc, x, y, wide.as_ptr(), wide.len() as i32);
            SelectObject(self.dc, previous);
        }
    }

    /// 文字像素宽度。
    pub fn text_width(&self, text: &str, font: &Font) -> i32 {
        let wide: Vec<u16> = text.encode_utf16().collect();
        if wide.is_empty() {
            return 0;
        }

        unsafe {
            let previous = SelectObject(self.dc, font.handle as HGDIOBJ);
            let mut size = SIZE { cx: 0, cy: 0 };
            GetTextExtentPoint32W(self.dc, wide.as_ptr(), wide.len() as i32, &mut size);
            SelectObject(self.dc, previous);
            size.cx
        }
    }

    /// 文字单元格高度（上伸 + 下伸）。
    pub fn text_height(&self, font: &Font) -> i32 {
        unsafe {
            let previous = SelectObject(self.dc, font.handle as HGDIOBJ);
            let mut metrics: TEXTMETRICW = zeroed();
            GetTextMetricsW(self.dc, &mut metrics);
            SelectObject(self.dc, previous);
            metrics.tmHeight
        }
    }

    /// 导出为 RGBA 像素（BGRA -> RGBA，Alpha 固定 255）。
    pub fn to_rgba(&self) -> Vec<u8> {
        let count = (self.width as usize) * (self.height as usize);
        let mut output = Vec::with_capacity(count * 4);
        let pixels = unsafe { std::slice::from_raw_parts(self.bits as *const u32, count) };

        for &pixel in pixels {
            let blue = (pixel & 0xFF) as u8;
            let green = ((pixel >> 8) & 0xFF) as u8;
            let red = ((pixel >> 16) & 0xFF) as u8;
            output.extend_from_slice(&[red, green, blue, 255]);
        }

        output
    }
}

impl Drop for Canvas {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.previous);
            DeleteObject(self.bitmap as HGDIOBJ);
            DeleteDC(self.dc);
        }
    }
}

/// GDI 字体句柄包装。
pub struct Font {
    handle: HFONT,
}

impl Font {
    /// 按像素高度创建字体。
    pub fn new(pixel_height: i32, family: &str) -> Font {
        Font::with_underline(pixel_height, family, false)
    }

    pub fn with_underline(pixel_height: i32, family: &str, underline: bool) -> Font {
        let name = crate::platform::shell::to_wide(family);

        let handle = unsafe {
            CreateFontW(
                -pixel_height.max(1),
                0,
                0,
                0,
                FW_NORMAL,
                0,
                if underline { 1 } else { 0 },
                0,
                DEFAULT_CHARSET as DWORD,
                OUT_TT_PRECIS as DWORD,
                CLIP_DEFAULT_PRECIS as DWORD,
                ANTIALIASED_QUALITY as DWORD,
                (DEFAULT_PITCH | FF_DONTCARE) as DWORD,
                name.as_ptr(),
            )
        };

        Font { handle }
    }

    pub fn is_valid(&self) -> bool {
        !self.handle.is_null()
    }
}

impl Drop for Font {
    fn drop(&mut self) {
        unsafe {
            DeleteObject(self.handle as HGDIOBJ);
        }
    }
}
