//! Artemis Engine ASB file (.asb/.iet)
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use crate::utils::escape::*;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::io::{Read, Write};
use std::ops::Index;
use std::sync::Arc;
use stylua_lib::{Config as LuaFormatterConfig, OutputVerification, format_code};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug)]
/// The builder for Artemis ASB scripts.
pub struct ArtemisAsbBuilder {}

impl ArtemisAsbBuilder {
    /// Creates a new instance of `ArtemisAsbBuilder`.
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
        filename: &str,
        encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script + Send + Sync>> {
        Ok(Box::new(Asb::new(buf, encoding, config, filename)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["asb", "iet"]
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

    fn can_create_file(&self) -> bool {
        true
    }

    fn create_file<'a>(
        &'a self,
        filename: &'a str,
        writer: Box<dyn WriteSeek + 'a>,
        encoding: Encoding,
        file_encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<()> {
        create_file(
            filename,
            writer,
            encoding,
            file_encoding,
            config.custom_yaml,
        )
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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

    fn control_index(&self) -> Option<usize> {
        self.attributes.get("\x0bindex")?.parse().ok()
    }

    fn is_internal(&self) -> bool {
        self.name.starts_with('\x0b')
    }

    fn is_internal_goto(&self) -> bool {
        self.name == "\x0bgoto"
    }

    fn to_source(&self, indent: usize) -> String {
        self.to_source_as(&self.name, indent)
    }

    fn to_source_as(&self, name: &str, indent: usize) -> String {
        let mut source = "\t".repeat(indent);
        source.push('[');
        source.push_str(name);
        for (key, value) in &self.attributes {
            if !key.starts_with('\x0b') {
                source.push(' ');
                source.push_str(key);
                source.push_str("=\"");
                // Artemis tag attributes are script expressions, not XML.
                // Escaping here changes operators and string literals, e.g.
                // `$foo + 'bar'` must remain an expression.
                source.push_str(value);
                source.push('"');
            }
        }
        source.push(']');
        source
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
enum Item {
    Command(Command),
    Label(String),
}

impl Item {
    pub fn is_command(&self) -> bool {
        matches!(self, Item::Command(_))
    }

    pub fn command_name_in(&self, names: &HashSet<String>) -> Option<String> {
        if let Item::Command(cmd) = self {
            if names.contains(&cmd.name) {
                return Some(cmd.name.clone());
            }
        }
        None
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
    hcls_index: usize,
    end_tag: &'a str,
}

impl<'a> TextParser<'a> {
    pub fn new(str: &'a str, hcls_index: usize, end_tag: &'a str) -> Self {
        let text: Vec<&'a str> = UnicodeSegmentation::graphemes(str, true).collect();
        let len = text.len();
        TextParser {
            items: Vec::new(),
            text,
            pos: 0,
            len,
            hcls_index,
            end_tag,
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
        let mut hcls = Command::new(self.end_tag.to_string(), 0);
        if self.end_tag == "hcls" {
            hcls.attributes
                .insert("0".to_string(), self.hcls_index.to_string());
        }
        self.items.push(Item::Command(hcls));
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
        if self.items.is_empty() {
            self.items
                .push(Item::Command(Command::new("msgon".to_string(), 0)));
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
/// The Artemis ASB script.
pub struct Asb {
    items: Vec<Item>,
    custom_yaml: bool,
    is_iet: bool,
    format_lua: bool,
    decompile: bool,
    end_tags: Arc<HashSet<String>>,
}

impl Asb {
    /// Creates a new Artemis ASB script from the given buffer.
    ///
    /// * `buf` - The buffer containing the ASB data.
    /// * `encoding` - The encoding used for the ASB data.
    /// * `config` - Extra configuration options.
    pub fn new(
        buf: Vec<u8>,
        encoding: Encoding,
        config: &ExtraConfig,
        filename: &str,
    ) -> Result<Self> {
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
        Ok(Asb {
            items,
            custom_yaml: config.custom_yaml,
            is_iet: std::path::Path::new(filename)
                .extension()
                .map_or(false, |ext| ext.eq_ignore_ascii_case("iet")),
            format_lua: config.artemis_asb_format_lua,
            decompile: config.artemis_asb_decompile,
            end_tags: config.artemis_asb_end_tags.clone(),
        })
    }

    fn to_string(&self, items: &[Item]) -> Result<String> {
        if self.custom_yaml {
            Ok(serde_yaml_ng::to_string(items)?)
        } else {
            Ok(serde_json::to_string_pretty(items)?)
        }
    }

    fn format_lua(&self, script: &str) -> Result<String> {
        let mut config = LuaFormatterConfig::new();
        config.indent_type = stylua_lib::IndentType::Spaces;
        config.indent_width = 2;
        config.column_width = 120;
        config.line_endings = stylua_lib::LineEndings::Unix;
        Ok(format_code(script, config, None, OutputVerification::None)?)
    }

    fn decompile(&self) -> String {
        let items = if self.format_lua {
            self.items
                .iter()
                .map(|item| {
                    if let Item::Command(command) = item
                        && command.name == "lua"
                        && let Some(script) = command.attributes.get("script")
                    {
                        let mut command = command.clone();
                        command.attributes.insert(
                            "script".to_string(),
                            match self.format_lua(script) {
                                Ok(script) => script,
                                Err(_) => {
                                    eprintln!("Warning: Failed to format Lua script.");
                                    crate::COUNTER.inc_warning();
                                    script.clone()
                                }
                            },
                        );
                        return Item::Command(command);
                    }
                    item.clone()
                })
                .collect::<Vec<_>>()
        } else {
            self.items.clone()
        };
        Self::decompile_items(&items)
    }

    fn decompile_items(items: &[Item]) -> String {
        let mut source = String::new();
        let mut start = 0;
        for (index, item) in items.iter().enumerate() {
            if let Item::Label(label) = item {
                Self::decompile_range(&items[start..index], 0, index - start, 0, &mut source);
                source.push('*');
                source.push_str(label);
                source.push('\n');
                start = index + 1;
            }
        }
        Self::decompile_range(&items[start..], 0, items.len() - start, 0, &mut source);
        source
    }

    fn command_at(items: &[Item], index: usize) -> Option<&Command> {
        match items.get(index) {
            Some(Item::Command(command)) => Some(command),
            _ => None,
        }
    }

    fn goto_before(items: &[Item], index: usize) -> Option<usize> {
        Self::command_at(items, index.checked_sub(1)?)
            .filter(|command| command.is_internal_goto())?
            .control_index()
            .filter(|target| *target <= items.len())
    }

    fn has_local_control_target(items: &[Item], index: usize, end: usize, target: usize) -> bool {
        index < target
            && target < end
            && Self::command_at(items, target).is_some_and(Command::is_internal_goto)
    }

    /// Finds an end for control flow whose virtual address cannot be mapped to
    /// an item offset. This happens when a script has multiple labels: Artemis
    /// keeps the compiler's file-wide virtual addresses, while `items` is a
    /// label-local slice here.
    fn implicit_control_end(
        items: &[Item],
        start: usize,
        end: usize,
        control_end: Option<usize>,
    ) -> usize {
        let mut has_command = false;
        let mut index = start;
        while index < end {
            let Some(command) = Self::command_at(items, index) else {
                index += 1;
                continue;
            };
            if command.is_internal_goto() {
                return index;
            }
            if matches!(command.name.as_str(), "if" | "elseif" | "loop") {
                let nested_end = command.control_index();
                let is_nested = control_end
                    .zip(nested_end)
                    .is_some_and(|(control_end, nested_end)| nested_end < control_end);
                if !is_nested && has_command {
                    return index;
                }
                if !is_nested {
                    return index;
                }

                // A smaller virtual end belongs to a nested control. Skip its
                // first branch while looking for the enclosing control's end.
                let next = Self::implicit_control_end(items, index + 1, end, nested_end);
                index = next.max(index + 1);
                if Self::command_at(items, index).is_some_and(Command::is_internal_goto) {
                    index += 1;
                }
                continue;
            }
            if matches!(command.name.as_str(), "return" | "exit" | "jump" | "stop") {
                return index + 1;
            }
            has_command = true;
            index += 1;
        }
        end
    }

    fn terminal_before_nested_control(items: &[Item], start: usize, end: usize) -> Option<usize> {
        for index in start..end {
            let command = Self::command_at(items, index)?;
            if matches!(command.name.as_str(), "if" | "elseif" | "loop") {
                return None;
            }
            if matches!(command.name.as_str(), "return" | "exit" | "jump" | "stop") {
                return Some(index + 1);
            }
        }
        None
    }

    fn decompile_range(
        items: &[Item],
        mut index: usize,
        end: usize,
        indent: usize,
        source: &mut String,
    ) {
        while index < end {
            let Some(command) = Self::command_at(items, index) else {
                index += 1;
                continue;
            };
            if command.is_internal() {
                index += 1;
                continue;
            }
            if command.name == "lua" {
                source.push_str(&"\t".repeat(indent));
                source.push_str("[lua]\n");
                if let Some(script) = command.attributes.get("script") {
                    source.push_str(script);
                    if !script.ends_with('\n') {
                        source.push('\n');
                    }
                }
                source.push_str(&"\t".repeat(indent));
                source.push_str("[/lua]\n");
                index += 1;
                continue;
            }
            let control_end = command.control_index();
            if matches!(command.name.as_str(), "if" | "elseif")
                && (control_end
                    .is_none_or(|control_end| control_end <= index || control_end >= end)
                    || !control_end.is_some_and(|control_end| {
                        Self::has_local_control_target(items, index, end, control_end)
                    }))
            {
                let branch_end = Self::implicit_control_end(items, index + 1, end, control_end);
                source.push_str(&command.to_source_as(
                    if command.name == "elseif" {
                        "if"
                    } else {
                        &command.name
                    },
                    indent,
                ));
                source.push('\n');
                Self::decompile_range(items, index + 1, branch_end, indent + 1, source);
                let mut next = branch_end;
                while let Some(elseif) = Self::command_at(items, next)
                    && elseif.name == "elseif"
                    && elseif.control_index() == control_end
                {
                    let next_end = Self::implicit_control_end(items, next + 1, end, control_end);
                    source.push_str(&elseif.to_source(indent));
                    source.push('\n');
                    Self::decompile_range(items, next + 1, next_end, indent + 1, source);
                    next = next_end;
                }
                source.push_str(&"\t".repeat(indent));
                source.push_str("[/if]\n");
                index = next;
                continue;
            }
            if matches!(command.name.as_str(), "if" | "elseif")
                && let Some(branch_end) = control_end
                && index < branch_end
                && branch_end <= end
                && Self::has_local_control_target(items, index, end, branch_end)
            {
                let branch_end = Self::terminal_before_nested_control(items, index + 1, branch_end)
                    .unwrap_or(branch_end);
                let mut flow_end = Self::goto_before(items, branch_end);
                let mut next_branch = branch_end;
                while flow_end.is_none() && next_branch < end {
                    let Some(next) = Self::command_at(items, next_branch) else {
                        break;
                    };
                    if next.name != "elseif" {
                        break;
                    }
                    let Some(next_end) = next.control_index() else {
                        break;
                    };
                    if next_branch >= next_end || next_end > end {
                        break;
                    }
                    flow_end = Self::goto_before(items, next_end);
                    next_branch = next_end;
                }
                if let Some(flow_end) =
                    flow_end.filter(|flow_end| *flow_end >= branch_end && *flow_end <= end)
                {
                    source.push_str(&command.to_source(indent));
                    source.push('\n');
                    Self::decompile_range(
                        items,
                        index + 1,
                        branch_end.saturating_sub(1),
                        indent + 1,
                        source,
                    );

                    let mut branch = branch_end;
                    while branch < flow_end {
                        if let Some(elseif) = Self::command_at(items, branch)
                            && elseif.name == "elseif"
                            && let Some(next_end) = elseif.control_index()
                            && branch < next_end
                            && next_end <= flow_end
                        {
                            source.push_str(&elseif.to_source(indent));
                            source.push('\n');
                            Self::decompile_range(
                                items,
                                branch + 1,
                                next_end.saturating_sub(1),
                                indent + 1,
                                source,
                            );
                            branch = next_end;
                        } else {
                            source.push_str(&"\t".repeat(indent));
                            source.push_str("[else]\n");
                            Self::decompile_range(items, branch, flow_end, indent + 1, source);
                            branch = flow_end;
                        }
                    }
                    source.push_str(&"\t".repeat(indent));
                    source.push_str("[/if]\n");
                    index = flow_end;
                    continue;
                }

                source.push_str(&command.to_source(indent));
                source.push('\n');
                Self::decompile_range(items, index + 1, branch_end, indent + 1, source);
                source.push_str(&"\t".repeat(indent));
                source.push_str("[/if]\n");
                index = branch_end;
                continue;
            }
            if command.name == "loop"
                && let Some(loop_end) = control_end
                && index < loop_end
                && loop_end <= end
            {
                source.push_str(&command.to_source(indent));
                source.push('\n');
                Self::decompile_range(items, index + 1, loop_end, indent + 1, source);
                source.push_str(&"\t".repeat(indent));
                source.push_str("[/loop]\n");
                index = loop_end;
                continue;
            }
            source.push_str(&command.to_source(indent));
            source.push('\n');
            index += 1;
        }
    }
}

impl Script for Asb {
    fn default_output_script_type(&self) -> OutputScriptType {
        if self.is_iet {
            OutputScriptType::Custom
        } else {
            OutputScriptType::Json
        }
    }

    fn is_output_supported(&self, out: OutputScriptType) -> bool {
        if self.is_iet {
            matches!(out, OutputScriptType::Custom)
        } else {
            true
        }
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn custom_output_extension<'a>(&'a self) -> &'a str {
        if self.decompile {
            "txt"
        } else if self.custom_yaml {
            "yaml"
        } else {
            "json"
        }
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
                        "print" => {
                            cur_mes.push_str(&escape_text(&cmd["data"]));
                        }
                        "rt" => {
                            cur_mes.push('\n');
                        }
                        _ => {
                            if self.end_tags.contains(&cmd.name) {
                                in_print = false;
                                messages.push(Message {
                                    name: name.take(),
                                    message: cur_mes,
                                });
                                cur_mes = String::new();
                                continue;
                            }
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
                    "msgon" => {
                        in_print = true;
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
        _filename: &str,
        encoding: Encoding,
        replacement: Option<&'a ReplacementTable>,
    ) -> Result<()> {
        file.write_all(b"ASB\0\0")?;
        let mut items = self.items.clone();
        let mut name_index = None;
        let mut mes_index = 0;
        let mut item_index = 0;
        let mut print_index = None;
        let mut hcls_index = 1;
        while item_index < items.len() {
            if let Some(print_ind) = print_index.clone() {
                if let Some(end_tag) = items[item_index].command_name_in(&self.end_tags) {
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
                    let new_cmds =
                        TextParser::new(&m.replace("\n", "<rt>"), hcls_index, &end_tag).parse()?;
                    hcls_index += 1;
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
                    "msgon" => {
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

    fn custom_export(&self, filename: &std::path::Path, encoding: Encoding) -> Result<()> {
        let s = if self.decompile {
            self.decompile()
        } else if self.format_lua {
            let items: Vec<_> = self
                .items
                .iter()
                .map(|s| {
                    if let Item::Command(cmd) = s {
                        if cmd.name == "lua" {
                            if let Some(script) = cmd.attributes.get("script") {
                                let mut cmd = cmd.clone();
                                cmd.attributes.insert(
                                    "script".to_string(),
                                    match self.format_lua(script) {
                                        Ok(s) => s,
                                        Err(_) => {
                                            eprintln!("Warning: Failed to format Lua script.");
                                            crate::COUNTER.inc_warning();
                                            script.clone()
                                        }
                                    },
                                );
                                return Item::Command(cmd);
                            }
                        }
                    }
                    s.clone()
                })
                .collect();
            self.to_string(&items)?
        } else {
            self.to_string(&self.items)?
        };
        let s = encode_string(encoding, &s, false)?;
        let mut file = std::fs::File::create(filename)?;
        file.write_all(&s)?;
        Ok(())
    }

    fn custom_import<'a>(
        &'a self,
        custom_filename: &'a str,
        file: Box<dyn WriteSeek + 'a>,
        encoding: Encoding,
        output_encoding: Encoding,
    ) -> Result<()> {
        if self.decompile {
            return Err(anyhow::anyhow!(
                "Importing Artemis ASB decompiled text is not supported."
            ));
        }
        create_file(
            custom_filename,
            file,
            encoding,
            output_encoding,
            self.custom_yaml,
        )
    }
}

/// Creates a new ASB file.
///
/// * `custom_filename` - The path ot the input file.
/// * `writer` - The writer to write the ASB script.
/// * `encoding` - The encoding used for the ASB script.
/// * `output_encoding` - The encoding used for the input file.
/// * `yaml` - Whether to use YAML format instead of JSON for the input file.
pub fn create_file<'a>(
    custom_filename: &'a str,
    mut writer: Box<dyn WriteSeek + 'a>,
    encoding: Encoding,
    output_encoding: Encoding,
    yaml: bool,
) -> Result<()> {
    let f = crate::utils::files::read_file(custom_filename)?;
    let s = decode_to_string(output_encoding, &f, true)?;
    let items: Vec<Item> = if yaml {
        serde_yaml_ng::from_str(&s)?
    } else {
        serde_json::from_str(&s)?
    };
    writer.write_all(b"ASB\0\0")?;
    writer.write_u32(items.len() as u32)?;
    for item in items {
        writer.write_item(&item, encoding)?;
    }
    Ok(())
}

#[test]
fn test_parse() {
    let text = "Hello &lt; &amp; World!<tag><tags x=\"123\"><name 0=\"Ok\">Test";
    let parser = TextParser::new(text, 1, "hcls");
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
                attributes: BTreeMap::from([("0".to_string(), "1".to_string())]),
            }),
        ]
    )
}

#[test]
fn test_parse2() {
    let text = "<ruby text=\"test\">OK</ruby>Hello &lt; &amp; World!<tag><tags x=\"123\"><name 0=\"Ok\">Test";
    let parser = TextParser::new(text, 1, "click");
    let items = parser.parse().unwrap();
    assert_eq!(
        items,
        vec![
            Item::Command(Command {
                name: "msgon".to_string(),
                line_number: 0,
                attributes: BTreeMap::new(),
            }),
            Item::Command(Command {
                name: "ruby".to_string(),
                line_number: 0,
                attributes: [("text".to_string(), "test".to_string())].into(),
            }),
            Item::Command(Command {
                name: "print".to_string(),
                line_number: 0,
                attributes: [("data".to_string(), "OK".to_string())].into(),
            }),
            Item::Command(Command {
                name: "/ruby".to_string(),
                line_number: 0,
                attributes: BTreeMap::new(),
            }),
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
                name: "click".to_string(),
                line_number: 0,
                attributes: BTreeMap::new(),
            }),
        ]
    )
}

#[test]
fn test_decompile_flow_control() {
    let command = |name: &str, attributes: &[(&str, &str)]| {
        Item::Command(Command {
            name: name.to_string(),
            line_number: 0,
            attributes: attributes
                .iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect(),
        })
    };
    let script = Asb {
        items: vec![
            Item::Label("top".to_string()),
            command("if", &[("\x0bindex", "3"), ("estimate", "$a")]),
            command("print", &[("data", "yes")]),
            command("\x0bgoto", &[("\x0bindex", "5")]),
            command("elseif", &[("\x0bindex", "5"), ("estimate", "$b")]),
            command("print", &[("data", "maybe")]),
            command("print", &[("data", "after")]),
            command("lua", &[("script", "function test()\nend")]),
            command("loop", &[("\x0bindex", "9"), ("estimate", "$loop")]),
            command("print", &[("data", "again")]),
            command("\x0bgoto", &[("\x0bindex", "7")]),
        ],
        custom_yaml: false,
        is_iet: true,
        format_lua: true,
        decompile: true,
        end_tags: Arc::new(HashSet::new()),
    };

    let source = script.decompile();
    assert!(!source.contains('\x0b'));
    assert!(source.contains("[if estimate=\"$a\"]"));
    assert!(source.contains("[if estimate=\"$b\"]"));
    assert!(source.contains("[/if]"));
    let formatted_lua = script.format_lua("function test()\nend").unwrap();
    assert!(source.contains("[lua]"));
    assert!(source.contains(&formatted_lua));
    assert!(source.contains("[/lua]"));
    assert!(source.contains("[loop estimate=\"$loop\"]"));
    assert!(source.contains("[/loop]"));
}

#[test]
fn test_decompile_nested_flow_with_file_wide_indices() {
    let command = |name: &str, attributes: &[(&str, &str)]| {
        Item::Command(Command {
            name: name.to_string(),
            line_number: 0,
            attributes: attributes
                .iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect(),
        })
    };
    let script = Asb {
        items: vec![
            Item::Label("nested".to_string()),
            command("if", &[("\x0bindex", "50"), ("estimate", "$outer")]),
            command("if", &[("\x0bindex", "30"), ("estimate", "$inner")]),
            command("print", &[("data", "inside")]),
            command("\x0bgoto", &[("\x0bindex", "31")]),
            command("print", &[("data", "after inner")]),
            command("return", &[]),
            command("print", &[("data", "outside")]),
        ],
        custom_yaml: false,
        is_iet: true,
        format_lua: false,
        decompile: true,
        end_tags: Arc::new(HashSet::new()),
    };

    assert_eq!(
        script.decompile(),
        "*nested\n[if estimate=\"$outer\"]\n\t[if estimate=\"$inner\"]\n\t\t[print data=\"inside\"]\n\t[/if]\n\t[print data=\"after inner\"]\n\t[return]\n[/if]\n[print data=\"outside\"]\n"
    );
}

#[test]
fn test_decompile_attribute_values_are_not_xml_escaped() {
    let command = Command {
        name: "debugprint".to_string(),
        line_number: 0,
        attributes: [("data".to_string(), "$foo + 'bar' & <baz>".to_string())].into(),
    };

    assert_eq!(
        command.to_source(0),
        "[debugprint data=\"$foo + 'bar' & <baz>\"]"
    );
}
