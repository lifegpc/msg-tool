pub mod base;
#[cfg(feature = "bgi")]
pub mod bgi;
#[cfg(feature = "circus")]
pub mod circus;
#[cfg(feature = "escude")]
pub mod escude;

pub use base::{Script, ScriptBuilder};

lazy_static::lazy_static! {
    pub static ref BUILDER: Vec<Box<dyn ScriptBuilder + Sync + Send>> = vec![
        #[cfg(feature = "circus")]
        Box::new(circus::script::CircusMesScriptBuilder::new()),
        #[cfg(feature = "bgi")]
        Box::new(bgi::script::BGIScriptBuilder::new()),
        #[cfg(feature = "escude-arc")]
        Box::new(escude::archive::EscudeBinArchiveBuilder::new()),
        #[cfg(feature = "escude")]
        Box::new(escude::script::EscudeBinScriptBuilder::new()),
        #[cfg(feature = "escude")]
        Box::new(escude::list::EscudeBinListBuilder::new()),
    ];
    pub static ref ALL_EXTS: Vec<String> =
        BUILDER.iter().flat_map(|b| b.extensions()).map(|s| s.to_string()).collect();
    pub static ref ARCHIVE_EXTS: Vec<String> =
        BUILDER.iter().filter(|b| b.is_archive()).flat_map(|b| b.extensions()).map(|s| s.to_string()).collect();
}
