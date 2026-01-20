//! Entis GLS CSX Script Support
//!
//! Ported from Crsky/EntisGLS_Tools C# project  
//! Original license: GPL-3.0
mod base;
mod v1;
mod v2;

use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use anyhow::Result;
use base::ECSImage;
use v1::ECSExecutionImageV1;
use v2::ECSExecutionImageV2;

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
        &ScriptType::EntisGlsCsx
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
    img: Box<dyn ECSImage>,
    disasm: bool,
    custom_yaml: bool,
}

impl CSXScript {
    pub fn new(buf: Vec<u8>, config: &ExtraConfig) -> Result<Self> {
        let reader = MemReader::new(buf);
        let img = {
            match ECSExecutionImageV1::new(reader.to_ref(), config) {
                Ok(img) => Box::new(img),
                Err(_) => Box::new(ECSExecutionImageV2::new(reader.to_ref(), config)?)
                    as Box<dyn ECSImage>,
            }
        };
        Ok(Self {
            img,
            disasm: config.entis_gls_csx_disasm,
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

    fn import_messages<'a>(
        &'a self,
        messages: Vec<Message>,
        file: Box<dyn WriteSeek + 'a>,
        _filename: &str,
        _encoding: Encoding,
        replacement: Option<&'a ReplacementTable>,
    ) -> Result<()> {
        self.img.import(messages, file, replacement)
    }

    fn multiple_message_files(&self) -> bool {
        true
    }

    fn extract_multiple_messages(&self) -> Result<std::collections::HashMap<String, Vec<Message>>> {
        self.img.export_multi()
    }

    fn import_multiple_messages<'a>(
        &'a self,
        messages: std::collections::HashMap<String, Vec<Message>>,
        file: Box<dyn WriteSeek + 'a>,
        _filename: &str,
        _encoding: Encoding,
        replacement: Option<&'a ReplacementTable>,
    ) -> Result<()> {
        self.img.import_multi(messages, file, replacement)
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

    fn custom_import<'a>(
        &'a self,
        custom_filename: &'a str,
        file: Box<dyn WriteSeek + 'a>,
        _encoding: Encoding,
        output_encoding: Encoding,
    ) -> Result<()> {
        if self.disasm {
            Err(anyhow::anyhow!(
                "Importing from disassembly is not supported."
            ))
        } else {
            let data = crate::utils::files::read_file(custom_filename)?;
            let s = decode_to_string(output_encoding, &data, false)?;
            let messages: Vec<String> = if self.custom_yaml {
                serde_yaml_ng::from_str(&s)?
            } else {
                serde_json::from_str(&s)?
            };
            self.img.import_all(messages, file)?;
            Ok(())
        }
    }
}
