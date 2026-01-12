//! Buriko General Interpreter/Ethornell compressed file in archive
use crate::ext::io::*;
use crate::ext::vec::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::bit_stream::*;
use crate::utils::num_range::*;
use anyhow::Result;
use rand::Rng;
use std::collections::BinaryHeap;
use std::io::{Seek, Write};

#[derive(Debug)]
struct HuffmanCode {
    code: u16,
    depth: u8,
}

impl std::cmp::PartialEq for HuffmanCode {
    fn eq(&self, other: &Self) -> bool {
        self.code == other.code && self.depth == other.depth
    }
}

impl std::cmp::Eq for HuffmanCode {}

impl std::cmp::PartialOrd for HuffmanCode {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        let cmp = self.depth.cmp(&other.depth);
        if cmp == std::cmp::Ordering::Equal {
            Some(self.code.cmp(&other.code))
        } else {
            Some(cmp)
        }
    }
}

impl std::cmp::Ord for HuffmanCode {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let cmp = self.depth.cmp(&other.depth);
        if cmp == std::cmp::Ordering::Equal {
            self.code.cmp(&other.code)
        } else {
            cmp
        }
    }
}

#[derive(Clone, Debug)]
struct HuffmanNode {
    is_parent: bool,
    code: Option<u16>,
    left_index: usize,
    right_index: usize,
}

/// Decoder for Buriko General Interpreter/Ethornell compressed files (DSC format).
pub struct DscDecoder<'a> {
    stream: MsbBitStream<MemReaderRef<'a>>,
    key: u32,
    magic: u32,
    output_size: u32,
    dec_count: u32,
}

impl<'a> DscDecoder<'a> {
    /// Creates a new DscDecoder from the given data slice.
    pub fn new(data: &'a [u8]) -> Result<Self> {
        let mut reader = MemReaderRef::new(data);
        let magic = (reader.read_u16()? as u32) << 16;
        reader.pos = 0x10;
        let key = reader.read_u32()?;
        let output_size = reader.read_u32()?;
        let dec_count = reader.read_u32()?;
        let stream = MsbBitStream::new(reader);
        Ok(DscDecoder {
            stream,
            key,
            magic,
            output_size,
            dec_count,
        })
    }

    /// Unpacks the DSC file and returns the decompressed data.
    pub fn unpack(mut self) -> Result<Vec<u8>> {
        self.stream.m_input.pos = 0x20;
        let mut codes = Vec::new();
        for i in 0..512 {
            let src = self.stream.m_input.read_u8()?;
            let depth = src.overflowing_sub(self.update_key()).0;
            if depth > 0 {
                codes.push(HuffmanCode { code: i, depth })
            }
        }
        codes.sort();
        let root = Self::create_huffman_tree(codes);
        self.huffman_decompress(root)
    }

    fn create_huffman_tree(codes: Vec<HuffmanCode>) -> Vec<HuffmanNode> {
        let mut trees = Vec::with_capacity(1024);
        trees.resize(
            1024,
            HuffmanNode {
                is_parent: false,
                code: None,
                left_index: 0,
                right_index: 0,
            },
        );
        let mut left_index = vec![0usize; 512];
        let mut right_index = vec![0usize; 512];
        let mut next_node_index = 1usize;
        let mut depth_nodes = 1usize;
        let mut depth = 0u8;
        let mut left_child = true;
        let mut n = 0;
        while n < codes.len() {
            let huffman_node_index = left_child;
            left_child = !left_child;
            let mut depth_existed_nodes = 0;
            while n < codes.len() && codes[n].depth == depth {
                let index = if huffman_node_index {
                    left_index[depth_existed_nodes]
                } else {
                    right_index[depth_existed_nodes]
                };
                trees[index].code = Some(codes[n].code);
                n += 1;
                depth_existed_nodes += 1;
            }
            let depth_nodes_to_create = depth_nodes - depth_existed_nodes;
            for i in 0..depth_nodes_to_create {
                let index = if huffman_node_index {
                    left_index[depth_existed_nodes + i]
                } else {
                    right_index[depth_existed_nodes + i]
                };
                let node = &mut trees[index];
                node.is_parent = true;
                if left_child {
                    left_index[i * 2] = next_node_index;
                    node.left_index = next_node_index;
                    next_node_index += 1;
                    left_index[i * 2 + 1] = next_node_index;
                    node.right_index = next_node_index;
                    next_node_index += 1;
                } else {
                    right_index[i * 2] = next_node_index;
                    node.left_index = next_node_index;
                    next_node_index += 1;
                    right_index[i * 2 + 1] = next_node_index;
                    node.right_index = next_node_index;
                    next_node_index += 1;
                }
            }
            depth += 1;
            depth_nodes = depth_nodes_to_create * 2;
        }
        trees
    }

    fn huffman_decompress(&mut self, nodes: Vec<HuffmanNode>) -> Result<Vec<u8>> {
        let output_size = self.output_size as usize;
        let mut output = Vec::with_capacity(output_size);
        let mut dst = 0;
        output.resize(output_size, 0);
        for _ in 0..self.dec_count {
            let mut current_node = &nodes[0];
            loop {
                let bit = self.stream.get_next_bit()?;
                if !bit {
                    current_node = &nodes[current_node.left_index]
                } else {
                    current_node = &nodes[current_node.right_index]
                }
                if !current_node.is_parent {
                    break;
                }
            }
            let code = *current_node.code.as_ref().unwrap();
            if code >= 256 {
                let mut offset = self.stream.get_bits(12)?;
                let count = ((code & 0xFF) + 2) as usize;
                offset += 2;
                output.copy_overlapped(dst - offset as usize, dst, count);
                dst += count;
            } else {
                output[dst] = code as u8;
                dst += 1;
            }
        }
        if dst != output_size {
            eprintln!(
                "Warning: Output size mismatch, expected {}, got {}",
                self.output_size, dst
            );
            crate::COUNTER.inc_warning();
        }
        Ok(output)
    }

    fn update_key(&mut self) -> u8 {
        let v0 = 20021 * (self.key & 0xffff);
        let mut v1 = self.magic | (self.key >> 16);
        v1 = v1
            .overflowing_mul(20021)
            .0
            .overflowing_add(self.key.overflowing_mul(346).0)
            .0;
        v1 = overf::wrapping!(v1 + (v0 >> 16)) & 0xffff;
        self.key = (v1 << 16) + (v0 & 0xffff) + 1;
        v1 as u8
    }
}

#[derive(Debug, Clone, Copy)]
enum LzssOp {
    Literal(u8),
    Match { len: u16, offset: u16 },
}

#[derive(Debug)]
struct FreqNode {
    freq: u32,
    symbol: Option<u16>,
    left: Option<Box<FreqNode>>,
    right: Option<Box<FreqNode>>,
}
impl PartialEq for FreqNode {
    fn eq(&self, other: &Self) -> bool {
        self.freq == other.freq
    }
}
impl Eq for FreqNode {}
impl PartialOrd for FreqNode {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for FreqNode {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.freq.cmp(&self.freq)
    }
}

fn calculate_huffman_depths(freqs: &[u32]) -> Vec<u8> {
    const MAX_DEPTH: u8 = 9;

    // 收集所有非零频率的符号
    let mut symbols_with_freq: Vec<(u16, u32)> = freqs
        .iter()
        .enumerate()
        .filter_map(|(symbol, &freq)| {
            if freq > 0 {
                Some((symbol as u16, freq))
            } else {
                None
            }
        })
        .collect();

    let mut depths = vec![0u8; 512];

    if symbols_with_freq.is_empty() {
        return depths;
    }

    if symbols_with_freq.len() == 1 {
        depths[symbols_with_freq[0].0 as usize] = 1;
        return depths;
    }

    // 使用受限Huffman算法
    loop {
        let current_depths = build_huffman_tree(&symbols_with_freq);
        let max_depth = current_depths.iter().max().copied().unwrap_or(0);

        if max_depth <= MAX_DEPTH {
            // 将深度映射回原始数组
            for &(symbol, _) in &symbols_with_freq {
                let symbol_index = symbols_with_freq
                    .iter()
                    .position(|(s, _)| *s == symbol)
                    .unwrap();
                depths[symbol as usize] = current_depths[symbol_index];
            }
            break;
        }

        // 如果深度超限，调整频率
        adjust_frequencies_for_depth_limit(&mut symbols_with_freq);
    }

    depths
}

fn build_huffman_tree(symbols_with_freq: &[(u16, u32)]) -> Vec<u8> {
    let mut heap = BinaryHeap::new();

    // 添加所有叶子节点
    for &(symbol, freq) in symbols_with_freq {
        heap.push(FreqNode {
            freq,
            symbol: Some(symbol),
            left: None,
            right: None,
        });
    }

    // 构建Huffman树
    while heap.len() > 1 {
        let node1 = heap.pop().unwrap();
        let node2 = heap.pop().unwrap();
        let new_node = FreqNode {
            freq: node1.freq + node2.freq,
            symbol: None,
            left: Some(Box::new(node1)),
            right: Some(Box::new(node2)),
        };
        heap.push(new_node);
    }

    // 计算深度
    let mut depths = vec![0u8; symbols_with_freq.len()];
    if let Some(root) = heap.pop() {
        calculate_depths(&root, 0, symbols_with_freq, &mut depths);
    }

    depths
}

fn calculate_depths(
    node: &FreqNode,
    depth: u8,
    symbols_with_freq: &[(u16, u32)],
    depths: &mut [u8],
) {
    if let Some(symbol) = node.symbol {
        let symbol_index = symbols_with_freq
            .iter()
            .position(|(s, _)| *s == symbol)
            .unwrap();
        depths[symbol_index] = if depth == 0 { 1 } else { depth };
    } else {
        if let Some(ref left) = node.left {
            calculate_depths(left, depth + 1, symbols_with_freq, depths);
        }
        if let Some(ref right) = node.right {
            calculate_depths(right, depth + 1, symbols_with_freq, depths);
        }
    }
}

fn adjust_frequencies_for_depth_limit(symbols_with_freq: &mut [(u16, u32)]) {
    // 按频率排序
    symbols_with_freq.sort_by(|a, b| a.1.cmp(&b.1));

    // 使用Package-Merge算法的简化版本
    // 这里使用一个启发式方法：增加低频符号的频率
    let min_freq = symbols_with_freq[0].1;
    let adjustment = (min_freq as f64 * 0.1).max(1.0) as u32;

    // 找到频率最低的几个符号并调整它们的频率
    let num_to_adjust = (symbols_with_freq.len() / 4).max(1);
    for i in 0..num_to_adjust.min(symbols_with_freq.len()) {
        symbols_with_freq[i].1 += adjustment;
    }
}

fn generate_canonical_codes(depths: &[u8]) -> Vec<Option<(u16, u8)>> {
    let mut codes_with_depths = vec![];
    for (symbol, &depth) in depths.iter().enumerate() {
        if depth > 0 {
            codes_with_depths.push((symbol as u16, depth));
        }
    }
    codes_with_depths.sort_by(|a, b| {
        let depth_cmp = a.1.cmp(&b.1);
        if depth_cmp == std::cmp::Ordering::Equal {
            a.0.cmp(&b.0)
        } else {
            depth_cmp
        }
    });

    let mut huffman_codes = vec![None; 512];
    let mut current_code = 0u16;
    let mut last_depth = 0u8;

    for &(symbol, depth) in &codes_with_depths {
        if last_depth != 0 {
            current_code <<= depth - last_depth;
        }
        huffman_codes[symbol as usize] = Some((current_code, depth));
        current_code += 1;
        last_depth = depth;
    }

    huffman_codes
}

/// Encoder for Buriko General Interpreter/Ethornell compressed files (DSC format).
pub struct DscEncoder<'a, T: Write + Seek> {
    stream: MsbBitWriter<'a, T>,
    magic: u32,
    key: u32,
    dec_count: u32,
    min_len: usize,
}

impl<'a, T: Write + Seek> DscEncoder<'a, T> {
    /// Creates a new DscEncoder with the given writer and minimum length for LZSS compression.
    pub fn new(writer: &'a mut T, min_len: usize) -> Self {
        let stream = MsbBitWriter::new(writer);
        DscEncoder {
            stream,
            magic: 0x5344 << 16, // "DS"
            key: rand::rng().random(),
            dec_count: 0,
            min_len,
        }
    }

    /// Packs the given data into the DSC format using LZSS compression.
    pub fn pack(mut self, data: &[u8]) -> Result<()> {
        // LZSS compression
        let mut ops = vec![];
        let mut pos = 0;

        const MAX_LEN: usize = 257;
        const WINDOW_SIZE: usize = 4097;

        let mut head: Vec<i32> = vec![-1; 1 << 16];
        let mut prev: Vec<i32> = vec![-1; data.len()];

        while pos < data.len() {
            let max_len = (data.len() - pos).min(MAX_LEN);
            let mut best_len = 0;
            let mut best_offset = 0;

            if max_len >= self.min_len {
                let limit = pos.saturating_sub(WINDOW_SIZE);
                let key = (data[pos] as u16) << 8 | data[pos + 1] as u16;
                let mut match_pos_i32 = head[key as usize];

                while match_pos_i32 != -1 {
                    let match_pos = match_pos_i32 as usize;
                    if match_pos < limit {
                        break;
                    }

                    if data.get(match_pos + best_len) == data.get(pos + best_len) {
                        let mut current_len = 0;
                        for i in 0..max_len {
                            if data.get(pos + i) != data.get(match_pos + i) {
                                break;
                            }
                            current_len += 1;
                        }

                        if current_len > best_len {
                            best_len = current_len;
                            best_offset = pos - match_pos;
                            if best_len >= max_len {
                                break;
                            }
                        }
                    }
                    match_pos_i32 = prev[match_pos];
                }
            }

            if best_len >= self.min_len && best_offset >= 2 {
                ops.push(LzssOp::Match {
                    len: best_len as u16,
                    offset: best_offset as u16,
                });
                for i in 0..best_len {
                    if pos + i + 1 < data.len() {
                        let key = (data[pos + i] as u16) << 8 | data[pos + i + 1] as u16;
                        let current_pos = pos + i;
                        prev[current_pos] = head[key as usize];
                        head[key as usize] = current_pos as i32;
                    }
                }
                pos += best_len;
            } else {
                ops.push(LzssOp::Literal(data[pos]));
                if pos + 1 < data.len() {
                    let key = (data[pos] as u16) << 8 | data[pos + 1] as u16;
                    prev[pos] = head[key as usize];
                    head[key as usize] = pos as i32;
                }
                pos += 1;
            }
        }

        let symbols: Vec<u16> = ops
            .iter()
            .map(|op| match op {
                LzssOp::Literal(byte) => *byte as u16,
                LzssOp::Match { len, .. } => 256 + (len - 2),
            })
            .collect();
        self.dec_count = symbols.len() as u32;

        let mut freqs = vec![0u32; 512];
        for &s in &symbols {
            freqs[s as usize] += 1;
        }

        let depths = calculate_huffman_depths(&freqs);
        let huffman_codes = generate_canonical_codes(&depths);

        self.stream.writer.write_all(b"DSC FORMAT 1.00\0")?;
        self.stream.writer.seek(std::io::SeekFrom::Start(0x10))?;
        self.stream.writer.write_u32(self.key)?;
        self.stream.writer.write_u32(data.len() as u32)?;
        self.stream.writer.write_u32(self.dec_count)?;
        self.stream.writer.seek(std::io::SeekFrom::Start(0x20))?;

        for depth in depths.iter() {
            let key = self.update_key();
            self.stream.writer.write_u8(depth.overflowing_add(key).0)?;
        }

        for op in &ops {
            match op {
                LzssOp::Literal(byte) => {
                    let symbol = *byte as u16;
                    let (code, len) = huffman_codes[symbol as usize].unwrap();
                    self.stream.put_bits(code as u32, len)?;
                }
                LzssOp::Match { len, offset } => {
                    let symbol = 256 + (len - 2);
                    let (code, huff_len) = huffman_codes[symbol as usize].unwrap();
                    self.stream.put_bits(code as u32, huff_len)?;
                    self.stream.put_bits((*offset - 2) as u32, 12)?;
                }
            }
        }
        self.stream.flush()?;
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

#[derive(Debug)]
/// Builder for DSC scripts.
pub struct DscBuilder {}

impl DscBuilder {
    /// Creates a new instance of `DscBuilder`.
    pub fn new() -> Self {
        DscBuilder {}
    }
}

impl ScriptBuilder for DscBuilder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Cp932
    }

    fn default_archive_encoding(&self) -> Option<Encoding> {
        Some(Encoding::Cp932)
    }

    fn build_script(
        &self,
        buf: Vec<u8>,
        _filename: &str,
        _encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(Dsc::new(buf, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::BGIDsc
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 16 && buf.starts_with(b"DSC FORMAT 1.00\0") {
            return Some(255);
        }
        None
    }

    fn can_create_file(&self) -> bool {
        true
    }

    fn create_file<'a>(
        &'a self,
        filename: &'a str,
        mut writer: Box<dyn WriteSeek + 'a>,
        _encoding: Encoding,
        _file_encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<()> {
        let encoder = DscEncoder::new(&mut writer, config.bgi_compress_min_len);
        let data = crate::utils::files::read_file(filename)?;
        encoder.pack(&data)?;
        Ok(())
    }
}

#[derive(Debug)]
/// DSC script
pub struct Dsc {
    data: Vec<u8>,
    min_len: usize,
}

impl Dsc {
    /// Creates a new Dsc script
    ///
    /// * `buf` - The buffer containing the DSC data.
    /// * `config` - Extra configuration options.
    pub fn new(buf: Vec<u8>, config: &ExtraConfig) -> Result<Self> {
        if buf.len() < 16 || !buf.starts_with(b"DSC FORMAT 1.00\0") {
            return Err(anyhow::anyhow!("Invalid DSC format"));
        }
        let decoder = DscDecoder::new(&buf)?;
        let data = decoder.unpack()?;
        Ok(Dsc {
            data,
            min_len: config.bgi_compress_min_len,
        })
    }
}

impl Script for Dsc {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Custom
    }

    fn is_output_supported(&self, output: OutputScriptType) -> bool {
        matches!(output, OutputScriptType::Custom)
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn custom_output_extension(&self) -> &'static str {
        ""
    }

    fn custom_export(&self, filename: &std::path::Path, _encoding: Encoding) -> Result<()> {
        let mut f = std::fs::File::create(filename)?;
        f.write_all(&self.data)?;
        Ok(())
    }

    fn custom_import<'a>(
        &'a self,
        custom_filename: &'a str,
        mut file: Box<dyn WriteSeek + 'a>,
        _encoding: Encoding,
        _output_encoding: Encoding,
    ) -> Result<()> {
        let encoder = DscEncoder::new(&mut file, self.min_len);
        let data = crate::utils::files::read_file(custom_filename)?;
        encoder.pack(&data)?;
        Ok(())
    }
}

/// Parses the minimum length for LZSS compression from a string.
pub fn parse_min_length(len: &str) -> Result<usize, String> {
    number_range(len, 2, 256)
}
