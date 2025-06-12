use crate::ext::io::*;
use anyhow::Result;
use std::io::Write;

pub struct MsbBitStream<'a> {
    pub m_input: MemReaderRef<'a>,
    m_bits: u32,
    m_cached_bits: u32,
}

impl<'a> MsbBitStream<'a> {
    pub fn new(input: MemReaderRef<'a>) -> Self {
        MsbBitStream {
            m_input: input,
            m_bits: 0,
            m_cached_bits: 0,
        }
    }

    pub fn get_bits(&mut self, count: u32) -> Result<u32> {
        while self.m_cached_bits < count {
            let byte = self.m_input.read_u8()?;
            self.m_bits = (self.m_bits << 8) | byte as u32;
            self.m_cached_bits += 8;
        }
        let mask = (1 << count) - 1;
        self.m_cached_bits -= count;
        let result = (self.m_bits >> self.m_cached_bits) & mask;
        Ok(result)
    }

    pub fn get_next_bit(&mut self) -> Result<bool> {
        if self.m_cached_bits == 0 {
            let byte = self.m_input.read_u8()?;
            self.m_bits = (self.m_bits << 8) | byte as u32;
            self.m_cached_bits += 8;
        }
        self.m_cached_bits -= 1;
        let bit = (self.m_bits >> self.m_cached_bits) & 1 != 0;
        Ok(bit)
    }
}

pub struct MsbBitWriter<'a, T: Write> {
    writer: &'a mut T,
    buffer: u32,
    buffer_size: u32,
}

impl<'a, T: Write> MsbBitWriter<'a, T> {
    pub fn new(writer: &'a mut T) -> Self {
        MsbBitWriter {
            writer,
            buffer: 0,
            buffer_size: 0,
        }
    }

    pub fn flush(&mut self) -> Result<()> {
        if self.buffer_size > 0 {
            self.writer.write_u8((self.buffer & 0xFF) as u8)?;
            self.buffer = 0;
            self.buffer_size = 0;
        }
        Ok(())
    }

    pub fn put_bits(&mut self, byte: u32, token_width: u8) -> Result<()> {
        for i in 0..token_width {
            self.put_bit((byte & (1 << (token_width - 1 - i))) != 0)?;
        }
        Ok(())
    }

    pub fn put_bit(&mut self, bit: bool) -> Result<()> {
        self.buffer <<= 1;
        if bit {
            self.buffer |= 1;
        }
        self.buffer_size += 1;
        if self.buffer_size == 8 {
            self.writer.write_u8((self.buffer & 0xFF) as u8)?;
            self.buffer_size -= 8;
        }
        Ok(())
    }
}
