//! Extensions for other crates.
pub mod atomic;
#[cfg(feature = "fancy-regex")]
pub mod fancy_regex;
pub mod io;
#[cfg(feature = "emote-psb")]
pub mod psb;
pub mod vec;
