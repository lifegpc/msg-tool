use crate::types::*;
use anyhow::Result;
#[cfg(feature = "jieba")]
use jieba_rs::Jieba;
use unicode_segmentation::UnicodeSegmentation;

const SPACE_STR_LIST: [&str; 2] = [" ", "　"];
const QUOTE_LIST: [(&str, &str); 4] = [("「", "」"), ("『", "』"), ("（", "）"), ("【", "】")];
const BREAK_SENTENCE_SYMBOLS: [&str; 6] = ["…", "，", "。", "？", "！", "—"];

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

fn check_is_end_quote(segs: &[&str], pos: usize) -> bool {
    let d = segs[pos];
    QUOTE_LIST.iter().any(|(_, close)| d == *close)
}

fn check_is_end_quote_or_symbol(segs: &[&str], pos: usize) -> bool {
    let d = segs[pos];
    QUOTE_LIST.iter().any(|(_, close)| d == *close) || BREAK_SENTENCE_SYMBOLS.contains(&d)
}

fn check_is_start_quote(s: &str) -> bool {
    QUOTE_LIST.iter().any(|(open, _)| s == *open)
}

fn take_trailing_start_quotes(buffer: &mut String) -> String {
    let (collected, trailing) = {
        let mut collected = buffer.graphemes(true).collect::<Vec<_>>();
        let mut trailing = Vec::new();
        while let Some(&last) = collected.last() {
            if check_is_start_quote(last) {
                collected.pop();
                trailing.push(last);
            } else {
                break;
            }
        }
        trailing.reverse();
        (collected.concat(), trailing.concat())
    };
    *buffer = collected;
    trailing
}

#[cfg(feature = "jieba")]
fn check_chinese_word_is_break(segs: &[&str], pos: usize, jieba: &Jieba) -> bool {
    let s = segs.join("");
    let mut breaked = jieba
        .cut(&s, false)
        .iter()
        .map(|s| s.graphemes(true).count())
        .collect::<Vec<_>>();
    let mut sum = 0;
    for i in breaked.iter_mut() {
        sum += *i;
        *i = sum;
    }
    breaked.binary_search(&pos).is_err()
}

#[cfg(not(feature = "jieba"))]
fn check_chinese_word_is_break(_segs: &[&str], _pos: usize, _jieba: &()) -> bool {
    false
}

pub struct FixedFormatter {
    length: usize,
    keep_original: bool,
    /// Whether to break words (ASCII only) at the end of the line.
    break_words: bool,
    /// Whether to insert a full-width space after a line break when a sentence starts with a full-width quotation mark.
    insert_fullwidth_space_at_line_start: bool,
    /// If a line break occurs in the middle of some symbols, bring the sentence to next line
    break_with_sentence: bool,
    #[cfg(feature = "jieba")]
    /// Jieba instance for Chinese word segmentation.
    jieba: Option<Jieba>,
    #[cfg(not(feature = "jieba"))]
    jieba: Option<()>,
    /// Do not remove space at the start of the line
    no_remove_space_at_line_start: bool,
    #[allow(unused)]
    typ: Option<ScriptType>,
}

impl FixedFormatter {
    pub fn new(
        length: usize,
        keep_original: bool,
        break_words: bool,
        insert_fullwidth_space_at_line_start: bool,
        break_with_sentence: bool,
        #[cfg(feature = "jieba")] break_chinese_words: bool,
        #[cfg(feature = "jieba")] jieba_dict: Option<String>,
        no_remove_space_at_line_start: bool,
        typ: Option<ScriptType>,
    ) -> Result<Self> {
        #[cfg(feature = "jieba")]
        let jieba = if !break_chinese_words {
            let mut jieba = Jieba::new();
            if let Some(dict) = jieba_dict {
                let file = std::fs::File::open(dict)?;
                let mut reader = std::io::BufReader::new(file);
                jieba.load_dict(&mut reader)?;
            }
            Some(jieba)
        } else {
            None
        };
        Ok(FixedFormatter {
            length,
            keep_original,
            break_words,
            insert_fullwidth_space_at_line_start,
            break_with_sentence,
            #[cfg(feature = "jieba")]
            jieba,
            #[cfg(not(feature = "jieba"))]
            jieba: None,
            no_remove_space_at_line_start,
            typ,
        })
    }

    #[cfg(test)]
    fn builder(length: usize) -> Self {
        FixedFormatter {
            length,
            keep_original: false,
            break_words: true,
            insert_fullwidth_space_at_line_start: false,
            break_with_sentence: false,
            jieba: None,
            typ: None,
            no_remove_space_at_line_start: false,
        }
    }

    #[cfg(test)]
    fn keep_original(mut self, keep: bool) -> Self {
        self.keep_original = keep;
        self
    }

    #[cfg(test)]
    fn break_words(mut self, break_words: bool) -> Self {
        self.break_words = break_words;
        self
    }

    #[cfg(test)]
    fn insert_fullwidth_space_at_line_start(mut self, insert: bool) -> Self {
        self.insert_fullwidth_space_at_line_start = insert;
        self
    }

    #[cfg(test)]
    fn break_with_sentence(mut self, break_with_sentence: bool) -> Self {
        self.break_with_sentence = break_with_sentence;
        self
    }

    #[cfg(all(feature = "jieba", test))]
    fn break_chinese_words(mut self, break_chinese_words: bool) -> Result<Self> {
        if !break_chinese_words {
            let jieba = Jieba::new();
            self.jieba = Some(jieba);
        } else {
            self.jieba = None;
        }
        Ok(self)
    }

    #[cfg(all(feature = "jieba", test))]
    fn add_dict(mut self, dict: &str, freq: Option<usize>, tag: Option<&str>) -> Self {
        if let Some(ref mut jieba) = self.jieba {
            jieba.add_word(&dict, freq, tag);
        }
        self
    }

    #[cfg(test)]
    fn no_remove_space_at_line_start(mut self, no_remove: bool) -> Self {
        self.no_remove_space_at_line_start = no_remove;
        self
    }

    #[cfg(test)]
    #[allow(dead_code)]
    fn typ(mut self, typ: Option<ScriptType>) -> Self {
        self.typ = typ;
        self
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
                if self.break_with_sentence
                    && !is_command
                    && !is_ruby_rt
                    && ((BREAK_SENTENCE_SYMBOLS.contains(&grapheme)
                        && i > 1
                        && BREAK_SENTENCE_SYMBOLS.contains(&vec[i - 1]))
                        || check_is_end_quote_or_symbol(&vec, i))
                {
                    let mut break_pos = None;
                    let segs = result.graphemes(true).collect::<Vec<_>>();
                    let is_end_quote = check_is_end_quote(&vec, i);
                    let mut end = segs.len();
                    for (j, ch) in segs.iter().enumerate().rev() {
                        if BREAK_SENTENCE_SYMBOLS.contains(ch) {
                            end = j;
                            if !is_end_quote {
                                break_pos = Some(j);
                            }
                        }
                        break;
                    }
                    for (j, ch) in segs[..end].iter().enumerate().rev() {
                        if j >= end {
                            continue;
                        }
                        if BREAK_SENTENCE_SYMBOLS.contains(ch) {
                            break_pos = Some(j + 1);
                            break;
                        }
                    }
                    if let Some(pos) = break_pos {
                        let mut head = segs[..pos].concat();
                        let mut remaining = segs[pos..].concat();
                        if self.break_with_sentence {
                            let trailing = take_trailing_start_quotes(&mut head);
                            if !trailing.is_empty() {
                                remaining.insert_str(0, &trailing);
                            }
                        }
                        let remaining = remaining.trim_start().to_string();
                        result = head;
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
                        current_length += remaining.graphemes(true).count();
                        main_content.clear();
                        pre_is_lf = true;
                    } else {
                        let trailing = if self.break_with_sentence {
                            take_trailing_start_quotes(&mut result)
                        } else {
                            String::new()
                        };
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
                        if !trailing.is_empty() {
                            result.push_str(&trailing);
                            current_length += trailing.graphemes(true).count();
                            main_content.push_str(&trailing);
                        }
                        pre_is_lf = true;
                    }
                } else if !self.break_words
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
                        let mut remaining = result[pos..].to_string();
                        result.truncate(pos);
                        if self.break_with_sentence {
                            let trailing = take_trailing_start_quotes(&mut result);
                            if !trailing.is_empty() {
                                remaining.insert_str(0, &trailing);
                            }
                        }
                        let remaining = remaining.trim_start().to_string();
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
                        let trailing = if self.break_with_sentence {
                            take_trailing_start_quotes(&mut result)
                        } else {
                            String::new()
                        };
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
                        if !trailing.is_empty() {
                            result.push_str(&trailing);
                            current_length += trailing.graphemes(true).count();
                            main_content.push_str(&trailing);
                        }
                        pre_is_lf = true;
                    }
                } else if self
                    .jieba
                    .as_ref()
                    .is_some_and(|s| check_chinese_word_is_break(&vec, i, s))
                    && !is_command
                    && !is_ruby_rt
                {
                    #[cfg(feature = "jieba")]
                    {
                        let jieba = self.jieba.as_ref().unwrap();
                        let s = vec.join("");
                        let mut breaked = jieba
                            .cut(&s, false)
                            .iter()
                            .map(|s| s.graphemes(true).count())
                            .collect::<Vec<_>>();
                        let mut sum = 0;
                        for i in breaked.iter_mut() {
                            sum += *i;
                            *i = sum;
                        }
                        let break_pos = match breaked.binary_search(&i) {
                            Ok(pos) => Some(pos),
                            Err(pos) => {
                                if pos == 0 {
                                    None
                                } else {
                                    Some(pos - 1)
                                }
                            }
                        };
                        if let Some(break_pos) = break_pos {
                            let pos = breaked[break_pos];
                            let segs = result.graphemes(true).collect::<Vec<_>>();
                            let remain_count = i - pos;
                            let pos = segs.len() - remain_count;
                            let mut head = segs[..pos].concat();
                            let mut remaining = segs[pos..].concat();
                            if self.break_with_sentence {
                                let trailing = take_trailing_start_quotes(&mut head);
                                if !trailing.is_empty() {
                                    remaining.insert_str(0, &trailing);
                                }
                            }
                            let remaining = remaining.trim_start().to_string();
                            result = head;
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
                            current_length += remaining.graphemes(true).count();
                            main_content.clear();
                            pre_is_lf = true;
                        } else {
                            let trailing = if self.break_with_sentence {
                                take_trailing_start_quotes(&mut result)
                            } else {
                                String::new()
                            };
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
                            if !trailing.is_empty() {
                                result.push_str(&trailing);
                                current_length += trailing.graphemes(true).count();
                                main_content.push_str(&trailing);
                            }
                            pre_is_lf = true;
                        }
                    }
                } else {
                    let trailing = if self.break_with_sentence {
                        take_trailing_start_quotes(&mut result)
                    } else {
                        String::new()
                    };
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
                    if !trailing.is_empty() {
                        result.push_str(&trailing);
                        current_length += trailing.graphemes(true).count();
                        main_content.push_str(&trailing);
                    }
                    pre_is_lf = true;
                }
            }

            if !self.no_remove_space_at_line_start
                && (current_length == 0 || pre_is_lf)
                && SPACE_STR_LIST.contains(&grapheme)
            {
                i += 1;
                continue;
            }

            result.push_str(grapheme);

            #[cfg(feature = "kirikiri")]
            if self.is_scn() {
                if grapheme == "#" {
                    i += 1;
                    while i < vec.len() && vec[i] != ";" {
                        result.push_str(vec[i]);
                        i += 1;
                    }
                    if i < vec.len() {
                        result.push_str(vec[i]);
                        i += 1;
                    }
                    continue;
                }
                if grapheme == "%" && i + 1 < vec.len() && vec[i + 1] == "r" {
                    result.push('r');
                    i += 2;
                    continue;
                }
            }

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

        result
    }
}

#[test]
fn test_format() {
    let formatter = FixedFormatter::builder(10);
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
    let fommater2 = FixedFormatter::builder(10).keep_original(true);
    assert_eq!(
        fommater2.format("● Th\n is is a te st."),
        "● Th\nis is a te\nst."
    );

    // Test break_words = false
    let no_break_formatter = FixedFormatter::builder(10).break_words(false);
    assert_eq!(
        no_break_formatter.format("Example text."),
        "Example \ntext."
    );

    let no_break_formatter2 = FixedFormatter::builder(6).break_words(false);
    assert_eq!(
        no_break_formatter2.format("Example text."),
        "Exampl\ne text\n."
    );

    let no_break_formatter3 = FixedFormatter::builder(7).break_words(false);
    assert_eq!(
        no_break_formatter3.format("Example text."),
        "Example\ntext."
    );

    let real_world_no_break_formatter = FixedFormatter::builder(32).break_words(false);
    assert_eq!(
        real_world_no_break_formatter.format("○咕噜咕噜（Temporary Magnetic Pattern Linkage）"),
        "○咕噜咕噜（Temporary Magnetic Pattern\nLinkage）"
    );

    let formatter3 = FixedFormatter::builder(10)
        .break_words(false)
        .insert_fullwidth_space_at_line_start(true);
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

    let formatter3b = FixedFormatter::builder(10)
        .break_words(false)
        .no_remove_space_at_line_start(true);

    assert_eq!(
        formatter3b.format("（This） 「is a test."),
        "（This） 「is\n a test."
    );

    let formatter4 = FixedFormatter::builder(10)
        .break_words(false)
        .break_with_sentence(true);
    assert_eq!(
        formatter4.format("『打断测，测试一下……』"),
        "『打断测，\n测试一下……』"
    );

    assert_eq!(
        formatter4.format("『打断测，测试一下。』"),
        "『打断测，\n测试一下。』"
    );

    assert_eq!(
        formatter4.format("『打断是测试一下哦……』"),
        "『打断是测试一下哦\n……』"
    );

    assert_eq!(
        formatter4.format("『打断测是测试一下。』"),
        "『打断测是测试一下。\n』"
    );

    assert_eq!(
        formatter4.format("『打断测试，测试一下。』"),
        "『打断测试，\n测试一下。』"
    );

    assert_eq!(
        formatter4.format("这打断测试，测试一下。"),
        "这打断测试，\n测试一下。"
    );

    assert_eq!(
        formatter4.format("这打断测试哦测试一下。。"),
        "这打断测试哦测试一下\n。。"
    );

    let formatter5 = FixedFormatter::builder(10)
        .break_words(false)
        .insert_fullwidth_space_at_line_start(true)
        .break_with_sentence(true);
    assert_eq!(
        formatter5.format("「一二三四『whatthe』"),
        "「一二三四\n\u{3000}『whatthe』"
    );

    let real_break_formatter = FixedFormatter::builder(27)
        .break_words(false)
        .break_with_sentence(true);
    assert_eq!(
        real_break_formatter.format("「他们就是想和阳见待在一个社团，在里面表现表现、耍耍帅，这样不就和她套上近乎了嘛！算盘珠子都打到我脸上了……」"),
        "「他们就是想和阳见待在一个社团，\n在里面表现表现、耍耍帅，这样不就和她套上近乎了嘛！算盘\n珠子都打到我脸上了……」"
    );

    assert_eq!(
        real_break_formatter
            .format("「在英山的话或许可以看看『moon river』『Lavir』或是『Patisserie Yuzuru』」"),
        "「在英山的话或许可以看看『moon river』\n『Lavir』或是『Patisserie Yuzuru\n』」"
    );

    #[cfg(feature = "circus")]
    {
        let circus_formatter = FixedFormatter::builder(10).typ(Some(ScriptType::Circus));
        assert_eq!(
            circus_formatter.format("● @cmd1@cmd2@cmd3中文字数是一\n　二三　四五六七八九十"),
            "● @cmd1@cmd2@cmd3中文字数是一二三\n四五六七八九十"
        );
        assert_eq!(
            circus_formatter
                .format("● @cmd1@cmd2@cmd3｛rubyText／中文｝字数是一\n　二三　四五六七八九十"),
            "● @cmd1@cmd2@cmd3｛rubyText／中文｝字数是一二三\n四五六七八九十"
        );
        let circus_formatter2 = FixedFormatter::builder(32).typ(Some(ScriptType::Circus));
        assert_eq!(
            circus_formatter2.format("@re1@re2@b1@t30@w1「当然现在我很幸福哦？\n　因为有你在身边」@n\n「@b1@t38@w1当然现在我很幸福哦？\n　因为有敦也君在身边」"),
            "@re1@re2@b1@t30@w1「当然现在我很幸福哦？因为有你在身边」@n\n「@b1@t38@w1当然现在我很幸福哦？因为有敦也君在身边」"
        );
    }

    #[cfg(feature = "kirikiri")]
    {
        let scn_formatter = FixedFormatter::builder(3)
            .break_words(false)
            .typ(Some(ScriptType::KirikiriScn));
        assert_eq!(
            scn_formatter.format("%test;[ruby]测[test]试打断。"),
            "%test;[ruby]测[test]试打\n断。"
        );
        assert_eq!(
            scn_formatter.format("%f$ハート$;#00ffadd6;♥%r打断测试"),
            "%f$ハート$;#00ffadd6;♥%r打断\n测试"
        )
    }
    #[cfg(feature = "jieba")]
    {
        let jieba_formatter = FixedFormatter::builder(8)
            .break_words(false)
            .break_chinese_words(false)
            .unwrap();
        assert_eq!(
            jieba_formatter.format("测试分词，我们中出了一个叛徒。"),
            "测试分词，我们中\n出了一个叛徒。"
        );
        let jieba_formatter2 = FixedFormatter::builder(8)
            .break_words(false)
            .break_chinese_words(false)
            .unwrap()
            .add_dict("中出", Some(114514), None);
        assert_eq!(
            jieba_formatter2
                .jieba
                .as_ref()
                .is_some_and(|s| s.has_word("中出")),
            true
        );
        assert_eq!(
            jieba_formatter2.format("测试分词，我们中出了一个叛徒。"),
            "测试分词，我们\n中出了一个叛徒。"
        );
    }
}
