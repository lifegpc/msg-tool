//! Extensions for other crates.
pub mod atomic;
#[cfg(feature = "fancy-regex")]
pub mod fancy_regex;
pub mod io;
#[cfg(feature = "emote-psb")]
pub mod psb;
#[cfg(feature = "markup5ever_rcdom")]
pub mod rcdom;
pub mod vec;
