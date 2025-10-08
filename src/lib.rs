//! A Rust library for exporting, importing, packing, and unpacking script files.
//!
//! For more information, please visit the [GitHub repository](https://github.com/lifegpc/msg-tool).
#![cfg_attr(any(docsrs, feature = "unstable"), feature(doc_cfg))]
pub mod ext;
pub mod format;
pub mod output_scripts;
pub mod scripts;
pub mod types;
pub mod utils;

lazy_static::lazy_static! {
    static ref COUNTER: utils::counter::Counter = utils::counter::Counter::new();
}

/// Returns a reference to the global counter instance.
pub fn get_counter() -> &'static utils::counter::Counter {
    &COUNTER
}
