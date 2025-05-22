use crate::types::*;
use unicode_segmentation::UnicodeSegmentation;

const SPACE_STR_LIST: [&str; 2] = [" ", "　"];

pub struct FixedFormatter {
    length: usize,
    keep_original: bool,
    typ: Option<ScriptType>,
}

impl FixedFormatter {
    pub fn new(length: usize, keep_original: bool, typ: Option<ScriptType>) -> Self {
        FixedFormatter {
            length,
            keep_original,
            typ,
        }
    }

    fn is_circus(&self) -> bool {
        matches!(self.typ, Some(ScriptType::Circus))
    }

    pub fn format(&self, message: &str) -> String {
        let mut result = String::new();
        let vec: Vec<_> = UnicodeSegmentation::graphemes(message, true).collect();
        let mut current_length = 0;
        let mut is_command = false;
        let mut pre_is_lf = false;
        let mut is_ruby = false;
        let mut is_ruby_rt = false;
        for grapheme in vec {
            if grapheme == "\n" {
                if self.keep_original {
                    result.push('\n');
                    current_length = 0;
                }
                pre_is_lf = true;
                continue;
            }
            if current_length >= self.length {
                result.push('\n');
                current_length = 0;
            }
            if (current_length == 0 || pre_is_lf) && SPACE_STR_LIST.contains(&grapheme) {
                continue;
            }
            result.push_str(grapheme);
            if self.is_circus() {
                if grapheme == "@" {
                    is_command = true;
                } else if is_command && grapheme.len() != 1
                    || !grapheme
                        .chars()
                        .next()
                        .unwrap_or(' ')
                        .is_ascii_alphanumeric()
                {
                    is_command = false;
                }
                if grapheme == "｛" {
                    is_ruby = true;
                    is_ruby_rt = true;
                } else if is_ruby && grapheme == "／" {
                    is_ruby_rt = false;
                    continue;
                } else if is_ruby && grapheme == "｝" {
                    is_ruby = false;
                    continue;
                }
            }
            if !is_command && !is_ruby_rt {
                current_length += 1;
            }
            pre_is_lf = false;
        }
        return result;
    }
}

#[test]
fn test_format() {
    let formatter = FixedFormatter::new(10, false, None);
    let message = "This is a test message.\nThis is another line.";
    let formatted_message = formatter.format(message);
    assert_eq!(
        formatted_message,
        "This is a \ntest messa\nge.This is\nanother li\nne."
    );
    assert_eq!(formatter.format("● This is a test."), "● This is \na test.");
    assert_eq!(
        formatter.format("● This is 　a test."),
        "● This is \na test."
    );
    let fommater2 = FixedFormatter::new(10, true, None);
    assert_eq!(
        fommater2.format("● Th\n is is a te st."),
        "● Th\nis is a te\nst."
    );
    let circus_formatter = FixedFormatter::new(10, false, Some(ScriptType::Circus));
    assert_eq!(
        circus_formatter.format("● @cmd1@cmd2@cmd3中文字数是一\n　二三　四五六七八九十"),
        "● @cmd1@cmd2@cmd3中文字数是一二三\n四五六七八九十"
    );
    assert_eq!(
        circus_formatter
            .format("● @cmd1@cmd2@cmd3｛rubyText／中文｝字数是一\n　二三　四五六七八九十"),
        "● @cmd1@cmd2@cmd3｛rubyText／中文｝字数是一二三\n四五六七八九十"
    );
}
