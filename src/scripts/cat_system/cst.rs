//! CatSystem2 Scene Script File (.cst)
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use anyhow::Result;
use fancy_regex::Regex;
use int_enum::IntEnum;
use std::io::{Read, Write};

#[derive(Debug)]
/// Builder for CatSystem2 Scene Script files.
pub struct CstScriptBuilder {}

impl CstScriptBuilder {
    /// Creates a new instance of `CstScriptBuilder`.
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
        _archive: Option<&Box<dyn Script>>,
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

trait CustomFn {
    fn write_patched_string(&mut self, s: &CstString, data: &[u8]) -> Result<usize>;
}

impl CustomFn for MemWriter {
    fn write_patched_string(&mut self, s: &CstString, data: &[u8]) -> Result<usize> {
        if data.len() + 1 > s.len {
            let pos = self.data.len();
            self.pos = pos;
            self.write_u8(1)?; // Start marker
            self.write_u8(u8::from(s.typ))?;
            self.write_all(data)?;
            self.write_u8(0)?; // Null terminator
            Ok(pos)
        } else {
            self.pos = s.address;
            self.write_u8(1)?; // Start marker
            self.write_u8(u8::from(s.typ))?;
            self.write_all(data)?;
            self.write_u8(0)?; // Null terminator
            Ok(s.address)
        }
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
/// CatSystem2 Scene Script.
pub struct CstScript {
    data: MemReader,
    compressed: bool,
    strings: Vec<CstString>,
    compress_level: u32,
}

impl CstScript {
    /// Creates a new instance of `CstScript` from a buffer.
    ///
    /// * `buf` - The buffer containing the script data.
    /// * `encoding` - The encoding of the script.
    /// * `config` - Extra configuration options.
    pub fn new(buf: Vec<u8>, encoding: Encoding, config: &ExtraConfig) -> Result<Self> {
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
            let offset = file.cpeek_u32_at(string_address_offset as u64 + i as u64 * 4)? as usize
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
            compress_level: config.zlib_compression_level,
        })
    }
}

lazy_static::lazy_static! {
    static ref CST_COMMAND_REGEX: Regex = Regex::new(r"^\d+\s+\w+\s+(.+)").unwrap();
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
                        message: s.text.replace("\\n", "\n"),
                        name: name.take(),
                    });
                }
                CstStringType::Character => {
                    name = Some(s.text.clone());
                }
                CstStringType::Command => {
                    if let Some(caps) = CST_COMMAND_REGEX.captures(&s.text)? {
                        if let Some(text) = caps.get(1) {
                            messages.push(Message {
                                message: text.as_str().to_string(),
                                name: None,
                            });
                        }
                    }
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
        let mut writer = MemWriter::from_vec(self.data.data.clone());
        let mut mess = messages.iter();
        let mut mes = mess.next();
        let strings_address_offset = 0x10 + self.data.cpeek_u32_at(0x8)? as usize;
        let strings_offset = 0x10 + self.data.cpeek_u32_at(0xC)? as usize;
        for (i, s) in self.strings.iter().enumerate() {
            match s.typ {
                CstStringType::Message => {
                    if s.text.is_empty() {
                        continue; // Skip empty messages
                    }
                    let m = match mes {
                        Some(m) => m,
                        None => {
                            return Err(anyhow::anyhow!("No enough messages."));
                        }
                    };
                    let mut message = m.message.clone();
                    if let Some(replacement) = replacement {
                        for (k, v) in &replacement.map {
                            message = message.replace(k, v);
                        }
                    }
                    message = message.replace("\n", "\\n");
                    let data = encode_string(encoding, &message, true)?;
                    let pos = writer.write_patched_string(s, &data)?;
                    if pos != s.address {
                        writer.write_u32_at(
                            strings_address_offset + i * 4,
                            (pos - strings_offset) as u32,
                        )?;
                    }
                    mes = mess.next();
                }
                CstStringType::Character => {
                    let m = match mes {
                        Some(m) => m,
                        None => {
                            return Err(anyhow::anyhow!("No enough messages."));
                        }
                    };
                    let mut name = match &m.name {
                        Some(name) => name.to_owned(),
                        None => return Err(anyhow::anyhow!("Message without name.")),
                    };
                    if let Some(replacement) = replacement {
                        for (k, v) in &replacement.map {
                            name = name.replace(k, v);
                        }
                    }
                    let data = encode_string(encoding, &name, true)?;
                    let pos = writer.write_patched_string(s, &data)?;
                    if pos != s.address {
                        writer.write_u32_at(
                            strings_address_offset + i * 4,
                            (pos - strings_offset) as u32,
                        )?;
                    }
                }
                CstStringType::Command => {
                    if let Some(caps) = CST_COMMAND_REGEX.captures(&s.text)? {
                        if let Some(mat) = caps.get(1) {
                            let m = match mes {
                                Some(m) => m,
                                None => {
                                    return Err(anyhow::anyhow!("No enough messages."));
                                }
                            };
                            let mut text = m.message.clone();
                            if let Some(replacement) = replacement {
                                for (k, v) in &replacement.map {
                                    text = text.replace(k, v);
                                }
                            }
                            let mut command_text = s.text.clone();
                            command_text.replace_range(mat.range(), &text);
                            let data = encode_string(encoding, &command_text, true)?;
                            let pos = writer.write_patched_string(s, &data)?;
                            if pos != s.address {
                                writer.write_u32_at(
                                    strings_address_offset + i * 4,
                                    (pos - strings_offset) as u32,
                                )?;
                            }
                            mes = mess.next();
                        }
                    }
                }
                _ => {}
            }
        }
        if mes.is_some() || mess.next().is_some() {
            return Err(anyhow::anyhow!("Not all messages were processed."));
        }
        let data_len = writer.data.len() as u32 - 0x10;
        writer.write_u32_at(0, data_len)?;
        let data = writer.into_inner();
        file.write_all(b"CatScene")?;
        file.write_u32(0)?; // Compressed size
        file.write_u32(data.len() as u32)?; // Uncompressed size
        if self.compressed {
            let mut encoder = flate2::write::ZlibEncoder::new(
                &mut file,
                flate2::Compression::new(self.compress_level),
            );
            encoder.write_all(&data)?;
            encoder.finish()?;
            let file_len = file.stream_position()?;
            let compressed_size = (file_len as u32) - 0x10;
            file.write_u32_at(8, compressed_size)?;
        } else {
            file.write_all(&data)?;
        }
        Ok(())
    }
}
