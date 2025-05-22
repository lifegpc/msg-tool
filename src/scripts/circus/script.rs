use super::info::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::{decode_to_string, encode_string};
use anyhow::Result;

pub struct CircusMesScriptBuilder {}

impl CircusMesScriptBuilder {
    pub const fn new() -> Self {
        CircusMesScriptBuilder {}
    }
}

impl ScriptBuilder for CircusMesScriptBuilder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Cp932
    }

    fn build_script(
        &self,
        filename: &str,
        encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(CircusMesScript::new(
            filename.as_ref(),
            encoding,
            config,
        )?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["mes"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::Circus
    }
}

#[derive(Debug)]
struct Token {
    offset: usize,
    length: usize,
    value: u8,
}

pub struct CircusMesScript {
    data: Vec<u8>,
    encoding: Encoding,
    is_new_ver: bool,
    version: u16,
    info: &'static ScriptInfo,
    asm_bin_offset: usize,
    blocks_offset: usize,
    tokens: Vec<Token>,
}

impl CircusMesScript {
    pub fn new(filename: &str, encoding: Encoding, config: &ExtraConfig) -> Result<Self> {
        let data = crate::utils::files::read_file(filename)?;
        let head0 = i32::from_le_bytes(data[0..4].try_into()?);
        let head1 = i32::from_le_bytes(data[4..8].try_into()?);
        let mut is_new_ver = false;
        let mut version = 0;
        let mut info = config
            .circus_mes_type
            .as_ref()
            .and_then(|name| ScriptInfo::query(name.as_ref()));
        let mut asm_bin_offset = 0;
        let mut blocks_offset = 0;
        if head1 == 0x3 {
            let offset = head0 * 0x6 + 0x4;
            if data.len() > offset as usize {
                if data.len() > offset as usize + 3 {
                    version =
                        u16::from_le_bytes(data[offset as usize..offset as usize + 2].try_into()?);
                    if info.is_none() {
                        info = ScriptInfo::query_by_version(version);
                    }
                    asm_bin_offset = offset as usize + 3;
                }
                blocks_offset = 8;
            }
            is_new_ver = true;
        } else {
            let offset = head0 * 0x4 + 0x4;
            if data.len() > offset as usize {
                if data.len() > offset as usize + 2 {
                    version =
                        u16::from_le_bytes(data[offset as usize..offset as usize + 2].try_into()?);
                    if info.is_none() {
                        info = ScriptInfo::query_by_version(version);
                    }
                    asm_bin_offset = offset as usize + 2;
                }
                blocks_offset = 4;
            }
        }
        let info = info.ok_or(anyhow::anyhow!("Failed to detect version."))?;
        let mut tokens = Vec::new();
        let mut offset = 0;
        let asm_bin_size = if asm_bin_offset == 0 {
            0
        } else {
            data.len() - asm_bin_offset
        };
        while offset < asm_bin_size {
            let value = data[asm_bin_offset + offset];
            let length = if info.uint8x2.its(value) {
                0x03
            } else if info.uint8str.its(value) {
                let mut len = 0x3;
                let mut temp = data[asm_bin_offset + offset + len - 1];
                while temp != 0x00 {
                    len += 0x1;
                    if asm_bin_offset + offset + len >= data.len() {
                        break;
                    }
                    temp = data[asm_bin_offset + offset + len - 1];
                }
                len
            } else if info.string.its(value) || info.encstr.its(value) {
                let mut len = 1;
                let mut temp = data[asm_bin_offset + offset + len - 1];
                while temp != 0x00 {
                    len += 0x1;
                    if asm_bin_offset + offset + len >= data.len() {
                        break;
                    }
                    temp = data[asm_bin_offset + offset + len - 1];
                }
                len
            } else if info.uint16x4.its(value) {
                0x09
            } else {
                return Err(anyhow::anyhow!(format!(
                    "Unknown token type: 0x{:02X} at offset {}",
                    value,
                    asm_bin_offset + offset
                )));
            };
            let token = Token {
                offset,
                length,
                value,
            };
            offset += length;
            tokens.push(token);
        }
        Ok(CircusMesScript {
            data,
            encoding,
            is_new_ver,
            version,
            info,
            asm_bin_offset,
            blocks_offset,
            tokens,
        })
    }
}

impl std::fmt::Debug for CircusMesScript {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CircusMesScript")
            .field("encoding", &self.encoding)
            .field("is_new_ver", &self.is_new_ver)
            .field("version", &self.version)
            .field("info", &self.info)
            .field("asm_bin_offset", &self.asm_bin_offset)
            .field("blocks_offset", &self.blocks_offset)
            .field("tokens", &self.tokens)
            .finish_non_exhaustive()
    }
}

impl Script for CircusMesScript {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Json
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::Fixed {
            length: 32,
            keep_original: false,
        }
    }

    fn extract_messages(&self) -> Result<Vec<Message>> {
        let mut mes = vec![];
        let mut name = None;
        for token in self.tokens.iter() {
            let mut t = None;
            if self.info.encstr.its(token.value) {
                let mut text = self.data[self.asm_bin_offset + token.offset + 1
                    ..self.asm_bin_offset + token.offset + token.length - 1]
                    .to_vec();
                for t in text.iter_mut() {
                    *t = (*t).overflowing_add(self.info.deckey).0;
                }
                t = Some(decode_to_string(self.encoding, &text)?);
                // println!("Token(enc): {:?}, {}", token, t.as_ref().unwrap());
            } else if token.value == self.info.optunenc {
                let text = &self.data[self.asm_bin_offset + token.offset + 1
                    ..self.asm_bin_offset + token.offset + token.length - 1];
                t = Some(decode_to_string(self.encoding, text)?);
                // println!("Token: {:?}, {}", token, t.as_ref().unwrap());
            }
            match t {
                Some(t) => {
                    if token.value == self.info.nameopcode {
                        name = Some(t);
                    } else {
                        let message = Message::new(t, name.take());
                        mes.push(message);
                    }
                }
                None => {}
            }
        }
        Ok(mes)
    }

    fn import_messages(
        &self,
        messages: Vec<Message>,
        filename: &str,
        encoding: Encoding,
        replacement: Option<&ReplacementTable>,
    ) -> Result<()> {
        let mut repls = Vec::new();
        if !encoding.is_jis() {
            fn insert_repl(
                repls: &mut Vec<(String, String)>,
                s: &'static str,
                encoding: Encoding,
            ) -> Result<()> {
                let jis = encode_string(Encoding::Cp932, s, true)?;
                let out = decode_to_string(encoding, &jis)?;
                repls.push((s.to_string(), out));
                Ok(())
            }
            let _ = insert_repl(&mut repls, "｛", encoding);
            let _ = insert_repl(&mut repls, "／", encoding);
            let _ = insert_repl(&mut repls, "｝", encoding);
            if repls.len() < 3 {
                println!(
                    "Warning: Some replacements cannot used in current encoding. Ruby text may be broken."
                );
                crate::COUNTER.inc_warning();
            }
        }
        match replacement {
            Some(repl) => {
                for (k, v) in repl.map.iter() {
                    repls.push((k.to_string(), v.to_string()));
                }
            }
            None => {}
        }
        let mut buffer = Vec::with_capacity(self.data.len());
        buffer.extend_from_slice(&self.data[..self.asm_bin_offset]);
        let mut nmes = Vec::with_capacity(messages.len());
        for m in messages {
            nmes.insert(0, m);
        }
        let mut mes = nmes.pop();
        let mut block_count = 0;
        for token in self.tokens.iter() {
            if !self.is_new_ver {
                let count = buffer.len() as u32;
                let offset = count - self.asm_bin_offset as u32 + 2;
                buffer[self.blocks_offset + block_count * 4
                    ..self.blocks_offset + block_count * 4 + 4]
                    .copy_from_slice(&offset.to_le_bytes());
                block_count += 1;
            }
            if self.info.encstr.its(token.value) {
                if mes.is_none() {
                    mes = nmes.pop();
                    if mes.is_none() {
                        return Err(anyhow::anyhow!("No more messages to import"));
                    }
                }
                let mut s = if token.value == self.info.nameopcode {
                    match mes.as_mut().unwrap().name.take() {
                        Some(s) => s,
                        None => {
                            let t = mes.as_ref().unwrap().message.clone();
                            mes = None;
                            t
                        }
                    }
                } else {
                    let t = mes.as_ref().unwrap().message.clone();
                    mes = None;
                    t
                };
                for i in repls.iter() {
                    s = s.replace(i.0.as_str(), i.1.as_str());
                }
                let mut text = encode_string(encoding, &s, false)?;
                buffer.push(token.value);
                for t in text.iter_mut() {
                    *t = (*t).overflowing_sub(self.info.deckey).0;
                }
                buffer.extend_from_slice(&text);
                buffer.push(0x00);
                continue;
            }
            if token.value == self.info.optunenc {
                if mes.is_none() {
                    mes = nmes.pop();
                    if mes.is_none() {
                        return Err(anyhow::anyhow!("No more messages to import"));
                    }
                }
                let mut s = if token.value == self.info.nameopcode {
                    match mes.as_mut().unwrap().name.take() {
                        Some(s) => s,
                        None => {
                            let t = mes.as_ref().unwrap().message.clone();
                            mes = None;
                            t
                        }
                    }
                } else {
                    let t = mes.as_ref().unwrap().message.clone();
                    mes = None;
                    t
                };
                for i in repls.iter() {
                    s = s.replace(i.0.as_str(), i.1.as_str());
                }
                buffer.push(token.value);
                let text = encode_string(encoding, &s, false)?;
                buffer.extend_from_slice(&text);
                buffer.push(0x00);
                continue;
            }
            if self.is_new_ver && (token.value == 0x3 || token.value == 0x4) {
                let count = buffer.len() as u32;
                let offset = count - self.asm_bin_offset as u32 + token.length as u32;
                let block = u32::from_le_bytes(
                    buffer[self.blocks_offset + block_count * 4
                        ..self.blocks_offset + block_count * 4 + 4]
                        .try_into()?,
                );
                let block = (block & (0xFF << 0x18)) | offset;
                buffer[self.blocks_offset + block_count * 4
                    ..self.blocks_offset + block_count * 4 + 4]
                    .copy_from_slice(&block.to_le_bytes());
                block_count += 1;
            }
            let len = std::cmp::min(
                self.asm_bin_offset + token.offset + token.length,
                self.data.len(),
            );
            buffer.extend_from_slice(&self.data[self.asm_bin_offset + token.offset..len]);
        }
        let mut f = crate::utils::files::write_file(filename)?;
        f.write_all(&buffer)?;
        Ok(())
    }
}
