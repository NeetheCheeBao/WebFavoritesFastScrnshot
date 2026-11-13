//! 构建脚本：把图标与版本信息写成 Windows 资源文件（`.res`）并链接进 exe。
//!
//! 全部用纯 Rust 生成，**不依赖 `windres` / `rc.exe`**：
//! `.res` 只是「资源条目顺序拼接」的二进制容器，图标数据直接取自 `.ico`，
//! 版本信息按 `VS_VERSIONINFO` 结构拼装。

use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=version.ini");
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=build.rs");

    if std::env::var_os("CARGO_CFG_WINDOWS").is_none() {
        return;
    }

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default());

    let (name, version) = read_metadata(&manifest_dir.join("version.ini"));

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

    let resource = build_resource(&images, &name, &version);
    let res_path = out_dir.join("app.res");
    if let Err(error) = std::fs::write(&res_path, &resource) {
        println!("cargo:warning=无法写入资源文件：{error}");
        return;
    }

    println!("cargo:rustc-link-arg={}", res_path.display());
}

/// 从 `version.ini` 读取程序名与版本号。
fn read_metadata(path: &Path) -> (String, String) {
    let mut name = String::from("WebFavoritesFastScrnshot");
    let mut version = String::from("1.0.0");

    if let Ok(text) = std::fs::read_to_string(path) {
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim().trim_matches('"').to_string();
            if value.is_empty() {
                continue;
            }
            match key.trim().to_ascii_lowercase().as_str() {
                "name" => name = value,
                "version" => version = value,
                _ => {}
            }
        }
    }

    (name, version)
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
fn build_resource(images: &[IconImage], name: &str, version: &str) -> Vec<u8> {
    let mut out = Vec::new();

    // 与 windres 的输出保持一致：先写一个空条目占位
    out.extend_from_slice(&entry(0, 0, 0, 0, &[]));

    // RT_ICON：每张图片一条，资源名从 1 开始
    for (index, image) in images.iter().enumerate() {
        out.extend_from_slice(&entry(3, index as u16 + 1, 0x1010, 0x0409, &image.data));
    }

    // RT_GROUP_ICON：图标组
    out.extend_from_slice(&entry(14, 1, 0x1010, 0x0409, &group_icon(images)));

    // RT_VERSION：版本信息
    out.extend_from_slice(&entry(16, 1, 0x0000, 0x0409, &version_info(name, version)));

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

/// 版本信息：`VS_VERSIONINFO` 结构。
fn version_info(name: &str, version: &str) -> Vec<u8> {
    let (major, minor, patch, build) = version_parts(version);

    let mut fixed = Vec::with_capacity(52);
    for value in [
        0xFEEF_04BDu32,        // 签名
        0x0001_0000,           // 结构版本
        (major << 16) | minor, // 文件版本 MS
        (patch << 16) | build, // 文件版本 LS
        (major << 16) | minor, // 产品版本 MS
        (patch << 16) | build, // 产品版本 LS
        0,                     // 标志掩码
        0,                     // 标志
        0x0004_0004,           // 操作系统：NT / Windows 32
        0x0000_0001,           // 文件类型：应用
        0,                     // 子类型
        0,                     // 日期 MS
        0,                     // 日期 LS
    ] {
        fixed.extend_from_slice(&value.to_le_bytes());
    }

    let string = |key: &str, value: &str| {
        let mut encoded: Vec<u8> = Vec::new();
        for unit in value.encode_utf16() {
            encoded.extend_from_slice(&unit.to_le_bytes());
        }
        encoded.extend_from_slice(&[0, 0]);
        // 字符串型节点：wValueLength 以字符计（含结尾 0）
        version_node(key, &encoded, (value.chars().count() + 1) as u16, 1, &[])
    };

    let table = version_node(
        "040904B0",
        &[],
        0,
        1,
        &[
            string("FileDescription", name),
            string("FileVersion", version),
            string("InternalName", name),
            string("OriginalFilename", &format!("{name}.exe")),
            string("ProductName", name),
            string("ProductVersion", version),
        ],
    );

    let string_file_info = version_node("StringFileInfo", &[], 0, 1, &[table]);

    let mut translation = Vec::with_capacity(4);
    translation.extend_from_slice(&0x0409u16.to_le_bytes());
    translation.extend_from_slice(&0x04B0u16.to_le_bytes());
    let var = version_node("Translation", &translation, 4, 0, &[]);
    let var_file_info = version_node("VarFileInfo", &[], 0, 1, &[var]);

    version_node(
        "VS_VERSION_INFO",
        &fixed,
        52,
        0,
        &[string_file_info, var_file_info],
    )
}

/// 一个版本信息节点：`wLength` + `wValueLength` + `wType` + 键 + 值 + 子节点。
fn version_node(
    key: &str,
    value: &[u8],
    value_length: u16,
    value_type: u16,
    children: &[Vec<u8>],
) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_le_bytes()); // 先占位，最后回填
    out.extend_from_slice(&value_length.to_le_bytes());
    out.extend_from_slice(&value_type.to_le_bytes());

    for unit in key.encode_utf16() {
        out.extend_from_slice(&unit.to_le_bytes());
    }
    out.extend_from_slice(&[0, 0]);
    while out.len() % 4 != 0 {
        out.push(0);
    }

    out.extend_from_slice(value);

    if !children.is_empty() {
        while out.len() % 4 != 0 {
            out.push(0);
        }
        for child in children {
            out.extend_from_slice(child);
            while out.len() % 4 != 0 {
                out.push(0);
            }
        }
    }

    let length = out.len() as u16;
    out[0..2].copy_from_slice(&length.to_le_bytes());
    out
}

/// 把 `2.0.0` 解析成四段数字。
fn version_parts(version: &str) -> (u32, u32, u32, u32) {
    let mut parts = version
        .split('.')
        .map(|part| part.trim().parse::<u32>().unwrap_or(0));

    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    )
}
