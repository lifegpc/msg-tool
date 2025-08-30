//! Kirikiri TJS NS0 binary encoded script
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::{decode_to_string, encode_string};
use crate::utils::struct_pack::*;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{Read, Seek, Write};

#[derive(Debug)]
/// Kirikiri TJS NS0 Script Builder
pub struct TjsNs0Builder {}

impl TjsNs0Builder {
    /// Creates a new instance of `TjsNs0Builder`
    pub fn new() -> Self {
        Self {}
    }
}

impl ScriptBuilder for TjsNs0Builder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Utf16LE
    }

    fn build_script(
        &self,
        buf: Vec<u8>,
        filename: &str,
        encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(TjsNs0::new(buf, filename, encoding, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["tjs", "pbd"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::KirikiriTjsNs0
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 12 && buf.starts_with(b"TJS/ns0\0TJS\0") {
            return Some(100);
        }
        None
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum TjsValue {
    Void(()),
    Int(i64),
    Str(String),
    Array(Vec<TjsValue>),
    Dict(BTreeMap<String, TjsValue>),
}

fn unpack_string<R: Read + Seek>(reader: &mut R, big: bool, encoding: Encoding) -> Result<String> {
    let len = u32::unpack(reader, big, encoding)? as usize;
    let tlen = if encoding.is_utf16le() { len * 2 } else { len };
    let mut buf = vec![0u8; tlen];
    reader.read_exact(&mut buf)?;
    let s = decode_to_string(encoding, &buf, true)?;
    Ok(s)
}

impl StructUnpack for TjsValue {
    fn unpack<R: Read + Seek>(reader: &mut R, big: bool, encoding: Encoding) -> Result<Self> {
        let typ = u16::unpack(reader, big, encoding)?;
        let typ_byte = (typ & 0xff) as u8;
        Ok(match typ_byte {
            0 => TjsValue::Void(()),
            2 => TjsValue::Str(unpack_string(reader, big, encoding)?),
            4 => TjsValue::Int(i64::unpack(reader, big, encoding)?),
            0x81 => {
                let arr_len = u32::unpack(reader, big, encoding)? as usize;
                let mut arr = Vec::with_capacity(arr_len);
                for _ in 0..arr_len {
                    arr.push(reader.read_struct::<TjsValue>(big, encoding)?);
                }
                TjsValue::Array(arr)
            }
            0xC1 => {
                let kv_len = u32::unpack(reader, big, encoding)? as usize;
                let mut dict = BTreeMap::new();
                for _ in 0..kv_len {
                    let key = unpack_string(reader, big, encoding)?;
                    let value = reader.read_struct::<TjsValue>(big, encoding)?;
                    dict.insert(key, value);
                }
                TjsValue::Dict(dict)
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Unsupported TJS/ns0 value type: {} at pos {}",
                    typ_byte,
                    reader.stream_position()? - 2
                ));
            }
        })
    }
}

#[derive(Debug)]
/// Kirikiri TJS NS0 Script
pub struct TjsNs0 {
    data: TjsValue,
    custom_yaml: bool,
}

impl TjsNs0 {
    /// Creates a new `TjsNs0` script from the given buffer and filename
    ///
    /// * `buf` - The buffer containing the TJS/ns0 data
    /// * `filename` - The name of the file
    /// * `encoding` - The encoding to use for strings
    /// * `config` - Extra configuration options
    pub fn new(
        buf: Vec<u8>,
        _filename: &str,
        encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<Self> {
        let mut reader = MemReader::new(buf);
        let mut header = [0u8; 16];
        reader.read_exact(&mut header)?;
        if &header != b"TJS/ns0\0TJS\0\0\0\0\0" {
            return Err(anyhow::anyhow!("Invalid TJS/ns0 header: {:?}", &header));
        }
        let data = TjsValue::unpack(&mut reader, false, encoding)?;
        Ok(Self {
            data,
            custom_yaml: config.custom_yaml,
        })
    }
}

impl Script for TjsNs0 {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Custom
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn is_output_supported(&self, output: OutputScriptType) -> bool {
        matches!(output, OutputScriptType::Custom)
    }

    fn custom_output_extension<'a>(&'a self) -> &'a str {
        if self.custom_yaml { "yaml" } else { "json" }
    }

    fn custom_export(&self, filename: &std::path::Path, encoding: Encoding) -> Result<()> {
        let s = if self.custom_yaml {
            serde_yaml_ng::to_string(&self.data)?
        } else {
            serde_json::to_string_pretty(&self.data)?
        };
        let s = encode_string(encoding, &s, false)?;
        let mut writer = crate::utils::files::write_file(filename)?;
        writer.write_all(&s)?;
        Ok(())
    }
}
