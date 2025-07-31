use crate::ext::fancy_regex::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use crate::utils::escape::*;
use anyhow::Result;
use fancy_regex::Regex;
use std::collections::HashSet;
use std::io::Write;
use std::ops::{Deref, DerefMut, Index, IndexMut};
use std::sync::Arc;

#[derive(Debug)]
pub struct KsBuilder {}

impl KsBuilder {
    pub fn new() -> Self {
        Self {}
    }
}

impl ScriptBuilder for KsBuilder {
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
        Ok(Box::new(KsScript::new(buf, encoding, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ks", "soc"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::Kirikiri
    }
}

trait Node {
    fn serialize(&self) -> String;
}

#[derive(Clone, Debug)]
struct CommentNode(String);

impl Node for CommentNode {
    fn serialize(&self) -> String {
        format!("; {}", self.0)
    }
}

#[derive(Clone, Debug)]
struct LabelNode {
    name: String,
    page: Option<String>,
}

impl Node for LabelNode {
    fn serialize(&self) -> String {
        if let Some(page) = &self.page {
            format!("*{}|{}", self.name, page)
        } else {
            format!("*{}", self.name)
        }
    }
}

#[derive(Clone, Debug)]
struct TextNode(String);

impl Node for TextNode {
    fn serialize(&self) -> String {
        // In KAG, [ is escaped as [[
        self.0.replace("[", "[[")
    }
}

#[derive(Clone, Debug)]
struct EmptyLineNode;

impl Node for EmptyLineNode {
    fn serialize(&self) -> String {
        String::new()
    }
}

#[derive(Clone, Debug)]
enum TagAttr {
    True,
    Str(String),
}

#[derive(Clone, Debug)]
struct TagNode {
    name: String,
    attributes: Vec<(String, TagAttr)>,
}

impl TagNode {
    fn serialize_attributes(&self) -> String {
        let mut parts = Vec::new();
        for (key, value) in self.attributes.iter() {
            match value {
                TagAttr::True => {
                    parts.push(key.clone());
                }
                TagAttr::Str(val) => {
                    if val.contains(" ") || val.contains("=") {
                        parts.push(format!("{}=\"{}\"", key, val));
                    } else {
                        parts.push(format!("{}={}", key, val));
                    }
                }
            }
        }
        parts.join(" ")
    }

    fn ser_attributes_xml(&self) -> String {
        let mut parts = Vec::new();
        for (key, value) in self.attributes.iter() {
            match value {
                TagAttr::True => {
                    parts.push(key.clone());
                }
                TagAttr::Str(val) => {
                    parts.push(format!("{}=\"{}\"", key, escape_xml_attr_value(val)));
                }
            }
        }
        parts.join(" ")
    }

    pub fn set_attr(&mut self, key: &str, value: String) {
        if let Some(attr) = self.attributes.iter_mut().find(|(k, _)| k == key) {
            attr.1 = TagAttr::Str(value);
        } else {
            self.attributes.push((key.to_string(), TagAttr::Str(value)));
        }
    }

    fn to_xml_tag(&self) -> String {
        let attr_str = self.ser_attributes_xml();
        if attr_str.is_empty() {
            format!("<{}>", self.name)
        } else {
            format!("<{} {}>", self.name, attr_str)
        }
    }
}

impl Node for TagNode {
    fn serialize(&self) -> String {
        let attr_str = self.serialize_attributes();
        if attr_str.is_empty() {
            format!("[{}]", self.name)
        } else {
            format!("[{} {}]", self.name, attr_str)
        }
    }
}

#[derive(Clone)]
struct CommandNode {
    inner: TagNode,
}

impl Deref for CommandNode {
    type Target = TagNode;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for CommandNode {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl std::fmt::Debug for CommandNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandNode")
            .field("name", &self.inner.name)
            .field("attributes", &self.inner.attributes)
            .finish()
    }
}

impl Node for CommandNode {
    fn serialize(&self) -> String {
        let attr_str = self.inner.serialize_attributes();
        if attr_str.is_empty() {
            format!("@{}", self.inner.name)
        } else {
            format!("@{} {}", self.inner.name, attr_str)
        }
    }
}

#[derive(Clone, Debug)]
struct ScriptBlockNode(String);

impl Node for ScriptBlockNode {
    fn serialize(&self) -> String {
        format!("[iscript]\n{}\n[endscript]", self.0)
    }
}

#[derive(Clone, Debug)]
enum ParsedLineNode {
    Text(TextNode),
    Tag(TagNode),
}

impl ParsedLineNode {
    fn to_xml(&self) -> String {
        match self {
            ParsedLineNode::Text(text_node) => escape_xml_text_value(&text_node.0),
            ParsedLineNode::Tag(tag_node) => {
                if tag_node.name == "r" && tag_node.attributes.is_empty() {
                    "\n".to_string()
                } else {
                    tag_node.to_xml_tag()
                }
            }
        }
    }

    fn is_np(&self) -> bool {
        matches!(self, ParsedLineNode::Tag(tag) if tag.name == "np")
    }
}

impl Node for ParsedLineNode {
    fn serialize(&self) -> String {
        match self {
            ParsedLineNode::Text(text_node) => text_node.serialize(),
            ParsedLineNode::Tag(tag_node) => tag_node.serialize(),
        }
    }
}

#[derive(Clone, Debug)]
struct ParsedLine(Vec<ParsedLineNode>);

impl ParsedLine {
    fn to_xml(&self) -> String {
        let mut s = String::new();
        for node in &self.0 {
            s.push_str(&node.to_xml());
        }
        s
    }
}

impl Deref for ParsedLine {
    type Target = Vec<ParsedLineNode>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ParsedLine {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Node for ParsedLine {
    fn serialize(&self) -> String {
        self.0
            .iter()
            .map(|node| node.serialize())
            .collect::<Vec<_>>()
            .join("")
    }
}

#[derive(Clone, Debug)]
enum ParsedScriptNode {
    Comment(CommentNode),
    Label(LabelNode),
    Command(CommandNode),
    ScriptBlock(ScriptBlockNode),
    Line(ParsedLine),
    EmptyLine(EmptyLineNode),
}

impl ParsedScriptNode {
    pub fn is_empty(&self) -> bool {
        matches!(self, ParsedScriptNode::EmptyLine(_))
    }

    pub fn set_attr(&mut self, key: &str, value: String) {
        if let ParsedScriptNode::Command(command) = self {
            command.set_attr(key, value);
        }
    }
}

impl Node for ParsedScriptNode {
    fn serialize(&self) -> String {
        match self {
            ParsedScriptNode::Comment(comment) => comment.serialize(),
            ParsedScriptNode::Label(label) => label.serialize(),
            ParsedScriptNode::Command(command) => command.serialize(),
            ParsedScriptNode::ScriptBlock(script_block) => script_block.serialize(),
            ParsedScriptNode::Line(line) => line.serialize(),
            ParsedScriptNode::EmptyLine(empty_line) => empty_line.serialize(),
        }
    }
}

#[derive(Clone, Debug)]
struct ParsedScript(Vec<ParsedScriptNode>);

impl Deref for ParsedScript {
    type Target = Vec<ParsedScriptNode>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ParsedScript {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Index<usize> for ParsedScript {
    type Output = ParsedScriptNode;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl IndexMut<usize> for ParsedScript {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        if index < self.0.len() {
            &mut self.0[index]
        } else {
            self.0.push(ParsedScriptNode::EmptyLine(EmptyLineNode));
            self.0.last_mut().unwrap()
        }
    }
}

impl Node for ParsedScript {
    fn serialize(&self) -> String {
        self.0
            .iter()
            .map(|node| node.serialize())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

lazy_static::lazy_static! {
    static ref LINE_SPLIT_RE: Regex = Regex::new(r"(\[.*?\])").unwrap();
    static ref ATTR_RE: Regex = Regex::new("([a-zA-Z0-9_]+)(?:=(\"[^\"]*\"|'[^']*'|[^\\s\\]]+))?").unwrap();
}

struct Parser {
    lines: Vec<String>,
}

impl Parser {
    pub fn new(script: &str) -> Self {
        let lines = script.lines().map(|s| s.to_string()).collect();
        Self { lines }
    }

    fn parse_attributes(attr_str: &str) -> Result<Vec<(String, TagAttr)>> {
        let mut attributes = Vec::new();
        for cap in ATTR_RE.captures_iter(attr_str) {
            let cap = cap?;
            let key = cap
                .get(1)
                .ok_or(anyhow::anyhow!("Invalid attribute key"))?
                .as_str()
                .to_string();
            let value = cap
                .get(2)
                .map(|v| {
                    let mut s = v.as_str().trim().to_string();
                    if s.starts_with("\"") && s.ends_with("\"") {
                        s = s[1..s.len() - 1].to_string();
                    } else if s.starts_with("'") && s.ends_with("'") {
                        s = s[1..s.len() - 1].to_string();
                    }
                    s = s.replace("`", "");
                    TagAttr::Str(s)
                })
                .unwrap_or(TagAttr::True);
            attributes.push((key, value));
        }
        Ok(attributes)
    }

    fn parse_tag_or_command(content: &str) -> Result<TagNode> {
        let parts = content.trim().split_ascii_whitespace().collect::<Vec<_>>();
        let tag_name = parts[0].to_string();
        let attr_string = parts[1..].join(" ");
        let attrs = Self::parse_attributes(&attr_string)?;
        Ok(TagNode {
            name: tag_name,
            attributes: attrs,
        })
    }

    fn parse(&self, preserve_empty_lines: bool) -> Result<ParsedScript> {
        let mut parsed_scripts = Vec::new();
        let mut in_script_block = false;
        let mut script_buffer = Vec::new();
        let mut i = 0;
        let line_count = self.lines.len();
        while i < line_count {
            let line = self.lines[i].trim();
            i += 1;
            if line.is_empty() {
                if preserve_empty_lines {
                    parsed_scripts.push(ParsedScriptNode::EmptyLine(EmptyLineNode));
                } else {
                    continue;
                }
            }
            if in_script_block {
                if line == "[endscript]" {
                    in_script_block = false;
                    parsed_scripts.push(ParsedScriptNode::ScriptBlock(ScriptBlockNode(
                        script_buffer.join("\n"),
                    )));
                    script_buffer.clear();
                } else {
                    script_buffer.push(line.to_string());
                }
                continue;
            }
            if line == "[iscript]" {
                in_script_block = true;
                continue;
            }
            if line.starts_with(";") {
                parsed_scripts.push(ParsedScriptNode::Comment(CommentNode(
                    line[1..].trim().to_string(),
                )));
                continue;
            }
            if line.starts_with("*") {
                let parts: Vec<&str> = line.split('|').collect();
                let label_name = parts[0][1..].trim().to_string();
                let page = if parts.len() > 1 {
                    Some(parts[1..].join("|"))
                } else {
                    None
                };
                parsed_scripts.push(ParsedScriptNode::Label(LabelNode {
                    name: label_name,
                    page,
                }));
                continue;
            }
            if line.starts_with("@") {
                let content = &line[1..];
                let tag_node = Self::parse_tag_or_command(content)?;
                parsed_scripts.push(ParsedScriptNode::Command(CommandNode { inner: tag_node }));
                continue;
            }
            let mut full_line = line.to_string();
            while full_line.ends_with("\\") {
                full_line.pop(); // Remove the trailing backslash
                full_line = full_line.trim_end().to_string();
                if i < line_count {
                    full_line.push(' ');
                    full_line.push_str(&self.lines[i].trim());
                    i += 1;
                } else {
                    break; // No more lines to append
                }
            }
            let mut parsed_line_nodes = Vec::new();
            for part in LINE_SPLIT_RE.py_split(&full_line)? {
                let part = part.trim();
                if part.is_empty() {
                    continue;
                }
                if part.starts_with("[") && part.ends_with("]") {
                    if part == "[[r]]" {
                        parsed_line_nodes.push(ParsedLineNode::Text(TextNode("[r]".to_string())));
                    } else if part == "[[[[" {
                        parsed_line_nodes.push(ParsedLineNode::Text(TextNode("[[".to_string())));
                    } else if part.starts_with("[[") {
                        parsed_line_nodes
                            .push(ParsedLineNode::Text(TextNode(part[1..].to_string())))
                    } else {
                        parsed_line_nodes.push(ParsedLineNode::Tag(Self::parse_tag_or_command(
                            &part[1..part.len() - 1],
                        )?));
                    }
                } else {
                    parsed_line_nodes.push(ParsedLineNode::Text(TextNode(part.to_string())));
                }
            }
            if !parsed_line_nodes.is_empty() {
                parsed_scripts.push(ParsedScriptNode::Line(ParsedLine(parsed_line_nodes)));
            }
        }
        Ok(ParsedScript(parsed_scripts))
    }
}

struct XMLTextParser {
    str: String,
    pos: usize,
}

impl XMLTextParser {
    pub fn new(text: &str) -> Self {
        Self {
            str: text.replace("\n", "<r>"),
            pos: 0,
        }
    }

    fn parse_tag(&mut self) -> Result<TagNode> {
        let mut name = String::new();
        let mut attributes = Vec::new();
        let mut is_name = true;
        let mut is_key = false;
        let mut is_value = false;
        let mut is_in_quote = false;
        let mut key = String::new();
        let mut value = String::new();
        while let Some(c) = self.next() {
            match c {
                '>' => {
                    if !name.is_empty() {
                        return Ok(TagNode { name, attributes });
                    } else {
                        return Err(anyhow::anyhow!("Empty tag name"));
                    }
                }
                ' ' | '\t' => {
                    if is_name {
                        is_name = false;
                        is_key = true;
                    } else if is_key {
                        if !key.is_empty() {
                            attributes.push((key.clone(), TagAttr::True));
                            key.clear();
                        }
                    } else if is_value {
                        if is_in_quote {
                            value.push(c);
                        } else {
                            if !value.is_empty() {
                                attributes.push((key.clone(), TagAttr::Str(unescape_xml(&value))));
                                key.clear();
                                value.clear();
                            }
                            is_key = true;
                            is_value = false;
                        }
                    }
                }
                '"' => {
                    if is_in_quote {
                        is_in_quote = false;
                        if !value.is_empty() {
                            attributes.push((key.clone(), TagAttr::Str(unescape_xml(&value))));
                            key.clear();
                            value.clear();
                        }
                        is_key = true;
                    } else {
                        is_in_quote = true;
                    }
                }
                '=' => {
                    if is_key {
                        is_key = false;
                        is_value = true;
                    }
                }
                _ => {
                    if is_name {
                        name.push(c);
                    } else if is_key {
                        key.push(c);
                    } else if is_value {
                        value.push(c);
                    } else {
                        return Err(anyhow::anyhow!("Unexpected character in tag: {}", c));
                    }
                }
            }
        }
        Err(anyhow::anyhow!("Unexpected end of input while parsing tag"))
    }

    pub fn parse(mut self) -> Result<Vec<ParsedLine>> {
        let mut lines = Vec::new();
        let mut current_line = Vec::new();
        let mut text = String::new();
        while let Some(c) = self.next() {
            match c {
                '<' => {
                    if !text.is_empty() {
                        current_line.push(ParsedLineNode::Text(TextNode(unescape_xml(&text))));
                        text.clear();
                    }
                    let tag = self.parse_tag()?;
                    let is_r = tag.name == "r";
                    current_line.push(ParsedLineNode::Tag(tag));
                    if is_r {
                        lines.push(ParsedLine(current_line));
                        current_line = Vec::new();
                    }
                }
                _ => text.push(c),
            }
        }
        if !text.is_empty() {
            current_line.push(ParsedLineNode::Text(TextNode(unescape_xml(&text))));
        }
        current_line.push(ParsedLineNode::Tag(TagNode {
            name: "np".to_string(),
            attributes: Vec::new(),
        }));
        lines.push(ParsedLine(current_line));
        Ok(lines)
    }

    fn next(&mut self) -> Option<char> {
        if self.pos < self.str.len() {
            let c = self.str[self.pos..].chars().next()?;
            self.pos += c.len_utf8();
            Some(c)
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct KsScript {
    bom: BomType,
    tree: ParsedScript,
    name_commands: Arc<HashSet<String>>,
    message_commands: Arc<HashSet<String>>,
    remove_empty_lines: bool,
}

impl KsScript {
    pub fn new(reader: Vec<u8>, encoding: Encoding, config: &ExtraConfig) -> Result<Self> {
        let (text, bom) = decode_with_bom_detect(encoding, &reader, true)?;
        let parser = Parser::new(&text);
        let tree = parser.parse(!config.kirikiri_remove_empty_lines)?;
        Ok(Self {
            bom,
            tree,
            name_commands: config.kirikiri_name_commands.clone(),
            message_commands: config.kirikiri_message_commands.clone(),
            remove_empty_lines: config.kirikiri_remove_empty_lines,
        })
    }
}

impl Script for KsScript {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Json
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn extract_messages(&self) -> Result<Vec<Message>> {
        let mut messages = Vec::new();
        let mut name = None;
        let mut message = String::new();
        for obj in self.tree.iter() {
            match obj {
                ParsedScriptNode::Label(_) => {
                    if !message.is_empty() {
                        messages.push(Message {
                            name: name.clone(),
                            message: message.trim_end_matches("<np>").to_owned(),
                        });
                        message.clear();
                        name = None;
                    }
                }
                ParsedScriptNode::Line(line) => {
                    if !message.ends_with("<np>") {
                        message.push_str(&line.to_xml())
                    }
                }
                ParsedScriptNode::Command(cmd) => {
                    if self.name_commands.contains(&cmd.name) {
                        for attr in &cmd.attributes {
                            if let TagAttr::Str(value) = &attr.1 {
                                if !value.is_empty() && !value.is_ascii() {
                                    name = Some(value.clone());
                                    break; // Only take the first name found
                                }
                            }
                        }
                    } else if self.message_commands.contains(&cmd.name) {
                        for attr in &cmd.attributes {
                            if let TagAttr::Str(value) = &attr.1 {
                                if !value.is_empty() && !value.is_ascii() {
                                    messages.push(Message {
                                        name: None,
                                        message: value.clone(),
                                    });
                                    break; // Only take the first message found
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        if !message.is_empty() {
            messages.push(Message {
                name,
                message: message.trim_end_matches("<np>").to_owned(),
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
        let mut mes = messages.iter();
        let mut cur_mes = None;
        let mut tree = self.tree.clone();
        let mut message_lines = Vec::new();
        let mut i = 0;
        let mut is_end = false;
        let mut name_command_block_line: Option<(usize, String)> = None;
        while i < tree.len() {
            match tree[i].clone() {
                ParsedScriptNode::Label(_) => {
                    if !message_lines.is_empty() {
                        let m: &Message = cur_mes
                            .take()
                            .ok_or(anyhow::anyhow!("Not enough messages"))?;
                        if let Some((line, key)) = name_command_block_line.take() {
                            let name = m
                                .name
                                .as_ref()
                                .ok_or(anyhow::anyhow!("Name not found in message"))?;
                            let mut name = name.clone();
                            if let Some(replacement) = replacement {
                                for (key, value) in replacement.map.iter() {
                                    name = name.replace(key, value);
                                }
                            }
                            tree[line].set_attr(&key, name);
                        }
                        let mut text = m.message.to_owned();
                        if let Some(replacement) = replacement {
                            for (key, value) in replacement.map.iter() {
                                text = text.replace(key, value);
                            }
                        }
                        let mess = XMLTextParser::new(&text).parse()?;
                        let diff = mess.len() as isize - message_lines.len() as isize;
                        let common_lines = message_lines.len().min(mess.len());
                        let mut last_index = message_lines.last().cloned().unwrap_or(0);
                        for j in 0..common_lines {
                            tree[message_lines[j]] = ParsedScriptNode::Line(mess[j].clone());
                        }
                        for j in common_lines..message_lines.len() {
                            tree.remove(message_lines[j] - (j - common_lines));
                        }
                        for i in common_lines..mess.len() {
                            let new_line = ParsedScriptNode::Line(mess[i].clone());
                            if last_index < tree.len() {
                                tree.insert(last_index + 1, new_line);
                                last_index += 1;
                            } else {
                                tree.push(new_line);
                            }
                        }
                        i = (i as isize + diff) as usize;
                    }
                    message_lines.clear();
                    is_end = false;
                    if cur_mes.is_none() {
                        cur_mes = mes.next();
                    }
                }
                ParsedScriptNode::Line(line) => {
                    if !is_end {
                        message_lines.push(i);
                        is_end = line.last().map(|e| e.is_np()).unwrap_or(false);
                    }
                }
                ParsedScriptNode::Command(cmd) => {
                    if self.name_commands.contains(&cmd.name) {
                        for attr in &cmd.attributes {
                            if let TagAttr::Str(value) = &attr.1 {
                                if !value.is_empty() && !value.is_ascii() {
                                    name_command_block_line = Some((i, attr.0.clone()));
                                    break; // Only update the first name found
                                }
                            }
                        }
                    } else if self.message_commands.contains(&cmd.name) {
                        for attr in &cmd.attributes {
                            if let TagAttr::Str(value) = &attr.1 {
                                if !value.is_empty() && !value.is_ascii() {
                                    let m = cur_mes
                                        .take()
                                        .ok_or(anyhow::anyhow!("Not enough messages"))?;
                                    let mut text = m.message.clone();
                                    if let Some(replacement) = replacement {
                                        for (key, value) in replacement.map.iter() {
                                            text = text.replace(key, value);
                                        }
                                    }
                                    tree[i].set_attr(&attr.0, text);
                                    cur_mes = mes.next();
                                    break; // Only update the first message found
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
            i += 1;
        }
        if !message_lines.is_empty() {
            let m: &Message = cur_mes
                .take()
                .ok_or(anyhow::anyhow!("Not enough messages"))?;
            if let Some((line, key)) = name_command_block_line.take() {
                let name = m
                    .name
                    .as_ref()
                    .ok_or(anyhow::anyhow!("Name not found in message"))?;
                let mut name = name.clone();
                if let Some(replacement) = replacement {
                    for (key, value) in replacement.map.iter() {
                        name = name.replace(key, value);
                    }
                }
                tree[line].set_attr(&key, name);
            }
            let mut text = m.message.to_owned();
            if let Some(replacement) = replacement {
                for (key, value) in replacement.map.iter() {
                    text = text.replace(key, value);
                }
            }
            let mess = XMLTextParser::new(&text).parse()?;
            let common_lines = message_lines.len().min(mess.len());
            let mut last_index = message_lines.last().cloned().unwrap_or(0);
            for j in 0..common_lines {
                tree[message_lines[j]] = ParsedScriptNode::Line(mess[j].clone());
            }
            for j in common_lines..message_lines.len() {
                tree.remove(message_lines[j] - (j - common_lines));
            }
            for i in common_lines..mess.len() {
                let new_line = ParsedScriptNode::Line(mess[i].clone());
                if last_index < tree.len() {
                    tree.insert(last_index + 1, new_line);
                    last_index += 1;
                } else {
                    tree.push(new_line);
                }
            }
        }
        if cur_mes.is_some() || mes.next().is_some() {
            return Err(anyhow::anyhow!("Some messages were not processed."));
        }
        if self.remove_empty_lines {
            tree.retain(|node| !node.is_empty());
        }
        let s = tree.serialize() + "\n";
        let data = encode_string_with_bom(encoding, &s, false, self.bom)?;
        file.write_all(&data)?;
        Ok(())
    }
}
