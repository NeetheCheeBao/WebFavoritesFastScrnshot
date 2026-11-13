//! 树控件使用的三个共享图标（星标 / 文件夹 / 链接页）。
//!
//! 图标在启动时一次性生成并加入 ImageList，绝不为每条书签单独创建，
//! 这是保证大树展开流畅的关键。

use native_windows_gui as nwg;

use crate::screenshot::png;

/// 图标尺寸（像素）。
const SIZE: usize = 16;

pub const ICON_FOLDER: i32 = 0;
pub const ICON_STAR: i32 = 1;
pub const ICON_LINK: i32 = 2;

/// 构建含三个图标的 ImageList。
pub fn build_image_list() -> Result<nwg::ImageList, nwg::NwgError> {
    let mut list = nwg::ImageList::default();
    nwg::ImageList::builder()
        .size((SIZE as i32, SIZE as i32))
        .initial(3)
        .grow(1)
        .build(&mut list)?;

    for pixels in [folder_icon(), star_icon(), link_icon()] {
        let encoded = png::encode_rgba(SIZE as u32, SIZE as u32, &pixels);
        if let Ok(bitmap) = nwg::Bitmap::from_bin(&encoded) {
            list.add_bitmap(&bitmap);
        }
    }

    Ok(list)
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
