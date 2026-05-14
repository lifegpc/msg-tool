//! Yu-Ris YSCFG files
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
pub struct YSCFGBuilder {}

impl YSCFGBuilder {
    /// Creates a new instance of `YSERBuilder`
    pub const fn new() -> Self {
        YSCFGBuilder {}
    }
}

impl ScriptBuilder for YSCFGBuilder {
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
        Ok(Box::new(YSCFG::new(MemReader::new(buf), encoding, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ybn"]
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 4 && buf.starts_with(b"YSCF") {
            return Some(20);
        }
        None
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::YurisYSCFG
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
struct YSCFGData {
    engine: u32,
    unk0: u32,
    compile: u32,
    screen_width: u32,
    screen_height: u32,
    enable: u32,
    image_type_slots: [u8; 8],
    sound_type_slots: [u8; 4],
    thread: u32,
    debug_mode: u32,
    sound: u32,
    window_resize: u32,
    window_frame: u32,
    file_priority_dev: u32,
    file_priority_debug: u32,
    file_priority_release: u32,
    unk1: u32,
    // #TODO: Better version handle
    #[skip_pack_if(self.engine < 500)]
    #[skip_unpack_if(engine < 500)]
    unk2: u32,
    #[skip_pack_if(self.engine < 500)]
    #[skip_unpack_if(engine < 500)]
    unk3: u32,
    #[skip_pack_if(self.engine < 500)]
    #[skip_unpack_if(engine < 500)]
    unk4: u32,
    #[pstring(u16)]
    caption: String,
}

#[derive(Debug)]
pub struct YSCFG {
    data: YSCFGData,
    custom_yaml: bool,
}

impl YSCFG {
    pub fn new<T: Read + Seek>(
        mut reader: T,
        encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<Self> {
        let mut sig = [0; 4];
        reader.read_exact(&mut sig)?;
        if &sig != b"YSCF" {
            anyhow::bail!("Unsupported YSCFG file.");
        }
        let data = YSCFGData::unpack(&mut reader, false, encoding, &None)?;
        Ok(Self {
            data,
            custom_yaml: config.custom_yaml,
        })
    }
}

impl Script for YSCFG {
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
    let data: YSCFGData = if yaml {
        serde_yaml_ng::from_str(&s).map_err(|e| anyhow::anyhow!("Failed to parse YAML: {}", e))?
    } else {
        serde_json::from_str(&s).map_err(|e| anyhow::anyhow!("Failed to parse JSON: {}", e))?
    };
    writer.write_all(b"YSCF")?;
    data.pack(&mut writer, false, encoding, &None)?;
    Ok(())
}
