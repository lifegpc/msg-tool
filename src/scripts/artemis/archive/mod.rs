//! Artemis Engine Archive
pub mod pf2;
pub mod pfs;
use crate::types::ScriptType;

fn detect_script_type(buf: &[u8], buf_len: usize, filename: &str) -> Option<ScriptType> {
    if buf_len >= 5 && buf.starts_with(b"ASB\0\0") {
        return Some(ScriptType::ArtemisAsb);
    }
    if super::ast::is_this_format(filename, buf, buf_len) {
        return Some(ScriptType::Artemis);
    }
    None
}
