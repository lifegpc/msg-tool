use super::*;
use crate::ext::mutex::*;
use crate::utils::lzss::*;
use std::sync::Mutex;

macro_rules! base_schema_impl {
    () => {
        fn hash_after_crypt(&self) -> bool {
            AsRef::<BaseSchema>::as_ref(self).hash_after_crypt
        }
        fn startup_tjs_not_encrypted(&self) -> bool {
            AsRef::<BaseSchema>::as_ref(self).startup_tjs_not_encrypted
        }
        fn obfuscated_index(&self) -> bool {
            AsRef::<BaseSchema>::as_ref(self).obfuscated_index
        }
    };
}

fn convert_u32_from_string(input: &str) -> Result<u32> {
    let s = input.trim();
    if s.is_empty() {
        anyhow::bail!("String is empty");
    }
    Ok(if s.starts_with("0x") || s.starts_with("0X") {
        u32::from_str_radix(&s[2..], 16)?
    } else if s.starts_with('#') {
        u32::from_str_radix(&s[1..], 16)?
    } else if s.to_lowercase().starts_with("&h") {
        u32::from_str_radix(&s[2..], 16)?
    } else {
        s.parse::<u32>()?
    })
}

trait IChainReactionCrypt: std::fmt::Debug {
    fn get_encryption_limit(&self, entry: &Xp3Entry) -> u32;
    fn init(&self, archive: &mut Xp3Archive) -> Result<()>;
}

#[derive(Debug)]
struct ChainReactionCryptBase {
    encryption_threshold_map: Mutex<HashMap<u32, u32>>,
    list_bin: String,
}

impl ChainReactionCryptBase {
    fn new(list_bin: String) -> Self {
        Self {
            encryption_threshold_map: Mutex::new(HashMap::new()),
            list_bin,
        }
    }

    fn init2(&self, mut bin: Vec<u8>) -> Result<()> {
        if !bin.starts_with(b"\"\r\n") {
            for _ in 0..3 {
                bin = Self::decode_list_bin(bin)?;
            }
            // std::fs::write("test.bin", &bin)?;
        }
        self.encryption_threshold_map.lock_blocking().clear();
        self.parse_list_bin(bin)
    }

    fn read_list_bin(archive: &mut Xp3Archive, list_name: &str) -> Result<Option<Vec<u8>>> {
        let bin = match archive.entries.iter().find(|x| x.name == list_name) {
            Some(index) => index.clone(),
            None => return Ok(None),
        };
        let mut entry = Entry::new2(
            archive.inner.clone(),
            bin,
            archive.base_offset,
            archive.crypt.clone(),
        );
        let mut data = Vec::new();
        entry.read_to_end(&mut data)?;
        Ok(Some(data))
    }

    fn parse_list_bin(&self, data: Vec<u8>) -> Result<()> {
        let mut map = self.encryption_threshold_map.lock_blocking();
        let decoded = decode_to_string(Encoding::Utf8, &data, true)?;
        for line in decoded.lines() {
            let line = line.trim();
            if line.is_empty() || !line.starts_with("0") {
                continue;
            }
            let pair: Vec<_> = line.split(',').collect();
            if pair.len() > 1 {
                let hash = convert_u32_from_string(pair[0])?;
                let threshold = convert_u32_from_string(pair[1])?;
                map.insert(hash, threshold);
            }
        }
        Ok(())
    }

    fn decode_list_bin(data: Vec<u8>) -> Result<Vec<u8>> {
        let mut header = [0; 0x30];
        Self::decode_dpd(&data[..0x30], &mut header)?;
        let hread = MemReaderRef::new(&header);
        let packed_size = hread.cpeek_u32_at(0x0c)? as usize;
        let unpacked_size = hread.cpeek_u32_at(0x10)? as usize;
        if packed_size > data.len() - 0x30 {
            anyhow::bail!("Data is too smail.");
        }
        let sig = &header[0..4];
        if sig == b"DPDC" {
            let mut decrypted = Vec::with_capacity(packed_size);
            decrypted.resize(packed_size, 0);
            Self::decode_dpd(&data[0x30..packed_size + 0x30], &mut decrypted)?;
            Ok(decrypted)
        } else if sig == b"SZLC" {
            let reader = MemReaderRef::new(&data[0x30..packed_size + 0x30]);
            let mut lzss = LzssReader::new(reader);
            let mut result = Vec::with_capacity(unpacked_size);
            lzss.read_to_end(&mut result)?;
            if result.len() > unpacked_size {
                result.truncate(unpacked_size);
            }
            Ok(result)
        } else if sig == b"ELRC" {
            let min_repeat = hread.cpeek_u32_at(0x1C)?;
            let mut decoded = Vec::with_capacity(unpacked_size);
            decoded.resize(unpacked_size, 0);
            Self::decode_rle(&data[0x30..packed_size + 0x30], &mut decoded, min_repeat)?;
            Ok(decoded)
        } else {
            anyhow::bail!("Unknown signature: {:?}", sig);
        }
    }

    fn decode_dpd(src: &[u8], dst: &mut [u8]) -> Result<()> {
        let length = src.len();
        if length != dst.len() {
            anyhow::bail!("Length no matched.");
        }
        if length < 8 {
            dst.copy_from_slice(src);
            return Ok(());
        }
        let tail = length & 3;
        if tail > 0 {
            dst[length - tail..].copy_from_slice(&src[length - tail..]);
        }
        let length = length / 4;
        let mut reader = MemReaderRef::new(src);
        let mut writer = MemWriterRef::new(dst);
        let mut val = reader.read_u32()?;
        for _ in 0..length - 1 {
            let nval = reader.read_u32()?;
            writer.write_u32(val ^ nval)?;
            val = nval;
        }
        let fdst = writer.peek_u32_at(0)?;
        writer.write_u32(fdst ^ val)?;
        Ok(())
    }

    fn decode_rle(src: &[u8], dst: &mut [u8], min_repeat: u32) -> Result<()> {
        let mut reader = MemReaderRef::new(src);
        let mut writer = MemWriterRef::new(dst);
        while !reader.is_eof() {
            let b = reader.read_u8()?;
            let mut repeat = 1;
            while repeat < min_repeat && !reader.is_eof() && reader.cpeek_u8()? == b {
                repeat += 1;
                reader.pos += 1;
            }
            if repeat == min_repeat {
                let ctl = reader.read_u8()?;
                if ctl > 0x7F {
                    repeat += (reader.read_u8()? as u32) + (((ctl & 0x7F) as u32) << 8) + 0x80;
                } else {
                    repeat += ctl as u32;
                }
            }
            for _ in 0..repeat {
                writer.write_u8(b)?;
            }
        }
        Ok(())
    }
}

impl IChainReactionCrypt for ChainReactionCryptBase {
    fn get_encryption_limit(&self, entry: &Xp3Entry) -> u32 {
        self.encryption_threshold_map
            .lock_blocking()
            .get(&entry.file_hash)
            .map(|s| *s)
            .unwrap_or(0x200)
    }
    fn init(&self, archive: &mut Xp3Archive) -> Result<()> {
        let bin = Self::read_list_bin(archive, &self.list_bin)?;
        if let Some(bin) = bin {
            if bin.len() >= 0x30 {
                self.init2(bin)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct ChainReactionCrypt {
    base: BaseSchema,
    inner: Box<dyn IChainReactionCrypt + Send + Sync>,
}

impl ChainReactionCrypt {
    pub fn new(base: BaseSchema) -> Self {
        Self {
            base,
            inner: Box::new(ChainReactionCryptBase::new("plugin/list.bin".into())),
        }
    }

    fn new_inner(base: BaseSchema, inner: Box<dyn IChainReactionCrypt + Send + Sync>) -> Self {
        Self { base, inner }
    }
}

impl AsRef<BaseSchema> for ChainReactionCrypt {
    fn as_ref(&self) -> &BaseSchema {
        &self.base
    }
}

impl Crypt for ChainReactionCrypt {
    base_schema_impl!();
    fn init(&self, archive: &mut Xp3Archive) -> Result<()> {
        self.inner.init(archive)
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
        Ok(Box::new(ChainReactionCryptReader::new(
            stream,
            cur_seg,
            (self.inner.get_encryption_limit(entry), entry.file_hash),
        )))
    }
    fn decrypt_with_seek<'a>(
        &self,
        entry: &Xp3Entry,
        cur_seg: &Segment,
        stream: Box<dyn ReadSeek + Send + Sync + 'a>,
    ) -> Result<Box<dyn ReadSeek + Send + Sync + 'a>> {
        Ok(Box::new(ChainReactionCryptReader::new(
            stream,
            cur_seg,
            (self.inner.get_encryption_limit(entry), entry.file_hash),
        )))
    }
}

impl<R: Read> Read for ChainReactionCryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.inner.read(buf)?;
        let (limit, hash) = self.key;
        let limit = limit as u64;
        let mut offset = self.seg_start + self.pos;
        if offset < limit {
            let count = (limit - offset).min(readed as u64);
            for t in buf[..count as usize].iter_mut() {
                *t ^= (offset ^ ((hash >> ((offset & 3) << 3)) as u8) as u64) as u8;
                offset += 1;
            }
        }
        self.pos += readed as u64;
        Ok(readed)
    }
}

#[derive(Debug)]
pub struct HachukanoCrypt {
    base: ChainReactionCryptBase,
}

impl HachukanoCrypt {
    pub fn new(base: BaseSchema) -> ChainReactionCrypt {
        ChainReactionCrypt::new_inner(
            base,
            Box::new(Self {
                base: ChainReactionCryptBase::new("plugins/list.txt".into()),
            }),
        )
    }
}

impl IChainReactionCrypt for HachukanoCrypt {
    fn get_encryption_limit(&self, entry: &Xp3Entry) -> u32 {
        let limit = self.base.get_encryption_limit(entry);
        match limit {
            0 => 0,
            1 => 0x100,
            2 => 0x200,
            3 => entry.original_size as u32,
            _ => limit,
        }
    }
    fn init(&self, archive: &mut Xp3Archive) -> Result<()> {
        self.base.init(archive)
    }
}

#[derive(Debug)]
pub struct ChocolatCrypt {
    base: ChainReactionCryptBase,
}

impl ChocolatCrypt {
    pub fn new(base: BaseSchema) -> ChainReactionCrypt {
        ChainReactionCrypt::new_inner(
            base,
            Box::new(Self {
                base: ChainReactionCryptBase::new("plugins/list.txt".into()),
            }),
        )
    }
}

impl IChainReactionCrypt for ChocolatCrypt {
    fn get_encryption_limit(&self, entry: &Xp3Entry) -> u32 {
        let limit = self.base.get_encryption_limit(entry);
        match limit {
            0 => 0,
            2 => entry.original_size as u32,
            _ => 0x100,
        }
    }
    fn init(&self, archive: &mut Xp3Archive) -> Result<()> {
        self.base.init(archive)
    }
}

#[derive(Debug)]
pub struct XanaduCrypt {
    base: BaseSchema,
    inner: ChainReactionCryptBase,
}

impl XanaduCrypt {
    pub fn new(base: BaseSchema) -> Self {
        Self {
            base,
            inner: ChainReactionCryptBase::new("plugins/list.txt".into()),
        }
    }
}

impl AsRef<BaseSchema> for XanaduCrypt {
    fn as_ref(&self) -> &BaseSchema {
        &self.base
    }
}

impl IChainReactionCrypt for XanaduCrypt {
    fn get_encryption_limit(&self, entry: &Xp3Entry) -> u32 {
        let limit = self.inner.get_encryption_limit(entry);
        match limit {
            0 => 0,
            2 => entry.original_size as u32,
            _ => 0x100,
        }
    }
    fn init(&self, archive: &mut Xp3Archive) -> Result<()> {
        let mut bin = ChainReactionCryptBase::read_list_bin(archive, "list2.txt")?;
        if bin.is_none() {
            bin = ChainReactionCryptBase::read_list_bin(archive, "plugins/list.txt")?;
        }
        if let Some(bin) = bin {
            self.inner.init2(bin)?;
        }
        Ok(())
    }
}

impl Crypt for XanaduCrypt {
    base_schema_impl!();
    fn init(&self, archive: &mut Xp3Archive) -> Result<()> {
        IChainReactionCrypt::init(self, archive)
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
        Ok(Box::new(XanaduCryptReader::new(
            stream,
            cur_seg,
            (self.get_encryption_limit(entry), entry.file_hash),
        )))
    }
    fn decrypt_with_seek<'a>(
        &self,
        entry: &Xp3Entry,
        cur_seg: &Segment,
        stream: Box<dyn ReadSeek + Send + Sync + 'a>,
    ) -> Result<Box<dyn ReadSeek + Send + Sync + 'a>> {
        Ok(Box::new(XanaduCryptReader::new(
            stream,
            cur_seg,
            (self.get_encryption_limit(entry), entry.file_hash),
        )))
    }
}

impl<R: Read> Read for XanaduCryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.inner.read(buf)?;
        let (limit, hash) = self.key;
        let limit = limit as u64;
        let mut offset = self.seg_start + self.pos;
        if offset < limit {
            let key = hash ^ (!0x03020100);
            let count = (limit - offset).min(readed as u64);
            for t in buf[..count as usize].iter_mut() {
                let extra = (((offset & 0xFF) >> 2) << 2) as u8;
                *t ^= (key >> (((offset & 3) << 3) as u32)) as u8 ^ extra;
                offset += 1;
            }
        }
        self.pos += readed as u64;
        Ok(readed)
    }
}
