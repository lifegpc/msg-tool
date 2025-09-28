//! HexenHaus PNG Image
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::img::*;
use anyhow::Result;
use std::io::{Read, Seek, SeekFrom};

#[derive(Debug)]
/// HexenHaus PNG Image Builder
pub struct PngImageBuilder {}

impl PngImageBuilder {
    /// Creates a new instance of `PngImageBuilder`
    pub fn new() -> Self {
        PngImageBuilder {}
    }
}

impl ScriptBuilder for PngImageBuilder {
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
        Ok(Box::new(PngImage::new(MemReader::new(data), config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["png"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::HexenHausPng
    }

    fn is_image(&self) -> bool {
        true
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 4 && buf.starts_with(b"IMGD") {
            return Some(10);
        }
        None
    }
}

#[derive(Debug)]
/// Extra information for PNG image
pub struct ExtraInfo {
    /// x offset
    pub offset_x: u32,
    /// y offset
    pub offset_y: u32,
}

#[derive(Debug)]
pub struct PngImage {
    reader: MemReader,
    extra: Option<ExtraInfo>,
}

impl PngImage {
    /// Creates a new instance of `PngImage`
    pub fn new(mut reader: MemReader, _config: &ExtraConfig) -> Result<Self> {
        let mut header = [0; 4];
        reader.read_exact(&mut header)?;
        if &header != b"IMGD" {
            return Err(anyhow::anyhow!("Not a valid HexenHaus PNG image"));
        }
        reader.seek(SeekFrom::End(-14))?;
        let cnt = reader.read_exact_vec(12)?;
        let extra = if cnt.starts_with(b"CNTR") {
            let mut cnt_reader = MemReaderRef::new(&cnt[4..]);
            let offset_x = cnt_reader.read_u32()?;
            let offset_y = cnt_reader.read_u32()?;
            Some(ExtraInfo { offset_x, offset_y })
        } else {
            None
        };
        Ok(PngImage { reader, extra })
    }
}

impl Script for PngImage {
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
        let mut reader = self.reader.to_ref();
        reader.pos = 0;
        let reader = StreamRegion::with_start_pos(reader, 0x10)?;
        let img = load_png(reader)?;
        Ok(img)
    }

    fn extra_info<'a>(&'a self) -> Option<Box<dyn AnyDebug + 'a>> {
        self.extra
            .as_ref()
            .map(|e| Box::new(e) as Box<dyn AnyDebug>)
    }
}
