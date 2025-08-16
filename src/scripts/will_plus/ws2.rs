//! WillPlus Script File (.ws2)
use super::ws2_disasm::*;
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use crate::utils::str::*;
use anyhow::Result;
use std::io::{Seek, SeekFrom, Write};

#[derive(Debug)]
/// WillPlus Script Builder
pub struct Ws2ScriptBuilder {}

impl Ws2ScriptBuilder {
    /// Creates a new instance of `Ws2ScriptBuilder`
    pub fn new() -> Self {
        Ws2ScriptBuilder {}
    }
}

impl ScriptBuilder for Ws2ScriptBuilder {
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
        if !config.will_plus_ws2_no_disasm {
            match Ws2DisasmScript::new(&buf, encoding, config, false) {
                Ok(script) => return Ok(Box::new(script)),
                Err(e) => {
                    eprintln!(
                        "WARNING: Failed to disassemble WS2 script: {}. An another parser is used.",
                        e
                    );
                    crate::COUNTER.inc_warning();
                }
            }
        }
        Ok(Box::new(Ws2Script::new(buf, encoding, config, false)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ws2"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::WillPlusWs2
    }
}

trait CustomFn {
    /// check if the current reader's position matches the given byte slice
    /// 0xFF in the slice is treated as a wildcard that matches any byte
    fn equal(&self, other: &[u8]) -> bool;
    /// Reads a string from the current position, decodes it using the specified encoding,
    fn get_ws2_string(&self, encoding: Encoding) -> Result<Ws2String>;
}

impl CustomFn for MemReader {
    fn equal(&self, other: &[u8]) -> bool {
        self.to_ref().equal(other)
    }

    fn get_ws2_string(&self, encoding: Encoding) -> Result<Ws2String> {
        self.to_ref().get_ws2_string(encoding)
    }
}

impl<'a> CustomFn for MemReaderRef<'a> {
    fn equal(&self, other: &[u8]) -> bool {
        if self.pos + other.len() > self.data.len() {
            return false;
        }
        for (i, &byte) in other.iter().enumerate() {
            if self.data[self.pos + i] != byte && byte != 0xFF {
                return false;
            }
        }
        true
    }

    fn get_ws2_string(&self, encoding: Encoding) -> Result<Ws2String> {
        let pos = self.pos;
        let s = self.cpeek_cstring()?;
        let decoded = decode_to_string(encoding, s.as_bytes(), true)?;
        Ok(Ws2String {
            pos,
            str: decoded,
            len: s.as_bytes_with_nul().len(),
            actor: None,
        })
    }
}

struct EncryptWriter<T: Write + Seek> {
    writer: T,
}

impl<T: Write + Seek> EncryptWriter<T> {
    pub fn new(writer: T) -> Self {
        EncryptWriter { writer }
    }
}

impl<T: Write + Seek> Write for EncryptWriter<T> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let encrypted: Vec<u8> = buf.iter().map(|&b| b.rotate_left(2)).collect();
        self.writer.write(&encrypted)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

impl<T: Write + Seek> Seek for EncryptWriter<T> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.writer.seek(pos)
    }
    fn stream_position(&mut self) -> std::io::Result<u64> {
        self.writer.stream_position()
    }
    fn rewind(&mut self) -> std::io::Result<()> {
        self.writer.rewind()
    }
    fn seek_relative(&mut self, offset: i64) -> std::io::Result<()> {
        self.writer.seek_relative(offset)
    }
}

#[derive(Debug)]
struct Ws2String {
    pos: usize,
    str: String,
    /// Length of the string in bytes, including the null terminator
    len: usize,
    actor: Option<Box<Ws2String>>,
}

#[derive(Debug)]
/// WillPlus Script (without disassembly)
pub struct Ws2Script {
    data: MemReader,
    strs: Vec<Ws2String>,
    /// Need encrypt when outputting
    encrypted: bool,
}

impl Ws2Script {
    /// Creates a new `Ws2Script`
    ///
    /// * `buf` - The buffer containing the script data
    /// * `encoding` - The encoding used for the script
    /// * `config` - Extra configuration options
    /// * `decrypted` - Whether the script is decrypted or not
    pub fn new(
        buf: Vec<u8>,
        encoding: Encoding,
        config: &ExtraConfig,
        decrypted: bool,
    ) -> Result<Self> {
        let mut reader = MemReader::new(buf);
        let mut strs = Vec::new();
        let mut actor = None;
        while !reader.is_eof() {
            if reader.equal(b"\x00\xFF\x0F\x02") {
                reader.pos += 4;
                if reader.cpeek_u8()? == 0 {
                    reader.pos += 1;
                    continue;
                }
                let mut continu = true;
                while !reader.is_eof() && continu {
                    reader.pos += 2;
                    let str = reader.get_ws2_string(encoding)?;
                    reader.pos += str.len + 4;
                    while reader.cpeek_u8()? != 0 {
                        reader.pos += 1;
                    }
                    reader.pos += 1;
                    if reader.cpeek_u8()? == 0xFF {
                        continu = false;
                    }
                    strs.push(str);
                }
            }
            if reader.equal(b"%LC") || reader.equal(b"%LF") {
                reader.pos += 3;
                let str = Box::new(reader.get_ws2_string(encoding)?);
                reader.pos += str.len + 4;
                actor = Some(str);
            }
            if reader.equal(b"char\0") {
                reader.pos += 5;
                let mut str = reader.get_ws2_string(encoding)?;
                reader.pos += str.len + 4;
                str.actor = actor.take();
                strs.push(str);
            }
            reader.pos += 1;
        }
        if !decrypted && strs.is_empty() {
            let mut data = reader.inner();
            Self::decrypt(&mut data);
            return Self::new(data, encoding, config, true);
        }
        Ok(Self {
            data: reader,
            strs,
            encrypted: decrypted,
        })
    }

    fn decrypt(data: &mut [u8]) {
        for byte in data.iter_mut() {
            *byte = (*byte).rotate_right(2);
        }
    }
}

impl Script for Ws2Script {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Json
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn extract_messages(&self) -> Result<Vec<Message>> {
        let mut messages = Vec::new();
        for str in &self.strs {
            let message = Message {
                message: str.str.trim_end_matches("%K%P").to_string(),
                name: str.actor.as_ref().map(|a| {
                    a.str
                        .trim_start_matches("%LC")
                        .trim_start_matches("%LF")
                        .to_string()
                }),
            };
            messages.push(message);
        }
        Ok(messages)
    }

    fn import_messages<'a>(
        &'a self,
        messages: Vec<Message>,
        file: Box<dyn WriteSeek + 'a>,
        _filename: &str,
        encoding: Encoding,
        replacement: Option<&'a ReplacementTable>,
    ) -> Result<()> {
        let mut mes = messages.iter();
        let mut m = mes.next();
        let mut file = if self.encrypted {
            Box::new(EncryptWriter::new(file))
        } else {
            file
        };
        file.write_all(&self.data.data)?;
        for str in &self.strs {
            let me = match m {
                Some(m) => m,
                None => {
                    return Err(anyhow::anyhow!("No enough messages."));
                }
            };
            if let Some(actor) = &str.actor {
                let prefix = if actor.str.starts_with("%LC") {
                    "%LC"
                } else if actor.str.starts_with("%LF") {
                    "%LF"
                } else {
                    ""
                };
                let target_len = actor.len - prefix.len() - 1; // -1 for null terminator
                let mut name = match me.name.as_ref() {
                    Some(name) => name.to_owned(),
                    None => return Err(anyhow::anyhow!("Message without name.")),
                };
                if let Some(replacement) = replacement {
                    for (k, v) in &replacement.map {
                        name = name.replace(k, v);
                    }
                }
                let mut encoded = encode_string(encoding, &name, true)?;
                if encoded.len() > target_len {
                    eprintln!("Warning: Name '{}' is too long, truncating.", name);
                    crate::COUNTER.inc_warning();
                    encoded = truncate_string(&name, target_len, encoding, true)?;
                }
                encoded.resize(target_len, 0x20); // Fill with spaces
                file.write_all_at(actor.pos as u64 + prefix.len() as u64, &encoded)?;
            }
            let suffix = if str.str.ends_with("%K%P") {
                "%K%P"
            } else {
                ""
            };
            let target_len = str.len - suffix.len() - 1; // -1 for null terminator
            let mut message = me.message.clone();
            if let Some(replacement) = replacement {
                for (k, v) in &replacement.map {
                    message = message.replace(k, v);
                }
            }
            let mut encoded = encode_string(encoding, &message, true)?;
            if encoded.len() > target_len {
                eprintln!("Warning: Message '{}' is too long, truncating.", message);
                crate::COUNTER.inc_warning();
                encoded = truncate_string(&message, target_len, encoding, true)?;
            }
            encoded.resize(target_len, 0x20); // Fill with spaces
            file.write_all_at(str.pos as u64, &encoded)?;
            m = mes.next();
        }
        if m.is_some() || mes.next().is_some() {
            return Err(anyhow::anyhow!("Too many messages provided."));
        }
        Ok(())
    }
}

#[derive(Debug)]
/// WillPlus Disassembled Script
pub struct Ws2DisasmScript {
    data: MemReader,
    texts: Vec<Ws2DString>,
    addresses: Vec<usize>,
    /// Need encrypt when outputting
    encrypted: bool,
    encoding: Encoding,
}

impl Ws2DisasmScript {
    /// Creates a new `Ws2DisasmScript`
    ///
    /// * `buf` - The buffer containing the script data
    /// * `encoding` - The encoding used for the script
    /// * `config` - Extra configuration options
    /// * `decrypted` - Whether the script is decrypted or not
    pub fn new(
        buf: &[u8],
        encoding: Encoding,
        config: &ExtraConfig,
        decrypted: bool,
    ) -> Result<Self> {
        match disassmble(&buf) {
            Ok((addresses, texts)) => {
                return Ok(Self {
                    data: MemReader::new(buf.to_vec()),
                    texts,
                    addresses,
                    encrypted: decrypted,
                    encoding,
                });
            }
            Err(e) => {
                if decrypted {
                    return Err(e);
                } else {
                    let mut data = buf.to_vec();
                    Ws2Script::decrypt(&mut data);
                    return Self::new(&data, encoding, config, true);
                }
            }
        }
    }
}

impl Script for Ws2DisasmScript {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Json
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn extract_messages(&self) -> Result<Vec<Message>> {
        let mut messages = Vec::new();
        let mut name = None;
        for text in &self.texts {
            match text.typ {
                StringType::Name => {
                    let text = decode_to_string(self.encoding, text.text.as_bytes(), false)?
                        .trim_start_matches("%LC")
                        .trim_start_matches("%LF")
                        .to_string();
                    name = Some(text);
                }
                StringType::Message => {
                    let message = decode_to_string(self.encoding, text.text.as_bytes(), false)?
                        .trim_end_matches("%K%P")
                        .to_string();
                    messages.push(Message {
                        message,
                        name: name.take(),
                    });
                }
                StringType::Internal => {}
            }
        }
        Ok(messages)
    }

    fn import_messages<'a>(
        &'a self,
        messages: Vec<Message>,
        file: Box<dyn WriteSeek + 'a>,
        _filename: &str,
        encoding: Encoding,
        replacement: Option<&'a ReplacementTable>,
    ) -> Result<()> {
        let mut output = if self.encrypted {
            Box::new(EncryptWriter::new(file))
        } else {
            file
        };
        let mut mes = messages.iter();
        let mut mess = mes.next();
        {
            let mut patcher = BinaryPatcher::new(
                MemReaderRef::new(&self.data.data),
                &mut output,
                |s| Ok(s),
                |s| Ok(s),
            )?;
            for s in &self.texts {
                let mut encoded = match s.typ {
                    StringType::Name => {
                        let prefix = if s.text.as_bytes().starts_with(b"%LC") {
                            "%LC"
                        } else if s.text.as_bytes().starts_with(b"%LF") {
                            "%LF"
                        } else {
                            ""
                        };
                        let m = match mess {
                            Some(m) => m,
                            None => {
                                return Err(anyhow::anyhow!("No enough messages."));
                            }
                        };
                        let mut name = match m.name.as_ref() {
                            Some(name) => name.to_owned(),
                            None => return Err(anyhow::anyhow!("Message without name.")),
                        };
                        if let Some(replacement) = replacement {
                            for (k, v) in &replacement.map {
                                name = name.replace(k, v);
                            }
                        }
                        name = prefix.to_owned() + &name;
                        encode_string(encoding, &name, false)?
                    }
                    StringType::Message => {
                        let suffix = if s.text.as_bytes().ends_with(b"%K%P") {
                            "%K%P"
                        } else {
                            ""
                        };
                        let m = match mess {
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
                        mess = mes.next();
                        message.push_str(suffix);
                        encode_string(encoding, &message, false)?
                    }
                    StringType::Internal => s.text.as_bytes().to_vec(),
                };
                encoded.push(0); // Null terminator
                patcher.copy_up_to(s.offset as u64)?;
                patcher.replace_bytes(s.len as u64, &encoded)?;
            }
            if mess.is_some() || mes.next().is_some() {
                return Err(anyhow::anyhow!("Too many messages provided."));
            }
            patcher.copy_up_to(self.data.data.len() as u64)?;
            for offset in &self.addresses {
                patcher.patch_u32_address(*offset as u64)?;
            }
        }
        output.flush()?;
        Ok(())
    }
}
