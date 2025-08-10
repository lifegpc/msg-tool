//! A simple text format that supports both original/llm/translated messages.
//!
//! A simple m3t file example:
//! ```text
//! ○ NAME: Example
//!
//! ○ Original message
//! △ LLM message
//! ● Translated message
//! ```
use crate::types::Message;
use anyhow::Result;

/// A parser for the M3T format.
pub struct M3tParser<'a> {
    str: &'a str,
    line: usize,
}

impl<'a> M3tParser<'a> {
    /// Creates a new M3tParser with the given string.
    pub fn new(str: &'a str) -> Self {
        M3tParser { str, line: 1 }
    }

    fn next_line(&mut self) -> Option<&'a str> {
        match self.str.find('\n') {
            Some(pos) => {
                let line = &self.str[..pos];
                self.str = &self.str[pos + 1..];
                self.line += 1;
                Some(line.trim())
            }
            None => {
                if !self.str.is_empty() {
                    let line = self.str;
                    self.str = "";
                    Some(line)
                } else {
                    None
                }
            }
        }
    }

    /// Parses the M3T format and returns a vector of messages.
    pub fn parse(&mut self) -> Result<Vec<Message>> {
        let mut messages = Vec::new();
        let mut name = None;
        let mut llm = None;
        while let Some(line) = self.next_line() {
            if line.is_empty() {
                continue;
            }
            if line.starts_with("○") {
                let line = line[3..].trim();
                if line.starts_with("NAME:") {
                    name = Some(line[5..].trim().to_string());
                }
            } else if line.starts_with("△") {
                let line = line[3..].trim();
                llm = Some(line);
            } else if line.starts_with("●") {
                let message = line[3..].trim();
                let message = if message
                    .trim_start_matches("「")
                    .trim_end_matches("」")
                    .is_empty()
                {
                    llm.take()
                        .unwrap_or_else(|| {
                            if message.starts_with("「") {
                                "「」"
                            } else {
                                ""
                            }
                        })
                        .replace("\\n", "\n")
                } else {
                    message.replace("\\n", "\n")
                };
                messages.push(Message::new(message, name.take()));
            } else {
                return Err(anyhow::anyhow!(
                    "Invalid line format at line {}: {}",
                    self.line,
                    line
                ));
            }
        }
        Ok(messages)
    }
}

/// A dumper for the M3T format.
pub struct M3tDumper {}

impl M3tDumper {
    /// Dumps the messages in M3T format.
    pub fn dump(messages: &[Message]) -> String {
        let mut result = String::new();
        for message in messages {
            if let Some(name) = &message.name {
                result.push_str(&format!("○ NAME: {}\n\n", name));
            }
            result.push_str(&format!("○ {}\n", message.message.replace("\n", "\\n")));
            if message.message.starts_with("「") {
                result.push_str("● 「」\n\n");
            } else {
                result.push_str("●\n\n");
            }
        }
        result
    }
}
