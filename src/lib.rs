//! A Rust library for exporting, importing, packing, and unpacking script files.
//!
//! For more information, please visit the [GitHub repository](https://github.com/lifegpc/msg-tool).
pub mod ext;
pub mod format;
pub mod output_scripts;
pub mod scripts;
pub mod types;
pub mod utils;

lazy_static::lazy_static! {
    static ref COUNTER: utils::counter::Counter = utils::counter::Counter::new();
}
