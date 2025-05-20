pub mod base;
pub mod circus;

pub use base::{Script, ScriptBuilder};

lazy_static::lazy_static! {
    pub static ref BUILDER: Vec<Box<dyn ScriptBuilder + Sync + Send>> = vec![
        Box::new(circus::script::CircusMesScriptBuilder::new()),
    ];
    pub static ref ALL_EXTS: Vec<String> =
        BUILDER.iter().flat_map(|b| b.extensions()).map(|s| s.to_string()).collect();
}
