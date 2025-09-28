use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use anyhow::{Result, anyhow};
use std::io::Write;

#[derive(Debug)]
/// Builder for general Artemis TXT scripts.
pub struct ArtemisTxtBuilder {}

impl ArtemisTxtBuilder {
    /// Creates a new builder instance.
    pub const fn new() -> Self {
        Self {}
    }
}

impl ScriptBuilder for ArtemisTxtBuilder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Cp932
    }

    fn build_script(
        &self,
        buf: Vec<u8>,
        _filename: &str,
        encoding: Encoding,
        _archive_encoding: Encoding,
        _config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(ArtemisTxtScript::new(buf, encoding)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["txt"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::ArtemisTxt
    }
}

#[derive(Debug, Clone)]
struct MessageRef {
    line_index: usize,
    speaker: Option<String>,
    speaker_line_index: Option<usize>,
}

#[derive(Debug)]
pub struct ArtemisTxtScript {
    lines: Vec<String>,
    message_map: Vec<MessageRef>,
    use_crlf: bool,
    trailing_newline: bool,
}

impl ArtemisTxtScript {
    fn new(buf: Vec<u8>, encoding: Encoding) -> Result<Self> {
        let script = decode_to_string(encoding, &buf, true)?;
        let use_crlf = script.contains("\r\n");
        let trailing_newline = script.ends_with('\n');
        let mut lines: Vec<String> = script
            .split('\n')
            .map(|line| {
                if use_crlf {
                    line.strip_suffix('\r').unwrap_or(line).to_string()
                } else {
                    line.to_string()
                }
            })
            .collect();
        if trailing_newline {
            // split('\n') keeps a trailing empty entry we do not want to lose
            if lines.last().map(|s| s.is_empty()).unwrap_or(false) {
                lines.pop();
            }
        }
        let message_map = Self::collect_messages(&lines);
        Ok(Self {
            lines,
            message_map,
            use_crlf,
            trailing_newline,
        })
    }

    fn collect_messages(lines: &[String]) -> Vec<MessageRef> {
        let mut refs = Vec::new();
        let mut current_speaker: Option<String> = None;
        let mut current_speaker_line: Option<usize> = None;
        for (idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if trimmed.starts_with("//") {
                continue;
            }
            if trimmed.starts_with('*') {
                continue;
            }
            if trimmed.starts_with('[') {
                continue;
            }
            if trimmed.starts_with('#') {
                match Self::parse_hash_speaker(trimmed) {
                    Some(name) => {
                        current_speaker = Some(name);
                        current_speaker_line = Some(idx);
                    }
                    None => {
                        current_speaker = None;
                        current_speaker_line = None;
                    }
                }
                continue;
            }

            let speaker = if Self::is_dialogue_line(trimmed) {
                current_speaker.clone()
            } else {
                None
            };
            let speaker_line_index = if speaker.is_some() {
                current_speaker_line
            } else {
                None
            };
            refs.push(MessageRef {
                line_index: idx,
                speaker,
                speaker_line_index,
            });
        }
        refs
    }

    fn parse_hash_speaker(line: &str) -> Option<String> {
        let content = line.trim_start_matches('#').trim();
        if content.is_empty() {
            return None;
        }
        let mut parts = content.split_whitespace();
        let token = parts.next()?;
        let upper = token.to_ascii_uppercase();
        if upper.starts_with("BGM")
            || upper.starts_with("SE")
            || upper.starts_with("FGA")
            || upper.starts_with("FG")
        {
            return None;
        }
        if token == "服装" {
            return None;
        }
        Some(token.to_string())
    }

    fn is_dialogue_line(line: &str) -> bool {
        match line.chars().next() {
            Some('"') | Some('“') | Some('〝') | Some('(') | Some('（') | Some('「')
            | Some('『') => true,
            _ => false,
        }
    }

    fn join_lines(&self, lines: &[String]) -> String {
        let newline = if self.use_crlf { "\r\n" } else { "\n" };
        let mut combined = lines.join(newline);
        if self.trailing_newline {
            combined.push_str(newline);
        }
        combined
    }

    fn set_speaker_line(line: &str, name: &str) -> String {
        if let Some(hash_pos) = line.find('#') {
            let after_hash = &line[hash_pos + 1..];
            let start_rel = after_hash
                .char_indices()
                .find(|(_, ch)| !ch.is_whitespace())
                .map(|(offset, _)| offset);
            let start_rel = match start_rel {
                Some(offset) => offset,
                None => {
                    let mut result = String::with_capacity(line.len() + name.len());
                    result.push_str(line);
                    result.push_str(name);
                    return result;
                }
            };
            let start = hash_pos + 1 + start_rel;
            let tail = &after_hash[start_rel..];
            let mut name_len = 0;
            let mut end_rel = tail.len();
            for (offset, ch) in tail.char_indices() {
                if ch.is_whitespace() {
                    end_rel = offset;
                    break;
                }
                name_len = offset + ch.len_utf8();
            }
            let end = if tail.is_empty() {
                start
            } else if end_rel == tail.len() {
                start + name_len
            } else {
                start + end_rel
            };
            let mut result = String::with_capacity(line.len() + name.len());
            result.push_str(&line[..start]);
            result.push_str(name);
            result.push_str(&line[end..]);
            return result;
        }
        format!("#{}", name)
    }
}

impl Script for ArtemisTxtScript {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Json
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn extract_messages(&self) -> Result<Vec<Message>> {
        let mut messages = Vec::with_capacity(self.message_map.len());
        for entry in &self.message_map {
            let text = self
                .lines
                .get(entry.line_index)
                .cloned()
                .unwrap_or_default();
            messages.push(Message {
                name: entry.speaker.clone(),
                message: text,
            });
        }
        Ok(messages)
    }

    fn import_messages<'a>(
        &'a self,
        messages: Vec<Message>,
        mut file: Box<dyn WriteSeek + 'a>,
        _filename: &str,
        encoding: Encoding,
        replacement: Option<&'a ReplacementTable>,
    ) -> Result<()> {
        if messages.len() != self.message_map.len() {
            return Err(anyhow!(
                "Message count mismatch: expected {}, got {}",
                self.message_map.len(),
                messages.len()
            ));
        }
        let mut output_lines = self.lines.clone();
        for (entry, message) in self.message_map.iter().zip(messages.iter()) {
            let mut text = message.message.clone();
            if let Some(repl) = replacement {
                for (from, to) in &repl.map {
                    text = text.replace(from, to);
                }
            }
            if let Some(line) = output_lines.get_mut(entry.line_index) {
                *line = text;
            }
            if let (Some(speaker_line_index), Some(name)) =
                (entry.speaker_line_index, message.name.as_ref())
            {
                let mut patched_name = name.clone();
                if let Some(repl) = replacement {
                    for (from, to) in &repl.map {
                        patched_name = patched_name.replace(from, to);
                    }
                }
                if let Some(line) = output_lines.get_mut(speaker_line_index) {
                    *line = Self::set_speaker_line(line, &patched_name);
                } else {
                    return Err(anyhow!(
                        "Speaker line index out of bounds: {}",
                        speaker_line_index
                    ));
                }
            }
        }
        let combined = self.join_lines(&output_lines);
        let encoded = encode_string(encoding, &combined, true)?;
        file.write_all(&encoded)?;
        Ok(())
    }
}
