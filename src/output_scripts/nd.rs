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

            let mut source = None;
            let mut translated = None;
            while let Some(line) = lines.get(index) {
                if line.is_empty() {
                    index += 1;
                    break;
                }
                let line_number = index + 1;
                if line.starts_with('#') {
                    index += 1;
                    continue;
                }
                if let Some(text) = line.strip_prefix("○ ") {
                    if source.is_none() {
                        source = Some((text, line_number));
                    }
                } else if let Some(text) = line.strip_prefix("● ") {
                    if translated.is_some() {
                        return Err(anyhow!(
                            "Multiple '● ' translation lines in ND unit at line {line_number}"
                        ));
                    }
                    translated = Some((text, line_number));
                } else {
                    return Err(anyhow!("Invalid ND line at line {line_number}: {line}"));
                }
                index += 1;
            }

            let (source_text, source_line) = source.ok_or_else(|| {
                anyhow!(
                    "Missing '○ ' source line in ND unit before line {}",
                    index + 1
                )
            })?;
            let (translated_text, translated_line) = translated.ok_or_else(|| {
                anyhow!("Missing '● ' translation line for source at line {source_line}")
            })?;

            let (name, source) = parse_speaker(source_text);
            let translated = if translated_text.is_empty() {
                translated_text
            } else if let Some(name) = name {
                let (translated_name, translated) = parse_speaker(translated_text);
                match translated_name {
                    Some(translated_name) if translated_name == name => translated,
                    Some(_) => {
                        return Err(anyhow!(
                            "Speaker marker does not match the source at line {translated_line}"
                        ));
                    }
                    None => {
                        return Err(anyhow!(
                            "Missing speaker marker for source at line {translated_line}"
                        ));
                    }
                }
            } else {
                translated_text
            };

            cells.push(NdCell {
                name: name.map(str::to_owned),
                source: source.replace("\\n", "\n"),
                translated: translated.replace("\\n", "\n"),
            });
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

fn parse_speaker(text: &str) -> (Option<&str>, &str) {
    let Some(after_open) = text.strip_prefix('【') else {
        return (None, text);
    };
    let Some(end) = after_open.find('】') else {
        return (None, text);
    };
    if end == 0 {
        return (None, text);
    }
    let Some(name) = after_open.get(..end) else {
        return (None, text);
    };
    let body_start = end.saturating_add('】'.len_utf8());
    let Some(body) = after_open.get(body_start..) else {
        return (None, text);
    };
    (Some(name), body)
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
