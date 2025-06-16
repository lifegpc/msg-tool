use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::bit_stream::*;
use crate::utils::struct_pack::*;
use anyhow::Ok;
use anyhow::Result;
use msg_tool_macro::*;
use std::io::{Read, Seek, Write};
use std::u32;

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
        let blue = ((pixel & 0x1F) << 3) as u8;
        let green = (((pixel >> 5) & 0x3F) << 2) as u8;
        let red = (((pixel >> 11) & 0x1F) << 3) as u8;

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
        let data = decoder.unpack()?;
        let color_type = match self.color_type {
            CbgColorType::Bgra32 => ImageColorType::Bgra,
            CbgColorType::Bgr24 => ImageColorType::Bgr,
            CbgColorType::Grayscale => ImageColorType::Grayscale,
            CbgColorType::Bgr565 => {
                return Ok(convert_bgr565_to_bgr24(
                    data,
                    self.header.width,
                    self.header.height,
                ));
            }
        };
        Ok(ImageData {
            width: self.header.width as u32,
            height: self.header.height as u32,
            color_type,
            depth: 8,
            data,
        })
    }
}

struct CbgDecoder<'a> {
    stream: MsbBitStream<'a>,
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

    fn unpack(mut self) -> Result<Vec<u8>> {
        self.stream.m_input.pos = 0x30;
        if self.info.version < 2 {
            return self.unpack_v1();
        }
        Err(anyhow::anyhow!("Unknown version: {}", self.info.version))
    }

    fn unpack_v1(&mut self) -> Result<Vec<u8>> {
        let leaf_nodes_weight = {
            let stream = MemReader::new(self.read_encoded()?);
            Self::read_weight_table(stream.to_ref(), 0x100)?
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
        Ok(output)
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

    fn read_weight_table(mut input: MemReaderRef<'_>, length: usize) -> Result<Vec<u32>> {
        let mut weights = Vec::with_capacity(length);
        for _ in 0..length {
            let weight = Self::read_int(&mut input)? as u32;
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

struct HuffmanNode {
    valid: bool,
    is_parent: bool,
    weight: u32,
    left_index: usize,
    right_index: usize,
}

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

    fn decode_token(&self, stream: &mut MsbBitStream<'_>) -> Result<usize> {
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
