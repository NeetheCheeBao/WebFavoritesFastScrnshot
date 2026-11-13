//! 语言包加载与文案查表。
//!
//! 语言文件放在项目根目录的 `lang/` 下，由 `lang/index.ini` 索引。
//! 运行时优先读取磁盘上的语言文件，缺失时使用编译期嵌入的副本，
//! 因此既支持"改文件即换文案"，也支持单文件分发。

use std::collections::HashMap;

use crate::config::{ini::Ini, read_text, resource_candidates};

/// 语言包索引（编译期嵌入）。
const INDEX_INI: &str = include_str!("../../lang/index.ini");

/// 内置语言包：语言代码 -> 文件内容（编译期嵌入）。
const EMBEDDED_LOCALES: &[(&str, &str)] = &[("zh_cn", include_str!("../../lang/zh_cn.ini"))];

/// 默认语言代码，索引文件缺失时使用。
const FALLBACK_CODE: &str = "zh_cn";

/// 已加载的语言包。
#[derive(Debug, Default)]
pub struct I18n {
    code: String,
    values: HashMap<String, String>,
}

impl I18n {
    /// 按索引文件加载默认语言包。
    pub fn load() -> I18n {
        let index = Ini::parse(INDEX_INI);
        let code = index
            .get("general", "default")
            .unwrap_or(FALLBACK_CODE)
            .to_ascii_lowercase();
        let file = index
            .get(&code, "file")
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("{code}.ini"));

        let text = read_text(&resource_candidates(&format!("lang/{file}")))
            .or_else(|| embedded_locale(&code).map(|s| s.to_string()))
            .or_else(|| embedded_locale(FALLBACK_CODE).map(|s| s.to_string()))
            .unwrap_or_default();

        I18n::from_text(&code, &text)
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

    /// 当前语言代码，例如 `zh_cn`。
    #[allow(dead_code)]
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

/// 按语言代码取内置语言包内容。
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
