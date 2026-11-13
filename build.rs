use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=build.rs");

    if std::env::var_os("CARGO_CFG_WINDOWS").is_none() {
        return;
    }

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default());

    let icon_path = manifest_dir.join("assets").join("icon.ico");
    let icon_bytes = match std::fs::read(&icon_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            println!("cargo:warning=无法读取图标 {}：{error}", icon_path.display());
            return;
        }
    };

    let images = match parse_ico(&icon_bytes) {
        Some(images) if !images.is_empty() => images,
        _ => {
            println!("cargo:warning=图标文件格式无法解析，跳过图标资源");
            return;
        }
    };

    let resource = build_resource(&images);
    let res_path = out_dir.join("app.res");
    if let Err(error) = std::fs::write(&res_path, &resource) {
        println!("cargo:warning=无法写入资源文件：{error}");
        return;
    }

    println!("cargo:rustc-link-arg={}", res_path.display());
}

/// `.ico` 中的一张图片。
struct IconImage {
    width: u8,
    height: u8,
    color_count: u8,
    planes: u16,
    bit_count: u16,
    data: Vec<u8>,
}

/// 解析 `.ico` 文件头与各图像数据。
fn parse_ico(bytes: &[u8]) -> Option<Vec<IconImage>> {
    if bytes.len() < 6 {
        return None;
    }

    let reserved = u16::from_le_bytes([bytes[0], bytes[1]]);
    let kind = u16::from_le_bytes([bytes[2], bytes[3]]);
    let count = u16::from_le_bytes([bytes[4], bytes[5]]) as usize;
    if reserved != 0 || kind != 1 || count == 0 {
        return None;
    }

    let mut images = Vec::with_capacity(count);
    for index in 0..count {
        let entry = 6 + index * 16;
        if entry + 16 > bytes.len() {
            return None;
        }

        let width = bytes[entry];
        let height = bytes[entry + 1];
        let color_count = bytes[entry + 2];
        let planes = u16::from_le_bytes([bytes[entry + 4], bytes[entry + 5]]);
        let bit_count = u16::from_le_bytes([bytes[entry + 6], bytes[entry + 7]]);
        let size = u32::from_le_bytes([
            bytes[entry + 8],
            bytes[entry + 9],
            bytes[entry + 10],
            bytes[entry + 11],
        ]) as usize;
        let offset = u32::from_le_bytes([
            bytes[entry + 12],
            bytes[entry + 13],
            bytes[entry + 14],
            bytes[entry + 15],
        ]) as usize;

        if offset + size > bytes.len() {
            return None;
        }

        images.push(IconImage {
            width,
            height,
            color_count,
            planes,
            bit_count,
            data: bytes[offset..offset + size].to_vec(),
        });
    }

    Some(images)
}

/// 拼装整个 `.res` 文件。
fn build_resource(images: &[IconImage]) -> Vec<u8> {
    let mut out = Vec::new();

    // 空条目占位（资源文件的固定开头）
    out.extend_from_slice(&entry(0, 0, 0, 0, &[]));

    // RT_ICON：每张图片一条，资源名从 1 开始
    for (index, image) in images.iter().enumerate() {
        out.extend_from_slice(&entry(3, index as u16 + 1, 0x1010, 0x0409, &image.data));
    }

    // RT_GROUP_ICON：图标组
    out.extend_from_slice(&entry(14, 1, 0x1010, 0x0409, &group_icon(images)));

    out
}

/// 一条资源记录：`dataSize` + `headerSize`(32) + 类型/名称/属性 + 数据。
fn entry(kind: u16, name: u16, memory_flags: u16, language: u16, data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(32 + data.len() + 4);

    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&32u32.to_le_bytes()); // 头部总长（含这两个 DWORD）

    out.extend_from_slice(&0xFFFFu16.to_le_bytes());
    out.extend_from_slice(&kind.to_le_bytes());
    out.extend_from_slice(&0xFFFFu16.to_le_bytes());
    out.extend_from_slice(&name.to_le_bytes());

    out.extend_from_slice(&0u32.to_le_bytes()); // 数据版本
    out.extend_from_slice(&memory_flags.to_le_bytes());
    out.extend_from_slice(&language.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // 版本
    out.extend_from_slice(&0u32.to_le_bytes()); // 特性

    out.extend_from_slice(data);
    while out.len() % 4 != 0 {
        out.push(0);
    }

    out
}

/// 图标组数据：`GRPICONDIR` + 每个图像的 `GRPICONDIRENTRY`。
fn group_icon(images: &[IconImage]) -> Vec<u8> {
    let mut out = Vec::with_capacity(6 + images.len() * 14);

    out.extend_from_slice(&0u16.to_le_bytes()); // 保留
    out.extend_from_slice(&1u16.to_le_bytes()); // 类型：图标
    out.extend_from_slice(&(images.len() as u16).to_le_bytes());

    for (index, image) in images.iter().enumerate() {
        out.push(image.width);
        out.push(image.height);
        out.push(image.color_count);
        out.push(0); // 保留
        out.extend_from_slice(&image.planes.to_le_bytes());
        out.extend_from_slice(&image.bit_count.to_le_bytes());
        out.extend_from_slice(&(image.data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(index as u16 + 1).to_le_bytes());
    }

    out
}
