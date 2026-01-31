//!Extensions for IO operations.
use crate::scripts::base::ReadSeek;
use crate::types::Encoding;
use crate::utils::encoding::{decode_to_string, encode_string};
use crate::utils::struct_pack::{StructPack, StructUnpack};
use std::ffi::CString;
use std::io::*;
use std::sync::{Arc, Mutex};

/// A trait to help to peek data from a reader.
pub trait Peek {
    /// Peeks data from the reader into the provided buffer.
    /// Returns the number of bytes read.
    fn peek(&mut self, buf: &mut [u8]) -> Result<usize>;
    /// Peeks data from the reader into the provided buffer.
    /// Returns an error if the buffer is not filled completely.
    fn peek_exact(&mut self, buf: &mut [u8]) -> Result<()>;
    /// Peeks data from the reader at a specific offset into the provided buffer.
    /// Returns the number of bytes read.
    fn peek_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize>;
    /// Peeks data from the reader at a specific offset into the provided buffer.
    /// Returns an error if the buffer is not filled completely.
    fn peek_exact_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<()>;
    /// Peeks data from the reader at a specific offset into a vector.
    /// Returns the vector containing the data read.
    fn peek_at_vec(&mut self, offset: u64, len: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; len];
        let bytes_read = self.peek_at(offset, &mut buf)?;
        if bytes_read < len {
            buf.truncate(bytes_read);
        }
        Ok(buf)
    }
    /// Peeks data from the reader at a specific offset into a vector.
    /// Returns an error if the buffer is not filled completely.
    fn peek_exact_at_vec(&mut self, offset: u64, len: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; len];
        self.peek_exact_at(offset, &mut buf)?;
        Ok(buf)
    }

    /// Peeks a [u8] from the reader.
    fn peek_u8(&mut self) -> Result<u8> {
        let mut buf = [0u8; 1];
        self.peek_exact(&mut buf)?;
        Ok(buf[0])
    }
    /// Peeks a [u16] from the reader in little-endian order.
    fn peek_u16(&mut self) -> Result<u16> {
        let mut buf = [0u8; 2];
        self.peek_exact(&mut buf)?;
        Ok(u16::from_le_bytes(buf))
    }
    /// Peeks a [u16] from the reader in big-endian order.
    fn peek_u16_be(&mut self) -> Result<u16> {
        let mut buf = [0u8; 2];
        self.peek_exact(&mut buf)?;
        Ok(u16::from_be_bytes(buf))
    }
    /// Peeks a [u32] from the reader in little-endian order.
    fn peek_u32(&mut self) -> Result<u32> {
        let mut buf = [0u8; 4];
        self.peek_exact(&mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }
    /// Peeks a [u32] from the reader in big-endian order.
    fn peek_u32_be(&mut self) -> Result<u32> {
        let mut buf = [0u8; 4];
        self.peek_exact(&mut buf)?;
        Ok(u32::from_be_bytes(buf))
    }
    /// Peeks a [u64] from the reader in little-endian order.
    fn peek_u64(&mut self) -> Result<u64> {
        let mut buf = [0u8; 8];
        self.peek_exact(&mut buf)?;
        Ok(u64::from_le_bytes(buf))
    }
    /// Peeks a [u64] from the reader in big-endian order.
    fn peek_u64_be(&mut self) -> Result<u64> {
        let mut buf = [0u8; 8];
        self.peek_exact(&mut buf)?;
        Ok(u64::from_be_bytes(buf))
    }
    /// Peeks a [u128] from the reader in little-endian order.
    fn peek_u128(&mut self) -> Result<u128> {
        let mut buf = [0u8; 16];
        self.peek_exact(&mut buf)?;
        Ok(u128::from_le_bytes(buf))
    }
    /// Peeks a [u128] from the reader in big-endian order.
    fn peek_u128_be(&mut self) -> Result<u128> {
        let mut buf = [0u8; 16];
        self.peek_exact(&mut buf)?;
        Ok(u128::from_be_bytes(buf))
    }
    /// Peeks an [i8] from the reader.
    fn peek_i8(&mut self) -> Result<i8> {
        let mut buf = [0u8; 1];
        self.peek_exact(&mut buf)?;
        Ok(i8::from_le_bytes(buf))
    }
    /// Peeks an [i16] from the reader in little-endian order.
    fn peek_i16(&mut self) -> Result<i16> {
        let mut buf = [0u8; 2];
        self.peek_exact(&mut buf)?;
        Ok(i16::from_le_bytes(buf))
    }
    /// Peeks an [i16] from the reader in big-endian order.
    fn peek_i16_be(&mut self) -> Result<i16> {
        let mut buf = [0u8; 2];
        self.peek_exact(&mut buf)?;
        Ok(i16::from_be_bytes(buf))
    }
    /// Peeks an [i32] from the reader in little-endian order.
    fn peek_i32(&mut self) -> Result<i32> {
        let mut buf = [0u8; 4];
        self.peek_exact(&mut buf)?;
        Ok(i32::from_le_bytes(buf))
    }
    /// Peeks an [i32] from the reader in big-endian order.
    fn peek_i32_be(&mut self) -> Result<i32> {
        let mut buf = [0u8; 4];
        self.peek_exact(&mut buf)?;
        Ok(i32::from_be_bytes(buf))
    }
    /// Peeks an [i64] from the reader in little-endian order.
    fn peek_i64(&mut self) -> Result<i64> {
        let mut buf = [0u8; 8];
        self.peek_exact(&mut buf)?;
        Ok(i64::from_le_bytes(buf))
    }
    /// Peeks an [i64] from the reader in big-endian order.
    fn peek_i64_be(&mut self) -> Result<i64> {
        let mut buf = [0u8; 8];
        self.peek_exact(&mut buf)?;
        Ok(i64::from_be_bytes(buf))
    }
    /// Peeks an [i128] from the reader in little-endian order.
    fn peek_i128(&mut self) -> Result<i128> {
        let mut buf = [0u8; 16];
        self.peek_exact(&mut buf)?;
        Ok(i128::from_le_bytes(buf))
    }
    /// Peeks an [i128] from the reader in big-endian order.
    fn peek_i128_be(&mut self) -> Result<i128> {
        let mut buf = [0u8; 16];
        self.peek_exact(&mut buf)?;
        Ok(i128::from_be_bytes(buf))
    }
    /// Peeks a [u8] at a specific offset from the reader.
    fn peek_u8_at(&mut self, offset: u64) -> Result<u8> {
        let mut buf = [0u8; 1];
        self.peek_exact_at(offset, &mut buf)?;
        Ok(buf[0])
    }
    /// Peeks a [u16] at a specific offset from the reader in little-endian order.
    fn peek_u16_at(&mut self, offset: u64) -> Result<u16> {
        let mut buf = [0u8; 2];
        self.peek_exact_at(offset, &mut buf)?;
        Ok(u16::from_le_bytes(buf))
    }
    /// Peeks a [u16] at a specific offset from the reader in big-endian order.
    fn peek_u16_be_at(&mut self, offset: u64) -> Result<u16> {
        let mut buf = [0u8; 2];
        self.peek_exact_at(offset, &mut buf)?;
        Ok(u16::from_be_bytes(buf))
    }
    /// Peeks a [u32] at a specific offset from the reader in little-endian order.
    fn peek_u32_at(&mut self, offset: u64) -> Result<u32> {
        let mut buf = [0u8; 4];
        self.peek_exact_at(offset, &mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }
    /// Peeks a [u32] at a specific offset from the reader in big-endian order.
    fn peek_u32_be_at(&mut self, offset: u64) -> Result<u32> {
        let mut buf = [0u8; 4];
        self.peek_exact_at(offset, &mut buf)?;
        Ok(u32::from_be_bytes(buf))
    }
    /// Peeks a [u64] at a specific offset from the reader in little-endian order.
    fn peek_u64_at(&mut self, offset: u64) -> Result<u64> {
        let mut buf = [0u8; 8];
        self.peek_exact_at(offset, &mut buf)?;
        Ok(u64::from_le_bytes(buf))
    }
    /// Peeks a [u64] at a specific offset from the reader in big-endian order.
    fn peek_u64_be_at(&mut self, offset: u64) -> Result<u64> {
        let mut buf = [0u8; 8];
        self.peek_exact_at(offset, &mut buf)?;
        Ok(u64::from_be_bytes(buf))
    }
    /// Peeks a [u128] at a specific offset from the reader in little-endian order.
    fn peek_u128_at(&mut self, offset: u64) -> Result<u128> {
        let mut buf = [0u8; 16];
        self.peek_exact_at(offset, &mut buf)?;
        Ok(u128::from_le_bytes(buf))
    }
    /// Peeks a [u128] at a specific offset from the reader in big-endian order.
    fn peek_u128_be_at(&mut self, offset: u64) -> Result<u128> {
        let mut buf = [0u8; 16];
        self.peek_exact_at(offset, &mut buf)?;
        Ok(u128::from_be_bytes(buf))
    }
    /// Peeks an [i8] at a specific offset from the reader.
    fn peek_i8_at(&mut self, offset: u64) -> Result<i8> {
        let mut buf = [0u8; 1];
        self.peek_exact_at(offset, &mut buf)?;
        Ok(i8::from_le_bytes(buf))
    }
    /// Peeks an [i16] at a specific offset from the reader in little-endian order.
    fn peek_i16_at(&mut self, offset: u64) -> Result<i16> {
        let mut buf = [0u8; 2];
        self.peek_exact_at(offset, &mut buf)?;
        Ok(i16::from_le_bytes(buf))
    }
    /// Peeks an [i16] at a specific offset from the reader in big-endian order.
    fn peek_i16_be_at(&mut self, offset: u64) -> Result<i16> {
        let mut buf = [0u8; 2];
        self.peek_exact_at(offset, &mut buf)?;
        Ok(i16::from_be_bytes(buf))
    }
    /// Peeks an [i32] at a specific offset from the reader in little-endian order.
    fn peek_i32_at(&mut self, offset: u64) -> Result<i32> {
        let mut buf = [0u8; 4];
        self.peek_exact_at(offset, &mut buf)?;
        Ok(i32::from_le_bytes(buf))
    }
    /// Peeks an [i32] at a specific offset from the reader in big-endian order.
    fn peek_i32_be_at(&mut self, offset: u64) -> Result<i32> {
        let mut buf = [0u8; 4];
        self.peek_exact_at(offset, &mut buf)?;
        Ok(i32::from_be_bytes(buf))
    }
    /// Peeks an [i64] at a specific offset from the reader in little-endian order.
    fn peek_i64_at(&mut self, offset: u64) -> Result<i64> {
        let mut buf = [0u8; 8];
        self.peek_exact_at(offset, &mut buf)?;
        Ok(i64::from_le_bytes(buf))
    }
    /// Peeks an [i64] at a specific offset from the reader in big-endian order.
    fn peek_i64_be_at(&mut self, offset: u64) -> Result<i64> {
        let mut buf = [0u8; 8];
        self.peek_exact_at(offset, &mut buf)?;
        Ok(i64::from_be_bytes(buf))
    }
    /// Peeks an [i128] at a specific offset from the reader in little-endian order.
    fn peek_i128_at(&mut self, offset: u64) -> Result<i128> {
        let mut buf = [0u8; 16];
        self.peek_exact_at(offset, &mut buf)?;
        Ok(i128::from_le_bytes(buf))
    }
    /// Peeks an [i128] at a specific offset from the reader in big-endian order.
    fn peek_i128_be_at(&mut self, offset: u64) -> Result<i128> {
        let mut buf = [0u8; 16];
        self.peek_exact_at(offset, &mut buf)?;
        Ok(i128::from_be_bytes(buf))
    }

    /// Peeks a C-style string (null-terminated) from the reader.
    fn peek_cstring(&mut self) -> Result<CString>;
    /// Peeks a C-style string (null-terminated) from the reader at a specific offset.
    fn peek_cstring_at(&mut self, offset: u64) -> Result<CString>;
    /// Peeks a fixed-length string from the reader.
    fn peek_fstring(&mut self, len: usize, encoding: Encoding, trim: bool) -> Result<String> {
        let mut buf = vec![0u8; len];
        self.peek_exact(&mut buf)?;
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
    /// Peeks a fixed-length string from the reader at a specific offset.
    fn peek_fstring_at(
        &mut self,
        offset: u64,
        len: usize,
        encoding: Encoding,
        trim: bool,
    ) -> Result<String> {
        let mut buf = vec![0u8; len];
        self.peek_exact_at(offset, &mut buf)?;
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

    /// Peeks a UTF-16 string (null-terminated) from the reader.
    /// Returns the raw bytes of the UTF-16 string. (Null terminator is not included)
    fn peek_u16string(&mut self) -> Result<Vec<u8>>;
    /// Peeks a UTF-16 string (null-terminated) from the reader at a specific offset.
    /// Returns the raw bytes of the UTF-16 string. (Null terminator is not included)
    fn peek_u16string_at(&mut self, offset: u64) -> Result<Vec<u8>>;

    /// Reads a struct from the reader.
    /// The struct must implement the `StructUnpack` trait.
    ///
    /// * `big` indicates whether the struct is in big-endian format.
    /// * `encoding` specifies the encoding to use for string fields in the struct.
    /// Returns the unpacked struct.
    fn read_struct<T: StructUnpack>(
        &mut self,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn std::any::Any>>,
    ) -> Result<T>;
    /// Reads a vector of structs from the reader.
    /// The structs must implement the `StructUnpack` trait.
    ///
    /// * `count` is the number of structs to read.
    /// * `big` indicates whether the structs are in big-endian format.
    /// * `encoding` specifies the encoding to use for string fields in the structs.
    /// Returns a vector of unpacked structs.
    fn read_struct_vec<T: StructUnpack>(
        &mut self,
        count: usize,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn std::any::Any>>,
    ) -> Result<Vec<T>> {
        self.read_struct_vec2(count, big, encoding, info, 4194304)
    }
    /// Reads a vector of structs from the reader.
    /// The structs must implement the `StructUnpack` trait.
    ///
    /// * `count` is the number of structs to read.
    /// * `big` indicates whether the structs are in big-endian format.
    /// * `encoding` specifies the encoding to use for string fields in the structs.
    /// Returns a vector of unpacked structs.
    fn read_struct_vec2<T: StructUnpack>(
        &mut self,
        count: usize,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn std::any::Any>>,
        max_preallocated_size: usize,
    ) -> Result<Vec<T>> {
        let mut vec = if size_of::<T>() * count <= max_preallocated_size {
            Vec::with_capacity(count)
        } else {
            Vec::new()
        };
        for _ in 0..count {
            vec.push(self.read_struct(big, encoding, info)?);
        }
        Ok(vec)
    }

    /// Peeks data and checks if it matches the provided data.
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
    /// Peeks data at a specific offset and checks if it matches the provided data.
    fn peek_and_equal_at(&mut self, offset: u64, data: &[u8]) -> Result<()> {
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

    fn peek_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        let current_pos = self.stream_position()?;
        self.seek(SeekFrom::Start(offset))?;
        let bytes_read = self.read(buf)?;
        self.seek(SeekFrom::Start(current_pos))?;
        Ok(bytes_read)
    }

    fn peek_exact_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<()> {
        let current_pos = self.stream_position()?;
        self.seek(SeekFrom::Start(offset))?;
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

    fn peek_cstring_at(&mut self, offset: u64) -> Result<CString> {
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

    fn peek_u16string(&mut self) -> Result<Vec<u8>> {
        let current_pos = self.stream_position()?;
        let mut buf = Vec::new();
        loop {
            let mut bytes = [0u8; 2];
            self.read_exact(&mut bytes)?;
            if bytes == [0, 0] {
                break;
            }
            buf.extend_from_slice(&bytes);
        }
        self.seek(SeekFrom::Start(current_pos))?;
        Ok(buf)
    }

    fn peek_u16string_at(&mut self, offset: u64) -> Result<Vec<u8>> {
        let current_pos = self.stream_position()?;
        let mut buf = Vec::new();
        self.seek(SeekFrom::Start(offset as u64))?;
        loop {
            let mut bytes = [0u8; 2];
            self.read_exact(&mut bytes)?;
            if bytes == [0, 0] {
                break;
            }
            buf.extend_from_slice(&bytes);
        }
        self.seek(SeekFrom::Start(current_pos))?;
        Ok(buf)
    }

    fn read_struct<S: StructUnpack>(
        &mut self,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn std::any::Any>>,
    ) -> Result<S> {
        S::unpack(self, big, encoding, info)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }
}

/// A trait to help to peek data from a reader in a thread-safe manner.
pub trait CPeek {
    /// Peeks data from the reader into the provided buffer.
    /// Returns the number of bytes read.
    fn cpeek(&self, buf: &mut [u8]) -> Result<usize>;
    /// Peeks data from the reader into the provided buffer.
    /// Returns an error if the buffer is not filled completely.
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
    /// Peeks data from the reader at a specific offset into the provided buffer.
    /// Returns the number of bytes read.
    fn cpeek_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize>;
    /// Peeks data from the reader at a specific offset into the provided buffer.
    /// Returns an error if the buffer is not filled completely.
    fn cpeek_exact_at(&self, offset: u64, buf: &mut [u8]) -> Result<()> {
        let bytes_read = self.cpeek_at(offset, buf)?;
        if bytes_read < buf.len() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "Not enough data to read",
            ));
        }
        Ok(())
    }
    /// Peeks data from the reader at a specific offset into a vector.
    /// Returns the vector containing the data read.
    fn cpeek_at_vec(&self, offset: u64, len: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; len];
        let bytes_read = self.cpeek_at(offset, &mut buf)?;
        if bytes_read < len {
            buf.truncate(bytes_read);
        }
        Ok(buf)
    }
    /// Peeks data from the reader at a specific offset into a vector.
    /// Returns an error if the buffer is not filled completely.
    fn cpeek_exact_at_vec(&self, offset: u64, len: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; len];
        self.cpeek_exact_at(offset, &mut buf)?;
        Ok(buf)
    }

    /// Peeks a [u8] from the reader.
    fn cpeek_u8(&self) -> Result<u8> {
        let mut buf = [0u8; 1];
        self.cpeek_exact(&mut buf)?;
        Ok(buf[0])
    }
    /// Peeks a [u16] from the reader in little-endian order.
    fn cpeek_u16(&self) -> Result<u16> {
        let mut buf = [0u8; 2];
        self.cpeek_exact(&mut buf)?;
        Ok(u16::from_le_bytes(buf))
    }
    /// Peeks a [u16] from the reader in big-endian order.
    fn cpeek_u16_be(&self) -> Result<u16> {
        let mut buf = [0u8; 2];
        self.cpeek_exact(&mut buf)?;
        Ok(u16::from_be_bytes(buf))
    }
    /// Peeks a [u32] from the reader in little-endian order.
    fn cpeek_u32(&self) -> Result<u32> {
        let mut buf = [0u8; 4];
        self.cpeek_exact(&mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }
    /// Peeks a [u32] from the reader in big-endian order.
    fn cpeek_u32_be(&self) -> Result<u32> {
        let mut buf = [0u8; 4];
        self.cpeek_exact(&mut buf)?;
        Ok(u32::from_be_bytes(buf))
    }
    /// Peeks a [u64] from the reader in little-endian order.
    fn cpeek_u64(&self) -> Result<u64> {
        let mut buf = [0u8; 8];
        self.cpeek_exact(&mut buf)?;
        Ok(u64::from_le_bytes(buf))
    }
    /// Peeks a [u64] from the reader in big-endian order.
    fn cpeek_u64_be(&self) -> Result<u64> {
        let mut buf = [0u8; 8];
        self.cpeek_exact(&mut buf)?;
        Ok(u64::from_be_bytes(buf))
    }
    /// Peeks a [u128] from the reader in little-endian order.
    fn cpeek_u128(&self) -> Result<u128> {
        let mut buf = [0u8; 16];
        self.cpeek_exact(&mut buf)?;
        Ok(u128::from_le_bytes(buf))
    }
    /// Peeks a [u128] from the reader in big-endian order.
    fn cpeek_u128_be(&self) -> Result<u128> {
        let mut buf = [0u8; 16];
        self.cpeek_exact(&mut buf)?;
        Ok(u128::from_be_bytes(buf))
    }
    /// Peeks an [i8] from the reader.
    fn cpeek_i8(&self) -> Result<i8> {
        let mut buf = [0u8; 1];
        self.cpeek_exact(&mut buf)?;
        Ok(i8::from_le_bytes(buf))
    }
    /// Peeks an [i16] from the reader in little-endian order.
    fn cpeek_i16(&self) -> Result<i16> {
        let mut buf = [0u8; 2];
        self.cpeek_exact(&mut buf)?;
        Ok(i16::from_le_bytes(buf))
    }
    /// Peeks an [i16] from the reader in big-endian order.
    fn cpeek_i16_be(&self) -> Result<i16> {
        let mut buf = [0u8; 2];
        self.cpeek_exact(&mut buf)?;
        Ok(i16::from_be_bytes(buf))
    }
    /// Peeks an [i32] from the reader in little-endian order.
    fn cpeek_i32(&self) -> Result<i32> {
        let mut buf = [0u8; 4];
        self.cpeek_exact(&mut buf)?;
        Ok(i32::from_le_bytes(buf))
    }
    /// Peeks an [i32] from the reader in big-endian order.
    fn cpeek_i32_be(&self) -> Result<i32> {
        let mut buf = [0u8; 4];
        self.cpeek_exact(&mut buf)?;
        Ok(i32::from_be_bytes(buf))
    }
    /// Peeks an [i64] from the reader in little-endian order.
    fn cpeek_i64(&self) -> Result<i64> {
        let mut buf = [0u8; 8];
        self.cpeek_exact(&mut buf)?;
        Ok(i64::from_le_bytes(buf))
    }
    /// Peeks an [i64] from the reader in big-endian order.
    fn cpeek_i64_be(&self) -> Result<i64> {
        let mut buf = [0u8; 8];
        self.cpeek_exact(&mut buf)?;
        Ok(i64::from_be_bytes(buf))
    }
    /// Peeks an [i128] from the reader in little-endian order.
    fn cpeek_i128(&self) -> Result<i128> {
        let mut buf = [0u8; 16];
        self.cpeek_exact(&mut buf)?;
        Ok(i128::from_le_bytes(buf))
    }
    /// Peeks an [i128] from the reader in big-endian order.
    fn cpeek_i128_be(&self) -> Result<i128> {
        let mut buf = [0u8; 16];
        self.cpeek_exact(&mut buf)?;
        Ok(i128::from_be_bytes(buf))
    }
    /// Peeks a [u8] at a specific offset from the reader.
    fn cpeek_u8_at(&self, offset: u64) -> Result<u8> {
        let mut buf = [0u8; 1];
        self.cpeek_exact_at(offset, &mut buf)?;
        Ok(buf[0])
    }
    /// Peeks a [u16] at a specific offset from the reader in little-endian order.
    fn cpeek_u16_at(&self, offset: u64) -> Result<u16> {
        let mut buf = [0u8; 2];
        self.cpeek_exact_at(offset, &mut buf)?;
        Ok(u16::from_le_bytes(buf))
    }
    /// Peeks a [u16] at a specific offset from the reader in big-endian order.
    fn cpeek_u16_be_at(&self, offset: u64) -> Result<u16> {
        let mut buf = [0u8; 2];
        self.cpeek_exact_at(offset, &mut buf)?;
        Ok(u16::from_be_bytes(buf))
    }
    /// Peeks a [u32] at a specific offset from the reader in little-endian order.
    fn cpeek_u32_at(&self, offset: u64) -> Result<u32> {
        let mut buf = [0u8; 4];
        self.cpeek_exact_at(offset, &mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }
    /// Peeks a [u32] at a specific offset from the reader in big-endian order.
    fn cpeek_u32_be_at(&self, offset: u64) -> Result<u32> {
        let mut buf = [0u8; 4];
        self.cpeek_exact_at(offset, &mut buf)?;
        Ok(u32::from_be_bytes(buf))
    }
    /// Peeks a [u64] at a specific offset from the reader in little-endian order.
    fn cpeek_u64_at(&self, offset: u64) -> Result<u64> {
        let mut buf = [0u8; 8];
        self.cpeek_exact_at(offset, &mut buf)?;
        Ok(u64::from_le_bytes(buf))
    }
    /// Peeks a [u64] at a specific offset from the reader in big-endian order.
    fn cpeek_u64_be_at(&self, offset: u64) -> Result<u64> {
        let mut buf = [0u8; 8];
        self.cpeek_exact_at(offset, &mut buf)?;
        Ok(u64::from_be_bytes(buf))
    }
    /// Peeks a [u128] at a specific offset from the reader in little-endian order.
    fn cpeek_u128_at(&self, offset: u64) -> Result<u128> {
        let mut buf = [0u8; 16];
        self.cpeek_exact_at(offset, &mut buf)?;
        Ok(u128::from_le_bytes(buf))
    }
    /// Peeks a [u128] at a specific offset from the reader in big-endian order.
    fn cpeek_u128_be_at(&self, offset: u64) -> Result<u128> {
        let mut buf = [0u8; 16];
        self.cpeek_exact_at(offset, &mut buf)?;
        Ok(u128::from_be_bytes(buf))
    }
    /// Peeks an [i8] at a specific offset from the reader.
    fn cpeek_i8_at(&self, offset: u64) -> Result<i8> {
        let mut buf = [0u8; 1];
        self.cpeek_exact_at(offset, &mut buf)?;
        Ok(i8::from_le_bytes(buf))
    }
    /// Peeks an [i16] at a specific offset from the reader in little-endian order.
    fn cpeek_i16_at(&self, offset: u64) -> Result<i16> {
        let mut buf = [0u8; 2];
        self.cpeek_exact_at(offset, &mut buf)?;
        Ok(i16::from_le_bytes(buf))
    }
    /// Peeks an [i16] at a specific offset from the reader in big-endian order.
    fn cpeek_i16_be_at(&self, offset: u64) -> Result<i16> {
        let mut buf = [0u8; 2];
        self.cpeek_exact_at(offset, &mut buf)?;
        Ok(i16::from_be_bytes(buf))
    }
    /// Peeks an [i32] at a specific offset from the reader in little-endian order.
    fn cpeek_i32_at(&self, offset: u64) -> Result<i32> {
        let mut buf = [0u8; 4];
        self.cpeek_exact_at(offset, &mut buf)?;
        Ok(i32::from_le_bytes(buf))
    }
    /// Peeks an [i32] at a specific offset from the reader in big-endian order.
    fn cpeek_i32_be_at(&self, offset: u64) -> Result<i32> {
        let mut buf = [0u8; 4];
        self.cpeek_exact_at(offset, &mut buf)?;
        Ok(i32::from_be_bytes(buf))
    }
    /// Peeks an [i64] at a specific offset from the reader in little-endian order.
    fn cpeek_i64_at(&self, offset: u64) -> Result<i64> {
        let mut buf = [0u8; 8];
        self.cpeek_exact_at(offset, &mut buf)?;
        Ok(i64::from_le_bytes(buf))
    }
    /// Peeks an [i64] at a specific offset from the reader in big-endian order.
    fn cpeek_i64_be_at(&self, offset: u64) -> Result<i64> {
        let mut buf = [0u8; 8];
        self.cpeek_exact_at(offset, &mut buf)?;
        Ok(i64::from_be_bytes(buf))
    }
    /// Peeks an [i128] at a specific offset from the reader in little-endian order.
    fn cpeek_i128_at(&self, offset: u64) -> Result<i128> {
        let mut buf = [0u8; 16];
        self.cpeek_exact_at(offset, &mut buf)?;
        Ok(i128::from_le_bytes(buf))
    }
    /// Peeks an [i128] at a specific offset from the reader in big-endian order.
    fn cpeek_i128_be_at(&self, offset: u64) -> Result<i128> {
        let mut buf = [0u8; 16];
        self.cpeek_exact_at(offset, &mut buf)?;
        Ok(i128::from_be_bytes(buf))
    }

    /// Peeks a C-style string (null-terminated) from the reader.
    fn cpeek_cstring(&self) -> Result<CString>;

    /// Peeks a C-style string (null-terminated) from the reader at a specific offset.
    fn cpeek_cstring_at(&self, offset: u64) -> Result<CString> {
        let mut buf = Vec::new();
        let mut byte = [0u8; 1];
        self.cpeek_at(offset, &mut byte)?;
        while byte[0] != 0 {
            buf.push(byte[0]);
            self.cpeek_at(offset + buf.len() as u64, &mut byte)?;
        }
        CString::new(buf).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    /// Peeks a fixed-length string from the reader.
    fn cpeek_fstring(&self, len: usize, encoding: Encoding, trim: bool) -> Result<String> {
        let mut buf = vec![0u8; len];
        self.cpeek_exact(&mut buf)?;
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
    /// Peeks a fixed-length string from the reader at a specific offset.
    fn cpeek_fstring_at(
        &self,
        offset: u64,
        len: usize,
        encoding: Encoding,
        trim: bool,
    ) -> Result<String> {
        let mut buf = vec![0u8; len];
        self.cpeek_exact_at(offset, &mut buf)?;
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

    /// Peeks a UTF-16 string (null-terminated) from the reader.
    /// Returns the raw bytes of the UTF-16 string. (Null terminator is not included)
    fn cpeek_u16string(&self) -> Result<Vec<u8>>;
    /// Peeks a UTF-16 string (null-terminated) from the reader at a specific offset.
    /// Returns the raw bytes of the UTF-16 string. (Null terminator is not
    fn cpeek_u16string_at(&self, offset: u64) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        let mut bytes = [0u8; 2];
        let mut current_offset = offset;
        loop {
            self.cpeek_exact_at(current_offset, &mut bytes)?;
            if bytes == [0, 0] {
                break;
            }
            buf.extend_from_slice(&bytes);
            current_offset += 2;
        }
        Ok(buf)
    }

    /// Peeks data and checks if it matches the provided data.
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
    /// Peeks data at a specific offset and checks if it matches the provided data.
    fn cpeek_and_equal_at(&self, offset: u64, data: &[u8]) -> Result<()> {
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

    fn cpeek_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize> {
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

    fn cpeek_u16string(&self) -> Result<Vec<u8>> {
        let mut lock = self.lock().map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::Other, "Failed to lock the mutex")
        })?;
        lock.peek_u16string()
    }
}

/// A trait to help to read data from a reader.
pub trait ReadExt {
    /// Reads a [u8] from the reader.
    fn read_u8(&mut self) -> Result<u8>;
    /// Reads a [u16] from the reader in little-endian order.
    fn read_u16(&mut self) -> Result<u16>;
    /// Reads a [u16] from the reader in big-endian order.
    fn read_u16_be(&mut self) -> Result<u16>;
    /// Reads a [u32] from the reader in little-endian order.
    fn read_u32(&mut self) -> Result<u32>;
    /// Reads a [u32] from the reader in big-endian order.
    fn read_u32_be(&mut self) -> Result<u32>;
    /// Reads a [u64] from the reader in little-endian order.
    fn read_u64(&mut self) -> Result<u64>;
    /// Reads a [u64] from the reader in big-endian order.
    fn read_u64_be(&mut self) -> Result<u64>;
    /// Reads a [u128] from the reader in little-endian order.
    fn read_u128(&mut self) -> Result<u128>;
    /// Reads a [u128] from the reader in big-endian order.
    fn read_u128_be(&mut self) -> Result<u128>;
    /// Reads an [i8] from the reader.
    fn read_i8(&mut self) -> Result<i8>;
    /// Reads an [i16] from the reader in little-endian order.
    fn read_i16(&mut self) -> Result<i16>;
    /// Reads an [i16] from the reader in big-endian order.
    fn read_i16_be(&mut self) -> Result<i16>;
    /// Reads an [i32] from the reader in little-endian order.
    fn read_i32(&mut self) -> Result<i32>;
    /// Reads an [i32] from the reader in big-endian order.
    fn read_i32_be(&mut self) -> Result<i32>;
    /// Reads an [i64] from the reader in little-endian order.
    fn read_i64(&mut self) -> Result<i64>;
    /// Reads an [i64] from the reader in big-endian order.
    fn read_i64_be(&mut self) -> Result<i64>;
    /// Reads an [i128] from the reader in little-endian order.
    fn read_i128(&mut self) -> Result<i128>;
    /// Reads an [i128] from the reader in big-endian order.
    fn read_i128_be(&mut self) -> Result<i128>;
    /// Reads a [f32] from the reader in little-endian order.
    fn read_f32(&mut self) -> Result<f32>;
    /// Reads a [f32] from the reader in big-endian order.
    fn read_f32_be(&mut self) -> Result<f32>;
    /// Reads a [f64] from the reader in little-endian order.
    fn read_f64(&mut self) -> Result<f64>;
    /// Reads a [f64] from the reader in big-endian order.
    fn read_f64_be(&mut self) -> Result<f64>;

    /// Reads a C-style string (null-terminated) from the reader.
    fn read_cstring(&mut self) -> Result<CString>;
    /// Reads a C-style string (null-terminated) from the reader with maximum length.
    /// * `len` is the maximum length of the string to read.
    /// * `encoding` specifies the encoding to use for the string.
    /// * `trim` indicates whether to trim the string after the first null byte.
    fn read_fstring(&mut self, len: usize, encoding: Encoding, trim: bool) -> Result<String>;

    /// Reads a UTF-16 string (null-terminated) from the reader.
    /// Returns the raw bytes of the UTF-16 string. (Null terminator is not included)
    fn read_u16string(&mut self) -> Result<Vec<u8>>;

    /// Reads some data from the reader into a vector.
    fn read_exact_vec(&mut self, len: usize) -> Result<Vec<u8>>;

    /// Reads data and checks if it matches the provided data.
    fn read_and_equal(&mut self, data: &[u8]) -> Result<()>;

    /// Reads as much data as possible into the provided buffer.
    /// Returns the number of bytes read.
    fn read_most(&mut self, buf: &mut [u8]) -> Result<usize>;
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
    fn read_f32(&mut self) -> Result<f32> {
        let mut buf = [0u8; 4];
        self.read_exact(&mut buf)?;
        Ok(f32::from_le_bytes(buf))
    }
    fn read_f32_be(&mut self) -> Result<f32> {
        let mut buf = [0u8; 4];
        self.read_exact(&mut buf)?;
        Ok(f32::from_be_bytes(buf))
    }
    fn read_f64(&mut self) -> Result<f64> {
        let mut buf = [0u8; 8];
        self.read_exact(&mut buf)?;
        Ok(f64::from_le_bytes(buf))
    }
    fn read_f64_be(&mut self) -> Result<f64> {
        let mut buf = [0u8; 8];
        self.read_exact(&mut buf)?;
        Ok(f64::from_be_bytes(buf))
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

    fn read_u16string(&mut self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        loop {
            let mut bytes = [0u8; 2];
            self.read_exact(&mut bytes)?;
            if bytes == [0, 0] {
                break;
            }
            buf.extend_from_slice(&bytes);
        }
        Ok(buf)
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

    fn read_most(&mut self, buf: &mut [u8]) -> Result<usize> {
        let mut total_read = 0;
        while total_read < buf.len() {
            match self.read(&mut buf[total_read..]) {
                Ok(0) => break, // EOF reached
                Ok(n) => total_read += n,
                Err(e) => return Err(e),
            }
        }
        Ok(total_read)
    }
}

/// A trait to help to write data to a writer.
pub trait WriteExt {
    /// Writes a [u8] to the writer.
    fn write_u8(&mut self, value: u8) -> Result<()>;
    /// Writes a [u16] to the writer in little-endian order.
    fn write_u16(&mut self, value: u16) -> Result<()>;
    /// Writes a [u16] to the writer in big-endian order.
    fn write_u16_be(&mut self, value: u16) -> Result<()>;
    /// Writes a [u32] to the writer in little-endian order.
    fn write_u32(&mut self, value: u32) -> Result<()>;
    /// Writes a [u32] to the writer in big-endian order.
    fn write_u32_be(&mut self, value: u32) -> Result<()>;
    /// Writes a [u64] to the writer in little-endian order.
    fn write_u64(&mut self, value: u64) -> Result<()>;
    /// Writes a [u64] to the writer in big-endian order.
    fn write_u64_be(&mut self, value: u64) -> Result<()>;
    /// Writes a [u128] to the writer in little-endian order.
    fn write_u128(&mut self, value: u128) -> Result<()>;
    /// Writes a [u128] to the writer in big-endian order.
    fn write_u128_be(&mut self, value: u128) -> Result<()>;
    /// Writes an [i8] to the writer.
    fn write_i8(&mut self, value: i8) -> Result<()>;
    /// Writes an [i16] to the writer in little-endian order.
    fn write_i16(&mut self, value: i16) -> Result<()>;
    /// Writes an [i16] to the writer in big-endian order.
    fn write_i16_be(&mut self, value: i16) -> Result<()>;
    /// Writes an [i32] to the writer in little-endian order.
    fn write_i32(&mut self, value: i32) -> Result<()>;
    /// Writes an [i32] to the writer in big-endian order.
    fn write_i32_be(&mut self, value: i32) -> Result<()>;
    /// Writes an [i64] to the writer in little-endian order.
    fn write_i64(&mut self, value: i64) -> Result<()>;
    /// Writes an [i64] to the writer in big-endian order.
    fn write_i64_be(&mut self, value: i64) -> Result<()>;
    /// Writes an [i128] to the writer in little-endian order.
    fn write_i128(&mut self, value: i128) -> Result<()>;
    /// Writes an [i128] to the writer in big-endian order.
    fn write_i128_be(&mut self, value: i128) -> Result<()>;
    /// Writes a [f32] to the writer in little-endian order.
    fn write_f32(&mut self, value: f32) -> Result<()>;
    /// Writes a [f32] to the writer in big-endian order.
    fn write_f32_be(&mut self, value: f32) -> Result<()>;
    /// Writes a [f64] to the writer in little-endian order.
    fn write_f64(&mut self, value: f64) -> Result<()>;
    /// Writes a [f64] to the writer in big-endian order.
    fn write_f64_be(&mut self, value: f64) -> Result<()>;

    /// Writes a C-style string (null-terminated) to the writer.
    fn write_cstring(&mut self, value: &CString) -> Result<()>;
    /// Writes a C-style string (null-terminated) from the reader with maximum length.
    /// * `data` is the string data to write.
    /// * `len` is the maximum length of the string to write. If the string is longer, it will be truncated if `truncate` is true otherwise an error is returned.
    /// * `encoding` specifies the encoding to use for the string.
    /// * `padding` indicates how to pad the string if it's shorter than `len`.
    /// * `truncate` indicates whether to truncate the string if it's longer than `len`.
    fn write_fstring(
        &mut self,
        data: &str,
        len: usize,
        encoding: Encoding,
        padding: u8,
        truncate: bool,
    ) -> Result<()>;
    /// Write a struct to the writer.
    fn write_struct<V: StructPack>(
        &mut self,
        value: &V,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn std::any::Any>>,
    ) -> Result<()>;
    /// Writes data from a reader to the writer.
    fn write_from<R: Read + Seek>(&mut self, reader: &mut R, offset: u64, len: u64) -> Result<()>;
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
    fn write_f32(&mut self, value: f32) -> Result<()> {
        self.write_all(&value.to_le_bytes())
    }
    fn write_f32_be(&mut self, value: f32) -> Result<()> {
        self.write_all(&value.to_be_bytes())
    }
    fn write_f64(&mut self, value: f64) -> Result<()> {
        self.write_all(&value.to_le_bytes())
    }
    fn write_f64_be(&mut self, value: f64) -> Result<()> {
        self.write_all(&value.to_be_bytes())
    }

    fn write_cstring(&mut self, value: &CString) -> Result<()> {
        self.write_all(value.as_bytes_with_nul())
    }

    fn write_fstring(
        &mut self,
        data: &str,
        len: usize,
        encoding: Encoding,
        padding: u8,
        truncate: bool,
    ) -> Result<()> {
        let encoded = encode_string(encoding, data, true).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Encoding error: {}", e),
            )
        })?;
        let final_data = if encoded.len() > len {
            if truncate {
                &encoded[..len]
            } else {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "String length exceeds the specified length and truncation is disabled",
                ));
            }
        } else {
            &encoded
        };
        self.write_all(final_data)?;
        let padding_len = len.saturating_sub(final_data.len());
        if padding_len > 0 {
            for _ in 0..padding_len {
                self.write_u8(padding)?;
            }
        }
        Ok(())
    }

    fn write_struct<V: StructPack>(
        &mut self,
        value: &V,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn std::any::Any>>,
    ) -> Result<()> {
        value
            .pack(self, big, encoding, info)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }

    fn write_from<R: Read + Seek>(&mut self, reader: &mut R, offset: u64, len: u64) -> Result<()> {
        reader.seek(SeekFrom::Start(offset))?;
        let mut remaining = len;
        let mut buffer = [0u8; 8192];
        while remaining > 0 {
            let to_read = std::cmp::min(remaining, buffer.len() as u64) as usize;
            let bytes_read = reader.read(&mut buffer[..to_read])?;
            if bytes_read == 0 {
                break; // EOF reached
            }
            self.write_all(&buffer[..bytes_read])?;
            remaining -= bytes_read as u64;
        }
        Ok(())
    }
}

/// A trait to help to write data to a writer at a specific offset.
pub trait WriteAt {
    /// Writes data to the writer at a specific offset.
    /// Returns the number of bytes written.
    fn write_at(&mut self, offset: u64, buf: &[u8]) -> Result<usize>;
    /// Writes all data to the writer at a specific offset.
    /// Returns an error if the write fails.
    fn write_all_at(&mut self, offset: u64, buf: &[u8]) -> Result<()>;

    /// Writes a [u8] at a specific offset.
    fn write_u8_at(&mut self, offset: u64, value: u8) -> Result<()> {
        self.write_all_at(offset, &value.to_le_bytes())
    }
    /// Writes a [u16] at a specific offset in little-endian order.
    fn write_u16_at(&mut self, offset: u64, value: u16) -> Result<()> {
        self.write_all_at(offset, &value.to_le_bytes())
    }
    /// Writes a [u16] at a specific offset in big-endian order.
    fn write_u16_be_at(&mut self, offset: u64, value: u16) -> Result<()> {
        self.write_all_at(offset, &value.to_be_bytes())
    }
    /// Writes a [u32] at a specific offset in little-endian order.
    fn write_u32_at(&mut self, offset: u64, value: u32) -> Result<()> {
        self.write_all_at(offset, &value.to_le_bytes())
    }
    /// Writes a [u32] at a specific offset in big-endian order.
    fn write_u32_be_at(&mut self, offset: u64, value: u32) -> Result<()> {
        self.write_all_at(offset, &value.to_be_bytes())
    }
    /// Writes a [u64] at a specific offset in little-endian order.
    fn write_u64_at(&mut self, offset: u64, value: u64) -> Result<()> {
        self.write_all_at(offset, &value.to_le_bytes())
    }
    /// Writes a [u64] at a specific offset in big-endian order.
    fn write_u64_be_at(&mut self, offset: u64, value: u64) -> Result<()> {
        self.write_all_at(offset, &value.to_be_bytes())
    }
    /// Writes a [u128] at a specific offset in little-endian order.
    fn write_u128_at(&mut self, offset: u64, value: u128) -> Result<()> {
        self.write_all_at(offset, &value.to_le_bytes())
    }
    /// Writes a [u128] at a specific offset in big-endian order.
    fn write_u128_be_at(&mut self, offset: u64, value: u128) -> Result<()> {
        self.write_all_at(offset, &value.to_be_bytes())
    }
    /// Writes an [i8] at a specific offset.
    fn write_i8_at(&mut self, offset: u64, value: i8) -> Result<()> {
        self.write_all_at(offset, &value.to_le_bytes())
    }
    /// Writes an [i16] at a specific offset in little-endian order.
    fn write_i16_at(&mut self, offset: u64, value: i16) -> Result<()> {
        self.write_all_at(offset, &value.to_le_bytes())
    }
    /// Writes an [i16] at a specific offset in big-endian order.
    fn write_i16_be_at(&mut self, offset: u64, value: i16) -> Result<()> {
        self.write_all_at(offset, &value.to_be_bytes())
    }
    /// Writes an [i32] at a specific offset in little-endian order.
    fn write_i32_at(&mut self, offset: u64, value: i32) -> Result<()> {
        self.write_all_at(offset, &value.to_le_bytes())
    }
    /// Writes an [i32] at a specific offset in big-endian order.
    fn write_i32_be_at(&mut self, offset: u64, value: i32) -> Result<()> {
        self.write_all_at(offset, &value.to_be_bytes())
    }
    /// Writes an [i64] at a specific offset in little-endian order.
    fn write_i64_at(&mut self, offset: u64, value: i64) -> Result<()> {
        self.write_all_at(offset, &value.to_le_bytes())
    }
    /// Writes an [i64] at a specific offset in big-endian order.
    fn write_i64_be_at(&mut self, offset: u64, value: i64) -> Result<()> {
        self.write_all_at(offset, &value.to_be_bytes())
    }
    /// Writes an [i128] at a specific offset in little-endian order.
    fn write_i128_at(&mut self, offset: u64, value: i128) -> Result<()> {
        self.write_all_at(offset, &value.to_le_bytes())
    }
    /// Writes an [i128] at a specific offset in big-endian order.
    fn write_i128_be_at(&mut self, offset: u64, value: i128) -> Result<()> {
        self.write_all_at(offset, &value.to_be_bytes())
    }

    /// Writes a C-style string (null-terminated) at a specific offset.
    fn write_cstring_at(&mut self, offset: u64, value: &CString) -> Result<()> {
        self.write_all_at(offset, value.as_bytes_with_nul())
    }
}

impl<T: Write + Seek> WriteAt for T {
    fn write_at(&mut self, offset: u64, buf: &[u8]) -> Result<usize> {
        let current_pos = self.stream_position()?;
        self.seek(SeekFrom::Start(offset as u64))?;
        let bytes_written = self.write(buf)?;
        self.seek(SeekFrom::Start(current_pos))?;
        Ok(bytes_written)
    }

    fn write_all_at(&mut self, offset: u64, buf: &[u8]) -> Result<()> {
        let current_pos = self.stream_position()?;
        self.seek(SeekFrom::Start(offset as u64))?;
        self.write_all(buf)?;
        self.seek(SeekFrom::Start(current_pos))?;
        Ok(())
    }
}

/// A trait to help to seek in a stream.
pub trait SeekExt {
    /// Returns the length of the stream.
    fn stream_length(&mut self) -> Result<u64>;
    /// Aligns the current position to the given alignment.
    /// Returns the new position after alignment.
    fn align(&mut self, align: u64) -> Result<u64>;
}

impl<T: Seek> SeekExt for T {
    fn stream_length(&mut self) -> Result<u64> {
        let current_pos = self.stream_position()?;
        let length = self.seek(SeekFrom::End(0))?;
        self.seek(SeekFrom::Start(current_pos))?;
        Ok(length)
    }

    fn align(&mut self, align: u64) -> Result<u64> {
        let current_pos = self.stream_position()?;
        let aligned_pos = (current_pos + align - 1) & !(align - 1);
        if aligned_pos != current_pos {
            self.seek(SeekFrom::Start(aligned_pos))?;
        }
        Ok(aligned_pos)
    }
}

#[derive(Clone)]
/// A memory reader that can read data from a vector of bytes.
pub struct MemReader {
    /// The data to read from.
    pub data: Vec<u8>,
    /// The current position in the data.
    pub pos: usize,
}

/// A memory reader that can read data from a slice of bytes.
#[derive(Clone)]
pub struct MemReaderRef<'a> {
    /// The data to read from.
    pub data: &'a [u8],
    /// The current position in the data.
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
    /// Creates a new `MemReader` with the given data.
    pub fn new(data: Vec<u8>) -> Self {
        MemReader { data, pos: 0 }
    }

    /// Creates a new [MemReaderRef] from the current data and position.
    pub fn to_ref<'a>(&'a self) -> MemReaderRef<'a> {
        MemReaderRef {
            data: &self.data,
            pos: self.pos,
        }
    }

    /// Checks if the reader has reached the end of the data.
    pub fn is_eof(&self) -> bool {
        self.pos >= self.data.len()
    }

    /// Returns the inner data of the reader.
    pub fn inner(self) -> Vec<u8> {
        self.data
    }
}

impl<'a> MemReaderRef<'a> {
    /// Creates a new `MemReaderRef` with the given data.
    pub fn new(data: &'a [u8]) -> Self {
        MemReaderRef { data, pos: 0 }
    }

    /// Checks if the reader has reached the end of the data.
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

    fn cpeek_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        self.to_ref().cpeek_at(offset, buf)
    }

    fn cpeek_cstring(&self) -> Result<CString> {
        self.to_ref().cpeek_cstring()
    }

    fn cpeek_u16string(&self) -> Result<Vec<u8>> {
        self.to_ref().cpeek_u16string()
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

    fn cpeek_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        let len = self.data.len();
        let offset = offset as usize;
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

    fn cpeek_u16string(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        let mut i = self.pos;
        while i + 1 < self.data.len() {
            let bytes = &self.data[i..i + 2];
            if bytes == [0, 0] {
                break;
            }
            buf.extend_from_slice(bytes);
            i += 2;
        }
        Ok(buf)
    }
}

/// A memory writer that can write data to a vector of bytes.
pub struct MemWriter {
    /// The data to write to.
    pub data: Vec<u8>,
    /// The current position in the data.
    pub pos: usize,
}

impl MemWriter {
    /// Creates a new `MemWriter` with an empty data vector.
    pub fn new() -> Self {
        MemWriter {
            data: Vec::new(),
            pos: 0,
        }
    }

    /// Creates a new `MemWriter` with the given capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        MemWriter {
            data: Vec::with_capacity(capacity),
            pos: 0,
        }
    }

    /// Creates a new `MemWriter` with the given data.
    pub fn from_vec(data: Vec<u8>) -> Self {
        MemWriter { data, pos: 0 }
    }

    /// Returns the inner data of the writer.
    pub fn into_inner(self) -> Vec<u8> {
        self.data
    }

    /// Returns a reference to the inner data of the writer.
    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    /// Returns a new `MemReaderRef` that references the current data and position.
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
    /// Seeks to a new position in the writer.
    /// If the new position is beyond the current length of the data, the data is resized when writing.
    /// (This means that seeking beyond the end does not immediately resize the data.)
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

    fn cpeek_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        self.to_ref().cpeek_at(offset, buf)
    }

    fn cpeek_cstring(&self) -> Result<CString> {
        self.to_ref().cpeek_cstring()
    }

    fn cpeek_u16string(&self) -> Result<Vec<u8>> {
        self.to_ref().cpeek_u16string()
    }
}

/// A memory reader that can read data from a slice of bytes.
pub struct MemWriterRef<'a> {
    /// The data to read from.
    pub data: &'a mut [u8],
    /// The current position in the data.
    pub pos: usize,
}

impl<'a> MemWriterRef<'a> {
    /// Creates a new `MemWriterRef` with the given data.
    pub fn new(data: &'a mut [u8]) -> Self {
        MemWriterRef { data, pos: 0 }
    }
}

impl<'a> Read for MemWriterRef<'a> {
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

impl<'a> Seek for MemWriterRef<'a> {
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
                if end_pos as usize > self.data.len() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Seek position is beyond the end of the data",
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

impl Write for MemWriterRef<'_> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        if self.pos >= self.data.len() {
            return Ok(0);
        }
        let bytes_to_write = buf.len().min(self.data.len() - self.pos);
        self.data[self.pos..self.pos + bytes_to_write].copy_from_slice(&buf[..bytes_to_write]);
        self.pos += bytes_to_write;
        Ok(bytes_to_write)
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

/// A region of a stream that can be read/write and seeked within a specified range.
#[derive(Debug)]
pub struct StreamRegion<T: Seek> {
    stream: T,
    start_pos: u64,
    end_pos: u64,
    cur_pos: u64,
}

impl<T: Seek> StreamRegion<T> {
    /// Creates a new `StreamRegion` with the specified stream and position range.
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

    /// Creates a new `StreamRegion` with the specified stream and size.
    ///
    /// The start position is the current position of the stream, and the end position is calculated as `start_pos + size`.
    pub fn with_size(mut stream: T, size: u64) -> Result<Self> {
        let start_pos = stream.stream_position()?;
        let end_pos = start_pos + size;
        Self::new(stream, start_pos, end_pos)
    }

    /// Creates a new `StreamRegion` with the specified stream and start position.
    /// The end position is determined by the length of the stream.
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
        self.stream.seek(SeekFrom::Start(new_pos))?;
        Ok(self.cur_pos)
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

impl<T: Seek + Write> Write for StreamRegion<T> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        if self.cur_pos + self.start_pos >= self.end_pos {
            return Ok(0); // EOF
        }
        self.stream
            .seek(SeekFrom::Start(self.start_pos + self.cur_pos))?;
        let bytes_to_write = (self.end_pos - self.start_pos - self.cur_pos) as usize;
        let m = buf.len().min(bytes_to_write);
        let written = self.stream.write(&buf[..m])?;
        self.cur_pos += written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> Result<()> {
        self.stream.flush()
    }
}

struct RangeMap {
    original: (u64, u64),
    new: (u64, u64),
}

/// A binary patcher that can be used to apply patches to binary data.
pub struct BinaryPatcher<
    R: Read + Seek,
    W: Write + Seek,
    A: Fn(u64) -> Result<u64>,
    O: Fn(u64) -> Result<u64>,
> {
    pub input: R,
    pub output: W,
    input_len: u64,
    address_to_offset: A,
    offset_to_address: O,
    range_maps: Vec<RangeMap>,
}

impl<R: Read + Seek, W: Write + Seek, A: Fn(u64) -> Result<u64>, O: Fn(u64) -> Result<u64>>
    BinaryPatcher<R, W, A, O>
{
    /// Creates a new `BinaryPatcher` with the specified input and output streams.
    pub fn new(
        mut input: R,
        output: W,
        address_to_offset: A,
        offset_to_address: O,
    ) -> Result<Self> {
        let input_len = input.stream_length()?;
        Ok(BinaryPatcher {
            input,
            output,
            input_len,
            address_to_offset,
            offset_to_address,
            range_maps: Vec::new(),
        })
    }

    /// Copies data from the input stream to the output stream up to the specified address of original stream.
    pub fn copy_up_to(&mut self, original_offset: u64) -> Result<()> {
        let cur_pos = self.input.stream_position()?;
        if original_offset < cur_pos || original_offset > self.input_len {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Original offset is out of bounds",
            ));
        }
        let bytes_to_copy = original_offset - cur_pos;
        std::io::copy(&mut (&mut self.input).take(bytes_to_copy), &mut self.output)?;
        Ok(())
    }

    /// Maps an original offset to a new offset in the output stream.
    pub fn map_offset(&mut self, original_offset: u64) -> Result<u64> {
        if original_offset > self.input_len {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Original offset is out of bounds",
            ));
        }
        let cur_pos = self.input.stream_position()?;
        if original_offset > cur_pos {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Original offset is beyond current position",
            ));
        }
        let mut start = 0;
        let mut end = self.range_maps.len();
        while start < end {
            let pivot = (start + end) / 2;
            let range = &self.range_maps[pivot];
            if original_offset < range.original.0 {
                end = pivot;
            } else if original_offset == range.original.0 {
                return Ok(range.new.0);
            } else if original_offset >= range.original.1 {
                start = pivot + 1;
            } else {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Can't map an offset inside a changed section",
                ));
            }
        }
        if start == 0 {
            return Ok(original_offset);
        }
        let index = start - 1;
        let range = &self.range_maps[index];
        let new_offset = original_offset + range.new.1 - range.original.1;
        let out_len = self.output.stream_length()?;
        if new_offset > out_len {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Mapped offset is beyond the end of the output stream",
            ));
        }
        Ok(new_offset)
    }

    /// Replaces bytes in the output stream with new data, starting from the current position in the input stream.
    ///
    /// * `original_length` - The length of the original data to be replaced.
    /// * `new_data` - The new data to write to the output stream.
    pub fn replace_bytes(&mut self, original_length: u64, new_data: &[u8]) -> Result<()> {
        self.replace_bytes_with_write(original_length, |writer| writer.write_all(new_data))
    }

    /// Replaces bytes in the output stream with new data, starting from the current position in the input stream.
    ///
    /// * `original_length` - The length of the original data to be replaced.
    /// * `write` - A function that writes the new data to the output stream.
    pub fn replace_bytes_with_write<F: Fn(&mut W) -> Result<()>>(
        &mut self,
        original_length: u64,
        write: F,
    ) -> Result<()> {
        let cur_pos = self.input.stream_position()?;
        if cur_pos + original_length > self.input_len {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Original length exceeds input length",
            ));
        }
        let new_data_offset = self.output.stream_position()?;
        write(&mut self.output)?;
        let new_data_length = self.output.stream_position()? - new_data_offset;
        if new_data_length != original_length {
            self.range_maps.push(RangeMap {
                original: (cur_pos, cur_pos + original_length),
                new: (new_data_offset, new_data_offset + new_data_length),
            });
        }
        self.input
            .seek(SeekFrom::Start(cur_pos + original_length))?;
        Ok(())
    }

    /// Patches a u32 value in the output stream at the specified original offset.
    pub fn patch_u32(&mut self, original_offset: u64, value: u32) -> Result<()> {
        let input_pos = self.input.stream_position()?;
        if input_pos < original_offset + 4 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Original offset is out of bounds for u32 patching",
            ));
        }
        let new_offset = self.map_offset(original_offset)?;
        self.output.seek(SeekFrom::Start(new_offset))?;
        self.output.write_u32(value)?;
        self.output.seek(SeekFrom::End(0))?;
        Ok(())
    }

    /// Patches a u32 value in big-endian order in the output stream at the specified original offset.
    pub fn patch_u32_be(&mut self, original_offset: u64, value: u32) -> Result<()> {
        let input_pos = self.input.stream_position()?;
        if input_pos < original_offset + 4 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Original offset is out of bounds for u32 patching",
            ));
        }
        let new_offset = self.map_offset(original_offset)?;
        self.output.seek(SeekFrom::Start(new_offset))?;
        self.output.write_u32_be(value)?;
        self.output.seek(SeekFrom::End(0))?;
        Ok(())
    }

    /// Patches a u32 address in the output stream at the specified original offset.
    pub fn patch_u32_address(&mut self, original_offset: u64) -> Result<()> {
        let input_pos = self.input.stream_position()?;
        if input_pos < original_offset + 4 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Original offset is out of bounds for u32 address patching",
            ));
        }
        let original_address = self.input.peek_u32_at(original_offset)?;
        let new_offset = self.map_offset(original_offset)?;
        let offset = (self.address_to_offset)(original_address as u64)?;
        let offset = self.map_offset(offset)?;
        let new_addr = (self.offset_to_address)(offset)?;
        self.output.seek(SeekFrom::Start(new_offset))?;
        self.output.write_u32(new_addr as u32)?;
        self.output.seek(SeekFrom::End(0))?;
        Ok(())
    }
}

/// A thread-safe wrapper around a Mutex-protected writer/reader.
#[derive(Debug)]
pub struct MutexWrapper<T> {
    inner: Arc<Mutex<T>>,
    pos: u64,
}

impl<T> MutexWrapper<T> {
    /// Creates a new `MutexWrapper` with the given inner value.
    pub fn new(inner: Arc<Mutex<T>>, pos: u64) -> Self {
        MutexWrapper { inner, pos }
    }
}

impl<T: Read + Seek> Read for MutexWrapper<T> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let mut lock = self.inner.lock().map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::Other, "Failed to lock the mutex")
        })?;
        lock.seek(SeekFrom::Start(self.pos))?;
        let readed = lock.read(buf)?;
        self.pos += readed as u64;
        Ok(readed)
    }
}

impl<T: Read + Seek> Seek for MutexWrapper<T> {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        let mut lock = self.inner.lock().map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::Other, "Failed to lock the mutex")
        })?;
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset,
            SeekFrom::End(offset) => {
                let len = lock.stream_length()?;
                (len as i64 + offset as i64) as u64
            }
            SeekFrom::Current(offset) => (self.pos as i64 + offset as i64) as u64,
        };
        if new_pos > lock.stream_length()? {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Seek position is beyond the end of the stream",
            ));
        }
        self.pos = new_pos;
        Ok(self.pos)
    }

    fn stream_position(&mut self) -> Result<u64> {
        Ok(self.pos)
    }

    fn rewind(&mut self) -> Result<()> {
        self.pos = 0;
        Ok(())
    }
}

/// A writer that does nothing and always succeeds.
pub struct EmptyWriter;

impl EmptyWriter {
    /// Creates a new `EmptyWriter`.
    pub fn new() -> Self {
        Self {}
    }
}

impl Write for EmptyWriter {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
/// A readable stream that starts with a given prefix before the actual data.
pub struct PrefixStream<T> {
    prefix: Vec<u8>,
    pos: usize,
    inner: T,
}

impl<T> PrefixStream<T> {
    /// Creates a new `PrefixStream` with the given prefix and inner stream.
    pub fn new(prefix: Vec<u8>, inner: T) -> Self {
        PrefixStream {
            prefix,
            pos: 0,
            inner,
        }
    }
}

impl<T: Read> Read for PrefixStream<T> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        if self.pos < self.prefix.len() {
            let bytes_to_read = std::cmp::min(buf.len(), self.prefix.len() - self.pos);
            buf[..bytes_to_read].copy_from_slice(&self.prefix[self.pos..self.pos + bytes_to_read]);
            self.pos += bytes_to_read;
            Ok(bytes_to_read)
        } else {
            self.inner.read(buf)
        }
    }
}

impl<T: Seek> Seek for PrefixStream<T> {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        let prefix_len = self.prefix.len() as u64;
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset,
            SeekFrom::End(offset) => {
                let inner_len = self.inner.stream_length()?;
                if offset < 0 {
                    if (-offset) as u64 > inner_len + prefix_len {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "Seek position is before the start of the stream",
                        ));
                    }
                    inner_len + prefix_len - (-offset) as u64
                } else {
                    inner_len + prefix_len + offset as u64
                }
            }
            SeekFrom::Current(offset) => {
                let current_pos = self.stream_position()?;
                if offset < 0 {
                    if (-offset) as u64 > current_pos + prefix_len {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "Seek position is before the start of the stream",
                        ));
                    }
                    prefix_len + current_pos - (-offset) as u64
                } else {
                    prefix_len + current_pos + offset as u64
                }
            }
        };
        if new_pos < prefix_len {
            self.pos = new_pos as usize;
            self.inner.rewind()?;
        } else {
            self.pos = self.prefix.len();
            self.inner.seek(SeekFrom::Start(new_pos - prefix_len))?;
        }
        Ok(new_pos)
    }

    fn stream_position(&mut self) -> Result<u64> {
        Ok(self.pos as u64 + self.inner.stream_position()?)
    }

    fn rewind(&mut self) -> Result<()> {
        self.pos = 0;
        self.inner.rewind()?;
        Ok(())
    }
}

/// A readable stream that concatenates multiple streams.
#[derive(Debug)]
pub struct MultipleReadStream {
    streams: Vec<Box<dyn ReadSeek>>,
    stream_lengths: Vec<u64>,
    total_length: u64,
    pos: u64,
}

impl MultipleReadStream {
    /// Creates a new `MultipleReadStream`.
    pub fn new() -> Self {
        MultipleReadStream {
            streams: Vec::new(),
            stream_lengths: Vec::new(),
            total_length: 0,
            pos: 0,
        }
    }

    /// Adds a new stream to the end of the concatenated streams.
    pub fn add_stream<T: ReadSeek + 'static>(&mut self, mut stream: T) -> Result<()> {
        let length = stream.stream_length()?;
        self.streams.push(Box::new(stream));
        self.stream_lengths.push(self.total_length);
        self.total_length += length;
        Ok(())
    }

    /// Adds a new boxed stream to the end of the concatenated streams.
    pub fn add_stream_boxed(&mut self, mut stream: Box<dyn ReadSeek>) -> Result<()> {
        let length = stream.stream_length()?;
        self.streams.push(stream);
        self.stream_lengths.push(self.total_length);
        self.total_length += length;
        Ok(())
    }
}

impl Read for MultipleReadStream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        if self.pos >= self.total_length {
            return Ok(0);
        }
        let (stream_index, stream_offset) = match self.stream_lengths.binary_search_by(|&len| {
            if len > self.pos {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Less
            }
        }) {
            Ok(index) => (index, 0),
            Err(0) => (0, self.pos),
            Err(index) => (index - 1, self.pos - self.stream_lengths[index - 1]),
        };
        let stream = &mut self.streams[stream_index];
        stream.seek(SeekFrom::Start(stream_offset))?;
        let bytes_read = stream.read(buf)?;
        self.pos += bytes_read as u64;
        Ok(bytes_read)
    }
}

impl Seek for MultipleReadStream {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset,
            SeekFrom::End(offset) => {
                if offset < 0 {
                    if (-offset) as u64 > self.total_length {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "Seek position is before the start of the stream",
                        ));
                    }
                    self.total_length - (-offset) as u64
                } else {
                    self.total_length + offset as u64
                }
            }
            SeekFrom::Current(offset) => {
                if offset < 0 {
                    if (-offset) as u64 > self.pos {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "Seek position is before the start of the stream",
                        ));
                    }
                    self.pos - (-offset) as u64
                } else {
                    self.pos + offset as u64
                }
            }
        };
        if new_pos > self.total_length {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Seek position is beyond the end of the stream",
            ));
        }
        self.pos = new_pos;
        Ok(self.pos)
    }

    fn stream_position(&mut self) -> Result<u64> {
        Ok(self.pos)
    }

    fn rewind(&mut self) -> Result<()> {
        self.pos = 0;
        Ok(())
    }
}

pub struct TrackStream<'a, T> {
    inner: T,
    total: &'a mut u64,
}

impl<'a, T> TrackStream<'a, T> {
    pub fn new(inner: T, total: &'a mut u64) -> Self {
        TrackStream { inner, total }
    }
}

impl<'a, T: Read> Read for TrackStream<'a, T> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let readed = self.inner.read(buf)?;
        *self.total += readed as u64;
        Ok(readed)
    }
}

impl<'a, T: Write> Write for TrackStream<'a, T> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let written = self.inner.write(buf)?;
        *self.total += written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> Result<()> {
        self.inner.flush()
    }
}

/// A reader that forwards reads to an inner reader, but ensures that all reads are aligned to a specified alignment boundary.
#[derive(Debug)]
pub struct AlignedReader<const A: usize, T: Read> {
    inner: T,
    buffer: [u8; A],
    buffer_size: usize,
}

impl<const A: usize, T: Read> AlignedReader<A, T> {
    /// Creates a new `AlignedReader` with the given inner reader.
    pub fn new(inner: T) -> Self {
        AlignedReader {
            inner,
            buffer: [0; A],
            buffer_size: 0,
        }
    }
}

impl<const A: usize, T: Read> Read for AlignedReader<A, T> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let mut total_read = 0;
        let mut needed = buf.len();
        if needed <= self.buffer_size {
            buf[..needed].copy_from_slice(&self.buffer[..needed]);
            self.buffer.copy_within(needed..self.buffer_size, 0);
            self.buffer_size -= needed;
            return Ok(needed);
        }
        if self.buffer_size > 0 {
            buf[..self.buffer_size].copy_from_slice(&self.buffer[..self.buffer_size]);
            total_read += self.buffer_size;
            needed -= self.buffer_size;
            self.buffer_size = 0;
        }
        let read_len = needed / A * A;
        if read_len > 0 {
            let readed = self
                .inner
                .read(&mut buf[total_read..total_read + read_len])?;
            total_read += readed;
            needed -= readed;
            // EOF reached
            if readed < read_len {
                return Ok(total_read);
            }
        }
        if needed > 0 {
            let readed = self.inner.read(&mut self.buffer)?;
            let to_copy = needed.min(readed);
            buf[total_read..total_read + to_copy].copy_from_slice(&self.buffer[..to_copy]);
            total_read += to_copy;
            self.buffer_size = readed - to_copy;
            self.buffer.copy_within(to_copy..readed, 0);
        }
        Ok(total_read)
    }
}

/// A writer that forwards reads to an inner writer, but ensures that all writes are aligned to a specified alignment boundary.
#[derive(Debug)]
pub struct AlignedWriter<const A: usize, T: Write> {
    inner: T,
    buffer: [u8; A],
    buffer_size: usize,
}

impl<const A: usize, T: Write> AlignedWriter<A, T> {
    /// Creates a new `AlignedWriter` with the given inner writer.
    pub fn new(inner: T) -> Self {
        AlignedWriter {
            inner,
            buffer: [0; A],
            buffer_size: 0,
        }
    }
}

impl<const A: usize, T: Write> Write for AlignedWriter<A, T> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let mut total_writted = 0;
        let mut needed = buf.len();
        if self.buffer_size > 0 {
            let to_copy = (A - self.buffer_size).min(needed);
            self.buffer[self.buffer_size..self.buffer_size + to_copy]
                .copy_from_slice(&buf[..to_copy]);
            self.buffer_size += to_copy;
            total_writted += to_copy;
            needed -= to_copy;
            if self.buffer_size == A {
                let writed = self.inner.write(&self.buffer)?;
                if writed % A != 0 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::WriteZero,
                        "Failed to write all aligned bytes",
                    ));
                }
                self.buffer_size -= writed;
            } else {
                return Ok(total_writted);
            }
        }
        let mut write_len = needed / A * A;
        while write_len > 0 {
            let writed = self
                .inner
                .write(&buf[total_writted..total_writted + write_len])?;
            if writed % A != 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::WriteZero,
                    "Failed to write all aligned bytes",
                ));
            }
            total_writted += writed;
            needed -= writed;
            write_len = needed / A * A;
        }
        if needed > 0 {
            self.buffer[..needed].copy_from_slice(&buf[total_writted..total_writted + needed]);
            self.buffer_size = needed;
            total_writted += needed;
        }
        Ok(total_writted)
    }

    fn flush(&mut self) -> Result<()> {
        self.inner.flush()
    }
}

impl<const A: usize, T: Write> Drop for AlignedWriter<A, T> {
    fn drop(&mut self) {
        if self.buffer_size > 0 {
            if let Err(err) = self.inner.write_all(&self.buffer[..self.buffer_size]) {
                eprintln!("Failed to flush AlignedWriter buffer: {}", err);
                crate::COUNTER.inc_error();
            }
            self.buffer_size = 0;
        }
    }
}
