pub mod base;
pub mod bgi;
pub mod circus;
pub mod escude;

pub use base::{Script, ScriptBuilder};

lazy_static::lazy_static! {
    pub static ref BUILDER: Vec<Box<dyn ScriptBuilder + Sync + Send>> = vec![
        Box::new(circus::script::CircusMesScriptBuilder::new()),
        Box::new(bgi::script::BGIScriptBuilder::new()),
        Box::new(escude::archive::EscudeBinArchiveBuilder::new()),
        Box::new(escude::script::EscudeBinScriptBuilder::new()),
    ];
    pub static ref ALL_EXTS: Vec<String> =
        BUILDER.iter().flat_map(|b| b.extensions()).map(|s| s.to_string()).collect();
}
