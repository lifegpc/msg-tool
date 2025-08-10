use super::types::*;
use crate::utils::escape::*;
use std::io::Write;

struct LenChecker {
    target_len: usize,
    current_len: usize,
}

impl LenChecker {
    fn new(target_len: usize) -> Self {
        LenChecker {
            target_len,
            current_len: 0,
        }
    }

    fn check(&mut self, value: &Value) -> bool {
        match value {
            Value::Float(f) => {
                if f.fract() == 0.0 {
                    self.current_len += format!("{:.1}", f).len();
                } else {
                    self.current_len += format!("{}", f).len();
                }
            }
            Value::Int(i) => self.current_len += format!("{}", i).len(),
            Value::Str(s) => {
                self.current_len += s.len()
                    + if lua_str_contains_need_escape(s) {
                        4
                    } else {
                        2
                    }
            }
            Value::KeyVal((k, v)) => {
                if let Some(key) = k.as_str() {
                    self.current_len += key.as_bytes().len()
                        + if lua_key_contains_need_escape(key) {
                            7
                        } else {
                            3
                        };
                } else {
                    self.current_len += 1; // for '['
                    if !self.check(k) {
                        return false;
                    }
                    self.current_len += 4; // for '] = '
                }
                if !self.check(v) {
                    return false;
                }
            }
            Value::Array(arr) => {
                self.current_len += 1;
                for v in arr {
                    if !self.check(v) {
                        return false;
                    }
                    self.current_len += 2;
                }
                self.current_len += 1;
            }
            Value::Null => {
                self.current_len += 3; // "nil"
            }
        }
        if self.current_len > self.target_len {
            return false;
        }
        true
    }
}

/// A dumper for Artemis AST scripts.
pub struct Dumper<'a> {
    current_indent: usize,
    writer: Box<dyn Write + 'a>,
    indent: Option<usize>,
    max_line_width: usize,
    current_line_width: usize,
}

impl<'a> Dumper<'a> {
    /// Creates a new Dumper with the specified writer.
    ///
    /// default indent size is 4 spaces, and max line width is 100 characters.
    pub fn new<W: Write + 'a>(writer: W) -> Self {
        Dumper {
            current_indent: 0,
            writer: Box::new(writer),
            indent: Some(4),
            max_line_width: 100,
            current_line_width: 0,
        }
    }

    /// Sets the indent size for the dumper.
    pub fn set_indent(&mut self, indent: usize) {
        self.indent = Some(indent);
    }

    /// Disables indentation for the dumper.
    pub fn set_no_indent(&mut self) {
        self.indent = None;
    }

    /// Sets the maximum line width for the dumper.
    pub fn set_max_line_width(&mut self, max_line_width: usize) {
        self.max_line_width = max_line_width;
    }

    fn dump_f64(&mut self, f: &f64) -> std::io::Result<()> {
        if f.fract() == 0.0 {
            write!(self.writer, "{:.1}", f)
        } else {
            write!(self.writer, "{}", f)
        }
    }

    /// Dumps the AST file to the writer.
    pub fn dump(mut self, ast: &AstFile) -> std::io::Result<()> {
        if self.indent.is_none() {
            if let Some(astver) = ast.astver {
                self.writer.write(b"astver=")?;
                self.dump_f64(&astver)?;
            }
            if let Some(astname) = &ast.astname {
                self.writer.write(b"\nastname = ")?;
                if lua_str_contains_need_escape(astname) {
                    self.writer.write(b"[[")?;
                    self.writer.write(astname.as_bytes())?;
                    self.writer.write(b"]]")?;
                } else {
                    self.writer.write(b"\"")?;
                    self.writer.write(astname.as_bytes())?;
                    self.writer.write(b"\"")?;
                }
            };
            self.writer.write(b"\nast=")?;
            self.dump_value(&ast.ast)?;
        } else {
            if let Some(astver) = ast.astver {
                self.writer.write(b"astver = ")?;
                self.dump_f64(&astver)?;
            }
            if let Some(astname) = &ast.astname {
                self.writer.write(b"\nastname = ")?;
                if lua_str_contains_need_escape(&astname) {
                    self.writer.write(b"[[")?;
                    self.writer.write(astname.as_bytes())?;
                    self.writer.write(b"]]")?;
                } else {
                    self.writer.write(b"\"")?;
                    self.writer.write(astname.as_bytes())?;
                    self.writer.write(b"\"")?;
                }
            };
            self.writer.write(b"\nast = ")?;
            self.current_line_width = 6;
            self.dump_value(&ast.ast)?;
        }
        self.writer.write(b"\n")?;
        Ok(())
    }

    fn dump_value(&mut self, v: &Value) -> std::io::Result<()> {
        if self.indent.is_none() {
            match v {
                Value::Float(f) => self.dump_f64(f)?,
                Value::Int(i) => write!(self.writer, "{}", i)?,
                Value::Str(s) => {
                    if lua_str_contains_need_escape(s) {
                        self.writer.write(b"[[")?;
                        self.writer.write(s.as_bytes())?;
                        self.writer.write(b"]]")?;
                    } else {
                        self.writer.write(b"\"")?;
                        self.writer.write(s.as_bytes())?;
                        self.writer.write(b"\"")?;
                    }
                }
                Value::KeyVal((k, v)) => {
                    if let Some(k) = k.as_str() {
                        if lua_key_contains_need_escape(k) {
                            self.writer.write(b"[\"")?;
                            self.writer.write(k.as_bytes())?;
                            self.writer.write(b"\"]=")?;
                        } else {
                            self.writer.write(k.as_bytes())?;
                            self.writer.write(b"=")?;
                        }
                    } else {
                        self.writer.write(b"[")?;
                        self.dump_value(k)?;
                        self.writer.write(b"]=")?;
                    }
                    self.dump_value(v)?;
                }
                Value::Array(arr) => {
                    self.writer.write(b"{")?;
                    for (i, v) in arr.iter().enumerate() {
                        if i > 0 {
                            self.writer.write(b",")?;
                        }
                        self.dump_value(v)?;
                    }
                    self.writer.write(b"}")?;
                }
                Value::Null => {
                    self.writer.write(b"nil")?;
                }
            }
        } else {
            match v {
                Value::Float(f) => self.dump_f64(f)?,
                Value::Int(i) => write!(self.writer, "{}", i)?,
                Value::Str(s) => {
                    if lua_str_contains_need_escape(s) {
                        self.writer.write(b"[[")?;
                        self.writer.write(s.as_bytes())?;
                        self.writer.write(b"]]")?;
                    } else {
                        self.writer.write(b"\"")?;
                        self.writer.write(s.as_bytes())?;
                        self.writer.write(b"\"")?;
                    }
                }
                Value::KeyVal((k, v)) => {
                    if let Some(k) = k.as_str() {
                        let bytes = k.as_bytes();
                        if lua_key_contains_need_escape(k) {
                            self.writer.write(b"[\"")?;
                            self.writer.write(bytes)?;
                            self.writer.write(b"\"] = ")?;
                            self.current_line_width += bytes.len() + 7;
                        } else {
                            self.writer.write(bytes)?;
                            self.writer.write(b" = ")?;
                            self.current_line_width += bytes.len() + 3;
                        }
                    } else {
                        self.writer.write(b"[")?;
                        self.current_line_width += 1;
                        self.dump_value(k)?;
                        self.writer.write(b"] = ")?;
                        self.current_line_width += 4; // for '] = '
                    };
                    if v.is_array() {
                        let tlen = self.current_line_width + self.current_indent;
                        if tlen < self.max_line_width {
                            let mut checker = LenChecker::new(self.max_line_width - tlen);
                            if checker.check(v) {
                                self.dump_value_in_one(v)?;
                                return Ok(());
                            }
                        }
                    }
                    self.dump_value(v)?;
                }
                Value::Array(a) => {
                    let tlen = self.current_line_width + self.current_indent;
                    if tlen < self.max_line_width {
                        let mut checker = LenChecker::new(self.max_line_width - tlen);
                        if checker.check(v) {
                            self.dump_value_in_one(v)?;
                            return Ok(());
                        }
                    }
                    self.writer.write(b"{\n")?;
                    self.current_indent += self.indent.unwrap();
                    for (i, v) in a.iter().enumerate() {
                        if i > 0 {
                            self.writer.write(b",\n")?;
                        }
                        self.dump_indent()?;
                        self.current_line_width = 0;
                        self.dump_value(v)?;
                    }
                    self.current_indent -= self.indent.unwrap();
                    self.writer.write(b",\n")?;
                    self.dump_indent()?;
                    self.writer.write(b"}")?;
                }
                Value::Null => {
                    self.writer.write(b"nil")?;
                }
            }
        }
        Ok(())
    }

    fn dump_indent(&mut self) -> std::io::Result<()> {
        for _ in 0..self.current_indent {
            self.writer.write(b" ")?;
        }
        Ok(())
    }

    fn dump_value_in_one(&mut self, v: &Value) -> std::io::Result<()> {
        match v {
            Value::Float(f) => self.dump_f64(f)?,
            Value::Int(i) => write!(self.writer, "{}", i)?,
            Value::Str(s) => {
                if lua_str_contains_need_escape(s) {
                    self.writer.write(b"[[")?;
                    self.writer.write(s.as_bytes())?;
                    self.writer.write(b"]]")?;
                } else {
                    self.writer.write(b"\"")?;
                    self.writer.write(s.as_bytes())?;
                    self.writer.write(b"\"")?;
                }
            }
            Value::KeyVal((k, v)) => {
                if let Some(k) = k.as_str() {
                    if lua_key_contains_need_escape(k) {
                        self.writer.write(b"[\"")?;
                        self.writer.write(k.as_bytes())?;
                        self.writer.write(b"\"]=")?;
                    } else {
                        self.writer.write(k.as_bytes())?;
                        self.writer.write(b"=")?;
                    }
                } else {
                    self.writer.write(b"[")?;
                    self.dump_value_in_one(k)?;
                    self.writer.write(b"]=")?;
                };
                self.dump_value_in_one(v)?;
            }
            Value::Array(arr) => {
                self.writer.write(b"{")?;
                for (i, v) in arr.iter().enumerate() {
                    if i > 0 {
                        self.writer.write(b", ")?;
                    }
                    self.dump_value_in_one(v)?;
                }
                self.writer.write(b"}")?;
            }
            Value::Null => {
                self.writer.write(b"nil")?;
            }
        }
        Ok(())
    }
}
