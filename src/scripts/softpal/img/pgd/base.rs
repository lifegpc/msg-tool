use crate::ext::io::*;
use crate::ext::vec::*;
use crate::types::*;
use crate::utils::img::*;
use crate::utils::struct_pack::*;
use anyhow::Result;
use msg_tool_macro::*;
use std::io::{Read, Seek, SeekFrom, Write};

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct PgdGeHeader {
    pub offset_x: u32,
    pub offset_y: u32,
    pub width: u32,
    pub height: u32,
    // untested
    pub canvas_width: u32,
    // untested
    pub canvas_height: u32,
    pub mode: u32,
}

pub struct PgdReader<T: Read + Seek> {
    pub input: T,
    output: Vec<u8>,
    width: u32,
    height: u32,
    _bpp: u8,
    method: u32,
    format: Option<ImageColorType>,
}

impl<T: Read + Seek> PgdReader<T> {
    pub fn new(mut input: T, pos: u64) -> Result<Self> {
        input.seek(SeekFrom::Start(pos))?;
        let unpacked_size = input.read_u32()?;
        input.read_u32()?; // packed size
        let output = vec![0u8; unpacked_size as usize];
        Ok(PgdReader {
            input,
            output,
            width: 0,
            height: 0,
            _bpp: 0,
            method: 0,
            format: None,
        })
    }

    pub fn with_ge_header(input: T, header: &PgdGeHeader) -> Result<Self> {
        let mut s = Self::new(input, 0x20)?;
        s.width = header.width;
        s.height = header.height;
        s.method = header.mode;
        Ok(s)
    }

    pub fn unpack_ge(mut self) -> Result<ImageData> {
        self.unpack_ge_pre()?;
        let data = match self.method {
            1 => self.post_process1()?,
            2 => self.post_process2()?,
            3 => self.post_process3()?,
            _ => return Err(anyhow::anyhow!("Unsupported GE mode: {}", self.method)),
        };
        let color_type = self
            .format
            .ok_or_else(|| anyhow::anyhow!("Unknown image format"))?;
        Ok(ImageData {
            width: self.width,
            height: self.height,
            color_type,
            depth: 8,
            data,
        })
    }

    fn unpack_ge_pre(&mut self) -> Result<()> {
        let mut dst = 0;
        let mut ctl = 2;
        let len = self.output.len();
        while dst < len {
            ctl >>= 1;
            if ctl == 1 {
                ctl = self.input.read_u8()? as i32 | 0x100;
            }
            let mut count;
            if ctl & 1 != 0 {
                let mut offset = self.input.read_u16()? as usize;
                count = offset & 7;
                if offset & 8 == 0 {
                    count = count << 8 | (self.input.read_u8()? as usize);
                }
                count += 4;
                offset >>= 4;
                self.output.copy_overlapped(dst - offset, dst, count);
            } else {
                count = self.input.read_u8()? as usize;
                self.input.read_exact(&mut self.output[dst..dst + count])?;
            }
            dst += count;
        }
        Ok(())
    }

    fn post_process1(&mut self) -> Result<Vec<u8>> {
        self.format = Some(ImageColorType::Bgra);
        let input = &self.output;
        let mut output = Vec::with_capacity(input.len());
        let plane_size = input.len() / 4;
        let a_src = 0;
        let r_src = plane_size;
        let g_src = plane_size * 2;
        let b_src = plane_size * 3;
        for i in 0..plane_size {
            output.push(input[b_src + i]);
            output.push(input[g_src + i]);
            output.push(input[r_src + i]);
            output.push(input[a_src + i]);
        }
        Ok(output)
    }

    #[inline(always)]
    fn clamp(v: i32) -> u8 {
        if v > 255 {
            255
        } else if v < 0 {
            0
        } else {
            v as u8
        }
    }

    fn post_process2(&mut self) -> Result<Vec<u8>> {
        self.format = Some(ImageColorType::Bgr);
        let input = &self.output;
        let stride = self.width as usize * 3;
        let segment_size = self.width as usize * self.height as usize / 4;
        let mut src0 = 0;
        let mut src1 = segment_size;
        let mut src2 = segment_size * 2;
        let mut output = vec![0u8; stride * self.height as usize];
        let mut dst = 0;
        let points = [0, 1, self.width, self.width + 1];
        for _y in (1..=(self.height as usize / 2)).rev() {
            for _x in (1..=(self.width as usize / 2)).rev() {
                let i0 = input[src0] as i8;
                let i1 = input[src1] as i8;
                let b = 226 * i0 as i32;
                let g = -43 * i0 as i32 - 89 * i1 as i32;
                let r = 179 * i1 as i32;
                src0 += 1;
                src1 += 1;
                for i in 0..4 {
                    let mut offset = points[i] as usize;
                    let base_value = (input[src2 + offset] as i32) << 7;
                    offset = dst + 3 * offset;
                    output[offset] = Self::clamp(base_value + b);
                    output[offset + 1] = Self::clamp(base_value + g);
                    output[offset + 2] = Self::clamp(base_value + r);
                }
                src2 += 2;
                dst += 6;
            }
            src2 += self.width as usize;
            dst += stride;
        }
        Ok(output)
    }

    fn post_process3(&mut self) -> Result<Vec<u8>> {
        let input = &self.output;
        let reader = MemReaderRef::new(input);
        let bbp = reader.cpeek_u16_at(0x2)?;
        self.format = Some(if bbp == 24 {
            ImageColorType::Bgr
        } else if bbp == 32 {
            ImageColorType::Bgra
        } else {
            return Err(anyhow::anyhow!("Unsupported bpp: {}", bbp));
        });
        self.width = reader.cpeek_u16_at(0x4)? as u32;
        self.height = reader.cpeek_u16_at(0x6)? as u32;
        self.post_process_pal(input, 8, bbp as usize / 8)
    }

    fn post_process_pal(&self, input: &[u8], mut src: usize, pixel_size: usize) -> Result<Vec<u8>> {
        let stride = self.width as usize * pixel_size;
        let mut output = vec![0u8; stride * self.height as usize];
        let mut ctl = src;
        src += self.height as usize;
        let mut dst = 0;
        for _row in 0..self.height as usize {
            let c = input[ctl];
            ctl += 1;
            if c & 1 != 0 {
                let mut prev = dst;
                for _ in 0..pixel_size {
                    output[dst] = input[src];
                    dst += 1;
                    src += 1;
                }
                let mut count = stride - pixel_size;
                while count > 0 {
                    count -= 1;
                    output[dst] = output[prev].wrapping_sub(input[src]);
                    dst += 1;
                    prev += 1;
                    src += 1;
                }
            } else if c & 2 != 0 {
                let mut prev = dst - stride;
                let mut count = stride;
                while count > 0 {
                    count -= 1;
                    output[dst] = output[prev].wrapping_sub(input[src]);
                    dst += 1;
                    prev += 1;
                    src += 1;
                }
            } else {
                for _ in 0..pixel_size {
                    output[dst] = input[src];
                    dst += 1;
                    src += 1;
                }
                let mut prev = dst - stride;
                let mut count = stride - pixel_size;
                while count > 0 {
                    count -= 1;
                    output[dst] = (((output[prev] as u16)
                        .wrapping_add(output[dst - pixel_size] as u16)
                        / 2) as u8)
                        .wrapping_sub(input[src]);
                    dst += 1;
                    prev += 1;
                    src += 1;
                }
            }
        }
        Ok(output)
    }
}

pub struct PgdWriter {
    data: ImageData,
    method: u32,
}

impl PgdWriter {
    pub fn new(data: ImageData) -> Self {
        Self { data, method: 3 }
    }

    pub fn with_method(mut self, method: u32) -> Self {
        self.method = method;
        self
    }

    pub fn pack_ge<W: Write>(mut self, mut writer: W) -> Result<()> {
        let data = match self.method {
            3 => self.process3()?,
            _ => panic!("Unsupported GE mode: {}", self.method),
        };
        let unpacked_len = data.len() as u32;
        let compressed = ge_fake_compress(&data)?;
        let packed_len = compressed.len() as u32;
        writer.write_u32(unpacked_len)?;
        writer.write_u32(packed_len)?;
        writer.write_all(&compressed)?;
        Ok(())
    }

    fn process3(&mut self) -> Result<Vec<u8>> {
        let bpp = self.data.color_type.bpp(8) as usize;
        let width = self.data.width as u16;
        let height = self.data.height as u16;
        let mut data = MemWriter::new();
        data.write_u16(0)?; // unk
        data.write_u16(bpp as u16)?;
        data.write_u16(width)?;
        data.write_u16(height)?;
        data.write_all(&self.process_pal()?)?;
        Ok(data.into_inner())
    }

    fn process_pal(&mut self) -> Result<Vec<u8>> {
        let bpp = match self.data.color_type {
            ImageColorType::Bgr => 3,
            ImageColorType::Bgra => 4,
            ImageColorType::Rgb => {
                convert_rgb_to_bgr(&mut self.data)?;
                3
            }
            ImageColorType::Rgba => {
                convert_rgba_to_bgra(&mut self.data)?;
                4
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Unsupported color type for palettized PGD: {:?}",
                    self.data.color_type
                ));
            }
        };
        // Fixed mode
        let ctl = vec![1u8; self.data.height as usize];
        let stride = self.data.width as usize * bpp;
        let mut output = vec![0u8; stride * self.data.height as usize];
        let mut dst = 0;
        for _ in 0..self.data.height as usize {
            let mut prev = dst;
            for _ in 0..bpp {
                output[dst] = self.data.data[dst];
                dst += 1;
            }
            let mut count = stride - bpp;
            while count > 0 {
                count -= 1;
                output[dst] = self.data.data[prev].wrapping_sub(self.data.data[dst]);
                dst += 1;
                prev += 1;
            }
        }
        let mut result = Vec::with_capacity(ctl.len() + output.len());
        result.extend_from_slice(&ctl);
        result.extend_from_slice(&output);
        Ok(result)
    }
}

fn ge_fake_compress(data: &[u8]) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut pos = 0;
    let data_len = data.len();

    while pos < data_len {
        // 每8个数据块需要一个控制字节
        // 控制字节为0表示接下来8个操作都是直接数据复制
        output.push(0u8);

        // 处理最多8个数据块
        for _ in 0..8 {
            if pos >= data_len {
                break;
            }

            // 计算当前块的大小（最大255字节）
            let chunk_size = std::cmp::min(255, data_len - pos);

            // 写入块大小
            output.push(chunk_size as u8);

            // 写入数据
            output.extend_from_slice(&data[pos..pos + chunk_size]);

            pos += chunk_size;
        }
    }

    Ok(output)
}
