use crate::types::*;
use unicode_segmentation::UnicodeSegmentation;

const SPACE_STR_LIST: [&str; 2] = [" ", "　"];
const QUOTE_LIST: [(&str, &str); 4] = [("「", "」"), ("『", "』"), ("（", "）"), ("【", "】")];

fn check_is_ascii_alphanumeric(s: &str) -> bool {
    for c in s.chars() {
        if !c.is_ascii_alphanumeric() {
            return false;
        }
    }
    true
}

fn check_need_fullwidth_space(s: &str) -> bool {
    let has_start_quote = QUOTE_LIST.iter().any(|(open, _)| s.starts_with(open));
    if !has_start_quote {
        return false;
    }
    for (open, close) in QUOTE_LIST.iter() {
        let open_index = s.rfind(open);
        if let Some(open_index) = open_index {
            let index = s.rfind(close);
            match index {
                Some(idx) => {
                    return idx < open_index;
                }
                None => return true,
            }
        }
    }
    false
}

pub struct FixedFormatter {
    length: usize,
    keep_original: bool,
    /// Whether to break words (ASCII only) at the end of the line.
    break_words: bool,
    /// Whether to insert a full-width space after a line break when a sentence starts with a full-width quotation mark.
    insert_fullwidth_space_at_line_start: bool,
    #[allow(unused)]
    typ: Option<ScriptType>,
}

impl FixedFormatter {
    pub fn new(
        length: usize,
        keep_original: bool,
        break_words: bool,
        insert_fullwidth_space_at_line_start: bool,
        typ: Option<ScriptType>,
    ) -> Self {
        FixedFormatter {
            length,
            keep_original,
            break_words,
            insert_fullwidth_space_at_line_start,
            typ,
        }
    }

    #[cfg(feature = "circus")]
    fn is_circus(&self) -> bool {
        matches!(self.typ, Some(ScriptType::Circus))
    }

    #[cfg(not(feature = "circus"))]
    fn is_circus(&self) -> bool {
        false
    }

    #[cfg(feature = "kirikiri")]
    fn is_scn(&self) -> bool {
        matches!(self.typ, Some(ScriptType::KirikiriScn))
    }

    #[cfg(not(feature = "kirikiri"))]
    fn is_scn(&self) -> bool {
        false
    }

    pub fn format(&self, message: &str) -> String {
        let mut result = String::new();
        let vec: Vec<_> = UnicodeSegmentation::graphemes(message, true).collect();
        let mut current_length = 0;
        let mut is_command = false;
        let mut pre_is_lf = false;
        let mut is_ruby = false;
        let mut is_ruby_rt = false;
        let mut last_command = None;
        let mut i = 0;
        // Store main content of the line (excluding commands and ruby)
        let mut main_content = String::new();
        let mut first_line = true;
        let mut need_insert_fullwidth_space = false;

        while i < vec.len() {
            let grapheme = vec[i];

            if grapheme == "\n" {
                if self.keep_original
                    || (self.is_circus() && last_command.as_ref().is_some_and(|cmd| cmd == "@n"))
                {
                    result.push('\n');
                    current_length = 0;
                    if first_line {
                        if self.insert_fullwidth_space_at_line_start {
                            if check_need_fullwidth_space(&main_content) {
                                need_insert_fullwidth_space = true;
                            }
                        }
                    }
                    if need_insert_fullwidth_space {
                        result.push('　');
                        current_length += 1;
                    }
                    main_content.clear();
                    first_line = false;
                }
                pre_is_lf = true;
                i += 1;
                continue;
            }

            // Check if we need to break and handle word breaking
            if current_length >= self.length {
                if !self.break_words
                    && !is_command
                    && !is_ruby_rt
                    && check_is_ascii_alphanumeric(grapheme)
                {
                    // Look back to find a good break point (space or non-ASCII)
                    let mut break_pos = None;
                    let mut temp_length = current_length;
                    let mut j = result.len();

                    // Find the last space or non-ASCII character position
                    for ch in result.chars().rev() {
                        if ch == ' ' || ch == '　' || !ch.is_ascii() {
                            break_pos = Some(j);
                            break;
                        }
                        if ch.is_ascii_alphabetic() {
                            temp_length -= 1;
                            if temp_length == 0 {
                                break;
                            }
                        }
                        j -= ch.len_utf8();
                    }

                    // If we found a good break point, move content after it to next line
                    if let Some(pos) = break_pos {
                        let remaining = result[pos..].trim_start().to_string();
                        result.truncate(pos);
                        result.push('\n');
                        current_length = 0;
                        if first_line {
                            if self.insert_fullwidth_space_at_line_start {
                                if check_need_fullwidth_space(&main_content) {
                                    need_insert_fullwidth_space = true;
                                }
                            }
                            first_line = false;
                        }
                        if need_insert_fullwidth_space {
                            result.push('　');
                            current_length += 1;
                        }
                        result.push_str(&remaining);
                        current_length += remaining.chars().count();
                        main_content.clear();
                        pre_is_lf = true;
                    } else {
                        result.push('\n');
                        current_length = 0;
                        if first_line {
                            if self.insert_fullwidth_space_at_line_start {
                                if check_need_fullwidth_space(&main_content) {
                                    need_insert_fullwidth_space = true;
                                }
                            }
                            first_line = false;
                        }
                        if need_insert_fullwidth_space {
                            result.push('　');
                            current_length += 1;
                        }
                        main_content.clear();
                        pre_is_lf = true;
                    }
                } else {
                    result.push('\n');
                    current_length = 0;
                    if first_line {
                        if self.insert_fullwidth_space_at_line_start {
                            if check_need_fullwidth_space(&main_content) {
                                need_insert_fullwidth_space = true;
                            }
                        }
                        first_line = false;
                    }
                    if need_insert_fullwidth_space {
                        result.push('　');
                        current_length += 1;
                    }
                    main_content.clear();
                    pre_is_lf = true;
                }
            }

            if (current_length == 0 || pre_is_lf) && SPACE_STR_LIST.contains(&grapheme) {
                i += 1;
                continue;
            }

            result.push_str(grapheme);

            if self.is_circus() {
                if grapheme == "@" {
                    is_command = true;
                    last_command = Some(String::new());
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
                    i += 1;
                    continue;
                } else if is_ruby && grapheme == "｝" {
                    is_ruby = false;
                    i += 1;
                    continue;
                }
            }

            if self.is_scn() {
                if grapheme == "%" {
                    is_command = true;
                } else if is_command && grapheme == ";" {
                    is_command = false;
                    i += 1;
                    continue;
                }
                if grapheme == "[" {
                    is_ruby = true;
                    is_ruby_rt = true;
                    i += 1;
                    continue;
                } else if is_ruby && grapheme == "]" {
                    is_ruby = false;
                    is_ruby_rt = false;
                    i += 1;
                    continue;
                }
            }

            if is_command {
                if let Some(ref mut cmd) = last_command {
                    cmd.push_str(grapheme);
                }
            }

            if !is_command && !is_ruby_rt {
                current_length += 1;
                main_content.push_str(grapheme);
            }

            pre_is_lf = false;
            i += 1;
        }

        return result;
    }
}

#[test]
fn test_format() {
    let formatter = FixedFormatter::new(10, false, true, false, None);
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
    let fommater2 = FixedFormatter::new(10, true, true, false, None);
    assert_eq!(
        fommater2.format("● Th\n is is a te st."),
        "● Th\nis is a te\nst."
    );

    // Test break_words = false
    let no_break_formatter = FixedFormatter::new(10, false, false, false, None);
    assert_eq!(
        no_break_formatter.format("Example text."),
        "Example \ntext."
    );

    let no_break_formatter2 = FixedFormatter::new(6, false, false, false, None);
    assert_eq!(
        no_break_formatter2.format("Example text."),
        "Exampl\ne text\n."
    );

    let no_break_formatter3 = FixedFormatter::new(7, false, false, false, None);
    assert_eq!(
        no_break_formatter3.format("Example text."),
        "Example\ntext."
    );

    let real_world_no_break_formatter = FixedFormatter::new(32, false, false, false, None);
    assert_eq!(
        real_world_no_break_formatter.format("○咕噜咕噜（Temporary Magnetic Pattern Linkage）"),
        "○咕噜咕噜（Temporary Magnetic Pattern\nLinkage）"
    );

    let formatter3 = FixedFormatter::new(10, false, false, true, None);
    assert_eq!(
        formatter3.format("「This is a test."),
        "「This is a\n\u{3000}test."
    );

    assert_eq!(
        formatter3.format("（This） is a test."),
        "（This） is \na test."
    );

    assert_eq!(
        formatter3.format("（long text test here, test 1234"),
        "（long text\n\u{3000}test here\n\u{3000}, test \n\u{3000}1234"
    );

    assert_eq!(
        formatter3.format("（This） 「is a test."),
        "（This） 「is\n\u{3000}a test."
    );

    #[cfg(feature = "circus")]
    {
        let circus_formatter =
            FixedFormatter::new(10, false, true, false, Some(ScriptType::Circus));
        assert_eq!(
            circus_formatter.format("● @cmd1@cmd2@cmd3中文字数是一\n　二三　四五六七八九十"),
            "● @cmd1@cmd2@cmd3中文字数是一二三\n四五六七八九十"
        );
        assert_eq!(
            circus_formatter
                .format("● @cmd1@cmd2@cmd3｛rubyText／中文｝字数是一\n　二三　四五六七八九十"),
            "● @cmd1@cmd2@cmd3｛rubyText／中文｝字数是一二三\n四五六七八九十"
        );
        let circus_formatter2 =
            FixedFormatter::new(32, false, true, false, Some(ScriptType::Circus));
        assert_eq!(
            circus_formatter2.format("@re1@re2@b1@t30@w1「当然现在我很幸福哦？\n　因为有你在身边」@n\n「@b1@t38@w1当然现在我很幸福哦？\n　因为有敦也君在身边」"),
            "@re1@re2@b1@t30@w1「当然现在我很幸福哦？因为有你在身边」@n\n「@b1@t38@w1当然现在我很幸福哦？因为有敦也君在身边」"
        );
    }

    #[cfg(feature = "kirikiri")]
    {
        let scn_formatter =
            FixedFormatter::new(3, false, false, false, Some(ScriptType::KirikiriScn));
        assert_eq!(
            scn_formatter.format("%test;[ruby]测[test]试打断。"),
            "%test;[ruby]测[test]试打\n断。"
        );
    }
}
