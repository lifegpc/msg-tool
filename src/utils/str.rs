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
