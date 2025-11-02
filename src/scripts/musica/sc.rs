//! Musica Script (.sc)
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use anyhow::Result;
use std::io::Write;

#[derive(Debug)]
/// Musica Script Builder
pub struct MusicaBuilder {}

impl MusicaBuilder {
    /// Create a new MusicaBuilder
    pub fn new() -> Self {
        MusicaBuilder {}
    }
}

impl ScriptBuilder for MusicaBuilder {
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
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(MusicaScript::new(buf, encoding, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["sc"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::Musica
    }
}

#[derive(Debug)]
pub struct MusicaScript {
    lines: Vec<Vec<String>>,
}

impl MusicaScript {
    pub fn new(buf: Vec<u8>, encoding: Encoding, _config: &ExtraConfig) -> Result<Self> {
        let decoded = decode_to_string(encoding, &buf, true)?;
        let mut lines = Vec::new();
        for line in decoded.lines() {
            let parts: Vec<String> = line.split(' ').map(|s| s.to_string()).collect();
            lines.push(parts);
        }
        Ok(MusicaScript { lines })
    }
}

impl Script for MusicaScript {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Json
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn extract_messages(&self) -> Result<Vec<Message>> {
        let mut messages = Vec::new();
        for parts in &self.lines {
            if parts.is_empty() {
                continue;
            }
            // .message <id> <voice> <name> <text>
            if parts[0] == ".message" && parts.len() >= 5 {
                let name = parts[3].clone();
                let text = parts[4].clone();
                let message = Message {
                    name: if name.is_empty() { None } else { Some(name) },
                    message: text,
                };
                messages.push(message);
            }
        }
        Ok(messages)
    }

    fn import_messages<'a>(
        &'a self,
        messages: Vec<Message>,
        file: Box<dyn WriteSeek + 'a>,
        _filename: &str,
        encoding: Encoding,
        replacement: Option<&'a ReplacementTable>,
    ) -> Result<()> {
        let mut writer = std::io::BufWriter::new(file);
        let mut mes = messages.iter();
        let mut me = mes.next();
        for parts in &self.lines {
            let mut parts = parts.clone();
            if parts.is_empty() {
                writeln!(writer)?;
                continue;
            }
            if parts[0] == ".message" && parts.len() >= 5 {
                let m = match me {
                    Some(m) => m,
                    None => return Err(anyhow::anyhow!("Not enough messages to import.")),
                };
                if !parts[3].is_empty() {
                    let mut name = match &m.name {
                        Some(n) => n.clone(),
                        None => {
                            return Err(anyhow::anyhow!(
                                "Message name is missing for message: {}",
                                m.message
                            ));
                        }
                    };
                    if let Some(repl) = replacement {
                        for (k, v) in &repl.map {
                            name = name.replace(k, v);
                        }
                    }
                    parts[3] = name.replace(' ', "\u{3000}");
                }
                let mut text = m.message.clone();
                if let Some(repl) = replacement {
                    for (k, v) in &repl.map {
                        text = text.replace(k, v);
                    }
                }
                parts[4] = text.replace(' ', "\u{3000}");
                me = mes.next();
            }
            let line = parts.join(" ");
            let d = encode_string(encoding, &line, false)?;
            writer.write_all(&d)?;
            writeln!(writer)?;
        }
        if me.is_some() || mes.next().is_some() {
            return Err(anyhow::anyhow!("Too many messages to import."));
        }
        writer.flush()?;
        Ok(())
    }
}
