use super::*;

#[derive(Debug)]
pub struct NVLCrypt {
    base: BaseSchema,
    key: [u8; 8],
}

impl NVLCrypt {
    pub fn new(base: BaseSchema, key: &[u8]) -> Result<Self> {
        if key.len() != 8 {
            anyhow::bail!("Key must be 8 bytes.");
        }
        Ok(Self {
            base,
            key: key.try_into()?,
        })
    }

    fn get_key(&self, hash: u32) -> [u8; 12] {
        let mut key = [0; 12];
        key[..4].copy_from_slice(&hash.to_le_bytes());
        key[4..].copy_from_slice(&self.key);
        key
    }
}

impl Crypt for NVLCrypt {
    fn hash_after_crypt(&self) -> bool {
        self.base.hash_after_crypt
    }
    fn startup_tjs_not_encrypted(&self) -> bool {
        self.base.startup_tjs_not_encrypted
    }
    fn obfuscated_index(&self) -> bool {
        self.base.obfuscated_index
    }
    fn read_name<'a>(&self, reader: &mut Box<dyn Read + 'a>) -> Result<(String, u64)> {
        let hash_key = reader.read_u32()?;
        let name_hash = reader.read_u32()?;
        let extension_hash = reader.read_u32()?;
        Ok((
            format!(
                "{:08x}.{:08x}",
                hash_key ^ name_hash,
                hash_key ^ extension_hash
            ),
            12,
        ))
    }
    fn decrypt_supported(&self) -> bool {
        true
    }
    fn decrypt_seek_supported(&self) -> bool {
        true
    }
    fn decrypt<'a>(
        &self,
        entry: &Xp3Entry,
        cur_seg: &Segment,
        stream: Box<dyn Read + Send + Sync + 'a>,
    ) -> Result<Box<dyn ReadDebug + Send + Sync + 'a>> {
        Ok(Box::new(NVLCryptReader::new(
            stream,
            cur_seg,
            self.get_key(entry.file_hash),
        )))
    }
    fn decrypt_with_seek<'a>(
        &self,
        entry: &Xp3Entry,
        cur_seg: &Segment,
        stream: Box<dyn ReadSeek + Send + Sync + 'a>,
    ) -> Result<Box<dyn ReadSeek + Send + Sync + 'a>> {
        Ok(Box::new(NVLCryptReader::new(
            stream,
            cur_seg,
            self.get_key(entry.file_hash),
        )))
    }
}

impl<R: Read> Read for NVLCryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.inner.read(buf)?;
        let mut offset = ((self.pos + self.seg_start) % 12) as usize;
        for t in (&mut buf[..readed]).iter_mut() {
            *t ^= self.key[offset];
            offset = (offset + 1) % 12;
        }
        self.pos += readed as u64;
        Ok(readed)
    }
}
