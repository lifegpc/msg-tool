//! Qlie Engine Scenario script (.s)
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use anyhow::Result;
use std::io::{Read, Seek, Write};

#[derive(Debug)]
/// Qlie Engine Scenario script builder
pub struct QlieScriptBuilder {}

impl QlieScriptBuilder {
    /// Create a new QlieScriptBuilder
    pub fn new() -> Self {
        Self {}
    }
}

impl ScriptBuilder for QlieScriptBuilder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Cp932
    }

    fn build_script(
        &self,
        buf: Vec<u8>,
        _filename: &str,
        encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(QlieScript::new(
            MemReader::new(buf),
            encoding,
            config,
        )?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["s"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::Qlie
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if is_this_format(buf, buf_len) {
            Some(20)
        } else {
            None
        }
    }
}

/// Check if the buffer is in Qlie script format
pub fn is_this_format(buf: &[u8], buf_len: usize) -> bool {
    if buf_len < 2 {
        return false;
    }
    let mut reader = MemReaderRef::new(&buf[..buf_len]);
    let mut parser = match QlieParser::new(&mut reader, Encoding::Utf8) {
        Ok(p) => p,
        Err(_) => return false,
    };
    loop {
        let line = match parser.next_line() {
            Ok(Some(l)) => l,
            Ok(None) => break,
            Err(_) => return false,
        };
        let line = line.trim();
        if line.to_lowercase() == "@@@avg\\header.s" {
            return true;
        }
    }
    return false;
}

#[derive(Debug, Clone)]
enum TagData {
    Simple(String),
    KeyValue(String, String),
}

#[derive(Debug, Clone)]
struct Tag {
    name: String,
    args: Vec<TagData>,
}

impl Tag {
    fn from_str(s: &str) -> Result<Self> {
        let mut current = String::new();
        let mut name = None;
        let mut arg_key = None;
        let mut args = Vec::new();
        let mut in_quote = false;
        for c in s.chars() {
            if !in_quote && c == ':' {
                if name.is_none() {
                    return Err(anyhow::anyhow!("Invalid tag name: {}", s));
                }
                arg_key = Some(current.to_string());
                current.clear();
                continue;
            }
            if !in_quote && c == ',' {
                if let Some(key) = arg_key.take() {
                    args.push(TagData::KeyValue(key, current.to_string()));
                } else if !current.is_empty() {
                    if name.is_none() {
                        name = Some(current.to_string());
                    } else {
                        args.push(TagData::Simple(current.to_string()));
                    }
                }
                current.clear();
                continue;
            }
            if c == '"' {
                in_quote = !in_quote;
                continue;
            }
            current.push(c);
        }
        if !current.is_empty() {
            if let Some(key) = arg_key.take() {
                args.push(TagData::KeyValue(key, current.to_string()));
            } else {
                if name.is_none() {
                    name = Some(current.to_string());
                } else {
                    args.push(TagData::Simple(current.to_string()));
                }
            }
        }
        Ok(Self {
            name: name.ok_or(anyhow::anyhow!("Invalid tag name"))?,
            args,
        })
    }

    fn dump(&self) -> String {
        let mut parts = Vec::new();
        parts.push(self.name.clone());
        for arg in &self.args {
            match arg {
                TagData::Simple(s) => {
                    if s.contains(',') || s.contains(':') {
                        parts.push(format!("\"{}\"", s));
                    } else {
                        parts.push(s.clone());
                    }
                }
                TagData::KeyValue(k, v) => {
                    let v_str = if v.contains(',') || v.contains(':') {
                        format!("\"{}\"", v)
                    } else {
                        v.clone()
                    };
                    parts.push(format!("{}:{}", k, v_str));
                }
            }
        }
        parts.join(",")
    }
}

#[derive(Debug, Clone)]
enum QlieParsedLine {
    /// `@@label`
    Label(String),
    /// `@@@path`
    Include(String),
    /// `^tag,attr,...`
    LineTag(Tag),
    /// `\command,args,...`
    Command(Tag),
    /// `【name】`
    Name(String),
    /// `％sound`
    Sound(String),
    /// Normal text line
    Text(String),
    /// Empty line
    Empty,
}

struct QlieParser<T> {
    reader: T,
    encoding: Encoding,
    bom: BomType,
    parsed: Vec<QlieParsedLine>,
    is_crlf: bool,
}

impl<T: Read + Seek> QlieParser<T> {
    pub fn new(mut reader: T, mut encoding: Encoding) -> Result<Self> {
        let mut bom = [0; 3];
        let valid_len = reader.peek(&mut bom)?;
        let bom = if valid_len >= 2 {
            if bom[0] == 0xFF && bom[1] == 0xFE {
                BomType::Utf16LE
            } else if bom[0] == 0xFE && bom[1] == 0xFF {
                BomType::Utf16BE
            } else if valid_len >= 3 && bom[0] == 0xEF && bom[1] == 0xBB && bom[2] == 0xBF {
                BomType::Utf8
            } else {
                BomType::None
            }
        } else {
            BomType::None
        };
        match bom {
            BomType::Utf16LE => {
                encoding = Encoding::Utf16LE;
                reader.seek_relative(2)?;
            }
            BomType::Utf16BE => {
                encoding = Encoding::Utf16BE;
                reader.seek_relative(2)?;
            }
            BomType::Utf8 => {
                encoding = Encoding::Utf8;
                reader.seek_relative(3)?;
            }
            BomType::None => {}
        }
        Ok(Self {
            reader,
            encoding,
            bom,
            parsed: Vec::new(),
            is_crlf: false,
        })
    }

    fn next_line(&mut self) -> Result<Option<String>> {
        let mut sbuf = Vec::new();
        let mut is_eof = false;
        if self.encoding.is_utf16le() {
            let mut buf = [0; 2];
            loop {
                let readed = self.reader.read(&mut buf)?;
                if readed == 0 {
                    is_eof = true;
                    break;
                }
                if buf == [0x0A, 0x00] {
                    break;
                }
                sbuf.extend_from_slice(&buf);
            }
        } else if self.encoding.is_utf16be() {
            let mut buf = [0; 2];
            loop {
                let readed = self.reader.read(&mut buf)?;
                if readed == 0 {
                    is_eof = true;
                    break;
                }
                if buf == [0x00, 0x0A] {
                    break;
                }
                sbuf.extend_from_slice(&buf);
            }
        } else {
            let mut buf = [0; 1];
            loop {
                let readed = self.reader.read(&mut buf)?;
                if readed == 0 {
                    is_eof = true;
                    break;
                }
                if buf[0] == 0x0A {
                    break;
                }
                sbuf.push(buf[0]);
            }
        }
        if sbuf.is_empty() {
            return Ok(if is_eof { None } else { Some(String::new()) });
        }
        let mut s = decode_to_string(self.encoding, &sbuf, true)?;
        if s.ends_with("\r") {
            s.pop();
            self.is_crlf = true;
        }
        Ok(Some(s))
    }

    pub fn parse(&mut self) -> Result<()> {
        while let Some(line) = self.next_line()? {
            let line = line.trim();
            if line.is_empty() {
                self.parsed.push(QlieParsedLine::Empty);
            } else if line.starts_with("@@@") {
                self.parsed
                    .push(QlieParsedLine::Include(line[3..].to_string()));
            } else if line.starts_with("@@") {
                self.parsed
                    .push(QlieParsedLine::Label(line[2..].to_string()));
            } else if line.starts_with("^") {
                let tag = Tag::from_str(&line[1..])?;
                self.parsed.push(QlieParsedLine::LineTag(tag));
            } else if line.starts_with("\\") {
                let tag = Tag::from_str(&line[1..])?;
                self.parsed.push(QlieParsedLine::Command(tag));
            } else if line.starts_with("【") && line.ends_with("】") {
                let name = line[3..line.len() - 3].to_string();
                self.parsed.push(QlieParsedLine::Name(name));
            } else if line.starts_with("％") {
                let sound = line[3..].to_string();
                self.parsed.push(QlieParsedLine::Sound(sound));
            } else {
                self.parsed.push(QlieParsedLine::Text(line.to_string()));
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
struct QlieDumper<T: Write> {
    writer: T,
    encoding: Encoding,
    is_crlf: bool,
}

impl<T: Write> QlieDumper<T> {
    pub fn new(mut writer: T, bom: BomType, mut encoding: Encoding, is_crlf: bool) -> Result<Self> {
        match bom {
            BomType::Utf16LE => {
                encoding = Encoding::Utf16LE;
            }
            BomType::Utf16BE => {
                encoding = Encoding::Utf16BE;
            }
            BomType::Utf8 => {
                encoding = Encoding::Utf8;
            }
            BomType::None => {}
        }
        writer.write_all(bom.as_bytes())?;
        Ok(Self {
            writer,
            encoding,
            is_crlf,
        })
    }

    fn write_line(&mut self, line: &str) -> Result<()> {
        let line = if self.is_crlf {
            format!("{}\r\n", line)
        } else {
            format!("{}\n", line)
        };
        let data = encode_string(self.encoding, &line, false)?;
        self.writer.write_all(&data)?;
        Ok(())
    }

    pub fn dump(mut self, data: &[QlieParsedLine]) -> Result<()> {
        for line in data {
            match line {
                QlieParsedLine::Label(s) => {
                    self.write_line(&format!("@@{}", s))?;
                }
                QlieParsedLine::Include(s) => {
                    self.write_line(&format!("@@@{}", s))?;
                }
                QlieParsedLine::LineTag(tag) => {
                    self.write_line(&format!("^{}", tag.dump()))?;
                }
                QlieParsedLine::Command(cmd) => {
                    self.write_line(&format!("\\{}", cmd.dump()))?;
                }
                QlieParsedLine::Name(name) => {
                    self.write_line(&format!("【{}】", name))?;
                }
                QlieParsedLine::Sound(sound) => {
                    self.write_line(&format!("％{}", sound))?;
                }
                QlieParsedLine::Text(text) => {
                    self.write_line(text)?;
                }
                QlieParsedLine::Empty => {
                    self.write_line("")?;
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct QlieScript {
    bom: BomType,
    parsed: Vec<QlieParsedLine>,
    is_crlf: bool,
}

impl QlieScript {
    /// Create a new QlieScript
    pub fn new<T: Read + Seek>(data: T, encoding: Encoding, _config: &ExtraConfig) -> Result<Self> {
        let mut parser = QlieParser::new(data, encoding)?;
        parser.parse()?;
        Ok(Self {
            bom: parser.bom,
            parsed: parser.parsed,
            is_crlf: parser.is_crlf,
        })
    }
}

impl Script for QlieScript {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Json
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn extract_messages(&self) -> Result<Vec<Message>> {
        let mut messages = Vec::new();
        let mut name = None;
        for line in &self.parsed {
            match line {
                QlieParsedLine::Name(n) => {
                    name = Some(n.to_string());
                }
                QlieParsedLine::Text(text) => {
                    messages.push(Message::new(text.replace("[n]", "\n"), name.take()));
                }
                QlieParsedLine::LineTag(tag) => {
                    if tag.name.to_lowercase() == "select" {
                        for arg in &tag.args {
                            match arg {
                                TagData::Simple(s) => {
                                    messages.push(Message::new(s.clone(), None));
                                }
                                _ => {
                                    return Err(anyhow::anyhow!(
                                        "Invalid select tag argument: {:?}.",
                                        tag
                                    ));
                                }
                            }
                        }
                    } else if tag.name.to_lowercase() == "savetext" {
                        if tag.args.len() >= 1 {
                            match &tag.args[0] {
                                TagData::Simple(s) => {
                                    messages.push(Message::new(s.clone(), None));
                                }
                                _ => {
                                    return Err(anyhow::anyhow!(
                                        "Invalid savetext tag argument: {:?}.",
                                        tag
                                    ));
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(messages)
    }

    fn import_messages<'a>(
        &'a self,
        messages: Vec<Message>,
        file: Box<dyn WriteSeek + 'a>,
        _filename: &str,
        encoding: Encoding,
        replacement: Option<&'a ReplacementTable>,
    ) -> Result<()> {
        let mut mess = messages.iter();
        let mut mes = mess.next();
        let mut lines = self.parsed.clone();
        let mut name_index = None;
        let mut index = 0;
        let line_len = lines.len();
        while index < line_len {
            let line = lines[index].clone();
            match line {
                QlieParsedLine::Name(_) => {
                    name_index = Some(index);
                }
                QlieParsedLine::LineTag(tag) => {
                    if tag.name.to_lowercase() == "select" {
                        let mut new_tag = Tag {
                            name: tag.name.clone(),
                            args: Vec::new(),
                        };
                        for _ in &tag.args {
                            let mut message = match mes {
                                Some(m) => m.message.clone(),
                                None => {
                                    return Err(anyhow::anyhow!("Not enough messages to import."));
                                }
                            };
                            mes = mess.next();
                            if let Some(repl) = replacement {
                                for (k, v) in &repl.map {
                                    message = message.replace(k, v);
                                }
                            }
                            new_tag.args.push(TagData::Simple(message));
                        }
                        lines[index] = QlieParsedLine::LineTag(new_tag);
                    } else if tag.name.to_lowercase() == "savetext" {
                        if tag.args.len() >= 1 {
                            let mut message = match mes {
                                Some(m) => m.message.clone(),
                                None => {
                                    return Err(anyhow::anyhow!("Not enough messages to import."));
                                }
                            };
                            mes = mess.next();
                            if let Some(repl) = replacement {
                                for (k, v) in &repl.map {
                                    message = message.replace(k, v);
                                }
                            }
                            let new_tag = Tag {
                                name: tag.name.clone(),
                                args: vec![TagData::Simple(message)],
                            };
                            lines[index] = QlieParsedLine::LineTag(new_tag);
                        }
                    }
                }
                QlieParsedLine::Text(_) => {
                    if let Some(name_index) = name_index.take() {
                        let mut name = match mes {
                            Some(m) => match &m.name {
                                Some(n) => n.clone(),
                                None => return Err(anyhow::anyhow!("Expected name for message.")),
                            },
                            None => return Err(anyhow::anyhow!("Not enough messages to import.")),
                        };
                        if let Some(repl) = replacement {
                            for (k, v) in &repl.map {
                                name = name.replace(k, v);
                            }
                        }
                        lines[name_index] = QlieParsedLine::Name(name);
                    }
                    let mut message = match mes {
                        Some(m) => m.message.clone(),
                        None => return Err(anyhow::anyhow!("Not enough messages to import.")),
                    };
                    mes = mess.next();
                    if let Some(repl) = replacement {
                        for (k, v) in &repl.map {
                            message = message.replace(k, v);
                        }
                    }
                    lines[index] = QlieParsedLine::Text(message.replace("\n", "[n]"));
                }
                _ => {}
            }
            index += 1;
        }
        let dumper = QlieDumper::new(file, self.bom, encoding, self.is_crlf)?;
        dumper.dump(&lines)?;
        Ok(())
    }
}

#[test]
fn test_tag() {
    let s = "tag1,\"test:a,c\",best:\"va,2:3\"";
    let parts = Tag::from_str(s).unwrap();
    assert_eq!(parts.name, "tag1");
    assert_eq!(parts.args.len(), 2);
    match &parts.args[0] {
        TagData::Simple(v) => assert_eq!(v, "test:a,c"),
        _ => panic!("Expected Simple"),
    }
    match &parts.args[1] {
        TagData::KeyValue(k, v) => {
            assert_eq!(k, "best");
            assert_eq!(v, "va,2:3");
        }
        _ => panic!("Expected KeyValue"),
    }
    let dumped = parts.dump();
    assert_eq!(dumped, s);
}
