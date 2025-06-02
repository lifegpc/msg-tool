use crate::ext::io::*;
use anyhow::Result;
use std::io::{Read, Seek};

pub struct CryptoReader<T: Read + Seek> {
    reader: T,
    _key: u32,
    max_pos: u32,
}

impl<T: Read + Seek> CryptoReader<T> {
    pub fn new(mut reader: T) -> Result<Self> {
        let _key = reader.peek_u32_at(0x8)?;
        let mut s = CryptoReader {
            reader,
            _key,
            max_pos: 0,
        };
        s.init()?;
        Ok(s)
    }

    fn key(&mut self) -> u32 {
        self._key ^= 0x65AC9365;
        self._key ^= (((self._key >> 1) ^ self._key) >> 3) ^ (((self._key << 1) ^ self._key) << 3);
        return self._key;
    }

    fn init(&mut self) -> Result<()> {
        let _key = self._key;
        self.max_pos = (self.reader.peek_u32_at(0xC)? ^ self.key()) * 12 + 0xC;
        self._key = _key;
        Ok(())
    }
}

impl<T: Read + Seek> Read for CryptoReader<T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let remaing = self.max_pos as usize + 0x8 - self.reader.stream_position()? as usize;
        let count = buf.len().min(remaing);
        let readed = self.reader.read(&mut buf[..count])?;
        for i in 0..readed / 4 {
            let val = u32::from_le_bytes(buf[i * 4..i * 4 + 4].try_into().map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Failed to convert slice to u32",
                )
            })?);
            let decrypted = val ^ self.key();
            buf[i * 4..i * 4 + 4].copy_from_slice(&decrypted.to_le_bytes());
        }
        Ok(readed)
    }
}
