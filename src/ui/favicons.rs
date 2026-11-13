//! 收藏夹内嵌 favicon 的解码：data URI → base64 → PNG → 16×16 RGBA。

use std::io::Read;

/// 图标尺寸（像素），与 `icons::SIZE` 一致。
const SIZE: usize = 16;

/// 解码 `data:image/png;base64,...`，输出 16×16 RGBA；不支持的格式返回 `None`。
pub fn decode_data_uri(uri: &str) -> Option<Vec<u8>> {
    let (header, payload) = uri.split_once(',')?;
    if !header.contains("base64") {
        return None;
    }

    let bytes = decode_base64(payload.trim())?;
    let (width, height, rgba) = decode_png(&bytes)?;
    Some(scale_to_size(width, height, &rgba))
}

fn decode_base64(input: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let mut buffer: u32 = 0;
    let mut bits = 0;

    for byte in input.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            b'\r' | b'\n' | b' ' | b'\t' => continue,
            _ => return None,
        } as u32;

        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }

    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn decode_png(data: &[u8]) -> Option<(usize, usize, Vec<u8>)> {
    const SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
    if data.len() < 8 || data[..8] != SIGNATURE {
        return None;
    }

    let mut pos = 8;
    let mut width = 0usize;
    let mut height = 0usize;
    let mut depth = 0u8;
    let mut color_type = 0u8;
    let mut interlace = 0u8;
    let mut palette: Vec<[u8; 3]> = Vec::new();
    let mut palette_alpha: Vec<u8> = Vec::new();
    let mut idat: Vec<u8> = Vec::new();

    while pos + 8 <= data.len() {
        let length = u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        let kind = &data[pos + 4..pos + 8];
        let body_start = pos + 8;
        let body_end = body_start.checked_add(length)?;
        if body_end > data.len() {
            return None;
        }
        let body = &data[body_start..body_end];

        match kind {
            b"IHDR" => {
                if body.len() < 13 {
                    return None;
                }
                width = u32::from_be_bytes([body[0], body[1], body[2], body[3]]) as usize;
                height = u32::from_be_bytes([body[4], body[5], body[6], body[7]]) as usize;
                depth = body[8];
                color_type = body[9];
                interlace = body[12];
            }
            b"PLTE" => {
                palette = body.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();
            }
            b"tRNS" => {
                palette_alpha = body.to_vec();
            }
            b"IDAT" => idat.extend_from_slice(body),
            b"IEND" => break,
            _ => {}
        }

        pos = body_end + 4;
    }

    if width == 0 || height == 0 || depth != 8 || interlace != 0 {
        return None;
    }

    let channels = match color_type {
        0 => 1,
        2 => 3,
        3 => 1,
        4 => 2,
        6 => 4,
        _ => return None,
    };

    let mut raw = Vec::new();
    flate2::read::ZlibDecoder::new(&idat[..])
        .read_to_end(&mut raw)
        .ok()?;

    let stride = width * channels;
    if raw.len() < (stride + 1) * height {
        return None;
    }

    let mut pixels = vec![0u8; stride * height];
    for y in 0..height {
        let filter = raw[y * (stride + 1)];
        let line = &raw[y * (stride + 1) + 1..y * (stride + 1) + 1 + stride];
        let start = y * stride;
        let (before, current) = pixels.split_at_mut(start);
        let prev: Option<&[u8]> = if y == 0 { None } else { Some(&before[start - stride..]) };

        for x in 0..stride {
            let left = if x >= channels { current[x - channels] } else { 0 };
            let up = prev.map(|p| p[x]).unwrap_or(0);
            let up_left = if x >= channels { prev.map(|p| p[x - channels]).unwrap_or(0) } else { 0 };

            current[x] = match filter {
                0 => line[x],
                1 => line[x].wrapping_add(left),
                2 => line[x].wrapping_add(up),
                3 => line[x].wrapping_add(((left as u16 + up as u16) / 2) as u8),
                4 => line[x].wrapping_add(paeth(left, up, up_left)),
                _ => return None,
            };
        }
    }

    let mut rgba = Vec::with_capacity(width * height * 4);
    for i in 0..width * height {
        let px = &pixels[i * channels..(i + 1) * channels];
        let (r, g, b, a) = match color_type {
            0 => (px[0], px[0], px[0], 255),
            2 => (px[0], px[1], px[2], 255),
            3 => {
                let index = px[0] as usize;
                let rgb = *palette.get(index)?;
                let alpha = palette_alpha.get(index).copied().unwrap_or(255);
                (rgb[0], rgb[1], rgb[2], alpha)
            }
            4 => (px[0], px[0], px[0], px[1]),
            6 => (px[0], px[1], px[2], px[3]),
            _ => return None,
        };
        rgba.extend_from_slice(&[r, g, b, a]);
    }

    Some((width, height, rgba))
}

fn paeth(left: u8, up: u8, up_left: u8) -> u8 {
    let p = left as i16 + up as i16 - up_left as i16;
    let pa = (p - left as i16).abs();
    let pb = (p - up as i16).abs();
    let pc = (p - up_left as i16).abs();

    if pa <= pb && pa <= pc {
        left
    } else if pb <= pc {
        up
    } else {
        up_left
    }
}

/// 缩放或裁剪到 `SIZE × SIZE`，按预乘 alpha 求均值，避免透明边缘发黑。
fn scale_to_size(width: usize, height: usize, rgba: &[u8]) -> Vec<u8> {
    if width == SIZE && height == SIZE {
        return rgba.to_vec();
    }

    let mut out = vec![0u8; SIZE * SIZE * 4];
    for dy in 0..SIZE {
        let y0 = dy * height / SIZE;
        let y1 = ((dy + 1) * height / SIZE).max(y0 + 1).min(height);
        for dx in 0..SIZE {
            let x0 = dx * width / SIZE;
            let x1 = ((dx + 1) * width / SIZE).max(x0 + 1).min(width);

            let mut sum_a = 0u32;
            let mut sum_r = 0u32;
            let mut sum_g = 0u32;
            let mut sum_b = 0u32;
            let mut count = 0u32;

            for y in y0..y1 {
                for x in x0..x1 {
                    let i = (y * width + x) * 4;
                    let a = rgba[i + 3] as u32;
                    sum_a += a;
                    sum_r += rgba[i] as u32 * a;
                    sum_g += rgba[i + 1] as u32 * a;
                    sum_b += rgba[i + 2] as u32 * a;
                    count += 1;
                }
            }

            let o = (dy * SIZE + dx) * 4;
            if count == 0 || sum_a == 0 {
                continue;
            }
            out[o] = (sum_r / sum_a) as u8;
            out[o + 1] = (sum_g / sum_a) as u8;
            out[o + 2] = (sum_b / sum_a) as u8;
            out[o + 3] = (sum_a / count) as u8;
        }
    }

    out
}
