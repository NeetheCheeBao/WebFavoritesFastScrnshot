//! 最小 PNG 编码器：8 位 RGBA、无交错、单 IDAT。
//!
//! 只服务于"把内存里的像素写成 .png"，因此不追求压缩率与通用性。

use std::io::Write;

use crc32fast::Hasher;
use flate2::write::ZlibEncoder;
use flate2::Compression;

const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// 把 RGBA 像素编码成 PNG 字节流。
pub fn encode_rgba(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let stride = width as usize * 4;
    let mut output = Vec::with_capacity(stride * height as usize / 2 + 128);
    output.extend_from_slice(&SIGNATURE);

    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    header.push(8); // 位深
    header.push(6); // 颜色类型：真彩 + Alpha
    header.push(0); // 压缩方式：deflate
    header.push(0); // 过滤方式：标准
    header.push(0); // 交错：无
    write_chunk(&mut output, b"IHDR", &header);

    let mut raw = Vec::with_capacity((stride + 1) * height as usize);
    for row in 0..height as usize {
        raw.push(0); // 每行使用过滤器 0（None）
        let start = row * stride;
        raw.extend_from_slice(&rgba[start..start + stride]);
    }

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    if encoder.write_all(&raw).is_ok() {
        if let Ok(compressed) = encoder.finish() {
            write_chunk(&mut output, b"IDAT", &compressed);
        }
    }

    write_chunk(&mut output, b"IEND", &[]);
    output
}

fn write_chunk(output: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    output.extend_from_slice(&(data.len() as u32).to_be_bytes());
    output.extend_from_slice(kind);
    output.extend_from_slice(data);

    let mut hasher = Hasher::new();
    hasher.update(kind);
    hasher.update(data);
    output.extend_from_slice(&hasher.finalize().to_be_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_valid_signature_and_chunks() {
        let png = encode_rgba(2, 2, &[255; 16]);
        assert_eq!(&png[..8], &SIGNATURE);
        assert_eq!(&png[12..16], b"IHDR");
        assert_eq!(&png[png.len() - 8..png.len() - 4], b"IEND");
    }
}
