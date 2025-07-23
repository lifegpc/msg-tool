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
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(AstScript::new(buf, encoding, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ast"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::Artemis
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
                            }
                        }
                        match lang {
                            Some(l) => l,
                            // No text found, continue to next block
                            None => continue,
                        }
                    }
                };
                let tex = &text[lan];
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
            // #TODO: SELECTS
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
        _messages: Vec<Message>,
        mut file: Box<dyn WriteSeek + 'a>,
        encoding: Encoding,
        _replacement: Option<&'a ReplacementTable>,
    ) -> Result<()> {
        let ast = self.ast.clone();
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
