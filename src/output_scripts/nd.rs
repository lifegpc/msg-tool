//! Parser and dumper for the ND (Name/Dialogue) text exchange format.

use crate::types::{ExtendedMessage, Message};
use anyhow::{Result, anyhow};

#[derive(Debug)]
struct NdCell {
    name: Option<String>,
    source: String,
    translated: String,
}

/// A parser for ND files.
pub struct NdParser<'a> {
    text: &'a str,
}

impl<'a> NdParser<'a> {
    /// Creates a parser from decoded ND text.
    pub fn new(text: &'a str) -> Self {
        Self { text }
    }

    fn parse_cells(&self) -> Result<Vec<NdCell>> {
        let normalized = self.text.replace("\r\n", "\n").replace('\r', "\n");
        let lines = normalized.split('\n').collect::<Vec<_>>();
        let mut cells = Vec::new();
        let mut index = 0;

        while index < lines.len() {
            while lines
                .get(index)
                .is_some_and(|line| line.is_empty() || line.starts_with('#'))
            {
                index += 1;
            }
            if index == lines.len() {
                break;
            }

            let source_line = index + 1;
            let source_text = lines
                .get(index)
                .ok_or_else(|| anyhow!("Missing source line at line {source_line}"))?
                .strip_prefix("○ ")
                .ok_or_else(|| anyhow!("Expected '○ ' source line at line {source_line}"))?;
            index += 1;

            let translated_line = index + 1;
            let translated_text = lines
                .get(index)
                .ok_or_else(|| anyhow!("Missing '● ' translation line after line {source_line}"))?
                .strip_prefix("● ")
                .ok_or_else(|| {
                    anyhow!("Expected '● ' translation line at line {translated_line}")
                })?;
            index += 1;

            let (name, source) = parse_speaker(source_text, source_line)?;
            let (translated_name, translated) = parse_speaker(translated_text, translated_line)?;
            if let Some(translated_name) = translated_name {
                if name.as_deref() != Some(translated_name) {
                    return Err(anyhow!(
                        "Speaker marker does not match the source at line {translated_line}"
                    ));
                }
            }
            if translated
                .chars()
                .any(|ch| matches!(ch, '○' | '●' | '【' | '】'))
            {
                return Err(anyhow!(
                    "ND structure markers are not allowed in translated text at line {translated_line}"
                ));
            }

            cells.push(NdCell {
                name: name.map(str::to_owned),
                source: source.replace("\\n", "\n"),
                translated: translated.replace("\\n", "\n"),
            });

            let mut saw_blank = false;
            while let Some(line) = lines
                .get(index)
                .filter(|line| line.is_empty() || line.starts_with('#'))
            {
                saw_blank |= line.is_empty();
                index += 1;
            }
            if index < lines.len() && !saw_blank {
                return Err(anyhow!(
                    "Expected a blank line between ND units before line {}",
                    index + 1
                ));
            }
        }

        Ok(cells)
    }

    /// Parses ND units into messages, retaining source text for empty translations.
    pub fn parse(&mut self) -> Result<Vec<Message>> {
        Ok(self
            .parse_cells()?
            .into_iter()
            .map(|cell| {
                let message = if cell.translated.is_empty() {
                    cell.source
                } else {
                    cell.translated
                };
                Message::new(message, cell.name)
            })
            .collect())
    }

    /// Parses ND units into source/translation pairs.
    pub fn parse_as_vec(&mut self) -> Result<Vec<(String, String)>> {
        Ok(self
            .parse_cells()?
            .into_iter()
            .map(|cell| (cell.source, cell.translated))
            .collect())
    }

    /// Parses ND units into extended messages.
    pub fn parse_as_extend(&mut self) -> Result<Vec<ExtendedMessage>> {
        Ok(self
            .parse_cells()?
            .into_iter()
            .map(|cell| ExtendedMessage {
                name: cell.name,
                source: cell.source,
                translated: cell.translated,
                llm: None,
            })
            .collect())
    }
}

fn parse_speaker(text: &str, line: usize) -> Result<(Option<&str>, &str)> {
    if let Some(text) = text.strip_prefix('【') {
        let end = text
            .find('】')
            .ok_or_else(|| anyhow!("Unclosed speaker marker at line {line}"))?;
        let name = text
            .get(..end)
            .ok_or_else(|| anyhow!("Invalid speaker marker at line {line}"))?;
        if name.is_empty() {
            return Err(anyhow!("Empty speaker marker at line {line}"));
        }
        let body_start = end + '】'.len_utf8();
        let body = text
            .get(body_start..)
            .ok_or_else(|| anyhow!("Invalid speaker marker at line {line}"))?;
        Ok((Some(name), body))
    } else {
        Ok((None, text))
    }
}

/// A dumper for the ND format. Output always uses LF line endings.
pub struct NdDumper;

impl NdDumper {
    /// Dumps messages with empty translation placeholders.
    pub fn dump(messages: &[Message]) -> String {
        let mut result = String::new();
        for message in messages {
            let source = add_speaker(message.name.as_deref(), &message.message);
            let translation = add_speaker(message.name.as_deref(), "");
            result.push_str("○ ");
            result.push_str(&escape_newlines(&source));
            result.push_str("\n● ");
            result.push_str(&escape_newlines(&translation));
            result.push_str("\n\n");
        }
        result
    }

    /// Dumps extended messages with their source and translated text.
    pub fn dump_extended(messages: &[ExtendedMessage]) -> String {
        let mut result = String::new();
        for message in messages {
            let source = add_speaker(message.name.as_deref(), &message.source);
            let translated = add_speaker(message.name.as_deref(), &message.translated);
            result.push_str("○ ");
            result.push_str(&escape_newlines(&source));
            result.push_str("\n● ");
            result.push_str(&escape_newlines(&translated));
            result.push_str("\n\n");
        }
        result
    }
}

fn add_speaker(name: Option<&str>, text: &str) -> String {
    match name {
        Some(name) => format!("【{name}】{text}"),
        None => text.to_owned(),
    }
}

fn escape_newlines(text: &str) -> String {
    text.replace('\n', "\\n")
}
