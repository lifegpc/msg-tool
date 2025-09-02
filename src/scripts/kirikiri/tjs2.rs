//! Kirikiri compiled TJS2 script
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use crate::utils::struct_pack::*;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::io::{Read, Seek, Write};

#[derive(Debug)]
/// Kirikiri TJS2 Script Builder
pub struct Tjs2Builder {}

impl Tjs2Builder {
    /// Creates a new instance of `Tjs2Builder`
    pub fn new() -> Self {
        Self {}
    }
}

impl ScriptBuilder for Tjs2Builder {
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
        Ok(Box::new(Tjs2::new(buf, encoding, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["tjs"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::KirikiriTjs2
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        // TJS2 tag 100 version
        if buf_len >= 8 && buf.starts_with(b"TJS2100\0") {
            return Some(40);
        }
        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DataArea {
    byte_array: Vec<u8>,
    short_array: Vec<i16>,
    long_array: Vec<i32>,
    longlong_array: Vec<i64>,
    double_array: Vec<f64>,
    string_array: Vec<String>,
    octet_array: Vec<Vec<u8>>,
}

impl StructUnpack for DataArea {
    fn unpack<R: Read + Seek>(reader: &mut R, big: bool, encoding: Encoding) -> Result<Self> {
        reader.align(4)?;
        let start_loc = reader.stream_position()?;
        let mut data_tag = [0; 4];
        reader.read_exact(&mut data_tag)?;
        if &data_tag != b"DATA" {
            return Err(anyhow::anyhow!("Invalid DATA tag"));
        }
        let data_size = u32::unpack(reader, big, encoding)?;
        let count = u32::unpack(reader, big, encoding)? as usize;
        let byte_array = reader.read_exact_vec(count)?;
        reader.align(4)?;
        let short_count = u32::unpack(reader, big, encoding)? as usize;
        let short_array = reader.read_struct_vec(short_count, big, encoding)?;
        reader.align(4)?;
        let long_count = u32::unpack(reader, big, encoding)? as usize;
        let long_array = reader.read_struct_vec(long_count, big, encoding)?;
        let longlong_count = u32::unpack(reader, big, encoding)? as usize;
        let longlong_array = reader.read_struct_vec(longlong_count, big, encoding)?;
        let double_count = u32::unpack(reader, big, encoding)? as usize;
        let double_array = reader.read_struct_vec(double_count, big, encoding)?;
        let str_count = u32::unpack(reader, big, encoding)? as usize;
        let mut string_array = Vec::with_capacity(str_count);
        for _ in 0..str_count {
            let str_len = u32::unpack(reader, big, encoding)? as usize;
            let str_bytes = reader.read_exact_vec(if encoding.is_utf16le() {
                str_len * 2
            } else {
                str_len
            })?;
            let s = decode_to_string(encoding, &str_bytes, true)?;
            reader.align(4)?;
            string_array.push(s);
        }
        let octet_count = u32::unpack(reader, big, encoding)? as usize;
        let mut octet_array = Vec::with_capacity(octet_count);
        for _ in 0..octet_count {
            let octet_len = u32::unpack(reader, big, encoding)? as usize;
            let octet_bytes = reader.read_exact_vec(octet_len)?;
            reader.align(4)?;
            octet_array.push(octet_bytes);
        }
        let end_loc = reader.stream_position()?;
        if end_loc - start_loc != data_size as u64 {
            return Err(anyhow::anyhow!(
                "DATA size mismatch: expected {}, got {}",
                data_size,
                end_loc - start_loc
            ));
        }
        Ok(DataArea {
            byte_array,
            short_array,
            long_array,
            longlong_array,
            double_array,
            string_array,
            octet_array,
        })
    }
}

impl StructPack for DataArea {
    fn pack<W: Write>(&self, writer: &mut W, big: bool, encoding: Encoding) -> Result<()> {
        writer.write_all(b"DATA")?;
        let mut tmp = MemWriter::new();
        tmp.write_struct(&(self.byte_array.len() as u32), big, encoding)?;
        tmp.write_all(&self.byte_array)?;
        tmp.align(4)?;
        tmp.write_struct(&(self.short_array.len() as u32), big, encoding)?;
        for v in &self.short_array {
            tmp.write_struct(v, big, encoding)?;
        }
        tmp.align(4)?;
        tmp.write_struct(&(self.long_array.len() as u32), big, encoding)?;
        for v in &self.long_array {
            tmp.write_struct(v, big, encoding)?;
        }
        tmp.write_struct(&(self.longlong_array.len() as u32), big, encoding)?;
        for v in &self.longlong_array {
            tmp.write_struct(v, big, encoding)?;
        }
        tmp.write_struct(&(self.double_array.len() as u32), big, encoding)?;
        for v in &self.double_array {
            tmp.write_struct(v, big, encoding)?;
        }
        tmp.write_struct(&(self.string_array.len() as u32), big, encoding)?;
        for s in &self.string_array {
            let encoded = encode_string(encoding, s, false)?;
            let str_len = if encoding.is_utf16le() {
                encoded.len() / 2
            } else {
                encoded.len()
            };
            tmp.write_struct(&(str_len as u32), big, encoding)?;
            tmp.write_all(&encoded)?;
            tmp.align(4)?;
        }
        tmp.write_struct(&(self.octet_array.len() as u32), big, encoding)?;
        for o in &self.octet_array {
            tmp.write_struct(&(o.len() as u32), big, encoding)?;
            tmp.write_all(o)?;
            tmp.align(4)?;
        }
        // make sure final size is aligned to 4 bytes
        tmp.data.resize(tmp.pos, 0);
        let data = tmp.into_inner();
        writer.write_struct(&(data.len() as u32 + 8), big, encoding)?;
        writer.write_all(&data)?;
        Ok(())
    }
}

/// Kirikiri TJS2 Script
#[derive(Debug)]
pub struct Tjs2 {
    data_area: DataArea,
    remaing: Vec<u8>,
    custom_yaml: bool,
}

impl Tjs2 {
    /// Creates a new `Tjs2` script from the given buffer
    ///
    /// * `buf` - The buffer containing the TJS2 data
    /// * `encoding` - The encoding to use for strings
    /// * `config` - Extra configuration options
    pub fn new(buf: Vec<u8>, encoding: Encoding, config: &ExtraConfig) -> Result<Self> {
        let mut reader = MemReader::new(buf);
        let mut header = [0u8; 8];
        reader.read_exact(&mut header)?;
        if &header != b"TJS2100\0" {
            return Err(anyhow::anyhow!("Invalid TJS2 header: {:?}", &header));
        }
        let _file_size = reader.read_u32()?;
        let data_area = DataArea::unpack(&mut reader, false, encoding)?;
        let mut remaing = Vec::new();
        reader.read_to_end(&mut remaing)?;
        Ok(Self {
            data_area,
            remaing,
            custom_yaml: config.custom_yaml,
        })
    }
}

impl Script for Tjs2 {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Json
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn is_output_supported(&self, _: OutputScriptType) -> bool {
        true
    }

    fn custom_output_extension<'a>(&'a self) -> &'a str {
        if self.custom_yaml { "yaml" } else { "json" }
    }

    fn extract_messages(&self) -> Result<Vec<Message>> {
        let mut messages = Vec::new();
        for s in self.data_area.string_array.iter() {
            messages.push(Message {
                name: None,
                message: s.clone(),
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
        let mut data_area = self.data_area.clone();
        data_area.string_array = messages
            .iter()
            .map(|m| {
                let mut s = m.message.clone();
                if let Some(table) = replacement {
                    for (from, to) in &table.map {
                        s = s.replace(from, to);
                    }
                }
                s
            })
            .collect();
        file.write_all(b"TJS2100\0")?;
        file.write_u32(0)?; // placeholder for file size
        data_area.pack(&mut file, false, encoding)?;
        file.write_all(&self.remaing)?;
        let file_size = file.stream_length()?;
        file.write_u32_at(8, file_size as u32)?; // write actual file size
        Ok(())
    }

    fn custom_export(&self, filename: &std::path::Path, encoding: Encoding) -> Result<()> {
        let s = if self.custom_yaml {
            serde_yaml_ng::to_string(&self.data_area)?
        } else {
            serde_json::to_string_pretty(&self.data_area)?
        };
        let encoded = encode_string(encoding, &s, false)?;
        let mut file = crate::utils::files::write_file(filename)?;
        file.write_all(&encoded)?;
        Ok(())
    }

    fn custom_import<'a>(
        &'a self,
        custom_filename: &'a str,
        mut file: Box<dyn WriteSeek + 'a>,
        encoding: Encoding,
        output_encoding: Encoding,
    ) -> Result<()> {
        let data = crate::utils::files::read_file(custom_filename)?;
        let s = decode_to_string(output_encoding, &data, true)?;
        let data_area: DataArea = if self.custom_yaml {
            serde_yaml_ng::from_str(&s)?
        } else {
            serde_json::from_str(&s)?
        };
        file.write_all(b"TJS2100\0")?;
        file.write_u32(0)?; // placeholder for file size
        data_area.pack(&mut file, false, encoding)?;
        file.write_all(&self.remaing)?;
        let file_size = file.stream_length()?;
        file.write_u32_at(8, file_size as u32)?; // write actual file size
        Ok(())
    }
}
