//! HTML 实体解码：覆盖常用命名实体与全部数字实体，未收录的按原文保留。

/// 常用命名实体表（不含结尾分号）。
const NAMED: &[(&str, char)] = &[
    ("amp", '&'),
    ("lt", '<'),
    ("gt", '>'),
    ("quot", '"'),
    ("apos", '\''),
    ("nbsp", '\u{00a0}'),
    ("ensp", '\u{2002}'),
    ("emsp", '\u{2003}'),
    ("thinsp", '\u{2009}'),
    ("zwnj", '\u{200c}'),
    ("zwj", '\u{200d}'),
    ("shy", '\u{00ad}'),
    ("copy", '©'),
    ("reg", '®'),
    ("trade", '™'),
    ("hellip", '…'),
    ("mdash", '—'),
    ("ndash", '–'),
    ("lsquo", '‘'),
    ("rsquo", '’'),
    ("ldquo", '“'),
    ("rdquo", '”'),
    ("laquo", '«'),
    ("raquo", '»'),
    ("lsaquo", '‹'),
    ("rsaquo", '›'),
    ("bull", '•'),
    ("middot", '·'),
    ("dagger", '†'),
    ("Dagger", '‡'),
    ("prime", '′'),
    ("Prime", '″'),
    ("permil", '‰'),
    ("sect", '§'),
    ("para", '¶'),
    ("deg", '°'),
    ("plusmn", '±'),
    ("times", '×'),
    ("divide", '÷'),
    ("frac12", '½'),
    ("frac14", '¼'),
    ("frac34", '¾'),
    ("sup1", '¹'),
    ("sup2", '²'),
    ("sup3", '³'),
    ("micro", 'µ'),
    ("cent", '¢'),
    ("pound", '£'),
    ("yen", '¥'),
    ("euro", '€'),
    ("curren", '¤'),
    ("larr", '←'),
    ("uarr", '↑'),
    ("rarr", '→'),
    ("darr", '↓'),
    ("harr", '↔'),
    ("crarr", '↵'),
    ("lArr", '⇐'),
    ("rArr", '⇒'),
    ("hArr", '⇔'),
    ("infin", '∞'),
    ("ne", '≠'),
    ("le", '≤'),
    ("ge", '≥'),
    ("asymp", '≈'),
    ("equiv", '≡'),
    ("sum", '∑'),
    ("radic", '√'),
    ("spades", '♠'),
    ("clubs", '♣'),
    ("hearts", '♥'),
    ("diams", '♦'),
    ("loz", '◊'),
    ("star", '☆'),
    ("starf", '★'),
    ("check", '✓'),
    ("cross", '✗'),
    ("iexcl", '¡'),
    ("iquest", '¿'),
    ("acute", '´'),
    ("uml", '¨'),
    ("macr", '¯'),
    ("cedil", '¸'),
    ("ordf", 'ª'),
    ("ordm", 'º'),
    ("not", '¬'),
    ("brvbar", '¦'),
    ("Agrave", 'À'),
    ("Aacute", 'Á'),
    ("Acirc", 'Â'),
    ("Atilde", 'Ã'),
    ("Auml", 'Ä'),
    ("Aring", 'Å'),
    ("AElig", 'Æ'),
    ("Ccedil", 'Ç'),
    ("Egrave", 'È'),
    ("Eacute", 'É'),
    ("Ecirc", 'Ê'),
    ("Euml", 'Ë'),
    ("Igrave", 'Ì'),
    ("Iacute", 'Í'),
    ("Icirc", 'Î'),
    ("Iuml", 'Ï'),
    ("Ntilde", 'Ñ'),
    ("Ograve", 'Ò'),
    ("Oacute", 'Ó'),
    ("Ocirc", 'Ô'),
    ("Otilde", 'Õ'),
    ("Ouml", 'Ö'),
    ("Oslash", 'Ø'),
    ("Ugrave", 'Ù'),
    ("Uacute", 'Ú'),
    ("Ucirc", 'Û'),
    ("Uuml", 'Ü'),
    ("Yacute", 'Ý'),
    ("szlig", 'ß'),
    ("agrave", 'à'),
    ("aacute", 'á'),
    ("acirc", 'â'),
    ("atilde", 'ã'),
    ("auml", 'ä'),
    ("aring", 'å'),
    ("aelig", 'æ'),
    ("ccedil", 'ç'),
    ("egrave", 'è'),
    ("eacute", 'é'),
    ("ecirc", 'ê'),
    ("euml", 'ë'),
    ("igrave", 'ì'),
    ("iacute", 'í'),
    ("icirc", 'î'),
    ("iuml", 'ï'),
    ("ntilde", 'ñ'),
    ("ograve", 'ò'),
    ("oacute", 'ó'),
    ("ocirc", 'ô'),
    ("otilde", 'õ'),
    ("ouml", 'ö'),
    ("oslash", 'ø'),
    ("ugrave", 'ù'),
    ("uacute", 'ú'),
    ("ucirc", 'û'),
    ("uuml", 'ü'),
    ("yacute", 'ý'),
    ("yuml", 'ÿ'),
];

pub fn unescape(input: &str) -> String {
    if !input.contains('&') {
        return input.to_string();
    }

    let mut out = String::with_capacity(input.len());
    let mut rest = input;

    while let Some(pos) = rest.find('&') {
        out.push_str(&rest[..pos]);
        let tail = &rest[pos..];

        match decode_entity(tail) {
            Some((ch, consumed)) => {
                out.push(ch);
                rest = &tail[consumed..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }

    out.push_str(rest);
    out
}

/// 尝试解码以 `&` 开头的实体，返回（字符, 消耗的字节数）。
fn decode_entity(text: &str) -> Option<(char, usize)> {
    let semi = text.find(';')?;
    // 命名实体最长不会超过这个长度，避免把正文里的 & 当成实体扫描太远
    if semi > 12 {
        return None;
    }

    let body = &text[1..semi];
    let consumed = semi + 1;

    if let Some(number) = body.strip_prefix('#') {
        let value = if let Some(hex) = number.strip_prefix(['x', 'X']) {
            u32::from_str_radix(hex, 16).ok()?
        } else {
            number.parse::<u32>().ok()?
        };
        return char::from_u32(value).map(|ch| (ch, consumed));
    }

    NAMED
        .iter()
        .find(|(name, _)| *name == body)
        .map(|(_, ch)| (*ch, consumed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_named_and_numeric() {
        assert_eq!(unescape("A &amp; B"), "A & B");
        assert_eq!(unescape("&lt;div&gt;"), "<div>");
        assert_eq!(unescape("&#20320;&#22909;"), "你好");
        assert_eq!(unescape("&#x4F60;"), "你");
        assert_eq!(unescape("100% &notanentity"), "100% &notanentity");
        assert_eq!(unescape("a &amp b"), "a &amp b");
    }
}
