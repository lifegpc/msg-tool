use crate::ext::atomic::*;
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::bit_stream::*;
use crate::utils::struct_pack::*;
use anyhow::Ok;
use anyhow::Result;
use msg_tool_macro::*;
use std::io::{Read, Seek, Write};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub struct BgiCBGBuilder {}

impl BgiCBGBuilder {
    pub const fn new() -> Self {
        BgiCBGBuilder {}
    }
}

impl ScriptBuilder for BgiCBGBuilder {
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
        Ok(Box::new(BgiCBG::new(data, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::BGICbg
    }

    fn is_image(&self) -> bool {
        true
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 0x10 && buf.starts_with(b"CompressedBG___") {
            return Some(255);
        }
        None
    }
}

#[derive(Debug, StructPack, StructUnpack)]
struct BgiCBGHeader {
    width: u16,
    height: u16,
    bpp: u32,
    _unk: u64,
    intermediate_length: u32,
    key: u32,
    enc_length: u32,
    check_sum: u8,
    check_xor: u8,
    version: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CbgColorType {
    Bgra32,
    Bgr24,
    Grayscale,
    Bgr565,
}

fn convert_bgr565_to_bgr24(input: Vec<u8>, width: u16, height: u16) -> ImageData {
    let pixel_count = width as usize * height as usize;
    let mut output = Vec::with_capacity(pixel_count * 3);

    for chunk in input.chunks_exact(2) {
        let pixel = u16::from_le_bytes([chunk[0], chunk[1]]);

        let blue_5bit = (pixel & 0x1) as u8;
        let green_6bit = ((pixel >> 5) & 0x3) as u8;
        let red_5bit = ((pixel >> 11) & 0x1) as u8;

        let blue = ((blue_5bit as u16 * 255) / 31) as u8;
        let green = ((green_6bit as u16 * 255) / 63) as u8;
        let red = ((red_5bit as u16 * 255) / 31) as u8;

        output.push(blue);
        output.push(green);
        output.push(red);
    }

    ImageData {
        width: width as u32,
        height: height as u32,
        color_type: ImageColorType::Bgr,
        depth: 8,
        data: output,
    }
}

#[derive(Debug)]
pub struct BgiCBG {
    header: BgiCBGHeader,
    data: MemReader,
    color_type: CbgColorType,
}

impl BgiCBG {
    pub fn new(data: Vec<u8>, _config: &ExtraConfig) -> Result<Self> {
        let mut reader = MemReader::new(data);
        let mut magic = [0u8; 16];
        reader.read_exact(&mut magic)?;
        if !magic.starts_with(b"CompressedBG___") {
            return Err(anyhow::anyhow!("Invalid magic: {:?}", magic));
        }
        let header = BgiCBGHeader::unpack(&mut reader, false, Encoding::Cp932)?;
        if header.version > 2 {
            return Err(anyhow::anyhow!("Unsupported version: {}", header.version));
        }
        let color_type = match header.bpp {
            32 => CbgColorType::Bgra32,
            24 => CbgColorType::Bgr24,
            8 => CbgColorType::Grayscale,
            16 => {
                if header.version == 2 {
                    return Err(anyhow::anyhow!("Unsupported BPP 16 in version 2"));
                }
                CbgColorType::Bgr565
            }
            _ => return Err(anyhow::anyhow!("Unsupported BPP: {}", header.bpp)),
        };
        Ok(BgiCBG {
            header,
            data: reader,
            color_type,
        })
    }
}

impl Script for BgiCBG {
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
        let decoder = CbgDecoder::new(self.data.to_ref(), &self.header, self.color_type)?;
        Ok(decoder.unpack()?)
    }
}

struct CbgDecoder<'a> {
    stream: MsbBitStream<MemReaderRef<'a>>,
    info: &'a BgiCBGHeader,
    color_type: CbgColorType,
    key: u32,
    magic: u32,
    pixel_size: u8,
    stride: usize,
}

impl<'a> CbgDecoder<'a> {
    fn new(
        reader: MemReaderRef<'a>,
        info: &'a BgiCBGHeader,
        color_type: CbgColorType,
    ) -> Result<Self> {
        let magic = 0;
        let key = info.key;
        let stream = MsbBitStream::new(reader);
        let pixel_size = info.bpp as u8 / 8;
        let stride = info.width as usize * (info.bpp as usize / 8);
        Ok(CbgDecoder {
            stream,
            info,
            key,
            magic,
            color_type,
            pixel_size,
            stride,
        })
    }

    fn unpack(mut self) -> Result<ImageData> {
        self.stream.m_input.pos = 0x30;
        if self.info.version < 2 {
            return self.unpack_v1();
        } else if self.info.version == 2 {
            if self.info.enc_length < 0x80 {
                return Err(anyhow::anyhow!(
                    "Invalid encoded length: {}",
                    self.info.enc_length
                ));
            }
            return self.unpack_v2();
        }
        Err(anyhow::anyhow!("Unknown version: {}", self.info.version))
    }

    fn unpack_v1(&mut self) -> Result<ImageData> {
        let leaf_nodes_weight = {
            let stream = MemReader::new(self.read_encoded()?);
            let mut stream_ref = stream.to_ref();
            Self::read_weight_table(&mut stream_ref, 0x100)?
        };
        let tree = HuffmanTree::new(&leaf_nodes_weight, false);
        let mut packed = Vec::with_capacity(self.info.intermediate_length as usize);
        packed.resize(self.info.intermediate_length as usize, 0);
        self.huffman_decompress(&tree, &mut packed)?;
        let buf_size = self.stride * self.info.height as usize;
        let mut output = Vec::with_capacity(buf_size);
        output.resize(buf_size, 0);
        Self::unpack_zeros(&packed, &mut output);
        self.reverse_average_sampling(&mut output);
        let color_type = match self.color_type {
            CbgColorType::Bgra32 => ImageColorType::Bgra,
            CbgColorType::Bgr24 => ImageColorType::Bgr,
            CbgColorType::Grayscale => ImageColorType::Grayscale,
            CbgColorType::Bgr565 => {
                return Ok(convert_bgr565_to_bgr24(
                    output,
                    self.info.width,
                    self.info.height,
                ));
            }
        };
        Ok(ImageData {
            width: self.info.width as u32,
            height: self.info.height as u32,
            color_type,
            depth: 8,
            data: output,
        })
    }

    // #TODO: Fix
    fn unpack_v2(&mut self) -> Result<ImageData> {
        let dct_data = self.read_encoded()?;
        let mut dct = [[0f32; 64]; 2];
        for i in 0..0x80usize {
            dct[i >> 6][i & 0x3f] = DCT_TABLE[i & 0x3f] * dct_data[i] as f32;
        }
        let base_offset = self.stream.m_input.pos;
        let tree1 = HuffmanTree::new(
            &Self::read_weight_table(&mut self.stream.m_input, 0x10)?,
            true,
        );
        let tree2 = HuffmanTree::new(
            &Self::read_weight_table(&mut self.stream.m_input, 0xB0)?,
            true,
        );
        let bpp = self.info.bpp;
        let width = ((self.info.width as i32 + 7) & -8) as u16;
        let height = ((self.info.height as i32 + 7) & -8) as u16;
        let y_blocks = height / 8;
        let mut offsets = Vec::with_capacity(y_blocks as usize + 1);
        let input_base = self.stream.m_input.pos + (y_blocks as usize + 1) * 4 - base_offset;
        for _ in 0..y_blocks + 1 {
            let offset = self.stream.m_input.read_u32()?;
            offsets.push(offset as usize - input_base);
        }
        let input = self.stream.m_input.data[self.stream.m_input.pos..].to_vec();
        let pad_skip = ((width as usize >> 3) + 7) >> 3;
        let mut tasks = Vec::with_capacity(y_blocks as usize + 1);
        let output_size = width as usize * height as usize * 4;
        let mut output = Vec::with_capacity(output_size);
        output.resize(output_size, 0);
        let output = Mutex::new(output);
        let decoder = Arc::new(ParallelCbgDecoder {
            input,
            output,
            bpp: bpp,
            width,
            height,
            tree1,
            tree2,
            dct,
            has_alpha: AtomicBool::new(false),
        });
        let mut dst = 0usize;
        for i in 0..y_blocks {
            let block_offset = offsets[i as usize] + pad_skip;
            let next_offset = if i + 1 == y_blocks {
                decoder.input.len()
            } else {
                offsets[(i + 1) as usize]
            };
            let cdst = dst;
            let decoder_ref = Arc::clone(&decoder);
            let task = std::thread::spawn(move || {
                decoder_ref.unpack_block(block_offset, next_offset - block_offset, cdst)
            });
            tasks.push(task);
            dst += decoder.width as usize * 32;
        }
        if self.info.bpp == 32 {
            let decoder_ref = Arc::clone(&decoder);
            let task =
                std::thread::spawn(move || decoder_ref.unpack_alpha(offsets[y_blocks as usize]));
            tasks.push(task);
        }
        for task in tasks {
            task.join()
                .map_err(|e| anyhow::anyhow!("Failed to join thread: {:?}", e))??;
        }
        let has_alpha = decoder.has_alpha.qload();
        let width = decoder.width as u32;
        let height = decoder.height as u32;
        let mut output = decoder
            .output
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock output: {}", e))?
            .clone();
        if !has_alpha {
            let mut src_idx = 0;
            let mut dst_idx = 0;
            for _ in 0..self.info.height {
                for _ in 0..self.info.width {
                    output[dst_idx] = output[src_idx];
                    output[dst_idx + 1] = output[src_idx + 1];
                    output[dst_idx + 2] = output[src_idx + 2];
                    src_idx += 4;
                    dst_idx += 3;
                }
            }
            output.truncate(dst_idx);
        }
        Ok(ImageData {
            width,
            height,
            color_type: if has_alpha {
                ImageColorType::Bgra
            } else {
                ImageColorType::Bgr
            },
            depth: 8,
            data: output,
        })
    }

    fn read_encoded(&mut self) -> Result<Vec<u8>> {
        let mut output = Vec::with_capacity(self.info.enc_length as usize);
        output.resize(self.info.enc_length as usize, 0);
        self.stream.m_input.read_exact(&mut output)?;
        let mut sum = 0u8;
        let mut xor = 0u8;
        for i in 0..output.len() {
            output[i] = output[i].wrapping_sub(self.update_key());
            sum = sum.wrapping_add(output[i]);
            xor ^= output[i];
        }
        if sum != self.info.check_sum || xor != self.info.check_xor {
            return Err(anyhow::anyhow!(
                "Checksum mismatch: sum={}, xor={}",
                sum,
                xor
            ));
        }
        Ok(output)
    }

    fn read_int(input: &mut MemReaderRef<'_>) -> Result<i32> {
        let mut v = 0;
        let mut code_length = 0;
        loop {
            let code = input.read_i8()?;
            if code_length >= 32 {
                return Err(anyhow::anyhow!(
                    "Failed to raed int: code={}, code_length={}",
                    code,
                    code_length
                ));
            }
            v |= ((code & 0x7f) as i32) << code_length;
            code_length += 7;
            if code & -128 == 0 {
                break;
            }
        }
        Ok(v)
    }

    fn read_weight_table(input: &mut MemReaderRef<'_>, length: usize) -> Result<Vec<u32>> {
        let mut weights = Vec::with_capacity(length);
        for _ in 0..length {
            let weight = Self::read_int(input)? as u32;
            weights.push(weight);
        }
        Ok(weights)
    }

    fn huffman_decompress(&mut self, tree: &HuffmanTree, output: &mut [u8]) -> Result<()> {
        for dst in 0..output.len() {
            output[dst] = tree.decode_token(&mut self.stream)? as u8;
        }
        Ok(())
    }

    fn unpack_zeros(input: &[u8], output: &mut [u8]) {
        let mut dst = 0;
        let mut dec_zero = 0;
        let mut src = 0;
        while dst < output.len() {
            let mut code_length = 0;
            let mut count = 0;
            let mut code;
            loop {
                if src >= input.len() {
                    return;
                }
                code = input[src];
                src += 1;
                count |= ((code & 0x7f) as usize) << code_length;
                code_length += 7;
                if code & 0x80 == 0 {
                    break;
                }
            }
            if dst + count > output.len() {
                break;
            }
            if dec_zero == 0 {
                if src + count > input.len() {
                    break;
                }
                output[dst..dst + count].copy_from_slice(&input[src..src + count]);
                src += count;
            } else {
                for i in 0..count {
                    output[dst + i] = 0;
                }
            }
            dec_zero ^= 1;
            dst += count;
        }
    }

    fn reverse_average_sampling(&self, output: &mut [u8]) {
        for y in 0..self.info.height {
            let line = y as usize * self.stride;
            for x in 0..self.info.width {
                let pixel = line + x as usize * self.pixel_size as usize;
                for p in 0..self.pixel_size {
                    let mut avg = 0u32;
                    if x > 0 {
                        avg = avg.wrapping_add(
                            output[pixel + p as usize - self.pixel_size as usize] as u32,
                        );
                    }
                    if y > 0 {
                        avg = avg.wrapping_add(output[pixel + p as usize - self.stride] as u32);
                    }
                    if x > 0 && y > 0 {
                        avg /= 2;
                    }
                    if avg != 0 {
                        output[pixel + p as usize] =
                            output[pixel + p as usize].wrapping_add(avg as u8);
                    }
                }
            }
        }
    }

    fn update_key(&mut self) -> u8 {
        let v0 = 20021 * (self.key & 0xffff);
        let mut v1 = self.magic | (self.key >> 16);
        v1 = v1
            .overflowing_mul(20021)
            .0
            .overflowing_add(self.key.overflowing_mul(346).0)
            .0;
        v1 = (v1 + (v0 >> 16)) & 0xffff;
        self.key = (v1 << 16) + (v0 & 0xffff) + 1;
        v1 as u8
    }
}

#[derive(Debug)]
struct HuffmanNode {
    valid: bool,
    is_parent: bool,
    weight: u32,
    left_index: usize,
    right_index: usize,
}

#[derive(Debug)]
struct HuffmanTree {
    nodes: Vec<HuffmanNode>,
}

impl HuffmanTree {
    fn new(weights: &[u32], v2: bool) -> Self {
        let mut nodes = Vec::with_capacity(weights.len() * 2);
        let mut root_node_weight = 0u32;
        for weight in weights {
            let node = HuffmanNode {
                valid: *weight != 0,
                is_parent: false,
                weight: *weight,
                left_index: 0,
                right_index: 0,
            };
            nodes.push(node);
            root_node_weight = root_node_weight.wrapping_add(*weight);
        }
        let mut child_node_index = [0usize; 2];
        loop {
            let mut weight = 0u32;
            for i in 0usize..2usize {
                let mut min_weight = u32::MAX;
                child_node_index[i] = usize::MAX;
                let mut n = 0;
                if v2 {
                    while n < nodes.len() {
                        if nodes[n].valid {
                            min_weight = nodes[n].weight;
                            child_node_index[i] = n;
                            n += 1;
                            break;
                        }
                        n += 1;
                    }
                    n = n.max(i + 1);
                }
                while n < nodes.len() {
                    if nodes[n].valid && nodes[n].weight < min_weight {
                        min_weight = nodes[n].weight;
                        child_node_index[i] = n;
                    }
                    n += 1;
                }
                if child_node_index[i] == usize::MAX {
                    continue;
                }
                nodes[child_node_index[i]].valid = false;
                weight = weight.wrapping_add(nodes[child_node_index[i]].weight);
            }
            let parent_node = HuffmanNode {
                valid: true,
                is_parent: true,
                left_index: child_node_index[0],
                right_index: child_node_index[1],
                weight,
            };
            nodes.push(parent_node);
            if weight >= root_node_weight {
                break;
            }
        }
        Self { nodes }
    }

    fn decode_token(&self, stream: &mut MsbBitStream<MemReaderRef<'_>>) -> Result<usize> {
        let mut node_index = self.nodes.len() - 1;
        loop {
            let bit = stream.get_next_bit()?;
            if !bit {
                node_index = self.nodes[node_index].left_index;
            } else {
                node_index = self.nodes[node_index].right_index;
            }
            if !self.nodes[node_index].is_parent {
                return Ok(node_index);
            }
        }
    }
}

const DCT_TABLE: [f32; 64] = [
    1.00000000, 1.38703990, 1.30656302, 1.17587554, 1.00000000, 0.78569496, 0.54119611, 0.27589938,
    1.38703990, 1.92387950, 1.81225491, 1.63098633, 1.38703990, 1.08979023, 0.75066054, 0.38268343,
    1.30656302, 1.81225491, 1.70710683, 1.53635550, 1.30656302, 1.02655995, 0.70710677, 0.36047992,
    1.17587554, 1.63098633, 1.53635550, 1.38268340, 1.17587554, 0.92387950, 0.63637930, 0.32442334,
    1.00000000, 1.38703990, 1.30656302, 1.17587554, 1.00000000, 0.78569496, 0.54119611, 0.27589938,
    0.78569496, 1.08979023, 1.02655995, 0.92387950, 0.78569496, 0.61731654, 0.42521504, 0.21677275,
    0.54119611, 0.75066054, 0.70710677, 0.63637930, 0.54119611, 0.42521504, 0.29289323, 0.14931567,
    0.27589938, 0.38268343, 0.36047992, 0.32442334, 0.27589938, 0.21677275, 0.14931567, 0.07612047,
];

const BLOCK_FILL_ORDER: [u8; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

struct ParallelCbgDecoder {
    input: Vec<u8>,
    output: Mutex<Vec<u8>>,
    bpp: u32,
    width: u16,
    height: u16,
    tree1: HuffmanTree,
    tree2: HuffmanTree,
    dct: [[f32; 64]; 2],
    has_alpha: AtomicBool,
}

impl ParallelCbgDecoder {
    fn unpack_block(&self, offset: usize, length: usize, dst: usize) -> Result<()> {
        let input = MemReaderRef::new(&self.input[offset..offset + length]);
        let mut reader = MsbBitStream::new(input);
        let block_size = CbgDecoder::read_int(&mut reader.m_input)?;
        let mut color_data = Vec::with_capacity(block_size as usize);
        color_data.resize(block_size as usize, 0i16);
        let mut acc = 0;
        let mut i = 0;
        while i < block_size && reader.m_input.pos < reader.m_input.data.len() {
            let count = self.tree1.decode_token(&mut reader)?;
            if count != 0 {
                let mut v = reader.get_bits(count as u32)? as i32;
                if (v >> (count - 1)) == 0 {
                    v = (-1 << count | v) + 1;
                }
                acc += v;
            }
            color_data[i as usize] = acc as i16;
            i += 64;
        }
        if reader.m_cached_bits & 7 != 0 {
            reader.get_bits(reader.m_cached_bits & 7)?;
        }
        i = 0;
        while i < block_size && reader.m_input.pos < reader.m_input.data.len() {
            let mut index = 1;
            while reader.m_input.pos < reader.m_input.data.len() {
                let mut code = self.tree2.decode_token(&mut reader)?;
                if code == 0 {
                    break;
                }
                if code == 0xf {
                    index += 0x10;
                    continue;
                }
                index += code & 0xf;
                if index >= 64 {
                    break;
                }
                code >>= 4;
                let mut v = reader.get_bits(code as u32)? as i32;
                if code != 0 && (v >> (code - 1)) == 0 {
                    v = (-1 << code | v) + 1;
                }
                color_data[i as usize + BLOCK_FILL_ORDER[index as usize] as usize] = v as i16;
                index += 1;
            }
            i += 64;
        }
        if self.bpp == 8 {
            self.decode_grayscale(&color_data, dst)?;
        } else {
            self.decode_rgb(&color_data, dst)?;
        }
        Ok(())
    }

    fn decode_rgb(&self, data: &[i16], mut dst: usize) -> Result<()> {
        let block_count = self.width / 8;
        for i in 0..block_count {
            let mut src = i as usize * 64;
            let mut yuv_blocks = [[0; 3]; 64];
            for channel in 0..3 {
                self.decode_dct(channel, data, src, &mut yuv_blocks)?;
                src += self.width as usize * 8;
            }
            let mut output = self
                .output
                .lock()
                .map_err(|e| anyhow::anyhow!("Failed to lock output: {}", e))?;
            for j in 0..64 {
                let cy = yuv_blocks[j][0] as f32;
                let cb = yuv_blocks[j][1] as f32;
                let cr = yuv_blocks[j][2] as f32;
                let r = cy + 1.402 * cr - 178.956;
                let g = cy - 0.34414 * cb - 0.71414 * cr + 135.95984;
                let b = cy + 1.772 * cb - 226.316;
                let y = j >> 3;
                let x = j & 7;
                let p = (y * self.width as usize + x) * 4 + dst;
                output[p] = Self::float_to_byte(b);
                output[p + 1] = Self::float_to_byte(g);
                output[p + 2] = Self::float_to_byte(r);
            }
            dst += 32;
        }
        Ok(())
    }

    fn decode_grayscale(&self, data: &[i16], mut dst: usize) -> Result<()> {
        let mut src = 0;
        let block_count = self.width / 8;
        for _ in 0..block_count {
            let mut yuv_blocks = [[0; 3]; 64];
            self.decode_dct(0, data, src, &mut yuv_blocks)?;
            src += 64;
            let mut output = self
                .output
                .lock()
                .map_err(|e| anyhow::anyhow!("Failed to lock output: {}", e))?;
            for j in 0usize..64 {
                let y = j >> 3;
                let x = j & 7;
                let p = (y * self.width as usize + x) * 4 + dst;
                output[p] = yuv_blocks[j][0] as u8;
                output[p + 1] = yuv_blocks[j][0] as u8;
                output[p + 2] = yuv_blocks[j][0] as u8;
            }
            dst += 32;
        }
        Ok(())
    }

    fn unpack_alpha(&self, offset: usize) -> Result<()> {
        let mut input = MemReaderRef::new(&self.input[offset..]);
        if input.read_i32()? != 1 {
            return Ok(());
        }
        let mut dst = 3;
        let mut ctl = 1 << 1;
        let mut output = self
            .output
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock output: {}", e))?;
        while dst < output.len() {
            ctl >>= 1;
            if ctl == 1 {
                ctl = (input.read_u8()? as i32) | 0x100;
            }
            if ctl & 1 != 0 {
                let v = input.read_u16()? as i32;
                let mut x = v & 0x3f;
                if x > 0x1f {
                    x |= -0x40;
                }
                let mut y = (v >> 6) & 7;
                if y != 0 {
                    y |= -8;
                }
                let count = ((v >> 9) & 0x7f) + 3;
                let mut src =
                    (dst as isize + (x as isize + y as isize * self.width as isize) * 4) as usize;
                if src >= dst {
                    return Ok(());
                }
                for _ in 0..count {
                    output[dst] = output[src];
                    src += 4;
                    dst += 4;
                }
            } else {
                output[dst] = input.read_u8()?;
                dst += 4;
            }
        }
        self.has_alpha.qsave(true);
        Ok(())
    }

    fn decode_dct(
        &self,
        channel: usize,
        data: &[i16],
        src: usize,
        output: &mut [[i16; 3]; 64],
    ) -> Result<()> {
        let mut v1;
        let mut v2;
        let mut v3;
        let mut v4;
        let mut v5;
        let mut v6;
        let mut v7;
        let mut v8;
        let mut v9;
        let mut v10;
        let mut v11;
        let mut v12;
        let mut v13;
        let mut v14;
        let mut v15;
        let mut v16;
        let mut v17;
        let d = if channel > 0 { 1 } else { 0 };
        let mut tmp = [[0f32; 8]; 8];
        for i in 0..8usize {
            if 0 == data[src + 8 + i]
                && 0 == data[src + 16 + i]
                && 0 == data[src + 24 + i]
                && 0 == data[src + 32 + i]
                && 0 == data[src + 40 + i]
                && 0 == data[src + 48 + i]
                && 0 == data[src + 56 + i]
            {
                let t = (data[src + i] as f32) * self.dct[d][i];
                tmp[0][i] = t;
                tmp[1][i] = t;
                tmp[2][i] = t;
                tmp[3][i] = t;
                tmp[4][i] = t;
                tmp[5][i] = t;
                tmp[6][i] = t;
                tmp[7][i] = t;
                continue;
            }
            v1 = (data[src + i] as f32) * self.dct[d][i];
            v2 = (data[src + 8 + i] as f32) * self.dct[d][i + 8];
            v3 = (data[src + 16 + i] as f32) * self.dct[d][i + 16];
            v4 = (data[src + 24 + i] as f32) * self.dct[d][i + 24];
            v5 = (data[src + 32 + i] as f32) * self.dct[d][i + 32];
            v6 = (data[src + 40 + i] as f32) * self.dct[d][i + 40];
            v7 = (data[src + 48 + i] as f32) * self.dct[d][i + 48];
            v8 = (data[src + 56 + i] as f32) * self.dct[d][i + 56];

            v10 = v1 + v5;
            v11 = v1 - v5;
            v12 = v3 + v7;
            v13 = (v3 - v7) * 1.414213562 - v12;
            v1 = v10 + v12;
            v7 = v10 - v12;
            v3 = v11 + v13;
            v5 = v11 - v13;
            v14 = v2 + v8;
            v15 = v2 - v8;
            v16 = v6 + v4;
            v17 = v6 - v4;
            v8 = v14 + v16;
            v11 = (v14 - v16) * 1.414213562;
            v9 = (v17 + v15) * 1.847759065;
            v10 = 1.082392200 * v15 - v9;
            v13 = -2.613125930 * v17 + v9;
            v6 = v13 - v8;
            v4 = v11 - v6;
            v2 = v10 + v4;
            tmp[0][i] = v1 + v8;
            tmp[1][i] = v3 + v6;
            tmp[2][i] = v5 + v4;
            tmp[3][i] = v7 - v2;
            tmp[4][i] = v7 + v2;
            tmp[5][i] = v5 - v4;
            tmp[6][i] = v3 - v6;
            tmp[7][i] = v1 - v8;
        }
        let mut dst = 0;
        for i in 0..8usize {
            v10 = tmp[i][0] + tmp[i][4];
            v11 = tmp[i][0] - tmp[i][4];
            v12 = tmp[i][2] + tmp[i][6];
            v13 = tmp[i][2] - tmp[i][6];
            v14 = tmp[i][1] + tmp[i][7];
            v15 = tmp[i][1] - tmp[i][7];
            v16 = tmp[i][5] + tmp[i][3];
            v17 = tmp[i][5] - tmp[i][3];

            v13 = 1.414213562 * v13 - v12;
            v1 = v10 + v12;
            v7 = v10 - v12;
            v3 = v11 + v13;
            v5 = v11 - v13;
            v8 = v14 + v16;
            v11 = (v14 - v16) * 1.414213562;
            v9 = (v15 + v17) * 1.847759065;
            v10 = v9 - v15 * 1.082392200;
            v13 = v9 - v17 * 2.613125930;
            v6 = v13 - v8;
            v4 = v11 - v6;
            v2 = v10 - v4;

            output[dst][channel] = Self::float_to_short(v1 + v8);
            output[dst + 1][channel] = Self::float_to_short(v3 + v6);
            output[dst + 2][channel] = Self::float_to_short(v5 + v4);
            output[dst + 3][channel] = Self::float_to_short(v7 + v2);
            output[dst + 4][channel] = Self::float_to_short(v7 - v2);
            output[dst + 5][channel] = Self::float_to_short(v5 - v4);
            output[dst + 6][channel] = Self::float_to_short(v3 - v6);
            output[dst + 7][channel] = Self::float_to_short(v1 - v8);
            dst += 8;
        }
        Ok(())
    }

    fn float_to_short(f: f32) -> i16 {
        let a = 0x80 + (f as i32 >> 3);
        if a <= 0 {
            0
        } else if a <= 0xff {
            a as i16
        } else if a < 0x180 {
            0xff
        } else {
            0
        }
    }

    fn float_to_byte(f: f32) -> u8 {
        if f >= 255.0 {
            0xff
        } else if f <= 0.0 {
            0
        } else {
            f as u8
        }
    }
}
