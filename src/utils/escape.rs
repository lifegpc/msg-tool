use fancy_regex::Regex;

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
}

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

#[test]
fn test_unescape_xml() {
    assert_eq!(
        unescape_xml("Hello &amp;amp; World &lt;script&gt;alert(&#x27;XSS&#x27;)&lt;/script&gt;"),
        "Hello &amp; World <script>alert('XSS')</script>"
    );
    assert_eq!(unescape_xml("&#20320;TEST&#x20;"), "你TEST ");
}
