//! Buriko General Interpreter/Ethornell BSI Script (._bsi)
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::{decode_to_string, encode_string};
use anyhow::Result;
use std::collections::BTreeMap;
use std::ffi::CString;

#[derive(Debug)]
/// Builder for BGI BSI scripts.
pub struct BGIBsiScriptBuilder {}

impl BGIBsiScriptBuilder {
    /// Creates a new instance of `BGIBsiScriptBuilder`.
    pub fn new() -> Self {
        BGIBsiScriptBuilder {}
    }
}

impl ScriptBuilder for BGIBsiScriptBuilder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Utf8
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
        Ok(Box::new(BGIBsiScript::new(buf, encoding, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["_bsi"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::BGIBsi
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
        _config: &ExtraConfig,
    ) -> Result<()> {
        create_file(filename, writer, encoding, file_encoding)
    }
}

#[derive(Debug)]
/// BGI BSI script.
pub struct BGIBsiScript {
    /// Section name and its data map.
    pub data: BTreeMap<String, BTreeMap<String, String>>,
}

impl BGIBsiScript {
    /// Creates a new instance of `BGIBsiScript` from a buffer.
    ///
    /// * `buf` - The buffer containing the script data.
    /// * `encoding` - The encoding of the script.
    /// * `config` - Extra configuration options.
    pub fn new(buf: Vec<u8>, encoding: Encoding, _config: &ExtraConfig) -> Result<Self> {
        let mut data = BTreeMap::new();
        let mut reader = MemReader::new(buf);
        let section_count = reader.read_u32()?;
        for _ in 0..section_count {
            let section_name = reader.read_cstring()?;
            let section_name = decode_to_string(encoding, section_name.as_bytes(), true)?;
            let mut section_data = BTreeMap::new();
            let entry_count = reader.read_u32()?;
            for _ in 0..entry_count {
                let key = reader.read_cstring()?;
                let key = decode_to_string(encoding, key.as_bytes(), true)?;
                let value = reader.read_cstring()?;
                let value = decode_to_string(encoding, value.as_bytes(), true)?;
                section_data.insert(key, value);
            }
            data.insert(section_name, section_data);
        }
        if !reader.is_eof() {
            eprintln!(
                "Warning: BGIBsiScript data not fully read, remaining bytes: {}",
                reader.data.len() - reader.pos
            );
            crate::COUNTER.inc_warning();
        }
        Ok(BGIBsiScript { data })
    }
}

impl Script for BGIBsiScript {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Custom
    }

    fn is_output_supported(&self, output: OutputScriptType) -> bool {
        matches!(output, OutputScriptType::Custom)
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn custom_output_extension(&self) -> &'static str {
        "json"
    }

    fn custom_export(&self, filename: &std::path::Path, encoding: Encoding) -> Result<()> {
        let s = serde_json::to_string_pretty(&self.data)
            .map_err(|e| anyhow::anyhow!("Failed to write BSI Map data to JSON: {}", e))?;
        let mut writer = crate::utils::files::write_file(filename)?;
        let s = encode_string(encoding, &s, false)?;
        writer.write_all(&s)?;
        writer.flush()?;
        Ok(())
    }

    fn custom_import<'a>(
        &'a self,
        custom_filename: &'a str,
        file: Box<dyn WriteSeek + 'a>,
        encoding: Encoding,
        output_encoding: Encoding,
    ) -> Result<()> {
        create_file(custom_filename, file, encoding, output_encoding)
    }
}

fn create_file<'a>(
    custom_filename: &'a str,
    mut writer: Box<dyn WriteSeek + 'a>,
    encoding: Encoding,
    output_encoding: Encoding,
) -> Result<()> {
    let input = crate::utils::files::read_file(custom_filename)?;
    let s = decode_to_string(output_encoding, &input, true)?;
    let data: BTreeMap<String, BTreeMap<String, String>> = serde_json::from_str(&s)
        .map_err(|e| anyhow::anyhow!("Failed to read BSI Map data from JSON: {}", e))?;
    writer.write_u32(data.len() as u32)?;
    for (section_name, section_data) in data {
        let section_name_bytes = encode_string(encoding, &section_name, false)?;
        let section_name = CString::new(section_name_bytes)?;
        writer.write_cstring(&section_name)?;
        writer.write_u32(section_data.len() as u32)?;
        for (key, value) in section_data {
            let key_bytes = encode_string(encoding, &key, false)?;
            let key = CString::new(key_bytes)?;
            writer.write_cstring(&key)?;
            let value_bytes = encode_string(encoding, &value, false)?;
            let value = CString::new(value_bytes)?;
            writer.write_cstring(&value)?;
        }
    }
    Ok(())
}
