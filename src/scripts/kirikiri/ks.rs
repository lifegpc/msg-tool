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
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(KsScript::new(buf, encoding, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ks"]
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

#[derive(Debug)]
pub struct KsScript {
    bom: BomType,
    tree: ParsedScript,
    name_commands: Arc<HashSet<String>>,
    message_commands: Arc<HashSet<String>>,
}

impl KsScript {
    pub fn new(reader: Vec<u8>, encoding: Encoding, config: &ExtraConfig) -> Result<Self> {
        let (text, bom) = decode_with_bom_detect(encoding, &reader)?;
        let parser = Parser::new(&text);
        let tree = parser.parse(!config.kirikiri_remove_empty_lines)?;
        Ok(Self {
            bom,
            tree,
            name_commands: config.kirikiri_name_commands.clone(),
            message_commands: config.kirikiri_message_commands.clone(),
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
                ParsedScriptNode::Line(line) => message.push_str(&line.to_xml()),
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
            messages.push(Message { name, message });
        }
        Ok(messages)
    }

    fn import_messages<'a>(
        &'a self,
        messages: Vec<Message>,
        mut file: Box<dyn WriteSeek + 'a>,
        encoding: Encoding,
        _replacement: Option<&'a ReplacementTable>,
    ) -> Result<()> {
        let mut mes = messages.iter();
        let mut _cur_mes = mes.next();
        let mut tree = self.tree.clone();
        for obj in tree.iter_mut() {
            match obj {
                _ => {}
            }
        }
        let s = tree.serialize();
        let data = encode_string_with_bom(encoding, &s, false, self.bom)?;
        file.write_all(&data)?;
        Ok(())
    }
}
