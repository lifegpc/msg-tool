use crate::utils::encoding::decode_to_string;
use crate::{types::Encoding, utils::struct_pack::StructUnpack};
use std::ffi::CString;
use std::io::*;
use std::sync::Mutex;

pub trait Peek {
    fn peek(&mut self, buf: &mut [u8]) -> Result<usize>;
    fn peek_exact(&mut self, buf: &mut [u8]) -> Result<()>;
    fn peek_at(&mut self, offset: usize, buf: &mut [u8]) -> Result<usize>;
    fn peek_exact_at(&mut self, offset: usize, buf: &mut [u8]) -> Result<()>;
    fn peek_at_vec(&mut self, offset: usize, len: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; len];
        let bytes_read = self.peek_at(offset, &mut buf)?;
        if bytes_read < len {
            buf.truncate(bytes_read);
        }
        Ok(buf)
    }
    fn peek_exact_at_vec(&mut self, offset: usize, len: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; len];
        self.peek_exact_at(offset, &mut buf)?;
        Ok(buf)
    }

    fn peek_u8(&mut self) -> Result<u8> {
        let mut buf = [0u8; 1];
        self.peek_exact(&mut buf)?;
        Ok(buf[0])
    }
    fn peek_u16(&mut self) -> Result<u16> {
        let mut buf = [0u8; 2];
        self.peek_exact(&mut buf)?;
        Ok(u16::from_le_bytes(buf))
    }
    fn peek_u16_be(&mut self) -> Result<u16> {
        let mut buf = [0u8; 2];
        self.peek_exact(&mut buf)?;
        Ok(u16::from_be_bytes(buf))
    }
    fn peek_u32(&mut self) -> Result<u32> {
        let mut buf = [0u8; 4];
        self.peek_exact(&mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }
    fn peek_u32_be(&mut self) -> Result<u32> {
        let mut buf = [0u8; 4];
        self.peek_exact(&mut buf)?;
        Ok(u32::from_be_bytes(buf))
    }
    fn peek_u64(&mut self) -> Result<u64> {
        let mut buf = [0u8; 8];
        self.peek_exact(&mut buf)?;
        Ok(u64::from_le_bytes(buf))
    }
    fn peek_u64_be(&mut self) -> Result<u64> {
        let mut buf = [0u8; 8];
        self.peek_exact(&mut buf)?;
        Ok(u64::from_be_bytes(buf))
    }
    fn peek_u128(&mut self) -> Result<u128> {
        let mut buf = [0u8; 16];
        self.peek_exact(&mut buf)?;
        Ok(u128::from_le_bytes(buf))
    }
    fn peek_u128_be(&mut self) -> Result<u128> {
        let mut buf = [0u8; 16];
        self.peek_exact(&mut buf)?;
        Ok(u128::from_be_bytes(buf))
    }
    fn peek_i8(&mut self) -> Result<i8> {
        let mut buf = [0u8; 1];
        self.peek_exact(&mut buf)?;
        Ok(i8::from_le_bytes(buf))
    }
    fn peek_i16(&mut self) -> Result<i16> {
        let mut buf = [0u8; 2];
        self.peek_exact(&mut buf)?;
        Ok(i16::from_le_bytes(buf))
    }
    fn peek_i16_be(&mut self) -> Result<i16> {
        let mut buf = [0u8; 2];
        self.peek_exact(&mut buf)?;
        Ok(i16::from_be_bytes(buf))
    }
    fn peek_i32(&mut self) -> Result<i32> {
        let mut buf = [0u8; 4];
        self.peek_exact(&mut buf)?;
        Ok(i32::from_le_bytes(buf))
    }
    fn peek_i32_be(&mut self) -> Result<i32> {
        let mut buf = [0u8; 4];
        self.peek_exact(&mut buf)?;
        Ok(i32::from_be_bytes(buf))
    }
    fn peek_i64(&mut self) -> Result<i64> {
        let mut buf = [0u8; 8];
        self.peek_exact(&mut buf)?;
        Ok(i64::from_le_bytes(buf))
    }
    fn peek_i64_be(&mut self) -> Result<i64> {
        let mut buf = [0u8; 8];
        self.peek_exact(&mut buf)?;
        Ok(i64::from_be_bytes(buf))
    }
    fn peek_i128(&mut self) -> Result<i128> {
        let mut buf = [0u8; 16];
        self.peek_exact(&mut buf)?;
        Ok(i128::from_le_bytes(buf))
    }
    fn peek_i128_be(&mut self) -> Result<i128> {
        let mut buf = [0u8; 16];
        self.peek_exact(&mut buf)?;
        Ok(i128::from_be_bytes(buf))
    }
    fn peek_u8_at(&mut self, offset: usize) -> Result<u8> {
        let mut buf = [0u8; 1];
        self.peek_exact_at(offset, &mut buf)?;
        Ok(buf[0])
    }
    fn peek_u16_at(&mut self, offset: usize) -> Result<u16> {
        let mut buf = [0u8; 2];
        self.peek_exact_at(offset, &mut buf)?;
        Ok(u16::from_le_bytes(buf))
    }
    fn peek_u16_be_at(&mut self, offset: usize) -> Result<u16> {
        let mut buf = [0u8; 2];
        self.peek_exact_at(offset, &mut buf)?;
        Ok(u16::from_be_bytes(buf))
    }
    fn peek_u32_at(&mut self, offset: usize) -> Result<u32> {
        let mut buf = [0u8; 4];
        self.peek_exact_at(offset, &mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }
    fn peek_u32_be_at(&mut self, offset: usize) -> Result<u32> {
        let mut buf = [0u8; 4];
        self.peek_exact_at(offset, &mut buf)?;
        Ok(u32::from_be_bytes(buf))
    }
    fn peek_u64_at(&mut self, offset: usize) -> Result<u64> {
        let mut buf = [0u8; 8];
        self.peek_exact_at(offset, &mut buf)?;
        Ok(u64::from_le_bytes(buf))
    }
    fn peek_u64_be_at(&mut self, offset: usize) -> Result<u64> {
        let mut buf = [0u8; 8];
        self.peek_exact_at(offset, &mut buf)?;
        Ok(u64::from_be_bytes(buf))
    }
    fn peek_u128_at(&mut self, offset: usize) -> Result<u128> {
        let mut buf = [0u8; 16];
        self.peek_exact_at(offset, &mut buf)?;
        Ok(u128::from_le_bytes(buf))
    }
    fn peek_u128_be_at(&mut self, offset: usize) -> Result<u128> {
        let mut buf = [0u8; 16];
        self.peek_exact_at(offset, &mut buf)?;
        Ok(u128::from_be_bytes(buf))
    }
    fn peek_i8_at(&mut self, offset: usize) -> Result<i8> {
        let mut buf = [0u8; 1];
        self.peek_exact_at(offset, &mut buf)?;
        Ok(i8::from_le_bytes(buf))
    }
    fn peek_i16_at(&mut self, offset: usize) -> Result<i16> {
        let mut buf = [0u8; 2];
        self.peek_exact_at(offset, &mut buf)?;
        Ok(i16::from_le_bytes(buf))
    }
    fn peek_i16_be_at(&mut self, offset: usize) -> Result<i16> {
        let mut buf = [0u8; 2];
        self.peek_exact_at(offset, &mut buf)?;
        Ok(i16::from_be_bytes(buf))
    }
    fn peek_i32_at(&mut self, offset: usize) -> Result<i32> {
        let mut buf = [0u8; 4];
        self.peek_exact_at(offset, &mut buf)?;
        Ok(i32::from_le_bytes(buf))
    }
    fn peek_i32_be_at(&mut self, offset: usize) -> Result<i32> {
        let mut buf = [0u8; 4];
        self.peek_exact_at(offset, &mut buf)?;
        Ok(i32::from_be_bytes(buf))
    }
    fn peek_i64_at(&mut self, offset: usize) -> Result<i64> {
        let mut buf = [0u8; 8];
        self.peek_exact_at(offset, &mut buf)?;
        Ok(i64::from_le_bytes(buf))
    }
    fn peek_i64_be_at(&mut self, offset: usize) -> Result<i64> {
        let mut buf = [0u8; 8];
        self.peek_exact_at(offset, &mut buf)?;
        Ok(i64::from_be_bytes(buf))
    }
    fn peek_i128_at(&mut self, offset: usize) -> Result<i128> {
        let mut buf = [0u8; 16];
        self.peek_exact_at(offset, &mut buf)?;
        Ok(i128::from_le_bytes(buf))
    }
    fn peek_i128_be_at(&mut self, offset: usize) -> Result<i128> {
        let mut buf = [0u8; 16];
        self.peek_exact_at(offset, &mut buf)?;
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

    fn peek_and_equal(&mut self, data: &[u8]) -> Result<()> {
        let mut buf = vec![0u8; data.len()];
        self.peek_exact(&mut buf)?;
        if buf != data {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Data does not match",
            ));
        }
        Ok(())
    }
    fn peek_and_equal_at(&mut self, offset: usize, data: &[u8]) -> Result<()> {
        let mut buf = vec![0u8; data.len()];
        self.peek_exact_at(offset, &mut buf)?;
        if buf != data {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Data does not match at offset",
            ));
        }
        Ok(())
    }
}

impl<T: Read + Seek> Peek for T {
    fn peek(&mut self, buf: &mut [u8]) -> Result<usize> {
        let current_pos = self.stream_position()?;
        let bytes_read = self.read(buf)?;
        self.seek(SeekFrom::Start(current_pos))?;
        Ok(bytes_read)
    }

    fn peek_exact(&mut self, buf: &mut [u8]) -> Result<()> {
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

    fn peek_exact_at(&mut self, offset: usize, buf: &mut [u8]) -> Result<()> {
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

pub trait CPeek {
    fn cpeek(&self, buf: &mut [u8]) -> Result<usize>;
    fn cpeek_exact(&self, buf: &mut [u8]) -> Result<()> {
        let bytes_read = self.cpeek(buf)?;
        if bytes_read < buf.len() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "Not enough data to read",
            ));
        }
        Ok(())
    }
    fn cpeek_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize>;
    fn cpeek_exact_at(&self, offset: usize, buf: &mut [u8]) -> Result<()> {
        let bytes_read = self.cpeek_at(offset, buf)?;
        if bytes_read < buf.len() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "Not enough data to read",
            ));
        }
        Ok(())
    }
    fn cpeek_at_vec(&self, offset: usize, len: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; len];
        let bytes_read = self.cpeek_at(offset, &mut buf)?;
        if bytes_read < len {
            buf.truncate(bytes_read);
        }
        Ok(buf)
    }
    fn cpeek_exact_at_vec(&self, offset: usize, len: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; len];
        self.cpeek_exact_at(offset, &mut buf)?;
        Ok(buf)
    }

    fn cpeek_u8(&self) -> Result<u8> {
        let mut buf = [0u8; 1];
        self.cpeek_exact(&mut buf)?;
        Ok(buf[0])
    }
    fn cpeek_u16(&self) -> Result<u16> {
        let mut buf = [0u8; 2];
        self.cpeek_exact(&mut buf)?;
        Ok(u16::from_le_bytes(buf))
    }
    fn cpeek_u16_be(&self) -> Result<u16> {
        let mut buf = [0u8; 2];
        self.cpeek_exact(&mut buf)?;
        Ok(u16::from_be_bytes(buf))
    }
    fn cpeek_u32(&self) -> Result<u32> {
        let mut buf = [0u8; 4];
        self.cpeek_exact(&mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }
    fn cpeek_u32_be(&self) -> Result<u32> {
        let mut buf = [0u8; 4];
        self.cpeek_exact(&mut buf)?;
        Ok(u32::from_be_bytes(buf))
    }
    fn cpeek_u64(&self) -> Result<u64> {
        let mut buf = [0u8; 8];
        self.cpeek_exact(&mut buf)?;
        Ok(u64::from_le_bytes(buf))
    }
    fn cpeek_u64_be(&self) -> Result<u64> {
        let mut buf = [0u8; 8];
        self.cpeek_exact(&mut buf)?;
        Ok(u64::from_be_bytes(buf))
    }
    fn cpeek_u128(&self) -> Result<u128> {
        let mut buf = [0u8; 16];
        self.cpeek_exact(&mut buf)?;
        Ok(u128::from_le_bytes(buf))
    }
    fn cpeek_u128_be(&self) -> Result<u128> {
        let mut buf = [0u8; 16];
        self.cpeek_exact(&mut buf)?;
        Ok(u128::from_be_bytes(buf))
    }
    fn cpeek_i8(&self) -> Result<i8> {
        let mut buf = [0u8; 1];
        self.cpeek_exact(&mut buf)?;
        Ok(i8::from_le_bytes(buf))
    }
    fn cpeek_i16(&self) -> Result<i16> {
        let mut buf = [0u8; 2];
        self.cpeek_exact(&mut buf)?;
        Ok(i16::from_le_bytes(buf))
    }
    fn cpeek_i16_be(&self) -> Result<i16> {
        let mut buf = [0u8; 2];
        self.cpeek_exact(&mut buf)?;
        Ok(i16::from_be_bytes(buf))
    }
    fn cpeek_i32(&self) -> Result<i32> {
        let mut buf = [0u8; 4];
        self.cpeek_exact(&mut buf)?;
        Ok(i32::from_le_bytes(buf))
    }
    fn cpeek_i32_be(&self) -> Result<i32> {
        let mut buf = [0u8; 4];
        self.cpeek_exact(&mut buf)?;
        Ok(i32::from_be_bytes(buf))
    }
    fn cpeek_i64(&self) -> Result<i64> {
        let mut buf = [0u8; 8];
        self.cpeek_exact(&mut buf)?;
        Ok(i64::from_le_bytes(buf))
    }
    fn cpeek_i64_be(&self) -> Result<i64> {
        let mut buf = [0u8; 8];
        self.cpeek_exact(&mut buf)?;
        Ok(i64::from_be_bytes(buf))
    }
    fn cpeek_i128(&self) -> Result<i128> {
        let mut buf = [0u8; 16];
        self.cpeek_exact(&mut buf)?;
        Ok(i128::from_le_bytes(buf))
    }
    fn cpeek_i128_be(&self) -> Result<i128> {
        let mut buf = [0u8; 16];
        self.cpeek_exact(&mut buf)?;
        Ok(i128::from_be_bytes(buf))
    }
    fn cpeek_u8_at(&self, offset: usize) -> Result<u8> {
        let mut buf = [0u8; 1];
        self.cpeek_exact_at(offset, &mut buf)?;
        Ok(buf[0])
    }
    fn cpeek_u16_at(&self, offset: usize) -> Result<u16> {
        let mut buf = [0u8; 2];
        self.cpeek_exact_at(offset, &mut buf)?;
        Ok(u16::from_le_bytes(buf))
    }
    fn cpeek_u16_be_at(&self, offset: usize) -> Result<u16> {
        let mut buf = [0u8; 2];
        self.cpeek_exact_at(offset, &mut buf)?;
        Ok(u16::from_be_bytes(buf))
    }
    fn cpeek_u32_at(&self, offset: usize) -> Result<u32> {
        let mut buf = [0u8; 4];
        self.cpeek_exact_at(offset, &mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }
    fn cpeek_u32_be_at(&self, offset: usize) -> Result<u32> {
        let mut buf = [0u8; 4];
        self.cpeek_exact_at(offset, &mut buf)?;
        Ok(u32::from_be_bytes(buf))
    }
    fn cpeek_u64_at(&self, offset: usize) -> Result<u64> {
        let mut buf = [0u8; 8];
        self.cpeek_exact_at(offset, &mut buf)?;
        Ok(u64::from_le_bytes(buf))
    }
    fn cpeek_u64_be_at(&self, offset: usize) -> Result<u64> {
        let mut buf = [0u8; 8];
        self.cpeek_exact_at(offset, &mut buf)?;
        Ok(u64::from_be_bytes(buf))
    }
    fn cpeek_u128_at(&self, offset: usize) -> Result<u128> {
        let mut buf = [0u8; 16];
        self.cpeek_exact_at(offset, &mut buf)?;
        Ok(u128::from_le_bytes(buf))
    }
    fn cpeek_u128_be_at(&self, offset: usize) -> Result<u128> {
        let mut buf = [0u8; 16];
        self.cpeek_exact_at(offset, &mut buf)?;
        Ok(u128::from_be_bytes(buf))
    }
    fn cpeek_i8_at(&self, offset: usize) -> Result<i8> {
        let mut buf = [0u8; 1];
        self.cpeek_exact_at(offset, &mut buf)?;
        Ok(i8::from_le_bytes(buf))
    }
    fn cpeek_i16_at(&self, offset: usize) -> Result<i16> {
        let mut buf = [0u8; 2];
        self.cpeek_exact_at(offset, &mut buf)?;
        Ok(i16::from_le_bytes(buf))
    }
    fn cpeek_i16_be_at(&self, offset: usize) -> Result<i16> {
        let mut buf = [0u8; 2];
        self.cpeek_exact_at(offset, &mut buf)?;
        Ok(i16::from_be_bytes(buf))
    }
    fn cpeek_i32_at(&self, offset: usize) -> Result<i32> {
        let mut buf = [0u8; 4];
        self.cpeek_exact_at(offset, &mut buf)?;
        Ok(i32::from_le_bytes(buf))
    }
    fn cpeek_i32_be_at(&self, offset: usize) -> Result<i32> {
        let mut buf = [0u8; 4];
        self.cpeek_exact_at(offset, &mut buf)?;
        Ok(i32::from_be_bytes(buf))
    }
    fn cpeek_i64_at(&self, offset: usize) -> Result<i64> {
        let mut buf = [0u8; 8];
        self.cpeek_exact_at(offset, &mut buf)?;
        Ok(i64::from_le_bytes(buf))
    }
    fn cpeek_i64_be_at(&self, offset: usize) -> Result<i64> {
        let mut buf = [0u8; 8];
        self.cpeek_exact_at(offset, &mut buf)?;
        Ok(i64::from_be_bytes(buf))
    }
    fn cpeek_i128_at(&self, offset: usize) -> Result<i128> {
        let mut buf = [0u8; 16];
        self.cpeek_exact_at(offset, &mut buf)?;
        Ok(i128::from_le_bytes(buf))
    }
    fn cpeek_i128_be_at(&self, offset: usize) -> Result<i128> {
        let mut buf = [0u8; 16];
        self.cpeek_exact_at(offset, &mut buf)?;
        Ok(i128::from_be_bytes(buf))
    }

    fn cpeek_cstring(&self) -> Result<CString>;

    fn cpeek_cstring_at(&self, offset: usize) -> Result<CString> {
        let mut buf = Vec::new();
        let mut byte = [0u8; 1];
        self.cpeek_at(offset, &mut byte)?;
        while byte[0] != 0 {
            buf.push(byte[0]);
            self.cpeek_at(offset + buf.len(), &mut byte)?;
        }
        CString::new(buf).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    fn cpeek_and_equal(&self, data: &[u8]) -> Result<()> {
        let mut buf = vec![0u8; data.len()];
        self.cpeek_exact(&mut buf)?;
        if buf != data {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Data does not match",
            ));
        }
        Ok(())
    }
    fn cpeek_and_equal_at(&self, offset: usize, data: &[u8]) -> Result<()> {
        let mut buf = vec![0u8; data.len()];
        self.cpeek_exact_at(offset, &mut buf)?;
        if buf != data {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Data does not match at offset",
            ));
        }
        Ok(())
    }
}

impl<T: Peek> CPeek for Mutex<T> {
    fn cpeek(&self, buf: &mut [u8]) -> Result<usize> {
        let mut lock = self.lock().map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::Other, "Failed to lock the mutex")
        })?;
        lock.peek(buf)
    }

    fn cpeek_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        let mut lock = self.lock().map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::Other, "Failed to lock the mutex")
        })?;
        lock.peek_at(offset, buf)
    }

    fn cpeek_cstring(&self) -> Result<CString> {
        let mut lock = self.lock().map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::Other, "Failed to lock the mutex")
        })?;
        lock.peek_cstring()
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

    fn read_and_equal(&mut self, data: &[u8]) -> Result<()>;
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
        let s = decode_to_string(encoding, &buf, true)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(s)
    }

    fn read_exact_vec(&mut self, len: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; len];
        self.read_exact(&mut buf)?;
        Ok(buf)
    }

    fn read_and_equal(&mut self, data: &[u8]) -> Result<()> {
        let mut buf = vec![0u8; data.len()];
        self.read_exact(&mut buf)?;
        if buf != data {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Data does not match",
            ));
        }
        Ok(())
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

    fn write_cstring(&mut self, value: &CString) -> Result<()>;
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

    fn write_cstring(&mut self, value: &CString) -> Result<()> {
        self.write_all(value.as_bytes_with_nul())
    }
}

pub trait WriteAt {
    fn write_at(&mut self, offset: usize, buf: &[u8]) -> Result<usize>;
    fn write_all_at(&mut self, offset: usize, buf: &[u8]) -> Result<()>;

    fn write_u8_at(&mut self, offset: usize, value: u8) -> Result<()> {
        self.write_all_at(offset, &value.to_le_bytes())
    }
    fn write_u16_at(&mut self, offset: usize, value: u16) -> Result<()> {
        self.write_all_at(offset, &value.to_le_bytes())
    }
    fn write_u16_be_at(&mut self, offset: usize, value: u16) -> Result<()> {
        self.write_all_at(offset, &value.to_be_bytes())
    }
    fn write_u32_at(&mut self, offset: usize, value: u32) -> Result<()> {
        self.write_all_at(offset, &value.to_le_bytes())
    }
    fn write_u32_be_at(&mut self, offset: usize, value: u32) -> Result<()> {
        self.write_all_at(offset, &value.to_be_bytes())
    }
    fn write_u64_at(&mut self, offset: usize, value: u64) -> Result<()> {
        self.write_all_at(offset, &value.to_le_bytes())
    }
    fn write_u64_be_at(&mut self, offset: usize, value: u64) -> Result<()> {
        self.write_all_at(offset, &value.to_be_bytes())
    }
    fn write_u128_at(&mut self, offset: usize, value: u128) -> Result<()> {
        self.write_all_at(offset, &value.to_le_bytes())
    }
    fn write_u128_be_at(&mut self, offset: usize, value: u128) -> Result<()> {
        self.write_all_at(offset, &value.to_be_bytes())
    }
    fn write_i8_at(&mut self, offset: usize, value: i8) -> Result<()> {
        self.write_all_at(offset, &value.to_le_bytes())
    }
    fn write_i16_at(&mut self, offset: usize, value: i16) -> Result<()> {
        self.write_all_at(offset, &value.to_le_bytes())
    }
    fn write_i16_be_at(&mut self, offset: usize, value: i16) -> Result<()> {
        self.write_all_at(offset, &value.to_be_bytes())
    }
    fn write_i32_at(&mut self, offset: usize, value: i32) -> Result<()> {
        self.write_all_at(offset, &value.to_le_bytes())
    }
    fn write_i32_be_at(&mut self, offset: usize, value: i32) -> Result<()> {
        self.write_all_at(offset, &value.to_be_bytes())
    }
    fn write_i64_at(&mut self, offset: usize, value: i64) -> Result<()> {
        self.write_all_at(offset, &value.to_le_bytes())
    }
    fn write_i64_be_at(&mut self, offset: usize, value: i64) -> Result<()> {
        self.write_all_at(offset, &value.to_be_bytes())
    }
    fn write_i128_at(&mut self, offset: usize, value: i128) -> Result<()> {
        self.write_all_at(offset, &value.to_le_bytes())
    }
    fn write_i128_be_at(&mut self, offset: usize, value: i128) -> Result<()> {
        self.write_all_at(offset, &value.to_be_bytes())
    }

    fn write_cstring_at(&mut self, offset: usize, value: &CString) -> Result<()> {
        self.write_all_at(offset, value.as_bytes_with_nul())
    }
}

impl<T: Write + Seek> WriteAt for T {
    fn write_at(&mut self, offset: usize, buf: &[u8]) -> Result<usize> {
        let current_pos = self.stream_position()?;
        self.seek(SeekFrom::Start(offset as u64))?;
        let bytes_written = self.write(buf)?;
        self.seek(SeekFrom::Start(current_pos))?;
        Ok(bytes_written)
    }

    fn write_all_at(&mut self, offset: usize, buf: &[u8]) -> Result<()> {
        let current_pos = self.stream_position()?;
        self.seek(SeekFrom::Start(offset as u64))?;
        self.write_all(buf)?;
        self.seek(SeekFrom::Start(current_pos))?;
        Ok(())
    }
}

pub trait SeekExt {
    fn stream_length(&mut self) -> Result<u64>;
}

impl<T: Seek> SeekExt for T {
    fn stream_length(&mut self) -> Result<u64> {
        let current_pos = self.stream_position()?;
        let length = self.seek(SeekFrom::End(0))?;
        self.seek(SeekFrom::Start(current_pos))?;
        Ok(length)
    }
}

pub struct MemReader {
    pub data: Vec<u8>,
    pub pos: usize,
}

#[derive(Clone)]
pub struct MemReaderRef<'a> {
    pub data: &'a [u8],
    pub pos: usize,
}

impl std::fmt::Debug for MemReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemReader")
            .field("pos", &self.pos)
            .field("data_length", &self.data.len())
            .finish_non_exhaustive()
    }
}

impl<'a> std::fmt::Debug for MemReaderRef<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemReaderRef")
            .field("pos", &self.pos)
            .field("data_length", &self.data.len())
            .finish_non_exhaustive()
    }
}

impl MemReader {
    pub fn new(data: Vec<u8>) -> Self {
        MemReader { data, pos: 0 }
    }

    pub fn to_ref<'a>(&'a self) -> MemReaderRef<'a> {
        MemReaderRef {
            data: &self.data,
            pos: self.pos,
        }
    }

    pub fn is_eof(&self) -> bool {
        self.pos >= self.data.len()
    }

    pub fn inner(self) -> Vec<u8> {
        self.data
    }
}

impl<'a> MemReaderRef<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        MemReaderRef { data, pos: 0 }
    }

    pub fn is_eof(&self) -> bool {
        self.pos >= self.data.len()
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

impl CPeek for MemReader {
    fn cpeek(&self, buf: &mut [u8]) -> Result<usize> {
        self.to_ref().cpeek(buf)
    }

    fn cpeek_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        self.to_ref().cpeek_at(offset, buf)
    }

    fn cpeek_cstring(&self) -> Result<CString> {
        self.to_ref().cpeek_cstring()
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

impl<'a> CPeek for MemReaderRef<'a> {
    fn cpeek(&self, buf: &mut [u8]) -> Result<usize> {
        let len = self.data.len();
        let bytes_to_read = std::cmp::min(buf.len(), len - self.pos);
        buf[..bytes_to_read].copy_from_slice(&self.data[self.pos..self.pos + bytes_to_read]);
        Ok(bytes_to_read)
    }

    fn cpeek_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        let len = self.data.len();
        if offset >= len {
            return Ok(0);
        }
        let bytes_to_read = std::cmp::min(buf.len(), len - offset);
        buf[..bytes_to_read].copy_from_slice(&self.data[offset..offset + bytes_to_read]);
        Ok(bytes_to_read)
    }

    fn cpeek_cstring(&self) -> Result<CString> {
        let mut buf = Vec::new();
        for &byte in &self.data[self.pos..] {
            if byte == 0 {
                break;
            }
            buf.push(byte);
        }
        CString::new(buf).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }
}

pub struct MemWriter {
    pub data: Vec<u8>,
    pub pos: usize,
}

impl MemWriter {
    pub fn new() -> Self {
        MemWriter {
            data: Vec::new(),
            pos: 0,
        }
    }

    pub fn from_vec(data: Vec<u8>) -> Self {
        MemWriter { data, pos: 0 }
    }

    pub fn into_inner(self) -> Vec<u8> {
        self.data
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    pub fn to_ref<'a>(&'a self) -> MemReaderRef<'a> {
        MemReaderRef {
            data: &self.data,
            pos: self.pos,
        }
    }
}

impl std::fmt::Debug for MemWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemWriter")
            .field("pos", &self.pos)
            .field("data_length", &self.data.len())
            .finish_non_exhaustive()
    }
}

impl Write for MemWriter {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        if self.pos + buf.len() > self.data.len() {
            self.data.resize(self.pos + buf.len(), 0);
        }
        let bytes_written = buf.len();
        self.data[self.pos..self.pos + bytes_written].copy_from_slice(buf);
        self.pos += bytes_written;
        Ok(bytes_written)
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

impl Seek for MemWriter {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        match pos {
            SeekFrom::Start(offset) => {
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
                let new_pos = self.pos as i64 + offset;
                if new_pos < 0 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Seek position is negative",
                    ));
                }
                self.pos = new_pos as usize;
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

impl CPeek for MemWriter {
    fn cpeek(&self, buf: &mut [u8]) -> Result<usize> {
        self.to_ref().cpeek(buf)
    }

    fn cpeek_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        self.to_ref().cpeek_at(offset, buf)
    }

    fn cpeek_cstring(&self) -> Result<CString> {
        self.to_ref().cpeek_cstring()
    }
}

pub struct StreamRegion<T: Seek> {
    stream: T,
    start_pos: u64,
    end_pos: u64,
    cur_pos: u64,
}

impl<T: Seek> StreamRegion<T> {
    pub fn new(stream: T, start_pos: u64, end_pos: u64) -> Result<Self> {
        if start_pos > end_pos {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Start position cannot be greater than end position",
            ));
        }
        Ok(Self {
            stream,
            start_pos,
            end_pos,
            cur_pos: 0,
        })
    }

    pub fn with_start_pos(mut stream: T, start_pos: u64) -> Result<Self> {
        let end_pos = stream.stream_length()?;
        Self::new(stream, start_pos, end_pos)
    }
}

impl<T: Read + Seek> Read for StreamRegion<T> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        if self.cur_pos + self.start_pos >= self.end_pos {
            return Ok(0); // EOF
        }
        self.stream
            .seek(SeekFrom::Start(self.start_pos + self.cur_pos))?;
        let bytes_to_read = (self.end_pos - self.start_pos - self.cur_pos) as usize;
        let m = buf.len().min(bytes_to_read);
        let readed = self.stream.read(&mut buf[..m])?;
        self.cur_pos += readed as u64;
        Ok(readed)
    }
}

impl<T: Seek> Seek for StreamRegion<T> {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        let new_pos = match pos {
            SeekFrom::Start(offset) => self.start_pos + offset,
            SeekFrom::End(offset) => (self.end_pos as i64 + offset as i64) as u64,
            SeekFrom::Current(offset) => {
                (self.start_pos as i64 + self.cur_pos as i64 + offset as i64) as u64
            }
        };
        if new_pos < self.start_pos || new_pos > self.end_pos {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Seek position out of bounds",
            ));
        }
        self.cur_pos = new_pos - self.start_pos;
        self.stream.seek(SeekFrom::Start(new_pos))
    }

    fn stream_position(&mut self) -> Result<u64> {
        Ok(self.cur_pos)
    }

    fn rewind(&mut self) -> Result<()> {
        self.cur_pos = 0;
        self.stream.seek(SeekFrom::Start(self.start_pos))?;
        Ok(())
    }
}
