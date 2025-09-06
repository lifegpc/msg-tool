//! tools to process gettext po/pot files
//!
//! See [spec](https://www.gnu.org/software/gettext/manual/html_node/PO-Files.html)
use crate::types::*;
use anyhow::{Result, anyhow};
use std::collections::HashMap;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug)]
/// A comment line
pub enum Comment {
    /// Translator comment, starting with `# `
    Translator(String),
    /// Extracted comment, starting with `#.`.
    Extracted(String),
    /// Reference, starting with `#:`.
    Reference(String),
    /// Flag, starting with `#,`.
    Flag(Vec<String>),
    /// Previous untranslated string, starting with `#|`.
    Previous(String),
    /// Previous message block, starting with `#~`.
    PreviousStr(String),
}

#[derive(Debug)]
/// A line in a .po file.
pub enum PoLine {
    /// A comment line, starting with `#`.
    Comment(Comment),
    /// A msgid line
    MsgId(String),
    /// A msgstr line
    MsgStr(String),
    /// A msgctxt line
    MsgCtxt(String),
    /// A msgid_plural line
    MsgIdPlural(String),
    /// A msgstr[n] line
    MsgStrN(usize, String),
    /// Empty line
    EmptyLine,
}

fn dump_text_in_multi_lines(s: &str) -> Result<String> {
    if s.contains("\n") {
        let mut result = Vec::new();
        result.push("\"\"".to_string());
        let mut s = s;
        while let Some(pos) = s.find('\n') {
            let line = &s[..pos + 1];
            result.push(format!("\"{}\"", escape_c_str(line)?));
            s = &s[pos + 1..];
        }
        if !s.is_empty() {
            result.push(format!("\"{}\"", escape_c_str(s)?));
        }
        Ok(result.join("\n"))
    } else {
        Ok(format!("\"{}\"", escape_c_str(s)?))
    }
}

impl PoLine {
    fn dump(&self) -> Result<String> {
        Ok(match self {
            PoLine::Comment(c) => match c {
                Comment::Translator(s) => format!("# {}", s),
                Comment::Extracted(s) => format!("#. {}", s),
                Comment::Reference(s) => format!("#: {}", s),
                Comment::Flag(flags) => format!("#, {}", flags.join(", ")),
                Comment::Previous(s) => format!("#| {}", s),
                Comment::PreviousStr(s) => format!("#~ {}", s),
            },
            PoLine::MsgId(s) => format!("msgid {}", dump_text_in_multi_lines(s)?),
            PoLine::MsgStr(s) => format!("msgstr {}", dump_text_in_multi_lines(s)?),
            PoLine::MsgCtxt(s) => format!("msgctxt {}", dump_text_in_multi_lines(s)?),
            PoLine::MsgIdPlural(s) => format!("msgid_plural {}", dump_text_in_multi_lines(s)?),
            PoLine::MsgStrN(n, s) => format!("msgstr[{}] {}", n, dump_text_in_multi_lines(s)?),
            PoLine::EmptyLine => String::new(),
        })
    }
}

#[derive(Debug)]
pub enum MsgStr {
    Single(String),
    Plural(Vec<(usize, String)>),
}

#[derive(Debug)]
pub struct PoEntry {
    comments: Vec<Comment>,
    msgctxt: Option<String>,
    msgid: String,
    msgid_plural: Option<String>,
    msgstr: MsgStr,
}

/// Escapes a string according to C-style rules.
///
/// This function handles common escape sequences like \n, \t, \", \\, etc.
/// For other ASCII control characters or non-printable characters, it uses octal notation (e.g., \0, \177).
///
/// # Arguments
/// * `s`: The string slice to be escaped.
///
/// # Returns
/// A `Result<String>` containing the new escaped string.
pub fn escape_c_str(s: &str) -> Result<String> {
    let mut escaped = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            '\\' => escaped.push_str("\\\\"),
            '\"' => escaped.push_str("\\\""),
            '\0' => escaped.push_str("\\0"),
            '\x08' => escaped.push_str("\\b"),
            '\x0c' => escaped.push_str("\\f"),
            '\x0b' => escaped.push_str("\\v"),
            '\x07' => escaped.push_str("\\a"),
            c if c.is_ascii_control() && c != '\n' && c != '\r' && c != '\t' => {
                escaped.push_str(&format!("\\{:03o}", c as u8));
            }
            _ => escaped.push(c),
        }
    }
    Ok(escaped)
}

/// Unescapes a string that has been escaped C-style.
///
/// This function parses common escape sequences (like \n, \t, \", \\) as well as
/// octal (\ooo) and hexadecimal (\xHH) escape notations.
///
/// # Arguments
/// * `s`: The string slice containing C-style escape sequences.
///
/// # Returns
/// A `Result<String>` containing the new unescaped string.
/// If an invalid escape sequence is encountered, an error is returned.
pub fn unescape_c_str(s: &str) -> Result<String> {
    let mut unescaped = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => unescaped.push('\n'),
                Some('r') => unescaped.push('\r'),
                Some('t') => unescaped.push('\t'),
                Some('b') => unescaped.push('\x08'),
                Some('f') => unescaped.push('\x0c'),
                Some('v') => unescaped.push('\x0b'),
                Some('a') => unescaped.push('\x07'),
                Some('\\') => unescaped.push('\\'),
                Some('\'') => unescaped.push('\''),
                Some('\"') => unescaped.push('\"'),
                Some('?') => unescaped.push('?'),
                Some(o @ '0'..='7') => {
                    let mut octal = String::new();
                    octal.push(o);
                    while let Some(peek_char) = chars.peek() {
                        if peek_char.is_digit(8) && octal.len() < 3 {
                            octal.push(chars.next().unwrap());
                        } else {
                            break;
                        }
                    }
                    let value = u8::from_str_radix(&octal, 8).map_err(|e| {
                        anyhow!("Invalid octal escape sequence: \\{}: {}", octal, e)
                    })?;
                    unescaped.push(value as char);
                }
                // --- FIX START: Reworked hexadecimal parsing logic ---
                Some('x') => {
                    let mut hex = String::new();

                    // Read the first character, which must be a hex digit
                    if let Some(c1) = chars.peek() {
                        if c1.is_ascii_hexdigit() {
                            hex.push(chars.next().unwrap());
                        } else {
                            // Handle cases like \xG
                            return Err(anyhow!(
                                "Invalid hex escape sequence: \\x followed by non-hex character '{}'",
                                c1
                            ));
                        }
                    } else {
                        // Handle cases where \x is at the end of the string
                        return Err(anyhow!(
                            "Invalid hex escape sequence: \\x must be followed by a hex digit"
                        ));
                    }

                    // Try to read the second character, which must also be a hex digit
                    if let Some(c2) = chars.peek() {
                        if c2.is_ascii_hexdigit() {
                            hex.push(chars.next().unwrap());
                        } else {
                            // Handle cases like \xFG
                            // We have successfully parsed one digit (like F), but it's followed by an invalid hex character (like G)
                            // As per the test requirements, this should be an error
                            return Err(anyhow!(
                                "Invalid hex escape sequence: \\x{} followed by non-hex character '{}'",
                                hex,
                                c2
                            ));
                        }
                    }

                    let value =
                        u8::from_str_radix(&hex, 16).expect("Hex parsing should be valid here");
                    unescaped.push(value as char);
                }
                // --- FIX END ---
                Some(other) => {
                    return Err(anyhow!("Unknown escape sequence: \\{}", other));
                }
                None => {
                    return Err(anyhow!("String cannot end with a single backslash"));
                }
            }
        } else {
            unescaped.push(c);
        }
    }
    Ok(unescaped)
}

pub struct PoDumper {
    entries: Vec<PoLine>,
}

impl PoDumper {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    fn gen_start_str(encoding: Encoding) -> String {
        let mut map = HashMap::new();
        let content_type = match encoding.charset() {
            Some(e) => format!("text/plain; charset={}", e),
            None => String::from("text/plain"),
        };
        map.insert("Content-Type", content_type);
        map.insert("X-Generator", String::from("msg-tool"));
        map.insert("MIME-Version", String::from("1.0"));
        let mut result = String::new();
        for (k, v) in map {
            result.push_str(&format!("{}: {}\n", k, v));
        }
        result
    }

    pub fn dump(mut self, entries: &[Message], encoding: Encoding) -> Result<String> {
        self.add_entry(PoEntry {
            comments: vec![
                Comment::Translator(String::from("Generated by msg-tool")),
                Comment::Flag(vec![String::from("fuzzy")]),
            ],
            msgctxt: None,
            msgid: String::new(),
            msgid_plural: None,
            msgstr: MsgStr::Single(Self::gen_start_str(encoding)),
        });
        let mut added_messages: HashMap<&String, usize> = HashMap::new();
        for entry in entries {
            let count = added_messages.get(&entry.message).map(|&s| s).unwrap_or(0);
            self.add_entry(PoEntry {
                comments: entry
                    .name
                    .as_ref()
                    .map(|name| vec![Comment::Translator(format!("NAME: {}", name))])
                    .unwrap_or_default(),
                msgctxt: if count > 0 {
                    Some(format!("{}", count))
                } else {
                    None
                },
                msgid: entry.message.clone(),
                msgid_plural: None,
                msgstr: MsgStr::Single(String::new()),
            });
            added_messages.insert(&entry.message, count + 1);
        }
        let mut result = String::new();
        for line in &self.entries {
            result.push_str(&line.dump()?);
            result.push('\n');
        }
        Ok(result)
    }

    fn add_entry(&mut self, entry: PoEntry) {
        for comment in entry.comments {
            self.entries.push(PoLine::Comment(comment));
        }
        if let Some(ctx) = entry.msgctxt {
            self.entries.push(PoLine::MsgCtxt(ctx));
        }
        self.entries.push(PoLine::MsgId(entry.msgid));
        let is_plural = entry.msgid_plural.is_some();
        if let Some(plural) = entry.msgid_plural {
            self.entries.push(PoLine::MsgIdPlural(plural));
        }
        match entry.msgstr {
            MsgStr::Single(s) => {
                if is_plural {
                    self.entries.push(PoLine::MsgStrN(0, s));
                } else {
                    self.entries.push(PoLine::MsgStr(s));
                }
            }
            MsgStr::Plural(v) => {
                for (n, s) in v {
                    self.entries.push(PoLine::MsgStrN(n, s));
                }
            }
        }
        self.entries.push(PoLine::EmptyLine);
    }
}

pub struct PoParser<'a> {
    texts: Vec<&'a str>,
    pos: usize,
    llm_mark: Option<&'a str>,
}

impl<'a> PoParser<'a> {
    pub fn new(s: &'a str, llm_mark: Option<&'a str>) -> Self {
        Self {
            texts: s.graphemes(true).collect(),
            pos: 0,
            llm_mark,
        }
    }

    pub fn parse_lines(&mut self) -> Result<Vec<PoLine>> {
        let mut lines = Vec::new();
        while let Some(f) = self.next_line() {
            let f = f.trim();
            if f.starts_with("#") {
                if f.len() < 2 {
                    return Err(anyhow!("Invalid comment line: {}", f));
                }
                let c = &f[1..];
                if c.starts_with(' ') {
                    lines.push(PoLine::Comment(Comment::Translator(c[1..].to_string())));
                } else if c.starts_with('.') {
                    lines.push(PoLine::Comment(Comment::Extracted(
                        c[1..].trim_start().to_string(),
                    )));
                } else if c.starts_with(':') {
                    lines.push(PoLine::Comment(Comment::Reference(
                        c[1..].trim_start().to_string(),
                    )));
                } else if c.starts_with(',') {
                    let flags = c[1..].split(',').map(|s| s.trim().to_string()).collect();
                    lines.push(PoLine::Comment(Comment::Flag(flags)));
                } else if c.starts_with('|') {
                    lines.push(PoLine::Comment(Comment::Previous(
                        c[1..].trim_start().to_string(),
                    )));
                } else if c.starts_with('~') {
                    lines.push(PoLine::Comment(Comment::PreviousStr(
                        c[1..].trim_start().to_string(),
                    )));
                } else {
                    return Err(anyhow!("Unknown comment type: {}", f));
                }
            } else if f.starts_with("msgid ") {
                let content = self.read_string_literal(&f[6..])?;
                lines.push(PoLine::MsgId(content));
            } else if f.starts_with("msgstr ") {
                let content = self.read_string_literal(&f[7..])?;
                lines.push(PoLine::MsgStr(content));
            } else if f.starts_with("msgctxt ") {
                let content = self.read_string_literal(&f[8..])?;
                lines.push(PoLine::MsgCtxt(content));
            } else if f.starts_with("msgid_plural ") {
                let content = self.read_string_literal(&f[13..])?;
                lines.push(PoLine::MsgIdPlural(content));
            } else if f.starts_with("msgstr[") {
                let end_bracket = f
                    .find(']')
                    .ok_or_else(|| anyhow!("Invalid msgstr[n] line: {}", f))?;
                let index_str = &f[7..end_bracket];
                let index: usize = index_str
                    .parse()
                    .map_err(|_| anyhow!("Invalid index in msgstr[n]: {}", index_str))?;
                let content = self.read_string_literal(&f[end_bracket + 1..])?;
                lines.push(PoLine::MsgStrN(index, content));
            } else if f.trim().is_empty() {
                lines.push(PoLine::EmptyLine);
            } else if f.starts_with('"') {
                // This is a continuation of a previous string.
                // According to GNU gettext manual, a string literal cannot appear on its own.
                // It must follow a keyword. However, some tools generate this.
                // We will append it to the last string-like element.
                let content = self.read_string_literal(&f)?;
                if let Some(last_line) = lines.last_mut() {
                    match last_line {
                        PoLine::MsgId(s) => s.push_str(&content),
                        PoLine::MsgStr(s) => s.push_str(&content),
                        PoLine::MsgCtxt(s) => s.push_str(&content),
                        PoLine::MsgIdPlural(s) => s.push_str(&content),
                        PoLine::MsgStrN(_, s) => s.push_str(&content),
                        _ => return Err(anyhow!("Orphan string literal continuation: {}", f)),
                    }
                } else {
                    return Err(anyhow!("Orphan string literal continuation: {}", f));
                }
            } else {
                return Err(anyhow!("Unknown line type: {}", f));
            }
        }
        Ok(lines)
    }

    fn read_string_literal(&mut self, s: &str) -> Result<String> {
        let mut content = String::new();
        let current_line_str = s.trim();
        if current_line_str.starts_with('"') && current_line_str.ends_with('"') {
            content.push_str(&unescape_c_str(
                &current_line_str[1..current_line_str.len() - 1],
            )?);
        } else {
            return Err(anyhow!("Invalid string literal: {}", s));
        }

        while let Some(peeked_line) = self.peek_line() {
            if peeked_line.trim().starts_with('"') {
                self.next_line(); // consume it
                let trimmed_line = peeked_line.trim();
                if trimmed_line.starts_with('"') && trimmed_line.ends_with('"') {
                    content.push_str(&unescape_c_str(&trimmed_line[1..trimmed_line.len() - 1])?);
                } else {
                    return Err(anyhow!(
                        "Invalid string literal continuation: {}",
                        peeked_line
                    ));
                }
            } else {
                break;
            }
        }
        Ok(content)
    }

    pub fn parse_entries(&mut self) -> Result<Vec<PoEntry>> {
        let lines = self.parse_lines()?;
        let mut entries = Vec::new();
        let mut current_entry_lines: Vec<PoLine> = Vec::new();

        for line in lines {
            if let PoLine::EmptyLine = line {
                if !current_entry_lines.is_empty() {
                    entries.push(self.build_entry_from_lines(current_entry_lines)?);
                    current_entry_lines = Vec::new();
                }
            } else {
                current_entry_lines.push(line);
            }
        }

        if !current_entry_lines.is_empty() {
            entries.push(self.build_entry_from_lines(current_entry_lines)?);
        }

        Ok(entries)
    }

    fn build_entry_from_lines(&self, lines: Vec<PoLine>) -> Result<PoEntry> {
        let mut comments = Vec::new();
        let mut msgctxt: Option<String> = None;
        let mut msgid: Option<String> = None;
        let mut msgid_plural: Option<String> = None;
        let mut msgstr: Option<String> = None;
        let mut msgstr_plural: Vec<(usize, String)> = Vec::new();

        for line in lines {
            match line {
                PoLine::Comment(c) => comments.push(c),
                PoLine::MsgCtxt(s) => {
                    if msgctxt.is_some() {
                        return Err(anyhow!("Duplicate msgctxt in entry"));
                    }
                    msgctxt = Some(s);
                }
                PoLine::MsgId(s) => {
                    if msgid.is_some() {
                        return Err(anyhow!("Duplicate msgid in entry"));
                    }
                    msgid = Some(s);
                }
                PoLine::MsgIdPlural(s) => {
                    if msgid_plural.is_some() {
                        return Err(anyhow!("Duplicate msgid_plural in entry"));
                    }
                    msgid_plural = Some(s);
                }
                PoLine::MsgStr(s) => {
                    if msgstr.is_some() {
                        return Err(anyhow!("Duplicate msgstr in entry"));
                    }
                    msgstr = Some(s);
                }
                PoLine::MsgStrN(n, s) => {
                    if msgstr_plural.iter().any(|(i, _)| *i == n) {
                        return Err(anyhow!("Duplicate msgstr[{}] in entry", n));
                    }
                    msgstr_plural.push((n, s));
                }
                PoLine::EmptyLine => {
                    // This should not be reached if called from parse_entries
                    return Err(anyhow!("Unexpected empty line in build_entry_from_lines"));
                }
            }
        }

        let final_msgstr = if !msgstr_plural.is_empty() {
            if msgstr.is_some() {
                return Err(anyhow!(
                    "Mixing msgstr and msgstr[n] in the same entry is not allowed"
                ));
            }
            MsgStr::Plural(msgstr_plural)
        } else {
            MsgStr::Single(msgstr.unwrap_or_default())
        };

        Ok(PoEntry {
            comments,
            msgctxt,
            msgid: msgid.ok_or_else(|| anyhow!("Entry is missing msgid"))?,
            msgid_plural,
            msgstr: final_msgstr,
        })
    }

    fn peek(&self) -> Option<&'a str> {
        self.texts.get(self.pos).copied()
    }

    fn peek_line(&self) -> Option<String> {
        let mut line = String::new();
        let mut current_pos = self.pos;
        while let Some(c) = self.texts.get(current_pos).copied() {
            current_pos += 1;
            if c == "\n" || c == "\r\n" {
                break;
            }
            line.push_str(c);
        }
        if line.is_empty() && current_pos >= self.texts.len() {
            None
        } else {
            Some(line)
        }
    }

    fn next_line(&mut self) -> Option<String> {
        let mut line = String::new();
        while let Some(c) = self.next() {
            if c == "\n" || c == "\r\n" {
                break;
            }
            line.push_str(c);
        }
        if line.is_empty() && self.peek().is_none() {
            None
        } else {
            Some(line)
        }
    }

    fn next(&mut self) -> Option<&'a str> {
        let r = self.texts.get(self.pos).copied();
        if r.is_some() {
            self.pos += 1;
        }
        r
    }

    pub fn parse(&mut self) -> Result<Vec<Message>> {
        let mut messages = Vec::new();
        let mut llm = None;
        let mut name = None;
        for entry in self.parse_entries()? {
            if entry.msgid.is_empty() {
                // This is the header entry, skip it
                continue;
            }
            for comment in &entry.comments {
                if let Comment::Translator(s) = comment {
                    let s = s.trim();
                    if s.starts_with("NAME:") {
                        name = Some(s[5..].trim().to_string());
                    } else if s.starts_with("LLM:") {
                        llm = Some(s[4..].trim().to_string());
                    }
                }
            }
            let message = match entry.msgstr {
                MsgStr::Single(s) => {
                    let s = s.trim();
                    if s.is_empty() {
                        llm.take()
                            .map(|mut llm| {
                                if let Some(mark) = self.llm_mark {
                                    llm.push_str(mark);
                                }
                                llm
                            })
                            .unwrap_or_else(|| {
                                String::from(if entry.msgid.is_empty() { "" } else { "" })
                            })
                    } else {
                        let mut tmp = s.to_string();
                        if let Some(llm) = llm.take() {
                            if tmp == llm {
                                if let Some(mark) = self.llm_mark {
                                    tmp.push_str(mark);
                                }
                            }
                        }
                        tmp
                    }
                }
                MsgStr::Plural(_) => {
                    return Err(anyhow!("Plural msgstr not supported in this context"));
                }
            };
            let m = Message::new(message, name.take());
            messages.push(m);
        }
        Ok(messages)
    }
}

// --- Unit Tests ---
#[cfg(test)]
mod c_escape_tests {
    use super::*;

    #[test]
    fn test_escape_basic() {
        assert_eq!(escape_c_str("hello world").unwrap(), "hello world");
    }

    #[test]
    fn test_escape_quotes_and_slashes() {
        assert_eq!(
            escape_c_str(r#"he"llo\world"#).unwrap(),
            r#"he\"llo\\world"#
        );
    }

    #[test]
    fn test_escape_control_chars() {
        assert_eq!(escape_c_str("a\nb\tc\rd").unwrap(), r#"a\nb\tc\rd"#);
        assert_eq!(escape_c_str("\0").unwrap(), r#"\0"#);
    }

    #[test]
    fn test_escape_other_control_chars_as_octal() {
        assert_eq!(escape_c_str("\x07").unwrap(), r#"\a"#);
        assert_eq!(escape_c_str("\x7f").unwrap(), r#"\177"#);
    }

    #[test]
    fn test_unescape_basic() {
        assert_eq!(unescape_c_str("hello world").unwrap(), "hello world");
    }

    #[test]
    fn test_unescape_quotes_and_slashes() {
        assert_eq!(
            unescape_c_str(r#"he\"llo\\world"#).unwrap(),
            r#"he"llo\world"#
        );
    }

    #[test]
    fn test_unescape_control_chars() {
        assert_eq!(unescape_c_str(r#"a\nb\tc\rd"#).unwrap(), "a\nb\tc\rd");
    }

    #[test]
    fn test_unescape_octal() {
        assert_eq!(unescape_c_str(r#"\101"#).unwrap(), "A");
        assert_eq!(unescape_c_str(r#"\60"#).unwrap(), "0");
        assert_eq!(unescape_c_str(r#"\0"#).unwrap(), "\0");
        assert_eq!(unescape_c_str(r#"\177"#).unwrap(), "\x7f");
        assert_eq!(unescape_c_str(r#"hello\101world"#).unwrap(), "helloAworld");
    }

    #[test]
    fn test_unescape_hex() {
        assert_eq!(unescape_c_str(r#"\x41"#).unwrap(), "A");
        assert_eq!(unescape_c_str(r#"\x30"#).unwrap(), "0");
        assert_eq!(unescape_c_str(r#"\x7F"#).unwrap(), "\x7f");
        assert_eq!(unescape_c_str(r#"\x7f"#).unwrap(), "\x7f");
        assert_eq!(unescape_c_str(r#"hello\x41world"#).unwrap(), "helloAworld");
        // A single hex digit is also valid
        assert_eq!(unescape_c_str(r#"\xF"#).unwrap(), "\x0f");
    }

    #[test]
    fn test_unescape_mixed() {
        let original = "A\tB\"C\\D\0E";
        let escaped = r#"A\tB\"C\\D\0E"#;
        assert_eq!(unescape_c_str(escaped).unwrap(), original);
    }

    #[test]
    fn test_unescape_invalid_sequence() {
        assert!(unescape_c_str(r#"\q"#).is_err());
        assert!(unescape_c_str(r#"hello\"#).is_err());
        assert!(unescape_c_str(r#"\x"#).is_err());
        // New: test \x followed immediately by an invalid character
        assert!(unescape_c_str(r#"\xG"#).is_err());
        // This should now pass
        assert!(unescape_c_str(r#"\xFG"#).is_err());
        assert!(unescape_c_str(r#"\8"#).is_err());
    }
}
