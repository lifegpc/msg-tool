mod fixed;

use crate::types::*;

pub fn fmt_message(mes: &mut Vec<Message>, opt: FormatOptions) {
    match opt {
        FormatOptions::Fixed {
            length,
            keep_original,
        } => {
            let formatter = fixed::FixedFormatter::new(length, keep_original);
            for message in mes.iter_mut() {
                message.message = formatter.format(&message.message);
            }
        }
        FormatOptions::None => {}
    }
}
