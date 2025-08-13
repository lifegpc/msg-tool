//! Buriko General Interpreter/Ethornell Script
use super::parser::*;
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::{decode_to_string, encode_string};
use anyhow::Result;
use fancy_regex::Regex;
use lazy_static::lazy_static;
use std::collections::{BTreeMap, HashMap};

#[derive(Debug)]
/// Builder for BGI scripts.
pub struct BGIScriptBuilder {}

impl BGIScriptBuilder {
    /// Creates a new instance of `BGIScriptBuilder`.
    pub fn new() -> Self {
        BGIScriptBuilder {}
    }
}

impl ScriptBuilder for BGIScriptBuilder {
    fn default_encoding(&self) -> Encoding {
        #[cfg(not(windows))]
        return Encoding::Cp932;
        #[cfg(windows)]
        // Use Windows API first, because encoding-rs does not support PRIVATE USE AREA characters
        return Encoding::CodePage(932);
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
        Ok(Box::new(BGIScript::new(buf, encoding, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::BGI
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len > 28 && buf.starts_with(b"BurikoCompiledScriptVer1.00\0") {
            return Some(255);
        }
        None
    }
}

/// BGI Script
pub struct BGIScript {
    data: MemReader,
    encoding: Encoding,
    strings: Vec<BGIString>,
    is_v1: bool,
    is_v1_instr: bool,
    offset: usize,
    import_duplicate: bool,
    append: bool,
}

impl std::fmt::Debug for BGIScript {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BGIScript")
            .field("encoding", &self.encoding)
            .finish_non_exhaustive()
    }
}

impl BGIScript {
    /// Creates a new instance of `BGIScript` from a buffer.
    ///
    /// * `data` - The buffer containing the script data.
    /// * `encoding` - The encoding of the script.
    /// * `config` - Extra configuration options.
    pub fn new(data: Vec<u8>, encoding: Encoding, config: &ExtraConfig) -> Result<Self> {
        let data = MemReader::new(data);
        if data.data.starts_with(b"BurikoCompiledScriptVer1.00\0") {
            let mut parser = V1Parser::new(data.to_ref(), encoding)?;
            parser.disassemble()?;
            let strings = parser.strings.clone();
            let offset = parser.offset;
            Ok(Self {
                data,
                encoding,
                strings,
                is_v1: true,
                is_v1_instr: true,
                offset,
                import_duplicate: config.bgi_import_duplicate,
                append: !config.bgi_disable_append,
            })
        } else {
            let mut is_v1_instr = false;
            let strings = {
                let mut parser = V0Parser::new(data.to_ref());
                match parser.disassemble() {
                    Ok(_) => parser.strings,
                    Err(_) => {
                        let mut parser = V1Parser::new(data.to_ref(), encoding)?;
                        parser.disassemble()?;
                        is_v1_instr = true;
                        parser.strings
                    }
                }
            };
            Ok(Self {
                data,
                encoding,
                strings,
                is_v1: false,
                is_v1_instr,
                offset: 0,
                import_duplicate: config.bgi_import_duplicate,
                append: !config.bgi_disable_append,
            })
        }
    }

    fn read_string(&self, offset: usize) -> Result<String> {
        let start = self.offset + offset;
        let string_data = self.data.cpeek_cstring_at(start as u64)?;
        // sometimes string has private use area characters, so we disable strict checking
        let string = decode_to_string(self.encoding, string_data.as_bytes(), false)?;
        Ok(string)
    }

    fn output_with_ruby(str: &mut String, ruby: &mut Vec<String>) -> Result<()> {
        if ruby.is_empty() {
            return Ok(());
        }
        if ruby.len() % 2 != 0 {
            return Err(anyhow::anyhow!("Ruby strings count is not even."));
        }
        for i in (0..ruby.len()).step_by(2) {
            let ruby_str = &ruby[i];
            let ruby_text = &ruby[i + 1];
            if ruby_str.is_empty() || ruby_text.is_empty() {
                continue;
            }
            *str = str.replace(ruby_str, &format!("<r{ruby_text}>{ruby_str}</r>"));
        }
        ruby.clear();
        Ok(())
    }
}

impl Script for BGIScript {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Json
    }

    fn default_format_type(&self) -> FormatOptions {
        if self.is_v1_instr {
            FormatOptions::None
        } else {
            FormatOptions::Fixed {
                length: 32,
                keep_original: false,
            }
        }
    }

    fn extract_messages(&self) -> Result<Vec<Message>> {
        let mut messages = Vec::new();
        let mut name = None;
        let mut ruby = Vec::new();
        for bgi_string in &self.strings {
            match bgi_string.typ {
                BGIStringType::Name => {
                    name = Some(self.read_string(bgi_string.address)?);
                }
                BGIStringType::Message => {
                    let mut message = self.read_string(bgi_string.address)?;
                    if !ruby.is_empty() {
                        Self::output_with_ruby(&mut message, &mut ruby)?;
                    }
                    messages.push(Message {
                        name: name.take(),
                        message: message,
                    });
                }
                BGIStringType::Ruby => {
                    let ruby_str = self.read_string(bgi_string.address)?;
                    ruby.push(ruby_str);
                }
                _ => {}
            }
        }
        Ok(messages)
    }

    fn import_messages<'a>(
        &'a self,
        mut messages: Vec<Message>,
        mut file: Box<dyn WriteSeek + 'a>,
        encoding: Encoding,
        replacement: Option<&'a ReplacementTable>,
    ) -> Result<()> {
        if !self.import_duplicate {
            let mut used = HashMap::new();
            let mut extra = HashMap::new();
            let mut mes = messages.iter_mut();
            let mut cur_mes = mes.next();
            let mut old_offset = 0;
            let mut new_offset = 0;
            let mut rubys = Vec::new();
            let mut parsed_ruby = false;
            if self.append {
                file.write_all(&self.data.data)?;
                new_offset = self.data.data.len();
            }
            for curs in &self.strings {
                if !curs.is_internal() {
                    if cur_mes.is_none() {
                        cur_mes = mes.next();
                    }
                }
                if used.contains_key(&curs.address) && curs.is_internal() {
                    let (_, new_address) = used.get(&curs.address).unwrap();
                    file.write_u32_at(curs.offset, *new_address as u32)?;
                    continue;
                }
                let nmes = match curs.typ {
                    BGIStringType::Internal => self.read_string(curs.address)?,
                    BGIStringType::Ruby => {
                        if !self.is_v1 && self.is_v1_instr {
                            if rubys.is_empty() {
                                if parsed_ruby {
                                    String::from("<")
                                } else {
                                    rubys = match &mut cur_mes {
                                        Some(m) => parse_ruby_from_text(&mut m.message)?,
                                        None => return Err(anyhow::anyhow!("No enough messages.")),
                                    };
                                    parsed_ruby = true;
                                    if rubys.is_empty() {
                                        String::from("<")
                                    } else {
                                        let ruby_str = rubys.remove(0);
                                        ruby_str
                                    }
                                }
                            } else {
                                rubys.remove(0)
                            }
                        } else {
                            self.read_string(curs.address)?
                        }
                    }
                    BGIStringType::Name => match &cur_mes {
                        Some(m) => {
                            if let Some(name) = &m.name {
                                let mut name = name.clone();
                                if let Some(replacement) = replacement {
                                    for (key, value) in replacement.map.iter() {
                                        name = name.replace(key, value);
                                    }
                                }
                                name
                            } else {
                                return Err(anyhow::anyhow!("Name is missing for message."));
                            }
                        }
                        None => return Err(anyhow::anyhow!("No enough messages.")),
                    },
                    BGIStringType::Message => {
                        if !rubys.is_empty() {
                            eprintln!("Warning: Some ruby strings are unused: {:?}", rubys);
                            crate::COUNTER.inc_warning();
                            rubys.clear();
                        }
                        parsed_ruby = false;
                        let mes = match &cur_mes {
                            Some(m) => {
                                let mut message = m.message.clone();
                                if let Some(replacement) = replacement {
                                    for (key, value) in replacement.map.iter() {
                                        message = message.replace(key, value);
                                    }
                                }
                                message
                            }
                            None => return Err(anyhow::anyhow!("No enough messages.")),
                        };
                        cur_mes.take();
                        mes
                    }
                };
                let in_used = match used.get(&curs.address) {
                    Some((s, address)) => {
                        if s == &nmes {
                            file.write_u32_at(curs.offset, *address as u32)?;
                            continue;
                        }
                        if let Some(address) = extra.get(&nmes) {
                            file.write_u32_at(curs.offset, *address as u32)?;
                            continue;
                        }
                        true
                    }
                    None => false,
                };
                let bgi_str_old_offset = curs.address + self.offset;
                if !self.append && old_offset < bgi_str_old_offset {
                    file.write_all(&self.data.data[old_offset..bgi_str_old_offset])?;
                    new_offset += bgi_str_old_offset - old_offset;
                    old_offset = bgi_str_old_offset;
                }
                let old_str_len = self
                    .data
                    .cpeek_cstring_at(bgi_str_old_offset as u64)?
                    .as_bytes_with_nul()
                    .len();
                let nmess = encode_string(encoding, &nmes, false)?;
                let write_to_original = self.append && !in_used && nmess.len() + 1 <= old_str_len;
                if write_to_original {
                    file.write_all_at(bgi_str_old_offset, &nmess)?;
                    file.write_u8_at(bgi_str_old_offset + nmess.len(), 0)?; // null terminator
                } else {
                    file.write_all(&nmess)?;
                    file.write_u8(0)?; // null terminator
                }
                let new_address = if write_to_original {
                    bgi_str_old_offset - self.offset
                } else {
                    new_offset - self.offset
                };
                file.write_u32_at(curs.offset, new_address as u32)?;
                if in_used {
                    extra.insert(nmes, new_address);
                } else {
                    used.insert(curs.address, (nmes, new_address));
                }
                old_offset += old_str_len;
                if !write_to_original {
                    new_offset += nmess.len() + 1; // +1 for null terminator
                }
            }
            if cur_mes.is_some() || mes.next().is_some() {
                return Err(anyhow::anyhow!("Some messages were not processed."));
            }
            if !self.append && old_offset < self.data.data.len() {
                file.write_all(&self.data.data[old_offset..])?;
            }
            return Ok(());
        }
        let mut mes = messages.iter_mut();
        let mut cur_mes = None;
        let mut strs = self.strings.iter();
        let mut nstrs = Vec::new();
        let mut cur_str = strs.next();
        let mut old_offset = 0;
        let mut new_offset = 0;
        let mut rubys = Vec::new();
        let mut parsed_ruby = false;
        if self.append {
            file.write_all(&self.data.data)?;
            new_offset = self.data.data.len();
        }
        while let Some(curs) = cur_str {
            if !curs.is_internal() {
                if cur_mes.is_none() {
                    cur_mes = mes.next();
                }
            }
            let bgi_str_old_offset = curs.address + self.offset;
            if !self.append && old_offset < bgi_str_old_offset {
                file.write_all(&self.data.data[old_offset..bgi_str_old_offset])?;
                new_offset += bgi_str_old_offset - old_offset;
                old_offset = bgi_str_old_offset;
            }
            let old_str_len = self
                .data
                .cpeek_cstring_at((curs.address + self.offset) as u64)?
                .as_bytes_with_nul()
                .len();
            let nmes = match curs.typ {
                BGIStringType::Internal => self.read_string(curs.address)?,
                BGIStringType::Ruby => {
                    if !self.is_v1 && self.is_v1_instr {
                        if rubys.is_empty() {
                            if parsed_ruby {
                                String::from("<")
                            } else {
                                rubys = match &mut cur_mes {
                                    Some(m) => parse_ruby_from_text(&mut m.message)?,
                                    None => return Err(anyhow::anyhow!("No enough messages.")),
                                };
                                parsed_ruby = true;
                                if rubys.is_empty() {
                                    String::from("<")
                                } else {
                                    let ruby_str = rubys.remove(0);
                                    ruby_str
                                }
                            }
                        } else {
                            rubys.remove(0)
                        }
                    } else {
                        self.read_string(curs.address)?
                    }
                }
                BGIStringType::Name => match &cur_mes {
                    Some(m) => {
                        if let Some(name) = &m.name {
                            let mut name = name.clone();
                            if let Some(replacement) = replacement {
                                for (key, value) in replacement.map.iter() {
                                    name = name.replace(key, value);
                                }
                            }
                            name
                        } else {
                            return Err(anyhow::anyhow!("Name is missing for message."));
                        }
                    }
                    None => return Err(anyhow::anyhow!("No enough messages.")),
                },
                BGIStringType::Message => {
                    if !rubys.is_empty() {
                        eprintln!("Warning: Some ruby strings are unused: {:?}", rubys);
                        crate::COUNTER.inc_warning();
                        rubys.clear();
                    }
                    parsed_ruby = false;
                    let mes = match &cur_mes {
                        Some(m) => {
                            let mut message = m.message.clone();
                            if let Some(replacement) = replacement {
                                for (key, value) in replacement.map.iter() {
                                    message = message.replace(key, value);
                                }
                            }
                            message
                        }
                        None => return Err(anyhow::anyhow!("No enough messages.")),
                    };
                    cur_mes.take();
                    mes
                }
            };
            let nmes = encode_string(encoding, &nmes, false)?;
            file.write_all(&nmes)?;
            file.write_u8(0)?;
            let new_str_len = nmes.len() + 1; // +1 for null terminator
            let new_address = new_offset - self.offset;
            nstrs.push(BGIString {
                offset: curs.offset,
                address: new_address,
                typ: curs.typ.clone(),
            });
            old_offset += old_str_len;
            new_offset += new_str_len;
            cur_str = strs.next();
        }
        if cur_mes.is_some() || mes.next().is_some() {
            return Err(anyhow::anyhow!("Some messages were not processed."));
        }
        for str in nstrs {
            file.write_u32_at(str.offset, str.address as u32)?;
        }
        if !self.append && old_offset < self.data.data.len() {
            file.write_all(&self.data.data[old_offset..])?;
        }
        Ok(())
    }
}

lazy_static! {
    static ref RUBY_REGEX: Regex = Regex::new(r"<r([^>]+)>([^<]+)</r>").unwrap();
}

fn parse_ruby_from_text(text: &mut String) -> Result<Vec<String>> {
    let mut map = BTreeMap::new();
    for i in RUBY_REGEX.captures_iter(&text) {
        let i = i?;
        let ruby_text = i.get(1).map_or("", |m| m.as_str());
        let ruby_str = i.get(2).map_or("", |m| m.as_str());
        if !ruby_text.is_empty() && !ruby_str.is_empty() {
            map.insert(ruby_str.to_owned(), ruby_text.to_owned());
        }
    }
    let mut result = Vec::new();
    for (ruby_str, ruby_text) in map {
        *text = text.replace(&format!("<r{ruby_text}>{ruby_str}</r>"), &ruby_str);
        result.push(ruby_str);
        result.push(ruby_text);
    }
    Ok(result)
}

#[test]
fn test_parse_ruby_from_text() {
    let mut text =
        String::from("This is a test <rRubyText>RubyString</r> and <rAnotherText>AnotherRuby</r>.");
    let ruby = parse_ruby_from_text(&mut text).unwrap();
    assert_eq!(text, "This is a test RubyString and AnotherRuby.");
    assert_eq!(
        ruby,
        vec![
            "AnotherRuby".to_string(),
            "AnotherText".to_string(),
            "RubyString".to_string(),
            "RubyText".to_string()
        ]
    );
}
