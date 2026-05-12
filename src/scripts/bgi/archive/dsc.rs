//! Buriko General Interpreter/Ethornell compressed file in archive
use crate::ext::io::*;
use crate::ext::vec::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::bit_stream::*;
use crate::utils::num_range::*;
use anyhow::Result;
use rand::RngExt;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchMode {
    Store,
    Rle,
    NonLazy,
    Lazy,
}

#[derive(Debug, Clone, Copy)]
pub struct CompressConfig {
    pub good_length: usize,
    pub max_lazy: usize,
    pub nice_length: usize,
    pub max_chain: usize,
    pub mode: MatchMode,
}

pub const COMPRESS_CONFIGS: [CompressConfig; 10] = [
    // 0: Store (No compression)
    CompressConfig {
        good_length: 0,
        max_lazy: 0,
        nice_length: 0,
        max_chain: 0,
        mode: MatchMode::Store,
    },
    // 1: RLE (Fastest) - Matches repeated patterns directly
    CompressConfig {
        good_length: 4,
        max_lazy: 0,
        nice_length: 8,
        max_chain: 0,
        mode: MatchMode::Rle,
    },
    // 2: RLE
    CompressConfig {
        good_length: 4,
        max_lazy: 0,
        nice_length: 16,
        max_chain: 0,
        mode: MatchMode::Rle,
    },
    // 3: Non-lazy match
    CompressConfig {
        good_length: 4,
        max_lazy: 0,
        nice_length: 32,
        max_chain: 8,
        mode: MatchMode::NonLazy,
    },
    // 4: Non-lazy match
    CompressConfig {
        good_length: 4,
        max_lazy: 0,
        nice_length: 64,
        max_chain: 16,
        mode: MatchMode::NonLazy,
    },
    // 5: Lazy match
    CompressConfig {
        good_length: 8,
        max_lazy: 16,
        nice_length: 32,
        max_chain: 32,
        mode: MatchMode::Lazy,
    },
    // 6: Lazy match
    CompressConfig {
        good_length: 8,
        max_lazy: 16,
        nice_length: 128,
        max_chain: 128,
        mode: MatchMode::Lazy,
    },
    // 7: Lazy match
    CompressConfig {
        good_length: 8,
        max_lazy: 32,
        nice_length: 128,
        max_chain: 256,
        mode: MatchMode::Lazy,
    },
    // 8: Lazy match
    CompressConfig {
        good_length: 32,
        max_lazy: 128,
        nice_length: 258,
        max_chain: 1024,
        mode: MatchMode::Lazy,
    },
    // 9: Lazy match (Best)
    CompressConfig {
        good_length: 32,
        max_lazy: 258,
        nice_length: 258,
        max_chain: 4096,
        mode: MatchMode::Lazy,
    },
];

/// Computes optimal length-limited Huffman code depths using the Package-Merge algorithm.
fn package_merge(freqs: &[u32], max_len: u8) -> Vec<u8> {
    let max_len = max_len as usize;
    let mut depths = vec![0u8; freqs.len()];

    let mut symbols: Vec<(u64, Vec<usize>)> = freqs
        .iter()
        .enumerate()
        .filter(|&(_, &f)| f > 0)
        .map(|(i, &f)| (f as u64, vec![i]))
        .collect();

    let n = symbols.len();
    if n == 0 {
        return depths;
    }
    if n == 1 {
        depths[symbols[0].1[0]] = 1;
        return depths;
    }

    symbols.sort_by_key(|x| x.0);
    let mut prev_list = symbols.clone();

    for _ in 1..max_len {
        let mut current_list = symbols.clone();
        for p in 0..(prev_list.len() / 2) {
            let left = &prev_list[p * 2];
            let right = &prev_list[p * 2 + 1];

            let combined_weight = left.0 + right.0;
            let mut combined_indices = Vec::with_capacity(left.1.len() + right.1.len());
            combined_indices.extend_from_slice(&left.1);
            combined_indices.extend_from_slice(&right.1);

            current_list.push((combined_weight, combined_indices));
        }
        current_list.sort_by_key(|x| x.0);
        prev_list = current_list;
    }

    let items_to_select = (2 * n).saturating_sub(2);
    let take_count = std::cmp::min(items_to_select, prev_list.len());

    for i in 0..take_count {
        for &sym in &prev_list[i].1 {
            depths[sym] += 1;
        }
    }
    depths
}

fn calculate_huffman_depths(freqs: &[u32]) -> Vec<u8> {
    const MAX_DEPTH: u8 = 9;
    package_merge(freqs, MAX_DEPTH)
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

#[inline(always)]
fn find_match(
    data: &[u8],
    pos: usize,
    head: &[i32],
    prev: &[i32],
    config: &CompressConfig,
) -> (usize, usize) {
    if config.mode == MatchMode::Store || pos + 2 > data.len() {
        return (0, 0);
    }

    let max_len = (data.len() - pos).min(257);

    // 低等级 RLE: 仅扫描由于格式限制的最小可用距离 (pos - 2)
    if config.mode == MatchMode::Rle {
        let mut best_len = 0;
        if pos >= 2 {
            while best_len < max_len && data.get(pos + best_len) == data.get(pos - 2 + best_len) {
                best_len += 1;
            }
        }
        if best_len >= 2 {
            return (best_len, 2);
        }
        return (0, 0);
    }

    let limit = pos.saturating_sub(4097);
    let key = ((data[pos] as u16) << 8) | (data[pos + 1] as u16);
    let mut match_pos_i32 = head[key as usize];
    let mut chain_length = config.max_chain;

    let mut best_len = 0;
    let mut best_offset = 0;

    while match_pos_i32 != -1 && chain_length > 0 {
        let match_pos = match_pos_i32 as usize;
        if match_pos < limit {
            break;
        }

        // 格式强制限制最小的 offset >= 2
        if pos - match_pos < 2 {
            match_pos_i32 = prev[match_pos];
            chain_length -= 1;
            continue;
        }

        // 快速剪枝优化
        if best_len < max_len {
            if data.get(match_pos + best_len) != data.get(pos + best_len) {
                match_pos_i32 = prev[match_pos];
                chain_length -= 1;
                continue;
            }
        }

        let mut current_len = 0;
        while current_len < max_len
            && data.get(pos + current_len) == data.get(match_pos + current_len)
        {
            current_len += 1;
        }

        if current_len > best_len {
            best_len = current_len;
            best_offset = pos - match_pos;
            if current_len >= config.nice_length {
                break;
            }
            if current_len >= config.good_length {
                chain_length >>= 2;
            }
        }

        match_pos_i32 = prev[match_pos];
        chain_length -= 1;
    }

    if best_len >= 2 {
        (best_len, best_offset)
    } else {
        (0, 0)
    }
}

/// Encoder for Buriko General Interpreter/Ethornell compressed files (DSC format).
pub struct DscEncoder<'a, T: Write + Seek> {
    stream: MsbBitWriter<'a, T>,
    magic: u32,
    key: u32,
    dec_count: u32,
    level: u8,
}

impl<'a, T: Write + Seek> DscEncoder<'a, T> {
    /// Creates a new DscEncoder with the given writer and compression level (0-9).
    pub fn new(writer: &'a mut T, level: u8) -> Self {
        let stream = MsbBitWriter::new(writer);
        DscEncoder {
            stream,
            magic: 0x5344 << 16, // "DS"
            key: rand::rng().random(),
            dec_count: 0,
            level: level.min(9),
        }
    }

    /// Packs the given data into the DSC format using configured LZSS compression.
    pub fn pack(mut self, data: &[u8]) -> Result<()> {
        let mut ops = vec![];
        let mut pos = 0;
        let config = &COMPRESS_CONFIGS[self.level as usize];

        let mut head: Vec<i32> = vec![-1; 1 << 16];
        let mut prev: Vec<i32> = vec![-1; data.len()];

        let insert_dict = |p: usize, head: &mut [i32], prev: &mut [i32]| {
            if config.mode != MatchMode::Rle && p + 1 < data.len() {
                let key = ((data[p] as u16) << 8) | (data[p + 1] as u16);
                prev[p] = head[key as usize];
                head[key as usize] = p as i32;
            }
        };

        while pos < data.len() {
            if config.mode == MatchMode::Store {
                ops.push(LzssOp::Literal(data[pos]));
                pos += 1;
                continue;
            }

            let (match_len, match_offset) = find_match(data, pos, &head, &prev, config);

            if match_len >= 2 {
                let mut lazy_match = false;

                // 延迟匹配逻辑
                if config.mode == MatchMode::Lazy
                    && match_len <= config.max_lazy
                    && pos + 1 < data.len()
                {
                    insert_dict(pos, &mut head, &mut prev);

                    let (next_len, _) = find_match(data, pos + 1, &head, &prev, config);

                    if next_len > match_len {
                        lazy_match = true;
                    }
                }

                if lazy_match {
                    ops.push(LzssOp::Literal(data[pos]));
                    pos += 1;
                    continue;
                }

                ops.push(LzssOp::Match {
                    len: match_len as u16,
                    offset: match_offset as u16,
                });

                let start_insert = if config.mode == MatchMode::Lazy
                    && match_len <= config.max_lazy
                    && pos + 1 < data.len()
                {
                    1 // 如果进行了延迟检查，pos 已被插入
                } else {
                    0
                };

                for i in start_insert..match_len {
                    insert_dict(pos + i, &mut head, &mut prev);
                }
                pos += match_len;
            } else {
                ops.push(LzssOp::Literal(data[pos]));
                insert_dict(pos, &mut head, &mut prev);
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
    ) -> Result<Box<dyn Script + Send + Sync>> {
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
        let encoder = DscEncoder::new(&mut writer, config.bgi_compress_level);
        let data = crate::utils::files::read_file(filename)?;
        encoder.pack(&data)?;
        Ok(())
    }
}

#[derive(Debug)]
/// DSC script
pub struct Dsc {
    data: Vec<u8>,
    level: u8,
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
            level: config.bgi_compress_level,
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
        let encoder = DscEncoder::new(&mut file, self.level);
        let data = crate::utils::files::read_file(custom_filename)?;
        encoder.pack(&data)?;
        Ok(())
    }
}

/// Parses the compression level for LZSS compression from a string.
pub fn parse_compress_level(level: &str) -> Result<u8, String> {
    number_range(level, 0, 9).map(|v| v as u8)
}
