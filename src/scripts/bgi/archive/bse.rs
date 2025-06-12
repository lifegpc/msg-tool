use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use anyhow::Result;
use std::io::{Read, Seek};

pub trait BseGenerator {
    fn next_key(&mut self) -> u32;
}

fn rtor(v: u32, count: u8) -> u32 {
    let count = count & 0x1F;
    return v >> count | v << (32 - count);
}

fn rot_byte_r(v: u8, count: u8) -> u8 {
    let count = count & 0x07;
    return v >> count | v << (8 - count);
}

fn rot_byte_l(v: u8, count: u8) -> u8 {
    let count = count & 0x07;
    return v << count | v >> (8 - count);
}

pub struct BseGenerator100 {
    key: u32,
}

impl BseGenerator100 {
    pub fn new(key: u32) -> Self {
        BseGenerator100 { key }
    }
}

impl BseGenerator for BseGenerator100 {
    fn next_key(&mut self) -> u32 {
        let key = self
            .key
            .overflowing_mul(257)
            .0
            .overflowing_shr(8)
            .0
            .overflowing_add(self.key.overflowing_mul(97).0)
            .0
            .overflowing_add(23)
            .0
            ^ 0xA6CD9B75;
        self.key = rtor(key, 16);
        self.key
    }
}

pub struct BseGenerator101 {
    key: u32,
}

impl BseGenerator101 {
    pub fn new(key: u32) -> Self {
        BseGenerator101 { key }
    }
}

impl BseGenerator for BseGenerator101 {
    fn next_key(&mut self) -> u32 {
        let key = self
            .key
            .overflowing_mul(127)
            .0
            .overflowing_shr(7)
            .0
            .overflowing_add(self.key.overflowing_mul(83).0)
            .0
            .overflowing_add(53)
            .0
            ^ 0xB97A7E5C;
        self.key = rtor(key, 16);
        self.key
    }
}

pub struct BseReader<T: Read + Seek, F: Fn(&[u8], usize, &str) -> Option<&'static ScriptType>> {
    reader: T,
    header: [u8; 0x40],
    detect: F,
    pos: u64,
    filename: String,
}

impl<T: Read + Seek, F: Fn(&[u8], usize, &str) -> Option<&'static ScriptType>> BseReader<T, F> {
    pub fn new(mut reader: T, detect: F, filename: &str) -> Result<Self> {
        let version = reader.peek_u16_at(0x8)?;
        if version != 0x0100 && version != 0x0101 {
            return Err(anyhow::anyhow!("Unsupported BSE version: {}", version));
        }
        let _checksum = reader.peek_u16_at(0xA)?;
        let key: u32 = reader.peek_u32_at(0xC)?;
        let mut header = [0u8; 0x40];
        reader.peek_extract_at(0x10, &mut header)?;
        let generator: Box<dyn BseGenerator> = if version == 0x0100 {
            Box::new(BseGenerator100::new(key))
        } else {
            Box::new(BseGenerator101::new(key))
        };
        Self::decode_header(&mut header, generator)?;
        Ok(BseReader {
            reader,
            header,
            detect,
            pos: 0,
            filename: filename.to_string(),
        })
    }

    fn decode_header(data: &mut [u8; 0x40], mut generator: Box<dyn BseGenerator>) -> Result<()> {
        let mut decoded = [false; 0x40];
        for _ in 0..0x40 {
            let mut dst = generator.next_key() as usize & 0x3F;
            while decoded[dst] {
                dst = (dst + 1) & 0x3F;
            }
            let shift = (generator.next_key() & 7) as u8;
            let right_shift = generator.next_key() & 1 == 0;
            let symbol = data[dst].overflowing_sub(generator.next_key() as u8).0;
            data[dst] = if right_shift {
                rot_byte_r(symbol, shift)
            } else {
                rot_byte_l(symbol, shift)
            };
            decoded[dst] = true;
        }
        Ok(())
    }
}

impl<T: Read + Seek, F: Fn(&[u8], usize, &str) -> Option<&'static ScriptType>> Read
    for BseReader<T, F>
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.pos < 0x40 {
            let bytes_to_read = 0x40 - self.pos;
            buf[..bytes_to_read as usize].copy_from_slice(&self.header[self.pos as usize..]);
            self.pos += bytes_to_read;
            Ok(bytes_to_read as usize)
        } else {
            let true_pos = self.pos + 0x10;
            self.reader.seek(std::io::SeekFrom::Start(true_pos))?;
            let bytes_read = self.reader.read(buf)?;
            self.pos += bytes_read as u64;
            Ok(bytes_read)
        }
    }
}

impl<T: Read + Seek, F: Fn(&[u8], usize, &str) -> Option<&'static ScriptType>> ArchiveContent
    for BseReader<T, F>
{
    fn name(&self) -> &str {
        &self.filename
    }

    fn script_type(&self) -> Option<&ScriptType> {
        (self.detect)(&self.header, self.header.len(), &self.filename)
    }
}
