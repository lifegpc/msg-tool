//! Yu-Ris YSER files
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use crate::utils::struct_pack::*;
use anyhow::Result;
use msg_tool_macro::*;
use serde::{Deserialize, Serialize};
use std::io::{Read, Seek, Write};

#[derive(Debug)]
pub struct YSERBuilder {}

impl YSERBuilder {
    /// Creates a new instance of `YSERBuilder`
    pub const fn new() -> Self {
        YSERBuilder {}
    }
}

impl ScriptBuilder for YSERBuilder {
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
    ) -> Result<Box<dyn Script + Send + Sync>> {
        Ok(Box::new(YSER::new(MemReader::new(buf), encoding, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ybn"]
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 4 && buf.starts_with(b"YSER") {
            return Some(20);
        }
        None
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::YurisYSER
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

#[derive(Debug, StructPack, StructUnpack, Deserialize, Serialize)]
struct StringData {
    unk: u32,
    #[cstring]
    s: String,
}

#[derive(Debug, StructPack, StructUnpack, Deserialize, Serialize)]
struct YSERData {
    engine: u32,
    #[pvec(u64)]
    strings: Vec<StringData>,
}

#[derive(Debug)]
pub struct YSER {
    data: YSERData,
    custom_yaml: bool,
}

impl YSER {
    pub fn new<T: Read + Seek>(
        mut reader: T,
        encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<Self> {
        let mut sig = [0; 4];
        reader.read_exact(&mut sig)?;
        if &sig != b"YSER" {
            anyhow::bail!("Unsupported YSER file.");
        }
        let data = YSERData::unpack(&mut reader, false, encoding, &None)?;
        Ok(Self {
            data,
            custom_yaml: config.custom_yaml,
        })
    }
}

impl Script for YSER {
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
        if self.custom_yaml { "yaml" } else { "json" }
    }

    fn custom_export(&self, filename: &std::path::Path, encoding: Encoding) -> Result<()> {
        let s = if self.custom_yaml {
            serde_yaml_ng::to_string(&self.data)
                .map_err(|e| anyhow::anyhow!("Failed to serialize to YAML: {}", e))?
        } else {
            serde_json::to_string_pretty(&self.data)
                .map_err(|e| anyhow::anyhow!("Failed to serialize to JSON: {}", e))?
        };
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
        create_file(
            custom_filename,
            file,
            encoding,
            output_encoding,
            self.custom_yaml,
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
    let data: YSERData = if yaml {
        serde_yaml_ng::from_str(&s).map_err(|e| anyhow::anyhow!("Failed to parse YAML: {}", e))?
    } else {
        serde_json::from_str(&s).map_err(|e| anyhow::anyhow!("Failed to parse JSON: {}", e))?
    };
    writer.write_all(b"YSER")?;
    data.pack(&mut writer, false, encoding, &None)?;
    Ok(())
}
