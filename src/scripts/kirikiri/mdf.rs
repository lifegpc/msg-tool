//! Kirikiri Zlib-Compressed File
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use anyhow::Result;
use std::io::Read;

#[derive(Debug)]
/// Kirikiri MDF Script Builder
pub struct MdfBuilder {}

impl MdfBuilder {
    /// Creates a new instance of `MdfBuilder`
    pub fn new() -> Self {
        Self {}
    }
}

impl ScriptBuilder for MdfBuilder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Utf8
    }

    fn build_script(
        &self,
        buf: Vec<u8>,
        filename: &str,
        _encoding: Encoding,
        _archive_encoding: Encoding,
        _config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(Mdf::new(buf, filename)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::KirikiriMdf
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 4 && buf.starts_with(b"mdf\0") {
            Some(10)
        } else {
            None
        }
    }
}

#[derive(Debug)]
/// Kirikiri MDF Script
pub struct Mdf {
    data: MemReader,
    ext: String,
}

impl Mdf {
    /// Creates a new `Mdf` script from the given buffer and filename
    ///
    /// * `buf` - The buffer containing the MDF data
    /// * `filename` - The name of the file (used for extension detection)
    pub fn new(buf: Vec<u8>, filename: &str) -> Result<Self> {
        let mut data = MemReader::new(buf);
        let mut header = [0u8; 4];
        data.read_exact(&mut header)?;
        if &header != b"mdf\0" {
            return Err(anyhow::anyhow!("Invalid MDF header"));
        }
        Ok(Self {
            data,
            ext: std::path::Path::new(filename)
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string(),
        })
    }

    pub(crate) fn unpack(mut data: MemReaderRef) -> Result<Vec<u8>> {
        let size = data.read_u32()?;
        let mut decoder = flate2::read::ZlibDecoder::new(data);
        let mut result = Vec::new();
        decoder.read_to_end(&mut result)?;
        if size as usize != result.len() {
            eprintln!(
                "Warning: MDF unpacked size mismatch: expected {}, got {}",
                size,
                result.len()
            );
            crate::COUNTER.inc_warning();
        }
        Ok(result)
    }
}

impl Script for Mdf {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Custom
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn is_output_supported(&self, output: OutputScriptType) -> bool {
        matches!(output, OutputScriptType::Custom)
    }

    fn custom_output_extension<'a>(&'a self) -> &'a str {
        &self.ext
    }

    fn custom_export(&self, filename: &std::path::Path, _encoding: Encoding) -> Result<()> {
        let data = Self::unpack(MemReaderRef::new(&self.data.data[4..]))?;
        let mut writer = crate::utils::files::write_file(filename)?;
        writer.write_all(&data)?;
        Ok(())
    }
}
