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
#[cfg(feature = "utils-mmx")]
pub mod mmx;
pub mod name_replacement;
pub mod num_range;
#[cfg(feature = "utils-pcm")]
pub mod pcm;
#[cfg(feature = "utils-psd")]
pub mod psd;
#[cfg(feature = "utils-rc4")]
pub mod rc4;
#[cfg(feature = "utils-serde-base64bytes")]
pub mod serde_base64bytes;
#[cfg(feature = "utils-str")]
pub mod str;
pub mod struct_pack;
pub mod threadpool;
#[cfg(feature = "utils-xored-stream")]
pub mod xored_stream;

#[cfg(windows)]
pub use encoding_win::WinError;
