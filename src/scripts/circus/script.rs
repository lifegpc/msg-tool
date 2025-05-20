use super::info::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::decode_to_string;
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

    fn extract_messages(&self) -> Result<Vec<Message>> {
        let mut mes = vec![];
        let mut name = None;
        for token in self.tokens.iter() {
            let mut t = None;
            if self.info.encstr.its(token.value) {
                let mut text = self.data[self.asm_bin_offset + token.offset + 1
                    ..self.asm_bin_offset + token.offset + token.length]
                    .to_vec();
                for t in text.iter_mut() {
                    *t = (*t).overflowing_add(self.info.deckey).0;
                }
                t = Some(decode_to_string(self.encoding, &text)?);
                // println!("Token(enc): {:?}, {}", token, t.as_ref().unwrap());
            } else if token.value == self.info.optunenc {
                let text = &self.data[self.asm_bin_offset + token.offset + 1
                    ..self.asm_bin_offset + token.offset + token.length];
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
}
