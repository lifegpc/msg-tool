//! Ported from CSXTools C# project
//! See parent module documentation for more details.
mod disasm;
mod img;
mod types;

use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
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
}

impl CSXScript {
    pub fn new(buf: Vec<u8>, _config: &ExtraConfig) -> Result<Self> {
        let reader = MemReader::new(buf);
        let img = ECSExecutionImage::new(reader)?;
        Ok(Self { img })
    }
}

impl Script for CSXScript {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Custom
    }

    fn is_output_supported(&self, output: OutputScriptType) -> bool {
        matches!(output, OutputScriptType::Custom)
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn custom_output_extension<'a>(&'a self) -> &'a str {
        "s"
    }

    fn custom_export(&self, filename: &std::path::Path, _encoding: Encoding) -> Result<()> {
        let file = crate::utils::files::write_file(filename)?;
        self.img.disasm(Box::new(file))?;
        Ok(())
    }
}
