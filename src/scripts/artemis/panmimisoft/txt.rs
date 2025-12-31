use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use crate::utils::escape::*;
use anyhow::Result;
use std::collections::HashSet;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone)]
/// Artemis TXT script builder
pub struct TxtBuilder {}

impl TxtBuilder {
    /// Creates a new instance of `TxtBuilder`
    pub const fn new() -> Self {
        Self {}
    }
}

impl ScriptBuilder for TxtBuilder {
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
        Ok(Box::new(TxtScript::new(buf, encoding, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["txt"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::ArtemisPanmimisoftTxt
    }
}

/// Artemis TXT script nodes
pub trait Node {
    /// Serialize the node to a string representation.
    fn serialize(&self) -> String;
}

#[derive(Debug, Clone, PartialEq)]
/// Represents a comment node in Artemis TXT scripts.
pub struct CommentNode(pub String);

impl Node for CommentNode {
    fn serialize(&self) -> String {
        format!("//{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq)]
/// Empty Line Node
pub struct EmptyLineNode;

impl Node for EmptyLineNode {
    fn serialize(&self) -> String {
        String::new()
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Represents a label node in Artemis TXT scripts.
pub struct LabelNode(pub String);

impl Node for LabelNode {
    fn serialize(&self) -> String {
        format!("*{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Represents a tag node in Artemis TXT scripts.
pub struct TagNode {
    /// The name of the tag.
    pub name: String,
    /// The attributes of the tag, represented as a vector of key-value pairs.
    pub attributes: Vec<(String, Option<String>)>,
}

impl Node for TagNode {
    fn serialize(&self) -> String {
        let attributes = self
            .attributes
            .iter()
            .map(|(key, value)| {
                if let Some(val) = value {
                    format!("{}=\"{}\"", key, val)
                } else {
                    key.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        if attributes.is_empty() {
            format!("[{}]", self.name)
        } else {
            format!("[{} {}]", self.name, attributes)
        }
    }
}

impl TagNode {
    fn ser_attributes_xml(&self) -> String {
        let mut parts = Vec::new();
        for (key, value) in self.attributes.iter() {
            match value {
                None => {
                    parts.push(key.clone());
                }
                Some(val) => {
                    parts.push(format!("{}=\"{}\"", key, escape_xml_attr_value(val)));
                }
            }
        }
        parts.join(" ")
    }

    /// Get attribute value of attribute in tag by name.
    pub fn get_attr(&self, attr: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(key, _)| key == attr)
            .and_then(|(_, value)| value.as_deref())
    }

    /// Returns true if the tag is not suitable for name.
    pub fn is_blocked_name(&self, set: &HashSet<String>) -> bool {
        self.name.is_ascii() || set.contains(&self.name)
    }

    /// Checks if the tag has a specific attribute.
    pub fn has_attr(&self, attr: &str) -> bool {
        self.attributes.iter().any(|(key, _)| key == attr)
    }

    /// Sets the value of an attribute in the tag.
    pub fn set_attr(&mut self, attr: &str, value: Option<String>) {
        if let Some(pos) = self.attributes.iter().position(|(key, _)| key == attr) {
            self.attributes[pos].1 = value;
        } else {
            self.attributes.push((attr.to_string(), value));
        }
    }

    /// Converts the node to a xml-like string representation.
    pub fn to_xml(&self) -> String {
        let attributes = self.ser_attributes_xml();
        if attributes.is_empty() {
            format!("<{}>", self.name)
        } else {
            format!("<{} {}>", self.name, attributes)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Represents a text node in Artemis TXT scripts.
pub struct TextNode(pub String);

#[derive(Debug, Clone, PartialEq)]
/// Represents a node in a TXT line.
pub enum TxtLineNode {
    Comment(CommentNode),
    Tag(TagNode),
    Text(TextNode),
}

impl TxtLineNode {
    /// Checks if the node is a comment.
    pub fn is_comment(&self) -> bool {
        matches!(self, TxtLineNode::Comment(_))
    }

    /// Checks if the node is a tag.
    ///
    /// * `tag` - The name of the tag.
    pub fn is_tag(&self, tag: &str) -> bool {
        matches!(self, TxtLineNode::Tag(node) if node.name == tag)
    }

    /// Returns true if the tag is a blocked name.
    pub fn is_tag_blocked_name(&self, set: &HashSet<String>) -> bool {
        if let TxtLineNode::Tag(node) = self {
            node.is_blocked_name(set)
        } else {
            false
        }
    }

    /// Returns an iterator over the keys of the attributes of the tag node.
    pub fn tag_attr_keys<'a>(&'a self) -> Box<dyn Iterator<Item = &'a str> + 'a> {
        if let TxtLineNode::Tag(node) = self {
            Box::new(node.attributes.iter().map(|(key, _)| key.as_str()))
        } else {
            Box::new(std::iter::empty())
        }
    }

    pub fn tag_get_attr<'a>(&'a self, attr: &str) -> Option<&'a str> {
        if let TxtLineNode::Tag(node) = self {
            node.get_attr(attr)
        } else {
            None
        }
    }

    /// Checks if the tag has a specific attribute.
    pub fn tag_has_attr(&self, attr: &str) -> bool {
        if let TxtLineNode::Tag(node) = self {
            node.attributes.iter().any(|(key, _)| key == attr)
        } else {
            false
        }
    }

    pub fn tag_set_attr(&mut self, attr: &str, value: Option<String>) {
        if let TxtLineNode::Tag(node) = self {
            node.set_attr(attr, value);
        }
    }

    /// Converts the node to a xml-like string representation.
    pub fn to_xml(&self) -> String {
        match self {
            TxtLineNode::Comment(_) => String::new(), // Ignore comments in XML
            TxtLineNode::Tag(n) => {
                if (n.name == "rt2" || n.name == "ret2") && n.attributes.is_empty() {
                    "\n".to_string()
                } else {
                    n.to_xml()
                }
            }
            TxtLineNode::Text(n) => escape_xml_text_value(&n.0),
        }
    }
}

impl Node for TxtLineNode {
    fn serialize(&self) -> String {
        match self {
            TxtLineNode::Comment(node) => node.serialize(),
            TxtLineNode::Tag(node) => node.serialize(),
            TxtLineNode::Text(node) => node.0.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Represents a line in Artemis TXT scripts, which can contain multiple nodes.
pub struct TxtLine(pub Vec<TxtLineNode>);

impl Deref for TxtLine {
    type Target = Vec<TxtLineNode>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for TxtLine {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Node for TxtLine {
    fn serialize(&self) -> String {
        self.0
            .iter()
            .map(|node| node.serialize())
            .collect::<Vec<_>>()
            .join("")
    }
}

impl TxtLine {
    /// Converts the line to a xml-like string representation.
    pub fn to_xml(&self) -> String {
        self.0
            .iter()
            .map(|node| node.to_xml())
            .collect::<Vec<_>>()
            .join("")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LineTag {
    pub name: String,
    pub list: Vec<String>,
}

impl Node for LineTag {
    fn serialize(&self) -> String {
        let list = self.list.join(",");
        format!("#{} {}", self.name, list)
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Represents a parsed line in Artemis TXT scripts.
pub enum ParsedLine {
    /// Empty line.
    Empty(EmptyLineNode),
    /// Comment line.
    Comment(CommentNode),
    /// Label line.
    Label(LabelNode),
    /// Line
    Line(TxtLine),
    /// Line tag
    LineTag(LineTag),
    /// Comment Line starts with ;>>
    Comment2(CommentNode),
}

impl Node for ParsedLine {
    fn serialize(&self) -> String {
        match self {
            ParsedLine::Empty(node) => node.serialize(),
            ParsedLine::Comment(node) => node.serialize(),
            ParsedLine::Label(node) => node.serialize(),
            ParsedLine::Line(line) => line.serialize(),
            ParsedLine::LineTag(node) => node.serialize(),
            ParsedLine::Comment2(node) => format!(";>>{}", node.0),
        }
    }
}

impl ParsedLine {
    /// Returns the length of the line.
    pub fn len(&self) -> usize {
        match self {
            ParsedLine::Empty(_) => 0,
            ParsedLine::Comment(_) => 0,
            ParsedLine::Label(_) => 0,
            ParsedLine::Line(line) => line.len(),
            ParsedLine::LineTag(_) => 0,
            ParsedLine::Comment2(_) => 0,
        }
    }

    /// Push a node to the line.
    pub fn push(&mut self, node: TxtLineNode) {
        if let ParsedLine::Line(line) = self {
            line.push(node);
        } else {
            // Do'nt care about other types
        }
    }

    /// Inserts a node at the specified index in the line.
    pub fn insert(&mut self, index: usize, node: TxtLineNode) {
        if let ParsedLine::Line(line) = self {
            line.insert(index, node);
        } else {
            // Do'nt care about other types
        }
    }

    /// Remove a node at the specified index from the line.
    pub fn remove(&mut self, index: usize) -> Option<TxtLineNode> {
        if let ParsedLine::Line(line) = self {
            if index < line.len() {
                Some(line.remove(index))
            } else {
                None
            }
        } else {
            // Do'nt care about other types
            None
        }
    }
}

#[derive(Debug, Clone)]
/// Represents a parsed Artemis TXT script.
pub struct ParsedScript(pub Vec<ParsedLine>);

impl Deref for ParsedScript {
    type Target = Vec<ParsedLine>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ParsedScript {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Node for ParsedScript {
    fn serialize(&self) -> String {
        self.0
            .iter()
            .map(|line| line.serialize())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Parser for Artemis TXT scripts.
pub struct Parser {
    lines: Vec<String>,
}

impl Parser {
    pub fn new<S: AsRef<str> + ?Sized>(script: &S) -> Self {
        let lines = script.as_ref().lines().map(|s| s.to_string()).collect();
        Self { lines }
    }

    pub fn parse(&self, preserve_empty_lines: bool) -> Result<ParsedScript> {
        let mut parsed_script = Vec::new();
        let mut i = 0;
        let line_count = self.lines.len();
        while i < line_count {
            let line = self.lines[i].trim();
            i += 1;
            if line.is_empty() {
                if preserve_empty_lines {
                    parsed_script.push(ParsedLine::Empty(EmptyLineNode));
                }
                continue;
            }
            if line.starts_with("//") {
                parsed_script.push(ParsedLine::Comment(CommentNode(line[2..].to_string())));
                continue;
            }
            if line.starts_with("*") {
                let label = line[1..].trim().to_string();
                parsed_script.push(ParsedLine::Label(LabelNode(label)));
                continue;
            }
            if line.starts_with("#") {
                let rest = line[1..].trim();
                let mut parts = rest.splitn(2, ' ');
                let name = parts
                    .next()
                    .ok_or(anyhow::anyhow!("Invalid line tag: {}", line))?
                    .to_string();
                let list = if let Some(list_str) = parts.next() {
                    list_str
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .collect::<Vec<_>>()
                } else {
                    Vec::new()
                };
                parsed_script.push(ParsedLine::LineTag(LineTag { name, list }));
                continue;
            }
            if line.starts_with(";>>") {
                parsed_script.push(ParsedLine::Comment2(CommentNode(line[3..].to_string())));
                continue;
            }
            let mut temp = String::new();
            let mut nodes = Vec::new();
            let mut line_graphs = line.graphemes(true).collect::<Vec<_>>();
            let mut line_pos = 0;
            let mut is_comment = false;
            while line_pos < line_graphs.len() {
                let graph = line_graphs[line_pos];
                line_pos += 1;
                temp.push_str(graph);
                if is_comment {
                    continue;
                }
                if !is_comment && temp.ends_with("//") && temp.len() > 2 {
                    nodes.push(TxtLineNode::Text(TextNode(
                        temp[..temp.len() - 2].to_string(),
                    )));
                    temp.clear();
                    is_comment = true;
                    continue;
                }
                if graph == "[" {
                    if !temp.trim_end_matches("[").is_empty() {
                        nodes.push(TxtLineNode::Text(TextNode(
                            temp.trim_end_matches("[").to_string(),
                        )));
                    }
                    // Tag may end in another line, so we need check it.
                    while !line_graphs[line_pos..].contains(&"]") {
                        if i < line_count {
                            let nline = self.lines[i].trim();
                            i += 1;
                            // Add next line to the current line
                            line_graphs.push("\n");
                            line_graphs.extend(nline.graphemes(true));
                        } else {
                            break;
                        }
                    }
                    let (tag, nextpos) = TagParser {
                        graphs: &line_graphs,
                        pos: line_pos,
                    }
                    .parse()?;
                    line_pos = nextpos;
                    nodes.push(TxtLineNode::Tag(tag));
                    temp.clear();
                    continue;
                }
            }
            if is_comment {
                nodes.push(TxtLineNode::Comment(CommentNode(temp)));
            } else {
                if !temp.is_empty() {
                    nodes.push(TxtLineNode::Text(TextNode(temp)));
                }
            }
            parsed_script.push(ParsedLine::Line(TxtLine(nodes)));
        }
        Ok(ParsedScript(parsed_script))
    }
}

struct TagParser<'a> {
    graphs: &'a [&'a str],
    pos: usize,
}

impl<'a> TagParser<'a> {
    fn peek(&self) -> Option<&'a str> {
        self.graphs.get(self.pos).cloned()
    }

    fn eat(&mut self) {
        if self.pos < self.graphs.len() {
            self.pos += 1;
        }
    }

    fn next(&mut self) -> Option<&'a str> {
        if self.pos < self.graphs.len() {
            let graph = self.graphs[self.pos];
            self.pos += 1;
            Some(graph)
        } else {
            None
        }
    }

    fn is_indent(&self, indent: &str) -> bool {
        let mut pos = self.pos;
        for ident in indent.graphemes(true) {
            if pos >= self.graphs.len() || self.graphs[pos] != ident {
                return false;
            }
            pos += 1;
        }
        true
    }

    fn eat_all_equal(&mut self) {
        while let Some(graph) = self.peek() {
            if graph == "=" {
                self.eat();
            } else {
                break;
            }
        }
    }

    fn parse(&mut self) -> Result<(TagNode, usize)> {
        let name = self.parse_tag()?;
        self.erase_whitespace();
        let mut attributes = Vec::new();
        loop {
            let graph = match self.peek() {
                Some(g) => g,
                None => {
                    return Err(anyhow::anyhow!(
                        "Unexpected end of tag parsing: {}",
                        self.graphs.join("")
                    ));
                }
            };
            if graph == "]" {
                self.eat();
                break;
            }
            if graph == " " || graph == "\t" {
                self.eat();
                continue;
            }
            if graph == "=" {
                return Err(anyhow::anyhow!("Unexpected '=' without attribute name"));
            }
            let attr_name = self.parse_attr_name()?;
            self.erase_whitespace();
            let graph = match self.peek() {
                Some(g) => g,
                None => {
                    return Err(anyhow::anyhow!(
                        "Unexpected end of tag parsing: {}",
                        self.graphs.join("")
                    ));
                }
            };
            if graph == "]" {
                self.eat();
                attributes.push((attr_name, None));
                break;
            }
            if graph == "=" {
                // Sometimes the script contains multiple equal signs
                // We just ignore them
                // Example: [イベントCG st = "拡大/ev005/a" add = "拡大/ev005/y2,拡大/ev005/r5" left = "min ~ max" top = "max ~ 1/4" mtime = "60000" ease == "減速" mfade = "1000" hide = "1"]
                self.eat_all_equal();
                self.erase_whitespace();
                let value = self.parse_attr_value()?;
                attributes.push((attr_name, Some(value)));
                self.erase_whitespace();
            } else {
                attributes.push((attr_name, None));
                self.erase_whitespace();
                continue;
            }
        }
        return Ok((TagNode { name, attributes }, self.pos));
    }

    fn erase_whitespace(&mut self) {
        while let Some(graph) = self.peek() {
            if graph == " " || graph == "\t" {
                self.eat();
            } else {
                break;
            }
        }
    }

    fn parse_attr_name(&mut self) -> Result<String> {
        let mut attr_name = String::new();
        while let Some(graph) = self.peek() {
            if graph == "=" || graph == " " || graph == "\t" || graph == "]" {
                break;
            }
            attr_name.push_str(graph);
            self.eat();
        }
        if attr_name.is_empty() {
            return Err(anyhow::anyhow!("Empty attribute name found"));
        }
        Ok(attr_name)
    }

    fn parse_attr_value(&mut self) -> Result<String> {
        let mut value = String::new();
        if !self.is_indent("\"") {
            return Err(anyhow::anyhow!(
                "Expected attribute value to start with a quote: {}",
                self.graphs.join("")
            ));
        }
        self.eat(); // Skip the opening quote
        while let Some(graph) = self.next() {
            if graph == "\"" {
                break; // End of attribute value
            }
            value.push_str(graph);
        }
        Ok(value)
    }

    fn parse_tag(&mut self) -> Result<String> {
        let mut tag = String::new();
        while let Some(graph) = self.peek() {
            if graph == " " || graph == "\t" || graph == "]" {
                break;
            }
            tag.push_str(graph);
            self.eat();
        }
        if tag.is_empty() {
            return Err(anyhow::anyhow!("Empty tag found"));
        }
        Ok(tag)
    }
}

struct XMLTextParser<'a> {
    str: &'a str,
    lang: &'a str,
    pos: usize,
    no_rt2: bool,
}

impl<'a> XMLTextParser<'a> {
    pub fn new(text: &'a str, lang: &'a str, no_rt2: bool) -> Self {
        Self {
            str: text,
            lang,
            pos: 0,
            no_rt2,
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
                            attributes.push((key.clone(), None));
                            key.clear();
                        }
                    } else if is_value {
                        if is_in_quote {
                            value.push(c);
                        } else {
                            if !value.is_empty() {
                                attributes.push((key.clone(), Some(unescape_xml(&value))));
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
                            attributes.push((key.clone(), Some(unescape_xml(&value))));
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
        if !self.no_rt2 {
            current_line.push(TxtLineNode::Tag(TagNode {
                name: "lang".to_string(),
                attributes: vec![(self.lang.to_string(), None)],
            }));
        }
        while let Some(c) = self.next() {
            match c {
                '<' => {
                    if !text.is_empty() {
                        current_line.push(TxtLineNode::Text(TextNode(unescape_xml(&text))));
                        text.clear();
                    }
                    let tag = self.parse_tag()?;
                    let is_r = tag.name == "rt2" || tag.name == "ret2";
                    current_line.push(TxtLineNode::Tag(tag));
                    if is_r {
                        lines.push(ParsedLine::Line(TxtLine(current_line)));
                        current_line = Vec::new();
                    }
                }
                '\n' => {
                    if !text.is_empty() {
                        current_line.push(TxtLineNode::Text(TextNode(unescape_xml(&text))));
                        text.clear();
                    }
                    if !self.no_rt2 {
                        current_line.push(TxtLineNode::Tag(TagNode {
                            name: "rt2".to_string(),
                            attributes: Vec::new(),
                        }));
                    }
                    lines.push(ParsedLine::Line(TxtLine(current_line)));
                    current_line = Vec::new();
                }
                _ => text.push(c),
            }
        }
        if !text.is_empty() {
            current_line.push(TxtLineNode::Text(TextNode(unescape_xml(&text))));
        }
        if !self.no_rt2 {
            current_line.push(TxtLineNode::Tag(TagNode {
                name: "/lang".to_string(),
                attributes: Vec::new(),
            }));
        }
        lines.push(ParsedLine::Line(TxtLine(current_line)));
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
pub struct TxtScript {
    tree: ParsedScript,
    blacklist_names: Arc<HashSet<String>>,
    lang: Option<String>,
}

impl TxtScript {
    /// Creates a new instance of `TxtScript` from the given buffer and encoding.
    pub fn new(buf: Vec<u8>, encoding: Encoding, config: &ExtraConfig) -> Result<Self> {
        let script = decode_to_string(encoding, &buf, true)?;
        let parser = Parser::new(&script);
        let tree = parser.parse(true)?;
        Ok(Self {
            tree,
            blacklist_names: config.artemis_panmimisoft_txt_blacklist_names.clone(),
            lang: config.artemis_panmimisoft_txt_lang.clone(),
        })
    }
}

impl Script for TxtScript {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Json
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn extract_messages(&self) -> Result<Vec<Message>> {
        let mut messages = Vec::new();
        let mut i = 0;
        let len = self.tree.len();
        let mut last_tag_block: Option<TagNode> = None;
        let mut lang = self.lang.as_ref().map(|s| s.as_str());
        let mut message = TxtLine(Vec::new());
        let mut in_lang_block = false;
        let mut droped_lang_block = false;
        let mut is_selectblk = false;
        let mut has_printlang = false;
        for line in &self.tree.0 {
            if let ParsedLine::Line(txt_line) = line {
                for node in txt_line.iter() {
                    if node.is_tag("printlang") {
                        has_printlang = true;
                        break;
                    }
                }
            }
        }
        if !has_printlang {
            let mut message_started = false;
            let mut adv_started = false;
            while i < len {
                let line = &self.tree[i];
                match line {
                    ParsedLine::Empty(_) => {
                        let mes = message.to_xml();
                        message.clear();
                        message_started = false;
                        if !mes.is_empty() && adv_started {
                            let name = match &last_tag_block {
                                Some(block) => Some(if let Some(name) = block.get_attr("name") {
                                    name.to_string()
                                } else {
                                    block.name.clone()
                                }),
                                _ => None,
                            };
                            messages.push(Message { name, message: mes });
                        }
                        last_tag_block = None;
                    }
                    ParsedLine::Line(line) => {
                        if !message.is_empty() && message_started {
                            message.push(TxtLineNode::Tag(TagNode {
                                name: "rt2".into(),
                                attributes: vec![],
                            }));
                        }
                        for node in line.iter() {
                            if node.is_tag("adv") {
                                adv_started = true;
                            } else if node.is_tag("selectbtn_init") {
                                is_selectblk = true;
                            } else if node.is_tag("selectbtn") {
                                let text = node.tag_get_attr("text").ok_or(anyhow::anyhow!(
                                    "No text attribute found in selectbtn tag"
                                ))?;
                                messages.push(Message {
                                    name: None,
                                    message: text.to_string(),
                                });
                            } else if node.is_tag("/selectbtn") {
                                is_selectblk = false;
                            } else if let TxtLineNode::Tag(tag) = node {
                                if !message_started {
                                    if !tag.is_blocked_name(&self.blacklist_names) {
                                        last_tag_block = Some(tag.clone());
                                    }
                                } else {
                                    message.push(node.clone());
                                }
                            } else if node.is_comment() {
                                // Ignore comments
                            } else {
                                message_started = true;
                                message.push(node.clone());
                            }
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
        }
        while i < len {
            let line = &self.tree[i];
            if let ParsedLine::Line(line) = line {
                for node in line.iter() {
                    if node.is_tag("lang") {
                        let lan = match lang {
                            Some(l) => l,
                            None => {
                                for key in node.tag_attr_keys() {
                                    lang = Some(key);
                                    break;
                                }
                                match lang {
                                    Some(l) => l,
                                    None => {
                                        return Err(anyhow::anyhow!(
                                            "No language found in lang tag"
                                        ));
                                    }
                                }
                            }
                        };
                        if node.tag_has_attr(lan) {
                            in_lang_block = true;
                        } else {
                            droped_lang_block = true;
                        }
                    } else if node.is_tag("/lang") {
                        in_lang_block = false;
                        droped_lang_block = false;
                    } else if node.is_tag("printlang") {
                        let mes = message.to_xml();
                        message.clear();
                        if !mes.is_empty() {
                            let name = match &last_tag_block {
                                Some(block) => Some(if let Some(name) = block.get_attr("name") {
                                    name.to_string()
                                } else {
                                    block.name.clone()
                                }),
                                _ => None,
                            };
                            messages.push(Message { name, message: mes });
                        }
                        last_tag_block = None;
                    } else if node.is_tag("selectbtn_init") {
                        is_selectblk = true;
                    } else if node.is_tag("selectbtn") {
                        let mut lan = match lang {
                            Some(l) => l,
                            None => {
                                for key in node.tag_attr_keys() {
                                    if key == "label" || key == "call" {
                                        continue;
                                    }
                                    lang = Some(key);
                                    break;
                                }
                                match lang {
                                    Some(l) => l,
                                    None => {
                                        return Err(anyhow::anyhow!(
                                            "No language found in selectbtn tag"
                                        ));
                                    }
                                }
                            }
                        };
                        if !node.tag_has_attr(lan) {
                            for key in node.tag_attr_keys() {
                                if key == "label" || key == "call" {
                                    continue;
                                }
                                lan = key;
                                break;
                            }
                        }
                        if let Some(t) = node.tag_get_attr(lan) {
                            messages.push(Message {
                                name: None,
                                message: t.to_string(),
                            });
                        }
                    } else if node.is_tag("/selectbtn") {
                        is_selectblk = false;
                    } else if in_lang_block {
                        message.push(node.clone());
                    } else if droped_lang_block {
                        // Drop the message if we are in a dropped lang block
                    } else if is_selectblk {
                        // Drop other nodes in select block
                    } else if let TxtLineNode::Tag(tag) = node {
                        if !tag.is_blocked_name(&self.blacklist_names) {
                            last_tag_block = Some(tag.clone());
                        }
                    }
                }
            }
            i += 1;
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
        let mut output = self.tree.clone();
        let mut current_line = 0;
        let mut last_tag_block_loc = None;
        let mut lang = self.lang.clone();
        let mut mes = messages.iter();
        let mut mess = mes.next();
        let mut lang_block_index = None;
        let mut lang_end_block_index = None;
        let mut in_lang_block = false;
        let mut droped_lang_block = false;
        let mut is_selectblk = false;
        let mut has_printlang = false;
        for line in &output.0 {
            if let ParsedLine::Line(txt_line) = line {
                for node in txt_line.iter() {
                    if node.is_tag("printlang") {
                        has_printlang = true;
                        break;
                    }
                }
            }
        }
        if !has_printlang {
            let mut adv_started = false;
            let mut started_line: Option<usize> = None;
            while current_line < output.len() {
                let line = output[current_line].clone();
                match &line {
                    ParsedLine::Empty(_) => {
                        if adv_started {
                            if let Some(start) = started_line {
                                let m = match mess {
                                    Some(m) => m,
                                    None => {
                                        return Err(anyhow::anyhow!("Not enough messages."));
                                    }
                                };
                                let mut message = m.message.clone();
                                if let Some(repl) = replacement {
                                    for (k, v) in &repl.map {
                                        message = message.replace(k, v);
                                    }
                                }
                                if let Some(name) = &m.name {
                                    let block_index: (usize, usize) =
                                        match last_tag_block_loc.take() {
                                            Some(data) => data,
                                            None => {
                                                return Err(anyhow::anyhow!(
                                                    "No name tag block found before message."
                                                ));
                                            }
                                        };
                                    let mut name = name.clone();
                                    if let Some(repl) = replacement {
                                        for (k, v) in &repl.map {
                                            name = name.replace(k, v);
                                        }
                                    }
                                    let mblock = &mut output[block_index.0];
                                    if let ParsedLine::Line(txt_line) = mblock {
                                        let block = txt_line[block_index.1].clone();
                                        if let TxtLineNode::Tag(mut tag) = block {
                                            tag.set_attr("name", Some(name));
                                            txt_line[block_index.1] = TxtLineNode::Tag(tag);
                                        } else {
                                            return Err(anyhow::anyhow!(
                                                "Last tag block is not a tag: {:?}",
                                                mblock
                                            ));
                                        }
                                    } else {
                                        return Err(anyhow::anyhow!(
                                            "Last tag block is not a line: {:?}",
                                            mblock
                                        ));
                                    }
                                }
                                let nodes = XMLTextParser::new(&message, "", true).parse()?;
                                let ori_len = (current_line - start) as isize;
                                let new_len = nodes.len() as isize;
                                for _ in start..current_line {
                                    output.remove(start);
                                }
                                let mut start_index = start;
                                for node in nodes {
                                    output.insert(start_index, node);
                                    start_index += 1;
                                }
                                current_line = (current_line as isize + new_len - ori_len) as usize;
                                mess = mes.next();
                            }
                        }
                        started_line = None;
                        last_tag_block_loc = None;
                    }
                    ParsedLine::Line(line) => {
                        for (i, node) in line.iter().enumerate() {
                            if node.is_tag("adv") {
                                adv_started = true;
                            } else if node.is_tag("selectbtn_init") {
                                is_selectblk = true;
                            } else if node.is_tag("selectbtn") {
                                let m = match mess {
                                    Some(m) => m,
                                    None => {
                                        return Err(anyhow::anyhow!("Not enough messages."));
                                    }
                                };
                                let mut message = m.message.clone();
                                if let Some(repl) = replacement {
                                    for (k, v) in &repl.map {
                                        message = message.replace(k, v);
                                    }
                                }
                                let mut node = node.clone();
                                node.tag_set_attr("text", Some(message));
                                let block = &mut output[current_line];
                                if let ParsedLine::Line(txt_line) = block {
                                    txt_line[i] = node;
                                } else {
                                    return Err(anyhow::anyhow!(
                                        "Selectbtn line is not a line: {:?}",
                                        block
                                    ));
                                }
                                mess = mes.next();
                            } else if let TxtLineNode::Tag(tag) = node {
                                if started_line.is_none() {
                                    if !tag.is_blocked_name(&self.blacklist_names) {
                                        last_tag_block_loc = Some((current_line, i));
                                    }
                                }
                            } else if node.is_comment() {
                                // Ignore comments
                            } else {
                                if started_line.is_none() {
                                    started_line = Some(current_line);
                                }
                            }
                        }
                    }
                    _ => {}
                }
                current_line += 1;
            }
        }
        while current_line < output.len() {
            let line = output[current_line].clone();
            if let ParsedLine::Line(line) = &line {
                for (i, node) in line.iter().enumerate() {
                    if node.is_tag("lang") {
                        let lan = match lang.as_ref() {
                            Some(l) => l.as_str(),
                            None => {
                                for key in node.tag_attr_keys() {
                                    lang = Some(key.to_string());
                                    break;
                                }
                                match lang.as_ref() {
                                    Some(l) => l.as_str(),
                                    None => {
                                        return Err(anyhow::anyhow!(
                                            "No language found in lang tag"
                                        ));
                                    }
                                }
                            }
                        };
                        if node.tag_has_attr(lan) {
                            in_lang_block = true;
                            lang_block_index = Some((current_line, i));
                        } else {
                            droped_lang_block = true;
                        }
                    } else if node.is_tag("/lang") {
                        if in_lang_block {
                            lang_end_block_index = Some((current_line, i));
                        }
                        in_lang_block = false;
                        droped_lang_block = false;
                    } else if node.is_tag("printlang") {
                        let lan = lang
                            .as_ref()
                            .map(|s| s.as_str())
                            .ok_or(anyhow::anyhow!("No language specified."))?;
                        let m = match mess {
                            Some(m) => m,
                            None => {
                                return Err(anyhow::anyhow!("Not enough messages."));
                            }
                        };
                        if let Some(name) = &m.name {
                            let block_index: (usize, usize) = match last_tag_block_loc.take() {
                                Some(data) => data,
                                None => {
                                    return Err(anyhow::anyhow!(
                                        "No name tag block found before printlang.",
                                    ));
                                }
                            };
                            let mut name = name.clone();
                            if let Some(repl) = replacement {
                                for (k, v) in &repl.map {
                                    name = name.replace(k, v);
                                }
                            }
                            let mblock = &mut output[block_index.0];
                            if let ParsedLine::Line(txt_line) = mblock {
                                let block = txt_line[block_index.1].clone();
                                if let TxtLineNode::Tag(mut tag) = block {
                                    tag.set_attr("name", Some(name));
                                    txt_line[block_index.1] = TxtLineNode::Tag(tag);
                                } else {
                                    return Err(anyhow::anyhow!(
                                        "Last tag block is not a tag: {:?}",
                                        mblock
                                    ));
                                }
                            } else {
                                return Err(anyhow::anyhow!(
                                    "Last tag block is not a line: {:?}",
                                    mblock
                                ));
                            }
                        }
                        let mut message = m.message.clone();
                        if let Some(repl) = replacement {
                            for (k, v) in &repl.map {
                                message = message.replace(k, v);
                            }
                        }
                        let mut nodes = XMLTextParser::new(&message, lan, false).parse()?;
                        if lang_block_index.is_some() && lang_end_block_index.is_some() {
                            let start_index = lang_block_index.unwrap();
                            let end_index = lang_end_block_index.unwrap();
                            if start_index.1 != 0 {
                                let block = output[start_index.0].clone();
                                if let ParsedLine::Line(txt_line) = block {
                                    for i in 0..start_index.1 {
                                        nodes[0].insert(i, txt_line[i].clone());
                                    }
                                } else {
                                    return Err(anyhow::anyhow!(
                                        "Lang block start is not a line: {:?}",
                                        block
                                    ));
                                }
                            }
                            if end_index.1 + 1 < output[end_index.0].len() {
                                let block = output[end_index.0].clone();
                                if let ParsedLine::Line(txt_line) = block {
                                    for i in end_index.1 + 1..txt_line.len() {
                                        nodes.last_mut().unwrap().push(txt_line[i].clone());
                                    }
                                } else {
                                    return Err(anyhow::anyhow!(
                                        "Lang block end is not a line: {:?}",
                                        block
                                    ));
                                }
                            }
                            let ori_len = (end_index.0 - start_index.0 + 1) as isize;
                            let new_len = nodes.len() as isize;
                            for _ in start_index.0..=end_index.0 {
                                output.remove(start_index.0);
                            }
                            let mut start_index = start_index.0;
                            for node in nodes {
                                output.insert(start_index, node);
                                start_index += 1;
                            }
                            current_line = (current_line as isize + new_len - ori_len) as usize;
                        } else {
                            // Add a new lang block if not exists
                            for node in nodes {
                                output.insert(current_line, node);
                                current_line += 1;
                            }
                        }
                        lang_block_index = None;
                        lang_end_block_index = None;
                        mess = mes.next();
                        last_tag_block_loc = None;
                    } else if node.is_tag("selectbtn_init") {
                        is_selectblk = true;
                    } else if node.is_tag("selectbtn") {
                        let lan = match lang.as_ref() {
                            Some(l) => l.as_str(),
                            None => {
                                for key in node.tag_attr_keys() {
                                    if key == "label" || key == "call" {
                                        continue;
                                    }
                                    lang = Some(key.to_string());
                                    break;
                                }
                                match lang.as_ref() {
                                    Some(l) => l.as_str(),
                                    None => {
                                        return Err(anyhow::anyhow!(
                                            "No language found in selectbtn tag"
                                        ));
                                    }
                                }
                            }
                        };
                        let m = match mess {
                            Some(m) => m,
                            None => {
                                return Err(anyhow::anyhow!("Not enough messages."));
                            }
                        };
                        let mut message = m.message.clone();
                        if let Some(repl) = replacement {
                            for (k, v) in &repl.map {
                                message = message.replace(k, v);
                            }
                        }
                        let mut node = node.clone();
                        node.tag_set_attr(lan, Some(message));
                        let block = &mut output[current_line];
                        if let ParsedLine::Line(txt_line) = block {
                            txt_line[i] = node;
                        } else {
                            return Err(anyhow::anyhow!("selectbtn tag not in line: {:?}", block));
                        }
                        mess = mes.next();
                    } else if node.is_tag("/selectbtn") {
                        is_selectblk = false;
                    } else if in_lang_block {
                        // Do nothing
                    } else if droped_lang_block {
                        // Drop the message if we are in a dropped lang block
                    } else if is_selectblk {
                        // Ignore other nodes in select block
                    } else if let TxtLineNode::Tag(tag) = node {
                        if !tag.is_blocked_name(&self.blacklist_names) {
                            last_tag_block_loc = Some((current_line, i));
                        }
                    }
                }
            }
            current_line += 1;
        }
        let s = output.serialize();
        let encoded = encode_string(encoding, &s, false)?;
        file.write_all(&encoded)?;
        file.flush()?;
        Ok(())
    }
}

/// Reads tags list from tag.ini file.
pub fn read_tags_from_ini<P: AsRef<std::path::Path>>(path: P) -> Result<HashSet<String>> {
    let conf = ini::Ini::load_from_file(path)?;
    let set = HashSet::from_iter(conf.sections().flat_map(|s| s.map(|s| s.to_string())));
    eprintln!(
        "Read tags from ini: {}",
        set.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(",")
    );
    Ok(set)
}

#[test]
fn test_xml_parser() {
    let data = "测试文本\nok<r a=\"b\">测试<b o=\"文本\n换行\">";
    let data = XMLTextParser::new(data, "en", false).parse().unwrap();
    assert_eq!(
        data,
        vec![
            ParsedLine::Line(TxtLine(vec![
                TxtLineNode::Tag(TagNode {
                    name: "lang".to_string(),
                    attributes: vec![("en".to_string(), None)],
                }),
                TxtLineNode::Text(TextNode("测试文本".to_string())),
                TxtLineNode::Tag(TagNode {
                    name: "rt2".to_string(),
                    attributes: vec![],
                }),
            ])),
            ParsedLine::Line(TxtLine(vec![
                TxtLineNode::Text(TextNode("ok".to_string())),
                TxtLineNode::Tag(TagNode {
                    name: "r".to_string(),
                    attributes: vec![("a".to_string(), Some("b".to_string()))],
                }),
                TxtLineNode::Text(TextNode("测试".to_string())),
                TxtLineNode::Tag(TagNode {
                    name: "b".to_string(),
                    attributes: vec![("o".to_string(), Some("文本\n换行".to_string()))],
                }),
                TxtLineNode::Tag(TagNode {
                    name: "/lang".to_string(),
                    attributes: vec![],
                }),
            ])),
        ],
    );
}
