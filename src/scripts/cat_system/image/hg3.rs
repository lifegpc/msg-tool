use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::bit_stream::*;
use crate::utils::img::*;
use crate::utils::struct_pack::*;
use anyhow::Result;
use flate2::{Decompress, FlushDecompress};
use msg_tool_macro::*;
use std::io::{Read, Seek, Write};

#[derive(Debug)]
pub struct Hg3ImageBuilder {}

impl Hg3ImageBuilder {
    pub const fn new() -> Self {
        Hg3ImageBuilder {}
    }
}

impl ScriptBuilder for Hg3ImageBuilder {
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
        Ok(Box::new(Hg3Image::new(data, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["hg3"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::CatSystemHg3
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 4 && &buf[0..4] == b"HG-3" {
            return Some(255);
        }
        None
    }
}

#[derive(Debug, Clone, StructPack, StructUnpack)]
struct Hg3Entry {
    header_size: u32,
    _unk: u32,
    width: u32,
    height: u32,
    bpp: u32,
    offset_x: u32,
    offset_y: u32,
    canvas_width: u32,
    canvas_height: u32,
}

#[derive(Debug)]
pub struct Hg3Image {
    data: MemReader,
    entries: Vec<(Hg3Entry, usize, usize)>,
    draw_canvas: bool,
}

impl Hg3Image {
    pub fn new(buf: Vec<u8>, config: &ExtraConfig) -> Result<Self> {
        let mut reader = MemReader::new(buf);
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;
        if &magic != b"HG-3" {
            return Err(anyhow::anyhow!("Invalid HG-3 image format"));
        }
        let mut offset = 0xC;
        let mut entries = Vec::new();
        let len = reader.data.len();
        while offset + 0x14 < len && reader.cpeek_and_equal_at(offset + 8, b"stdinfo").is_ok() {
            let mut section_size = reader.cpeek_u32_at(offset)?;
            if section_size == 0 {
                section_size = (len - offset as usize) as u32;
            }
            let stdinfo_size = reader.cpeek_u32_at(offset + 0x10)?;
            if reader
                .cpeek_and_equal_at(offset + 8 + stdinfo_size as usize, b"img")
                .is_ok()
            {
                reader.pos = offset + 16;
                let entry = Hg3Entry::unpack(&mut reader, false, Encoding::Cp932)?;
                entries.push((entry, offset + 8, section_size as usize - 8));
            }
            offset += section_size as usize;
        }
        if entries.is_empty() {
            return Err(anyhow::anyhow!("No valid entries found in HG-3 image"));
        }
        Ok(Hg3Image {
            data: reader,
            entries,
            draw_canvas: config.cat_system_image_canvas,
        })
    }
}

impl Script for Hg3Image {
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
        if self.entries.len() > 1 {
            eprintln!(
                "WARN: There are multiple entries in the HG-3 image, only the first one will be exported."
            );
            crate::COUNTER.inc_warning();
        }
        let (entry, offset, size) = &self.entries[0];
        let data = &self.data.data[*offset..*offset + *size];
        let reader = Hg3Reader {
            m_input: MemReaderRef::new(data),
            m_info: entry.clone(),
            m_pixel_size: entry.bpp / 8,
        };
        let mut img = reader.unpack()?;
        if self.draw_canvas {
            if entry.canvas_width > 0 && entry.canvas_height > 0 {
                img = draw_on_canvas(
                    img,
                    entry.canvas_width,
                    entry.canvas_height,
                    entry.offset_x,
                    entry.offset_y,
                )?;
            }
        }
        Ok(img)
    }
}

pub struct Hg3Reader<'a> {
    m_input: MemReaderRef<'a>,
    m_info: Hg3Entry,
    m_pixel_size: u32,
}

impl<'a> Hg3Reader<'a> {
    pub fn unpack_stream(
        &mut self,
        data_offset: usize,
        data_packed: usize,
        data_unpacked: usize,
        ctl_packed: usize,
        ctl_unpacked: usize,
    ) -> Result<Vec<u8>> {
        let ctl_offset = data_offset + data_packed;
        let mut data = Vec::with_capacity(data_unpacked);
        data.resize(data_unpacked, 0);
        let z = &self.m_input.data[data_offset..data_offset + data_packed];
        let mut decompressor = Decompress::new(true);
        decompressor.decompress(z, &mut data, FlushDecompress::Finish)?;
        let z = &self.m_input.data[ctl_offset..ctl_offset + ctl_packed];
        let mut ctl = Vec::with_capacity(ctl_unpacked);
        ctl.resize(ctl_unpacked, 0);
        let mut decompressor = Decompress::new(true);
        decompressor.decompress(z, &mut ctl, FlushDecompress::Finish)?;
        let mut bits = LsbBitStream::new(MemReaderRef::new(&ctl));
        let mut copy = bits.get_next_bit()?;
        let output_size = Self::get_bit_count(&mut bits)? as usize;
        let mut output = Vec::with_capacity(output_size);
        output.resize(output_size, 0);
        let mut src = 0;
        let mut dst = 0;
        while dst < output_size {
            let count = Self::get_bit_count(&mut bits)? as usize;
            if copy {
                output[dst..dst + count].copy_from_slice(&data[src..src + count]);
                src += count;
            }
            dst += count;
            copy = !copy;
        }
        Ok(self.apply_delta(&output))
    }

    fn get_bit_count(bits: &mut LsbBitStream<MemReaderRef<'_>>) -> Result<u32> {
        let mut n = 0;
        while !bits.get_next_bit()? {
            n += 1;
            if n >= 0x20 {
                return Err(anyhow::anyhow!("Overflow at HG-3 Reader."));
            }
        }
        let mut value = 1;
        for _ in 0..n {
            value = (value << 1) | (bits.get_next_bit()? as u32);
        }
        Ok(value)
    }

    fn convert_value(mut val: u8) -> u8 {
        let carry = val & 1 != 0;
        val >>= 1;
        if carry { val ^ 0xff } else { val }
    }

    fn apply_delta(&self, pixels: &[u8]) -> Vec<u8> {
        let mut table = [[0u32; 0x100]; 4];
        for i in 0..0x100u32 {
            let mut val = i & 0xC0;
            val <<= 6;
            val |= i & 0x30;
            val <<= 6;
            val |= i & 0x0C;
            val <<= 6;
            val |= i & 0x03;
            table[0][i as usize] = val << 6;
            table[1][i as usize] = val << 4;
            table[2][i as usize] = val << 2;
            table[3][i as usize] = val;
        }
        let pxl_len = pixels.len();
        let plane_size = pxl_len / 4;
        let mut plane0 = 0;
        let mut plane1 = plane0 + plane_size;
        let mut plane2 = plane1 + plane_size;
        let mut plane3 = plane2 + plane_size;
        let mut output = Vec::with_capacity(pxl_len);
        output.resize(pxl_len, 0);
        let mut dst = 0;
        while dst < pxl_len {
            let val = table[0][pixels[plane0] as usize]
                | table[1][pixels[plane1] as usize]
                | table[2][pixels[plane2] as usize]
                | table[3][pixels[plane3] as usize];
            plane0 += 1;
            plane1 += 1;
            plane2 += 1;
            plane3 += 1;
            output[dst] = Self::convert_value(val as u8);
            dst += 1;
            output[dst] = Self::convert_value((val >> 8) as u8);
            dst += 1;
            output[dst] = Self::convert_value((val >> 16) as u8);
            dst += 1;
            output[dst] = Self::convert_value((val >> 24) as u8);
            dst += 1;
        }
        let stride = self.m_info.width * self.m_pixel_size;
        for x in self.m_pixel_size..stride {
            output[x as usize] =
                output[x as usize].wrapping_add(output[x as usize - self.m_pixel_size as usize]);
        }
        let mut prev = 0;
        for _ in 1..self.m_info.height {
            let line = prev + stride;
            for x in 0..stride {
                output[line as usize + x as usize] = output[line as usize + x as usize]
                    .wrapping_add(output[prev as usize + x as usize]);
            }
            prev = line;
        }
        output
    }

    fn unpack(mut self) -> Result<ImageData> {
        self.m_input.pos = self.m_info.header_size as usize;
        let mut image_type = [0; 8];
        self.m_input.read_exact(&mut image_type)?;
        if &image_type == b"img0000 " {
            return self.unpack_img0000();
        } else {
            return Err(anyhow::anyhow!("Unsupported image type: {:?}", image_type));
        }
    }

    fn unpack_img0000(&mut self) -> Result<ImageData> {
        self.m_input.pos = self.m_info.header_size as usize + 0x18;
        let packed_data_size = self.m_input.read_u32()?;
        let data_size = self.m_input.read_u32()?;
        let ctl_packed_size = self.m_input.read_u32()?;
        let ctl_size = self.m_input.read_u32()?;
        let data = self.unpack_stream(
            self.m_info.header_size as usize + 0x28,
            packed_data_size as usize,
            data_size as usize,
            ctl_packed_size as usize,
            ctl_size as usize,
        )?;
        let fmt = match self.m_info.bpp {
            24 => ImageColorType::Bgr,
            32 => ImageColorType::Bgra,
            _ => {
                return Err(anyhow::anyhow!(
                    "Unsupported BPP: {} in HG-3 image",
                    self.m_info.bpp
                ));
            }
        };
        let mut img = ImageData {
            width: self.m_info.width,
            height: self.m_info.height,
            color_type: fmt,
            depth: 8,
            data,
        };
        flip_image(&mut img)?;
        Ok(img)
    }
}

fn draw_on_canvas(
    img: ImageData,
    canvas_width: u32,
    canvas_height: u32,
    offset_x: u32,
    offset_y: u32,
) -> Result<ImageData> {
    let bytes_per_pixel = img.color_type.bpp(img.depth) as u32 / 8;
    let mut canvas_data = vec![0u8; (canvas_width * canvas_height * bytes_per_pixel) as usize];
    let canvas_stride = canvas_width * bytes_per_pixel;
    let img_stride = img.width * bytes_per_pixel;

    for y in 0..img.height {
        let canvas_y = y + offset_y;
        if canvas_y >= canvas_height {
            continue;
        }
        let canvas_start = (canvas_y * canvas_stride + offset_x * bytes_per_pixel) as usize;
        let img_start = (y * img_stride) as usize;
        let copy_len = img_stride as usize;
        if canvas_start + copy_len > canvas_data.len() {
            continue;
        }
        canvas_data[canvas_start..canvas_start + copy_len]
            .copy_from_slice(&img.data[img_start..img_start + copy_len]);
    }

    Ok(ImageData {
        width: canvas_width,
        height: canvas_height,
        color_type: img.color_type,
        depth: img.depth,
        data: canvas_data,
    })
}
