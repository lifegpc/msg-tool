//! Circus Differential Image File (.crx)
use super::crx::CrxImage;
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use anyhow::Result;
use std::io::{Read, Seek};

#[derive(Debug)]
/// Circus CRXD Image Builder
pub struct CrxdImageBuilder {}

impl CrxdImageBuilder {
    /// Creates a new instance of `CrxdImageBuilder`.
    pub fn new() -> Self {
        Self {}
    }
}

impl ScriptBuilder for CrxdImageBuilder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Cp932
    }

    fn build_script(
        &self,
        data: Vec<u8>,
        filename: &str,
        encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
        archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(CrxdImage::new(
            MemReader::new(data),
            filename,
            encoding,
            config,
            archive,
        )?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["crx"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::CircusCrxd
    }

    fn is_image(&self) -> bool {
        true
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 4 && buf.starts_with(b"CRXD") {
            return Some(255);
        }
        None
    }
}

#[derive(Debug)]
/// Circus CRXD Image
pub struct CrxdImage {
    base: CrxImage,
    diff: CrxImage,
}

impl CrxdImage {
    /// Creates a new `CrxdImage` from the given data and configuration.
    ///
    /// * `data` - The reader to read the CRXD image from.
    /// * `filename` - The name of the file to read.
    /// * `encoding` - The encoding to use for string fields.
    /// * `config` - Extra configuration options.
    /// * `archive` - Optional archive to read the image from.
    pub fn new<T: Read + Seek>(
        data: T,
        filename: &str,
        encoding: Encoding,
        config: &ExtraConfig,
        archive: Option<&Box<dyn Script>>,
    ) -> Result<Self> {
        let mut reader = data;
        let mut magic = [0; 4];
        reader.read_exact(&mut magic)?;
        if magic != *b"CRXD" {
            return Err(anyhow::anyhow!("Invalid CRXD magic"));
        }
        reader.seek_relative(4)?;
        let offset = reader.read_u32()?;
        let name = reader.read_fstring(0x14, encoding, true)?;
        let base = if let Some(archive) = archive {
            CrxImage::new(
                archive.open_file_by_offset(offset as u64)?.to_data()?,
                config,
            )?
        } else {
            let mut nf = std::path::PathBuf::from(filename);
            nf.set_file_name(name);
            let f = std::fs::File::open(nf)?;
            CrxImage::new(std::io::BufReader::new(f), config)?
        }
        .with_canvas(false);
        let mut typ = [0; 4];
        reader.read_exact(&mut typ)?;
        if typ == *b"CRXJ" {
            reader.seek_relative(4)?;
            let offset = reader.read_u32()?;
            let diff = Self::read_diff(
                archive
                    .ok_or(anyhow::anyhow!("No archive provided"))?
                    .open_file_by_offset(offset as u64)?
                    .to_data()?,
                archive.clone(),
                config,
            )?;
            return Ok(Self { base, diff });
        } else if typ == *b"CRXG" {
            let reader = StreamRegion::with_start_pos(reader, 0x20)?;
            let diff = CrxImage::new(reader, config)?.with_canvas(false);
            return Ok(Self { base, diff });
        }
        Err(anyhow::anyhow!("Unsupported diff CRXD type: {:?}", typ))
    }

    fn read_diff<T: Read + Seek>(
        mut reader: T,
        archive: Option<&Box<dyn Script>>,
        config: &ExtraConfig,
    ) -> Result<CrxImage> {
        let mut magic = [0; 4];
        reader.read_exact(&mut magic)?;
        if magic != *b"CRXD" {
            return Err(anyhow::anyhow!("Invalid CRXD magic"));
        }
        reader.seek_relative(0x1C)?;
        let mut typ = [0; 4];
        reader.read_exact(&mut typ)?;
        if typ == *b"CRXJ" {
            reader.seek_relative(4)?;
            let offset = reader.read_u32()?;
            return Self::read_diff(
                archive
                    .ok_or(anyhow::anyhow!("No archive provided"))?
                    .open_file_by_offset(offset as u64)?
                    .to_data()?,
                archive,
                config,
            );
        } else if typ == *b"CRXG" {
            let reader = StreamRegion::with_start_pos(reader, 0x20)?;
            return Ok(CrxImage::new(reader, config)?.with_canvas(false));
        }
        Err(anyhow::anyhow!("Unsupported diff CRXD type: {:?}", typ))
    }
}

impl Script for CrxdImage {
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
        self.base.draw_diff(&self.diff)
    }
}
