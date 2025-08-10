//! Escape and Unescape Utilities
use fancy_regex::Regex;

/// Escapes special characters in XML attribute values.
pub fn escape_xml_attr_value(s: &str) -> String {
    let mut escaped = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(c),
        }
    }
    escaped
}

/// Escapes special characters in XML text values.
pub fn escape_xml_text_value(s: &str) -> String {
    let mut escaped = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(c),
        }
    }
    escaped
}

lazy_static::lazy_static! {
    static ref XML_NCR_BASE10_REGEX: Regex = Regex::new(r"&#(\d+);").unwrap();
    static ref XML_NCR_BASE16_REGEX: Regex = Regex::new(r"&#x([0-9a-fA-F]+);").unwrap();
    static ref LUA_NCR_BASE10_REGEX: Regex = Regex::new(r"\\(\d{3})").unwrap();
    static ref LUA_NCR_BASE16_REGEX: Regex = Regex::new(r"\\x([0-9a-fA-F]{2})").unwrap();
    static ref LUA_NCR_BASE16_U_REGEX: Regex = Regex::new(r"\\u([0-9a-fA-F]{4})").unwrap();
}

/// Unescapes XML character references and entities.
pub fn unescape_xml(s: &str) -> String {
    let mut s = s.to_owned();
    s = XML_NCR_BASE10_REGEX
        .replace_all(&s, |caps: &fancy_regex::Captures| {
            let codepoint = caps[1].parse::<u32>().unwrap_or(0);
            char::from_u32(codepoint).map_or("�".to_string(), |c| c.to_string())
        })
        .to_string();
    s = XML_NCR_BASE16_REGEX
        .replace_all(&s, |caps: &fancy_regex::Captures| {
            let codepoint = u32::from_str_radix(&caps[1], 16).unwrap_or(0);
            char::from_u32(codepoint).map_or("�".to_string(), |c| c.to_string())
        })
        .to_string();
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

/// Unescapes Lua string escape sequences.
pub fn unescape_lua_str(s: &str) -> String {
    let mut s = s.to_owned();
    s = s
        .replace("\\n", "\n")
        .replace("\\r", "\r")
        .replace("\\t", "\t")
        .replace("\\v", "\x0A")
        .replace("\\b", "\x08")
        .replace("\\f", "\x0C")
        .replace("\\'", "'")
        .replace("\\\"", "\"");
    s = LUA_NCR_BASE10_REGEX
        .replace_all(&s, |caps: &fancy_regex::Captures| {
            let codepoint = caps[1].parse::<u32>().unwrap_or(0);
            char::from_u32(codepoint).map_or("�".to_string(), |c| c.to_string())
        })
        .to_string();
    s = s.replace("\\0", "\0");
    s = LUA_NCR_BASE16_REGEX
        .replace_all(&s, |caps: &fancy_regex::Captures| {
            let codepoint = u32::from_str_radix(&caps[1], 16).unwrap_or(0);
            char::from_u32(codepoint).map_or("�".to_string(), |c| c.to_string())
        })
        .to_string();
    s = LUA_NCR_BASE16_U_REGEX
        .replace_all(&s, |caps: &fancy_regex::Captures| {
            let codepoint = u32::from_str_radix(&caps[1], 16).unwrap_or(0);
            char::from_u32(codepoint).map_or("�".to_string(), |c| c.to_string())
        })
        .to_string();
    s.replace("\\\\", "\\")
}

/// Checks if a string contains characters that need to be escaped in Lua strings.
pub fn lua_str_contains_need_escape(s: &str) -> bool {
    s.contains('\\')
        || s.contains('\n')
        || s.contains('\r')
        || s.contains('\t')
        || s.contains('\x0A')
        || s.contains('\x08')
        || s.contains('\x0C')
        || s.contains('\'')
        || s.contains('"')
}

/// Checks if a string contains characters that need to be escaped in Lua keys.
pub fn lua_key_contains_need_escape(s: &str) -> bool {
    s.chars().next().map_or(false, |c| c.is_ascii_digit())
}

#[test]
fn test_unescape_xml() {
    assert_eq!(
        unescape_xml("Hello &amp;amp; World &lt;script&gt;alert(&#x27;XSS&#x27;)&lt;/script&gt;"),
        "Hello &amp; World <script>alert('XSS')</script>"
    );
    assert_eq!(unescape_xml("&#20320;TEST&#x20;"), "你TEST ");
}

#[test]
fn test_unescape_lua_str() {
    assert_eq!(unescape_lua_str(r"Hello\nWorld"), "Hello\nWorld");
    assert_eq!(unescape_lua_str(r"Tab:\tEnd"), "Tab:\tEnd");
    assert_eq!(unescape_lua_str("Quote: \\' and \\\""), "Quote: ' and \"");
    assert_eq!(unescape_lua_str(r"Backslash:\\Test"), "Backslash:\\Test");
    assert_eq!(unescape_lua_str(r"\065\066\067"), "ABC");
    assert_eq!(unescape_lua_str(r"\x41\x42\x43"), "ABC");
    assert_eq!(unescape_lua_str(r"\u4F60\u597D"), "你好");
    assert_eq!(unescape_lua_str(r"Null:\0End"), "Null:\0End");
    assert_eq!(unescape_lua_str(r"Mix:\n\x41\065\u4F60"), "Mix:\nAA你");
}
