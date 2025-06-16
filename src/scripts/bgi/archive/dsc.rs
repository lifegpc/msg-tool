use crate::ext::io::*;
use crate::ext::vec::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::bit_stream::*;
use anyhow::Result;
use std::io::Write;

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

pub struct DscDecoder<'a> {
    stream: MsbBitStream<'a>,
    key: u32,
    magic: u32,
    output_size: u32,
    dec_count: u32,
}

impl<'a> DscDecoder<'a> {
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
        v1 = (v1 + (v0 >> 16)) & 0xffff;
        self.key = (v1 << 16) + (v0 & 0xffff) + 1;
        v1 as u8
    }
}

#[derive(Debug)]
pub struct DscBuilder {}

impl DscBuilder {
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
        _config: &ExtraConfig,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(Dsc::new(buf)?))
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
}

#[derive(Debug)]
pub struct Dsc {
    data: Vec<u8>,
}

impl Dsc {
    pub fn new(buf: Vec<u8>) -> Result<Self> {
        if buf.len() < 16 || !buf.starts_with(b"DSC FORMAT 1.00\0") {
            return Err(anyhow::anyhow!("Invalid DSC format"));
        }
        let decoder = DscDecoder::new(&buf)?;
        let data = decoder.unpack()?;
        Ok(Dsc { data })
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
        "unk"
    }

    fn custom_export(&self, filename: &std::path::Path, _encoding: Encoding) -> Result<()> {
        let mut f = std::fs::File::create(filename)?;
        f.write_all(&self.data)?;
        Ok(())
    }
}
