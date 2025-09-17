use crate::ext::io::*;
use crate::ext::vec::*;
use crate::types::*;
use crate::utils::encoding::*;
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

impl PgdGeHeader {
    pub fn is_base_file(&self) -> bool {
        self.offset_x == 0
            && self.offset_y == 0
            && self.width == self.canvas_width
            && self.height == self.canvas_height
    }
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct PgdDiffHeader {
    pub offset_x: u16,
    pub offset_y: u16,
    pub width: u16,
    pub height: u16,
    pub bpp: u16,
    #[fstring = 0x22]
    pub base_name: String,
}

pub struct PgdReader<T: Read + Seek> {
    pub input: T,
    output: Vec<u8>,
    width: u32,
    height: u32,
    bpp: u8,
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
            bpp: 0,
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

    pub fn with_diff_header(input: T, header: &PgdDiffHeader) -> Result<Self> {
        let mut s = Self::new(input, 0x30)?;
        s.width = header.width as u32;
        s.height = header.height as u32;
        s.bpp = header.bpp as u8;
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

    pub fn unpack_overlay(&mut self) -> Result<ImageData> {
        self.unpack_ge_pre()?;
        let data = self.post_process_pal(&self.output, 0, self.bpp as usize / 8)?;
        let color_type = match self.bpp {
            24 => ImageColorType::Bgr,
            32 => ImageColorType::Bgra,
            _ => {
                return Err(anyhow::anyhow!(
                    "Unsupported bpp for overlay PGD: {}",
                    self.bpp
                ));
            }
        };
        Ok(ImageData {
            width: self.width,
            height: self.height,
            color_type,
            depth: 8,
            data,
        })
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
    fake_compress: bool,
}

impl PgdWriter {
    pub fn new(data: ImageData, fake_compress: bool) -> Self {
        Self {
            data,
            method: 3,
            fake_compress,
        }
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
        let compressed = if self.fake_compress {
            ge_fake_compress(&data)?
        } else {
            ge_compress(&data)?
        };
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

// 新增：基于散列的快速 LZSS 压缩，兼容 unpack_ge_pre
fn ge_compress(data: &[u8]) -> Result<Vec<u8>> {
    const MIN_MATCH: usize = 4;
    const MAX_LEN: usize = 0x7FF + 4; // 2047 + 4 = 2051
    const MAX_DIST: usize = 0xFFF; // 12-bit distance
    const HASH_BITS: usize = 16;
    const HASH_SIZE: usize = 1 << HASH_BITS;

    #[inline(always)]
    fn hash3(bytes: &[u8]) -> usize {
        // 3字节哈希，乘黄金常数，取高 HASH_BITS 位
        let v = ((bytes[0] as u32) << 16) ^ ((bytes[1] as u32) << 8) ^ (bytes[2] as u32);
        (v.wrapping_mul(0x9E3779B1) >> (32 - HASH_BITS)) as usize
    }

    let n = data.len();
    if n == 0 {
        return Ok(Vec::new());
    }

    let mut out = Vec::with_capacity(n / 2 + 16);

    // 控制块状态
    let mut ctrl_pos = out.len();
    out.push(0u8); // 占位
    let mut ctrl: u8 = 0;
    let mut ctrl_cnt: u8 = 0;

    // 哈希表：保存最近出现位置
    let mut head = vec![usize::MAX; HASH_SIZE];

    // 延迟字面量缓冲
    let mut lit_start = 0usize;
    let mut lit_len = 0usize;

    // 辅助：开始新的控制块
    #[inline(always)]
    fn start_block(buf: &mut Vec<u8>, ctrl_pos: &mut usize, ctrl: &mut u8, ctrl_cnt: &mut u8) {
        *ctrl = 0;
        *ctrl_cnt = 0;
        *ctrl_pos = buf.len();
        buf.push(0u8);
    }

    // 辅助：写入控制字节
    #[inline(always)]
    fn flush_ctrl(buf: &mut Vec<u8>, ctrl_pos: usize, ctrl: u8) {
        if let Some(slot) = buf.get_mut(ctrl_pos) {
            *slot = ctrl;
        }
    }

    // 辅助：输出字面量（可拆分为多个 <=255 的条目）
    let flush_literals = |out: &mut Vec<u8>,
                          ctrl: &mut u8,
                          ctrl_cnt: &mut u8,
                          ctrl_pos: &mut usize,
                          lit_start: &mut usize,
                          lit_len: &mut usize| {
        while *lit_len > 0 {
            if *ctrl_cnt == 8 {
                flush_ctrl(out, *ctrl_pos, *ctrl);
                start_block(out, ctrl_pos, ctrl, ctrl_cnt);
            }
            let chunk = std::cmp::min(255, *lit_len);
            out.push(chunk as u8);
            out.extend_from_slice(&data[*lit_start..*lit_start + chunk]);
            *lit_start += chunk;
            *lit_len -= chunk;
            *ctrl_cnt += 1; // 字面量控制位为0，无需设置位
        }
    };

    let mut pos = 0usize;

    while pos < n {
        // 尝试匹配
        let mut best_len = 0usize;
        let mut best_dist = 0usize;

        if pos + MIN_MATCH <= n {
            let h = hash3(&data[pos..pos + 3]);
            let cand = head[h];
            if cand != usize::MAX && cand < pos {
                let dist = pos - cand;
                if dist > 0 && dist <= MAX_DIST {
                    // 计算匹配长度
                    let max_len = std::cmp::min(MAX_LEN, n - pos);
                    // 快速比较
                    let mut l = 0usize;
                    while l < max_len && data[cand + l] == data[pos + l] {
                        l += 1;
                    }
                    if l >= MIN_MATCH {
                        best_len = l;
                        best_dist = dist;
                    }
                }
            }
        }

        if best_len >= MIN_MATCH {
            // 先刷新字面量
            flush_literals(
                &mut out,
                &mut ctrl,
                &mut ctrl_cnt,
                &mut ctrl_pos,
                &mut lit_start,
                &mut lit_len,
            );

            // 控制块满则换块
            if ctrl_cnt == 8 {
                flush_ctrl(&mut out, ctrl_pos, ctrl);
                start_block(&mut out, &mut ctrl_pos, &mut ctrl, &mut ctrl_cnt);
            }

            let l = best_len.min(MAX_LEN);
            let dist = best_dist;

            // 写入回溯条目：u16 (LE)，必要时再跟长度低8位
            let len_minus4 = l - 4;
            let mut word: u16 = ((dist as u16) << 4) as u16;
            if len_minus4 <= 7 {
                // 短长度：bit3=1，低3位为长度
                word |= (len_minus4 as u16) | 0x8;
                out.push((word & 0xFF) as u8);
                out.push((word >> 8) as u8);
            } else {
                // 扩展长度：bit3=0，低3位为长度高3位，随后写低8位
                word |= ((len_minus4 >> 8) as u16) & 0x7;
                out.push((word & 0xFF) as u8);
                out.push((word >> 8) as u8);
                out.push((len_minus4 & 0xFF) as u8);
            }

            // 设置控制位为1
            let bit = 1u8.wrapping_shl(ctrl_cnt as u32);
            ctrl |= bit;
            ctrl_cnt += 1;

            // 更新哈希表（覆盖匹配范围，避免过度开销也可简化为只更新起始位）
            let end = pos + l;
            let upd_end = end
                .saturating_sub(MIN_MATCH - 1)
                .min(n.saturating_sub(MIN_MATCH));
            let mut p = pos;
            while p <= upd_end {
                let h = hash3(&data[p..p + 3]);
                head[h] = p;
                p += 1;
            }

            pos += l;
            lit_start = pos;
            lit_len = 0;
        } else {
            // 作为字面量
            if pos + 2 < n {
                let h = hash3(&data[pos..pos + 3]);
                head[h] = pos;
            }
            pos += 1;
            lit_len += 1;

            // 字面量满255则立刻输出一条
            if lit_len == 255 {
                flush_literals(
                    &mut out,
                    &mut ctrl,
                    &mut ctrl_cnt,
                    &mut ctrl_pos,
                    &mut lit_start,
                    &mut lit_len,
                );
            }
        }
    }

    // 结束时刷新剩余字面量与控制字节
    if lit_len > 0 {
        flush_literals(
            &mut out,
            &mut ctrl,
            &mut ctrl_cnt,
            &mut ctrl_pos,
            &mut lit_start,
            &mut lit_len,
        );
    }
    flush_ctrl(&mut out, ctrl_pos, ctrl);

    Ok(out)
}
