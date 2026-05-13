//! Yu-Ris YSCM files
use super::types::*;
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use crate::utils::struct_pack::*;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::io::{Read, Seek};

#[derive(Debug)]
pub struct YSCMBuilder {}

impl YSCMBuilder {
    /// Creates a new instance of `YSCMBuilder`
    pub const fn new() -> Self {
        YSCMBuilder {}
    }
}

impl ScriptBuilder for YSCMBuilder {
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
        Ok(Box::new(YSCM::new(MemReader::new(buf), encoding, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ybn"]
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 4 && buf.starts_with(b"YSCM") {
            return Some(20);
        }
        None
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::YurisYSCM
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct YSCMData {
    pub engine: u32,
    pub opcodes: Vec<CodeMeta>,
}

#[derive(Debug)]
pub struct YSCM {
    pub(crate) data: YSCMData,
    custom_yaml: bool,
}

impl YSCM {
    pub fn new<T: Read + Seek>(
        mut reader: T,
        encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<Self> {
        let mut sig = [0; 4];
        reader.read_exact(&mut sig)?;
        if &sig != b"YSCM" {
            anyhow::bail!("Unsupported YSCM file.");
        }
        let engine = reader.read_u32()?;
        let opcode_count = reader.read_u32()?;
        reader.skip(4)?;
        let mut opcodes = Vec::with_capacity(opcode_count as usize);
        for _ in 0..opcode_count {
            opcodes.push(CodeMeta::unpack(&mut reader, false, encoding, &None)?);
        }
        Ok(Self {
            data: YSCMData { engine, opcodes },
            custom_yaml: config.custom_yaml,
        })
    }
}

impl Script for YSCM {
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
}
