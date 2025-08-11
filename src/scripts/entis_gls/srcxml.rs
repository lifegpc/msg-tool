//! Entis GLS engine XML Script (.srcxml)
use crate::ext::io::*;
use crate::ext::rcdom::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use anyhow::Result;
use markup5ever_rcdom::{Handle, RcDom, SerializableHandle};
use xml5ever::driver::parse_document;
use xml5ever::serialize::serialize;
use xml5ever::tendril::TendrilSink;

#[derive(Debug)]
/// A builder for Entis GLS srcxml scripts.
pub struct SrcXmlScriptBuilder {}

impl SrcXmlScriptBuilder {
    /// Creates a new instance of `SrcXmlScriptBuilder`.
    pub fn new() -> Self {
        Self {}
    }
}

impl ScriptBuilder for SrcXmlScriptBuilder {
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
        Ok(Box::new(SrcXmlScript::new(buf, encoding, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["srcxml"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::EntisGls
    }
}

pub struct SrcXmlScript {
    decoded: String,
    handle: Handle,
    lang: Option<String>,
}

impl std::fmt::Debug for SrcXmlScript {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SrcXmlScript")
            .field("handle", &self.handle)
            .field("lang", &self.lang)
            .finish()
    }
}

impl SrcXmlScript {
    pub fn new(buf: Vec<u8>, encoding: Encoding, config: &ExtraConfig) -> Result<Self> {
        let decoded = decode_to_string(encoding, &buf, false)?;
        let dom = parse_document(RcDom::default(), Default::default())
            .from_utf8()
            .one(decoded.as_bytes());
        {
            let error = dom.errors.try_borrow()?;
            for e in error.iter() {
                eprintln!("WARN: Error parsing srcxml: {}", e);
                crate::COUNTER.inc_warning();
            }
        }
        Ok(Self {
            decoded,
            handle: dom.document,
            lang: config.entis_gls_srcxml_lang.clone(),
        })
    }
}

impl Script for SrcXmlScript {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Json
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn extract_messages(&self) -> Result<Vec<Message>> {
        let mut messages = Vec::new();
        let mut lang = self.lang.clone();
        for i in self.handle.children.try_borrow()?.iter() {
            if i.is_element("xscript") {
                for code in i.children.try_borrow()?.iter() {
                    if code.is_element("code") {
                        for ins in code.children.try_borrow()?.iter() {
                            if ins.is_element("msg") {
                                let lan = match lang.as_ref() {
                                    Some(l) => l.as_str(),
                                    None => {
                                        for attr in ins.element_attr_keys()? {
                                            if attr.starts_with("name_")
                                                || attr.starts_with("text_")
                                            {
                                                lang = Some(attr[5..].to_string());
                                                break;
                                            }
                                        }
                                        lang.as_ref().map(|s| s.as_str()).unwrap_or("")
                                    }
                                };
                                let name_ref = if lan.is_empty() {
                                    "name"
                                } else {
                                    &format!("name_{}", lan)
                                };
                                let mut name = ins.get_attr_value(name_ref)?;
                                if name.as_ref().is_some_and(|s| s.is_empty()) {
                                    name = None;
                                }
                                let text_ref = if lan.is_empty() {
                                    "text"
                                } else {
                                    &format!("text_{}", lan)
                                };
                                let message = ins
                                    .get_attr_value(text_ref)?
                                    .ok_or(anyhow::anyhow!("text not found"))?;
                                messages.push(Message { name, message })
                            } else if ins.is_element("select") {
                                for menu in ins.children.try_borrow()?.iter() {
                                    if menu.is_element("menu") {
                                        let lan = match lang.as_ref() {
                                            Some(l) => l.as_str(),
                                            None => {
                                                for attr in ins.element_attr_keys()? {
                                                    if attr.starts_with("name_")
                                                        || attr.starts_with("text_")
                                                    {
                                                        lang = Some(attr[5..].to_string());
                                                        break;
                                                    }
                                                }
                                                lang.as_ref().map(|s| s.as_str()).unwrap_or("")
                                            }
                                        };
                                        let text_ref = if lan.is_empty() {
                                            "text"
                                        } else {
                                            &format!("text_{}", lan)
                                        };
                                        let message = menu
                                            .get_attr_value(text_ref)?
                                            .ok_or(anyhow::anyhow!("text not found"))?;
                                        messages.push(Message {
                                            name: None,
                                            message,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
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
        let dom = parse_document(RcDom::default(), Default::default())
            .from_utf8()
            .one(self.decoded.as_bytes());
        let root = dom.document;
        if !encoding.is_utf8() {
            let mut childrens = root.children.try_borrow_mut()?;
            if childrens.len() > 1 && childrens[0].is_processing_instruction("xml") {
                childrens.remove(0);
            }
        }
        let doc: SerializableHandle = root.clone().into();
        let mut output = MemWriter::new();
        serialize(&mut output, &doc, Default::default())
            .map_err(|e| anyhow::anyhow!("Error serializing srcxml: {}", e))?;
        if encoding.is_utf8() {
            file.write_all(&output.data)?;
            return Ok(());
        }
        let s = decode_to_string(Encoding::Utf8, &output.data, true)?;
        let encoded = encode_string(encoding, &s, false)?;
        file.write_all(&encoded)?;
        Ok(())
    }
}
