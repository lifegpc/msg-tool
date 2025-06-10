use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::{decode_to_string, encode_string};
use anyhow::Result;

#[derive(Debug)]
pub struct ItufuruScriptBuilder {}

impl ItufuruScriptBuilder {
    pub const fn new() -> Self {
        ItufuruScriptBuilder {}
    }
}

impl ScriptBuilder for ItufuruScriptBuilder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Cp932
    }

    fn build_script(
        &self,
        data: Vec<u8>,
        _filename: &str,
        encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(ItufuruScript::new(data, encoding, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::YaneuraoItufuru
    }
}

#[derive(Debug)]
struct ItufuruString {
    instr: u16,
    len_pos: usize,
    len: u16,
}

#[derive(Debug)]
pub struct ItufuruScript {
    data: MemReader,
    strings: Vec<ItufuruString>,
    encoding: Encoding,
}

impl ItufuruScript {
    pub fn new(buf: Vec<u8>, encoding: Encoding, _config: &ExtraConfig) -> Result<Self> {
        let mut reader = MemReader::new(buf);
        let mut strings = Vec::new();
        let len = reader.data.len();

        while reader.pos + 1 < len {
            let instr = reader.read_u16()?;
            // 普通文本 0x2
            // 选项     0x1e
            // 文件名   0x1
            // 背景     0x13
            // 声音     0x27
            if instr == 0x2 || instr == 0x1e || instr == 0x1 || instr == 0x13 || instr == 0x27 {
                let len_pos = reader.pos;
                let len = reader.read_u16()?;
                match reader.read_cstring() {
                    Ok(s) => {
                        let slen = s.as_bytes_with_nul().len() as u16;
                        if slen != len {
                            reader.pos = len_pos;
                            continue;
                        }
                        if instr == 0x2 && !s.as_bytes().ends_with(b"\n") {
                            reader.pos = len_pos;
                            continue;
                        }
                        if instr != 0x2 && instr != 0x1e {
                            continue;
                        }
                        strings.push(ItufuruString {
                            instr,
                            len_pos,
                            len,
                        });
                    }
                    Err(_) => {
                        reader.pos = len_pos;
                        continue;
                    }
                }
            }
        }

        Ok(ItufuruScript {
            data: reader,
            strings,
            encoding,
        })
    }
}

impl Script for ItufuruScript {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Json
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn extract_messages(&self) -> Result<Vec<Message>> {
        let mut messages = Vec::new();
        for i in self.strings.iter() {
            let str_pos = i.len_pos + 2; // Skip the length bytes
            let s = self.data.cpeek_cstring_at(str_pos)?;
            let decoded = decode_to_string(self.encoding, s.as_bytes())?;
            messages.push(Message {
                name: None,
                message: decoded,
            });
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
        if self.strings.len() != messages.len() {
            return Err(anyhow::anyhow!(
                "Number of messages does not match the number of strings in the script"
            ));
        }
        let mut old_pos = 0;
        for (old, new) in self.strings.iter().zip(messages) {
            if old_pos < old.len_pos {
                file.write_all(&self.data.data[old_pos..old.len_pos])?;
                old_pos = old.len_pos;
            }
            let mut nstr = new.message;
            if let Some(repl) = replacement {
                for (from, to) in repl.map.iter() {
                    nstr = nstr.replace(from, to);
                }
            }
            if old.instr == 0x2 && !nstr.ends_with('\n') {
                nstr.push('\n');
            }
            let encoded = encode_string(encoding, &nstr, false)?;
            let new_len = encoded.len() as u16 + 1;
            file.write_u16(new_len)?;
            file.write_all(&encoded)?;
            file.write_all(&[0])?; // Null terminator
            old_pos += 2 + old.len as usize;
        }
        if old_pos < self.data.data.len() {
            file.write_all(&self.data.data[old_pos..])?;
        }
        Ok(())
    }
}
