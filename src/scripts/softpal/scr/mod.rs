//! Softpal script (.src)
mod disasm;

use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use anyhow::Result;
use disasm::*;
use std::collections::HashMap;
use std::io::{Read, Write};

#[derive(Debug)]
/// Softpal script builder
pub struct SoftpalScriptBuilder {}

impl SoftpalScriptBuilder {
    /// Create a new Softpal script builder
    pub fn new() -> Self {
        Self {}
    }
}

impl ScriptBuilder for SoftpalScriptBuilder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Cp932
    }

    fn build_script(
        &self,
        buf: Vec<u8>,
        filename: &str,
        encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
        archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(SoftpalScript::new(
            buf, filename, encoding, config, archive,
        )?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["src"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::Softpal
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 4 && buf.starts_with(b"Sv20") {
            return Some(10);
        }
        None
    }
}

#[derive(Debug)]
/// Softpal SRC Script
pub struct SoftpalScript {
    data: MemReader,
    strs: Vec<PalString>,
    texts: MemReader,
    encoding: Encoding,
    label_offsets: Vec<u32>,
}

impl SoftpalScript {
    /// Create a new Softpal script
    pub fn new(
        buf: Vec<u8>,
        filename: &str,
        encoding: Encoding,
        _config: &ExtraConfig,
        archive: Option<&Box<dyn Script>>,
    ) -> Result<Self> {
        let texts = Self::load_texts_data(Self::load_file(filename, archive, "TEXT.DAT")?)?;
        let points_data = MemReader::new(Self::load_file(filename, archive, "POINT.DAT")?);
        let label_offsets = Self::load_point_data(points_data)?;
        let strs = Disasm::new(&buf, &label_offsets)?.disassemble::<MemWriter>(None)?;
        Ok(Self {
            data: MemReader::new(buf),
            strs,
            encoding,
            texts,
            label_offsets,
        })
    }

    fn load_file(filename: &str, archive: Option<&Box<dyn Script>>, name: &str) -> Result<Vec<u8>> {
        if let Some(archive) = archive {
            Ok(archive
                .open_file_by_name(name, true)
                .map_err(|e| anyhow::anyhow!("Failed to open file {} in archive: {}", name, e))?
                .data()?)
        } else {
            let mut path = std::path::PathBuf::from(filename);
            path.set_file_name(name);
            std::fs::read(path).map_err(|e| anyhow::anyhow!("Failed to read file {}: {}", name, e))
        }
    }

    fn load_texts_data(data: Vec<u8>) -> Result<MemReader> {
        let mut writer = MemWriter::from_vec(data);
        if writer.data.len() >= 0x14 {
            let ind = writer.cpeek_u32_at(0x10)?;
            writer.pos = 0x10;
            if ind != 0 {
                let mut shift = 4;
                for _ in 0..(writer.data.len() / 4 - 4) {
                    let mut data = writer.cpeek_u32()?;
                    let mut add = data.to_le_bytes();
                    add[0] = add[0].rotate_left(shift);
                    shift = (shift + 1) % 8;
                    data = u32::from_le_bytes(add);
                    data ^= 0x084DF873 ^ 0xFF987DEE;
                    writer.write_u32(data)?;
                }
            }
        }
        Ok(MemReader::new(writer.into_inner()))
    }

    fn load_point_data(mut data: MemReader) -> Result<Vec<u32>> {
        let mut magic = [0u8; 16];
        data.read_exact(&mut magic)?;
        if magic != *b"$POINT_LIST_****" {
            return Err(anyhow::anyhow!("Invalid point list magic: {:?}", magic));
        }
        let mut label_offsets = Vec::new();
        while !data.is_eof() {
            label_offsets.push(data.read_u32()? + CODE_OFFSET);
        }
        label_offsets.reverse();
        Ok(label_offsets)
    }
}

impl Script for SoftpalScript {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Json
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn is_output_supported(&self, _: OutputScriptType) -> bool {
        true
    }

    fn multiple_message_files(&self) -> bool {
        true
    }

    fn extract_messages(&self) -> Result<Vec<Message>> {
        let mut messages = Vec::new();
        let mut name = None;
        for str in &self.strs {
            let addr = self.data.cpeek_u32_at(str.offset as u64)?;
            let text = self.texts.cpeek_cstring_at(addr as u64 + 4)?;
            let text =
                decode_to_string(self.encoding, text.as_bytes(), false)?.replace("<br>", "\n");
            match str.typ {
                StringType::Name => {
                    if text.is_empty() {
                        continue; // Skip empty names
                    }
                    name = Some(text);
                }
                StringType::Message => messages.push(Message {
                    name: name.take(),
                    message: text,
                }),
                StringType::Hover => messages.push(Message::new(text, None)),
                StringType::Label => {} // Ignore labels
            }
        }
        Ok(messages)
    }

    fn extract_multiple_messages(&self) -> Result<HashMap<String, Vec<Message>>> {
        let mut hovers = Vec::new();
        let mut messages = Vec::new();
        let mut label = None;
        let mut name = None;
        let mut result = HashMap::new();
        for str in &self.strs {
            let addr = self.data.cpeek_u32_at(str.offset as u64)?;
            let text = self.texts.cpeek_cstring_at(addr as u64 + 4)?;
            let text =
                decode_to_string(self.encoding, text.as_bytes(), false)?.replace("<br>", "\n");
            match str.typ {
                StringType::Name => {
                    if text.is_empty() {
                        continue; // Skip empty names
                    }
                    name = Some(text);
                }
                StringType::Message => messages.push(Message::new(text, name.take())),
                StringType::Hover => hovers.push(Message::new(text, None)),
                StringType::Label => {
                    if !messages.is_empty() {
                        let key = label.take().unwrap_or_else(|| "default".to_string());
                        if result.contains_key(&key) {
                            eprintln!(
                                "Warning: Duplicate label '{}', overwriting previous messages.",
                                key
                            );
                            crate::COUNTER.inc_warning();
                        }
                        result.insert(key, messages);
                        messages = Vec::new();
                    }
                    label = Some(text);
                }
            }
        }
        if !messages.is_empty() {
            let key = label.take().unwrap_or_else(|| "default".to_string());
            result.insert(key, messages);
        }
        if !hovers.is_empty() {
            result.insert("hover".to_string(), hovers);
        }
        Ok(result)
    }

    fn import_messages<'a>(
        &'a self,
        messages: Vec<Message>,
        mut file: Box<dyn WriteSeek + 'a>,
        filename: &str,
        encoding: Encoding,
        replacement: Option<&'a ReplacementTable>,
    ) -> Result<()> {
        let mut texts_filename = std::path::PathBuf::from(filename);
        texts_filename.set_file_name("TEXT.DAT");
        let mut texts = Vec::new();
        let mut reader = self.texts.to_ref();
        reader.pos = 0x10;
        while !reader.is_eof() {
            reader.pos += 4; // Skip index
            texts.push(reader.read_cstring()?)
        }
        let mut texts_file = std::fs::File::create(&texts_filename)
            .map_err(|e| anyhow::anyhow!("Failed to create TEXT.DAT file: {}", e))?;
        file.write_all(&self.data.data)?;
        let mut mes = messages.iter();
        let mut mess = mes.next();
        let texts_data_len = self.texts.data.len() as u32;
        let mut num_offset_map: HashMap<u32, u32> = HashMap::new();
        for str in &self.strs {
            let addr = self.data.cpeek_u32_at(str.offset as u64)?;
            if addr + 4 > texts_data_len {
                continue;
            }
            if str.typ.is_label() {
                continue; // Ignore labels
            }
            let m = match mess {
                Some(m) => m,
                None => return Err(anyhow::anyhow!("Not enough messages.")),
            };
            let mut text = match str.typ {
                StringType::Name => match &m.name {
                    Some(name) => name.clone(),
                    None => return Err(anyhow::anyhow!("Missing name for message.")),
                },
                StringType::Message => {
                    let m = m.message.clone();
                    mess = mes.next();
                    m
                }
                StringType::Hover => {
                    let m = m.message.clone();
                    mess = mes.next();
                    m
                }
                StringType::Label => continue, // Ignore labels
            };
            if let Some(repl) = replacement {
                for (from, to) in repl.map.iter() {
                    text = text.replace(from, to);
                }
            }
            text = text.replace("\n", "<br>");
            let encoded = encode_string(encoding, &text, false)?;
            let s = std::ffi::CString::new(encoded)?;
            let num = texts.len() as u32;
            num_offset_map.insert(num, str.offset);
            texts.push(s);
        }
        if mess.is_some() || mes.next().is_some() {
            return Err(anyhow::anyhow!("Some messages were not processed."));
        }
        texts_file.write_all(b"$TEXT_LIST__")?;
        texts_file.write_u32(texts.len() as u32)?;
        let mut nf = MemWriter::new();
        for (num, text) in texts.into_iter().enumerate() {
            let num = num as u32;
            let newaddr = nf.pos as u32 + 0x10;
            if let Some(offset) = num_offset_map.get(&num) {
                file.write_u32_at(*offset as u64, newaddr)?;
            }
            nf.write_u32(num)?;
            nf.write_cstring(&text)?;
        }
        nf.pos = 0;
        let mut shift = 4;
        for _ in 0..(nf.data.len() / 4) {
            let mut data = nf.cpeek_u32()?;
            data ^= 0x084DF873 ^ 0xFF987DEE;
            let mut add = data.to_le_bytes();
            add[0] = add[0].rotate_right(shift);
            shift = (shift + 1) % 8;
            data = u32::from_le_bytes(add);
            nf.write_u32(data)?;
        }
        texts_file.write_all(&nf.data)?;
        Ok(())
    }

    fn import_multiple_messages<'a>(
        &'a self,
        messages: HashMap<String, Vec<Message>>,
        mut file: Box<dyn WriteSeek + 'a>,
        filename: &str,
        encoding: Encoding,
        replacement: Option<&'a ReplacementTable>,
    ) -> Result<()> {
        let mut texts_filename = std::path::PathBuf::from(filename);
        texts_filename.set_file_name("TEXT.DAT");
        let mut texts = Vec::new();
        let mut reader = self.texts.to_ref();
        reader.pos = 0x10;
        while !reader.is_eof() {
            reader.pos += 4; // Skip index
            texts.push(reader.read_cstring()?)
        }
        let mut texts_file = std::fs::File::create(&texts_filename)
            .map_err(|e| anyhow::anyhow!("Failed to create TEXT.DAT file: {}", e))?;
        file.write_all(&self.data.data)?;
        let hover_messages = messages.get("hover").cloned().unwrap_or_default();
        let mut hover_iter = hover_messages.iter();
        let mut hover_mes = hover_iter.next();
        let mut cur_label: Option<String> = None;
        let mut cur_messages = messages
            .get(cur_label.as_ref().map(|s| s.as_str()).unwrap_or("default"))
            .cloned()
            .unwrap_or_default();
        let mut cur_iter = cur_messages.iter();
        let mut cur_mes = cur_iter.next();
        let texts_data_len = self.texts.data.len() as u32;
        let mut num_offset_map: HashMap<u32, u32> = HashMap::new();
        for str in &self.strs {
            let addr = self.data.cpeek_u32_at(str.offset as u64)?;
            if addr + 4 > texts_data_len {
                continue;
            }
            let mut text = match str.typ {
                StringType::Label => {
                    if cur_mes.is_some() || cur_iter.next().is_some() {
                        return Err(anyhow::anyhow!(
                            "Not all messages were used for label {}.",
                            cur_label.as_ref().map(|s| s.as_str()).unwrap_or("default")
                        ));
                    }
                    let text = self.texts.cpeek_cstring_at(addr as u64 + 4)?;
                    let text = decode_to_string(self.encoding, text.as_bytes(), false)?
                        .replace("<br>", "\n");
                    cur_messages = messages.get(text.as_str()).cloned().unwrap_or_default();
                    cur_iter = cur_messages.iter();
                    cur_mes = cur_iter.next();
                    cur_label = Some(text);
                    // We don't need update labels
                    continue;
                }
                StringType::Hover => {
                    let m = match hover_mes {
                        Some(m) => m,
                        None => return Err(anyhow::anyhow!("Not enough hover messages.")),
                    };
                    let m = m.message.clone();
                    hover_mes = hover_iter.next();
                    m
                }
                StringType::Name => {
                    let m = match cur_mes {
                        Some(m) => m,
                        None => {
                            return Err(anyhow::anyhow!(
                                "Not enough messages for label {}.",
                                cur_label.as_ref().map(|s| s.as_str()).unwrap_or("default")
                            ));
                        }
                    };
                    let name = match &m.name {
                        Some(name) => name.clone(),
                        None => return Err(anyhow::anyhow!("Missing name for message.")),
                    };
                    name
                }
                StringType::Message => {
                    let m = match cur_mes {
                        Some(m) => m,
                        None => {
                            return Err(anyhow::anyhow!(
                                "Not enough messages for label {}.",
                                cur_label.as_ref().map(|s| s.as_str()).unwrap_or("default")
                            ));
                        }
                    };
                    let m = m.message.clone();
                    cur_mes = cur_iter.next();
                    m
                }
            };
            if let Some(repl) = replacement {
                for (from, to) in repl.map.iter() {
                    text = text.replace(from, to);
                }
            }
            text = text.replace("\n", "<br>");
            let encoded = encode_string(encoding, &text, false)?;
            let s = std::ffi::CString::new(encoded)?;
            let num = texts.len() as u32;
            num_offset_map.insert(num, str.offset);
            texts.push(s);
        }
        if cur_mes.is_some() || cur_iter.next().is_some() {
            return Err(anyhow::anyhow!(
                "Some messages were not processed for label {}.",
                cur_label.as_ref().map(|s| s.as_str()).unwrap_or("default")
            ));
        }
        if hover_mes.is_some() || hover_iter.next().is_some() {
            return Err(anyhow::anyhow!("Some hover messages were not processed."));
        }
        texts_file.write_all(b"$TEXT_LIST__")?;
        texts_file.write_u32(texts.len() as u32)?;
        let mut nf = MemWriter::new();
        for (num, text) in texts.into_iter().enumerate() {
            let num = num as u32;
            let newaddr = nf.pos as u32 + 0x10;
            if let Some(offset) = num_offset_map.get(&num) {
                file.write_u32_at(*offset as u64, newaddr)?;
            }
            nf.write_u32(num)?;
            nf.write_cstring(&text)?;
        }
        nf.pos = 0;
        let mut shift = 4;
        for _ in 0..(nf.data.len() / 4) {
            let mut data = nf.cpeek_u32()?;
            data ^= 0x084DF873 ^ 0xFF987DEE;
            let mut add = data.to_le_bytes();
            add[0] = add[0].rotate_right(shift);
            shift = (shift + 1) % 8;
            data = u32::from_le_bytes(add);
            nf.write_u32(data)?;
        }
        texts_file.write_all(&nf.data)?;
        Ok(())
    }

    fn custom_output_extension<'a>(&'a self) -> &'a str {
        "txt"
    }

    fn custom_export(&self, filename: &std::path::Path, _encoding: Encoding) -> Result<()> {
        let file = std::fs::File::create(filename)
            .map_err(|e| anyhow::anyhow!("Failed to create file {}: {}", filename.display(), e))?;
        let mut file = std::io::BufWriter::new(file);
        Disasm::new(&self.data.data, &self.label_offsets)?.disassemble(Some(&mut file))?;
        Ok(())
    }
}
