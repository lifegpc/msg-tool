use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use anyhow::Result;
use std::io::Read;

#[derive(Debug)]
pub struct CstlScriptBuilder {}

impl CstlScriptBuilder {
    pub fn new() -> Self {
        CstlScriptBuilder {}
    }
}

impl ScriptBuilder for CstlScriptBuilder {
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
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(CstlScript::new(buf, encoding, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["cstl"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::CatSystemCstl
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 4 && buf.starts_with(b"CSTL") {
            return Some(15);
        }
        None
    }
}

trait CustomFn {
    fn read_size(&mut self) -> Result<usize>;
}

impl<T: Read> CustomFn for T {
    fn read_size(&mut self) -> Result<usize> {
        let mut size = 0;
        loop {
            let len = self.read_u8()?;
            size += len as usize;
            if len != 0xFF {
                break;
            }
        }
        Ok(size)
    }
}

#[derive(Debug)]
struct CstlScript {
    langs: Vec<String>,
    data: Vec<Vec<Message>>,
    lang_index: Option<usize>,
}

impl CstlScript {
    pub fn new(buf: Vec<u8>, encoding: Encoding, config: &ExtraConfig) -> Result<Self> {
        let mut langs = Vec::new();
        let mut data = Vec::new();
        let mut reader = MemReader::new(buf);
        let mut magic = [0; 4];
        reader.read_exact(&mut magic)?;
        if &magic != b"CSTL" {
            return Err(anyhow::anyhow!("Invalid CSTL magic number"));
        }
        let unk = reader.read_u32()?;
        if unk != 0 {
            return Err(anyhow::anyhow!("Unknown CSTL unk value: {}", unk));
        }
        let lang_count = reader.read_size()?;
        for _ in 0..lang_count {
            let len = reader.read_size()?;
            let s = reader.read_fstring(len, encoding, false)?;
            langs.push(s);
            data.push(Vec::new());
        }
        let count = reader.read_size()?;
        let mut i = 0;
        loop {
            let name_len = reader.read_size()?;
            let name = if name_len > 0 {
                Some(reader.read_fstring(name_len, encoding, false)?)
            } else {
                None
            };
            let mes_len = reader.read_size()?;
            let message = reader.read_fstring(mes_len, encoding, false)?;
            data[i % lang_count].push(Message { name, message });
            i += 1;
            if reader.is_eof() {
                break;
            }
        }
        if i != count * lang_count {
            return Err(anyhow::anyhow!(
                "CSTL data count mismatch: expected {}, got {}",
                i,
                count * langs.len()
            ));
        }
        for (i, lang) in langs.iter().enumerate() {
            if data[i].len() != count {
                return Err(anyhow::anyhow!(
                    "CSTL language '{}' data count mismatch: expected {}, got {}",
                    lang,
                    count,
                    data[i].len()
                ));
            }
        }
        let lang_index = config
            .cat_system_cstl_lang
            .as_ref()
            .and_then(|lang| langs.iter().position(|l| l == lang));
        if config.cat_system_cstl_lang.is_some() && lang_index.is_none() {
            eprintln!(
                "Warning: specified language '{}' not found in CSTL script",
                config.cat_system_cstl_lang.as_ref().unwrap()
            );
            crate::COUNTER.inc_warning();
        }
        Ok(CstlScript {
            langs,
            data,
            lang_index,
        })
    }
}

impl Script for CstlScript {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Json
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn extract_messages(&self) -> Result<Vec<Message>> {
        if self.langs.is_empty() || self.data.is_empty() {
            return Err(anyhow::anyhow!("CSTL script has no languages or data"));
        }
        Ok(self.data[self.lang_index.unwrap_or(0)].clone())
    }
}
