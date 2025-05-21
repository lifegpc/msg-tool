use unicode_segmentation::UnicodeSegmentation;

pub struct FixedFormatter {
    length: usize,
    keep_original: bool,
}

impl FixedFormatter {
    pub fn new(length: usize, keep_original: bool) -> Self {
        FixedFormatter {
            length,
            keep_original,
        }
    }

    pub fn format(&self, message: &str) -> String {
        let mut result = String::new();
        let vec: Vec<_> = UnicodeSegmentation::graphemes(message, true).collect();
        let mut current_length = 0;
        for grapheme in vec {
            if grapheme == "\n" {
                if self.keep_original {
                    result.push('\n');
                    current_length = 0;
                }
                continue;
            }
            if current_length >= self.length {
                result.push('\n');
                current_length = 0;
            }
            result.push_str(grapheme);
            current_length += 1;
        }
        return result;
    }
}

#[test]
fn test_format() {
    let formatter = FixedFormatter::new(10, false);
    let message = "This is a test message.\nThis is another line.";
    let formatted_message = formatter.format(message);
    assert_eq!(
        formatted_message,
        "This is a \ntest messa\nge.This is\n another l\nine."
    );
    assert_eq!(formatter.format("● This is a test."), "● This is \na test.");
    let fommater2 = FixedFormatter::new(10, true);
    assert_eq!(
        fommater2.format("● Th\nis is a test."),
        "● Th\nis is a te\nst."
    );
}
