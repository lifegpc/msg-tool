//! Ported from CSXTools C# project
//! See parent module documentation for more details.
mod disasm;
mod img;
mod types;

use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use anyhow::Result;
use img::ECSExecutionImage;

#[derive(Debug)]
pub struct CSXScriptBuilder {}

impl CSXScriptBuilder {
    pub fn new() -> Self {
        Self {}
    }
}

impl ScriptBuilder for CSXScriptBuilder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Utf16LE
    }

    fn build_script(
        &self,
        buf: Vec<u8>,
        _filename: &str,
        _encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(CSXScript::new(buf, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["csx"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::EntisGlsCsx1
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 8 && &buf[0..8] == b"Entis\x1a\0\0" {
            Some(30)
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct CSXScript {
    img: ECSExecutionImage,
    disasm: bool,
    custom_yaml: bool,
}

impl CSXScript {
    pub fn new(buf: Vec<u8>, config: &ExtraConfig) -> Result<Self> {
        let reader = MemReader::new(buf);
        let img = ECSExecutionImage::new(reader)?;
        Ok(Self {
            img,
            disasm: config.entis_gls_csx_diasm,
            custom_yaml: config.custom_yaml,
        })
    }
}

impl Script for CSXScript {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Json
    }

    fn is_output_supported(&self, _output: OutputScriptType) -> bool {
        true
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn extract_messages(&self) -> Result<Vec<Message>> {
        self.img.export()
    }

    fn multiple_message_files(&self) -> bool {
        true
    }

    fn extract_multiple_messages(&self) -> Result<std::collections::HashMap<String, Vec<Message>>> {
        self.img.export_multi()
    }

    fn custom_output_extension<'a>(&'a self) -> &'a str {
        if self.disasm {
            "d.txt"
        } else if self.custom_yaml {
            "yaml"
        } else {
            "json"
        }
    }

    fn custom_export(&self, filename: &std::path::Path, encoding: Encoding) -> Result<()> {
        if self.disasm {
            let file = crate::utils::files::write_file(filename)?;
            let file = std::io::BufWriter::new(file);
            self.img.disasm(Box::new(file))?;
        } else {
            let messages = self.img.export_all()?;
            let s = if self.custom_yaml {
                serde_yaml_ng::to_string(&messages)?
            } else {
                serde_json::to_string_pretty(&messages)?
            };
            let s = encode_string(encoding, &s, false)?;
            let mut file = crate::utils::files::write_file(filename)?;
            file.write_all(&s)?;
        }
        Ok(())
    }
}
