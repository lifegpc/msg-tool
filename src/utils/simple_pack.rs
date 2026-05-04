use crate::ext::io::*;
use crate::types::*;
use crate::utils::encoding::*;
use anyhow::Result;
use std::io::{Read, Seek};

pub struct SimplePack<T: Read> {
    inner: T,
    total: u64,
    current: u64,
}

impl<T: Read> SimplePack<T> {
    pub fn next<'a>(&'a mut self) -> Result<Option<SimplePackEntry<'a, T>>> {
        if self.current >= self.total {
            return Ok(None);
        }
        let name = self.read_cstring()?;
        let name = decode_to_string(Encoding::Utf8, name.as_bytes(), true)?;
        let entry_size = self.read_u64()?;
        Ok(Some(SimplePackEntry {
            pack: self,
            total: entry_size,
            current: 0,
            name,
        }))
    }
}

impl<T: Read> Read for SimplePack<T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let remaining = self.total - self.current;
        if remaining == 0 {
            return Ok(0);
        }
        let to_read = std::cmp::min(remaining, buf.len() as u64) as usize;
        let bytes_read = self.inner.read(&mut buf[..to_read])?;
        self.current += bytes_read as u64;
        Ok(bytes_read)
    }
}

pub struct SimplePackEntry<'a, T: Read> {
    pub pack: &'a mut SimplePack<T>,
    total: u64,
    current: u64,
    pub name: String,
}

impl<'a, T: Read> SimplePackEntry<'a, T> {
    pub fn total_size(&self) -> u64 {
        self.total
    }

    pub fn is_eof(&self) -> bool {
        self.current >= self.total
    }
}

impl<'a, T: Read> Drop for SimplePackEntry<'a, T> {
    fn drop(&mut self) {
        let to_skip = self.total - self.current;
        if to_skip > 0 {
            if let Err(e) = self.pack.skip(to_skip) {
                eprintln!("Failed to skip remaining bytes in SimplePackEntry: {}", e);
                crate::COUNTER.inc_error();
            }
        }
    }
}

impl<'a, T: Read> Read for SimplePackEntry<'a, T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let remaining = self.total - self.current;
        if remaining == 0 {
            return Ok(0);
        }
        let to_read = std::cmp::min(remaining, buf.len() as u64) as usize;
        let bytes_read = self.pack.read(&mut buf[..to_read])?;
        self.current += bytes_read as u64;
        Ok(bytes_read)
    }
}

pub fn read_simple_pack<'a, T: Read + Seek + 'a>(
    mut reader: T,
) -> Result<SimplePack<Box<dyn Read + 'a>>> {
    reader.read_and_equal(b"SPCK")?;
    let flags = reader.read_u8()?;
    // not compressed
    if flags == 0 {
        let pos = reader.stream_position()?;
        let total = reader.stream_length()? - pos;
        Ok(SimplePack {
            inner: Box::new(reader),
            total,
            current: 0,
        })
    } else {
        let compressed_size = reader.read_u64()?;
        let uncompressed_size = reader.read_u64()?;
        let compressed = reader.take(compressed_size);
        let decompressed = zstd::stream::read::Decoder::new(compressed)?;
        Ok(SimplePack {
            inner: Box::new(decompressed),
            total: uncompressed_size,
            current: 0,
        })
    }
}
