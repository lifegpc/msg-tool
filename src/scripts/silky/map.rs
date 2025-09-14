use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use anyhow::Result;
use std::io::{Seek, SeekFrom};

#[derive(Debug)]
/// A builder for Silky Engine map scripts.
pub struct MapBuilder {}

impl MapBuilder {
    /// Creates a new `MapBuilder`.
    pub fn new() -> Self {
        Self {}
    }
}

impl ScriptBuilder for MapBuilder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Utf16LE
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
        Ok(Box::new(Map::new(buf, encoding, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["map"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::SilkyMap
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        let reader = MemReaderRef::new(&buf[..buf_len]);
        try_parse(reader).ok()
    }

    fn can_create_file(&self) -> bool {
        true
    }

    fn create_file<'a>(
        &'a self,
        filename: &'a str,
        writer: Box<dyn WriteSeek + 'a>,
        encoding: Encoding,
        file_encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<()> {
        create_file(
            filename,
            writer,
            encoding,
            file_encoding,
            config.custom_yaml,
        )
    }
}

fn create_file<'a>(
    custom_filename: &'a str,
    mut writer: Box<dyn WriteSeek + 'a>,
    encoding: Encoding,
    output_encoding: Encoding,
    yaml: bool,
) -> Result<()> {
    let input = crate::utils::files::read_file(custom_filename)?;
    let s = decode_to_string(output_encoding, &input, true)?;
    let strings: Vec<String> = if yaml {
        serde_yaml_ng::from_str(&s)?
    } else {
        serde_json::from_str(&s)?
    };
    writer.write_u32(strings.len() as u32)?;
    let header_len = 8 * strings.len();
    writer.seek_relative(header_len as i64)?;
    let mut offsets = Vec::with_capacity(strings.len());
    for s in strings {
        offsets.push(writer.stream_position()? as u32);
        let buf = if encoding.is_utf16le() {
            let mut buf = encode_string(encoding, &s, false)?;
            buf.extend_from_slice(&[0, 0]);
            buf
        } else {
            let mut buf = encode_string(encoding, &s, false)?;
            buf.push(0);
            buf
        };
        writer.write_all(&buf)?;
    }
    writer.seek(SeekFrom::Start(4))?;
    for (i, offset) in offsets.iter().enumerate() {
        writer.write_u32(i as u32)?;
        writer.write_u32(*offset)?;
    }
    Ok(())
}

#[derive(Debug)]
/// A Silky Engine map script.
struct Map {
    strings: Vec<String>,
    custom_yaml: bool,
}

impl Map {
    /// Creates a new `Map` from the given buffer and encoding.
    pub fn new(buf: Vec<u8>, encoding: Encoding, config: &ExtraConfig) -> Result<Self> {
        let mut data = MemReader::new(buf);
        let count = data.read_u32()?;
        let mut strings = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let _index = data.read_u32()?;
            let offset = data.read_u32()?;
            if encoding.is_utf16le() {
                let data = data.peek_u16string_at(offset as u64)?;
                let s = decode_to_string(encoding, &data, true)?;
                strings.push(s);
            } else {
                let data = data.peek_cstring_at(offset as u64)?;
                let s = decode_to_string(encoding, data.as_bytes(), true)?;
                strings.push(s);
            }
        }
        Ok(Self {
            strings,
            custom_yaml: config.custom_yaml,
        })
    }
}

impl Script for Map {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Json
    }

    fn is_output_supported(&self, _: OutputScriptType) -> bool {
        true
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn custom_output_extension<'a>(&'a self) -> &'a str {
        if self.custom_yaml { "yaml" } else { "json" }
    }

    fn extract_messages(&self) -> Result<Vec<Message>> {
        let mut messages = Vec::with_capacity(self.strings.len());
        for s in &self.strings {
            messages.push(Message::new(s.replace("\\n", "\n"), None));
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
                "The number of messages does not match. (expected {}, got {})",
                self.strings.len(),
                messages.len()
            ));
        }
        file.write_u32(messages.len() as u32)?;
        let header_len = 8 * messages.len();
        file.seek_relative(header_len as i64)?;
        let mut offsets = Vec::with_capacity(messages.len());
        for msg in messages {
            let mut m = msg.message.clone();
            if let Some(table) = replacement {
                for (k, v) in &table.map {
                    m = m.replace(k, v);
                }
            }
            m = m.replace("\n", "\\n");
            offsets.push(file.stream_position()? as u32);
            let buf = if encoding.is_utf16le() {
                let mut buf = encode_string(encoding, &m, false)?;
                buf.extend_from_slice(&[0, 0]);
                buf
            } else {
                let mut buf = encode_string(encoding, &m, false)?;
                buf.push(0);
                buf
            };
            file.write_all(&buf)?;
        }
        file.seek(SeekFrom::Start(4))?;
        for (i, offset) in offsets.iter().enumerate() {
            file.write_u32(i as u32)?;
            file.write_u32(*offset)?;
        }
        Ok(())
    }

    fn custom_export(&self, filename: &std::path::Path, encoding: Encoding) -> Result<()> {
        let s = if self.custom_yaml {
            serde_yaml_ng::to_string(&self.strings)?
        } else {
            serde_json::to_string_pretty(&self.strings)?
        };
        let s = encode_string(encoding, &s, false)?;
        let mut file = crate::utils::files::write_file(filename)?;
        file.write_all(&s)?;
        Ok(())
    }

    fn custom_import<'a>(
        &'a self,
        custom_filename: &'a str,
        file: Box<dyn WriteSeek + 'a>,
        encoding: Encoding,
        output_encoding: Encoding,
    ) -> Result<()> {
        create_file(
            custom_filename,
            file,
            encoding,
            output_encoding,
            self.custom_yaml,
        )
    }
}

fn try_parse(mut r: MemReaderRef) -> Result<u8> {
    let count = r.read_u32()?;
    let index = r.read_u32()?;
    if index != 0 {
        return Err(anyhow::anyhow!("Invalid index"));
    }
    let mut prv_offset = r.read_u32()?;
    if prv_offset < 4 + 8 * count {
        return Err(anyhow::anyhow!("Invalid offset"));
    }
    let tlen = r.data.len();
    for i in 1..count {
        if r.pos + 8 > tlen {
            break;
        }
        let index = r.read_u32()?;
        if index != i {
            return Err(anyhow::anyhow!("Invalid index"));
        }
        let offset = r.read_u32()?;
        if offset <= prv_offset {
            return Err(anyhow::anyhow!("Invalid offset"));
        }
        prv_offset = offset;
    }
    Ok(if (r.pos - 4) / 8 < 100 { 10 } else { 20 })
}
