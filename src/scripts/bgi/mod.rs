//! Buriko General Interpreter / Ethornell Scripts
#[cfg(feature = "bgi-arc")]
pub mod archive;
#[cfg(feature = "bgi-audio")]
pub mod audio;
pub mod bp;
pub mod bsi;
#[cfg(feature = "bgi-img")]
pub mod image;
mod parser;
pub mod script;
