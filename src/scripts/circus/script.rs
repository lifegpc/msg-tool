//! Circus Script File (.mes)
use super::info::*;
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::{decode_to_string, encode_string};
use anyhow::Result;

#[derive(Debug)]
/// Circus MES Script Builder
pub struct CircusMesScriptBuilder {}

impl CircusMesScriptBuilder {
    /// Creates a new instance of `CircusMesScriptBuilder`.
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
        buf: Vec<u8>,
        _filename: &str,
        encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(CircusMesScript::new(buf, encoding, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["mes"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::Circus
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        try_parse_header(MemReaderRef::new(&buf[..buf_len])).ok()
    }
}

fn try_parse_header(mut data: MemReaderRef<'_>) -> Result<u8> {
    let head0 = data.read_i32()?;
    let head1 = data.read_i32()?;
    if head1 == 0x3 {
        let offset = head0 as u64 * 0x6 + 0x4;
        let version = data.peek_u16_at(offset)?;
        if ScriptInfo::query_by_version(version).is_some() {
            return Ok(10);
        }
    } else {
        let offset = head0 as u64 * 0x4 + 0x4;
        let version = data.peek_u16_at(offset)?;
        if ScriptInfo::query_by_version(version).is_some() {
            return Ok(10);
        }
    }
    Err(anyhow::anyhow!("Not a Circus MES script"))
}

#[derive(Debug)]
struct Token {
    offset: usize,
    length: usize,
    value: u8,
}

/// Circus MES Script
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
    /// Creates a new `CircusMesScript` from the given data and configuration.
    ///
    /// * `data` - The data to read the MES script from.
    /// * `encoding` - The encoding to use for string fields.
    /// * `config` - Extra configuration options.
    pub fn new(data: Vec<u8>, encoding: Encoding, config: &ExtraConfig) -> Result<Self> {
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
            break_words: false,
            insert_fullwidth_space_at_line_start: true,
            break_with_sentence: true,
            #[cfg(feature = "jieba")]
            break_chinese_words: true,
            #[cfg(feature = "jieba")]
            jieba_dict: None,
            no_remove_space_at_line_start: false,
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
                t = Some(decode_to_string(self.encoding, &text, true)?);
                // println!("Token(enc): {:?}, {}", token, t.as_ref().unwrap());
            } else if token.value == self.info.optunenc {
                let text = &self.data[self.asm_bin_offset + token.offset + 1
                    ..self.asm_bin_offset + token.offset + token.length - 1];
                t = Some(decode_to_string(self.encoding, text, true)?);
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

    fn import_messages<'a>(
        &'a self,
        messages: Vec<Message>,
        writer: Box<dyn WriteSeek + 'a>,
        _filename: &str,
        encoding: Encoding,
        replacement: Option<&'a ReplacementTable>,
    ) -> Result<()> {
        let mut repls = Vec::new();
        if !encoding.is_jis() {
            fn insert_repl(
                repls: &mut Vec<(String, String)>,
                s: &'static str,
                encoding: Encoding,
            ) -> Result<()> {
                let jis = encode_string(Encoding::Cp932, s, true)?;
                let out = decode_to_string(encoding, &jis, true)?;
                repls.push((s.to_string(), out));
                Ok(())
            }
            let _ = insert_repl(&mut repls, "｛", encoding);
            let _ = insert_repl(&mut repls, "／", encoding);
            let _ = insert_repl(&mut repls, "｝", encoding);
            if repls.len() < 3 {
                eprintln!(
                    "Warning: Some replacements cannot used in current encoding. Ruby text may be broken."
                );
                crate::COUNTER.inc_warning();
            }
        }
        if let Some(repl) = replacement {
            for (k, v) in repl.map.iter() {
                repls.push((k.to_string(), v.to_string()));
            }
        }

        let source = MemReaderRef::new(&self.data);
        let mut patcher = BinaryPatcher::new(source, writer, |pos| Ok(pos), |pos| Ok(pos))?;

        let mut pending_messages: Vec<Message> = messages.into_iter().rev().collect();
        let mut current_message = pending_messages.pop();
        let mut block_updates: Vec<(u64, u32)> = Vec::new();
        let mut block_index = 0usize;

        for token in &self.tokens {
            let token_start = (self.asm_bin_offset + token.offset) as u64;
            patcher.copy_up_to(token_start)?;

            if !self.is_new_ver {
                let block_offset = (self.blocks_offset + block_index * 4) as u64;
                let new_offset = patcher.map_offset(token_start)?;
                let offset_value = (new_offset - self.asm_bin_offset as u64 + 2) as u32;
                block_updates.push((block_offset, offset_value));
                block_index += 1;
            }

            if self.info.is_message_opcode(token.value) {
                if current_message.is_none() {
                    current_message = pending_messages.pop();
                    if current_message.is_none() {
                        return Err(anyhow::anyhow!("No more messages to import"));
                    }
                }

                let mut text = {
                    let message = current_message.as_mut().unwrap();
                    if self.info.is_name_opcode(token.value) {
                        match message.name.take() {
                            Some(name) => name,
                            None => {
                                let msg = message.message.clone();
                                current_message = None;
                                msg
                            }
                        }
                    } else {
                        let msg = message.message.clone();
                        current_message = None;
                        msg
                    }
                };

                for (from, to) in &repls {
                    text = text.replace(from, to);
                }

                let mut token_bytes = Vec::with_capacity(text.len() + 2);
                token_bytes.push(token.value);
                let mut encoded = encode_string(encoding, &text, false)?;
                if self.info.is_encrypted_message(token.value) {
                    if encoded.contains(&self.info.deckey) {
                        eprintln!(
                            "Warning: text contains deckey 0x{:02X}, text may be truncated: {}",
                            self.info.deckey, text,
                        );
                        crate::COUNTER.inc_warning();
                    }
                    for b in &mut encoded {
                        *b = (*b).overflowing_sub(self.info.deckey).0;
                    }
                }
                token_bytes.extend_from_slice(&encoded);
                token_bytes.push(0x00);
                patcher.replace_bytes(token.length as u64, &token_bytes)?;
                continue;
            }

            if self.is_new_ver && (token.value == 0x03 || token.value == 0x04) {
                let block_offset = (self.blocks_offset + block_index * 4) as u64;
                let original_block = patcher.input.cpeek_u32_at(block_offset)?;
                let new_offset = patcher.map_offset(token_start)?;
                let offset = (new_offset - self.asm_bin_offset as u64 + token.length as u64) as u32;
                let value = (original_block & (0xFF << 0x18)) | offset;
                block_updates.push((block_offset, value));
                block_index += 1;
            }

            let token_end = token_start + token.length as u64;
            patcher.copy_up_to(token_end)?;
        }

        patcher.copy_up_to(self.data.len() as u64)?;

        for (offset, value) in block_updates {
            patcher.patch_u32(offset, value)?;
        }

        Ok(())
    }
}
