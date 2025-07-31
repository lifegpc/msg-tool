use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use crate::utils::escape::*;
use anyhow::Result;
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::ops::Index;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug)]
pub struct ArtemisAsbBuilder {}

impl ArtemisAsbBuilder {
    pub fn new() -> Self {
        ArtemisAsbBuilder {}
    }
}

impl ScriptBuilder for ArtemisAsbBuilder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Utf8
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
        Ok(Box::new(Asb::new(buf, encoding, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["asb"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::ArtemisAsb
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 5 && buf.starts_with(b"ASB\0\0") {
            return Some(20);
        }
        None
    }
}

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

#[derive(Clone, Debug, PartialEq)]
struct Command {
    pub name: String,
    pub line_number: u32,
    pub attributes: BTreeMap<String, String>,
}

impl Command {
    pub fn new(name: String, line_number: u32) -> Self {
        Command {
            name,
            line_number,
            attributes: BTreeMap::new(),
        }
    }

    pub fn to_xml(&self) -> String {
        let mut xml = format!("<{}", self.name);
        for (key, value) in &self.attributes {
            xml.push_str(&format!(" {}=\"{}\"", key, escape_xml_text_value(value)));
        }
        xml.push('>');
        xml
    }
}

impl<'a> Index<&'a str> for Command {
    type Output = str;
    fn index(&self, key: &'a str) -> &Self::Output {
        self.attributes.get(key).map_or("", |s| s.as_str())
    }
}

impl<'a> Index<&'a String> for Command {
    type Output = str;
    fn index(&self, key: &'a String) -> &Self::Output {
        self.attributes.get(key).map_or("", |s| s.as_str())
    }
}

impl Index<String> for Command {
    type Output = str;
    fn index(&self, key: String) -> &Self::Output {
        self.attributes.get(&key).map_or("", |s| s.as_str())
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Item {
    Command(Command),
    Label(String),
}

impl Item {
    pub fn is_command(&self) -> bool {
        matches!(self, Item::Command(_))
    }

    pub fn is_command_name(&self, name: &str) -> bool {
        if let Item::Command(cmd) = self {
            cmd.name == name
        } else {
            false
        }
    }
}

trait CustomReadFn {
    fn read_string(&mut self, encoding: Encoding) -> Result<String>;
    fn read_item(&mut self, encoding: Encoding) -> Result<Item>;
}

impl<T: Read> CustomReadFn for T {
    fn read_string(&mut self, encoding: Encoding) -> Result<String> {
        let len = self.read_u32()?;
        let data = self.read_exact_vec(len as usize)?;
        if self.read_u8()? != 0 {
            return Err(anyhow::anyhow!("String not null-terminated"));
        }
        let s = decode_to_string(encoding, &data, true)?;
        Ok(s)
    }

    fn read_item(&mut self, encoding: Encoding) -> Result<Item> {
        let typ = self.read_u32()?;
        match typ {
            0 => {
                let name = self.read_string(encoding)?;
                let line_number = self.read_u32()?;
                let mut command = Command::new(name, line_number);
                let attr_count = self.read_u32()?;
                for _ in 0..attr_count {
                    let key = self.read_string(encoding)?;
                    let value = self.read_string(encoding)?;
                    command.attributes.insert(key, value);
                }
                Ok(Item::Command(command))
            }
            1 => {
                let label = self.read_string(encoding)?;
                Ok(Item::Label(label))
            }
            _ => {
                return Err(anyhow::anyhow!("Unknown item type: {}", typ));
            }
        }
    }
}

trait CustomWriteFn {
    fn write_string(&mut self, s: &str, encoding: Encoding) -> Result<()>;
    fn write_item(&mut self, item: &Item, encoding: Encoding) -> Result<()>;
}

impl<T: Write> CustomWriteFn for T {
    fn write_string(&mut self, s: &str, encoding: Encoding) -> Result<()> {
        let data = encode_string(encoding, s, false)?;
        self.write_u32(data.len() as u32)?;
        self.write_all(&data)?;
        self.write_u8(0)?; // Null-terminated
        Ok(())
    }

    fn write_item(&mut self, item: &Item, encoding: Encoding) -> Result<()> {
        match item {
            Item::Command(cmd) => {
                self.write_u32(0)?; // Type 0 for Command
                self.write_string(&cmd.name, encoding)?;
                self.write_u32(cmd.line_number)?;
                self.write_u32(cmd.attributes.len() as u32)?;
                for (key, value) in &cmd.attributes {
                    self.write_string(key, encoding)?;
                    self.write_string(value, encoding)?;
                }
            }
            Item::Label(label) => {
                self.write_u32(1)?; // Type 1 for Label
                self.write_string(label, encoding)?;
            }
        }
        Ok(())
    }
}

struct TextParser<'a> {
    items: Vec<Item>,
    text: Vec<&'a str>,
    pos: usize,
    len: usize,
}

impl<'a> TextParser<'a> {
    pub fn new(str: &'a str) -> Self {
        let text: Vec<&'a str> = UnicodeSegmentation::graphemes(str, true).collect();
        let len = text.len();
        TextParser {
            items: Vec::new(),
            text,
            pos: 0,
            len,
        }
    }

    pub fn parse(mut self) -> Result<Vec<Item>> {
        while let Some(c) = self.peek() {
            match c {
                "<" => {
                    self.parse_tag()?;
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
                        self.items.push(Item::Command(Command {
                            name: "print".to_string(),
                            line_number: 0,
                            attributes: [("data".to_string(), unescape_xml(&text))].into(),
                        }))
                    }
                }
            }
        }
        self.items
            .push(Item::Command(Command::new("hcls".to_string(), 0)));
        Ok(self.items)
    }

    fn parse_tag(&mut self) -> Result<()> {
        self.parse_indent("<")?;
        let key = self.parse_key()?;
        self.erase_whitespace();
        let mut cmd = Command::new(key, 0);
        loop {
            let c = self.peek().ok_or(self.error2("Unexpected eof"))?;
            match c {
                ">" => {
                    self.eat_char();
                    break;
                }
                " " => {
                    self.eat_char();
                    continue;
                }
                _ => {
                    let key = self.parse_key()?;
                    self.parse_indent("=")?;
                    let value = self.parse_str()?;
                    cmd.attributes.insert(key, value);
                }
            }
        }
        self.items.push(Item::Command(cmd));
        Ok(())
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

    fn parse_str(&mut self) -> Result<String> {
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
        Ok(unescape_xml(&text))
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

    fn eat_char(&mut self) {
        if self.pos < self.len {
            self.pos += 1;
        }
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

#[derive(Debug)]
pub struct Asb {
    items: Vec<Item>,
}

impl Asb {
    pub fn new(buf: Vec<u8>, encoding: Encoding, _config: &ExtraConfig) -> Result<Self> {
        let mut data = MemReader::new(buf);
        let mut magic = [0; 5];
        data.read_exact(&mut magic)?;
        if &magic != b"ASB\0\0" {
            return Err(anyhow::anyhow!("Invalid ASB magic number: {:?}", magic));
        }
        let nums = data.read_u32()?;
        let mut items = Vec::with_capacity(nums as usize);
        for _ in 0..nums {
            items.push(data.read_item(encoding)?);
        }
        Ok(Asb { items })
    }
}

impl Script for Asb {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Json
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn extract_messages(&self) -> Result<Vec<Message>> {
        let mut messages = Vec::new();
        let mut name = None;
        let mut cur_mes = String::new();
        let mut in_print = false;
        for item in self.items.iter() {
            if in_print {
                if let Item::Command(cmd) = item {
                    match cmd.name.as_str() {
                        "hcls" => {
                            in_print = false;
                            messages.push(Message {
                                name: name.take(),
                                message: cur_mes,
                            });
                            cur_mes = String::new();
                        }
                        "print" => {
                            cur_mes.push_str(&escape_text(&cmd["data"]));
                        }
                        "rt" => {
                            cur_mes.push('\n');
                        }
                        _ => {
                            cur_mes.push_str(&cmd.to_xml());
                        }
                    }
                    continue;
                }
            }
            if let Item::Command(cmd) = item {
                match cmd.name.as_str() {
                    "print" => {
                        cur_mes.push_str(&escape_text(&cmd["data"]));
                        in_print = true;
                    }
                    "name" => {
                        let v = (cmd.attributes.len() - 1).to_string();
                        name = Some(cmd[v].to_owned());
                    }
                    "sel_text" => {
                        let t = &cmd["text"];
                        if !t.is_empty() {
                            messages.push(Message {
                                name: None,
                                message: t.to_owned(),
                            });
                        }
                    }
                    "RegisterTextToHistory" => {
                        let t = &cmd["1"];
                        if !t.is_empty() {
                            messages.push(Message {
                                name: None,
                                message: t.to_owned(),
                            });
                        }
                    }
                    _ => {}
                }
            }
        }
        if !cur_mes.is_empty() {
            messages.push(Message {
                name: name.take(),
                message: cur_mes,
            });
        }
        Ok(messages)
    }

    fn import_messages<'a>(
        &'a self,
        messages: Vec<Message>,
        mut file: Box<dyn WriteSeek + 'a>,
        encoding: Encoding,
        replacement: Option<&'a ReplacementTable>,
    ) -> Result<()> {
        file.write_all(b"ASB\0\0")?;
        let mut items = self.items.clone();
        let mut name_index = None;
        let mut mes_index = 0;
        let mut item_index = 0;
        let mut print_index = None;
        while item_index < items.len() {
            if let Some(print_ind) = print_index.clone() {
                if items[item_index].is_command_name("hcls") {
                    let message = messages
                        .get(mes_index)
                        .ok_or(anyhow::anyhow!("Not enough messages."))?;
                    if let Some(name_index) = name_index.take() {
                        let mut name = match &message.name {
                            Some(name) => name.to_owned(),
                            None => return Err(anyhow::anyhow!("Message without name.")),
                        };
                        if let Some(replacement) = replacement {
                            for (k, v) in &replacement.map {
                                name = name.replace(k, v);
                            }
                        }
                        if let Item::Command(cmd) = &mut items[name_index] {
                            if cmd.attributes.len() > 1 {
                                cmd.attributes
                                    .insert(format!("{}", cmd.attributes.len() - 1), name);
                            } else {
                                let oname = cmd
                                    .attributes
                                    .get("0")
                                    .ok_or(anyhow::anyhow!("No name attribute found."))?;
                                if oname != &name {
                                    cmd.attributes.insert("1".to_string(), name);
                                }
                            }
                        }
                    }
                    let mut m = message.message.clone();
                    if let Some(replacement) = replacement {
                        for (k, v) in &replacement.map {
                            m = m.replace(k, v);
                        }
                    }
                    let new_cmds = TextParser::new(&m.replace("\n", "<rt>")).parse()?;
                    let new_cmds_len = new_cmds.len();
                    items.splice(print_ind..=item_index, new_cmds);
                    print_index = None;
                    item_index = print_ind + new_cmds_len;
                    mes_index += 1;
                    continue;
                } else if items[item_index].is_command() {
                    item_index += 1;
                    continue;
                }
            }
            if let Item::Command(cmd) = &mut items[item_index] {
                match cmd.name.as_str() {
                    "print" => {
                        print_index = Some(item_index);
                    }
                    "name" => {
                        name_index = Some(item_index);
                    }
                    "sel_text" => {
                        let message = messages
                            .get(mes_index)
                            .ok_or(anyhow::anyhow!("Not enough messages."))?;
                        let mut m = message.message.clone();
                        if let Some(replacement) = replacement {
                            for (k, v) in &replacement.map {
                                m = m.replace(k, v);
                            }
                        }
                        cmd.attributes.insert("text".to_string(), m);
                        mes_index += 1;
                    }
                    "RegisterTextToHistory" => {
                        let message = messages
                            .get(mes_index)
                            .ok_or(anyhow::anyhow!("Not enough messages."))?;
                        let mut m = message.message.clone();
                        if let Some(replacement) = replacement {
                            for (k, v) in &replacement.map {
                                m = m.replace(k, v);
                            }
                        }
                        cmd.attributes.insert("1".to_string(), m);
                        mes_index += 1;
                    }
                    _ => {}
                }
            }
            item_index += 1;
        }
        if mes_index != messages.len() {
            return Err(anyhow::anyhow!(
                "Not all messages were processed, expected {}, got {}",
                messages.len(),
                mes_index
            ));
        }
        file.write_u32(items.len() as u32)?;
        for item in items {
            file.write_item(&item, encoding)?;
        }
        file.flush()?;
        Ok(())
    }
}

#[test]
fn test_parse() {
    let text = "Hello &lt; &amp; World!<tag><tags x=\"123\"><name 0=\"Ok\">Test";
    let parser = TextParser::new(text);
    let items = parser.parse().unwrap();
    assert_eq!(
        items,
        vec![
            Item::Command(Command {
                name: "print".to_string(),
                line_number: 0,
                attributes: [("data".to_string(), "Hello < & World!".to_string())].into(),
            }),
            Item::Command(Command {
                name: "tag".to_string(),
                line_number: 0,
                attributes: BTreeMap::new(),
            }),
            Item::Command(Command {
                name: "tags".to_string(),
                line_number: 0,
                attributes: [("x".to_string(), "123".to_string())].into(),
            }),
            Item::Command(Command {
                name: "name".to_string(),
                line_number: 0,
                attributes: [("0".to_string(), "Ok".to_string())].into(),
            }),
            Item::Command(Command {
                name: "print".to_string(),
                line_number: 0,
                attributes: [("data".to_string(), "Test".to_string())].into(),
            }),
            Item::Command(Command {
                name: "hcls".to_string(),
                line_number: 0,
                attributes: BTreeMap::new(),
            }),
        ]
    )
}
