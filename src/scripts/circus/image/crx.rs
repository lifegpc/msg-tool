//! Circus Image File (.crx)
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::img::*;
use crate::utils::struct_pack::*;
use anyhow::Result;
use clap::ValueEnum;
use clap::builder::PossibleValue;
use msg_tool_macro::*;
use overf::wrapping;
use std::io::{Read, Seek, Write};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Circus CRX Row Encoding Mode
pub enum CircusCrxMode {
    /// Encoding all rows with a fixed type.
    Fixed(u8),
    /// When importing, use origin mode; when creating, use best mode.
    Auto,
    /// Use origin mode for importing; when creating, fallback to best mode.
    Origin,
    /// Try to use the best mode for encoding.
    Best,
}

impl Default for CircusCrxMode {
    fn default() -> Self {
        CircusCrxMode::Auto
    }
}

impl CircusCrxMode {
    /// Returns mode for importing.
    pub fn for_importing(&self) -> Self {
        match self {
            CircusCrxMode::Auto => CircusCrxMode::Origin,
            _ => *self,
        }
    }

    /// Returns mode for creating.
    pub fn for_creating(&self) -> Self {
        match self {
            CircusCrxMode::Auto => CircusCrxMode::Best,
            CircusCrxMode::Origin => CircusCrxMode::Best,
            _ => *self,
        }
    }

    /// Checks if the mode is best.
    pub fn is_best(&self) -> bool {
        matches!(self, CircusCrxMode::Best)
    }

    /// Checks if the mode is origin.
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
/// Circus CRX Image Builder
pub struct CrxImageBuilder {}

impl CrxImageBuilder {
    /// Creates a new instance of `CrxImageBuilder`.
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
        _filename: &str,
        writer: Box<dyn WriteSeek + 'a>,
        options: &ExtraConfig,
    ) -> Result<()> {
        CrxImage::create_image(data, writer, options)
    }
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
struct Clip {
    field_0: u32,
    img_width: u16,
    img_height: u16,
    clip_offset_x: u16,
    clip_offset_y: u16,
    clip_width: u16,
    clip_height: u16,
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

#[derive(Clone, Debug)]
enum CrxImageData {
    RowEncoded(Vec<u8>),
    IndexedV1 {
        pixels: Vec<u8>,
        stride: usize,
        palette: Vec<u8>,
        palette_format: PaletteFormat,
        pixel_depth_bits: usize,
    },
    Direct(Vec<u8>),
}

impl CrxImageData {
    fn is_row_encoded(&self) -> bool {
        matches!(self, CrxImageData::RowEncoded(_))
    }
}

/// Circus CRX Image
pub struct CrxImage {
    header: Header,
    color_type: ImageColorType,
    data: CrxImageData,
    compress_level: u32,
    keep_original_bpp: bool,
    zstd: bool,
    zstd_compression_level: i32,
    row_type: CircusCrxMode,
    canvas: bool,
}

impl std::fmt::Debug for CrxImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let data_info = match &self.data {
            CrxImageData::RowEncoded(buf) => format!("row-encoded({})", buf.len()),
            CrxImageData::IndexedV1 { pixels, .. } => {
                format!("indexed-v1({})", pixels.len())
            }
            CrxImageData::Direct(buf) => format!("direct({})", buf.len()),
        };
        f.debug_struct("CrxImage")
            .field("header", &self.header)
            .field("color_type", &self.color_type)
            .field("data", &data_info)
            .finish()
    }
}

impl CrxImage {
    /// Creates a new `CrxImage` from the given data and configuration.
    ///
    /// * `data` - The reader to read the CRX image from.
    /// * `config` - Extra configuration options.
    pub fn new<T: Read + Seek>(data: T, config: &ExtraConfig) -> Result<Self> {
        let mut reader = data;
        let mut magic = [0; 4];
        reader.read_exact(&mut magic)?;
        if magic != *b"CRXG" {
            return Err(anyhow::anyhow!("Invalid CRX image magic"));
        }
        let header: Header = reader.read_struct(false, Encoding::Utf8, &None)?;
        if header.version == 0 || header.version > 3 {
            return Err(anyhow::anyhow!(
                "Unsupported CRX version: {}",
                header.version
            ));
        }

        let (color_type, data) = if header.version == 1 {
            let width = usize::from(header.width);
            let height = usize::from(header.height);
            if width == 0 || height == 0 {
                return Err(anyhow::anyhow!("CRX v1 image has zero dimensions"));
            }

            let bits_per_pixel = match header.bpp {
                0 => 24usize,
                1 => 32usize,
                _ => 8usize,
            };
            if bits_per_pixel % 8 != 0 {
                return Err(anyhow::anyhow!(
                    "Unsupported bits per pixel {} for CRX v1",
                    bits_per_pixel
                ));
            }
            let pixel_size = bits_per_pixel / 8;
            if pixel_size == 0 {
                return Err(anyhow::anyhow!("Invalid pixel size for CRX v1 image"));
            }

            let row_bytes = width
                .checked_mul(pixel_size)
                .ok_or_else(|| anyhow::anyhow!("CRX v1 row size overflow"))?;
            let stride = (row_bytes
                .checked_add(3)
                .ok_or_else(|| anyhow::anyhow!("CRX v1 stride overflow"))?)
                & !3usize;
            let output_len = stride
                .checked_mul(height)
                .ok_or_else(|| anyhow::anyhow!("CRX v1 buffer size overflow"))?;

            let palette = if bits_per_pixel == 8 {
                let raw_colors = usize::from(header.bpp);
                Some((
                    Self::read_v1_palette(&mut reader, raw_colors)?,
                    PaletteFormat::Rgb,
                ))
            } else {
                None
            };

            if (header.flags & 0x10) != 0 {
                reader.read_u32()?; // stored compressed size, ignored for v1
            }

            let pixels = Self::unpack_v1(&mut reader, output_len)?;

            if let Some((palette, palette_format)) = palette {
                let data = CrxImageData::IndexedV1 {
                    pixels,
                    stride,
                    palette,
                    palette_format,
                    pixel_depth_bits: bits_per_pixel,
                };
                (ImageColorType::Bgr, data)
            } else {
                let mut trimmed = Vec::with_capacity(
                    row_bytes
                        .checked_mul(height)
                        .ok_or_else(|| anyhow::anyhow!("CRX v1 buffer size overflow"))?,
                );
                for row in 0..height {
                    let start = row
                        .checked_mul(stride)
                        .ok_or_else(|| anyhow::anyhow!("CRX v1 row offset overflow"))?;
                    let end = start
                        .checked_add(row_bytes)
                        .ok_or_else(|| anyhow::anyhow!("CRX v1 row slice overflow"))?;
                    if end > pixels.len() {
                        return Err(anyhow::anyhow!(
                            "CRX v1 image data is shorter than expected"
                        ));
                    }
                    trimmed.extend_from_slice(&pixels[start..end]);
                }
                let color_type = match bits_per_pixel {
                    24 => ImageColorType::Bgr,
                    32 => ImageColorType::Bgra,
                    _ => ImageColorType::Bgr,
                };
                (color_type, CrxImageData::Direct(trimmed))
            }
        } else {
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
            let uncompressed = if compressed_data.starts_with(&[0x28, 0xb5, 0x2f, 0xfd]) {
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
            (color_type, CrxImageData::RowEncoded(uncompressed))
        };

        Ok(CrxImage {
            header,
            color_type,
            data,
            compress_level: config.zlib_compression_level,
            keep_original_bpp: config.circus_crx_keep_original_bpp,
            zstd: config.circus_crx_zstd,
            zstd_compression_level: config.zstd_compression_level,
            row_type: config.circus_crx_mode.for_importing(),
            canvas: config.circus_crx_canvas,
        })
    }

    /// Whether to draw image on canvas if canvas's width and height are specified in image.
    pub fn with_canvas(mut self, canvas: bool) -> Self {
        self.canvas = canvas;
        self
    }

    /// Draws another image on this image.
    ///
    /// Returns a new `ImageData` with the combined image.
    pub fn draw_diff(&self, diff: &Self) -> Result<ImageData> {
        let base_header = &self.header;
        let diff_header = &diff.header;
        let (img_width, img_height) =
            if base_header.clips.is_empty() && diff_header.clips.is_empty() {
                (
                    (base_header.width + base_header.inner_x)
                        .max(diff_header.width + diff_header.inner_x),
                    (base_header.height + base_header.inner_y)
                        .max(diff_header.height + diff_header.inner_y),
                )
            } else {
                if base_header.clips.is_empty() {
                    let clip = &diff_header.clips[0];
                    (clip.img_width, clip.img_height)
                } else {
                    let clip = &base_header.clips[0];
                    (clip.img_width, clip.img_height)
                }
            };
        let base = self.export_image()?;
        let mut nw = draw_on_canvas(
            base,
            img_width as u32,
            img_height as u32,
            base_header.inner_x as u32,
            base_header.inner_y as u32,
        )?;
        draw_on_img(
            &mut nw,
            &diff.export_image()?,
            diff_header.inner_x as u32,
            diff_header.inner_y as u32,
        )?;
        Ok(nw)
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

    fn read_v1_palette<T: Read>(reader: &mut T, raw_colors: usize) -> Result<Vec<u8>> {
        if raw_colors == 0 {
            return Err(anyhow::anyhow!("CRX v1 palette has zero colors"));
        }
        let color_size = if raw_colors == 0x0102 { 4usize } else { 3usize };
        let mut colors = raw_colors;
        if colors > 0x0100 {
            colors = 0x0100;
        }
        let palette_size = colors
            .checked_mul(color_size)
            .ok_or_else(|| anyhow::anyhow!("CRX v1 palette size overflow"))?;
        if palette_size == 0 {
            return Err(anyhow::anyhow!("CRX v1 palette size is zero"));
        }
        let mut palette_raw = vec![0u8; palette_size];
        reader.read_exact(&mut palette_raw)?;
        let mut palette = Vec::with_capacity(colors * 3);
        let mut pos = 0usize;
        while pos < palette_raw.len() {
            let r = palette_raw[pos];
            let mut g = palette_raw[pos + 1];
            let b = palette_raw[pos + 2];
            if b == 0xFF && g == 0x00 && r == 0xFF {
                g = 0xFF;
            }
            palette.push(r);
            palette.push(g);
            palette.push(b);
            pos += color_size;
        }
        Ok(palette)
    }

    fn unpack_v1<T: Read>(reader: &mut T, output_len: usize) -> Result<Vec<u8>> {
        const WINDOW_SIZE: usize = 0x10000;
        const WINDOW_MASK: usize = WINDOW_SIZE - 1;
        let mut window = vec![0u8; WINDOW_SIZE];
        let mut win_pos: usize = 0;
        let mut dst = vec![0u8; output_len];
        let mut dst_pos = 0usize;
        let mut flag: u16 = 0;
        while dst_pos < output_len {
            flag >>= 1;
            if (flag & 0x100) == 0 {
                let next = reader.read_u8()? as u16;
                flag = next | 0xFF00;
            }
            if (flag & 1) != 0 {
                let byte = reader.read_u8()?;
                window[win_pos] = byte;
                win_pos = (win_pos + 1) & WINDOW_MASK;
                dst[dst_pos] = byte;
                dst_pos += 1;
            } else {
                let control = reader.read_u8()?;
                let (count, offset_value) = if control >= 0xC0 {
                    let next = reader.read_u8()? as usize;
                    let offset = (((control as usize) & 0x03) << 8) | next;
                    let count = 4 + (((control as usize) >> 2) & 0x0F);
                    (count, offset)
                } else if (control & 0x80) != 0 {
                    let mut offset = (control & 0x1F) as usize;
                    let count = 2 + (((control as usize) >> 5) & 0x03);
                    if offset == 0 {
                        offset = reader.read_u8()? as usize;
                    }
                    (count, offset)
                } else if control == 0x7F {
                    let count = 2 + reader.read_u16()? as usize;
                    let offset = reader.read_u16()? as usize;
                    (count, offset)
                } else {
                    let offset = reader.read_u16()? as usize;
                    let count = control as usize + 4;
                    (count, offset)
                };

                let mut offset_pos = (win_pos.wrapping_sub(offset_value)) & WINDOW_MASK;
                for _ in 0..count {
                    if dst_pos >= output_len {
                        break;
                    }
                    let value = window[offset_pos];
                    offset_pos = (offset_pos + 1) & WINDOW_MASK;
                    window[win_pos] = value;
                    win_pos = (win_pos + 1) & WINDOW_MASK;
                    dst[dst_pos] = value;
                    dst_pos += 1;
                }
            }
        }
        Ok(dst)
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

    /// Creates a CRX image file from the given image data and writes it to the specified writer.
    ///
    /// * `data` - The input image data.
    /// * `writer` - The writer to write the CRX image to.
    /// * `config` - Extra configuration options.
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
        header.pack(&mut writer, false, Encoding::Utf8, &None)?;
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
        let width = usize::from(self.header.width);
        let height = usize::from(self.header.height);
        let mut img = match &self.data {
            CrxImageData::RowEncoded(encoded) => {
                let pixel_size = self.color_type.bpp(1) as usize;
                let row_bytes = pixel_size
                    .checked_mul(width)
                    .ok_or_else(|| anyhow::anyhow!("Image row size overflow"))?;
                let data_size = row_bytes
                    .checked_mul(height)
                    .ok_or_else(|| anyhow::anyhow!("Image buffer size overflow"))?;
                let mut data = vec![0u8; data_size];
                let mut encode_type = Vec::with_capacity(height);
                Self::decode_image(
                    &mut data,
                    encoded,
                    self.header.width,
                    self.header.height,
                    self.color_type.bpp(1) as u8,
                    &mut encode_type,
                )?;
                if self.color_type.bpp(1) == 4 && self.header.mode != 1 {
                    let alpha_flip = if self.header.mode == 2 { 0 } else { 0xFF };
                    for chunk in data.chunks_mut(4) {
                        let a = chunk[0];
                        let b = chunk[1];
                        let g = chunk[2];
                        let r = chunk[3];
                        chunk[0] = b;
                        chunk[1] = g;
                        chunk[2] = r;
                        chunk[3] = a ^ alpha_flip;
                    }
                }
                ImageData {
                    width: self.header.width as u32,
                    height: self.header.height as u32,
                    depth: 8,
                    color_type: self.color_type,
                    data,
                }
            }
            CrxImageData::Direct(pixels) => {
                let mut data = pixels.clone();
                if self.color_type == ImageColorType::Bgra && self.header.mode != 1 {
                    let alpha_flip = if self.header.mode == 2 { 0 } else { 0xFF };
                    for chunk in data.chunks_mut(4) {
                        let a = chunk[0];
                        let b = chunk[1];
                        let g = chunk[2];
                        let r = chunk[3];
                        chunk[0] = b;
                        chunk[1] = g;
                        chunk[2] = r;
                        chunk[3] = a ^ alpha_flip;
                    }
                }
                ImageData {
                    width: self.header.width as u32,
                    height: self.header.height as u32,
                    depth: 8,
                    color_type: self.color_type,
                    data,
                }
            }
            CrxImageData::IndexedV1 {
                pixels,
                stride,
                palette,
                palette_format,
                pixel_depth_bits,
            } => {
                let total_pixels = width
                    .checked_mul(height)
                    .ok_or_else(|| anyhow::anyhow!("Image dimensions overflow"))?;
                let mut indexed = Vec::with_capacity(total_pixels);
                for row in 0..height {
                    let start = row
                        .checked_mul(*stride)
                        .ok_or_else(|| anyhow::anyhow!("Row offset overflow"))?;
                    let end = start
                        .checked_add(width)
                        .ok_or_else(|| anyhow::anyhow!("Row slice overflow"))?;
                    if end > pixels.len() {
                        return Err(anyhow::anyhow!("CRX v1 indexed data is truncated"));
                    }
                    indexed.extend_from_slice(&pixels[start..end]);
                }
                let image = convert_index_palette_to_normal_bitmap(
                    &indexed,
                    *pixel_depth_bits,
                    palette,
                    *palette_format,
                    width,
                    height,
                )?;
                image
            }
        };

        if self.canvas {
            let (img_width, img_height) = if self.header.clips.is_empty() {
                (self.header.width as u32, self.header.height as u32)
            } else {
                let clip = &self.header.clips[0];
                (clip.img_width as u32, clip.img_height as u32)
            };
            img = draw_on_canvas(
                img,
                img_width,
                img_height,
                self.header.inner_x as u32,
                self.header.inner_y as u32,
            )?;
        }
        Ok(img)
    }

    fn import_image<'a>(
        &'a self,
        mut data: ImageData,
        _filename: &str,
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
        if new_header.version == 1 {
            new_header.version = 2; // Upgrade to version 2
        }
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
        let encoded = if self.row_type.is_origin() && self.data.is_row_encoded() {
            let mut row_type = Vec::with_capacity(self.header.height as usize);
            let pixel_size_bytes = self.color_type.bpp(1) as usize;
            let row_len = pixel_size_bytes
                .checked_mul(self.header.width as usize)
                .ok_or_else(|| anyhow::anyhow!("Row length overflow"))?
                .checked_add(1)
                .ok_or_else(|| anyhow::anyhow!("Row length overflow"))?;
            let buffer = match &self.data {
                CrxImageData::RowEncoded(buf) => buf,
                _ => {
                    return Err(anyhow::anyhow!(
                        "Original row type information is unavailable"
                    ));
                }
            };
            let mut cur_pos = 0usize;
            for _ in 0..self.header.height {
                if cur_pos >= buffer.len() {
                    return Err(anyhow::anyhow!("Row type offset exceeds buffer length"));
                }
                let cur_row_type = buffer[cur_pos];
                row_type.push(cur_row_type);
                let true_row_len = if cur_row_type < 4 {
                    row_len
                } else {
                    let mut offset = cur_pos + 1;
                    for _ in 0..pixel_size {
                        let mut remaing = self.header.width;
                        let value = buffer[offset];
                        offset += 1;
                        remaing -= 1;
                        if remaing == 0 {
                            continue;
                        }
                        if value == buffer[offset] {
                            offset += 1;
                            let count = buffer[offset] as u16;
                            offset += 1;
                            remaing = remaing
                                .checked_sub(count)
                                .ok_or_else(|| anyhow::anyhow!("Row run-length overflow"))?;
                        }
                        while remaing > 0 {
                            let value = buffer[offset];
                            offset += 1;
                            remaing -= 1;
                            if remaing == 0 {
                                break;
                            }
                            if value == buffer[offset] {
                                offset += 1;
                                let count = buffer[offset] as u16;
                                offset += 1;
                                remaing = remaing
                                    .checked_sub(count)
                                    .ok_or_else(|| anyhow::anyhow!("Row run-length overflow"))?;
                            }
                        }
                    }
                    offset - cur_pos
                };
                cur_pos = cur_pos
                    .checked_add(true_row_len)
                    .ok_or_else(|| anyhow::anyhow!("Row type offset overflow"))?;
            }
            Self::encode_image_origin(
                &data.data,
                new_header.width,
                new_header.height,
                pixel_size,
                &row_type,
            )?
        } else if self.row_type.is_best()
            || (self.row_type.is_origin() && !self.data.is_row_encoded())
        {
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
        new_header.pack(&mut file, false, Encoding::Utf8, &None)?;
        file.write_u32(compressed.len() as u32)?;
        file.write_all(&compressed)?;
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
            wrapping! {
                b += diff_pixel[0];
                g += diff_pixel[1];
                r += diff_pixel[2];
            }
            base.data[base_index] = b;
            base.data[base_index + 1] = g;
            base.data[base_index + 2] = r;
            if bpp == 4 {
                let mut a = base_pixel_orig[3];
                wrapping! {
                    a -= diff_pixel[3];
                }
                base.data[base_index + 3] = a;
            }
        }
    }
    Ok(())
}
