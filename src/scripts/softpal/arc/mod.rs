pub mod pac;

use crate::types::*;

fn detect_script_type(_filename: &str, data: &[u8]) -> Option<ScriptType> {
    if data.len() >= 4 && data.starts_with(b"Sv20") {
        return Some(ScriptType::Softpal);
    }
    #[cfg(feature = "softpal-img")]
    if data.len() >= 4 && data.starts_with(b"GE \0") {
        return Some(ScriptType::SoftpalPgdGe);
    }
    #[cfg(feature = "softpal-img")]
    if data.len() >= 4 && (data.starts_with(b"PGD3") || data.starts_with(b"PGD2")) {
        return Some(ScriptType::SoftpalPgd3);
    }
    None
}
