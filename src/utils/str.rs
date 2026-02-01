//! String Utilities
use crate::types::*;
use crate::utils::encoding::*;
use anyhow::Result;
use unicode_segmentation::UnicodeSegmentation;

/// Truncate a string to a specified length, encoding it with the given encoding.
/// Output size may less than or equal to the specified length.
pub fn truncate_string(s: &str, length: usize, encoding: Encoding, check: bool) -> Result<Vec<u8>> {
    let vec: Vec<_> = UnicodeSegmentation::graphemes(s, true).collect();
    let mut result = Vec::new();
    for graphemes in vec {
        let data = encode_string(encoding, graphemes, check)?;
        if result.len() + data.len() > length {
            break;
        }
        result.extend(data);
    }
    return Ok(result);
}

/// Truncate a string to a specified length, encoding it with the given encoding.
/// Output size may less than or equal to the specified length.
/// Returns the encoded bytes and the remaining string.
pub fn truncate_string2(
    s: &str,
    length: usize,
    encoding: Encoding,
) -> Result<(Vec<u8>, Option<&str>)> {
    let vec: Vec<_> = UnicodeSegmentation::graphemes(s, true).collect();
    let mut result = Vec::new();
    let mut used = 0;
    for graphemes in vec {
        let data = encode_string(encoding, graphemes, false)?;
        if result.len() + data.len() > length {
            break;
        }
        result.extend(data);
        used += graphemes.len();
    }
    let remaining = if used < s.len() {
        Some(&s[used..])
    } else {
        None
    };
    return Ok((result, remaining));
}

/// Truncate a string to a specified length, encoding it with the given encoding.
/// Output size may less than or equal to the specified length.
/// Returns the encoded bytes and the remaining string.
/// Will try splitting at line breaks first.
pub fn truncate_string_with_enter(
    s: &str,
    length: usize,
    encoding: Encoding,
) -> Result<(Vec<u8>, Option<&str>)> {
    if let Some(pos) = s.find('\n') {
        let (first, rest) = s.split_at(pos + 1);
        // Try encoding the first part with line break
        let data = encode_string(encoding, &first[..pos], false)?;
        if data.len() <= length {
            return Ok((data, if rest.is_empty() { None } else { Some(rest) }));
        }
    }
    truncate_string2(s, length, encoding)
}
