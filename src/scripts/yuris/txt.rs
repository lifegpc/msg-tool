//! Yu-Ris scenario text file (.txt)
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use anyhow::Result;
use std::ops::{Deref, DerefMut};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug)]
pub struct YurisTxtBuilder {}

impl YurisTxtBuilder {
    /// Creates a new instance of `YurisTxtBuilder`
    pub const fn new() -> Self {
        YurisTxtBuilder {}
    }
}

impl ScriptBuilder for YurisTxtBuilder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Cp932
    }

    fn build_script(
        &self,
        buf: Vec<u8>,
        _filename: &str,
        encoding: Encoding,
        _archive_encoding: Encoding,
        _config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script + Send + Sync>> {
        Ok(Box::new(YurisTxt::new(&buf, encoding)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["txt"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::YurisTxt
    }
}

trait INode {
    fn serialize(&self) -> String;
}

#[derive(Clone, Debug, PartialEq)]
struct CommentNode(String);

impl Deref for CommentNode {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for CommentNode {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl INode for CommentNode {
    fn serialize(&self) -> String {
        format!("//{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq)]
struct LabelNode(String);

impl Deref for LabelNode {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for LabelNode {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl INode for LabelNode {
    fn serialize(&self) -> String {
        format!("#{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq)]
struct CommandNode {
    name: String,
    args: Vec<String>,
    has_args: bool,
}

impl INode for CommandNode {
    fn serialize(&self) -> String {
        if !self.has_args {
            return format!("\\{}", self.name);
        }
        let mut s = format!("\\{}(", self.name);
        let mut first = true;
        for arg in &self.args {
            if first {
                first = false;
            } else {
                s.push_str(", ");
            }
            if arg.contains(" ") || arg.contains(",") {
                s.push_str(&format!("\"{}\"", arg));
            } else {
                s.push_str(arg);
            }
        }
        s.push(')');
        s
    }
}

#[derive(Clone, Debug, PartialEq)]
struct NameNode(String);

impl Deref for NameNode {
    type Target = String;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for NameNode {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl INode for NameNode {
    fn serialize(&self) -> String {
        format!("【{}】", self.0)
    }
}

#[derive(Clone, Debug, PartialEq)]
struct TextNode(String);

impl Deref for TextNode {
    type Target = String;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for TextNode {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl INode for TextNode {
    fn serialize(&self) -> String {
        self.0.clone()
    }
}

#[derive(Clone, Debug, PartialEq)]
struct CommentBlock(String);

impl Deref for CommentBlock {
    type Target = String;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for CommentBlock {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl INode for CommentBlock {
    fn serialize(&self) -> String {
        format!("/*{}*/", self.0)
    }
}

#[derive(Clone, Debug, PartialEq)]
enum LineNode {
    Comment(CommentNode),
    Comments(CommentBlock),
    Command(CommandNode),
    Name(NameNode),
    Text(TextNode),
}

impl INode for LineNode {
    fn serialize(&self) -> String {
        match self {
            Self::Comment(node) => node.serialize(),
            Self::Comments(node) => node.serialize(),
            Self::Command(node) => node.serialize(),
            Self::Name(node) => node.serialize(),
            Self::Text(node) => node.serialize(),
        }
    }
}

impl INode for Vec<LineNode> {
    fn serialize(&self) -> String {
        self.iter()
            .map(|s| s.serialize())
            .collect::<Vec<_>>()
            .join("")
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Line {
    Line(Vec<LineNode>),
    Empty,
    Label(LabelNode),
}

impl INode for Line {
    fn serialize(&self) -> String {
        match self {
            Self::Line(line) => line.serialize(),
            Self::Empty => String::new(),
            Self::Label(label) => label.serialize(),
        }
    }
}

impl INode for Vec<Line> {
    fn serialize(&self) -> String {
        self.iter()
            .map(|s| s.serialize())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[derive(Debug)]
struct Parser<'a> {
    lines: std::str::Lines<'a>,
    cur_line: &'a str,
    cur_pos: usize,
    line_num: u64,
    cur_line_chars: Vec<&'a str>,
}

impl<'a> Parser<'a> {
    fn new(data: &'a str) -> Self {
        Self {
            lines: data.lines(),
            cur_line: "",
            cur_pos: 0,
            line_num: 0,
            cur_line_chars: Vec::new(),
        }
    }

    fn error(&self, msg: impl std::fmt::Display) -> anyhow::Error {
        anyhow::anyhow!("{} at line {} char {}", msg, self.line_num, self.cur_pos)
    }

    fn parse(mut self) -> Result<Vec<Line>> {
        let mut lines = Vec::new();
        while let Some(line) = self.lines.next() {
            self.line_num += 1;
            self.cur_line = line;
            self.cur_line_chars = line.graphemes(true).collect();
            lines.push(self.parse_line()?);
        }
        Ok(lines)
    }

    fn add_next_line(&mut self) -> Result<()> {
        let next_line = self
            .lines
            .next()
            .ok_or_else(|| anyhow::anyhow!("Unexpected eof"))?;
        self.line_num += 1;
        self.cur_line = next_line;
        self.cur_line_chars.push("\n");
        self.cur_line_chars.extend(next_line.graphemes(true));
        Ok(())
    }

    fn parse_line(&mut self) -> Result<Line> {
        self.cur_pos = 0;
        if self.cur_line.trim_matches(' ').is_empty() {
            return Ok(Line::Empty);
        }
        let mut line = Vec::new();
        let mut text = String::new();
        while let Some(c) = self.peek_char() {
            // Skip space if text is empty
            if text.is_empty() && (c == " " || c == "\t") {
                self.cur_pos += 1;
                continue;
            }
            // Label
            if line.is_empty() && c == "#" {
                self.cur_pos += 1;
                let label = self.cur_line_chars[self.cur_pos..].join("");
                return Ok(Line::Label(LabelNode(label)));
            }
            if c == "/" {
                // Comment
                if let Some(c) = self.peek_char_offset(1) {
                    if c == "/" {
                        let ctext = text.trim_end_matches(' ').trim_end_matches('\t');
                        if !ctext.is_empty() {
                            line.push(LineNode::Text(TextNode(ctext.to_owned())));
                            text.clear();
                        }
                        self.cur_pos += 2;
                        let comment = self.cur_line_chars[self.cur_pos..].join("");
                        line.push(LineNode::Comment(CommentNode(comment)));
                        break;
                    } else if c == "*" {
                        let ctext = text.trim_end_matches(' ').trim_end_matches('\t');
                        if !ctext.is_empty() {
                            line.push(LineNode::Text(TextNode(ctext.to_owned())));
                            text.clear();
                        }
                        self.cur_pos += 2;
                        let start_pos = self.cur_pos;
                        let mut ok = false;
                        loop {
                            while let Some(c) = self.next_char() {
                                if c == "*" && self.peek_char().is_some_and(|c| c == "/") {
                                    let end_pos = self.cur_pos - 1;
                                    self.cur_pos += 1;
                                    ok = true;
                                    line.push(LineNode::Comments(CommentBlock(
                                        self.cur_line_chars[start_pos..end_pos].join(""),
                                    )));
                                    break;
                                }
                            }
                            if ok {
                                break;
                            }
                            self.add_next_line()?;
                        }
                        continue;
                    }
                }
            }
            // command
            if c == "\\" {
                // check \R
                if self.peek_char_offset(1).is_some_and(|c| c == "R") {
                    self.cur_pos += 2;
                    text.push_str("\\R");
                    continue;
                }
                let ctext = text.trim_end_matches(' ').trim_end_matches('\t');
                if !ctext.is_empty() {
                    line.push(LineNode::Text(TextNode(ctext.to_owned())));
                    text.clear();
                }
                line.push(LineNode::Command(self.parse_command()?));
                continue;
            }
            // name
            if c == "【" {
                let ctext = text.trim_end_matches(' ').trim_end_matches('\t');
                if !ctext.is_empty() {
                    line.push(LineNode::Text(TextNode(ctext.to_owned())));
                    text.clear();
                }
                line.push(LineNode::Name(self.parse_name()?));
                continue;
            }
            text.push_str(c);
            self.cur_pos += 1;
        }
        let ctext = text.trim_end_matches(' ').trim_end_matches('\t');
        if !ctext.is_empty() {
            line.push(LineNode::Text(TextNode(ctext.to_owned())));
        }
        Ok(Line::Line(line))
    }

    fn parse_command(&mut self) -> Result<CommandNode> {
        let c = self
            .next_char()
            .ok_or_else(|| self.error("Unexpected end of line"))?;
        if c != "\\" {
            return Err(self.error("Unexpected command start token"));
        }
        let mut name = String::new();
        let mut args = Vec::new();
        let mut in_quote = false;
        let mut arg = String::new();
        let mut ok = false;
        while let Some(c) = self.peek_char() {
            if c == "(" {
                ok = true;
                self.cur_pos += 1;
                break;
            }
            if c == ")" {
                return Err(self.error("Unexpected ) when parsing command"));
            }
            if !c.is_ascii() {
                break;
            }
            name.push_str(c);
            self.cur_pos += 1;
            continue;
        }
        if !ok {
            return Ok(CommandNode {
                name: name.trim_matches(' ').trim_matches('\t').to_owned(),
                args: Vec::new(),
                has_args: false,
            });
        }
        loop {
            let c = self
                .next_char()
                .ok_or_else(|| self.error("Unexpected end of line when parsing command"))?;
            if in_quote {
                if c == "\"" {
                    in_quote = false;
                    continue;
                }
            } else {
                if c == "\"" {
                    in_quote = true;
                    continue;
                }
                if c == " " || c == "\t" {
                    if arg.is_empty() {
                        continue;
                    }
                    let mut tmp = c.to_string();
                    while let Some(c) = self.peek_char() {
                        if c == " " || c == "\t" {
                            self.cur_pos += 1;
                            tmp.push_str(c);
                        } else if c == "," || c == ")" {
                            break;
                        } else {
                            arg.push_str(&tmp);
                            break;
                        }
                    }
                    continue;
                }
                if c == "," {
                    args.push(arg);
                    arg = String::new();
                    continue;
                }
                if c == ")" {
                    args.push(arg);
                    return Ok(CommandNode {
                        name: name.trim_matches(' ').trim_matches('\t').to_owned(),
                        args,
                        has_args: true,
                    });
                }
            }
            arg.push_str(c);
        }
    }

    fn parse_name(&mut self) -> Result<NameNode> {
        let c = self
            .next_char()
            .ok_or_else(|| self.error("Unexpected end of line"))?;
        if c != "【" {
            return Err(self.error("Unexpected command start token"));
        }
        let mut name = String::new();
        loop {
            let c = self
                .next_char()
                .ok_or_else(|| self.error("Unexpected end of line when parsing name"))?;
            if c == "】" {
                return Ok(NameNode(name));
            }
            name.push_str(c);
        }
    }

    fn peek_char(&self) -> Option<&'a str> {
        self.cur_line_chars.get(self.cur_pos).map(|s| *s)
    }

    fn peek_char_offset(&self, offset: isize) -> Option<&'a str> {
        let target = (self.cur_pos as isize + offset as isize) as usize;
        self.cur_line_chars.get(target).map(|s| *s)
    }

    fn next_char(&mut self) -> Option<&'a str> {
        let t = self.cur_line_chars.get(self.cur_pos).map(|s| *s);
        if t.is_some() {
            self.cur_pos += 1;
        }
        t
    }
}

#[derive(Debug)]
pub struct YurisTxt {
    data: Vec<Line>,
    bom: BomType,
}

impl YurisTxt {
    pub fn new<D: AsRef<[u8]> + ?Sized>(data: &D, encoding: Encoding) -> Result<Self> {
        let (text, bom) = decode_with_bom_detect(encoding, data.as_ref(), true)?;
        let data = Parser::new(&text).parse()?;
        Ok(Self { data, bom })
    }
}

impl Script for YurisTxt {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Json
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn extract_messages(&self) -> Result<Vec<Message>> {
        let mut messages = Vec::new();
        for line in &self.data {
            if let Line::Line(line) = line {
                let mut name = None;
                let mut message = String::new();
                for node in line.iter() {
                    if let LineNode::Name(n) = node {
                        name = Some(n.as_str());
                    } else if let LineNode::Text(text) = node {
                        message.push_str(&text.replace("\\R", "\n"));
                    } else if let LineNode::Command(cmd) = node {
                        if !message.is_empty() {
                            message.push_str(&cmd.serialize());
                        }
                        if cmd.name == "SEL" {
                            for arg in &cmd.args {
                                messages.push(Message::new(arg.to_owned(), None));
                            }
                        }
                    }
                }
                if !message.is_empty() {
                    messages.push(Message::new(message, name.map(|s| s.to_owned())));
                }
            }
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
        let mut data = self.data.clone();
        let mut mess = messages.iter();
        let mut mes = mess.next();
        for line in data.iter_mut() {
            if let Line::Line(line) = line {
                let mut name_index = None;
                let mut message_index = None;
                for (i, node) in line.iter_mut().enumerate() {
                    if let LineNode::Name(_) = node {
                        name_index = Some(i);
                    } else if let LineNode::Text(_) = node {
                        if message_index.is_none() {
                            message_index = Some(i);
                        }
                    } else if let LineNode::Command(cmd) = node {
                        if cmd.name == "SEL" {
                            for arg in cmd.args.iter_mut() {
                                let mut m = mes
                                    .ok_or_else(|| anyhow::anyhow!("No more messages to import"))?
                                    .message
                                    .clone();
                                mes = mess.next();
                                if let Some(rep) = replacement {
                                    for (k, v) in &rep.map {
                                        m = m.replace(k, v);
                                    }
                                }
                                *arg = m;
                            }
                        }
                    }
                }
                if let Some(message_idx) = message_index {
                    let m = mes.ok_or_else(|| anyhow::anyhow!("No more messages to import"))?;
                    mes = mess.next();
                    if let Some(name_idx) = name_index {
                        let mut name = m
                            .name
                            .as_ref()
                            .ok_or_else(|| anyhow::anyhow!("Message don't have name"))?
                            .clone();
                        if let Some(rep) = replacement {
                            for (k, v) in &rep.map {
                                name = name.replace(k, v);
                            }
                        }
                        if let LineNode::Name(n) = &mut line[name_idx] {
                            n.0 = name;
                        }
                    }
                    let mut m = m.message.replace("\n", "\\R");
                    if let Some(rep) = replacement {
                        for (k, v) in &rep.map {
                            m = m.replace(k, v);
                        }
                    }
                    let data = Parser::new(&m).parse()?;
                    if data.len() != 1 {
                        anyhow::bail!("parsed length is not 1.");
                    }
                    let li = data[0].clone();
                    match li {
                        Line::Label(_) => anyhow::bail!("Unsupported"),
                        Line::Empty => {
                            line.splice(message_idx.., []);
                        }
                        Line::Line(li) => {
                            line.splice(message_idx.., li);
                        }
                    }
                }
            }
        }
        if mes.is_some() || mess.next().is_some() {
            return Err(anyhow::anyhow!("Some messages were not processed."));
        }
        let data = data.serialize();
        let data = encode_string_with_bom(encoding, &data, false, self.bom)?;
        file.write_all(&data)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_parse1() {
        let data = "\\T( , 250 ) \t【name】\t「……なんて」\t";
        assert_eq!(
            Parser::new(data).parse().unwrap(),
            vec![Line::Line(vec![
                LineNode::Command(CommandNode {
                    name: "T".into(),
                    args: vec!["".into(), "250".into()],
                    has_args: true,
                }),
                LineNode::Name(NameNode("name".into())),
                LineNode::Text(TextNode("「……なんて」".into())),
            ])]
        );
    }
    #[test]
    fn test_parse2() {
        let data = "\\T(2 5 \t0\t ) //TEST\n\\T ( \"250 \" , \"Wor,ks\" )";
        assert_eq!(
            Parser::new(data).parse().unwrap(),
            vec![
                Line::Line(vec![
                    LineNode::Command(CommandNode {
                        name: "T".into(),
                        args: vec!["2 5 \t0".into()],
                        has_args: true,
                    }),
                    LineNode::Comment(CommentNode("TEST".into()))
                ]),
                Line::Line(vec![LineNode::Command(CommandNode {
                    name: "T".into(),
                    args: vec!["250 ".into(), "Wor,ks".into()],
                    has_args: true,
                }),])
            ]
        );
    }
    #[test]
    fn test_parse3() {
        let data = "\\VO(UDA_0_ALL_0007_0004)【ウダツ】「んで、昨日あの後どうしたん？　実習の日程はもう決まった\\Rのか？」";
        assert_eq!(
            Parser::new(data).parse().unwrap(),
            vec![Line::Line(vec![
                LineNode::Command(CommandNode {
                    name: "VO".into(),
                    args: vec!["UDA_0_ALL_0007_0004".into()],
                    has_args: true,
                }),
                LineNode::Name(NameNode("ウダツ".into())),
                LineNode::Text(TextNode(
                    "「んで、昨日あの後どうしたん？　実習の日程はもう決まった\\Rのか？」".into()
                )),
            ])]
        );
    }
    #[test]
    fn test_parse4() {
        let data = "\\GO.TITLE";
        assert_eq!(
            Parser::new(data).parse().unwrap(),
            vec![Line::Line(vec![LineNode::Command(CommandNode {
                name: "GO.TITLE".into(),
                args: vec![],
                has_args: false,
            }),])]
        );
    }
    #[test]
    fn test_parse5() {
        let data = r"TEST/*
\FOUT(600, 42, white)
\BG.CMXYZ(  402,    0, -45)
\BG(bg51 , 260, 0, 0)
\PSET(回想フレーム, 0)
\FIN(600, 41)
*/Test";
        assert_eq!(
            Parser::new(data).parse().unwrap(),
            vec![Line::Line(vec![
                LineNode::Text(TextNode("TEST".into())),
                LineNode::Comments(CommentBlock(
                    r"
\FOUT(600, 42, white)
\BG.CMXYZ(  402,    0, -45)
\BG(bg51 , 260, 0, 0)
\PSET(回想フレーム, 0)
\FIN(600, 41)
"
                    .into()
                )),
                LineNode::Text(TextNode("Test".into())),
            ])]
        );
    }
}
