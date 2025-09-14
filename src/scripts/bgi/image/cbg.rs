//! Buriko General Interpreter/Ethornell Compressed Image File
use crate::ext::atomic::*;
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::bit_stream::*;
use crate::utils::img::*;
use crate::utils::struct_pack::*;
use crate::utils::threadpool::*;
use anyhow::Result;
use msg_tool_macro::*;
use std::io::{Read, Seek, Write};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

#[derive(Debug)]
/// Builder for BGI Compressed Image scripts.
pub struct BgiCBGBuilder {}

impl BgiCBGBuilder {
    /// Creates a new instance of `BgiCBGBuilder`.
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
        _archive: Option<&Box<dyn Script>>,
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

    fn can_create_image_file(&self) -> bool {
        true
    }

    fn create_image_file<'a>(
        &'a self,
        data: ImageData,
        mut writer: Box<dyn WriteSeek + 'a>,
        _options: &ExtraConfig,
    ) -> Result<()> {
        let encoder = CbgEncoder::new(data)?;
        let data = encoder.encode()?;
        writer.write_all(&data)?;
        Ok(())
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
/// BGI Compressed Image script.
pub struct BgiCBG {
    header: BgiCBGHeader,
    data: MemReader,
    color_type: CbgColorType,
    decode_workers: usize,
}

impl BgiCBG {
    /// Creates a new instance of `BgiCBG` from a buffer.
    ///
    /// * `data` - The buffer containing the script data.
    /// * `config` - Extra configuration options.
    pub fn new(data: Vec<u8>, config: &ExtraConfig) -> Result<Self> {
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
            decode_workers: config.bgi_img_workers.max(1),
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
        let decoder = CbgDecoder::new(
            self.data.to_ref(),
            &self.header,
            self.color_type,
            self.decode_workers,
        )?;
        Ok(decoder.unpack()?)
    }

    fn import_image<'a>(
        &'a self,
        data: ImageData,
        mut file: Box<dyn WriteSeek + 'a>,
    ) -> Result<()> {
        let encoder = CbgEncoder::new(data)?;
        let encoded_data = encoder.encode()?;
        file.write_all(&encoded_data)?;
        Ok(())
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
    workers: usize,
}

impl<'a> CbgDecoder<'a> {
    fn new(
        reader: MemReaderRef<'a>,
        info: &'a BgiCBGHeader,
        color_type: CbgColorType,
        workers: usize,
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
            workers,
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

    fn unpack_v2(&mut self) -> Result<ImageData> {
        let dct_data = self.read_encoded()?;
        let mut dct = [[0.0f32; 64]; 2];
        for i in 0..0x80 {
            dct[i >> 6][i & 0x3f] = dct_data[i] as f32 * DCT_TABLE[i & 0x3f];
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

        let width = ((self.info.width as i32 + 7) & -8) as i32;
        let height = ((self.info.height as i32 + 7) & -8) as i32;
        let y_blocks = height / 8;

        let mut offsets = Vec::with_capacity((y_blocks + 1) as usize);
        let input_base =
            (self.stream.m_input.pos + ((y_blocks + 1) as usize * 4) - base_offset) as i32;

        for _ in 0..=y_blocks {
            let offset = self.stream.m_input.read_i32()?;
            offsets.push(offset - input_base);
        }

        let input = self.stream.m_input.data[self.stream.m_input.pos..].to_vec();
        let pad_skip = ((width >> 3) + 7) >> 3;

        let output_size = (width * height * 4) as usize;
        let output = vec![0u8; output_size];
        let output_mutex = Mutex::new(output);

        let decoder = Arc::new(ParallelCbgDecoder {
            input,
            output: output_mutex,
            bpp: self.info.bpp as i32,
            width,
            height,
            tree1,
            tree2,
            dct,
            has_alpha: AtomicBool::new(false),
        });

        let thread_pool = ThreadPool::new(self.workers, Some("cbg-decoder-worker-"), false)?;
        let mut dst = 0i32;

        for i in 0..y_blocks {
            let block_offset = offsets[i as usize] + pad_skip;
            let next_offset = if i + 1 == y_blocks {
                decoder.input.len() as i32
            } else {
                offsets[(i + 1) as usize]
            };
            let closure_dst = dst;
            let decoder_ref = Arc::clone(&decoder);

            thread_pool.execute(
                move |_| {
                    decoder_ref.unpack_block(block_offset, next_offset - block_offset, closure_dst)
                },
                true,
            )?;
            dst += width * 32;
        }

        if self.info.bpp == 32 {
            let decoder_ref = Arc::clone(&decoder);
            thread_pool.execute(
                move |_| decoder_ref.unpack_alpha(offsets[y_blocks as usize]),
                true,
            )?;
        }

        let tasks = thread_pool.into_results();

        for task in tasks {
            task?;
        }

        let has_alpha = decoder.has_alpha.qload();
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

        let color_type = if has_alpha {
            ImageColorType::Bgra
        } else {
            ImageColorType::Bgr
        };

        let img = ImageData {
            width: decoder.width as u32,
            height: decoder.height as u32,
            color_type,
            depth: 8,
            data: output,
        };

        if decoder.width != self.info.width as i32 || decoder.height != self.info.height as i32 {
            return Ok(draw_on_canvas(
                img,
                self.info.width as u32,
                self.info.height as u32,
                0,
                0,
            )?);
        }

        Ok(img)
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

    fn encode_token(&self, stream: &mut MsbBitWriter<impl Write>, token: usize) -> Result<()> {
        let mut path = Vec::new();
        if !self.find_path(self.nodes.len() - 1, token, &mut path) {
            return Err(anyhow::anyhow!("Token not found in Huffman tree"));
        }
        for &bit in path.iter().rev() {
            stream.put_bit(bit)?;
        }
        Ok(())
    }

    fn find_path(&self, node_index: usize, token: usize, path: &mut Vec<bool>) -> bool {
        if node_index == usize::MAX {
            return false;
        }
        let node = &self.nodes[node_index];
        if !node.is_parent {
            return node_index == token;
        }

        if self.find_path(node.left_index, token, path) {
            path.push(false);
            return true;
        }
        if self.find_path(node.right_index, token, path) {
            path.push(true);
            return true;
        }
        false
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
    bpp: i32,
    width: i32,
    height: i32,
    tree1: HuffmanTree,
    tree2: HuffmanTree,
    dct: [[f32; 64]; 2],
    has_alpha: AtomicBool,
}

impl ParallelCbgDecoder {
    fn unpack_block(&self, offset: i32, length: i32, dst: i32) -> Result<()> {
        let input = MemReaderRef::new(&self.input[offset as usize..(offset + length) as usize]);
        let mut reader = MsbBitStream::new(input);

        let block_size = CbgDecoder::read_int(&mut reader.m_input)?;
        if block_size == -1 {
            return Ok(());
        }

        let mut color_data = vec![0i16; block_size as usize];
        let mut acc = 0i32;
        let mut i = 0i32;

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

        if (reader.m_cached_bits & 7) != 0 {
            reader.get_bits(reader.m_cached_bits & 7)?;
        }

        i = 0;
        while i < block_size && reader.m_input.pos < reader.m_input.data.len() {
            let mut index = 1usize;
            while index < 64 && reader.m_input.pos < reader.m_input.data.len() {
                let code = self.tree2.decode_token(&mut reader)?;
                if code == 0 {
                    break;
                }
                if code == 0xf {
                    index += 0x10;
                    continue;
                }
                index += code & 0xf;
                if index >= BLOCK_FILL_ORDER.len() {
                    break;
                }
                let bits = code >> 4;
                let mut v = reader.get_bits(bits as u32)? as i32;
                if bits != 0 && (v >> (bits - 1)) == 0 {
                    v = (-1 << bits | v) + 1;
                }
                color_data[i as usize + BLOCK_FILL_ORDER[index] as usize] = v as i16;
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

    fn decode_rgb(&self, data: &[i16], dst: i32) -> Result<()> {
        let block_count = self.width / 8;
        let mut dst = dst as usize;

        for i in 0..block_count {
            let mut src = (i * 64) as usize;
            let mut ycbcr_block = [[0i16; 3]; 64];

            for channel in 0..3 {
                self.decode_dct(channel, data, src, &mut ycbcr_block)?;
                src += (self.width * 8) as usize;
            }

            let mut output = self
                .output
                .lock()
                .map_err(|e| anyhow::anyhow!("Failed to lock output: {}", e))?;

            for j in 0..64 {
                let cy = ycbcr_block[j][0] as f32;
                let cb = ycbcr_block[j][1] as f32;
                let cr = ycbcr_block[j][2] as f32;

                // Full-range YCbCr->RGB conversion
                let r = cy + 1.402f32 * cr - 178.956f32;
                let g = cy - 0.34414f32 * cb - 0.71414f32 * cr + 135.95984f32;
                let b = cy + 1.772f32 * cb - 226.316f32;

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

    fn decode_grayscale(&self, data: &[i16], dst: i32) -> Result<()> {
        let mut src = 0;
        let block_count = self.width / 8;
        let mut dst = dst as usize;

        for _ in 0..block_count {
            let mut ycbcr_block = [[0i16; 3]; 64];
            self.decode_dct(0, data, src, &mut ycbcr_block)?;
            src += 64;

            let mut output = self
                .output
                .lock()
                .map_err(|e| anyhow::anyhow!("Failed to lock output: {}", e))?;

            for j in 0..64 {
                let y = j >> 3;
                let x = j & 7;
                let p = (y * self.width as usize + x) * 4 + dst;
                let value = ycbcr_block[j][0] as u8;

                output[p] = value;
                output[p + 1] = value;
                output[p + 2] = value;
            }
            dst += 32;
        }
        Ok(())
    }

    fn unpack_alpha(&self, offset: i32) -> Result<()> {
        let mut input = MemReaderRef::new(&self.input[offset as usize..]);

        if input.read_i32()? != 1 {
            return Ok(());
        }

        let mut dst = 3;
        let mut ctl = 1i32 << 1;

        let mut output = self
            .output
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock output: {}", e))?;

        while dst < output.len() {
            ctl >>= 1;
            if ctl == 1 {
                ctl = (input.read_u8()? as i32) | 0x100;
            }

            if (ctl & 1) != 0 {
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

                let src = dst as isize + (x as isize + y as isize * self.width as isize) * 4;
                if src < 0 || src >= dst as isize {
                    return Ok(());
                }

                let mut src = src as usize;
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
        let d = if channel > 0 { 1 } else { 0 };
        let mut tmp = [[0f32; 8]; 8];

        for i in 0..8 {
            // Check if all AC coefficients are zero
            if data[src + 8 + i] == 0
                && data[src + 16 + i] == 0
                && data[src + 24 + i] == 0
                && data[src + 32 + i] == 0
                && data[src + 40 + i] == 0
                && data[src + 48 + i] == 0
                && data[src + 56 + i] == 0
            {
                let t = data[src + i] as f32 * self.dct[d][i];
                for row in 0..8 {
                    tmp[row][i] = t;
                }
                continue;
            }

            let v1 = data[src + i] as f32 * self.dct[d][i];
            let v2 = data[src + 8 + i] as f32 * self.dct[d][8 + i];
            let v3 = data[src + 16 + i] as f32 * self.dct[d][16 + i];
            let v4 = data[src + 24 + i] as f32 * self.dct[d][24 + i];
            let v5 = data[src + 32 + i] as f32 * self.dct[d][32 + i];
            let v6 = data[src + 40 + i] as f32 * self.dct[d][40 + i];
            let v7 = data[src + 48 + i] as f32 * self.dct[d][48 + i];
            let v8 = data[src + 56 + i] as f32 * self.dct[d][56 + i];

            let v10 = v1 + v5;
            let v11 = v1 - v5;
            let v12 = v3 + v7;
            let v13 = (v3 - v7) * 1.414213562f32 - v12;
            let v1 = v10 + v12;
            let v7 = v10 - v12;
            let v3 = v11 + v13;
            let v5 = v11 - v13;
            let v14 = v2 + v8;
            let v15 = v2 - v8;
            let v16 = v6 + v4;
            let v17 = v6 - v4;
            let v8 = v14 + v16;
            let v11 = (v14 - v16) * 1.414213562f32;
            let v9 = (v17 + v15) * 1.847759065f32;
            let v10 = 1.082392200f32 * v15 - v9;
            let v13 = -2.613125930f32 * v17 + v9;
            let v6 = v13 - v8;
            let v4 = v11 - v6;
            let v2 = v10 + v4;

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
        for i in 0..8 {
            let v10 = tmp[i][0] + tmp[i][4];
            let v11 = tmp[i][0] - tmp[i][4];
            let v12 = tmp[i][2] + tmp[i][6];
            let v13 = tmp[i][2] - tmp[i][6];
            let v14 = tmp[i][1] + tmp[i][7];
            let v15 = tmp[i][1] - tmp[i][7];
            let v16 = tmp[i][5] + tmp[i][3];
            let v17 = tmp[i][5] - tmp[i][3];

            let v13 = 1.414213562f32 * v13 - v12;
            let v1 = v10 + v12;
            let v7 = v10 - v12;
            let v3 = v11 + v13;
            let v5 = v11 - v13;
            let v8 = v14 + v16;
            let v11 = (v14 - v16) * 1.414213562f32;
            let v9 = (v17 + v15) * 1.847759065f32;
            let v10 = v9 - v15 * 1.082392200f32;
            let v13 = v9 - v17 * 2.613125930f32;
            let v6 = v13 - v8;
            let v4 = v11 - v6;
            let v2 = v10 - v4;

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
        let a = 0x80 + ((f as i32) >> 3);
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

struct CbgEncoder {
    header: BgiCBGHeader,
    stream: MemWriter,
    img: ImageData,
    key: u32,
    magic: u32,
}

impl CbgEncoder {
    pub fn new(mut img: ImageData) -> Result<Self> {
        if img.depth != 8 {
            return Err(anyhow::anyhow!("Unsupported image depth: {}", img.depth));
        }
        let bpp = match img.color_type {
            ImageColorType::Bgr => 24,
            ImageColorType::Bgra => 32,
            ImageColorType::Grayscale => 8,
            ImageColorType::Rgb => {
                convert_rgb_to_bgr(&mut img)?;
                24
            }
            ImageColorType::Rgba => {
                convert_rgba_to_bgra(&mut img)?;
                32
            }
        };
        let key = rand::random();
        let header = BgiCBGHeader {
            width: img.width as u16,
            height: img.height as u16,
            bpp,
            _unk: 0,
            intermediate_length: 0,
            key,
            enc_length: 0,
            check_sum: 0,
            check_xor: 0,
            version: 1,
        };

        Ok(CbgEncoder {
            header,
            stream: MemWriter::new(),
            img,
            key,
            magic: 0,
        })
    }

    pub fn encode(mut self) -> Result<Vec<u8>> {
        self.stream.write_all(b"CompressedBG___\0")?;
        let header_pos = self.stream.pos;
        self.stream.seek(std::io::SeekFrom::Current(0x20))?;

        let pixel_size = (self.header.bpp / 8) as usize;
        let stride = self.header.width as usize * pixel_size;
        let mut sampled_data = self.img.data.clone();
        self.average_sampling(&mut sampled_data, stride, pixel_size);

        let packed_data = Self::pack_zeros(&sampled_data);
        self.header.intermediate_length = packed_data.len() as u32;

        let mut frequencies = vec![0u32; 256];
        for &byte in &packed_data {
            frequencies[byte as usize] += 1;
        }
        if frequencies.iter().all(|&f| f == 0) {
            frequencies[0] = 1;
        }

        let tree = HuffmanTree::new(&frequencies, false);

        let mut weight_writer = MemWriter::new();
        for &weight in &frequencies {
            Self::write_int(&mut weight_writer, weight as i32)?;
        }
        let weight_data = weight_writer.into_inner();
        self.write_encoded(&weight_data)?;

        let mut bit_writer = MsbBitWriter::new(&mut self.stream);
        for &byte in &packed_data {
            tree.encode_token(&mut bit_writer, byte as usize)?;
        }
        bit_writer.flush()?;

        let final_pos = self.stream.pos;
        self.stream.pos = header_pos;
        self.header.pack(&mut self.stream, false, Encoding::Cp932)?;
        self.stream.pos = final_pos;

        Ok(self.stream.into_inner())
    }

    fn average_sampling(&self, data: &mut [u8], stride: usize, pixel_size: usize) {
        for y in (0..self.header.height as usize).rev() {
            let line = y * stride;
            for x in (0..self.header.width as usize).rev() {
                let pixel = line + x * pixel_size;
                for p in 0..pixel_size {
                    let mut avg = 0u32;
                    let mut count = 0;
                    if x > 0 {
                        avg = avg.wrapping_add(data[pixel + p - pixel_size] as u32);
                        count += 1;
                    }
                    if y > 0 {
                        avg = avg.wrapping_add(data[pixel + p - stride] as u32);
                        count += 1;
                    }
                    if count > 0 {
                        avg /= count;
                    }
                    if avg != 0 {
                        data[pixel + p] = data[pixel + p].wrapping_sub(avg as u8);
                    }
                }
            }
        }
    }

    fn pack_zeros(input: &[u8]) -> Vec<u8> {
        let mut output = Vec::new();
        let mut i = 0;
        let mut is_zero_run = false;

        while i < input.len() {
            let mut count = 0;
            if is_zero_run {
                while i + count < input.len() && input[i + count] == 0 {
                    count += 1;
                }
            } else {
                while i + count < input.len() && input[i + count] != 0 {
                    count += 1;
                }
            }

            let mut count_buf = Vec::new();
            let mut n = count;
            loop {
                let mut byte = (n & 0x7f) as u8;
                n >>= 7;
                if n > 0 {
                    byte |= 0x80;
                }
                count_buf.push(byte);
                if n == 0 {
                    break;
                }
            }
            output.extend_from_slice(&count_buf);

            if !is_zero_run {
                output.extend_from_slice(&input[i..i + count]);
            }
            i += count;
            is_zero_run = !is_zero_run;
        }
        output
    }

    fn write_int<W: Write>(writer: &mut W, mut value: i32) -> Result<()> {
        loop {
            let mut b = (value as u8) & 0x7f;
            value >>= 7;
            if value != 0 {
                b |= 0x80;
            }
            writer.write_u8(b)?;
            if value == 0 {
                break;
            }
        }
        Ok(())
    }

    fn write_encoded(&mut self, data: &[u8]) -> Result<()> {
        self.header.enc_length = data.len() as u32;
        let mut sum = 0u8;
        let mut xor = 0u8;
        let mut encoded_data = Vec::with_capacity(data.len());
        for &byte in data {
            let encrypted_byte = byte.wrapping_add(self.update_key());
            sum = sum.wrapping_add(byte);
            xor ^= byte;
            encoded_data.push(encrypted_byte);
        }
        self.header.check_sum = sum;
        self.header.check_xor = xor;
        self.stream.write_all(&encoded_data)?;
        Ok(())
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
