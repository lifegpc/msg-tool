use crate::utils::encoding::decode_to_string;
use crate::{types::Encoding, utils::struct_pack::StructUnpack};
use std::{ffi::CString, io::*};

pub trait Peek {
    fn peek(&mut self, buf: &mut [u8]) -> Result<usize>;
    fn peek_extract(&mut self, buf: &mut [u8]) -> Result<()>;
    fn peek_at(&mut self, offset: usize, buf: &mut [u8]) -> Result<usize>;
    fn peek_extract_at(&mut self, offset: usize, buf: &mut [u8]) -> Result<()>;
    fn peek_at_vec(&mut self, offset: usize, len: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; len];
        let bytes_read = self.peek_at(offset, &mut buf)?;
        if bytes_read < len {
            buf.truncate(bytes_read);
        }
        Ok(buf)
    }
    fn peek_extract_at_vec(&mut self, offset: usize, len: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; len];
        self.peek_extract_at(offset, &mut buf)?;
        Ok(buf)
    }

    fn peek_u8(&mut self) -> Result<u8> {
        let mut buf = [0u8; 1];
        self.peek_extract(&mut buf)?;
        Ok(buf[0])
    }
    fn peek_u16(&mut self) -> Result<u16> {
        let mut buf = [0u8; 2];
        self.peek_extract(&mut buf)?;
        Ok(u16::from_le_bytes(buf))
    }
    fn peek_u16_be(&mut self) -> Result<u16> {
        let mut buf = [0u8; 2];
        self.peek_extract(&mut buf)?;
        Ok(u16::from_be_bytes(buf))
    }
    fn peek_u32(&mut self) -> Result<u32> {
        let mut buf = [0u8; 4];
        self.peek_extract(&mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }
    fn peek_u32_be(&mut self) -> Result<u32> {
        let mut buf = [0u8; 4];
        self.peek_extract(&mut buf)?;
        Ok(u32::from_be_bytes(buf))
    }
    fn peek_u64(&mut self) -> Result<u64> {
        let mut buf = [0u8; 8];
        self.peek_extract(&mut buf)?;
        Ok(u64::from_le_bytes(buf))
    }
    fn peek_u64_be(&mut self) -> Result<u64> {
        let mut buf = [0u8; 8];
        self.peek_extract(&mut buf)?;
        Ok(u64::from_be_bytes(buf))
    }
    fn peek_u128(&mut self) -> Result<u128> {
        let mut buf = [0u8; 16];
        self.peek_extract(&mut buf)?;
        Ok(u128::from_le_bytes(buf))
    }
    fn peek_u128_be(&mut self) -> Result<u128> {
        let mut buf = [0u8; 16];
        self.peek_extract(&mut buf)?;
        Ok(u128::from_be_bytes(buf))
    }
    fn peek_i8(&mut self) -> Result<i8> {
        let mut buf = [0u8; 1];
        self.peek_extract(&mut buf)?;
        Ok(i8::from_le_bytes(buf))
    }
    fn peek_i16(&mut self) -> Result<i16> {
        let mut buf = [0u8; 2];
        self.peek_extract(&mut buf)?;
        Ok(i16::from_le_bytes(buf))
    }
    fn peek_i16_be(&mut self) -> Result<i16> {
        let mut buf = [0u8; 2];
        self.peek_extract(&mut buf)?;
        Ok(i16::from_be_bytes(buf))
    }
    fn peek_i32(&mut self) -> Result<i32> {
        let mut buf = [0u8; 4];
        self.peek_extract(&mut buf)?;
        Ok(i32::from_le_bytes(buf))
    }
    fn peek_i32_be(&mut self) -> Result<i32> {
        let mut buf = [0u8; 4];
        self.peek_extract(&mut buf)?;
        Ok(i32::from_be_bytes(buf))
    }
    fn peek_i64(&mut self) -> Result<i64> {
        let mut buf = [0u8; 8];
        self.peek_extract(&mut buf)?;
        Ok(i64::from_le_bytes(buf))
    }
    fn peek_i64_be(&mut self) -> Result<i64> {
        let mut buf = [0u8; 8];
        self.peek_extract(&mut buf)?;
        Ok(i64::from_be_bytes(buf))
    }
    fn peek_i128(&mut self) -> Result<i128> {
        let mut buf = [0u8; 16];
        self.peek_extract(&mut buf)?;
        Ok(i128::from_le_bytes(buf))
    }
    fn peek_i128_be(&mut self) -> Result<i128> {
        let mut buf = [0u8; 16];
        self.peek_extract(&mut buf)?;
        Ok(i128::from_be_bytes(buf))
    }
    fn peek_u8_at(&mut self, offset: usize) -> Result<u8> {
        let mut buf = [0u8; 1];
        self.peek_extract_at(offset, &mut buf)?;
        Ok(buf[0])
    }
    fn peek_u16_at(&mut self, offset: usize) -> Result<u16> {
        let mut buf = [0u8; 2];
        self.peek_extract_at(offset, &mut buf)?;
        Ok(u16::from_le_bytes(buf))
    }
    fn peek_u16_be_at(&mut self, offset: usize) -> Result<u16> {
        let mut buf = [0u8; 2];
        self.peek_extract_at(offset, &mut buf)?;
        Ok(u16::from_be_bytes(buf))
    }
    fn peek_u32_at(&mut self, offset: usize) -> Result<u32> {
        let mut buf = [0u8; 4];
        self.peek_extract_at(offset, &mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }
    fn peek_u32_be_at(&mut self, offset: usize) -> Result<u32> {
        let mut buf = [0u8; 4];
        self.peek_extract_at(offset, &mut buf)?;
        Ok(u32::from_be_bytes(buf))
    }
    fn peek_u64_at(&mut self, offset: usize) -> Result<u64> {
        let mut buf = [0u8; 8];
        self.peek_extract_at(offset, &mut buf)?;
        Ok(u64::from_le_bytes(buf))
    }
    fn peek_u64_be_at(&mut self, offset: usize) -> Result<u64> {
        let mut buf = [0u8; 8];
        self.peek_extract_at(offset, &mut buf)?;
        Ok(u64::from_be_bytes(buf))
    }
    fn peek_u128_at(&mut self, offset: usize) -> Result<u128> {
        let mut buf = [0u8; 16];
        self.peek_extract_at(offset, &mut buf)?;
        Ok(u128::from_le_bytes(buf))
    }
    fn peek_u128_be_at(&mut self, offset: usize) -> Result<u128> {
        let mut buf = [0u8; 16];
        self.peek_extract_at(offset, &mut buf)?;
        Ok(u128::from_be_bytes(buf))
    }
    fn peek_i8_at(&mut self, offset: usize) -> Result<i8> {
        let mut buf = [0u8; 1];
        self.peek_extract_at(offset, &mut buf)?;
        Ok(i8::from_le_bytes(buf))
    }
    fn peek_i16_at(&mut self, offset: usize) -> Result<i16> {
        let mut buf = [0u8; 2];
        self.peek_extract_at(offset, &mut buf)?;
        Ok(i16::from_le_bytes(buf))
    }
    fn peek_i16_be_at(&mut self, offset: usize) -> Result<i16> {
        let mut buf = [0u8; 2];
        self.peek_extract_at(offset, &mut buf)?;
        Ok(i16::from_be_bytes(buf))
    }
    fn peek_i32_at(&mut self, offset: usize) -> Result<i32> {
        let mut buf = [0u8; 4];
        self.peek_extract_at(offset, &mut buf)?;
        Ok(i32::from_le_bytes(buf))
    }
    fn peek_i32_be_at(&mut self, offset: usize) -> Result<i32> {
        let mut buf = [0u8; 4];
        self.peek_extract_at(offset, &mut buf)?;
        Ok(i32::from_be_bytes(buf))
    }
    fn peek_i64_at(&mut self, offset: usize) -> Result<i64> {
        let mut buf = [0u8; 8];
        self.peek_extract_at(offset, &mut buf)?;
        Ok(i64::from_le_bytes(buf))
    }
    fn peek_i64_be_at(&mut self, offset: usize) -> Result<i64> {
        let mut buf = [0u8; 8];
        self.peek_extract_at(offset, &mut buf)?;
        Ok(i64::from_be_bytes(buf))
    }
    fn peek_i128_at(&mut self, offset: usize) -> Result<i128> {
        let mut buf = [0u8; 16];
        self.peek_extract_at(offset, &mut buf)?;
        Ok(i128::from_le_bytes(buf))
    }
    fn peek_i128_be_at(&mut self, offset: usize) -> Result<i128> {
        let mut buf = [0u8; 16];
        self.peek_extract_at(offset, &mut buf)?;
        Ok(i128::from_be_bytes(buf))
    }

    fn peek_cstring(&mut self) -> Result<CString>;
    fn peek_cstring_at(&mut self, offset: usize) -> Result<CString>;

    fn read_struct<T: StructUnpack>(&mut self, big: bool, encoding: Encoding) -> Result<T>;
    fn read_struct_vec<T: StructUnpack>(
        &mut self,
        count: usize,
        big: bool,
        encoding: Encoding,
    ) -> Result<Vec<T>> {
        let mut vec = Vec::with_capacity(count);
        for _ in 0..count {
            vec.push(self.read_struct(big, encoding)?);
        }
        Ok(vec)
    }
}

impl<T: Read + Seek> Peek for T {
    fn peek(&mut self, buf: &mut [u8]) -> Result<usize> {
        let current_pos = self.stream_position()?;
        let bytes_read = self.read(buf)?;
        self.seek(SeekFrom::Start(current_pos))?;
        Ok(bytes_read)
    }

    fn peek_extract(&mut self, buf: &mut [u8]) -> Result<()> {
        let current_pos = self.stream_position()?;
        self.read_exact(buf)?;
        self.seek(SeekFrom::Start(current_pos))?;
        Ok(())
    }

    fn peek_at(&mut self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        let current_pos = self.stream_position()?;
        self.seek(SeekFrom::Start(offset as u64))?;
        let bytes_read = self.read(buf)?;
        self.seek(SeekFrom::Start(current_pos))?;
        Ok(bytes_read)
    }

    fn peek_extract_at(&mut self, offset: usize, buf: &mut [u8]) -> Result<()> {
        let current_pos = self.stream_position()?;
        self.seek(SeekFrom::Start(offset as u64))?;
        self.read_exact(buf)?;
        self.seek(SeekFrom::Start(current_pos))?;
        Ok(())
    }

    fn peek_cstring(&mut self) -> Result<CString> {
        let current_pos = self.stream_position()?;
        let mut buf = Vec::new();
        loop {
            let mut byte = [0u8; 1];
            self.read_exact(&mut byte)?;
            if byte[0] == 0 {
                break;
            }
            buf.push(byte[0]);
        }
        self.seek(SeekFrom::Start(current_pos))?;
        CString::new(buf).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    fn peek_cstring_at(&mut self, offset: usize) -> Result<CString> {
        let current_pos = self.stream_position()?;
        let mut buf = Vec::new();
        self.seek(SeekFrom::Start(offset as u64))?;
        loop {
            let mut byte = [0u8; 1];
            self.read_exact(&mut byte)?;
            if byte[0] == 0 {
                break;
            }
            buf.push(byte[0]);
        }
        self.seek(SeekFrom::Start(current_pos))?;
        CString::new(buf).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    fn read_struct<S: StructUnpack>(&mut self, big: bool, encoding: Encoding) -> Result<S> {
        S::unpack(self, big, encoding)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }
}

pub trait ReadExt {
    fn read_u8(&mut self) -> Result<u8>;
    fn read_u16(&mut self) -> Result<u16>;
    fn read_u16_be(&mut self) -> Result<u16>;
    fn read_u32(&mut self) -> Result<u32>;
    fn read_u32_be(&mut self) -> Result<u32>;
    fn read_u64(&mut self) -> Result<u64>;
    fn read_u64_be(&mut self) -> Result<u64>;
    fn read_u128(&mut self) -> Result<u128>;
    fn read_u128_be(&mut self) -> Result<u128>;
    fn read_i8(&mut self) -> Result<i8>;
    fn read_i16(&mut self) -> Result<i16>;
    fn read_i16_be(&mut self) -> Result<i16>;
    fn read_i32(&mut self) -> Result<i32>;
    fn read_i32_be(&mut self) -> Result<i32>;
    fn read_i64(&mut self) -> Result<i64>;
    fn read_i64_be(&mut self) -> Result<i64>;
    fn read_i128(&mut self) -> Result<i128>;
    fn read_i128_be(&mut self) -> Result<i128>;

    fn read_cstring(&mut self) -> Result<CString>;
    fn read_fstring(&mut self, len: usize, encoding: Encoding, trim: bool) -> Result<String>;

    fn read_exact_vec(&mut self, len: usize) -> Result<Vec<u8>>;
}

impl<T: Read> ReadExt for T {
    fn read_u8(&mut self) -> Result<u8> {
        let mut buf = [0u8; 1];
        self.read_exact(&mut buf)?;
        Ok(buf[0])
    }
    fn read_u16(&mut self) -> Result<u16> {
        let mut buf = [0u8; 2];
        self.read_exact(&mut buf)?;
        Ok(u16::from_le_bytes(buf))
    }
    fn read_u16_be(&mut self) -> Result<u16> {
        let mut buf = [0u8; 2];
        self.read_exact(&mut buf)?;
        Ok(u16::from_be_bytes(buf))
    }
    fn read_u32(&mut self) -> Result<u32> {
        let mut buf = [0u8; 4];
        self.read_exact(&mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }
    fn read_u32_be(&mut self) -> Result<u32> {
        let mut buf = [0u8; 4];
        self.read_exact(&mut buf)?;
        Ok(u32::from_be_bytes(buf))
    }
    fn read_u64(&mut self) -> Result<u64> {
        let mut buf = [0u8; 8];
        self.read_exact(&mut buf)?;
        Ok(u64::from_le_bytes(buf))
    }
    fn read_u64_be(&mut self) -> Result<u64> {
        let mut buf = [0u8; 8];
        self.read_exact(&mut buf)?;
        Ok(u64::from_be_bytes(buf))
    }
    fn read_u128(&mut self) -> Result<u128> {
        let mut buf = [0u8; 16];
        self.read_exact(&mut buf)?;
        Ok(u128::from_le_bytes(buf))
    }
    fn read_u128_be(&mut self) -> Result<u128> {
        let mut buf = [0u8; 16];
        self.read_exact(&mut buf)?;
        Ok(u128::from_be_bytes(buf))
    }
    fn read_i8(&mut self) -> Result<i8> {
        let mut buf = [0u8; 1];
        self.read_exact(&mut buf)?;
        Ok(i8::from_le_bytes(buf))
    }
    fn read_i16(&mut self) -> Result<i16> {
        let mut buf = [0u8; 2];
        self.read_exact(&mut buf)?;
        Ok(i16::from_le_bytes(buf))
    }
    fn read_i16_be(&mut self) -> Result<i16> {
        let mut buf = [0u8; 2];
        self.read_exact(&mut buf)?;
        Ok(i16::from_be_bytes(buf))
    }
    fn read_i32(&mut self) -> Result<i32> {
        let mut buf = [0u8; 4];
        self.read_exact(&mut buf)?;
        Ok(i32::from_le_bytes(buf))
    }
    fn read_i32_be(&mut self) -> Result<i32> {
        let mut buf = [0u8; 4];
        self.read_exact(&mut buf)?;
        Ok(i32::from_be_bytes(buf))
    }
    fn read_i64(&mut self) -> Result<i64> {
        let mut buf = [0u8; 8];
        self.read_exact(&mut buf)?;
        Ok(i64::from_le_bytes(buf))
    }
    fn read_i64_be(&mut self) -> Result<i64> {
        let mut buf = [0u8; 8];
        self.read_exact(&mut buf)?;
        Ok(i64::from_be_bytes(buf))
    }
    fn read_i128(&mut self) -> Result<i128> {
        let mut buf = [0u8; 16];
        self.read_exact(&mut buf)?;
        Ok(i128::from_le_bytes(buf))
    }
    fn read_i128_be(&mut self) -> Result<i128> {
        let mut buf = [0u8; 16];
        self.read_exact(&mut buf)?;
        Ok(i128::from_be_bytes(buf))
    }

    fn read_cstring(&mut self) -> Result<CString> {
        let mut buf = Vec::new();
        loop {
            let mut byte = [0u8; 1];
            self.read_exact(&mut byte)?;
            if byte[0] == 0 {
                break;
            }
            buf.push(byte[0]);
        }
        CString::new(buf).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }
    fn read_fstring(&mut self, len: usize, encoding: Encoding, trim: bool) -> Result<String> {
        let mut buf = vec![0u8; len];
        self.read_exact(&mut buf)?;
        if trim {
            let first_zero = buf.iter().position(|&b| b == 0);
            if let Some(pos) = first_zero {
                buf.truncate(pos);
            }
        }
        let s = decode_to_string(encoding, &buf)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(s)
    }

    fn read_exact_vec(&mut self, len: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; len];
        self.read_exact(&mut buf)?;
        Ok(buf)
    }
}

pub trait WriteExt {
    fn write_u8(&mut self, value: u8) -> Result<()>;
    fn write_u16(&mut self, value: u16) -> Result<()>;
    fn write_u16_be(&mut self, value: u16) -> Result<()>;
    fn write_u32(&mut self, value: u32) -> Result<()>;
    fn write_u32_be(&mut self, value: u32) -> Result<()>;
    fn write_u64(&mut self, value: u64) -> Result<()>;
    fn write_u64_be(&mut self, value: u64) -> Result<()>;
    fn write_u128(&mut self, value: u128) -> Result<()>;
    fn write_u128_be(&mut self, value: u128) -> Result<()>;
    fn write_i8(&mut self, value: i8) -> Result<()>;
    fn write_i16(&mut self, value: i16) -> Result<()>;
    fn write_i16_be(&mut self, value: i16) -> Result<()>;
    fn write_i32(&mut self, value: i32) -> Result<()>;
    fn write_i32_be(&mut self, value: i32) -> Result<()>;
    fn write_i64(&mut self, value: i64) -> Result<()>;
    fn write_i64_be(&mut self, value: i64) -> Result<()>;
    fn write_i128(&mut self, value: i128) -> Result<()>;
    fn write_i128_be(&mut self, value: i128) -> Result<()>;
}

impl<T: Write> WriteExt for T {
    fn write_u8(&mut self, value: u8) -> Result<()> {
        self.write_all(&value.to_le_bytes())
    }
    fn write_u16(&mut self, value: u16) -> Result<()> {
        self.write_all(&value.to_le_bytes())
    }
    fn write_u16_be(&mut self, value: u16) -> Result<()> {
        self.write_all(&value.to_be_bytes())
    }
    fn write_u32(&mut self, value: u32) -> Result<()> {
        self.write_all(&value.to_le_bytes())
    }
    fn write_u32_be(&mut self, value: u32) -> Result<()> {
        self.write_all(&value.to_be_bytes())
    }
    fn write_u64(&mut self, value: u64) -> Result<()> {
        self.write_all(&value.to_le_bytes())
    }
    fn write_u64_be(&mut self, value: u64) -> Result<()> {
        self.write_all(&value.to_be_bytes())
    }
    fn write_u128(&mut self, value: u128) -> Result<()> {
        self.write_all(&value.to_le_bytes())
    }
    fn write_u128_be(&mut self, value: u128) -> Result<()> {
        self.write_all(&value.to_be_bytes())
    }
    fn write_i8(&mut self, value: i8) -> Result<()> {
        self.write_all(&value.to_le_bytes())
    }
    fn write_i16(&mut self, value: i16) -> Result<()> {
        self.write_all(&value.to_le_bytes())
    }
    fn write_i16_be(&mut self, value: i16) -> Result<()> {
        self.write_all(&value.to_be_bytes())
    }
    fn write_i32(&mut self, value: i32) -> Result<()> {
        self.write_all(&value.to_le_bytes())
    }
    fn write_i32_be(&mut self, value: i32) -> Result<()> {
        self.write_all(&value.to_be_bytes())
    }
    fn write_i64(&mut self, value: i64) -> Result<()> {
        self.write_all(&value.to_le_bytes())
    }
    fn write_i64_be(&mut self, value: i64) -> Result<()> {
        self.write_all(&value.to_be_bytes())
    }
    fn write_i128(&mut self, value: i128) -> Result<()> {
        self.write_all(&value.to_le_bytes())
    }
    fn write_i128_be(&mut self, value: i128) -> Result<()> {
        self.write_all(&value.to_be_bytes())
    }
}

pub struct MemReader {
    data: Vec<u8>,
    pos: usize,
}

pub struct MemReaderRef<'a> {
    data: &'a [u8],
    pos: usize,
}

impl std::fmt::Debug for MemReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemReader")
            .field("pos", &self.pos)
            .field("data_length", &self.data.len())
            .finish_non_exhaustive()
    }
}

impl MemReader {
    pub fn new(data: Vec<u8>) -> Self {
        MemReader { data, pos: 0 }
    }

    pub fn to_ref(&self) -> MemReaderRef {
        MemReaderRef {
            data: &self.data,
            pos: self.pos,
        }
    }
}

impl<'a> MemReaderRef<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        MemReaderRef { data, pos: 0 }
    }
}

impl Read for MemReader {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        if self.pos >= self.data.len() {
            return Ok(0);
        }
        let bytes_to_read = buf.len().min(self.data.len() - self.pos);
        let mut bu = &self.data[self.pos..self.pos + bytes_to_read];
        bu.read(buf)?;
        self.pos += bytes_to_read;
        Ok(bytes_to_read)
    }
}

impl Seek for MemReader {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        match pos {
            SeekFrom::Start(offset) => {
                if offset > self.data.len() as u64 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Seek position is beyond the end of the data",
                    ));
                }
                self.pos = offset as usize;
            }
            SeekFrom::End(offset) => {
                let end_pos = self.data.len() as i64 + offset;
                if end_pos < 0 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Seek from end resulted in negative position",
                    ));
                }
                self.pos = end_pos as usize;
            }
            SeekFrom::Current(offset) => {
                let new_pos = (self.pos as i64 + offset) as usize;
                if new_pos > self.data.len() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Seek position is beyond the end of the data",
                    ));
                }
                self.pos = new_pos;
            }
        }
        Ok(self.pos as u64)
    }

    fn stream_position(&mut self) -> Result<u64> {
        Ok(self.pos as u64)
    }

    fn rewind(&mut self) -> Result<()> {
        self.pos = 0;
        Ok(())
    }
}

impl<'a> Read for MemReaderRef<'a> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        if self.pos >= self.data.len() {
            return Ok(0);
        }
        let bytes_to_read = buf.len().min(self.data.len() - self.pos);
        let mut bu = &self.data[self.pos..self.pos + bytes_to_read];
        bu.read(buf)?;
        self.pos += bytes_to_read;
        Ok(bytes_to_read)
    }
}

impl<'a> Seek for MemReaderRef<'a> {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        match pos {
            SeekFrom::Start(offset) => {
                if offset > self.data.len() as u64 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Seek position is beyond the end of the data",
                    ));
                }
                self.pos = offset as usize;
            }
            SeekFrom::End(offset) => {
                let end_pos = self.data.len() as i64 + offset;
                if end_pos < 0 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Seek from end resulted in negative position",
                    ));
                }
                self.pos = end_pos as usize;
            }
            SeekFrom::Current(offset) => {
                let new_pos = (self.pos as i64 + offset) as usize;
                if new_pos > self.data.len() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Seek position is beyond the end of the data",
                    ));
                }
                self.pos = new_pos;
            }
        }
        Ok(self.pos as u64)
    }

    fn stream_position(&mut self) -> Result<u64> {
        Ok(self.pos as u64)
    }

    fn rewind(&mut self) -> Result<()> {
        self.pos = 0;
        Ok(())
    }
}
