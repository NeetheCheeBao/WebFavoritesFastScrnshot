//! 树控件的三个共享图标，启动时一次性生成并加入 ImageList。

use std::ptr;

use native_windows_gui as nwg;
use winapi::shared::windef::{HBITMAP, HGDIOBJ};
use winapi::um::commctrl::ImageList_ReplaceIcon;
use winapi::um::wingdi::{
    CreateBitmap, CreateDIBSection, DeleteObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    DIB_RGB_COLORS, RGBQUAD,
};
use winapi::um::winuser::{CreateIconIndirect, DestroyIcon, ICONINFO};

/// 图标尺寸（像素）。
const SIZE: usize = 16;

pub const ICON_FOLDER: i32 = 0;
pub const ICON_STAR: i32 = 1;
pub const ICON_LINK: i32 = 2;
/// 加载图标后，无 favicon 的站点使用的地球图标
pub const ICON_GLOBE: i32 = 3;
/// favicon 的起始索引（前四个是共享图标）。
pub const FAVICON_BASE: i32 = 4;

/// 构建含三个图标的 ImageList。
pub fn build_image_list() -> Result<nwg::ImageList, nwg::NwgError> {
    build_image_list_with_favicons(&[])
}

/// 三个基础图标 + favicon；favicon 从索引 3 开始，顺序与入参一致。
pub fn build_image_list_with_favicons(favicons: &[Vec<u8>]) -> Result<nwg::ImageList, nwg::NwgError> {
    let mut list = nwg::ImageList::default();
    nwg::ImageList::builder()
        .size((SIZE as i32, SIZE as i32))
        .initial((4 + favicons.len()) as i32)
        .grow(1)
        .build(&mut list)?;

    let link = link_icon();
    for pixels in [folder_icon(), star_icon(), link.clone(), globe_icon()] {
        add_pixels(&list, &pixels, &link);
    }
    for pixels in favicons {
        add_pixels(&list, pixels, &link);
    }

    Ok(list)
}

/// 加入一张 16×16 RGBA；失败时用链接图标兜底，保证索引不偏移。
fn add_pixels(list: &nwg::ImageList, pixels: &[u8], fallback: &[u8]) {
    for candidate in [pixels, fallback] {
        if add_rgba(list, candidate) {
            return;
        }
    }
}

/// 通过 HICON 加入 ImageList：`ImageList_ReplaceIcon` 会保留 alpha，
/// 而 nwg 的 `add_bitmap` 走 `ImageList_AddMasked`，会把纯黑当透明色抠掉。
fn add_rgba(list: &nwg::ImageList, rgba: &[u8]) -> bool {
    let Some(icon) = build_icon(rgba) else {
        return false;
    };

    unsafe {
        let index = ImageList_ReplaceIcon(list.handle, -1, icon);
        DestroyIcon(icon);
        index >= 0
    }
}

fn build_icon(rgba: &[u8]) -> Option<winapi::shared::windef::HICON> {
    if rgba.len() < SIZE * SIZE * 4 {
        return None;
    }

    unsafe {
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: SIZE as i32,
                biHeight: -(SIZE as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                biSizeImage: (SIZE * SIZE * 4) as u32,
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
        let color: HBITMAP = CreateDIBSection(
            ptr::null_mut(),
            &info,
            DIB_RGB_COLORS,
            &mut bits as *mut *mut winapi::ctypes::c_void,
            ptr::null_mut(),
            0,
        );
        if color.is_null() || bits.is_null() {
            return None;
        }

        let dst = std::slice::from_raw_parts_mut(bits as *mut u8, SIZE * SIZE * 4);
        let mut mask_bits = [0u8; SIZE * 2];
        for index in 0..SIZE * SIZE {
            let alpha = rgba[index * 4 + 3];
            dst[index * 4] = rgba[index * 4 + 2];
            dst[index * 4 + 1] = rgba[index * 4 + 1];
            dst[index * 4 + 2] = rgba[index * 4];
            dst[index * 4 + 3] = alpha;

            if alpha == 0 {
                let (x, y) = (index % SIZE, index / SIZE);
                mask_bits[y * 2 + x / 8] |= 0x80 >> (x % 8);
            }
        }

        let mask = CreateBitmap(
            SIZE as i32,
            SIZE as i32,
            1,
            1,
            mask_bits.as_ptr() as *const winapi::ctypes::c_void,
        );

        let mut icon_info: ICONINFO = std::mem::zeroed();
        icon_info.fIcon = 1;
        icon_info.hbmColor = color;
        icon_info.hbmMask = mask;

        let icon = CreateIconIndirect(&mut icon_info);

        DeleteObject(color as HGDIOBJ);
        DeleteObject(mask as HGDIOBJ);

        if icon.is_null() {
            None
        } else {
            Some(icon)
        }
    }
}

/// 极简 RGBA 画布，用于生成 16×16 图标。
struct IconCanvas {
    pixels: Vec<u8>,
}

impl IconCanvas {
    fn new() -> IconCanvas {
        IconCanvas {
            pixels: vec![0; SIZE * SIZE * 4],
        }
    }

    fn set(&mut self, x: usize, y: usize, color: (u8, u8, u8)) {
        if x >= SIZE || y >= SIZE {
            return;
        }
        let offset = (y * SIZE + x) * 4;
        self.pixels[offset] = color.0;
        self.pixels[offset + 1] = color.1;
        self.pixels[offset + 2] = color.2;
        self.pixels[offset + 3] = 255;
    }

    fn fill_rect(&mut self, x0: usize, y0: usize, x1: usize, y1: usize, color: (u8, u8, u8)) {
        for y in y0..y1 {
            for x in x0..x1 {
                self.set(x, y, color);
            }
        }
    }

    fn into_pixels(self) -> Vec<u8> {
        self.pixels
    }
}

fn folder_icon() -> Vec<u8> {
    let mut canvas = IconCanvas::new();
    let body = (255, 204, 51);
    let tab = (230, 184, 0);
    let edge = (201, 160, 0);

    canvas.fill_rect(2, 3, 8, 6, tab);
    canvas.fill_rect(2, 6, 14, 13, body);
    for x in 2..14 {
        canvas.set(x, 6, edge);
        canvas.set(x, 12, edge);
    }
    for y in 6..13 {
        canvas.set(2, y, edge);
        canvas.set(13, y, edge);
    }

    canvas.into_pixels()
}

fn star_icon() -> Vec<u8> {
    let mut canvas = IconCanvas::new();
    let body = (245, 197, 24);

    const POINTS: &[(usize, usize)] = &[
        (8, 2),
        (8, 3),
        (7, 3),
        (9, 3),
        (7, 4),
        (8, 4),
        (9, 4),
        (6, 5),
        (7, 5),
        (8, 5),
        (9, 5),
        (10, 5),
        (3, 6),
        (4, 6),
        (5, 6),
        (6, 6),
        (7, 6),
        (8, 6),
        (9, 6),
        (10, 6),
        (11, 6),
        (12, 6),
        (13, 6),
        (5, 7),
        (6, 7),
        (7, 7),
        (8, 7),
        (9, 7),
        (10, 7),
        (11, 7),
        (6, 8),
        (7, 8),
        (8, 8),
        (9, 8),
        (10, 8),
        (6, 9),
        (7, 9),
        (8, 9),
        (9, 9),
        (5, 10),
        (6, 10),
        (7, 10),
        (8, 10),
        (9, 10),
        (10, 10),
        (5, 11),
        (6, 11),
        (7, 11),
        (9, 11),
        (10, 11),
        (4, 12),
        (5, 12),
        (10, 12),
        (11, 12),
    ];

    for &(x, y) in POINTS {
        canvas.set(x, y, body);
    }

    canvas.into_pixels()
}

pub fn globe_icon() -> Vec<u8> {
    let mut canvas = IconCanvas::new();
    let center = 7.5f32;
    let radius = 6.2f32;
    let ink = (90, 90, 90);

    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f32 + 0.5 - center;
            let dy = y as f32 + 0.5 - center;
            let distance = (dx * dx + dy * dy).sqrt();
            if distance > radius + 0.6 {
                continue;
            }

            let ring = (distance - radius).abs() < 0.9;
            let equator = dy.abs() < 0.6;
            let meridian = (dx.abs() - radius * 0.55 * (1.0 - (dy / radius).powi(2)).max(0.0).sqrt()).abs() < 0.6;

            if ring || equator || meridian {
                canvas.set(x, y, ink);
            }
        }
    }

    canvas.into_pixels()
}

fn link_icon() -> Vec<u8> {
    let mut canvas = IconCanvas::new();

    canvas.fill_rect(4, 2, 12, 14, (240, 240, 240));
    for x in 4..12 {
        canvas.set(x, 2, (107, 107, 107));
        canvas.set(x, 13, (107, 107, 107));
    }
    for y in 2..14 {
        canvas.set(4, y, (107, 107, 107));
        canvas.set(11, y, (107, 107, 107));
    }
    for y in [6, 8, 10] {
        for x in 6..10 {
            canvas.set(x, y, (136, 136, 136));
        }
    }

    canvas.into_pixels()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icons_have_expected_size_and_alpha() {
        let pixels = folder_icon();
        assert_eq!(pixels.len(), SIZE * SIZE * 4);
        // 左上角保持透明，图形区域不透明
        assert_eq!(pixels[3], 0);
        let center = (6 * SIZE + 8) * 4;
        assert_eq!(pixels[center + 3], 255);
    }
}
