use super::types::*;
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use crate::utils::mmx::*;
use anyhow::Result;
use std::io::Read;

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
