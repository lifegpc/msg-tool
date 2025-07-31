use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::img::*;
use crate::utils::struct_pack::*;
use anyhow::Result;
use clap::ValueEnum;
use clap::builder::PossibleValue;
use msg_tool_macro::*;
use std::io::{Read, Seek, Write};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircusCrxMode {
    Fixed(u8),
    Auto,
    Origin,
    Best,
}

impl CircusCrxMode {
    pub fn for_importing(&self) -> Self {
        match self {
            CircusCrxMode::Auto => CircusCrxMode::Origin,
            _ => *self,
        }
    }

    pub fn for_creating(&self) -> Self {
        match self {
            CircusCrxMode::Auto => CircusCrxMode::Best,
            CircusCrxMode::Origin => CircusCrxMode::Best,
            _ => *self,
        }
    }

    pub fn is_best(&self) -> bool {
        matches!(self, CircusCrxMode::Best)
    }

    pub fn is_origin(&self) -> bool {
        matches!(self, CircusCrxMode::Origin)
    }
}

impl ValueEnum for CircusCrxMode {
    fn value_variants<'a>() -> &'a [Self] {
        &[
            CircusCrxMode::Fixed(0),
            CircusCrxMode::Fixed(1),
            CircusCrxMode::Fixed(2),
            CircusCrxMode::Fixed(3),
            CircusCrxMode::Fixed(4),
            CircusCrxMode::Auto,
            CircusCrxMode::Origin,
            CircusCrxMode::Best,
        ]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        Some(match self {
            CircusCrxMode::Fixed(0) => PossibleValue::new("0").help("Row type 0"),
            CircusCrxMode::Fixed(1) => PossibleValue::new("1").help("Row type 1"),
            CircusCrxMode::Fixed(2) => PossibleValue::new("2").help("Row type 2"),
            CircusCrxMode::Fixed(3) => PossibleValue::new("3").help("Row type 3"),
            CircusCrxMode::Fixed(4) => PossibleValue::new("4").help("Row type 4"),
            CircusCrxMode::Auto => PossibleValue::new("auto")
                .help("When importing, use origin mode, otherwise use best mode."),
            CircusCrxMode::Origin => PossibleValue::new("origin")
                .help("Use origin mode for importing. When creating, fallback to best mode."),
            CircusCrxMode::Best => PossibleValue::new("best").help("Try to use the best mode."),
            _ => return None,
        })
    }
}

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
        _archive: Option<&Box<dyn Script>>,
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

    fn can_create_image_file(&self) -> bool {
        true
    }

    fn create_image_file<'a>(
        &'a self,
        data: ImageData,
        writer: Box<dyn WriteSeek + 'a>,
        options: &ExtraConfig,
    ) -> Result<()> {
        CrxImage::create_image(data, writer, options)
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
    compress_level: u32,
    keep_original_bpp: bool,
    zstd: bool,
    zstd_compression_level: i32,
    row_type: CircusCrxMode,
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
    pub fn new<T: Read + Seek>(data: T, config: &ExtraConfig) -> Result<Self> {
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
            compress_level: config.zlib_compression_level,
            keep_original_bpp: config.circus_crx_keep_original_bpp,
            zstd: config.circus_crx_zstd,
            zstd_compression_level: config.zstd_compression_level,
            row_type: config.circus_crx_mode.for_importing(),
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

    fn encode_row0(dst: &mut Vec<u8>, src: &[u8], width: u16, pixel_size: u8, y: u16) {
        let pixel_size = pixel_size as usize;
        let mut src_p = y as usize * width as usize * pixel_size;
        for _ in 0..pixel_size {
            dst.push(src[src_p]);
            src_p += 1;
        }
        for _ in 1..width {
            for _ in 0..pixel_size {
                dst.push(src[src_p].wrapping_sub(src[src_p - pixel_size]));
                src_p += 1;
            }
        }
    }

    fn encode_row1(dst: &mut Vec<u8>, src: &[u8], width: u16, pixel_size: u8, y: u16) {
        let pixel_size = pixel_size as usize;
        let mut src_p = y as usize * width as usize * pixel_size;
        let mut prev_row_p = (y as usize - 1) * width as usize * pixel_size;
        for _ in 0..width {
            for _ in 0..pixel_size {
                dst.push(src[src_p].wrapping_sub(src[prev_row_p]));
                src_p += 1;
                prev_row_p += 1;
            }
        }
    }

    fn encode_row2(dst: &mut Vec<u8>, src: &[u8], width: u16, pixel_size: u8, y: u16) {
        let pixel_size = pixel_size as usize;
        let mut src_p = y as usize * width as usize * pixel_size;
        let mut prev_row_p = (y as usize - 1) * width as usize * pixel_size;
        for _ in 0..pixel_size {
            dst.push(src[src_p]);
            src_p += 1;
        }
        for _ in 1..width {
            for _ in 0..pixel_size {
                dst.push(src[src_p].wrapping_sub(src[prev_row_p]));
                src_p += 1;
                prev_row_p += 1;
            }
        }
    }

    fn encode_row3(dst: &mut Vec<u8>, src: &[u8], width: u16, pixel_size: u8, y: u16) {
        let pixel_size = pixel_size as usize;
        let mut src_p = y as usize * width as usize * pixel_size;
        let mut prev_row_p = (y as usize - 1) * width as usize * pixel_size + pixel_size;
        for _ in 0..width - 1 {
            for _ in 0..pixel_size {
                dst.push(src[src_p].wrapping_sub(src[prev_row_p]));
                src_p += 1;
                prev_row_p += 1;
            }
        }
        for _ in 0..pixel_size {
            dst.push(src[src_p]);
            src_p += 1;
        }
    }

    fn encode_row4(dst: &mut Vec<u8>, src: &[u8], width: u16, pixel_size: u8, y: u16) {
        let pixel_size = pixel_size as usize;
        let src_p = y as usize * width as usize * pixel_size;
        for offset in 0..pixel_size {
            let mut src_c = src_p + offset;
            let mut remaining = width;
            let value = src[src_c];
            src_c += pixel_size;
            dst.push(value);
            remaining -= 1;
            if remaining == 0 {
                continue;
            }
            let mut count = 0;
            loop {
                if count as u16 >= remaining || count >= 255 || src[src_c] != value {
                    break;
                }
                src_c += pixel_size;
                count += 1;
            }
            if count > 0 {
                dst.push(value);
                dst.push(count);
                remaining -= count as u16;
            }
            while remaining > 0 {
                let value = src[src_c];
                src_c += pixel_size;
                dst.push(value);
                remaining -= 1;
                if remaining == 0 {
                    break;
                }
                let mut count = 0;
                loop {
                    if count as u16 >= remaining || count >= 255 || src[src_c] != value {
                        break;
                    }
                    src_c += pixel_size;
                    count += 1;
                }
                if count > 0 {
                    dst.push(value);
                    dst.push(count);
                    remaining -= count as u16;
                }
            }
        }
    }

    fn encode_row_best(
        dst: &mut Vec<u8>,
        src: &[u8],
        width: u16,
        pixel_size: u8,
        y: u16,
    ) -> Result<()> {
        let mut buf = Vec::with_capacity(width as usize * pixel_size as usize);
        Self::encode_row0(&mut buf, src, width, pixel_size, y);
        let mut compressed_len = {
            let mut encoder =
                flate2::write::ZlibEncoder::new(MemWriter::new(), flate2::Compression::fast());
            encoder.write_all(&buf)?;
            let compressed_data = encoder.finish()?;
            compressed_data.into_inner().len()
        };
        let mut buf_row_type = 0;
        for row_type in 1..5u8 {
            if y == 0 && row_type < 4 {
                continue;
            }
            let mut newbuf = Vec::with_capacity(width as usize * pixel_size as usize);
            match row_type {
                1 => Self::encode_row1(&mut newbuf, src, width, pixel_size, y),
                2 => Self::encode_row2(&mut newbuf, src, width, pixel_size, y),
                3 => Self::encode_row3(&mut newbuf, src, width, pixel_size, y),
                4 => Self::encode_row4(&mut newbuf, src, width, pixel_size, y),
                _ => return Err(anyhow::anyhow!("Invalid row type: {}", row_type)),
            };
            let new_compressed_len = {
                let mut encoder =
                    flate2::write::ZlibEncoder::new(MemWriter::new(), flate2::Compression::fast());
                encoder.write_all(&newbuf)?;
                let compressed_data = encoder.finish()?;
                compressed_data.into_inner().len()
            };
            if new_compressed_len < compressed_len {
                compressed_len = new_compressed_len;
                buf = newbuf;
                buf_row_type = row_type;
            }
        }
        dst.push(buf_row_type);
        dst.extend_from_slice(&buf);
        Ok(())
    }

    fn encode_image_best(src: &[u8], width: u16, height: u16, pixel_size: u8) -> Result<Vec<u8>> {
        let size = width as usize * height as usize * pixel_size as usize + height as usize;
        let mut dst = Vec::with_capacity(size);
        for y in 0..height {
            Self::encode_row_best(&mut dst, src, width, pixel_size, y)?;
        }
        Ok(dst)
    }

    fn encode_image_fixed(
        src: &[u8],
        width: u16,
        height: u16,
        pixel_size: u8,
        row_type: u8,
    ) -> Result<Vec<u8>> {
        let size = width as usize * height as usize * pixel_size as usize + height as usize;
        let mut dst = Vec::with_capacity(size);
        for y in 0..height {
            let row_type = if y == 0 && row_type != 0 && row_type != 4 {
                0
            } else {
                row_type
            };
            dst.push(row_type);
            match row_type {
                0 => Self::encode_row0(&mut dst, src, width, pixel_size, y),
                1 => Self::encode_row1(&mut dst, src, width, pixel_size, y),
                2 => Self::encode_row2(&mut dst, src, width, pixel_size, y),
                3 => Self::encode_row3(&mut dst, src, width, pixel_size, y),
                4 => Self::encode_row4(&mut dst, src, width, pixel_size, y),
                _ => return Err(anyhow::anyhow!("Invalid row type: {}", row_type)),
            };
        }
        Ok(dst)
    }

    fn encode_image_origin(
        src: &[u8],
        width: u16,
        height: u16,
        pixel_size: u8,
        row_type: &[u8],
    ) -> Result<Vec<u8>> {
        if row_type.len() != height as usize {
            return Err(anyhow::anyhow!("Row type length does not match height"));
        }
        let size = width as usize * height as usize * pixel_size as usize + height as usize;
        let mut dst = Vec::with_capacity(size);
        for y in 0..height {
            let row_type = row_type[y as usize];
            dst.push(row_type);
            match row_type {
                0 => Self::encode_row0(&mut dst, src, width, pixel_size, y),
                1 => Self::encode_row1(&mut dst, src, width, pixel_size, y),
                2 => Self::encode_row2(&mut dst, src, width, pixel_size, y),
                3 => Self::encode_row3(&mut dst, src, width, pixel_size, y),
                4 => Self::encode_row4(&mut dst, src, width, pixel_size, y),
                _ => return Err(anyhow::anyhow!("Invalid row type: {}", row_type)),
            };
        }
        Ok(dst)
    }

    pub fn create_image<T: Write + Seek>(
        mut data: ImageData,
        mut writer: T,
        config: &ExtraConfig,
    ) -> Result<()> {
        let header = Header {
            inner_x: 0,
            inner_y: 0,
            width: data.width as u16,
            height: data.height as u16,
            version: 2,
            flags: 0x10, // Force add compressed data length
            bpp: match data.color_type {
                ImageColorType::Bgr => 0,
                ImageColorType::Bgra => 1,
                ImageColorType::Rgb => {
                    convert_rgb_to_bgr(&mut data)?;
                    0
                }
                ImageColorType::Rgba => {
                    convert_rgba_to_bgra(&mut data)?;
                    1
                }
                _ => {
                    return Err(anyhow::anyhow!(
                        "Unsupported color type: {:?}",
                        data.color_type
                    ));
                }
            },
            mode: 0,
            clips: Vec::new(),
        };
        let pixel_size = data.color_type.bpp(1) as u8;
        if data.color_type == ImageColorType::Bgra && header.mode != 1 {
            let alpha_flip = if header.mode == 2 { 0 } else { 0xFF };
            for i in (0..data.data.len()).step_by(4) {
                let b = data.data[i];
                let g = data.data[i + 1];
                let r = data.data[i + 2];
                let a = data.data[i + 3];
                data.data[i] = a ^ alpha_flip;
                data.data[i + 1] = b;
                data.data[i + 2] = g;
                data.data[i + 3] = r;
            }
        }
        let mode = config.circus_crx_mode.for_creating();
        let encoded = if mode.is_best() {
            Self::encode_image_best(&data.data, header.width, header.height, pixel_size)?
        } else if let CircusCrxMode::Fixed(mode) = mode {
            Self::encode_image_fixed(&data.data, header.width, header.height, pixel_size, mode)?
        } else {
            return Err(anyhow::anyhow!(
                "Unsupported row type for creating: {:?}",
                mode
            ));
        };
        let compressed = if config.circus_crx_zstd {
            let mut encoder = zstd::Encoder::new(MemWriter::new(), config.zstd_compression_level)?;
            encoder.write_all(&encoded)?;
            let compressed_data = encoder.finish()?;
            compressed_data.into_inner()
        } else {
            let mut encoder = flate2::write::ZlibEncoder::new(
                MemWriter::new(),
                flate2::Compression::new(config.zlib_compression_level),
            );
            encoder.write_all(&encoded)?;
            let compressed_data = encoder.finish()?;
            compressed_data.into_inner()
        };
        writer.write_all(b"CRXG")?;
        header.pack(&mut writer, false, Encoding::Utf8)?;
        writer.write_u32(compressed.len() as u32)?;
        writer.write_all(&compressed)?;
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
        let data_size = self.color_type.bpp(1) as usize
            * self.header.width as usize
            * self.header.height as usize;
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
            let alpha_flip = if self.header.mode == 2 { 0 } else { 0xFF };
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

    fn import_image<'a>(
        &'a self,
        mut data: ImageData,
        mut file: Box<dyn WriteSeek + 'a>,
    ) -> Result<()> {
        let mut color_type = match data.color_type {
            ImageColorType::Bgr => ImageColorType::Bgr,
            ImageColorType::Bgra => ImageColorType::Bgra,
            ImageColorType::Rgb => {
                convert_rgb_to_bgr(&mut data)?;
                ImageColorType::Bgr
            }
            ImageColorType::Rgba => {
                convert_rgba_to_bgra(&mut data)?;
                ImageColorType::Bgra
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Unsupported color type: {:?}",
                    data.color_type
                ));
            }
        };
        if data.width != self.header.width as u32 {
            return Err(anyhow::anyhow!(
                "Image width does not match: expected {}, got {}",
                self.header.width,
                data.width
            ));
        }
        if data.height != self.header.height as u32 {
            return Err(anyhow::anyhow!(
                "Image height does not match: expected {}, got {}",
                self.header.height,
                data.height
            ));
        }
        if data.depth != 8 {
            return Err(anyhow::anyhow!("Image depth must be 8, got {}", data.depth));
        }
        if data.color_type != self.color_type && self.keep_original_bpp {
            if self.color_type == ImageColorType::Bgr {
                convert_bgra_to_bgr(&mut data)?;
            } else if self.color_type == ImageColorType::Bgra {
                convert_bgr_to_bgra(&mut data)?;
            } else {
                return Err(anyhow::anyhow!(
                    "Unsupported color type for import: {:?}",
                    self.color_type
                ));
            }
            color_type = self.color_type;
        }
        let mut new_header = self.header.clone();
        new_header.bpp = match color_type {
            ImageColorType::Bgr => 0,
            ImageColorType::Bgra => 1,
            _ => return Err(anyhow::anyhow!("Unsupported color type: {:?}", color_type)),
        };
        new_header.flags |= 0x10; // Force add compressed data length
        let pixel_size = color_type.bpp(1) as u8;
        if color_type == ImageColorType::Bgra && self.header.mode != 1 {
            let alpha_flip = if self.header.mode == 2 { 0 } else { 0xFF };
            for i in (0..data.data.len()).step_by(4) {
                let b = data.data[i];
                let g = data.data[i + 1];
                let r = data.data[i + 2];
                let a = data.data[i + 3];
                data.data[i] = a ^ alpha_flip;
                data.data[i + 1] = b;
                data.data[i + 2] = g;
                data.data[i + 3] = r;
            }
        }
        let encoded = if self.row_type.is_origin() {
            let mut row_type = Vec::with_capacity(self.header.height as usize);
            let row_len = self.header.width as usize * self.color_type.bpp(1) as usize + 1;
            let mut cur_pos = 0;
            for _ in 0..self.header.height {
                row_type.push(self.data[cur_pos]);
                cur_pos += row_len;
            }
            Self::encode_image_origin(
                &data.data,
                new_header.width,
                new_header.height,
                pixel_size,
                &row_type,
            )?
        } else if self.row_type.is_best() {
            Self::encode_image_best(&data.data, new_header.width, new_header.height, pixel_size)?
        } else if let CircusCrxMode::Fixed(mode) = self.row_type {
            Self::encode_image_fixed(
                &data.data,
                new_header.width,
                new_header.height,
                pixel_size,
                mode,
            )?
        } else {
            return Err(anyhow::anyhow!(
                "Unsupported row type for import: {:?}",
                self.row_type
            ));
        };
        let compressed = if self.zstd {
            let mut encoder = zstd::Encoder::new(MemWriter::new(), self.zstd_compression_level)?;
            encoder.write_all(&encoded)?;
            let compressed_data = encoder.finish()?;
            compressed_data.into_inner()
        } else {
            let mut encoder = flate2::write::ZlibEncoder::new(
                MemWriter::new(),
                flate2::Compression::new(self.compress_level),
            );
            encoder.write_all(&encoded)?;
            let compressed_data = encoder.finish()?;
            compressed_data.into_inner()
        };
        file.write_all(b"CRXG")?;
        new_header.pack(&mut file, false, Encoding::Utf8)?;
        file.write_u32(compressed.len() as u32)?;
        file.write_all(&compressed)?;
        Ok(())
    }
}
