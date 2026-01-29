//! Qlie tiled PNG image (.png)
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::img::*;
use crate::utils::struct_pack::*;
use anyhow::Result;
use msg_tool_macro::*;
use std::io::{Read, Seek, Write};

#[derive(StructPack, StructUnpack, Debug, Clone)]
struct DpngHeader {
    /// DPNG
    magic: [u8; 4],
    /// Seems to be always 1
    _unk1: u32,
    tile_count: u32,
    image_width: u32,
    image_height: u32,
}

#[derive(StructPack, StructUnpack, Debug, Clone)]
struct Tile {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    size: u32,
    _unk: u64,
    #[pack_vec_len(self.size)]
    #[unpack_vec_len(size)]
    png_data: Vec<u8>,
}

#[derive(StructPack, StructUnpack, Debug, Clone)]
struct DpngFile {
    header: DpngHeader,
    #[pack_vec_len(self.header.tile_count)]
    #[unpack_vec_len(header.tile_count)]
    tiles: Vec<Tile>,
}

#[derive(Debug)]
/// Qlie DPNG image builder
pub struct DpngImageBuilder {}

impl DpngImageBuilder {
    pub fn new() -> Self {
        Self {}
    }
}

impl ScriptBuilder for DpngImageBuilder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Utf8
    }

    fn build_script(
        &self,
        buf: Vec<u8>,
        _filename: &str,
        _encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(DpngImage::new(MemReader::new(buf), config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["png"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::QlieDpng
    }

    fn is_image(&self) -> bool {
        true
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 4 && buf.starts_with(b"DPNG") {
            Some(20)
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct DpngImage {
    img: DpngFile,
}

impl DpngImage {
    pub fn new<T: Read + Seek>(mut data: T, _config: &ExtraConfig) -> Result<Self> {
        let img = DpngFile::unpack(&mut data, false, Encoding::Utf8, &None)?;
        if img.header.magic != *b"DPNG" {
            anyhow::bail!("Not a valid DPNG image");
        }
        if img.tiles.is_empty() {
            anyhow::bail!("DPNG image has no tiles");
        }
        Ok(DpngImage { img })
    }
}

impl Script for DpngImage {
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
        let mut base = load_png(MemReaderRef::new(&self.img.tiles[0].png_data))?;
        convert_to_rgba(&mut base)?;
        let mut base = draw_on_canvas(
            base,
            self.img.header.image_width,
            self.img.header.image_height,
            self.img.tiles[0].x,
            self.img.tiles[0].y,
        )?;
        for tile in &self.img.tiles[1..] {
            let mut diff = load_png(MemReaderRef::new(&tile.png_data))?;
            convert_to_rgba(&mut diff)?;
            draw_on_image(&mut base, &diff, tile.x, tile.y)?;
        }
        Ok(base)
    }
}
