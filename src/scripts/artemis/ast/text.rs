use super::types::*;
use crate::utils::escape::*;
use anyhow::Result;
use unicode_segmentation::UnicodeSegmentation;

fn escape_text(s: &str) -> String {
    let mut escaped = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            _ => escaped.push(c),
        }
    }
    escaped
}

pub struct TextGenerator {
    data: String,
}

impl TextGenerator {
    pub fn new() -> Self {
        TextGenerator {
            data: String::new(),
        }
    }

    pub fn generate(mut self, v: &Value) -> Result<String> {
        for (i, item) in v.members().enumerate() {
            match item {
                Value::Str(s) => {
                    self.data.push_str(&escape_text(s));
                }
                Value::Float(_) => {
                    return Err(anyhow::anyhow!(
                        "Unexpected float value at {} in text: item={:?}, {:?}",
                        i,
                        item,
                        v
                    ));
                }
                Value::Int(_) => {
                    return Err(anyhow::anyhow!(
                        "Unexpected int value at {} in text: item={:?}, {:?}",
                        i,
                        item,
                        v
                    ));
                }
                Value::KeyVal((k, _)) => {
                    if k != "name" {
                        return Err(anyhow::anyhow!(
                            "Unexpected key at {} in text: item={:?}, {:?}",
                            i,
                            item,
                            v
                        ));
                    }
                }
                Value::Array(arr) => {
                    self.data.push('<');
                    let mut first = true;
                    for item in arr {
                        if !first {
                            self.data.push(' ');
                        }
                        first = false;
                        match item {
                            Value::Str(s) => {
                                self.data.push_str(s);
                            }
                            Value::Float(f) => {
                                if f.fract() == 0.0 {
                                    self.data.push_str(&format!("{:.1}", f));
                                } else {
                                    self.data.push_str(&f.to_string());
                                }
                            }
                            Value::Int(i) => {
                                self.data.push_str(&i.to_string());
                            }
                            Value::KeyVal((k, v)) => {
                                self.data.push_str(k);
                                self.data.push('=');
                                match v.as_ref() {
                                    Value::Str(s) => {
                                        self.data.push('"');
                                        self.data.push_str(&escape_xml_attr_value(s));
                                        self.data.push('"');
                                    }
                                    Value::Float(f) => {
                                        if f.fract() == 0.0 {
                                            self.data.push_str(&format!("{:.1}", f));
                                        } else {
                                            self.data.push_str(&f.to_string());
                                        }
                                    }
                                    Value::Int(i) => {
                                        self.data.push_str(&i.to_string());
                                    }
                                    Value::Null => {}
                                    _ => {
                                        return Err(anyhow::anyhow!(
                                            "Unexpected value type in text: item={:?}, {:?}",
                                            item,
                                            arr
                                        ));
                                    }
                                }
                            }
                            Value::Array(_) => {
                                return Err(anyhow::anyhow!(
                                    "Unexpected nested array in text: item={:?}, {:?}",
                                    item,
                                    arr
                                ));
                            }
                            _ => {
                                first = true;
                            }
                        }
                    }
                    self.data.push('>');
                }
                _ => {}
            }
        }
        Ok(self.data)
    }
}

pub struct TextParser<'a> {
    data: Value,
    text: Vec<&'a str>,
    pos: usize,
    len: usize,
}

impl<'a> TextParser<'a> {
    pub fn new(str: &'a str) -> Self {
        let text: Vec<&'a str> = UnicodeSegmentation::graphemes(str, true).collect();
        let len = text.len();
        TextParser {
            data: Value::new_array(),
            text,
            pos: 0,
            len,
        }
    }

    pub fn parse(mut self) -> Result<Value> {
        while let Some(c) = self.peek() {
            match c {
                "<" => {
                    self.parse_array()?;
                }
                _ => {
                    let mut text = String::new();
                    self.eat_char();
                    text.push_str(c);
                    while let Some(b) = self.peek() {
                        if b == "<" {
                            break;
                        }
                        text.push_str(b);
                        self.eat_char();
                    }
                    if !text.is_empty() {
                        self.data.push_member(Value::Str(unescape_xml(&text)));
                    }
                }
            }
        }
        Ok(self.data)
    }

    fn parse_array(&mut self) -> Result<()> {
        let mut arr = Value::new_array();
        self.parse_indent("<")?;
        loop {
            let c = self.peek().ok_or(self.error2("Unexpected eof"))?;
            match c {
                ">" => {
                    self.eat_char();
                    break;
                }
                "-" | "." | "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" => {
                    arr.push_member(self.parse_any_number()?);
                }
                " " => {
                    self.eat_char();
                }
                _ => {
                    let key = self.parse_key()?;
                    let v = if self.is_indent("=") {
                        self.parse_indent("=")?;
                        Value::KeyVal((key, Box::new(self.parse_str()?)))
                    } else {
                        Value::Str(key)
                    };
                    arr.push_member(v);
                }
            }
        }
        self.data.push_member(arr);
        Ok(())
    }

    fn parse_any_number(&mut self) -> Result<Value> {
        self.erase_whitespace();
        let mut number = String::new();
        while let Some(c) = self.peek() {
            if c == "."
                || c == "-"
                || c == "0"
                || c == "1"
                || c == "2"
                || c == "3"
                || c == "4"
                || c == "5"
                || c == "6"
                || c == "7"
                || c == "8"
                || c == "9"
            {
                number.push_str(c);
                self.eat_char();
            } else {
                break;
            }
        }
        if number.contains(".") {
            number
                .parse()
                .map(Value::Float)
                .map_err(|e| self.error2(format!("failed to parse f64: {}", e)))
        } else {
            number
                .parse()
                .map(Value::Int)
                .map_err(|e| self.error2(format!("failed to parse i64: {}", e)))
        }
    }

    fn parse_key(&mut self) -> Result<String> {
        self.erase_whitespace();
        let mut key = String::new();
        while let Some(c) = self.peek() {
            if c == "=" || c == " " || c == ">" {
                break;
            }
            key.push_str(c);
            self.eat_char();
        }
        if key.is_empty() {
            return self.error("Expected key, but found nothing");
        }
        Ok(key)
    }

    fn parse_str(&mut self) -> Result<Value> {
        self.erase_whitespace();
        self.parse_indent("\"")?;
        let mut text = String::new();
        loop {
            match self.next().ok_or(self.error2("Unexpected eof"))? {
                "\"" => {
                    break;
                }
                t => {
                    text.push_str(t);
                }
            }
        }
        Ok(Value::Str(unescape_xml(&text)))
    }

    fn eat_char(&mut self) {
        if self.pos < self.len {
            self.pos += 1;
        }
    }

    fn erase_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c == " " {
                self.eat_char();
            } else {
                break;
            }
        }
    }

    fn is_indent(&self, indent: &str) -> bool {
        let mut pos = self.pos;
        for ident in indent.graphemes(true) {
            if pos >= self.len || self.text[pos] != ident {
                return false;
            }
            pos += 1;
        }
        true
    }

    fn parse_indent(&mut self, indent: &str) -> Result<()> {
        for ident in indent.graphemes(true) {
            match self.next() {
                Some(c) => {
                    if c != ident {
                        return self.error("Unexpected indent");
                    }
                }
                None => return self.error("Unexpected eof"),
            }
        }
        Ok(())
    }

    fn next(&mut self) -> Option<&'a str> {
        if self.pos < self.len {
            let item = self.text[self.pos];
            self.pos += 1;
            Some(item)
        } else {
            None
        }
    }

    fn peek(&self) -> Option<&'a str> {
        if self.pos < self.len {
            Some(self.text[self.pos])
        } else {
            None
        }
    }

    fn error2<T>(&self, msg: T) -> anyhow::Error
    where
        T: std::fmt::Display,
    {
        anyhow::anyhow!("Failed to parse at position {}: {}", self.pos, msg)
    }

    fn error<T, A>(&self, msg: T) -> Result<A>
    where
        T: std::fmt::Display,
    {
        Err(anyhow::anyhow!(
            "Failed to parse at position {}: {}",
            self.pos,
            msg
        ))
    }
}
