//! Qlie tiled PNG image (.png)
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::img::*;
use crate::utils::psd::*;
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

    fn can_create_image_file(&self) -> bool {
        true
    }

    fn create_image_file<'a>(
        &'a self,
        data: ImageData,
        filename: &str,
        writer: Box<dyn WriteSeek + 'a>,
        options: &ExtraConfig,
    ) -> Result<()> {
        if options.qlie_dpng_use_raw_png {
            create_raw_png_image(filename, writer, None)
        } else {
            create_image(data, writer, options)
        }
    }
}

#[derive(Debug)]
pub struct DpngImage {
    img: DpngFile,
    config: ExtraConfig,
}

impl DpngImage {
    pub fn new<T: Read + Seek>(mut data: T, config: &ExtraConfig) -> Result<Self> {
        let img = DpngFile::unpack(&mut data, false, Encoding::Utf8, &None)?;
        if img.header.magic != *b"DPNG" {
            anyhow::bail!("Not a valid DPNG image");
        }
        if img.tiles.is_empty() {
            anyhow::bail!("DPNG image has no tiles");
        }
        Ok(DpngImage {
            img,
            config: config.clone(),
        })
    }
}

impl Script for DpngImage {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Custom
    }

    fn is_output_supported(&self, output: OutputScriptType) -> bool {
        matches!(output, OutputScriptType::Custom)
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn custom_output_extension<'a>(&'a self) -> &'a str {
        "psd"
    }

    fn is_image(&self) -> bool {
        if self.config.qlie_dpng_psd {
            false
        } else {
            true
        }
    }

    fn export_image(&self) -> Result<ImageData> {
        let (idx, tile) = self
            .img
            .tiles
            .iter()
            .enumerate()
            .find(|(_, t)| t.size != 0)
            .ok_or_else(|| anyhow::anyhow!("DPNG image has no valid tiles with PNG data"))?;
        let mut base = load_png(MemReaderRef::new(&tile.png_data))?;
        convert_to_rgba(&mut base)?;
        let mut base = draw_on_canvas(
            base,
            self.img.header.image_width,
            self.img.header.image_height,
            tile.x,
            tile.y,
        )?;
        for tile in &self.img.tiles[idx + 1..] {
            if tile.size == 0 {
                continue;
            }
            let mut diff = load_png(MemReaderRef::new(&tile.png_data))?;
            convert_to_rgba(&mut diff)?;
            draw_on_image(&mut base, &diff, tile.x, tile.y)?;
        }
        Ok(base)
    }

    fn import_image<'a>(
        &'a self,
        data: ImageData,
        filename: &str,
        file: Box<dyn WriteSeek + 'a>,
    ) -> Result<()> {
        if self.config.qlie_dpng_use_raw_png {
            let img = load_png(std::fs::File::open(filename)?)?;
            if img.width != self.img.header.image_width
                || img.height != self.img.header.image_height
            {
                eprintln!(
                    "Warning: Image dimensions do not match original DPNG image (expected {}x{}, got {}x{})",
                    self.img.header.image_width,
                    self.img.header.image_height,
                    img.width,
                    img.height
                );
                crate::COUNTER.inc_warning();
            }
            create_raw_png_image(filename, file, Some(img))?;
        } else {
            if data.width != self.img.header.image_width
                || data.height != self.img.header.image_height
            {
                eprintln!(
                    "Warning: Image dimensions do not match original DPNG image (expected {}x{}, got {}x{})",
                    self.img.header.image_width,
                    self.img.header.image_height,
                    data.width,
                    data.height
                );
                crate::COUNTER.inc_warning();
            }
            create_image(data, file, &self.config)?;
        }
        Ok(())
    }

    fn custom_export(&self, filename: &std::path::Path, encoding: Encoding) -> Result<()> {
        let mut psd = PsdWriter::new(
            self.img.header.image_width,
            self.img.header.image_height,
            ImageColorType::Rgba,
            8,
        )?;
        let (idx, tile) = self
            .img
            .tiles
            .iter()
            .enumerate()
            .find(|(_, t)| t.size != 0)
            .ok_or_else(|| anyhow::anyhow!("DPNG image has no valid tiles with PNG data"))?;
        let mut base = load_png(MemReaderRef::new(&tile.png_data))?;
        psd.add_layer(&format!("layer_{}", idx), tile.x, tile.y, base.clone())?;
        convert_to_rgba(&mut base)?;
        let mut base = draw_on_canvas(
            base,
            self.img.header.image_width,
            self.img.header.image_height,
            tile.x,
            tile.y,
        )?;
        let mut idx2 = idx;
        for tile in &self.img.tiles[idx + 1..] {
            idx2 += 1;
            if tile.size == 0 {
                continue;
            }
            let mut diff = load_png(MemReaderRef::new(&tile.png_data))?;
            psd.add_layer(&format!("layer_{}", idx2), tile.x, tile.y, diff.clone())?;
            convert_to_rgba(&mut diff)?;
            draw_on_image(&mut base, &diff, tile.x, tile.y)?;
        }
        let file = std::fs::File::create(filename)?;
        let mut writer = std::io::BufWriter::new(file);
        psd.save(base, &mut writer, encoding)?;
        Ok(())
    }
}

fn create_raw_png_image<'a>(
    filename: &str,
    mut file: Box<dyn WriteSeek + 'a>,
    img: Option<ImageData>,
) -> Result<()> {
    let img = match img {
        Some(img) => img,
        None => load_png(std::fs::File::open(filename)?)?,
    };
    let header = DpngHeader {
        magic: *b"DPNG",
        _unk1: 1,
        tile_count: 1,
        image_width: img.width,
        image_height: img.height,
    };
    let png_data = crate::utils::files::read_file(filename)?;
    let tile = Tile {
        x: 0,
        y: 0,
        width: img.width,
        height: img.height,
        size: png_data.len() as u32,
        _unk: 0,
        png_data,
    };
    let dpng = DpngFile {
        header,
        tiles: vec![tile],
    };
    dpng.pack(&mut file, false, Encoding::Utf8, &None)?;
    Ok(())
}

fn create_image<'a>(
    image: ImageData,
    mut writer: Box<dyn WriteSeek + 'a>,
    config: &ExtraConfig,
) -> Result<()> {
    let header = DpngHeader {
        magic: *b"DPNG",
        _unk1: 1,
        tile_count: 1,
        image_width: image.width,
        image_height: image.height,
    };
    let mut png_data = MemWriter::new();
    let width = image.width;
    let height = image.height;
    encode_img_writer(image, ImageOutputType::Png, &mut png_data, config)?;
    let png_data = png_data.into_inner();
    let tile = Tile {
        x: 0,
        y: 0,
        width,
        height,
        size: png_data.len() as u32,
        _unk: 0,
        png_data,
    };
    let dpng = DpngFile {
        header,
        tiles: vec![tile],
    };
    dpng.pack(&mut writer, false, Encoding::Utf8, &None)?;
    Ok(())
}
