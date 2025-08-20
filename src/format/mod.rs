//! Module for formatting messages.
mod fixed;

use crate::types::*;

/// Formats messages with the given options.
pub fn fmt_message(mes: &mut Vec<Message>, opt: FormatOptions, typ: ScriptType) {
    match opt {
        FormatOptions::Fixed {
            length,
            keep_original,
            break_words,
        } => {
            let formatter =
                fixed::FixedFormatter::new(length, keep_original, break_words, Some(typ));
            for message in mes.iter_mut() {
                message.message = formatter.format(&message.message);
            }
        }
        FormatOptions::None => {}
    }
}
