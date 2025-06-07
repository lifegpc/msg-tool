use super::parser::*;
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::{decode_to_string, encode_string};
use anyhow::Result;

#[derive(Debug)]
pub struct BGIScriptBuilder {}

impl BGIScriptBuilder {
    pub fn new() -> Self {
        BGIScriptBuilder {}
    }
}

impl ScriptBuilder for BGIScriptBuilder {
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
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(BGIScript::new(buf, encoding, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::BGI
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len > 28 && buf.starts_with(b"BurikoCompiledScriptVer1.00\0") {
            return Some(255);
        }
        None
    }
}

pub struct BGIScript {
    data: MemReader,
    encoding: Encoding,
    strings: Vec<BGIString>,
    is_v1: bool,
    offset: usize,
}

impl std::fmt::Debug for BGIScript {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BGIScript")
            .field("encoding", &self.encoding)
            .finish_non_exhaustive()
    }
}

impl BGIScript {
    pub fn new(data: Vec<u8>, encoding: Encoding, _config: &ExtraConfig) -> Result<Self> {
        let data = MemReader::new(data);
        if data.data.starts_with(b"BurikoCompiledScriptVer1.00\0") {
            let mut parser = V1Parser::new(data.to_ref(), encoding)?;
            parser.disassemble()?;
            let strings = parser.strings.clone();
            let offset = parser.offset;
            Ok(Self {
                data,
                encoding,
                strings,
                is_v1: true,
                offset,
            })
        } else {
            let mut parser = V0Parser::new(data.to_ref());
            parser.disassemble()?;
            let strings = parser.strings.clone();
            Ok(Self {
                data,
                encoding,
                strings,
                is_v1: false,
                offset: 0,
            })
        }
    }

    fn read_string(&self, offset: usize) -> Result<String> {
        let start = self.offset + offset;
        let mut end = start;
        while self.data.data[end] != 0 {
            end += 1;
            if end >= self.data.data.len() {
                return Err(anyhow::anyhow!("String not null-terminated"));
            }
        }
        let string_data = &self.data.data[start..end];
        let string = decode_to_string(self.encoding, string_data)?;
        Ok(string)
    }
}

impl Script for BGIScript {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Json
    }

    fn default_format_type(&self) -> FormatOptions {
        if self.is_v1 {
            FormatOptions::None
        } else {
            FormatOptions::Fixed {
                length: 32,
                keep_original: false,
            }
        }
    }

    fn extract_messages(&self) -> Result<Vec<Message>> {
        let mut messages = Vec::new();
        let mut name = None;
        for bgi_string in &self.strings {
            match bgi_string.typ {
                BGIStringType::Name => {
                    name = Some(self.read_string(bgi_string.address)?);
                }
                BGIStringType::Message => {
                    let message = self.read_string(bgi_string.address)?;
                    messages.push(Message {
                        name: name.take(),
                        message: message,
                    });
                }
                _ => {}
            }
        }
        Ok(messages)
    }

    fn import_messages<'a>(
        &'a self,
        _messages: Vec<Message>,
        _filename: Box<dyn WriteSeek + 'a>,
        _encoding: Encoding,
        _replacement: Option<&'a ReplacementTable>,
    ) -> Result<()> {
        Ok(())
    }
}
