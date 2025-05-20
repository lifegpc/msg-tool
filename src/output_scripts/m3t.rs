use crate::types::Message;
use anyhow::Result;

pub struct M3tParser<'a> {
    str: &'a str,
    line: usize,
}

impl<'a> M3tParser<'a> {
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

    pub fn parse(&mut self) -> Result<Vec<Message>> {
        let mut messages = Vec::new();
        let mut name = None;
        while let Some(line) = self.next_line() {
            if line.is_empty() {
                continue;
            }
            if line.starts_with("○") {
                let line = line[3..].trim();
                if line.starts_with("NAME:") {
                    name = Some(line[5..].trim().to_string());
                }
            } else if line.starts_with("●") {
                let message = line[3..].trim();
                messages.push(Message::new(message.replace("\\n", "\n"), name.take()));
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

pub struct M3tDumper {}

impl M3tDumper {
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
