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
    llm_mark: Option<&'a str>,
}

impl<'a> M3tParser<'a> {
    /// Creates a new M3tParser with the given string.
    pub fn new(str: &'a str, llm_mark: Option<&'a str>) -> Self {
        M3tParser {
            str,
            line: 1,
            llm_mark,
        }
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

    pub fn parse_as_vec(&mut self) -> Result<Vec<(String, String)>> {
        let mut map = Vec::new();
        let mut ori = None;
        let mut llm = None;
        while let Some(line) = self.next_line() {
            if line.is_empty() {
                continue;
            }
            // Remove zero-width space characters
            let line = line.trim().trim_matches('\u{200b}');
            if line.starts_with("○") {
                let line = line[3..].trim();
                if !line.starts_with("NAME:") {
                    ori = Some(line.to_string());
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
                        .map(|s| {
                            let mut s = s.to_string();
                            if let Some(mark) = self.llm_mark {
                                s.push_str(mark);
                            }
                            s
                        })
                        .unwrap_or_else(|| {
                            String::from(if message.starts_with("「") {
                                "「」"
                            } else {
                                ""
                            })
                        })
                        .replace("\\n", "\n")
                } else {
                    let mut tmp = message.to_owned();
                    if let Some(llm) = llm.take() {
                        if tmp == llm {
                            if let Some(mark) = self.llm_mark {
                                tmp.push_str(mark);
                            }
                        }
                    }
                    tmp.replace("\\n", "\n")
                };
                if let Some(ori) = ori.take() {
                    map.push((ori, message));
                } else {
                    return Err(anyhow::anyhow!(
                        "Missing original message before translated message at line {}",
                        self.line
                    ));
                }
            } else {
                return Err(anyhow::anyhow!(
                    "Invalid line format at line {}: {}",
                    self.line,
                    line
                ));
            }
        }
        Ok(map)
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
            // Remove zero-width space characters
            let line = line.trim().trim_matches('\u{200b}');
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
                        .map(|s| {
                            let mut s = s.to_string();
                            if let Some(mark) = self.llm_mark {
                                s.push_str(mark);
                            }
                            s
                        })
                        .unwrap_or_else(|| {
                            String::from(if message.starts_with("「") {
                                "「」"
                            } else {
                                ""
                            })
                        })
                        .replace("\\n", "\n")
                } else {
                    let mut tmp = message.to_owned();
                    if let Some(llm) = llm.take() {
                        if tmp == llm {
                            if let Some(mark) = self.llm_mark {
                                tmp.push_str(mark);
                            }
                        }
                    }
                    tmp.replace("\\n", "\n")
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
    pub fn dump(messages: &[Message], no_quote: bool) -> String {
        let mut result = String::new();
        for message in messages {
            if let Some(name) = &message.name {
                result.push_str(&format!("○ NAME: {}\n\n", name));
            }
            result.push_str(&format!("○ {}\n", message.message.replace("\n", "\\n")));
            if !no_quote && message.message.starts_with("「") {
                result.push_str("● 「」\n\n");
            } else {
                result.push_str("●\n\n");
            }
        }
        result
    }
}

#[test]
fn test_zero_width_space() {
    let input = "○ NAME: Example\n\n○ Original message\n\u{200b}● 「」\n\n";
    let mut parser = M3tParser::new(input, None);
    let messages = parser.parse().unwrap();
    assert_eq!(messages.len(), 1);
    let map = M3tParser::new(input, None).parse_as_vec().unwrap();
    assert_eq!(map.len(), 1);
}
