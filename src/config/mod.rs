//! 程序元信息（名称、版本）与资源路径解析。
//!
//! 名称与版本来自项目根目录的 `version.ini`；文件缺失时回退到编译期嵌入的副本，
//! 因此单独分发 exe 也能正常显示标题。

pub mod ini;

use std::path::{Path, PathBuf};

use crate::i18n::I18n;

/// 编译期嵌入的版本文件，作为运行时缺失的后备。
const EMBEDDED_VERSION_INI: &str = include_str!("../../version.ini");

/// 状态栏空闲时显示的版权信息（写死在代码里，不进语言包）。
pub const COPYRIGHT: &str = "©2026 NeetheCheeBao";

/// 项目仓库地址（点击版权信息时打开）。
pub const REPOSITORY_URL: &str = "https://github.com/NeetheCheeBao/WebFavoritesFastScrnshot";

/// 程序名与版本号。
#[derive(Debug, Clone)]
pub struct AppMeta {
    pub name: String,
    pub version: String,
}

impl AppMeta {
    /// 依次尝试 exe 同级目录、当前工作目录，最后使用嵌入副本。
    pub fn load() -> AppMeta {
        let text = read_text(&resource_candidates("version.ini"))
            .unwrap_or_else(|| EMBEDDED_VERSION_INI.to_string());
        let ini = ini::Ini::parse(&text);

        AppMeta {
            name: ini.get_or("app", "name", "WebFavoritesFastScrnshot"),
            version: ini.get_or("app", "version", "1.0.0"),
        }
    }

    /// 按语言包模板拼出窗口标题，例如 `WebFavoritesFastScrnshot v1.0.2`。
    pub fn title(&self, i18n: &I18n) -> String {
        i18n.fmt(
            "app.window_title",
            &[("name", &self.name), ("version", &self.version)],
        )
    }
}

/// 可执行文件所在目录。
pub fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
}

/// 相对资源的候选路径：exe 同级目录优先，其次是当前工作目录。
pub fn resource_candidates(relative: &str) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(dir) = exe_dir() {
        paths.push(dir.join(relative));
    }
    if let Ok(cwd) = std::env::current_dir() {
        let candidate = cwd.join(relative);
        if !paths.contains(&candidate) {
            paths.push(candidate);
        }
    }
    paths
}

/// 按候选顺序读取第一个存在的文本文件。
pub fn read_text(paths: &[PathBuf]) -> Option<String> {
    for path in paths {
        if let Ok(text) = std::fs::read_to_string(path) {
            return Some(text);
        }
    }
    None
}

/// 判断路径是否以 `.html` 结尾（不区分大小写）。
pub fn is_html_path(path: &Path) -> bool {
    path.extension()
        .map(|ext| ext.eq_ignore_ascii_case("html"))
        .unwrap_or(false)
}
