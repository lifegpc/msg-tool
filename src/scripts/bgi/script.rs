use super::parser::*;
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::{decode_to_string, encode_string};
use anyhow::Result;
use std::collections::BTreeMap;

#[derive(Debug)]
pub struct BGIScriptBuilder {}

impl BGIScriptBuilder {
    pub fn new() -> Self {
        BGIScriptBuilder {}
    }
}

impl ScriptBuilder for BGIScriptBuilder {
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

pub struct BGIScript {
    data: MemReader,
    encoding: Encoding,
    strings: Vec<BGIString>,
    is_v1: bool,
    offset: usize,
    import_duplicate: bool,
}

impl std::fmt::Debug for BGIScript {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BGIScript")
            .field("encoding", &self.encoding)
            .finish_non_exhaustive()
    }
}

impl BGIScript {
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
                offset,
                import_duplicate: config.bgi_import_duplicate,
            })
        } else {
            let mut parser = V0Parser::new(data.to_ref());
            parser.disassemble()?;
            let strings = parser.strings.clone();
            Ok(Self {
                data,
                encoding,
                strings,
                is_v1: false,
                offset: 0,
                import_duplicate: config.bgi_import_duplicate,
            })
        }
    }

    fn read_string(&self, offset: usize) -> Result<String> {
        let start = self.offset + offset;
        let string_data = self.data.cpeek_cstring_at(start)?;
        let string = decode_to_string(self.encoding, string_data.as_bytes())?;
        Ok(string)
    }
}

impl Script for BGIScript {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Json
    }

    fn default_format_type(&self) -> FormatOptions {
        if self.is_v1 {
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
        for bgi_string in &self.strings {
            match bgi_string.typ {
                BGIStringType::Name => {
                    name = Some(self.read_string(bgi_string.address)?);
                }
                BGIStringType::Message => {
                    let message = self.read_string(bgi_string.address)?;
                    messages.push(Message {
                        name: name.take(),
                        message: message,
                    });
                }
                _ => {}
            }
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
        if !self.import_duplicate {
            let mut str_map = BTreeMap::new();
            let mut mes = messages.iter();
            let mut cur_mes = mes.next();
            for curs in self.strings.iter() {
                if !curs.is_internal() {
                    if cur_mes.is_none() {
                        cur_mes = mes.next();
                    }
                }
                if str_map.contains_key(&curs.address) {
                    continue;
                }
                let nmes = match curs.typ {
                    BGIStringType::Internal => self.read_string(curs.address)?,
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
                str_map.insert(curs.address, nmes);
            }
            if cur_mes.is_some() || mes.next().is_some() {
                return Err(anyhow::anyhow!("Some messages were not processed."));
            }
            let mut old_offset = 0;
            let mut new_offset = 0;
            let mut new_address_map = BTreeMap::new();
            for (address, nmes) in str_map {
                let bgi_str_old_offset = address + self.offset;
                if old_offset < bgi_str_old_offset {
                    file.write_all(&self.data.data[old_offset..bgi_str_old_offset])?;
                    new_offset += bgi_str_old_offset - old_offset;
                    old_offset = bgi_str_old_offset;
                }
                let old_str_len = self
                    .data
                    .cpeek_cstring_at(bgi_str_old_offset)?
                    .as_bytes_with_nul()
                    .len();
                let nmes = encode_string(encoding, &nmes, false)?;
                file.write_all(&nmes)?;
                file.write_u8(0)?; // null terminator
                let new_address = new_offset - self.offset;
                new_address_map.insert(address, new_address);
                old_offset += old_str_len;
                new_offset += nmes.len() + 1; // +1 for null terminator
            }
            if old_offset < self.data.data.len() {
                file.write_all(&self.data.data[old_offset..])?;
            }
            for bgis in self.strings.iter() {
                let new_address = new_address_map.get(&bgis.address).ok_or(anyhow::anyhow!(
                    "Address {} not found in new address map.",
                    bgis.address
                ))?;
                file.write_u32_at(bgis.offset, *new_address as u32)?;
            }
            return Ok(());
        }
        let mut mes = messages.iter();
        let mut cur_mes = None;
        let mut strs = self.strings.iter();
        let mut nstrs = Vec::new();
        let mut cur_str = strs.next();
        let mut old_offset = 0;
        let mut new_offset = 0;
        while let Some(curs) = cur_str {
            if !curs.is_internal() {
                if cur_mes.is_none() {
                    cur_mes = mes.next();
                }
            }
            let bgi_str_old_offset = curs.address + self.offset;
            if old_offset < bgi_str_old_offset {
                file.write_all(&self.data.data[old_offset..bgi_str_old_offset])?;
                new_offset += bgi_str_old_offset - old_offset;
                old_offset = bgi_str_old_offset;
            }
            let old_str_len = self
                .data
                .cpeek_cstring_at(curs.address + self.offset)?
                .as_bytes_with_nul()
                .len();
            let nmes = match curs.typ {
                BGIStringType::Internal => self.read_string(curs.address)?,
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
        if old_offset < self.data.data.len() {
            file.write_all(&self.data.data[old_offset..])?;
        }
        Ok(())
    }
}
