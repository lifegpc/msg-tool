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
        let lang_count = reader.read_u8()? as usize;
        for _ in 0..lang_count {
            let len = reader.read_u8()? as usize;
            let s = reader.read_fstring(len, encoding, false)?;
            langs.push(s);
            data.push(Vec::new());
        }
        let mut count = 0;
        loop {
            let len = reader.read_u8()?;
            if len == 0 {
                break; // End of data
            }
            count += len as usize;
        }
        let mut i = 0;
        let mut name = None;
        loop {
            let len = reader.read_u8()?;
            let s = reader.read_fstring(len as usize, encoding, false)?;
            if reader.is_eof() {
                data[i % lang_count].push(Message {
                    name: name.take(),
                    message: s,
                });
                i += 1;
                break;
            } else {
                let e = reader.read_u8()?;
                if e != 0 {
                    data[i % lang_count].push(Message {
                        name: name.take(),
                        message: s,
                    });
                    let s = reader.read_fstring(e as usize, encoding, false)?;
                    name = Some(s);
                    i += 1;
                } else {
                    data[i % lang_count].push(Message {
                        name: name.take(),
                        message: s,
                    });
                    i += 1;
                }
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
