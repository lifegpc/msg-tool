use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use crate::utils::str::*;
use anyhow::Result;
use std::io::Read;

#[derive(Debug)]
pub struct BinScriptBuilder {}

impl BinScriptBuilder {
    pub fn new() -> Self {
        BinScriptBuilder {}
    }
}

impl ScriptBuilder for BinScriptBuilder {
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
        Ok(Box::new(BinScript::new(buf, encoding, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["bin"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::HexenHaus
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 4 && buf.starts_with(b"NORI") {
            return Some(10);
        }
        None
    }
}

#[derive(Debug)]
struct BinString {
    str: String,
    pos: usize,
    len: usize,
}

#[derive(Debug)]
pub struct BinScript {
    data: MemReader,
    strs: Vec<BinString>,
}

impl BinScript {
    pub fn new(buf: Vec<u8>, encoding: Encoding, _config: &ExtraConfig) -> Result<Self> {
        let mut data = MemReader::new(buf);
        let mut header = [0; 4];
        data.read_exact(&mut header)?;
        if header != *b"NORI" {
            return Err(anyhow::anyhow!("Invalid HexenHaus bin script header"));
        }
        for c in data.data.iter_mut() {
            *c ^= 0x53;
        }
        data.pos = memchr::memmem::find(&data.data, b"_beginrp")
            .ok_or(anyhow::anyhow!("Failed to find _beginrp"))?;
        data.pos += 16;
        let mut p = [0; 2];
        let mut s = Vec::new();
        let data_len = data.data.len();
        let mut start_pos = data.pos;
        let mut strs = Vec::new();
        while data.pos < data_len {
            data.read_exact(&mut p)?;
            if p[0] == 0x53 {
                if s.len() > 2 {
                    if let Ok(c) = decode_to_string(encoding, &s[s.len() - 2..], true) {
                        if c != "」" && c != "。" && c != "』" {
                            s.pop();
                            s.pop();
                        }
                    } else {
                        s.pop();
                        s.pop();
                    }
                }
                if s.len() > 2 {
                    let d = decode_to_string(encoding, &s, true)?;
                    strs.push(BinString {
                        str: d,
                        pos: start_pos,
                        len: s.len(),
                    });
                }
                start_pos = data.pos;
                s.clear();
            } else if p[1] == 0x53 {
                if s.len() > 2 {
                    let d = decode_to_string(encoding, &s, true)?;
                    strs.push(BinString {
                        str: d,
                        pos: start_pos,
                        len: s.len(),
                    });
                }
                start_pos = data.pos;
                s.clear();
            } else {
                s.extend_from_slice(&p);
            }
        }
        if s.len() > 2 {
            s.pop();
            s.pop();
            if s.len() > 2 {
                let d = decode_to_string(encoding, &s, true)?;
                strs.push(BinString {
                    str: d,
                    pos: start_pos,
                    len: s.len(),
                });
            }
        }
        Ok(BinScript { data, strs })
    }
}

impl Script for BinScript {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Json
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn extract_messages(&self) -> Result<Vec<Message>> {
        let mut messages: Vec<Message> = Vec::new();
        for str in &self.strs {
            let message = if let Some(ind) = str.str.find("「") {
                let (name, mes) = str.str.split_at(ind);
                let mut name = name.to_string();
                if name.is_empty() {
                    if let Some(m) = messages.pop() {
                        name = m.message;
                    }
                }
                Message {
                    name: Some(name.to_string()),
                    message: mes.to_string(),
                }
            } else {
                Message {
                name: None,
                message: str.str.clone(),
                }
            };
            messages.push(message);
        }
        Ok(messages)
    }

    fn import_messages<'a>(
            &'a self,
            mut messages: Vec<Message>,
            mut file: Box<dyn WriteSeek + 'a>,
            encoding: Encoding,
            replacement: Option<&'a ReplacementTable>,
        ) -> Result<()> {
        let mut data = MemWriter::from_vec(self.data.data.clone());
        let mut i = 0;
        for str in self.strs.iter() {
            if i >= messages.len() {
                return Err(anyhow::anyhow!("Not enough messages."));
            }
            if let Some(ind) = str.str.find("「") {
                let (name, _) = str.str.split_at(ind);
                let mut target = String::new();
                if !name.is_empty() {
                    let mut name = match &messages[i].name {
                        Some(n) => n.to_owned(),
                        None => return Err(anyhow::anyhow!("Missing name for message.")),
                    };
                    if let Some(repl) = replacement {
                        for (k, v) in &repl.map {
                            name = name.replace(k, v);
                        }
                    };
                    target.push_str(&name);
                }
                let mut mes = messages[i].message.clone();
                if let Some(repl) = replacement {
                    for (k, v) in &repl.map {
                        mes = mes.replace(k, v);
                    }
                }
                target.push_str(&mes);
                let mut encoded = encode_string(encoding, &target, false)?;
                if encoded.len() > str.len {
                    eprintln!("Warning: Message '{}' is too long, truncating.", target);
                    crate::COUNTER.inc_warning();
                    encoded = truncate_string(&target, str.len, encoding, false)?;
                }
                while encoded.len() < str.len {
                    encoded.push(32); // Fill with spaces
                }
                data.write_all_at(str.pos, &encoded)?;
                i += 1;
            } else {
                let mut target = if let Some(name) = messages[i].name.take() {
                    name
                } else {
                    let s = messages[i].message.clone();
                    i += 1;
                    s
                };
                if let Some(repl) = replacement {
                    for (k, v) in &repl.map {
                        target = target.replace(k, v);
                    }
                }
                let mut encoded = encode_string(encoding, &target, false)?;
                if encoded.len() > str.len {
                    eprintln!("Warning: Message '{}' is too long, truncating.", target);
                    crate::COUNTER.inc_warning();
                    encoded = truncate_string(&target, str.len, encoding, false)?;
                }
                while encoded.len() < str.len {
                    encoded.push(32); // Fill with spaces
                }
                data.write_all_at(str.pos, &encoded)?;
            }
        }
        let mut data = data.into_inner();
        for d in data.iter_mut() {
            *d ^= 0x53;
        }
        file.write_all(&data)?;
        Ok(())
    }
}
