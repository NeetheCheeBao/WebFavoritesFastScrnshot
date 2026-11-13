//! 极简 INI 解析器：分节、键值对、行首注释、成对引号；不做行内注释剥离。

use std::collections::HashMap;

/// 解析后的 INI 内容：分节名（小写） -> 键（小写） -> 原始值
#[derive(Debug, Default)]
pub struct Ini {
    sections: HashMap<String, HashMap<String, String>>,
}

impl Ini {
    /// 解析一段 INI 文本。无法识别的行会被忽略，不会报错。
    pub fn parse(text: &str) -> Ini {
        let mut ini = Ini::default();
        let mut current = String::new();

        for raw_line in text.trim_start_matches('\u{feff}').lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }

            if line.starts_with('[') {
                if let Some(end) = line.find(']') {
                    current = line[1..end].trim().to_ascii_lowercase();
                    ini.sections.entry(current.clone()).or_default();
                }
                continue;
            }

            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim().to_ascii_lowercase();
            if key.is_empty() {
                continue;
            }
            let value = unquote(value.trim()).to_string();
            ini.sections
                .entry(current.clone())
                .or_default()
                .insert(key, value);
        }

        ini
    }

    pub fn get(&self, section: &str, key: &str) -> Option<&str> {
        self.sections
            .get(&section.to_ascii_lowercase())
            .and_then(|s| s.get(&key.to_ascii_lowercase()))
            .map(|s| s.as_str())
    }

    pub fn get_or(&self, section: &str, key: &str, default: &str) -> String {
        self.get(section, key).unwrap_or(default).to_string()
    }

    pub fn sections(&self) -> impl Iterator<Item = (&String, &HashMap<String, String>)> {
        self.sections.iter()
    }
}

fn unquote(value: &str) -> &str {
    let bytes = value.as_bytes();
    if bytes.len() >= 2 {
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return &value[1..value.len() - 1];
        }
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sections_and_comments() {
        let ini = Ini::parse(
            "\u{feff}# 注释\n[App]\nname = Demo\nversion=\"1.0.2\"\n\n[zh_CN]\nfile = zh_cn.ini\n",
        );
        assert_eq!(ini.get("app", "name"), Some("Demo"));
        assert_eq!(ini.get("APP", "VERSION"), Some("1.0.2"));
        assert_eq!(ini.get("zh_cn", "file"), Some("zh_cn.ini"));
        assert_eq!(ini.get("app", "missing"), None);
    }
}
