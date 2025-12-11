use super::base::*;
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::img::*;
use crate::utils::struct_pack::*;
use anyhow::Result;
use std::io::{Read, Seek};

#[derive(Debug)]
pub struct Pgd3Builder {}

impl Pgd3Builder {
    pub fn new() -> Self {
        Self {}
    }
}

impl ScriptBuilder for Pgd3Builder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Cp932
    }

    fn build_script(
        &self,
        buf: Vec<u8>,
        filename: &str,
        encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
        archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(Pgd3::new(
            MemReader::new(buf),
            filename,
            encoding,
            config,
            archive,
        )?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["pgd"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::SoftpalPgd3
    }

    fn is_image(&self) -> bool {
        true
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 4 && (buf.starts_with(b"PGD3") || buf.starts_with(b"PGD2")) {
            return Some(20);
        }
        None
    }
}

#[derive(Debug)]
pub struct Pgd3 {
    header: PgdDiffHeader,
    base_header: PgdGeHeader,
    base: ImageData,
    diff: ImageData,
    fake_compress: bool,
}

impl Pgd3 {
    pub fn new<R: Read + Seek>(
        mut reader: R,
        filename: &str,
        encoding: Encoding,
        config: &ExtraConfig,
        archive: Option<&Box<dyn Script>>,
    ) -> Result<Self> {
        let mut sig = [0u8; 4];
        reader.read_exact(&mut sig)?;
        if &sig != b"PGD3" && &sig != b"PGD2" {
            return Err(anyhow::anyhow!("Not a valid PGD3/PGD2 file"));
        }
        let header = PgdDiffHeader::unpack(&mut reader, false, encoding)?;
        let diff = PgdReader::with_diff_header(reader, &header)?.unpack_overlay()?;
        let base: Vec<u8> = if let Some(archive) = archive {
            let mut file = archive.open_file_by_name(&header.base_name, true)?;
            file.data()?
        } else {
            let path = {
                let mut pb = std::path::PathBuf::from(filename);
                pb.set_file_name(&header.base_name);
                pb
            };
            crate::utils::files::read_file(&path).map_err(|e| {
                anyhow::anyhow!("Failed to read base image file '{}': {}", path.display(), e)
            })?
        };
        let mut reader = MemReader::new(base);
        reader.read_exact(&mut sig)?;
        if &sig != b"GE \0" {
            return Err(anyhow::anyhow!(
                "Base image file '{}' is not a valid GE file",
                header.base_name
            ));
        }
        let base_header = PgdGeHeader::unpack(&mut reader, false, encoding)?;
        let base = PgdReader::with_ge_header(reader, &base_header)?.unpack_ge()?;
        Ok(Self {
            header,
            base_header,
            base,
            diff,
            fake_compress: config.pgd_fake_compress,
        })
    }
}

impl Script for Pgd3 {
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
        let mut base = if self.base_header.is_base_file() {
            self.base.clone()
        } else {
            draw_on_canvas(
                self.base.clone(),
                self.base_header.canvas_width,
                self.base_header.canvas_height,
                self.base_header.offset_x,
                self.base_header.offset_y,
            )?
        };
        draw_on_img(
            &mut base,
            &self.diff,
            self.header.offset_x as u32,
            self.header.offset_y as u32,
        )?;
        Ok(base)
    }

    fn import_image<'a>(
        &'a self,
        data: ImageData,
        mut file: Box<dyn WriteSeek + 'a>,
    ) -> Result<()> {
        let mut header = PgdGeHeader {
            offset_x: self.base_header.offset_x,
            offset_y: self.base_header.offset_y,
            width: self.base_header.width,
            height: self.base_header.height,
            canvas_height: self.base_header.canvas_height,
            canvas_width: self.base_header.canvas_width,
            mode: self.base_header.mode,
            _unk: self.base_header._unk,
        };
        if data.height != header.height {
            return Err(anyhow::anyhow!(
                "Image height does not match: expected {}, got {}",
                header.height,
                data.height
            ));
        }
        if data.width != header.width {
            return Err(anyhow::anyhow!(
                "Image width does not match: expected {}, got {}",
                header.width,
                data.width
            ));
        }
        header.mode = 3;
        file.write_all(b"GE \0")?;
        header.pack(&mut file, false, Encoding::Utf8)?;
        PgdWriter::new(data, self.fake_compress)
            .with_method(3)
            .pack_ge(&mut file)?;
        Ok(())
    }
}

fn draw_on_img(base: &mut ImageData, diff: &ImageData, left: u32, top: u32) -> Result<()> {
    if base.color_type != diff.color_type {
        return Err(anyhow::anyhow!(
            "Color types do not match: {:?} vs {:?}",
            base.color_type,
            diff.color_type
        ));
    }
    let bpp = base.color_type.bpp(1) as usize;
    let base_stride = base.width as usize * bpp;
    let diff_stride = diff.width as usize * bpp;

    for y in 0..diff.height {
        let base_y = top + y;
        if base_y >= base.height {
            continue; // Skip if the base image is not tall enough
        }

        for x in 0..diff.width {
            let base_x = left + x;
            if base_x >= base.width {
                continue; // Skip if the base image is not wide enough
            }

            let base_index = (base_y as usize * base_stride) + (base_x as usize * bpp);
            let diff_index = (y as usize * diff_stride) + (x as usize * bpp);

            let diff_pixel = &diff.data[diff_index..diff_index + bpp];
            let base_pixel_orig = base.data[base_index..base_index + bpp].to_vec();
            let mut b = base_pixel_orig[0];
            let mut g = base_pixel_orig[1];
            let mut r = base_pixel_orig[2];
            b ^= diff_pixel[0];
            g ^= diff_pixel[1];
            r ^= diff_pixel[2];
            base.data[base_index] = b;
            base.data[base_index + 1] = g;
            base.data[base_index + 2] = r;
            if bpp == 4 {
                let mut a = base_pixel_orig[3];
                a ^= diff_pixel[3];
                base.data[base_index + 3] = a;
            }
        }
    }
    Ok(())
}
