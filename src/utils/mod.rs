//! Utility functions and modules.
#[cfg(feature = "utils-bit-stream")]
pub mod bit_stream;
#[cfg(feature = "utils-blowfish")]
pub mod blowfish;
pub mod counter;
#[cfg(feature = "utils-crc32")]
pub mod crc32;
pub mod encoding;
#[cfg(windows)]
mod encoding_win;
#[cfg(feature = "utils-escape")]
pub mod escape;
pub mod files;
#[cfg(feature = "audio-flac")]
pub mod flac;
#[cfg(feature = "image")]
pub mod img;
#[cfg(feature = "image-jxl")]
pub mod jxl;
#[cfg(feature = "lossless-audio")]
pub mod lossless_audio;
mod macros;
pub mod name_replacement;
pub mod num_range;
#[cfg(feature = "utils-pcm")]
pub mod pcm;
#[cfg(feature = "utils-str")]
pub mod str;
pub mod struct_pack;
#[cfg(feature = "utils-threadpool")]
pub mod threadpool;

#[cfg(windows)]
pub use encoding_win::WinError;
