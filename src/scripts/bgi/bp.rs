//! Buriko General Interpreter/Ethornell BP Script (._bp)
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::{decode_to_string, encode_string};
use anyhow::Result;
use std::io::{Seek, SeekFrom};

#[derive(Debug)]
/// Builder for BGI BP scripts.
pub struct BGIBpScriptBuilder {}

impl BGIBpScriptBuilder {
    /// Creates a new instance of `BGIBpScriptBuilder`.
    pub fn new() -> Self {
        BGIBpScriptBuilder {}
    }
}

impl ScriptBuilder for BGIBpScriptBuilder {
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
        Ok(Box::new(BGIBpScript::new(buf, encoding, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["_bp"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::BGIBp
    }
}

#[derive(Debug)]
struct BpString {
    offset_pos: usize,
    text_offset: u16,
}

#[derive(Debug)]
/// BGI BP script.
pub struct BGIBpScript {
    data: MemReader,
    header_size: u32,
    strings: Vec<BpString>,
    encoding: Encoding,
}

impl BGIBpScript {
    /// Creates a new instance of `BGIBpScript` from a buffer.
    ///
    /// * `buf` - The buffer containing the script data.
    /// * `encoding` - The encoding of the script.
    /// * `config` - Extra configuration options.
    pub fn new(buf: Vec<u8>, encoding: Encoding, _config: &ExtraConfig) -> Result<Self> {
        let mut reader = MemReader::new(buf);
        let header_size = reader.read_u32()?;
        let instr_size = reader.read_u32()?;
        if header_size as usize + instr_size as usize != reader.data.len() {
            return Err(anyhow::anyhow!("Invalid bp script file size"));
        }
        let mut last_instr_pos = 0;
        reader.seek(SeekFrom::Start(header_size as u64))?;
        let max_instr_len = reader.data.len() - 4;
        let mut last_instr_is_valid = true;
        while reader.pos < max_instr_len {
            let instr = reader.cpeek_u32()?;
            if instr == 0x17 {
                last_instr_pos = reader.pos;
                reader.pos += 4;
            } else {
                reader.pos += 1;
            }
        }
        if last_instr_pos == 0 {
            // return Err(anyhow::anyhow!("No end instruction found in bp script"));
            last_instr_pos = reader.data.len();
            last_instr_is_valid = false;
        }
        reader.seek(SeekFrom::Start(header_size as u64))?;
        let mut strings = Vec::new();
        while reader.pos < last_instr_pos {
            let ins = reader.read_u8()?;
            if ins == 5 {
                let text_offset = reader.peek_u16()?;
                let text_address = reader.pos + text_offset as usize - 1;
                if (text_address >= last_instr_pos || !last_instr_is_valid)
                    && text_address < reader.data.len()
                    && (text_address == last_instr_pos || reader.data[text_address - 1] == 0)
                {
                    strings.push(BpString {
                        offset_pos: reader.pos,
                        text_offset,
                    });
                    reader.pos += 2;
                }
            }
        }
        return Ok(BGIBpScript {
            data: reader,
            header_size,
            strings,
            encoding,
        });
    }
}

impl Script for BGIBpScript {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Json
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn extract_messages(&self) -> Result<Vec<Message>> {
        let mut messages = Vec::new();
        for i in self.strings.iter() {
            let text_address = i.offset_pos + i.text_offset as usize - 1;
            // println!("offset: {}, text address: {}, text_offset: {}", i.offset_pos, text_address, i.text_offset);
            let str = self.data.cpeek_cstring_at(text_address as u64)?;
            let str = decode_to_string(self.encoding, str.as_bytes(), true)?;
            messages.push(Message {
                name: None,
                message: str,
            });
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
        if messages.len() != self.strings.len() {
            return Err(anyhow::anyhow!(
                "Number of messages does not match the number of strings in the script"
            ));
        }
        file.write_all(&self.data.data)?;
        let mut new_pos = self.data.data.len();
        for (i, mes) in self.strings.iter().zip(messages) {
            let text_address = i.offset_pos + i.text_offset as usize - 1;
            let old_str_len = self
                .data
                .cpeek_cstring_at(text_address as u64)?
                .as_bytes_with_nul()
                .len();
            let mut str = mes.message;
            if let Some(replacement) = replacement {
                for (key, value) in replacement.map.iter() {
                    str = str.replace(key, value);
                }
            }
            let mut str = encode_string(encoding, &str, false)?;
            str.push(0); // Null terminator
            let new_str_len = str.len();
            if new_str_len > old_str_len {
                file.write_all(&str)?;
                let new_text_offset = (new_pos - i.offset_pos + 1) as u16;
                file.write_u16_at(i.offset_pos as u64, new_text_offset)?;
                new_pos += new_str_len;
            } else {
                file.write_all_at(text_address as u64, &str)?;
            }
        }
        let new_instr_size = (new_pos - self.header_size as usize) as u32;
        file.write_u32_at(4, new_instr_size)?;
        Ok(())
    }
}
