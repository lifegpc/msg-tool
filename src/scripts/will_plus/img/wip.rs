use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::img::*;
use crate::utils::struct_pack::*;
use anyhow::{Context, Result, anyhow};
use msg_tool_macro::*;
use std::convert::TryFrom;
use std::io::{Read, Seek, SeekFrom, Write};
use std::ops::Range;

#[derive(Debug)]
/// Builder for WillPlus WIP images.
pub struct WillPlusWipImageBuilder {}

impl WillPlusWipImageBuilder {
    /// Creates a new `WillPlusWipImageBuilder` instance.
    pub const fn new() -> Self {
        Self {}
    }
}

impl ScriptBuilder for WillPlusWipImageBuilder {
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
        Ok(Box::new(WillPlusWipImage::new(
            MemReader::new(data),
            config,
        )?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["wip", "wi0", "msk", "mos"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::WillPlusWip
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 4 && buf.starts_with(b"WIPF") {
            Some(10)
        } else {
            None
        }
    }

    #[cfg(feature = "image")]
    fn is_image(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, StructPack, StructUnpack)]
struct FrameHeader {
    width: u32,
    height: u32,
    offset_x: u32,
    offset_y: u32,
    _reserved: i32,
    frame_size: u32,
}

#[derive(Debug, Clone)]
struct WillPlusWipFrame {
    index: usize,
    header: FrameHeader,
    data_range: Range<usize>,
    palette_range: Option<Range<usize>>,
}

#[derive(Debug)]
/// WillPlus WIP image reader.
pub struct WillPlusWipImage {
    data: MemReader,
    frames: Vec<WillPlusWipFrame>,
    bpp: u16,
}

impl WillPlusWipImage {
    /// Creates a `WillPlusWipImage` from raw data.
    pub fn new(mut data: MemReader, _config: &ExtraConfig) -> Result<Self> {
        if data.data.len() < 8 {
            return Err(anyhow!("WIP image too small"));
        }
        let mut header = [0u8; 4];
        data.read_exact(&mut header)?;
        if &header != b"WIPF" {
            return Err(anyhow!("Invalid WIP image header"));
        }

        let frame_count = data.read_u16()? as usize;
        if frame_count == 0 {
            return Err(anyhow!("WIP image has no frames"));
        }
        let bpp = data.read_u16()?;
        if bpp != 8 && bpp != 24 {
            return Err(anyhow!("Unsupported WIP bits-per-pixel: {}", bpp));
        }

        let index_size = frame_count
            .checked_mul(0x18)
            .ok_or_else(|| anyhow!("Frame table size overflow"))?;
        let mut data_offset = 8usize
            .checked_add(index_size)
            .ok_or_else(|| anyhow!("Frame data offset overflow"))?;
        if data.data.len() < data_offset {
            return Err(anyhow!("WIP image truncated before frame data"));
        }

        let mut frames = Vec::with_capacity(frame_count);
        for frame_index in 0..frame_count {
            let header_offset = 8 + frame_index * 0x18;
            data.seek(SeekFrom::Start(header_offset as u64))?;
            let header = FrameHeader::unpack(&mut data, false, Encoding::Utf8, &None)
                .with_context(|| format!("Failed to read header for frame {}", frame_index))?;
            let frame_size = usize::try_from(header.frame_size)
                .map_err(|_| anyhow!("Frame {} data size too large", frame_index))?;

            let data_start = data_offset;
            let data_end = data_start
                .checked_add(frame_size)
                .ok_or_else(|| anyhow!("Frame {} data range overflow", frame_index))?;
            if data_end > data.data.len() {
                return Err(anyhow!("Frame {} data exceeds file length", frame_index));
            }

            let (palette_range, next_offset) = if bpp == 8 {
                let palette_start = data_end;
                let palette_end = palette_start
                    .checked_add(0x400)
                    .ok_or_else(|| anyhow!("Frame {} palette range overflow", frame_index))?;
                if palette_end > data.data.len() {
                    return Err(anyhow!("Frame {} palette exceeds file length", frame_index));
                }
                (Some(palette_start..palette_end), palette_end)
            } else {
                (None, data_end)
            };

            frames.push(WillPlusWipFrame {
                index: frame_index,
                header,
                data_range: data_start..data_end,
                palette_range,
            });
            data_offset = next_offset;
        }

        if frames.is_empty() {
            return Err(anyhow!("No valid frames found in WIP image"));
        }

        Ok(WillPlusWipImage { data, frames, bpp })
    }

    fn decode_frame(&self, frame: &WillPlusWipFrame) -> Result<ImageData> {
        let width_usize = usize::try_from(frame.header.width)
            .map_err(|_| anyhow!("Frame {} width is too large", frame.index))?;
        let height_usize = usize::try_from(frame.header.height)
            .map_err(|_| anyhow!("Frame {} height is too large", frame.index))?;
        let plane_size = width_usize
            .checked_mul(height_usize)
            .ok_or_else(|| anyhow!("Frame {} dimensions overflow", frame.index))?;

        let compressed = self
            .data
            .data
            .get(frame.data_range.clone())
            .ok_or_else(|| anyhow!("Frame {} data range is invalid", frame.index))?;

        let expected = match self.bpp {
            8 => plane_size,
            24 => plane_size
                .checked_mul(3)
                .ok_or_else(|| anyhow!("Frame {} buffer size overflow for 24bpp", frame.index))?,
            _ => return Err(anyhow!("Unsupported bits-per-pixel: {}", self.bpp)),
        };

        let decoded = lzss_decompress(compressed, expected)
            .with_context(|| format!("Failed to decompress frame {}", frame.index))?;

        match self.bpp {
            24 => {
                let required = plane_size
                    .checked_mul(3)
                    .ok_or_else(|| anyhow!("Frame {} plane size overflow", frame.index))?;
                if decoded.len() < required {
                    return Err(anyhow!(
                        "Frame {} decompressed data too short: {} < {}",
                        frame.index,
                        decoded.len(),
                        required
                    ));
                }
                let mut pixels = Vec::with_capacity(required);
                for i in 0..plane_size {
                    let b = decoded[i];
                    let g = decoded[i + plane_size];
                    let r = decoded[i + plane_size * 2];
                    pixels.push(b);
                    pixels.push(g);
                    pixels.push(r);
                }
                Ok(ImageData {
                    width: frame.header.width,
                    height: frame.header.height,
                    color_type: ImageColorType::Bgr,
                    depth: 8,
                    data: pixels,
                })
            }
            8 => {
                let indices = decoded.get(0..plane_size).ok_or_else(|| {
                    anyhow!(
                        "Frame {} decompressed data too short for indices",
                        frame.index
                    )
                })?;
                let palette_range = frame.palette_range.as_ref().ok_or_else(|| {
                    anyhow!("Frame {} missing palette data for 8bpp image", frame.index)
                })?;
                let palette = self
                    .data
                    .data
                    .get(palette_range.clone())
                    .ok_or_else(|| anyhow!("Frame {} palette range is invalid", frame.index))?;
                convert_index_palette_to_normal_bitmap(
                    indices,
                    8,
                    palette,
                    PaletteFormat::RgbX,
                    width_usize,
                    height_usize,
                )
                .with_context(|| format!("Failed to apply palette for frame {}", frame.index))
            }
            _ => Err(anyhow!("Unsupported bits-per-pixel: {}", self.bpp)),
        }
    }
}

impl Script for WillPlusWipImage {
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
        if self.frames.len() > 1 {
            eprintln!("WARN: WIP image contains multiple frames, exporting only the first frame");
            crate::COUNTER.inc_warning();
        }
        self.frames
            .get(0)
            .ok_or_else(|| anyhow!("No frames available in WIP image"))
            .and_then(|frame| self.decode_frame(frame))
    }

    fn is_multi_image(&self) -> bool {
        self.frames.len() > 1
    }

    fn export_multi_image<'a>(
        &'a self,
    ) -> Result<Box<dyn Iterator<Item = Result<ImageDataWithName>> + 'a>> {
        Ok(Box::new(WillPlusWipIterator {
            image: self,
            index: 0,
        }))
    }
}

struct WillPlusWipIterator<'a> {
    image: &'a WillPlusWipImage,
    index: usize,
}

impl<'a> Iterator for WillPlusWipIterator<'a> {
    type Item = Result<ImageDataWithName>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(frame) = self.image.frames.get(self.index) {
            let frame_index = self.index;
            self.index += 1;
            Some(
                self.image
                    .decode_frame(frame)
                    .map(|data| ImageDataWithName {
                        name: format!("{:04}", frame_index),
                        data,
                    }),
            )
        } else {
            None
        }
    }
}

fn lzss_decompress(compressed: &[u8], expected: usize) -> Result<Vec<u8>> {
    let mut output = Vec::with_capacity(expected.max(0x1000));
    let mut window = [0u8; 0x1000];
    let mut window_index: usize = 1;
    let mut control: u32 = 0;
    let mut remaining = compressed.len();
    let mut cursor = 0usize;

    while remaining > 0 {
        control >>= 1;
        if control & 0x100 == 0 {
            if remaining == 0 {
                break;
            }
            let value = compressed[cursor];
            cursor += 1;
            remaining -= 1;
            control = (value as u32) | 0xFF00;
        }

        if control & 1 != 0 {
            if remaining < 1 {
                return Err(anyhow!("Unexpected end of data while reading literal"));
            }
            let value = compressed[cursor];
            cursor += 1;
            remaining -= 1;
            output.push(value);
            window[window_index] = value;
            window_index = (window_index + 1) & 0x0FFF;
        } else {
            if remaining < 2 {
                return Err(anyhow!(
                    "Unexpected end of data while reading back-reference"
                ));
            }
            let hi = compressed[cursor] as usize;
            let lo = compressed[cursor + 1] as usize;
            cursor += 2;
            remaining -= 2;
            let mut offset = (hi << 4) | (lo >> 4);
            let mut count = (lo & 0x0F) + 2;
            while count > 0 {
                let value = window[offset & 0x0FFF];
                offset = offset.wrapping_add(1);
                output.push(value);
                window[window_index] = value;
                window_index = (window_index + 1) & 0x0FFF;
                count -= 1;
            }
        }
    }

    if expected > 0 && output.len() < expected {
        return Err(anyhow!(
            "Decompressed data shorter than expected: {} < {}",
            output.len(),
            expected
        ));
    }

    Ok(output)
}
