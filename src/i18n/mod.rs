//! 语言包加载与文案查表；`lang/index.ini` 索引，磁盘优先，缺失时用编译期嵌入的副本。

use std::collections::HashMap;

use crate::config::{ini::Ini, read_text, resource_candidates};

/// 语言包索引（编译期嵌入）。
const INDEX_INI: &str = include_str!("../../lang/index.ini");

/// 内置语言包：语言代码 -> 文件内容（编译期嵌入）。
const EMBEDDED_LOCALES: &[(&str, &str)] = &[
    ("en_us", include_str!("../../lang/en_us.ini")),
    ("zh_cn", include_str!("../../lang/zh_cn.ini")),
];

/// 默认语言代码，索引文件缺失时使用。
const FALLBACK_CODE: &str = "zh_cn";

/// 已加载的语言包。
#[derive(Debug, Default)]
pub struct I18n {
    code: String,
    values: HashMap<String, String>,
}

impl I18n {
    pub fn load() -> I18n {
        let index = Ini::parse(&index_text());
        let code = index
            .get("general", "default")
            .unwrap_or(FALLBACK_CODE)
            .to_ascii_lowercase();

        I18n::load_code(&code).unwrap_or_else(|| {
            let text = embedded_locale(FALLBACK_CODE).unwrap_or_default();
            I18n::from_text(FALLBACK_CODE, text)
        })
    }

    /// 按语言代码加载；磁盘优先，其次编译期嵌入。
    pub fn load_code(code: &str) -> Option<I18n> {
        let index = Ini::parse(&index_text());
        let file = index
            .get(code, "file")
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("{code}.ini"));

        let text = read_text(&resource_candidates(&format!("lang/{file}")))
            .or_else(|| embedded_locale(code).map(|s| s.to_string()))?;

        Some(I18n::from_text(code, &text))
    }

    /// 从语言文件文本构建（供测试与将来切换语言使用）。
    pub fn from_text(code: &str, text: &str) -> I18n {
        let ini = Ini::parse(text);
        let mut values = HashMap::new();

        for (section, entries) in ini.sections() {
            for (key, value) in entries {
                values.insert(
                    format!("{section}.{key}"),
                    value.replace("\\n", "\n").replace("\\t", "\t"),
                );
            }
        }

        I18n {
            code: code.to_string(),
            values,
        }
    }

    /// 可用语言：`(代码, 显示名)`，按代码排序。
    pub fn languages() -> Vec<(String, String)> {
        let index = Ini::parse(&index_text());
        let mut list: Vec<(String, String)> = index
            .sections()
            .into_iter()
            .filter(|(section, _)| section.as_str() != "general")
            .filter_map(|(section, entries)| {
                let name = entries
                    .iter()
                    .find(|(key, _)| key.as_str() == "name")
                    .map(|(_, value)| value.clone())?;
                Some((section.clone(), name))
            })
            .collect();

        list.sort_by(|a, b| a.0.cmp(&b.0));
        list
    }

    /// 当前语言代码，例如 `zh_cn`。
    pub fn code(&self) -> &str {
        &self.code
    }

    /// 查表；缺失时原样返回键名，便于发现漏配。
    pub fn t<'a>(&'a self, key: &'a str) -> &'a str {
        self.values.get(key).map(|s| s.as_str()).unwrap_or(key)
    }

    /// 查表并替换 `{占位符}`。
    pub fn fmt(&self, key: &str, args: &[(&str, &str)]) -> String {
        let mut text = self.t(key).to_string();
        for (name, value) in args {
            text = text.replace(&format!("{{{name}}}"), value);
        }
        text
    }
}

/// 索引文件文本：磁盘优先，缺失时用编译期副本。
fn index_text() -> String {
    read_text(&resource_candidates("lang/index.ini")).unwrap_or_else(|| INDEX_INI.to_string())
}

fn embedded_locale(code: &str) -> Option<&'static str> {
    EMBEDDED_LOCALES
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, text)| *text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_default_locale() {
        let i18n = I18n::load();
        assert_eq!(i18n.code(), "zh_cn");
        assert_eq!(i18n.t("toolbar.open"), "打开文件");
        assert_eq!(i18n.t("no.such.key"), "no.such.key");
    }

    #[test]
    fn formats_placeholders_and_newlines() {
        let i18n = I18n::load();
        let title = i18n.fmt(
            "app.window_title",
            &[("name", "Demo"), ("version", "9.9.9")],
        );
        assert_eq!(title, "Demo v9.9.9");

        let text = i18n.fmt("error.file_missing", &[("path", "C:/a.html")]);
        assert!(text.contains('\n'));
        assert!(text.ends_with("C:/a.html"));
    }
}
