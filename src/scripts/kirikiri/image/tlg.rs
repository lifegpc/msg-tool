//! Kirikiri TLG Image File (.tlg)
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use anyhow::Result;
use libtlg_rs::*;
use std::io::{Read, Seek};

#[derive(Debug)]
/// Kirikiri TLG Script Builder
pub struct TlgImageBuilder {}

impl TlgImageBuilder {
    /// Creates a new instance of `TlgImageBuilder`
    pub const fn new() -> Self {
        TlgImageBuilder {}
    }
}

impl ScriptBuilder for TlgImageBuilder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Cp932
    }

    fn build_script(
        &self,
        data: Vec<u8>,
        _filename: &str,
        _encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(TlgImage::new(MemReader::new(data), config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["tlg", "tlg5", "tlg6"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::KirikiriTlg
    }

    fn is_image(&self) -> bool {
        true
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 11 {
            if is_valid_tlg(buf) {
                return Some(255);
            }
        }
        None
    }
}

#[derive(Debug)]
/// Kirikiri TLG Script
pub struct TlgImage {
    data: Tlg,
}

impl TlgImage {
    /// Create a new TLG script
    ///
    /// * `data` - The reader containing the TLG script data
    /// * `config` - Extra configuration options
    pub fn new<T: Read + Seek>(data: T, _config: &ExtraConfig) -> Result<Self> {
        let tlg = load_tlg(data)?;
        Ok(TlgImage { data: tlg })
    }
}

impl Script for TlgImage {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Json
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn is_image(&self) -> bool {
        true
    }

    fn export_image(&self) -> Result<ImageData> {
        Ok(ImageData {
            width: self.data.width,
            height: self.data.height,
            color_type: match self.data.color {
                TlgColorType::Bgr24 => ImageColorType::Bgr,
                TlgColorType::Bgra32 => ImageColorType::Bgra,
                TlgColorType::Grayscale8 => ImageColorType::Grayscale,
            },
            depth: 8,
            data: self.data.data.clone(),
        })
    }
}
