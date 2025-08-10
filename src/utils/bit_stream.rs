//! Bit stream utilities.
use crate::ext::io::*;
use anyhow::Result;
use std::io::{Read, Write};

/// A most significant bit (MSB) bit stream reader.
pub struct MsbBitStream<T: Read> {
    /// The input stream to read from.
    pub m_input: T,
    m_bits: u32,
    /// The number of bits currently cached.
    pub m_cached_bits: u32,
}

impl<T: Read> MsbBitStream<T> {
    /// Creates a new MSB bit stream reader.
    pub fn new(input: T) -> Self {
        MsbBitStream {
            m_input: input,
            m_bits: 0,
            m_cached_bits: 0,
        }
    }

    /// Reads a specified number of bits from the stream.
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

    /// Reads the next bit from the stream.
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

/// A most significant bit (MSB) bit writer.
pub struct MsbBitWriter<'a, T: Write> {
    /// The output stream to write to.
    pub writer: &'a mut T,
    buffer: u32,
    buffer_size: u32,
}

impl<'a, T: Write> MsbBitWriter<'a, T> {
    /// Creates a new MSB bit writer.
    pub fn new(writer: &'a mut T) -> Self {
        MsbBitWriter {
            writer,
            buffer: 0,
            buffer_size: 0,
        }
    }

    /// Flushes the buffer to the output stream.
    /// This writes any remaining bits in the buffer to the stream.
    pub fn flush(&mut self) -> Result<()> {
        if self.buffer_size > 0 {
            self.writer
                .write_u8(((self.buffer << (8 - self.buffer_size)) & 0xFF) as u8)?;
            self.buffer = 0;
            self.buffer_size = 0;
        }
        Ok(())
    }

    /// Puts a byte into the bit stream with a specified token width.
    pub fn put_bits(&mut self, byte: u32, token_width: u8) -> Result<()> {
        for i in 0..token_width {
            self.put_bit((byte & (1 << (token_width - 1 - i))) != 0)?;
        }
        Ok(())
    }

    /// Puts a single bit into the bit stream.
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

/// A least significant bit (LSB) bit stream reader.
pub struct LsbBitStream<T: Read> {
    /// The input stream to read from.
    pub m_input: T,
    m_bits: u32,
    /// The number of bits currently cached.
    pub m_cached_bits: u32,
}

impl<T: Read> LsbBitStream<T> {
    /// Creates a new LSB bit stream reader.
    pub fn new(input: T) -> Self {
        LsbBitStream {
            m_input: input,
            m_bits: 0,
            m_cached_bits: 0,
        }
    }

    /// Reads a specified number of bits from the stream.
    pub fn get_bits(&mut self, mut count: u32) -> Result<u32> {
        if self.m_cached_bits >= count {
            let mask = (1 << count) - 1;
            let value = self.m_bits & mask;
            self.m_bits >>= count;
            self.m_cached_bits -= count;
            Ok(value)
        } else {
            let mut value = self.m_bits & ((1 << self.m_cached_bits) - 1);
            count -= self.m_cached_bits;
            let mut shift = self.m_cached_bits;
            self.m_cached_bits = 0;
            while count >= 8 {
                let b = self.m_input.read_u8()?;
                value |= (b as u32) << shift;
                shift += 8;
                count -= 8;
            }
            if count > 0 {
                let b = self.m_input.read_u8()?;
                value |= ((b as u32) & ((1 << count) - 1)) << shift;
                self.m_bits = b as u32 >> count;
                self.m_cached_bits = 8 - count;
            }
            Ok(value)
        }
    }

    /// Reads the next bit from the stream.
    pub fn get_next_bit(&mut self) -> Result<bool> {
        Ok(self.get_bits(1)? == 1)
    }
}
