pub mod arcc;
pub mod wag;

use crate::types::ScriptType;

fn detect_script_type(_filename: &str, buf: &[u8]) -> Option<ScriptType> {
    if buf.len() >= 4 && buf.starts_with(b"NORI") {
        return Some(ScriptType::HexenHaus);
    }
    None
}
