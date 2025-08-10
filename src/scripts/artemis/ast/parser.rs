use super::types::*;
use crate::types::*;
use crate::utils::encoding::*;
use crate::utils::escape::unescape_lua_str;
use anyhow::Result;

/// A parser for Artemis AST scripts.
pub struct Parser<'a> {
    str: &'a [u8],
    pos: usize,
    len: usize,
    line: usize,
    line_index: usize,
    encoding: Encoding,
}

impl<'a> Parser<'a> {
    /// Creates a new parser for the given string with the specified encoding.
    ///
    /// * `str` - The string to parse.
    /// * `encoding` - The encoding of the string.
    pub fn new<S: AsRef<[u8]> + ?Sized>(str: &'a S, encoding: Encoding) -> Self {
        let str = str.as_ref();
        Parser {
            str,
            pos: 0,
            len: str.len(),
            line: 1,
            line_index: 1,
            encoding,
        }
    }

    /// Checks if input is a valid header for an AST file.
    pub fn try_parse_header(mut self) -> Result<()> {
        self.erase_whitespace();
        if self.is_indent(b"astver") {
            self.parse_indent(b"astver")?;
            self.parse_equal()?;
            self.parse_f64()?;
        } else if self.is_indent(b"astname") {
            self.parse_indent(b"astname")?;
            self.parse_equal()?;
        } else if self.is_indent(b"ast") {
            self.parse_indent(b"ast")?;
            self.parse_equal()?;
        } else {
            return self.error("expected 'astver', 'astname' or 'ast'");
        }
        Ok(())
    }

    /// Parses the AST file and returns an [AstFile] object.
    pub fn parse(mut self) -> Result<AstFile> {
        self.erase_whitespace();
        let astver = if self.is_indent(b"astver") {
            self.parse_indent(b"astver")?;
            self.parse_equal()?;
            Some(self.parse_f64()?)
        } else {
            None
        };
        self.erase_whitespace();
        let mut astname = None;
        if self.is_indent(b"astname") {
            self.parse_indent(b"astname")?;
            self.parse_equal()?;
            astname = Some(self.parse_any_str()?.to_string());
            self.erase_whitespace();
        }
        self.parse_indent(b"ast")?;
        self.parse_equal()?;
        let ast = self.parse_value()?;
        Ok(AstFile {
            astver,
            astname,
            ast,
        })
    }

    fn parse_equal(&mut self) -> Result<()> {
        self.erase_whitespace();
        match self.next() {
            Some(b'=') => Ok(()),
            _ => self.error("expected '='"),
        }
    }

    fn parse_value(&mut self) -> Result<Value> {
        self.erase_whitespace();
        match self.peek() {
            Some(t) => match t {
                b'"' => return self.parse_str().map(|x| Value::Str(x.to_string())),
                b'[' => {
                    self.eat_char();
                    match self.peek().ok_or(self.error2("unexpected eof"))? {
                        b'[' => {
                            self.pos -= 1; // Rewind to the first '['
                            self.parse_raw_str().map(|x| Value::Str(x))
                        }
                        _ => {
                            self.pos -= 1;
                            self.parse_key_val()
                        }
                    }
                }
                b'-' | b'.' | b'0'..=b'9' => return self.parse_any_number(),
                b'n' => {
                    if self.is_indent(b"nil") {
                        self.pos += 3; // Skip "nil"
                        Ok(Value::Null)
                    } else {
                        self.parse_key_val()
                    }
                }
                b'_' | b'a'..=b'z' | b'A'..=b'Z' | b']' => return self.parse_key_val(),
                b'{' => return self.parse_array(),
                _ => return self.error(format!("unexpected token: {}", t)),
            },
            None => return self.error("unexpected eof"),
        }
    }

    fn parse_array(&mut self) -> Result<Value> {
        self.erase_whitespace();
        self.parse_indent(b"{")?;
        let mut array = Vec::new();
        loop {
            self.erase_whitespace();
            match self.peek() {
                Some(b'}') => {
                    self.eat_char();
                    break;
                }
                Some(_) => {
                    let val = self.parse_value()?;
                    array.push(val);
                    match self.peek() {
                        Some(b',') => {
                            self.eat_char();
                        }
                        _ => {}
                    }
                }
                None => return self.error("unexpected eof"),
            }
        }
        Ok(Value::Array(array))
    }

    fn parse_any_number(&mut self) -> Result<Value> {
        self.erase_whitespace();
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c == b'.' || c == b'-' || c.is_ascii_digit() {
                self.eat_char();
            } else {
                break;
            }
        }
        let s = std::str::from_utf8(&self.str[start..self.pos])?;
        if s.contains('.') {
            s.parse()
                .map(Value::Float)
                .map_err(|e| self.error2(format!("failed to parse f64: {}", e)))
        } else {
            s.parse()
                .map(Value::Int)
                .map_err(|e| self.error2(format!("failed to parse i64: {}", e)))
        }
    }

    fn parse_any_str(&mut self) -> Result<String> {
        self.erase_whitespace();
        match self.peek().ok_or(self.error2("unexpected eof"))? {
            b'"' => self.parse_str(),
            b'[' => self.parse_raw_str(),
            _ => self.error("expected string or raw string"),
        }
    }

    fn parse_f64(&mut self) -> Result<f64> {
        self.erase_whitespace();
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c == b'.' || c == b'-' || c.is_ascii_digit() {
                self.eat_char();
            } else {
                break;
            }
        }
        let s = std::str::from_utf8(&self.str[start..self.pos])?;
        s.parse()
            .map_err(|e| self.error2(format!("failed to parse f64: {}", e)))
    }

    fn parse_str(&mut self) -> Result<String> {
        self.erase_whitespace();
        self.parse_indent(b"\"")?;
        let start = self.pos;
        let mut pc = None;
        let end = loop {
            match self.next() {
                Some(c) => {
                    if c == b'"' {
                        if pc.is_none_or(|x| x != b'\\') {
                            break self.pos - 1;
                        }
                    }
                    pc = Some(c);
                }
                None => return self.error("unexpected eof"),
            }
        };
        Ok(unescape_lua_str(
            &decode_to_string(self.encoding, &self.str[start..end], true)
                .map_err(|e| self.error2(e))?,
        ))
    }

    fn parse_raw_str(&mut self) -> Result<String> {
        self.erase_whitespace();
        self.parse_indent(b"[[")?;
        let start = self.pos;
        let mut pc = None;
        let end = loop {
            match self.next() {
                Some(c) => {
                    if c == b']' {
                        if pc.is_some_and(|x| x == b']') {
                            break self.pos - 2;
                        }
                    }
                    pc = Some(c);
                }
                None => return self.error("unexpected eof"),
            }
        };
        decode_to_string(self.encoding, &self.str[start..end], true).map_err(|e| self.error2(e))
    }

    fn erase_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c == b' ' || c == b'\t' || c == b'\n' || c == b'\r' {
                if c == b'\n' {
                    self.line += 1;
                    self.line_index = 1;
                } else {
                    self.line_index += 1;
                }
                self.eat_char();
            } else {
                break;
            }
        }
    }

    fn next(&mut self) -> Option<u8> {
        if self.pos < self.len {
            let c = self.str[self.pos];
            self.pos += 1;
            if c == b'\n' {
                self.line += 1;
                self.line_index = 1;
            } else {
                self.line_index += 1;
            }
            Some(c)
        } else {
            None
        }
    }

    fn peek(&self) -> Option<u8> {
        if self.pos < self.len {
            Some(self.str[self.pos])
        } else {
            None
        }
    }

    fn parse_key_val(&mut self) -> Result<Value> {
        let key = self.get_indent()?;
        self.parse_equal()?;
        let val = self.parse_value()?;
        Ok(Value::KeyVal((Box::new(key), Box::new(val))))
    }

    fn get_indent(&mut self) -> Result<Value> {
        self.erase_whitespace();
        let start = self.pos;
        let mut is_first = true;
        let end = loop {
            match self.peek() {
                Some(t) => match t {
                    b'_' | b'a'..=b'z' | b'A'..=b'Z' | b'"' => self.eat_char(),
                    b'[' => {
                        self.eat_char();
                        let v = self.parse_value()?;
                        let n = self.next().ok_or(self.error2("unexpected eof"))?;
                        if n != b']' {
                            return self.error("expected ']' after key");
                        }
                        return Ok(v);
                    }
                    b'0'..=b'9' => {
                        if is_first {
                            return self.error("unexpected digit");
                        }
                        self.eat_char();
                    }
                    b' ' | b'\t' | b'=' | b'\n' | b'\r' => break self.pos,
                    _ => return self.error("unexpected token"),
                },
                None => return self.error("unexpected eof"),
            }
            is_first = false;
        };
        let mut data = &self.str[start..end];
        if data.starts_with(b"[\"") && data.ends_with(b"\"]") {
            data = &data[2..data.len() - 2];
        }
        Ok(Value::Str(
            decode_to_string(self.encoding, data, true).map_err(|e| self.error2(e))?,
        ))
    }

    fn is_indent(&self, indent: &[u8]) -> bool {
        if self.pos + indent.len() > self.len {
            return false;
        }
        for (i, c) in indent.iter().enumerate() {
            if self.str[self.pos + i] != *c {
                return false;
            }
        }
        true
    }

    fn parse_indent(&mut self, indent: &[u8]) -> Result<()> {
        for c in indent {
            match self.next() {
                Some(x) => {
                    if x != *c {
                        return self.error("unexpected indent");
                    }
                }
                None => return self.error("unexpected eof"),
            }
        }
        Ok(())
    }

    fn eat_char(&mut self) {
        if self.pos < self.len {
            self.pos += 1;
        }
    }

    fn error2<T>(&self, msg: T) -> anyhow::Error
    where
        T: std::fmt::Display,
    {
        anyhow::Error::msg(format!(
            "Failed to parse at position line {} column {} (byte {}): {}",
            self.line, self.line_index, self.pos, msg
        ))
    }

    fn error<T, A>(&self, msg: T) -> Result<A>
    where
        T: std::fmt::Display,
    {
        Err(anyhow::Error::msg(format!(
            "Failed to parse at position line {} column {} (byte {}): {}",
            self.line, self.line_index, self.pos, msg
        )))
    }
}
