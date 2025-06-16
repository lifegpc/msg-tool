use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use anyhow::Result;
use std::io::{Read, Seek};

pub trait BseGenerator {
    fn next_key(&mut self) -> i32;
}

pub struct BseGenerator100 {
    key: i32,
}

impl BseGenerator100 {
    pub fn new(key: i32) -> Self {
        BseGenerator100 { key }
    }
}

impl BseGenerator for BseGenerator100 {
    fn next_key(&mut self) -> i32 {
        let key = (self
            .key
            .overflowing_mul(257)
            .0
            .overflowing_shr(8)
            .0
            .overflowing_add(self.key.overflowing_mul(97).0)
            .0
            .overflowing_add(23)
            .0) as u32
            ^ 0xA6CD9B75;
        self.key = key.rotate_right(16) as i32;
        self.key
    }
}

pub struct BseGenerator101 {
    key: i32,
}

impl BseGenerator101 {
    pub fn new(key: i32) -> Self {
        BseGenerator101 { key }
    }
}

impl BseGenerator for BseGenerator101 {
    fn next_key(&mut self) -> i32 {
        let key = (self
            .key
            .overflowing_mul(127)
            .0
            .overflowing_shr(7)
            .0
            .overflowing_add(self.key.overflowing_mul(83).0)
            .0
            .overflowing_add(53)
            .0) as u32
            ^ 0xB97A7E5C;
        self.key = key.rotate_right(16) as i32;
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
        reader.seek(std::io::SeekFrom::Start(8))?;
        let version = reader.read_u16()?;
        if version != 0x0100 && version != 0x0101 {
            return Err(anyhow::anyhow!("Unsupported BSE version: {}", version));
        }
        let _checksum = reader.read_u16()?;
        let key = reader.read_i32()?;
        let mut header = [0u8; 0x40];
        reader.read_exact(&mut header)?;
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
            let mut dst = (generator.next_key() & 0x3F) as usize;
            while decoded[dst] {
                dst = (dst + 1) & 0x3F;
            }
            let shift = generator.next_key() & 7;
            let right_shift = (generator.next_key() & 1) == 0;
            let key_byte = generator.next_key();
            let symbol = (data[dst] as i32).wrapping_sub(key_byte);
            let symbol = symbol as u8;
            data[dst] = if right_shift {
                symbol.rotate_right(shift as u32)
            } else {
                symbol.rotate_left(shift as u32)
            };
            decoded[dst] = true;
        }
        Ok(())
    }

    pub fn is_dsc(&self) -> bool {
        self.header.starts_with(b"DSC FORMAT 1.00")
    }
}

impl<T: Read + Seek, F: Fn(&[u8], usize, &str) -> Option<&'static ScriptType>> Read
    for BseReader<T, F>
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.pos < 0x40 {
            let bytes_to_read = (0x40 - self.pos).min(buf.len() as u64);
            buf[..bytes_to_read as usize].copy_from_slice(
                &self.header[self.pos as usize..self.pos as usize + bytes_to_read as usize],
            );
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
