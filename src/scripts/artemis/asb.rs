use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use crate::utils::escape::*;
use anyhow::Result;
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::ops::Index;

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

#[derive(Clone, Debug)]
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

#[derive(Clone, Debug)]
enum Item {
    Command(Command),
    Label(String),
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
                            cur_mes.push_str(&cmd["data"]);
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
                        cur_mes.push_str(&cmd["data"]);
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
        _messages: Vec<Message>,
        mut file: Box<dyn WriteSeek + 'a>,
        encoding: Encoding,
        _replacement: Option<&'a ReplacementTable>,
    ) -> Result<()> {
        file.write_all(b"ASB\0\0")?;
        let items = self.items.clone();
        file.write_u32(items.len() as u32)?;
        for item in items {
            file.write_item(&item, encoding)?;
        }
        file.flush()?;
        Ok(())
    }
}
