//! Module for formatting messages.
mod fixed;

use crate::types::*;
use anyhow::Result;

/// Formats messages with the given options.
pub fn fmt_message(mes: &mut Vec<Message>, opt: FormatOptions, typ: ScriptType) -> Result<()> {
    match opt {
        FormatOptions::Fixed {
            length,
            keep_original,
            break_words,
            insert_fullwidth_space_at_line_start,
            break_with_sentence,
            #[cfg(feature = "jieba")]
            break_chinese_words,
            #[cfg(feature = "jieba")]
            jieba_dict,
            no_remove_space_at_line_start,
        } => {
            let formatter = fixed::FixedFormatter::new(
                length,
                keep_original,
                break_words,
                insert_fullwidth_space_at_line_start,
                break_with_sentence,
                #[cfg(feature = "jieba")]
                break_chinese_words,
                #[cfg(feature = "jieba")]
                jieba_dict,
                no_remove_space_at_line_start,
                Some(typ),
            )?;
            for message in mes.iter_mut() {
                message.message = formatter.format(&message.message);
            }
        }
        FormatOptions::None => {}
    }
    Ok(())
}
