mod dump;
mod parser;
mod text;
mod types;

use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use anyhow::Result;
use std::io::Write;
use types::*;

#[derive(Debug)]
pub struct AstScriptBuilder {}

impl AstScriptBuilder {
    pub fn new() -> Self {
        AstScriptBuilder {}
    }
}

impl ScriptBuilder for AstScriptBuilder {
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
        Ok(Box::new(AstScript::new(buf, encoding, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ast"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::Artemis
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        let parser = parser::Parser::new(&buf[..buf_len], Encoding::Utf8);
        if parser.try_parse_header().is_ok() {
            Some(15)
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct AstScript {
    ast: AstFile,
    indent: Option<usize>,
    max_line_width: usize,
    no_indent: bool,
    lang: Option<String>,
}

impl AstScript {
    pub fn new(buf: Vec<u8>, encoding: Encoding, config: &ExtraConfig) -> Result<Self> {
        let parser = parser::Parser::new(&buf, encoding);
        let ast = parser.parse()?;
        Ok(AstScript {
            ast,
            indent: config.artemis_indent,
            max_line_width: config.artemis_max_line_width,
            no_indent: config.artemis_no_indent,
            lang: config.artemis_ast_lang.clone(),
        })
    }
}

impl Script for AstScript {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Json
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn extract_messages(&self) -> Result<Vec<Message>> {
        let mut messages = Vec::new();
        let ast = &self.ast.ast;
        let mut block_name = ast["label"]["top"]["block"]
            .as_str()
            .ok_or(anyhow::anyhow!("Missing top block name"))?;
        let mut block = &ast[block_name];
        let mut lang: Option<&str> = self.lang.as_ref().map(|s| s.as_str());
        loop {
            let savetitle = &block[Key("savetitle")];
            if savetitle.is_array() {
                if let Some(lang) = lang {
                    if let Some(title) = savetitle[lang].as_str() {
                        messages.push(Message {
                            name: None,
                            message: title.to_string(),
                        });
                    } else if let Some(title) = savetitle["text"].as_str() {
                        messages.push(Message {
                            name: None,
                            message: title.to_string(),
                        });
                    }
                } else if let Some(title) = savetitle["text"].as_str() {
                    messages.push(Message {
                        name: None,
                        message: title.to_string(),
                    });
                }
            }
            let text = &block["text"];
            if text.is_array() {
                let lan = match lang {
                    Some(l) => l,
                    None => {
                        for l in text.kv_keys() {
                            if l != "vo" {
                                lang = Some(l);
                                break;
                            }
                        }
                        match lang {
                            Some(l) => l,
                            // No text found, continue to next block
                            None => {
                                block_name = match block["linknext"].as_str() {
                                    Some(name) => name,
                                    None => break,
                                };
                                block = &ast[block_name];
                                continue;
                            }
                        }
                    }
                };
                let mut tex = &text[lan];
                if tex.is_null() {
                    for l in text.kv_keys() {
                        if l != "vo" {
                            tex = &text[l];
                            break;
                        }
                    }
                }
                for item in tex.members() {
                    let name = item["name"].last_member().as_string();
                    let message = text::TextGenerator::new().generate(item)?;
                    messages.push(Message {
                        name: name,
                        message: message
                            .replace("<rt2>", "\n")
                            .replace("<ret2>", "\n")
                            .trim_end_matches("\n")
                            .to_string(),
                    });
                }
            }
            let select = &block["select"];
            if select.is_array() {
                let lan = match lang {
                    Some(l) => l,
                    None => {
                        for l in select.kv_keys() {
                            if l != "vo" {
                                lang = Some(l);
                                break;
                            }
                        }
                        match lang {
                            Some(l) => l,
                            // No select text found, continue to next block
                            None => {
                                block_name = match block["linknext"].as_str() {
                                    Some(name) => name,
                                    None => break,
                                };
                                block = &ast[block_name];
                                continue;
                            }
                        }
                    }
                };
                let mut select_text = &select[lan];
                if select_text.is_null() {
                    for l in select.kv_keys() {
                        if l != "vo" {
                            select_text = &select[l];
                            break;
                        }
                    }
                }
                for item in select_text.members() {
                    if let Some(select) = item.as_str() {
                        messages.push(Message {
                            name: None,
                            message: select.to_string(),
                        });
                    }
                }
            }
            block_name = match block["linknext"].as_str() {
                Some(name) => name,
                None => break,
            };
            block = &ast[block_name];
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
        let mut ast = self.ast.clone();
        let root = &mut ast.ast;
        let mut block_name = root["label"]["top"]["block"]
            .as_string()
            .ok_or(anyhow::anyhow!("Missing top block name"))?;
        let mut block = &mut root[block_name];
        let mut mess = messages.iter();
        let mut mes = mess.next();
        let mut lang = self.lang.as_ref().map(|s| s.to_string());
        loop {
            if block[Key("savetitle")].is_array() {
                let lan = lang.as_ref().map(|s| s.as_str()).unwrap_or("text");
                let m = match mes {
                    Some(m) => m,
                    None => return Err(anyhow::anyhow!("Not enough messages.")),
                };
                let mut title = m.message.clone();
                if let Some(repl) = replacement {
                    for (k, v) in &repl.map {
                        title = title.replace(k, v);
                    }
                }
                block[Key("savetitle")][lan].set_string(title);
                mes = mess.next();
            }
            if block["text"].is_array() {
                let lan = match &lang {
                    Some(l) => l.as_str(),
                    None => {
                        for l in block["text"].kv_keys() {
                            if l != "vo" {
                                lang = Some(l.to_string());
                                break;
                            }
                        }
                        match lang {
                            Some(ref l) => l.as_str(),
                            // No text found, continue to next block
                            None => {
                                block_name = match block["linknext"].as_string() {
                                    Some(name) => name,
                                    None => break,
                                };
                                block = &mut root[block_name];
                                continue;
                            }
                        }
                    }
                };
                let origin_names: Vec<_> = {
                    let mut tex = &block["text"][lan];
                    if tex.is_null() {
                        for l in block["text"].kv_keys() {
                            if l != "vo" {
                                tex = &block["text"][l];
                                break;
                            }
                        }
                    }
                    tex.members().map(|m| m["name"].clone()).collect()
                };
                let mut arr = Value::new_array();
                for name in origin_names {
                    let m = match mes {
                        Some(m) => m,
                        None => return Err(anyhow::anyhow!("Not enough messages.")),
                    };
                    let mut text = m.message.clone();
                    if let Some(repl) = replacement {
                        for (k, v) in &repl.map {
                            text = text.replace(k, v);
                        }
                    }
                    if !text.ends_with("\n") {
                        text.push('\n');
                    }
                    let mut v = text::TextParser::new(&text.replace("\n", "<rt2>")).parse()?;
                    if name.is_array() {
                        let mut n = match &m.name {
                            Some(n) => n.to_string(),
                            None => return Err(anyhow::anyhow!("Message name is missing.")),
                        };
                        if let Some(repl) = replacement {
                            for (k, v) in &repl.map {
                                n = n.replace(k, v);
                            }
                        }
                        v.insert_member(0, Value::new_kv("name", name));
                        if v["name"].len() <= 1 {
                            if v["name"][0] != n {
                                v["name"].push_member(Value::Str(n));
                            }
                        } else {
                            v["name"].last_member_mut().set_string(n);
                        }
                    }
                    arr.push_member(v);
                    mes = mess.next();
                }
                block["text"][lan] = arr;
            }
            if block["select"].is_array() {
                let lan = match &lang {
                    Some(l) => l.as_str(),
                    None => {
                        for l in block["select"].kv_keys() {
                            if l != "vo" {
                                lang = Some(l.to_string());
                                break;
                            }
                        }
                        match lang {
                            Some(ref l) => l.as_str(),
                            // No text found, continue to next block
                            None => {
                                block_name = match block["linknext"].as_string() {
                                    Some(name) => name,
                                    None => break,
                                };
                                block = &mut root[block_name];
                                continue;
                            }
                        }
                    }
                };
                let select_count = {
                    let mut select = &block["select"][lan];
                    if select.is_null() {
                        for l in block["select"].kv_keys() {
                            if l != "vo" {
                                select = &block["select"][l];
                                break;
                            }
                        }
                    }
                    select.len()
                };
                let mut new_select = Value::new_array();
                for _ in 0..select_count {
                    let m = match mes {
                        Some(m) => m,
                        None => return Err(anyhow::anyhow!("Not enough messages.")),
                    };
                    let mut select_text = m.message.clone();
                    if let Some(repl) = replacement {
                        for (k, v) in &repl.map {
                            select_text = select_text.replace(k, v);
                        }
                    }
                    new_select.push_member(Value::Str(select_text));
                    mes = mess.next();
                }
                block["select"][lan] = new_select;
            }
            block_name = match block["linknext"].as_string() {
                Some(name) => name,
                None => break,
            };
            block = &mut root[block_name];
        }
        if mes.is_some() || mess.next().is_some() {
            return Err(anyhow::anyhow!("Not all messages were used."));
        }
        let mut writer = Vec::new();
        let mut dumper = dump::Dumper::new(&mut writer);
        if self.no_indent {
            dumper.set_no_indent();
        } else if let Some(indent) = self.indent {
            dumper.set_indent(indent);
        }
        dumper.set_max_line_width(self.max_line_width);
        dumper.dump(&ast)?;
        let data = String::from_utf8(writer)?;
        let encoded = encode_string(encoding, &data, false)?;
        file.write_all(&encoded)?;
        file.flush()?;
        Ok(())
    }
}

pub fn is_this_format(_filename: &str, buf: &[u8], buf_len: usize) -> bool {
    let parser = parser::Parser::new(&buf[..buf_len], Encoding::Utf8);
    parser.try_parse_header().is_ok()
}
