use super::types::*;
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use crate::utils::mmx::*;
use anyhow::Result;
use std::io::{Read, Seek, SeekFrom, Write};

pub trait Hasher {
    fn update(&mut self, data: &[u8]) -> Result<()>;
    fn finalize(&mut self) -> Result<u32>;
}

pub trait Encryption: std::fmt::Debug {
    fn is_unicode(&self) -> bool {
        false
    }
    fn compute_hash(&self, _data: &[u8]) -> Result<u32> {
        Ok(0)
    }
    fn create_hash(&self) -> Result<Box<dyn Hasher>> {
        Err(anyhow::anyhow!("Hasher not implemented"))
    }
    fn decrypt_name(&self, name: &mut [u8], hash: i32, encoding: Encoding) -> Result<String>;
    fn decrypt_entry<'a>(
        &self,
        stream: Box<dyn ReadSeek + 'a>,
        entry: &QlieEntry,
    ) -> Result<Box<dyn ReadDebug + 'a>>;
}

pub fn create_encryption(major: u8, minor: u8) -> Result<Box<dyn Encryption>> {
    match (major, minor) {
        (3, 1) => Ok(Box::new(Encryption31::new())),
        _ => Err(anyhow::anyhow!(
            "Unsupported encryption version: {}.{}",
            major,
            minor
        )),
    }
}

pub fn decompress<'a>(data: Box<dyn ReadDebug + 'a>) -> Result<Box<dyn ReadDebug + 'a>> {
    Ok(Box::new(Decompressor::new(data)?))
}

pub fn decrypt(data: &mut [u8], key: u32) -> Result<()> {
    let length = data.len();
    if length < 8 {
        // Nothing to decrypt
        return Ok(());
    }
    let mut data = MemWriterRef::new(data);
    const C1: u64 = 0xA73C5F9D;
    const C2: u64 = 0xCE24F523;
    const C3: u64 = 0xFEC9753E;
    let mut v5 = mmx_punpckldq2(C1);
    const V7: u64 = mmx_punpckldq2(C2);
    let mut v9 = mmx_punpckldq2(((length as u32).wrapping_add(key) as u64) ^ C3);
    for _ in 0..length / 8 {
        let d = data.peek_u64()?;
        v5 = mmx_p_add_d(v5, V7) ^ v9;
        v9 = d ^ v5;
        data.write_u64(v9)?;
    }
    Ok(())
}

pub fn encrypt(data: &mut [u8], key: u32) -> Result<()> {
    let length = data.len();
    if length < 8 {
        // Nothing to encrypt
        return Ok(());
    }
    let mut data = MemWriterRef::new(data);
    const C1: u64 = 0xA73C5F9D;
    const C2: u64 = 0xCE24F523;
    const C3: u64 = 0xFEC9753E;
    let mut v5 = mmx_punpckldq2(C1);
    const V7: u64 = mmx_punpckldq2(C2);
    let mut v9 = mmx_punpckldq2(((length as u32).wrapping_add(key) as u64) ^ C3);
    for _ in 0..length / 8 {
        let mut d = data.peek_u64()?;
        v5 = mmx_p_add_d(v5, V7) ^ v9;
        v9 = d;
        d ^= v5;
        data.write_u64(d)?;
    }
    Ok(())
}

pub fn get_common_key(data: &[u8]) -> Result<Vec<u8>> {
    let mut reader = MemReaderRef::new(data);
    let mut key = vec![0u8; 0x400];
    let mut writer = MemWriterRef::new(&mut key);
    for i in 0..0x100i32 {
        let temp = if (i % 3) != 0 {
            (i + 7) * -(i + 3)
        } else {
            (i + 7) * (i + 3)
        };
        writer.write_u32_at(i as u64 * 4, temp as u32)?;
    }
    let mut v1 = (reader.peek_u8_at(49)? % 0x49) as i32 + 0x80;
    let v2 = (reader.peek_u8_at(79)? % 7) as i32 + 7;
    let data_len = data.len() as i32;
    for i in 0..0x400 {
        v1 = (v1.wrapping_add(v2)) % data_len;
        key[i] ^= reader.peek_u8_at(v1 as u64)?;
    }
    // crate::utils::files::write_file("./testscripts/test.bin")?.write_all(&key)?;
    Ok(key)
}

#[derive(Debug)]
pub struct Encryption31 {}

impl Encryption31 {
    pub fn new() -> Self {
        Self {}
    }

    fn create_table(length: usize, mut value: u32, is_v1: bool) -> Result<Vec<u8>> {
        let mut table = Vec::with_capacity(length);
        let key: u32 = if is_v1 { 0x8DF21431 } else { 0x8A77F473 };
        for _ in 0..length {
            let t = (key as u64).wrapping_mul((value as u64) ^ (key as u64));
            value = ((t >> 32) + t) as u32;
            table.push(value);
        }
        let mut mem = MemWriter::with_capacity(length * 4);
        for i in table {
            mem.write_u32(i)?;
        }
        Ok(mem.into_inner())
    }

    pub fn compute_name_hash(&self, name: &[u16]) -> Result<u32> {
        let mut v2 = 0u32;
        let mut v3 = name.len() as u32;
        let mut v4 = 1u32;
        if v3 > 0 {
            loop {
                let n = (name[(v4 - 1) as usize] as u32) << (v4 & 7);
                v2 = v2.wrapping_add(n) & 0x3FFFFFFF;
                v4 += 1;
                v3 -= 1;
                if v3 == 0 {
                    break;
                }
            }
        }
        Ok(v2)
    }

    pub fn encrypt_name(&self, name: &mut [u8], hash: i32) -> Result<()> {
        if name.len() % 2 != 0 {
            return Err(anyhow::anyhow!(
                "Invalid name length for Unicode encryption"
            ));
        }
        let char_len = name.len() / 2;
        let cl = char_len as i32;
        let temp = (cl.wrapping_mul(cl) ^ cl ^ 0x3e13 ^ (hash >> 16) ^ hash) & 0xFFFF;
        let mut key = temp;
        for i in 0..char_len {
            key = temp
                .wrapping_add(i as i32)
                .wrapping_add(key.wrapping_mul(8));
            name[i * 2] ^= key as u8;
            name[i * 2 + 1] ^= (key >> 8) as u8;
        }
        Ok(())
    }
}

impl Encryption for Encryption31 {
    fn is_unicode(&self) -> bool {
        true
    }
    fn compute_hash(&self, data: &[u8]) -> Result<u32> {
        let mut hasher = Encryption31Hasher::new();
        hasher.update(data)?;
        Ok(hasher.finalize()?)
    }
    fn create_hash(&self) -> Result<Box<dyn Hasher>> {
        Ok(Box::new(Encryption31Hasher::new()))
    }
    fn decrypt_name(&self, name: &mut [u8], hash: i32, _encoding: Encoding) -> Result<String> {
        if name.len() % 2 != 0 {
            return Err(anyhow::anyhow!(
                "Invalid name length for Unicode decryption"
            ));
        }
        let char_len = name.len() / 2;
        let cl = char_len as i32;
        let temp = (cl.wrapping_mul(cl) ^ cl ^ 0x3e13 ^ (hash >> 16) ^ hash) & 0xFFFF;
        let mut key = temp;
        for i in 0..char_len {
            key = temp
                .wrapping_add(i as i32)
                .wrapping_add(key.wrapping_mul(8));
            name[i * 2] ^= key as u8;
            name[i * 2 + 1] ^= (key >> 8) as u8;
        }
        Ok(decode_to_string(Encoding::Utf16LE, &name, true)?)
    }
    fn decrypt_entry<'a>(
        &self,
        stream: Box<dyn ReadSeek + 'a>,
        entry: &QlieEntry,
    ) -> Result<Box<dyn ReadDebug + 'a>> {
        match entry.is_encrypted {
            // No encryption
            0 => Ok(Box::new(stream)),
            1 => Ok(Box::new(Encryption31DecryptV1::new(
                stream,
                entry.size,
                entry.name.clone(),
                entry.key,
            )?)),
            2 => Ok(Box::new(Encryption31DecryptV2::new(
                stream,
                entry.size,
                entry.name.clone(),
                entry.key,
                entry
                    .common_key
                    .clone()
                    .ok_or(anyhow::anyhow!("Missing common key"))?,
            )?)),
            _ => Err(anyhow::anyhow!(
                "Unsupported encryption flag: {}",
                entry.is_encrypted
            )),
        }
    }
}

pub struct Encryption31Hasher {
    hash: u64,
    key: u64,
    buffer: [u8; 8],
    buffer_len: usize,
}

impl Encryption31Hasher {
    pub fn new() -> Self {
        Self {
            hash: 0,
            key: 0,
            buffer: [0; 8],
            buffer_len: 0,
        }
    }

    fn update_internal(&mut self, data: u64) {
        const C: u64 = mmx_punpckldq2(0xA35793A7);
        self.hash = mmx_p_add_w(self.hash, C);
        let temp = mmx_p_add_w(self.key, self.hash ^ data);
        self.key = mmx_p_sll_d(temp, 3) | mmx_p_srl_d(temp, 0x1d);
    }
}

impl Hasher for Encryption31Hasher {
    fn update(&mut self, data: &[u8]) -> Result<()> {
        let mut used = 0;
        if self.buffer_len > 0 {
            let to_copy = (8 - self.buffer_len).min(data.len());
            self.buffer[self.buffer_len..self.buffer_len + to_copy]
                .copy_from_slice(&data[..to_copy]);
            self.buffer_len += to_copy;
            used += to_copy;
        }
        if self.buffer_len == 8 {
            let v = u64::from_le_bytes(self.buffer);
            self.update_internal(v);
            self.buffer_len = 0;
        }
        let round = (data.len() - used) / 8;
        let mut reader = MemReaderRef::new(&data[used..]);
        for _ in 0..round {
            let v = reader.read_u64()?;
            self.update_internal(v);
            used += 8;
        }
        let remaining = data.len() - used;
        if remaining > 0 {
            self.buffer[..remaining].copy_from_slice(&data[used..]);
            self.buffer_len = remaining;
        }
        Ok(())
    }

    fn finalize(&mut self) -> Result<u32> {
        let p1 = ((self.key as i16) as i32).wrapping_mul(((self.key >> 32) as i16) as i32);
        let p2 = (((self.key >> 16) as i16) as i32).wrapping_mul(((self.key >> 48) as i16) as i32);
        Ok((p1.wrapping_add(p2)) as u32)
    }
}

#[derive(Debug)]
struct Encryption31DecryptV1<'a> {
    stream: Box<dyn ReadSeek + 'a>,
    table: MemReader,
    v4: u32,
    v6: u64,
}

impl<'a> Encryption31DecryptV1<'a> {
    pub fn new(
        stream: Box<dyn ReadSeek + 'a>,
        size: u32,
        name: String,
        key: u32,
    ) -> Result<AlignedReader<8, Self>> {
        let mut v1 = 0x85F532u32;
        let mut v2 = 0x33F641u32;
        for (i, n) in name.encode_utf16().enumerate() {
            v1 = v1.wrapping_add((n as u32) << (i & 7));
            v2 ^= v1;
        }
        v2 = v2.wrapping_add(
            key ^ ((7 * (size & 0xFFFFFF))
                .wrapping_add(size)
                .wrapping_add(v1)
                .wrapping_add(v1 ^ size ^ 0x8F32DC)),
        );
        v2 = 9 * (v2 & 0xFFFFFF);
        let table = MemReader::new(Encryption31::create_table(0x40, v2, true)?);
        let v4 = 8 * (table.cpeek_u32_at(52)? & 0xF);
        let v6 = table.cpeek_u64_at(24)?;
        let inner = Self {
            stream,
            table,
            v4,
            v6,
        };
        Ok(AlignedReader::new(inner))
    }
}

impl<'a> Read for Encryption31DecryptV1<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.stream.read_most(buf)?;
        let round = readed / 8;
        let mut writer = MemWriterRef::new(buf);
        for _ in 0..round {
            let d = writer.peek_u64()?;
            let temp = self.table.cpeek_u64_at(self.v4 as u64)?;
            let v7 = mmx_p_add_d(self.v6 ^ temp, temp);
            let v8 = d ^ v7;
            writer.write_u64(v8)?;
            self.v6 = mmx_p_add_w(mmx_p_sll_d(mmx_p_add_b(v7, v8) ^ v8, 1), v8);
            self.v4 = (self.v4 + 8) & 0x7F;
        }
        Ok(readed)
    }
}

#[derive(Debug)]
pub struct Encryption31EncryptV1<T: Write> {
    stream: T,
    table: MemReader,
    v4: u32,
    v6: u64,
}

impl<T: Write> Encryption31EncryptV1<T> {
    pub fn new(stream: T, size: u32, name: String, key: u32) -> Result<AlignedWriter<8, Self>> {
        let mut v1 = 0x85F532u32;
        let mut v2 = 0x33F641u32;
        for (i, n) in name.encode_utf16().enumerate() {
            v1 = v1.wrapping_add((n as u32) << (i & 7));
            v2 ^= v1;
        }
        v2 = v2.wrapping_add(
            key ^ ((7 * (size & 0xFFFFFF))
                .wrapping_add(size)
                .wrapping_add(v1)
                .wrapping_add(v1 ^ size ^ 0x8F32DC)),
        );
        v2 = 9 * (v2 & 0xFFFFFF);
        let table = MemReader::new(Encryption31::create_table(0x40, v2, true)?);
        let v4 = 8 * (table.cpeek_u32_at(52)? & 0xF);
        let v6 = table.cpeek_u64_at(24)?;
        let inner = Self {
            stream,
            table,
            v4,
            v6,
        };
        Ok(AlignedWriter::new(inner))
    }
}

impl<T: Write> Write for Encryption31EncryptV1<T> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let round = buf.len() / 8;
        let mut reader = MemReaderRef::new(buf);
        for _ in 0..round {
            let d = reader.read_u64()?;
            let temp = self.table.cpeek_u64_at(self.v4 as u64)?;
            let v7 = mmx_p_add_d(self.v6 ^ temp, temp);
            let v8 = d ^ v7;
            self.stream.write_u64(v8)?;
            self.v6 = mmx_p_add_w(mmx_p_sll_d(mmx_p_add_b(v7, d) ^ d, 1), d);
            self.v4 = (self.v4 + 8) & 0x7F;
        }
        let remain = buf.len() % 8;
        if remain > 0 {
            self.stream.write_all(&buf[buf.len() - remain..])?;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.stream.flush()
    }
}

#[derive(Debug)]
struct Encryption31DecryptV2<'a> {
    stream: Box<dyn ReadSeek + 'a>,
    table: MemReader,
    v4: u32,
    v6: u64,
    common_key: MemReader,
}

impl<'a> Encryption31DecryptV2<'a> {
    pub fn new(
        stream: Box<dyn ReadSeek + 'a>,
        size: u32,
        name: String,
        key: u32,
        common_key: Vec<u8>,
    ) -> Result<AlignedReader<8, Self>> {
        let mut v1 = 0x86F7E2u32;
        let mut v2 = 0x4437F1u32;
        for (i, n) in name.encode_utf16().enumerate() {
            v1 = v1.wrapping_add((n as u32) << (i & 7));
            v2 ^= v1;
        }
        v2 = v2.wrapping_add(
            key ^ ((13 * (size & 0xFFFFFF))
                .wrapping_add(size)
                .wrapping_add(v1)
                .wrapping_add(v1 ^ size ^ 0x56E213)),
        );
        v2 = 13 * (v2 & 0xFFFFFF);
        let table = MemReader::new(Encryption31::create_table(0x40, v2, false)?);
        let v4 = 8 * (table.cpeek_u32_at(32)? & 0xD);
        let v6 = table.cpeek_u64_at(24)?;
        let inner = Self {
            stream,
            table,
            v4,
            v6,
            common_key: MemReader::new(common_key),
        };
        Ok(AlignedReader::new(inner))
    }
}

impl<'a> Read for Encryption31DecryptV2<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.stream.read_most(buf)?;
        let round = readed / 8;
        let mut writer = MemWriterRef::new(buf);
        for _ in 0..round {
            let d = writer.peek_u64()?;
            let temp_index1 = ((self.v4 & 0xF) * 8) as u64;
            let temp_index2 = ((self.v4 & 0x7F) * 8) as u64;
            let temp = self.table.cpeek_u64_at(temp_index1)?
                ^ self.common_key.cpeek_u64_at(temp_index2)?;
            let v7 = mmx_p_add_d(self.v6 ^ temp, temp);
            let v8 = d ^ v7;
            writer.write_u64(v8)?;
            self.v6 = mmx_p_add_w(mmx_p_sll_d(mmx_p_add_b(v7, v8) ^ v8, 1), v8);
            self.v4 = (self.v4 + 1) & 0x7F;
        }
        Ok(readed)
    }
}

#[derive(Debug)]
pub struct Encryption31EncryptV2<T: Write> {
    stream: T,
    table: MemReader,
    v4: u32,
    v6: u64,
    common_key: MemReader,
}

impl<T: Write> Encryption31EncryptV2<T> {
    pub fn new(
        stream: T,
        size: u32,
        name: String,
        key: u32,
        common_key: Vec<u8>,
    ) -> Result<AlignedWriter<8, Self>> {
        let mut v1 = 0x86F7E2u32;
        let mut v2 = 0x4437F1u32;
        for (i, n) in name.encode_utf16().enumerate() {
            v1 = v1.wrapping_add((n as u32) << (i & 7));
            v2 ^= v1;
        }
        v2 = v2.wrapping_add(
            key ^ ((13 * (size & 0xFFFFFF))
                .wrapping_add(size)
                .wrapping_add(v1)
                .wrapping_add(v1 ^ size ^ 0x56E213)),
        );
        v2 = 13 * (v2 & 0xFFFFFF);
        let table = MemReader::new(Encryption31::create_table(0x40, v2, false)?);
        let v4 = 8 * (table.cpeek_u32_at(32)? & 0xD);
        let v6 = table.cpeek_u64_at(24)?;
        let inner = Self {
            stream,
            table,
            v4,
            v6,
            common_key: MemReader::new(common_key),
        };
        Ok(AlignedWriter::new(inner))
    }
}

impl<T: Write> Write for Encryption31EncryptV2<T> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let round = buf.len() / 8;
        let mut reader = MemReaderRef::new(buf);
        for _ in 0..round {
            let d = reader.read_u64()?;
            let temp_index1 = ((self.v4 & 0xF) * 8) as u64;
            let temp_index2 = ((self.v4 & 0x7F) * 8) as u64;
            let temp = self.table.cpeek_u64_at(temp_index1)?
                ^ self.common_key.cpeek_u64_at(temp_index2)?;
            let v7 = mmx_p_add_d(self.v6 ^ temp, temp);
            let v8 = d ^ v7;
            self.stream.write_u64(v8)?;
            self.v6 = mmx_p_add_w(mmx_p_sll_d(mmx_p_add_b(v7, d) ^ d, 1), d);
            self.v4 = (self.v4 + 1) & 0x7F;
        }
        let remain = buf.len() % 8;
        if remain > 0 {
            self.stream.write_all(&buf[buf.len() - remain..])?;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.stream.flush()
    }
}

#[derive(Debug)]
pub struct Decompressor<'a> {
    stream: Box<dyn ReadDebug + 'a>,
    is_16bit: bool,
    temp: Vec<u8>,
    buf: Vec<u8>,
    buf_pos: usize,
}

impl<'a> Decompressor<'a> {
    pub fn new(mut stream: Box<dyn ReadDebug + 'a>) -> Result<Self> {
        let sign = stream.read_u32()?;
        if sign != 0xFF435031 {
            return Err(anyhow::anyhow!("Invalid compression signature"));
        }
        let is_16bit = stream.read_u32()? & 1 != 0;
        let _unpacked_size = stream.read_u32()?;
        let temp = vec![0u8; 0x1000];
        Ok(Self {
            stream,
            is_16bit,
            temp,
            buf: Vec::new(),
            buf_pos: 0,
        })
    }

    fn next_block(&mut self) -> Result<()> {
        self.buf.clear();
        self.buf_pos = 0;
        let mut buf = [0u8; 1];
        let readed = self.stream.read(&mut buf)?;
        if readed == 0 {
            return Ok(());
        }
        let mut buf_used = false;
        let mut table = [[0u8; 2]; 0x100];
        let mut i = 0u32;
        while i < 0x100 {
            let mut c = if !buf_used {
                buf_used = true;
                buf[0] as u32
            } else {
                self.stream.read_u8()? as u32
            };
            if c > 127 {
                c -= 127;
                while c > 0 {
                    table[i as usize][0] = i as u8;
                    c -= 1;
                    i += 1;
                }
            }
            c += 1;
            while c > 0 && i < 0x100 {
                table[i as usize][0] = self.stream.read_u8()?;
                if i as u8 != table[i as usize][0] {
                    table[i as usize][1] = self.stream.read_u8()?;
                }
                c -= 1;
                i += 1;
            }
        }
        let mut block_size = if self.is_16bit {
            self.stream.read_u16()? as usize
        } else {
            self.stream.read_u32()? as usize
        };
        let mut temp_length = 0usize;
        while block_size > 0 || temp_length > 0 {
            let c = if temp_length > 0 {
                temp_length -= 1;
                self.temp[temp_length]
            } else {
                block_size -= 1;
                self.stream.read_u8()?
            };
            if c == table[c as usize][0] {
                self.buf.push(c);
            } else {
                self.temp[temp_length] = table[c as usize][1];
                temp_length += 1;
                self.temp[temp_length] = table[c as usize][0];
                temp_length += 1;
            }
        }
        Ok(())
    }
}

impl<'a> Read for Decompressor<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut used = 0;
        while used < buf.len() {
            if self.buf_pos >= self.buf.len() {
                self.next_block()
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
                if self.buf.is_empty() {
                    break;
                }
            }
            let to_copy = (self.buf.len() - self.buf_pos).min(buf.len() - used);
            buf[used..used + to_copy]
                .copy_from_slice(&self.buf[self.buf_pos..self.buf_pos + to_copy]);
            self.buf_pos += to_copy;
            used += to_copy;
        }
        Ok(used)
    }
}

pub struct Compressor<W: Write + Seek> {
    stream: W,
    buffer: Vec<u8>,
    total_unpacked_size: u32,
    is_finished: bool,
}

impl<W: Write + Seek> Compressor<W> {
    pub fn new(mut stream: W) -> Result<Self> {
        stream.write_u32(0xFF435031)?;
        stream.write_u32(0)?;
        stream.write_u32(0)?;
        Ok(Self {
            stream,
            buffer: Vec::new(),
            total_unpacked_size: 0,
            is_finished: false,
        })
    }

    pub fn finish(&mut self) -> Result<()> {
        if self.is_finished {
            return Ok(());
        }
        if !self.buffer.is_empty() {
            self.flush_block()?;
        }
        let pos = self.stream.stream_position()?;
        self.stream.seek(SeekFrom::Start(8))?;
        self.stream.write_u32(self.total_unpacked_size)?;
        self.stream.seek(SeekFrom::Start(pos))?;
        self.is_finished = true;
        Ok(())
    }

    fn flush_block(&mut self) -> Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let (table, data) = compress_algo(&self.buffer);

        // Write table
        write_table(&mut self.stream, &table)?;

        // Write block size
        self.stream.write_u32(data.len() as u32)?;
        self.stream.write_all(&data)?;

        self.total_unpacked_size += self.buffer.len() as u32;
        self.buffer.clear();
        Ok(())
    }
}

impl<W: Write + Seek> Write for Compressor<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut pos = 0;
        while pos < buf.len() {
            let space = 0x10000 - self.buffer.len();
            let copy = space.min(buf.len() - pos);
            self.buffer.extend_from_slice(&buf[pos..pos + copy]);
            pos += copy;
            if self.buffer.len() >= 0x10000 {
                self.flush_block()
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.stream.flush()
    }
}

impl<W: Write + Seek> Drop for Compressor<W> {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

fn write_table<W: Write>(writer: &mut W, table: &[[u8; 2]; 256]) -> Result<()> {
    let mut i = 0;
    while i < 256 {
        // Count consecutive identities
        let mut n_identities = 0;
        let mut j = i;
        while j < 256 && table[j][0] == j as u8 {
            n_identities += 1;
            j += 1;
        }

        if n_identities > 0 {
            let k = n_identities.min(128);
            if i + k == 256 {
                writer.write_u8(127 + k as u8)?;
                i += k;
            } else {
                writer.write_u8(127 + k as u8)?;
                i += k;
                // Write explicit
                writer.write_u8(table[i][0])?;
                if table[i][0] != i as u8 {
                    writer.write_u8(table[i][1])?;
                }
                i += 1;
            }
        } else {
            let mut count = 0;
            let mut j = i;
            while j < 256 && count < 128 {
                if j + 1 < 256 && table[j][0] == j as u8 && table[j + 1][0] == (j + 1) as u8 {
                    break;
                }
                count += 1;
                j += 1;
            }

            writer.write_u8((count - 1) as u8)?;
            for k in 0..count {
                let curr = i + k;
                writer.write_u8(table[curr][0])?;
                if table[curr][0] != curr as u8 {
                    writer.write_u8(table[curr][1])?;
                }
            }
            i += count;
        }
    }
    Ok(())
}

fn compress_algo(input: &[u8]) -> ([[u8; 2]; 256], Vec<u8>) {
    let mut tokens = input.to_vec();
    let mut table = [[0u8; 2]; 256];
    for i in 0..256 {
        table[i][0] = i as u8;
    }

    let max_iterations = 256;
    for _ in 0..max_iterations {
        let mut pair_counts = vec![0u32; 65536];
        let mut max_pair_idx = 0;
        let mut max_pair_count = 0;

        if tokens.len() < 2 {
            break;
        }

        for i in 0..tokens.len() - 1 {
            let pair = ((tokens[i] as usize) << 8) | (tokens[i + 1] as usize);
            pair_counts[pair] += 1;
            if pair_counts[pair] > max_pair_count {
                max_pair_count = pair_counts[pair];
                max_pair_idx = pair;
            }
        }

        // Must appear at least twice to save space (2 bytes * 2 -> 1 byte * 2 + overhead)
        if max_pair_count < 2 {
            break;
        }

        let is_used = get_used_tokens(&tokens, &table);
        let mut unused = None;
        for i in 0..256 {
            if !is_used[i] {
                unused = Some(i as u8);
                break;
            }
        }

        if let Some(token) = unused {
            let left = (max_pair_idx >> 8) as u8;
            let right = (max_pair_idx & 0xFF) as u8;

            table[token as usize] = [left, right];

            let mut new_tokens = Vec::with_capacity(tokens.len());
            let mut i = 0;
            while i < tokens.len() {
                if i + 1 < tokens.len() && tokens[i] == left && tokens[i + 1] == right {
                    new_tokens.push(token);
                    i += 2;
                } else {
                    new_tokens.push(tokens[i]);
                    i += 1;
                }
            }
            tokens = new_tokens;
        } else {
            break;
        }
    }
    (table, tokens)
}

fn get_used_tokens(tokens: &[u8], table: &[[u8; 2]; 256]) -> [bool; 256] {
    let mut used = [false; 256];
    let mut stack = Vec::with_capacity(256);

    // Mark direct tokens
    for &t in tokens {
        if !used[t as usize] {
            used[t as usize] = true;
            stack.push(t);
        }
    }

    // Propagate
    while let Some(t) = stack.pop() {
        // If t is composite, mark children
        // Check if t is composite: table[t][0] != t
        let t_idx = t as usize;
        if table[t_idx][0] != t {
            let l = table[t_idx][0];
            let r = table[t_idx][1];

            if !used[l as usize] {
                used[l as usize] = true;
                stack.push(l);
            }
            if !used[r as usize] {
                used[r as usize] = true;
                stack.push(r);
            }
        }
    }
    used
}

pub fn compress(data: &[u8]) -> Result<Vec<u8>> {
    let mut cursor = std::io::Cursor::new(Vec::new());
    {
        let mut compressor = Compressor::new(&mut cursor)?;
        compressor.write_all(data)?;
        compressor.finish()?;
    }
    Ok(cursor.into_inner())
}

#[test]
fn test_compress_decompress() -> Result<()> {
    let data = b"The quick brown fox jumps over the lazy dog.".repeat(100);
    println!("Original size: {}", data.len());
    let compressed = compress(&data)?;
    println!("Compressed size: {}", compressed.len());
    let mut decompressed = decompress(Box::new(MemReaderRef::new(&compressed)))?;
    let mut output = Vec::new();
    decompressed.read_to_end(&mut output)?;
    assert_eq!(data.as_slice(), output.as_slice());
    Ok(())
}
