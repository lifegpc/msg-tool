mod cx;

use super::archive::*;
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use crate::utils::serde_base64bytes::*;
use crate::utils::simple_pack::*;
use anyhow::Result;
use msg_tool_xp3data::*;
use overf::wrapping as w;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Seek, SeekFrom};
use std::sync::Arc;

pub fn default_init_crypt(archive: &mut Xp3Archive) -> Result<()> {
    if archive.extras.iter().any(|extra| extra.is_filename_hash()) {
        let mut filename_map = HashMap::new();
        for extra in &archive.extras {
            if extra.is_filename_hash() {
                let mut reader = MemReaderRef::new(&extra.data);
                let hash = reader.read_u32()?;
                let name_length = reader.read_u16()?;
                let name = reader.read_exact_vec(name_length as usize * 2)?;
                let name = decode_to_string(Encoding::Utf16LE, &name, true)?;
                filename_map.insert(hash, name);
            }
        }
        archive.extras.retain(|extra| !extra.is_filename_hash());
        for entry in &mut archive.entries {
            if let Some(name) = filename_map.get(&entry.file_hash) {
                entry.name = name.clone();
            }
        }
    }
    Ok(())
}

pub trait Crypt: std::fmt::Debug {
    #[allow(dead_code)]
    /// whether Adler32 checksum should be calculated after contents have been encrypted.
    fn hash_after_crypt(&self) -> bool;

    /// whether the startup.tjs script is not encrypted even when the archive is encrypted.
    fn startup_tjs_not_encrypted(&self) -> bool;

    /// whether XP3 index is obfuscated:
    ///  - duplicate entries
    ///  - entries have additional dummy segments
    #[allow(dead_code)]
    fn obfuscated_index(&self) -> bool;

    /// Initializes the cryptographic context for the archive.
    fn init(&self, archive: &mut Xp3Archive) -> Result<()> {
        default_init_crypt(archive)
    }

    /// Read a entry name from archive index
    fn read_name<'a>(&self, reader: &mut Box<dyn Read + 'a>) -> Result<(String, u64)> {
        let name_length = reader.read_u16()?;
        let name = reader.read_exact_vec(name_length as usize * 2)?;
        Ok((
            decode_to_string(Encoding::Utf16LE, &name, true)?,
            name_length as u64 * 2 + 2,
        ))
    }

    /// Decrypts the given stream of data for the specified entry and segment.
    fn decrypt<'a>(
        &self,
        _entry: &Xp3Entry,
        _cur_seg: &Segment,
        _stream: Box<dyn Read + 'a>,
    ) -> Result<Box<dyn ReadDebug + 'a>> {
        Err(anyhow::anyhow!("This crypt does not support decrypt"))
    }

    /// Decrypts the given stream of data for the specified entry and segment, with seek support.
    fn decrypt_with_seek<'a>(
        &self,
        _entry: &Xp3Entry,
        _cur_seg: &Segment,
        _stream: Box<dyn ReadSeek + 'a>,
    ) -> Result<Box<dyn ReadSeek + 'a>> {
        Err(anyhow::anyhow!(
            "This crypt does not support decrypt with seek"
        ))
    }

    /// Returns true if this crypt support decrypt
    fn decrypt_supported(&self) -> bool {
        false
    }

    /// Returns true if this crypt support seek when decrypting
    fn decrypt_seek_supported(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CxSchema {
    mask: u32,
    offset: u32,
    prolog_order: Base64Bytes,
    odd_branch_order: Base64Bytes,
    even_branch_order: Base64Bytes,
    control_block_name: Option<String>,
    tpm_file_name: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "PascalCase", tag = "$type")]
enum CryptType {
    NoCrypt,
    FateCrypt,
    MizukakeCrypt,
    HashCrypt,
    #[serde(rename_all = "PascalCase")]
    XorCrypt {
        key: u8,
    },
    FlyingShineCrypt,
    CxEncryption(CxSchema),
    #[serde(rename_all = "PascalCase")]
    SenrenCxCrypt {
        #[serde(flatten)]
        cx: CxSchema,
        names_section_id: String,
    },
    #[serde(rename_all = "PascalCase")]
    CabbageCxCrypt {
        #[serde(flatten)]
        cx: CxSchema,
        names_section_id: String,
        random_seed: u32,
    },
    #[serde(rename_all = "PascalCase")]
    NanaCxCrypt {
        #[serde(flatten)]
        cx: CxSchema,
        names_section_id: String,
        random_seed: u32,
        yuz_key: Vec<u32>,
    },
    #[serde(rename_all = "PascalCase")]
    RiddleCxCrypt {
        #[serde(flatten)]
        cx: CxSchema,
        names_section_id: String,
        random_seed: u32,
        yuz_key: Vec<u32>,
        #[serde(default)]
        key1: u32,
        #[serde(default)]
        key2: u32,
    },
    SeitenCrypt,
    OkibaCrypt,
    DieselmineCrypt,
    DameganeCrypt,
    NephriteCrypt,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct BaseSchema {
    #[serde(default)]
    hash_after_crypt: bool,
    #[serde(default)]
    startup_tjs_not_encrypted: bool,
    #[serde(default)]
    obfuscated_index: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Schema {
    #[serde(flatten)]
    crypt: CryptType,
    title: Option<String>,
    #[serde(flatten)]
    base: BaseSchema,
}

impl Schema {
    pub fn create_crypt(&self, filename: &str) -> Result<Box<dyn Crypt>> {
        Ok(match &self.crypt {
            CryptType::NoCrypt => Box::new(NoCrypt::new()),
            CryptType::FateCrypt => Box::new(FateCrypt::new(self.base.clone())),
            CryptType::MizukakeCrypt => Box::new(MizukakeCrypt::new(self.base.clone())),
            CryptType::HashCrypt => Box::new(HashCrypt::new(self.base.clone())),
            CryptType::XorCrypt { key } => Box::new(XorCrypt::new(self.base.clone(), *key)),
            CryptType::FlyingShineCrypt => Box::new(FlyingShineCrypt::new(self.base.clone())),
            CryptType::CxEncryption(schema) => {
                Box::new(cx::CxEncryption::new(self.base.clone(), &schema, filename)?)
            }
            CryptType::SenrenCxCrypt {
                cx,
                names_section_id,
            } => Box::new(cx::SenrenCxCrypt::new(
                self.base.clone(),
                cx,
                filename,
                names_section_id.clone(),
            )?),
            CryptType::CabbageCxCrypt {
                cx,
                names_section_id,
                random_seed,
            } => Box::new(cx::CabbageCxCrypt::new(
                self.base.clone(),
                cx,
                filename,
                names_section_id.clone(),
                *random_seed,
            )?),
            CryptType::NanaCxCrypt {
                cx,
                names_section_id,
                random_seed,
                yuz_key,
            } => Box::new(cx::NanaCxCrypt::new(
                self.base.clone(),
                cx,
                filename,
                names_section_id.clone(),
                *random_seed,
                &yuz_key,
            )?),
            CryptType::RiddleCxCrypt {
                cx,
                names_section_id,
                random_seed,
                yuz_key,
                key1,
                key2,
            } => Box::new(cx::RiddleCxCrypt::new(
                self.base.clone(),
                cx,
                filename,
                names_section_id.clone(),
                *random_seed,
                &yuz_key,
                *key1,
                *key2,
            )?),
            CryptType::SeitenCrypt => Box::new(SeitenCrypt::new(self.base.clone())),
            CryptType::OkibaCrypt => Box::new(OkibaCrypt::new(self.base.clone())),
            CryptType::DieselmineCrypt => Box::new(DieselmineCrypt::new(self.base.clone())),
            CryptType::DameganeCrypt => Box::new(DameganeCrypt::new(self.base.clone())),
            CryptType::NephriteCrypt => Box::new(NephriteCrypt::new(self.base.clone())),
        })
    }
}

lazy_static::lazy_static! {
    static ref CRYPT_SCHEMA: BTreeMap<String, Schema> = {
        serde_json::from_str(&get_crypt_data()).expect("Failed to parse crypt.json")
    };
    static ref ALIAS_TABLE: HashMap<String, String> = {
        let mut table = HashMap::new();
        for (game, fulltitle) in get_supported_games_with_title() {
            if let Some(title) = fulltitle {
                let mut alias_count = 0usize;
                for part in title.split("|") {
                    let alias = part.trim();
                    table.insert(alias.to_string(), game.to_string());
                    alias_count += 1;
                }
                // also insert full title if there are multiple aliases
                if alias_count > 1 {
                    table.insert(title.to_string(), game.to_string());
                }
            }
        }
        table
    };
    static ref CX_CB_TABLE: HashMap<String, Vec<u32>> = {
        let reader = MemReaderRef::new(CX_CB_DATA);
        let mut pack = read_simple_pack(reader).expect("Failed to read cx_cb.pck");
        let mut table = HashMap::new();
        while let Some(mut entry) = pack.next().expect("Failed to read entry in cx_cb.pck") {
            let mut list = Vec::with_capacity(0x400);
            let errmsg = format!("Failed to read u32 in cx_cb.pck entry {}", entry.name);
            for _ in 0..0x400 {
                list.push(entry.read_u32().expect(&errmsg));
            }
            table.insert(entry.name.clone(), list);
        }
        table
    };
}

/// Get the supported game titles for encrypted xp3 archives.
pub fn get_supported_games() -> Vec<&'static str> {
    CRYPT_SCHEMA.keys().map(|s| s.as_str()).collect()
}

/// Get the supported game titles for encrypted xp3 archives with their full titles.
pub fn get_supported_games_with_title() -> Vec<(&'static str, Option<&'static str>)> {
    CRYPT_SCHEMA
        .iter()
        .map(|(k, v)| (k.as_str(), v.title.as_deref()))
        .collect()
}

pub fn query_crypt_schema(game: &str) -> Option<&'static Schema> {
    CRYPT_SCHEMA.get(game).or_else(|| {
        ALIAS_TABLE
            .get(game)
            .and_then(|real_game| CRYPT_SCHEMA.get(real_game))
    })
}

#[derive(Debug)]
pub struct NoCrypt {}

impl NoCrypt {
    pub fn new() -> Self {
        Self {}
    }
}

impl Crypt for NoCrypt {
    fn hash_after_crypt(&self) -> bool {
        false
    }
    fn startup_tjs_not_encrypted(&self) -> bool {
        false
    }
    fn obfuscated_index(&self) -> bool {
        false
    }
}

macro_rules! seek_impl {
    ($reader:ident<$t:ident>) => {
        impl<$t: Read + Seek> Seek for $reader<$t> {
            fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
                let new_pos: i64 = match pos {
                    SeekFrom::Start(offset) => offset as i64,
                    SeekFrom::End(offset) => self.seg_size as i64 + offset,
                    SeekFrom::Current(offset) => self.pos as i64 + offset,
                };
                let offset = new_pos - self.pos as i64;
                if offset != 0 {
                    self.inner.seek(SeekFrom::Current(offset))?;
                    self.pos = new_pos as u64;
                }
                Ok(self.pos)
            }
        }
    };
}

macro_rules! seek_reader_impl {
    ($reader:ident<$t:ident>) => {
        #[derive(msg_tool_macro::MyDebug)]
        struct $reader<$t: Read> {
            #[skip_fmt]
            inner: $t,
            /// Start offset of the current xp3 entry.
            seg_start: u64,
            seg_size: u64,
            pos: u64,
        }
        impl<$t: Read> $reader<$t> {
            pub fn new(inner: $t, seg: &Segment) -> Self {
                Self {
                    inner,
                    seg_start: seg.offset_in_file,
                    seg_size: seg.original_size,
                    pos: 0,
                }
            }
        }
        seek_impl!($reader<$t>);
    };
}

macro_rules! seek_reader_key_impl {
    ($reader:ident<$t:ident>, $key:ty) => {
        #[derive(msg_tool_macro::MyDebug)]
        #[allow(dead_code)]
        struct $reader<$t: Read> {
            #[skip_fmt]
            inner: $t,
            /// Start offset of the current xp3 entry.
            seg_start: u64,
            seg_size: u64,
            pos: u64,
            key: $key,
        }
        impl<$t: Read> $reader<$t> {
            pub fn new(inner: $t, seg: &Segment, key: $key) -> Self {
                Self {
                    inner,
                    seg_start: seg.offset_in_file,
                    seg_size: seg.original_size,
                    pos: 0,
                    key,
                }
            }
        }
        seek_impl!($reader<$t>);
    };
}

macro_rules! base_schema_impl {
    () => {
        fn hash_after_crypt(&self) -> bool {
            self.base.hash_after_crypt
        }
        fn startup_tjs_not_encrypted(&self) -> bool {
            self.base.startup_tjs_not_encrypted
        }
        fn obfuscated_index(&self) -> bool {
            self.base.obfuscated_index
        }
    };
}

macro_rules! seek_crypt_base_impl {
    ($crypt:ident, $reader:ident) => {
        #[derive(Debug)]
        pub struct $crypt {
            base: BaseSchema,
        }
        impl $crypt {
            pub fn new(base: BaseSchema) -> Self {
                Self { base }
            }
        }
        impl Crypt for $crypt {
            base_schema_impl!();
            fn decrypt_supported(&self) -> bool {
                true
            }
            fn decrypt_seek_supported(&self) -> bool {
                true
            }
            fn decrypt<'a>(
                &self,
                _entry: &Xp3Entry,
                cur_seg: &Segment,
                stream: Box<dyn Read + 'a>,
            ) -> Result<Box<dyn ReadDebug + 'a>> {
                Ok(Box::new($reader::new(stream, cur_seg)))
            }
            fn decrypt_with_seek<'a>(
                &self,
                _entry: &Xp3Entry,
                cur_seg: &Segment,
                stream: Box<dyn ReadSeek + 'a>,
            ) -> Result<Box<dyn ReadSeek + 'a>> {
                Ok(Box::new($reader::new(stream, cur_seg)))
            }
        }
    };
}

macro_rules! seek_crypt_impl {
    ($crypt:ident, $reader:ident<$t:ident>) => {
        seek_crypt_base_impl!($crypt, $reader);
        seek_reader_impl!($reader<$t>);
    };
}

seek_crypt_impl!(FateCrypt, FateCryptReader<T>);

impl<R: Read> Read for FateCryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        const XOR1_OFFSET: u64 = 0x13;
        const XOR3_OFFSET: u64 = 0x2ea29;
        let readed = self.inner.read(buf)?;
        for (i, t) in (&mut buf[..readed]).iter_mut().enumerate() {
            let tpos = self.seg_start + self.pos + i as u64;
            *t ^= 0x36;
            if tpos == XOR1_OFFSET {
                *t ^= 0x1;
            } else if tpos == XOR3_OFFSET {
                *t ^= 0x3;
            }
        }
        self.pos += readed as u64;
        Ok(readed)
    }
}

seek_crypt_impl!(MizukakeCrypt, MizukakeCryptReader<T>);

impl<R: Read> Read for MizukakeCryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.inner.read(buf)?;
        for (i, t) in (&mut buf[..readed]).iter_mut().enumerate() {
            let tpos = self.seg_start + self.pos + i as u64;
            if tpos == 0x103 {
                *t = (*t).wrapping_sub(1);
            }
            *t ^= 0xb6;
            if tpos == 0x3F82 {
                *t ^= 1;
            }
            if tpos == 0x83 {
                *t ^= 3;
            }
        }
        self.pos += readed as u64;
        Ok(readed)
    }
}

macro_rules! seek_crypt_filehash_key_u8_base_impl {
    ($crypt:ident, $reader:ident) => {
        #[derive(Debug)]
        pub struct $crypt {
            base: BaseSchema,
        }
        impl $crypt {
            pub fn new(base: BaseSchema) -> Self {
                Self { base }
            }
        }
        impl Crypt for $crypt {
            base_schema_impl!();
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
                stream: Box<dyn Read + 'a>,
            ) -> Result<Box<dyn ReadDebug + 'a>> {
                Ok(Box::new($reader::new(
                    stream,
                    cur_seg,
                    entry.file_hash as u8,
                )))
            }
            fn decrypt_with_seek<'a>(
                &self,
                entry: &Xp3Entry,
                cur_seg: &Segment,
                stream: Box<dyn ReadSeek + 'a>,
            ) -> Result<Box<dyn ReadSeek + 'a>> {
                Ok(Box::new($reader::new(
                    stream,
                    cur_seg,
                    entry.file_hash as u8,
                )))
            }
        }
    };
}

macro_rules! seek_crypt_filehash_key_u8_impl {
    ($crypt:ident,$reader:ident<$t:ident>) => {
        seek_crypt_filehash_key_u8_base_impl!($crypt, $reader);
        seek_reader_key_impl!($reader<$t>, u8);
    };
}

seek_crypt_filehash_key_u8_impl!(HashCrypt, HashCryptReader<T>);

impl<R: Read> Read for HashCryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.inner.read(buf)?;
        for t in (&mut buf[..readed]).iter_mut() {
            *t ^= self.key;
        }
        self.pos += readed as u64;
        Ok(readed)
    }
}

#[derive(Debug)]
pub struct XorCrypt {
    base: BaseSchema,
    key: u8,
}

impl XorCrypt {
    pub fn new(base: BaseSchema, key: u8) -> Self {
        Self { base, key }
    }
}

impl Crypt for XorCrypt {
    base_schema_impl!();
    fn decrypt_supported(&self) -> bool {
        true
    }
    fn decrypt_seek_supported(&self) -> bool {
        true
    }
    fn decrypt<'a>(
        &self,
        _entry: &Xp3Entry,
        cur_seg: &Segment,
        stream: Box<dyn Read + 'a>,
    ) -> Result<Box<dyn ReadDebug + 'a>> {
        Ok(Box::new(XorCryptReader::new(stream, cur_seg, self.key)))
    }
    fn decrypt_with_seek<'a>(
        &self,
        _entry: &Xp3Entry,
        cur_seg: &Segment,
        stream: Box<dyn ReadSeek + 'a>,
    ) -> Result<Box<dyn ReadSeek + 'a>> {
        Ok(Box::new(XorCryptReader::new(stream, cur_seg, self.key)))
    }
}

seek_reader_key_impl!(XorCryptReader<T>, u8);

impl<R: Read> Read for XorCryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.inner.read(buf)?;
        for t in (&mut buf[..readed]).iter_mut() {
            *t ^= self.key;
        }
        self.pos += readed as u64;
        Ok(readed)
    }
}

#[derive(Debug)]
pub struct FlyingShineCrypt {
    base: BaseSchema,
}

impl FlyingShineCrypt {
    pub fn new(base: BaseSchema) -> Self {
        Self { base }
    }

    fn adjust(&self, hash: u32) -> (u8, u32) {
        let mut shift = hash & 0xFF;
        if shift == 0 {
            shift = 0xF;
        }
        let mut key = ((hash >> 8) & 0xFF) as u8;
        if key == 0 {
            key = 0xF0;
        }
        (key, shift)
    }
}

impl Crypt for FlyingShineCrypt {
    base_schema_impl!();
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
        stream: Box<dyn Read + 'a>,
    ) -> Result<Box<dyn ReadDebug + 'a>> {
        Ok(Box::new(FlyingShineCryptReader::new(
            stream,
            cur_seg,
            self.adjust(entry.file_hash),
        )))
    }
    fn decrypt_with_seek<'a>(
        &self,
        entry: &Xp3Entry,
        cur_seg: &Segment,
        stream: Box<dyn ReadSeek + 'a>,
    ) -> Result<Box<dyn ReadSeek + 'a>> {
        Ok(Box::new(FlyingShineCryptReader::new(
            stream,
            cur_seg,
            self.adjust(entry.file_hash),
        )))
    }
}

seek_reader_key_impl!(FlyingShineCryptReader<T>, (u8, u32));

impl<R: Read> Read for FlyingShineCryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let (xor, shift) = self.key;
        let readed = self.inner.read(buf)?;
        for t in (&mut buf[..readed]).iter_mut() {
            *t ^= xor;
            *t = t.rotate_right(shift);
        }
        self.pos += readed as u64;
        Ok(readed)
    }
}

macro_rules! seek_crypt_filehash_key_base_impl {
    ($crypt:ident, $reader:ident) => {
        #[derive(Debug)]
        pub struct $crypt {
            base: BaseSchema,
        }
        impl $crypt {
            pub fn new(base: BaseSchema) -> Self {
                Self { base }
            }
        }
        impl Crypt for $crypt {
            base_schema_impl!();
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
                stream: Box<dyn Read + 'a>,
            ) -> Result<Box<dyn ReadDebug + 'a>> {
                Ok(Box::new($reader::new(stream, cur_seg, entry.file_hash)))
            }
            fn decrypt_with_seek<'a>(
                &self,
                entry: &Xp3Entry,
                cur_seg: &Segment,
                stream: Box<dyn ReadSeek + 'a>,
            ) -> Result<Box<dyn ReadSeek + 'a>> {
                Ok(Box::new($reader::new(stream, cur_seg, entry.file_hash)))
            }
        }
    };
}

macro_rules! seek_crypt_filehash_key_impl {
    ($crypt:ident,$reader:ident<$t:ident>) => {
        seek_crypt_filehash_key_base_impl!($crypt, $reader);
        seek_reader_key_impl!($reader<$t>, u32);
    };
}

seek_crypt_filehash_key_impl!(SeitenCrypt, SeitenCryptReader<T>);

impl<R: Read> Read for SeitenCryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.inner.read(buf)?;
        let mut offset = self.seg_start + self.pos;
        for t in (&mut buf[..readed]).iter_mut() {
            let mut shift;
            let key = self.key ^ (offset as u32);
            if key & 2 != 0 {
                shift = key & 0x18;
                let ebx = key >> shift;
                shift &= 8;
                *t ^= (ebx | (key >> shift)) as u8;
            }
            if key & 4 != 0 {
                w!(*t += key as u8);
            }
            if key & 8 != 0 {
                shift = key & 0x10;
                w!(*t -= (key >> shift) as u8);
            }
            offset += 1;
        }
        self.pos += readed as u64;
        Ok(readed)
    }
}

seek_crypt_filehash_key_impl!(OkibaCrypt, OkibaCryptReader<T>);

impl<R: Read> Read for OkibaCryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.inner.read(buf)?;
        let mut offset = self.seg_start + self.pos;
        let mut i = 0;
        if offset < 0x65 {
            let key = self.key >> 4;
            let limit = readed.min(0x65 - offset as usize);
            for _ in 0..limit {
                buf[i] ^= key as u8;
                i += 1;
                offset += 1;
            }
        }
        if i < readed {
            offset -= 0x65;
            let mut key = self.key;
            key = ((key & 0xff0000) << 8)
                | ((key & 0xff000000) >> 8)
                | ((key & 0xff00) >> 8)
                | ((key & 0xff) << 8);
            loop {
                buf[i] ^= (key >> (8 * (offset as u32 & 3))) as u8;
                offset += 1;
                i += 1;
                if i >= readed {
                    break;
                }
            }
        }
        self.pos += readed as u64;
        Ok(readed)
    }
}

seek_crypt_filehash_key_u8_impl!(DieselmineCrypt, DieselmineCryptReader<T>);

impl<R: Read> Read for DieselmineCryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.inner.read(buf)?;
        let key = self.key as i32;
        for (i, t) in (&mut buf[..readed]).iter_mut().enumerate() {
            let offset = self.seg_start + self.pos + i as u64;
            let key = if offset < 123 {
                21 * key
            } else if offset < 246 {
                -32 * key
            } else if offset < 369 {
                43 * key
            } else {
                -54 * key
            } as u8;
            *t ^= key;
        }
        self.pos += readed as u64;
        Ok(readed)
    }
}

seek_crypt_filehash_key_u8_impl!(DameganeCrypt, DameganeCryptReader<T>);

impl<R: Read> Read for DameganeCryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.inner.read(buf)?;
        for (i, t) in (&mut buf[..readed]).iter_mut().enumerate() {
            let offset = self.seg_start + self.pos + i as u64;
            let key = if offset & 1 != 0 {
                self.key
            } else {
                offset as u8
            };
            *t ^= key;
        }
        self.pos += readed as u64;
        Ok(readed)
    }
}

seek_crypt_filehash_key_u8_impl!(NephriteCrypt, NephriteCryptReader<T>);

impl<R: Read> Read for NephriteCryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.inner.read(buf)?;
        for (i, t) in (&mut buf[..readed]).iter_mut().enumerate() {
            let offset = self.seg_start + self.pos + i as u64;
            let key = if offset & 1 == 0 {
                self.key
            } else {
                offset as u8
            };
            *t ^= key;
        }
        self.pos += readed as u64;
        Ok(readed)
    }
}

#[test]
fn test_deserialize_crypt() {
    for (key, schema) in CRYPT_SCHEMA.iter() {
        println!("Title: {}, Schema: {:?}", key, schema);
    }
}

#[test]
fn test_cx_cb_table() {
    for (key, list) in CX_CB_TABLE.iter() {
        println!("Key: {}, List length: {}", key, list.len());
    }
}
