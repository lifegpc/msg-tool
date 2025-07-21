use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use anyhow::Result;
use int_enum::IntEnum;
use std::io::Read;

#[derive(Debug)]
pub struct CstScriptBuilder {}

impl CstScriptBuilder {
    pub fn new() -> Self {
        CstScriptBuilder {}
    }
}

impl ScriptBuilder for CstScriptBuilder {
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
        Ok(Box::new(CstScript::new(buf, encoding, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["cst"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::CatSystem
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 8 && buf.starts_with(b"CatScene") {
            return Some(255);
        }
        None
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, IntEnum)]
enum CstStringType {
    EmptyLine = 0x2,
    Paragraph = 0x03,
    Message = 0x20,
    Character = 0x21,
    Command = 0x30,
    FileName = 0xF0,
    LineNumber = 0xF1,
}

#[derive(Debug)]
struct CstString {
    typ: CstStringType,
    text: String,
    address: usize,
    /// text length (include null terminator)
    len: usize,
}

#[derive(Debug)]
pub struct CstScript {
    data: MemReader,
    compressed: bool,
    strings: Vec<CstString>,
}

impl CstScript {
    pub fn new(buf: Vec<u8>, encoding: Encoding, _config: &ExtraConfig) -> Result<Self> {
        let mut reader = MemReader::new(buf);
        let mut magic = [0; 8];
        reader.read_exact(&mut magic)?;
        if &magic != b"CatScene" {
            return Err(anyhow::anyhow!("Invalid CST script magic: {:?}", magic));
        }
        let compressed_size = reader.read_u32()?;
        let uncompressed_size = reader.read_u32()?;
        let mut file = if compressed_size == 0 {
            if uncompressed_size != reader.data.len() as u32 - 0x10 {
                return Err(anyhow::anyhow!(
                    "Uncompressed size mismatch: expected {}, got {}",
                    uncompressed_size,
                    reader.data.len() as u32 - 0x10
                ));
            }
            MemReader::new((&reader.data[0x10..]).to_vec())
        } else {
            let mut decoder = flate2::read::ZlibDecoder::new(reader);
            let mut data = Vec::with_capacity(uncompressed_size as usize);
            decoder.read_to_end(&mut data)?;
            MemReader::new(data)
        };
        let data_length = file.read_u32()?;
        if data_length as usize + 0x10 != file.data.len() {
            return Err(anyhow::anyhow!(
                "Data length mismatch: expected {}, got {}",
                data_length,
                file.data.len() - 0x10
            ));
        }
        let _clear_screen_count = file.read_u32()?;
        let string_address_offset = 0x10 + file.read_u32()?;
        let strings_offset = 0x10 + file.read_u32()?;
        let string_count = (strings_offset - string_address_offset) / 4;
        let mut strings = Vec::with_capacity(string_count as usize);
        for i in 0..string_count {
            let offset = file.cpeek_u32_at(string_address_offset as usize + i as usize * 4)?
                as usize
                + strings_offset as usize;
            file.pos = offset;
            let start_marker = file.read_u8()?;
            if start_marker != 1 {
                return Err(anyhow::anyhow!(
                    "Invalid start marker for string {}: expected 0x01, got {:02X}",
                    i,
                    start_marker
                ));
            }
            let typ = CstStringType::try_from(file.read_u8()?).map_err(|code| {
                anyhow::anyhow!("Invalid string type for string {}: {:02X}", i, code)
            })?;
            let str = file.read_cstring()?;
            let text = decode_to_string(encoding, str.as_bytes(), true)?;
            strings.push(CstString {
                typ,
                text,
                address: offset,
                len: str.as_bytes_with_nul().len(),
            });
        }
        Ok(CstScript {
            data: file,
            compressed: compressed_size != 0,
            strings,
        })
    }
}

impl Script for CstScript {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Json
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn extract_messages(&self) -> Result<Vec<Message>> {
        let mut messages = Vec::new();
        let mut name = None;
        for s in self.strings.iter() {
            match s.typ {
                CstStringType::Message => {
                    if s.text.is_empty() {
                        continue; // Skip empty messages
                    }
                    messages.push(Message {
                        message: s.text.to_string(),
                        name: name.take(),
                    });
                }
                CstStringType::Character => {
                    name = Some(s.text.clone());
                }
                // #TODO: Command
                _ => {}
            }
        }
        Ok(messages)
    }
}
