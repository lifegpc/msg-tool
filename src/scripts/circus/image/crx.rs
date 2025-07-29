use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::struct_pack::*;
use anyhow::Result;
use msg_tool_macro::*;
use std::io::{Read, Seek, Write};

#[derive(Debug)]
pub struct CrxImageBuilder {}

impl CrxImageBuilder {
    pub const fn new() -> Self {
        CrxImageBuilder {}
    }
}

impl ScriptBuilder for CrxImageBuilder {
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
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(CrxImage::new(MemReader::new(data), config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["crx"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::CircusCrx
    }

    fn is_image(&self) -> bool {
        true
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 4 && buf.starts_with(b"CRXG") {
            return Some(255);
        }
        None
    }
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
struct Clip {
    field_0: u32,
    clip_width: u16,
    clip_height: u16,
    field_8: u16,
    field_a: u16,
    width: u16,
    height: u16,
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
struct Header {
    inner_x: u16,
    inner_y: u16,
    width: u16,
    height: u16,
    version: u16,
    flags: u16,
    bpp: u16,
    mode: u16,
    #[skip_pack_if(self.version != 3)]
    #[skip_unpack_if(version != 3)]
    #[pvec(u32)]
    clips: Vec<Clip>,
}

pub struct CrxImage {
    header: Header,
    color_type: ImageColorType,
    data: Vec<u8>,
}

impl std::fmt::Debug for CrxImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CrxImage")
            .field("header", &self.header)
            .field("color_type", &self.color_type)
            .field("data_length", &self.data.len())
            .finish()
    }
}

impl CrxImage {
    pub fn new<T: Read + Seek>(data: T, _config: &ExtraConfig) -> Result<Self> {
        let mut reader = data;
        let mut magic = [0; 4];
        reader.read_exact(&mut magic)?;
        if magic != *b"CRXG" {
            return Err(anyhow::anyhow!("Invalid CRX image magic"));
        }
        let header: Header = reader.read_struct(false, Encoding::Utf8)?;
        if header.version < 2 || header.version > 3 {
            return Err(anyhow::anyhow!(
                "Unsupported CRX version: {}",
                header.version
            ));
        }
        let color_type = if header.bpp == 0 {
            ImageColorType::Bgr
        } else if header.bpp == 1 {
            ImageColorType::Bgra
        } else {
            return Err(anyhow::anyhow!("Unsupported CRX bpp: {}", header.bpp));
        };
        let compressed_size = if (header.flags & 0x10) == 0 {
            let len = reader.stream_length()?;
            (len - reader.stream_position()?) as u32
        } else {
            reader.read_u32()?
        };
        let compressed_data = reader.read_exact_vec(compressed_size as usize)?;
        let uncompessed = if compressed_data.starts_with(&[0x28, 0xb5, 0x2f, 0xfd]) {
            let mut decoder = zstd::Decoder::new(MemReaderRef::new(&compressed_data))?;
            let mut decompressed_data = Vec::new();
            decoder.read_to_end(&mut decompressed_data)?;
            decompressed_data
        } else {
            let mut decompressed_data = Vec::new();
            flate2::read::ZlibDecoder::new(MemReaderRef::new(&compressed_data))
                .read_to_end(&mut decompressed_data)?;
            decompressed_data
        };
        Ok(CrxImage {
            header,
            color_type,
            data: uncompessed,
        })
    }

     fn decode_row0(
        dst: &mut Vec<u8>,
        mut dst_p: usize,
        src: &[u8],
        mut src_p: usize,
        width: u16,
        pixel_size: u8,
    ) -> Result<usize> {
        let mut prev_p = dst_p;
        for _ in 0..pixel_size {
            dst[dst_p] = src[src_p];
            dst_p += 1;
            src_p += 1;
        }
        let remaining = width - 1;
        for _ in 0..remaining {
            for _ in 0..pixel_size {
                dst[dst_p] = src[src_p].overflowing_add(dst[prev_p]).0;
                dst_p += 1;
                src_p += 1;
                prev_p += 1;
            }
        }
        Ok(src_p)
    }

    fn decode_row1(
        dst: &mut Vec<u8>,
        mut dst_p: usize,
        src: &[u8],
        mut src_p: usize,
        width: u16,
        pixel_size: u8,
        mut prev_row_p: usize,
    ) -> Result<usize> {
        for _ in 0..width {
            for _ in 0..pixel_size {
                dst[dst_p] = src[src_p].overflowing_add(dst[prev_row_p]).0;
                dst_p += 1;
                src_p += 1;
                prev_row_p += 1;
            }
        }
        Ok(src_p)
    }

    fn decode_row2(
        dst: &mut Vec<u8>,
        mut dst_p: usize,
        src: &[u8],
        mut src_p: usize,
        width: u16,
        pixel_size: u8,
        mut prev_row_p: usize,
    ) -> Result<usize> {
        for _ in 0..pixel_size {
            dst[dst_p] = src[src_p];
            dst_p += 1;
            src_p += 1;
        }
        let remaining = width - 1;
        for _ in 0..remaining {
            for _ in 0..pixel_size {
                dst[dst_p] = src[src_p].overflowing_add(dst[prev_row_p]).0;
                dst_p += 1;
                src_p += 1;
                prev_row_p += 1;
            }
        }
        Ok(src_p)
    }

    fn decode_row3(
        dst: &mut Vec<u8>,
        mut dst_p: usize,
        src: &[u8],
        mut src_p: usize,
        width: u16,
        pixel_size: u8,
        mut prev_row_p: usize,
    ) -> Result<usize> {
        let count = width - 1;
        prev_row_p += pixel_size as usize;
        for _ in 0..count {
            for _ in 0..pixel_size {
                dst[dst_p] = src[src_p].overflowing_add(dst[prev_row_p]).0;
                dst_p += 1;
                src_p += 1;
                prev_row_p += 1;
            }
        }
        for _ in 0..pixel_size {
            dst[dst_p] = src[src_p];
            dst_p += 1;
            src_p += 1;
        }
        Ok(src_p)
    }

    fn decode_row4(
        dst: &mut Vec<u8>,
        dst_p: usize,
        src: &[u8],
        mut src_p: usize,
        width: u16,
        pixel_size: u8,
    ) -> Result<usize> {
        for offset in 0..pixel_size {
            let mut dst_c = dst_p + offset as usize;
            let mut remaining = width;
            let value = src[src_p];
            src_p += 1;
            dst[dst_c] = value;
            dst_c += pixel_size as usize;
            remaining -= 1;
            if remaining == 0 {
                continue;
            }
            if value == src[src_p] {
                src_p += 1;
                let count = src[src_p] as u16;
                src_p += 1;
                remaining -= count;
                for _ in 0..count {
                    dst[dst_c] = value;
                    dst_c += pixel_size as usize;
                }
            }
            while remaining > 0 {
                let value = src[src_p];
                src_p += 1;
                dst[dst_c] = value;
                dst_c += pixel_size as usize;
                remaining -= 1;
                if remaining == 0 {
                    break;
                }
                if value == src[src_p] {
                    src_p += 1;
                    let count = src[src_p] as u16;
                    src_p += 1;
                    remaining -= count;
                    for _ in 0..count {
                        dst[dst_c] = value;
                        dst_c += pixel_size as usize;
                    }
                }
            }
        }
        Ok(src_p)
    }

    fn decode_image(
        dst: &mut Vec<u8>,
        src: &[u8],
        width: u16,
        height: u16,
        pixel_size: u8,
        encode_type: &mut Vec<u8>,
    ) -> Result<()> {
        let mut src_p = 0;
        let mut dst_p = 0;
        let mut prev_row_p = 0;
        for _ in 0..height {
            let data = src[src_p];
            encode_type.push(data);
            src_p += 1;
            match data {
                0 => {
                    src_p = Self::decode_row0(dst, dst_p, src, src_p, width, pixel_size)?;
                }
                1 => {
                    src_p =
                        Self::decode_row1(dst, dst_p, src, src_p, width, pixel_size, prev_row_p)?;
                }
                2 => {
                    src_p =
                        Self::decode_row2(dst, dst_p, src, src_p, width, pixel_size, prev_row_p)?;
                }
                3 => {
                    src_p =
                        Self::decode_row3(dst, dst_p, src, src_p, width, pixel_size, prev_row_p)?;
                }
                4 => {
                    src_p = Self::decode_row4(dst, dst_p, src, src_p, width, pixel_size)?;
                }
                _ => {
                    return Err(anyhow::anyhow!("Invalid row type: {}", data));
                }
            }
            prev_row_p = dst_p;
            dst_p += pixel_size as usize * width as usize;
        }
        Ok(())
    }
}

impl Script for CrxImage {
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
        let data_size = self.color_type.bpp(1) as usize * self.header.width as usize * self.header.height as usize;
        let mut data = vec![0; data_size];
        let mut encode_type = Vec::new();
        Self::decode_image(
            &mut data,
            &self.data,
            self.header.width,
            self.header.height,
            self.color_type.bpp(1) as u8,
            &mut encode_type,
        )?;
        if self.color_type.bpp(1) == 4 && self.header.mode != 1 {
            let alpha_flip = if self.header.mode == 2 {
                0
            } else {
                0xFF
            };
            for i in (0..data_size).step_by(4) {
                let a = data[i];
                let b = data[i + 1];
                let g = data[i + 2];
                let r = data[i + 3];
                data[i] = b;
                data[i + 1] = g;
                data[i + 2] = r;
                data[i + 3] = a ^ alpha_flip;
            }
        }
        Ok(ImageData {
            width: self.header.width as u32,
            height: self.header.height as u32,
            depth: 8,
            color_type: self.color_type,
            data,
        })
    }
}
