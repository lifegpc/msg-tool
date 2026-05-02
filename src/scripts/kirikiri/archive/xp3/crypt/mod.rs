mod cx;
mod cz;

use super::Entry;
use super::archive::*;
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::case_insensitive_string::*;
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

type CIS = CaseInsensitiveStr;

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

fn default_read_name<'a>(reader: &mut Box<dyn Read + 'a>) -> Result<(String, u64)> {
    let name_length = reader.read_u16()?;
    let name = reader.read_exact_vec(name_length as usize * 2)?;
    Ok((
        decode_to_string(Encoding::Utf16LE, &name, true)?,
        name_length as u64 * 2 + 2,
    ))
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
        default_read_name(reader)
    }

    /// Decrypts the given stream of data for the specified entry and segment.
    fn decrypt<'a>(
        &self,
        _entry: &Xp3Entry,
        _cur_seg: &Segment,
        _stream: Box<dyn Read + Send + Sync + 'a>,
    ) -> Result<Box<dyn ReadDebug + Send + Sync + 'a>> {
        Err(anyhow::anyhow!("This crypt does not support decrypt"))
    }

    /// Decrypts the given stream of data for the specified entry and segment, with seek support.
    fn decrypt_with_seek<'a>(
        &self,
        _entry: &Xp3Entry,
        _cur_seg: &Segment,
        _stream: Box<dyn ReadSeek + Send + Sync + 'a>,
    ) -> Result<Box<dyn ReadSeek + Send + Sync + 'a>> {
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

    /// Determine whether the file with the given name and content need to be extra processed after decryption. (e.g. extra decryption by file type)
    fn need_filter(&self, _filename: &str, _buf: &[u8], _buf_len: usize) -> bool {
        false
    }

    /// Returns true if this crypt support seek when filtering
    fn filter_seek_supported(&self) -> bool {
        false
    }

    /// Apply extra processing to the decrypted content of the file.
    fn filter<'a>(&self, _entry: Entry<'a>) -> Result<Box<dyn ReadDebug + Send + Sync + 'a>> {
        Err(anyhow::anyhow!(
            "This crypt does not support content filter after decrypt"
        ))
    }

    /// Apply extra processing to the decrypted content of the file, with seek support.
    fn filter_with_seek<'a>(
        &self,
        _entry: Entry<'a>,
    ) -> Result<Box<dyn ReadSeek + Send + Sync + 'a>> {
        Err(anyhow::anyhow!(
            "This crypt does not support content filter with seek after decrypt"
        ))
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
    AlteredPinkCrypt,
    NatsupochiCrypt,
    PoringSoftCrypt,
    AppliqueCrypt,
    TokidokiCrypt,
    SourireCrypt,
    HibikiCrypt,
    #[serde(rename_all = "PascalCase")]
    AkabeiCrypt {
        seed: u32,
    },
    HaikuoCrypt,
    #[serde(rename_all = "PascalCase")]
    StripeCrypt {
        key: u8,
    },
    ExaCrypt,
    #[serde(rename_all = "PascalCase")]
    SmileCrypt {
        key_xor: u32,
        first_xor: u8,
        zero_xor: u8,
    },
    YuzuCrypt,
    HighRunningCrypt,
    KissCrypt,
    #[serde(rename_all = "PascalCase")]
    PuCaCrypt {
        hash_table: Vec<u32>,
        key_table: Base64Bytes,
    },
    #[serde(rename_all = "PascalCase")]
    RhapsodyCrypt {
        file_list_name: String,
    },
    #[serde(rename_all = "PascalCase")]
    MadoCrypt {
        seed: u32,
    },
    #[serde(rename_all = "PascalCase")]
    SmxCrypt {
        mask: u32,
        key_seq: Base64Bytes,
    },
    FestivalCrypt,
    PinPointCrypt,
    HybridCrypt,
    #[serde(rename_all = "PascalCase")]
    NekoWorksCrypt {
        key: Base64Bytes,
    },
    #[serde(rename_all = "PascalCase")]
    NinkiSeiyuuCrypt {
        key1: u64,
        key2: u64,
        key3: u64,
    },
    SyangrilaSmartCrypt,
    Kano2Crypt,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
#[allow(dead_code)]
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
    pub fn create_crypt(
        &self,
        filename: &str,
        config: &ExtraConfig,
    ) -> Result<Box<dyn Crypt + Send + Sync>> {
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
            CryptType::AlteredPinkCrypt => Box::new(AlteredPinkCrypt::new(self.base.clone())),
            CryptType::NatsupochiCrypt => Box::new(NatsupochiCrypt::new(self.base.clone())),
            CryptType::PoringSoftCrypt => Box::new(PoringSoftCrypt::new(self.base.clone())),
            CryptType::AppliqueCrypt => Box::new(AppliqueCrypt::new(self.base.clone())),
            CryptType::TokidokiCrypt => Box::new(TokidokiCrypt::new(self.base.clone())),
            CryptType::SourireCrypt => Box::new(SourireCrypt::new(self.base.clone())),
            CryptType::HibikiCrypt => Box::new(HibikiCrypt::new(self.base.clone())),
            CryptType::AkabeiCrypt { seed } => Box::new(AkabeiCrypt::new(self.base.clone(), *seed)),
            CryptType::HaikuoCrypt => Box::new(HaikuoCrypt::new(self.base.clone())),
            CryptType::StripeCrypt { key } => Box::new(StripeCrypt::new(self.base.clone(), *key)),
            CryptType::ExaCrypt => Box::new(ExaCrypt::new(self.base.clone())),
            CryptType::SmileCrypt {
                key_xor,
                first_xor,
                zero_xor,
            } => Box::new(SmileCrypt::new(
                self.base.clone(),
                *key_xor,
                *first_xor,
                *zero_xor,
            )),
            CryptType::YuzuCrypt => Box::new(YuzuCrypt::new(self.base.clone())),
            CryptType::HighRunningCrypt => Box::new(HighRunningCrypt::new(self.base.clone())),
            CryptType::KissCrypt => Box::new(cz::KissCrypt::new(self.base.clone())),
            CryptType::PuCaCrypt {
                hash_table,
                key_table,
            } => Box::new(PuCaCrypt::new(
                self.base.clone(),
                hash_table.clone(),
                key_table.bytes.clone(),
            )?),
            CryptType::RhapsodyCrypt { file_list_name } => Box::new(RhapsodyCrypt::new(
                self.base.clone(),
                &file_list_name,
                config.xp3_file_list_path.as_ref().map(|s| s.as_str()),
            )?),
            CryptType::MadoCrypt { seed } => Box::new(MadoCrypt::new(self.base.clone(), *seed)),
            CryptType::SmxCrypt { mask, key_seq } => {
                Box::new(SmxCrypt::new(self.base.clone(), *mask, &key_seq.bytes)?)
            }
            CryptType::FestivalCrypt => Box::new(FestivalCrypt::new(self.base.clone())),
            CryptType::PinPointCrypt => Box::new(PinPointCrypt::new(self.base.clone())),
            CryptType::HybridCrypt => Box::new(HybridCrypt::new(self.base.clone())),
            CryptType::NekoWorksCrypt { key } => {
                Box::new(NekoWorksCrypt::new(self.base.clone(), key.bytes.clone())?)
            }
            CryptType::NinkiSeiyuuCrypt { key1, key2, key3 } => Box::new(NinkiSeiyuuCrypt::new(
                self.base.clone(),
                *key1,
                *key2,
                *key3,
            )),
            CryptType::SyangrilaSmartCrypt => Box::new(SyangrilaSmartCrypt::new(self.base.clone())),
            CryptType::Kano2Crypt => Box::new(Kano2Crypt::new(self.base.clone())),
        })
    }
}

lazy_static::lazy_static! {
    static ref CRYPT_SCHEMA: BTreeMap<CaseInsensitiveString, Schema> = {
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

pub fn query_filename_list(name: &str) -> Result<String> {
    let reader = MemReaderRef::new(NAME_LIST_DATA);
    let mut pack = read_simple_pack(reader)?;
    while let Some(mut entry) = pack.next()? {
        if entry.name == name {
            let mut str = String::new();
            entry.read_to_string(&mut str)?;
            return Ok(str);
        }
    }
    Err(anyhow::anyhow!("Name list entry not found: {}", name))
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
    CRYPT_SCHEMA.get(CIS::from_str(game)).or_else(|| {
        ALIAS_TABLE
            .get(game)
            .and_then(|real_game| CRYPT_SCHEMA.get(CIS::from_str(real_game)))
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
            #[allow(unused)]
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

macro_rules! base_schema2_impl {
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
                stream: Box<dyn Read + Send + Sync + 'a>,
            ) -> Result<Box<dyn ReadDebug + Send + Sync + 'a>> {
                Ok(Box::new($reader::new(stream, cur_seg)))
            }
            fn decrypt_with_seek<'a>(
                &self,
                _entry: &Xp3Entry,
                cur_seg: &Segment,
                stream: Box<dyn ReadSeek + Send + Sync + 'a>,
            ) -> Result<Box<dyn ReadSeek + Send + Sync + 'a>> {
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
                stream: Box<dyn Read + Send + Sync + 'a>,
            ) -> Result<Box<dyn ReadDebug + Send + Sync + 'a>> {
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
                stream: Box<dyn ReadSeek + Send + Sync + 'a>,
            ) -> Result<Box<dyn ReadSeek + Send + Sync + 'a>> {
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

macro_rules! seek_crypt_key_base_impl {
    ($crypt:ident, $reader:ident, $typ:ty) => {
        #[derive(Debug)]
        pub struct $crypt {
            base: BaseSchema,
            key: $typ,
        }
        impl $crypt {
            pub fn new(base: BaseSchema, key: $typ) -> Self {
                Self { base, key }
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
                stream: Box<dyn Read + Send + Sync + 'a>,
            ) -> Result<Box<dyn ReadDebug + Send + Sync + 'a>> {
                Ok(Box::new($reader::new(stream, cur_seg, self.key)))
            }
            fn decrypt_with_seek<'a>(
                &self,
                _entry: &Xp3Entry,
                cur_seg: &Segment,
                stream: Box<dyn ReadSeek + Send + Sync + 'a>,
            ) -> Result<Box<dyn ReadSeek + Send + Sync + 'a>> {
                Ok(Box::new($reader::new(stream, cur_seg, self.key)))
            }
        }
    };
}

macro_rules! seek_crypt_key_impl {
    ($crypt:ident, $reader:ident<$t:ident>, $typ:ty) => {
        seek_crypt_key_base_impl!($crypt, $reader, $typ);
        seek_reader_key_impl!($reader<$t>, $typ);
    };
}

seek_crypt_key_impl!(XorCrypt, XorCryptReader<T>, u8);

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
        stream: Box<dyn Read + Send + Sync + 'a>,
    ) -> Result<Box<dyn ReadDebug + Send + Sync + 'a>> {
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
        stream: Box<dyn ReadSeek + Send + Sync + 'a>,
    ) -> Result<Box<dyn ReadSeek + Send + Sync + 'a>> {
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
                stream: Box<dyn Read + Send + Sync + 'a>,
            ) -> Result<Box<dyn ReadDebug + Send + Sync + 'a>> {
                Ok(Box::new($reader::new(stream, cur_seg, entry.file_hash)))
            }
            fn decrypt_with_seek<'a>(
                &self,
                entry: &Xp3Entry,
                cur_seg: &Segment,
                stream: Box<dyn ReadSeek + Send + Sync + 'a>,
            ) -> Result<Box<dyn ReadSeek + Send + Sync + 'a>> {
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

seek_crypt_impl!(AlteredPinkCrypt, AlteredPinkCryptReader<T>);

impl<R: Read> Read for AlteredPinkCryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.inner.read(buf)?;
        for (i, t) in (&mut buf[..readed]).iter_mut().enumerate() {
            let offset = self.seg_start + self.pos + i as u64;
            *t ^= ALTERED_PINK_KEY_TABLE[(offset & 0xFF) as usize];
        }
        self.pos += readed as u64;
        Ok(readed)
    }
}

seek_crypt_filehash_key_impl!(NatsupochiCrypt, NatsupochiCryptReader<T>);

impl<R: Read> Read for NatsupochiCryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.inner.read(buf)?;
        let key = (self.key >> 3) as u8;
        for t in (&mut buf[..readed]).iter_mut() {
            *t ^= key;
        }
        self.pos += readed as u64;
        Ok(readed)
    }
}

seek_crypt_filehash_key_impl!(PoringSoftCrypt, PoringSoftCryptReader<T>);

impl<R: Read> Read for PoringSoftCryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.inner.read(buf)?;
        let key = (!w!(self.key + 1)) as u8;
        for t in (&mut buf[..readed]).iter_mut() {
            *t ^= key;
        }
        self.pos += readed as u64;
        Ok(readed)
    }
}

seek_crypt_filehash_key_impl!(AppliqueCrypt, AppliqueCryptReader<T>);

impl<R: Read> Read for AppliqueCryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.inner.read(buf)?;
        let key = (self.key >> 12) as u8;
        let skip = (5 - (self.seg_start + self.pos).min(5) as usize).min(readed);
        for t in (&mut buf[skip..readed]).iter_mut() {
            *t ^= key;
        }
        self.pos += readed as u64;
        Ok(readed)
    }
}

#[derive(Debug)]
pub struct TokidokiCrypt {
    base: BaseSchema,
}

impl TokidokiCrypt {
    pub fn new(base: BaseSchema) -> Self {
        Self { base }
    }

    /// Retruns limit and key
    fn get_key(&self, entry: &Xp3Entry) -> Result<(u64, u32)> {
        let ext = entry
            .name
            .rsplit('.')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        if !ext.is_empty() {
            let ext = format!(".{}", ext);
            let mut ext_bin = encode_string(Encoding::Cp932, &ext, true)?;
            ext_bin.resize(4, 0);
            let mut reader = MemReaderRef::new(&ext_bin);
            let key = !reader.read_u32()?;
            if ext == ".asd" || ext == ".ks" || ext == ".tjs" {
                Ok((entry.original_size, key))
            } else {
                Ok((entry.original_size.min(0x100), key))
            }
        } else {
            Ok((entry.original_size.min(0x100), u32::MAX))
        }
    }
}

impl Crypt for TokidokiCrypt {
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
        stream: Box<dyn Read + Send + Sync + 'a>,
    ) -> Result<Box<dyn ReadDebug + Send + Sync + 'a>> {
        Ok(Box::new(TokidokiCryptReader::new(
            stream,
            cur_seg,
            self.get_key(entry)?,
        )))
    }
    fn decrypt_with_seek<'a>(
        &self,
        entry: &Xp3Entry,
        cur_seg: &Segment,
        stream: Box<dyn ReadSeek + Send + Sync + 'a>,
    ) -> Result<Box<dyn ReadSeek + Send + Sync + 'a>> {
        Ok(Box::new(TokidokiCryptReader::new(
            stream,
            cur_seg,
            self.get_key(entry)?,
        )))
    }
}

seek_reader_key_impl!(TokidokiCryptReader<T>, (u64, u32));

impl<R: Read> Read for TokidokiCryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let (limit, key) = self.key;
        let readed = self.inner.read(buf)?;
        for (i, t) in (&mut buf[..readed]).iter_mut().enumerate() {
            let offset = self.seg_start + self.pos + i as u64;
            if offset < limit {
                *t ^= (key >> ((offset as i32 & 3) << 3)) as u8;
            }
        }
        self.pos += readed as u64;
        Ok(readed)
    }
}

seek_crypt_filehash_key_impl!(SourireCrypt, SourireCryptReader<T>);

impl<R: Read> Read for SourireCryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.inner.read(buf)?;
        let key = (self.key ^ 0xCD) as u8;
        for t in (&mut buf[..readed]).iter_mut() {
            *t ^= key;
        }
        self.pos += readed as u64;
        Ok(readed)
    }
}

seek_crypt_filehash_key_impl!(HibikiCrypt, HibikiCryptReader<T>);

impl<R: Read> Read for HibikiCryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.inner.read(buf)?;
        let key1 = (self.key >> 5) as u8;
        let key2 = (self.key >> 8) as u8;
        for (i, t) in (&mut buf[..readed]).iter_mut().enumerate() {
            let offset = self.seg_start + self.pos + i as u64;
            let key = if offset <= 0x64 || offset & 4 != 0 {
                key1
            } else {
                key2
            };
            *t ^= key;
        }
        self.pos += readed as u64;
        Ok(readed)
    }
}

#[derive(Debug)]
pub struct AkabeiCrypt {
    base: BaseSchema,
    seed: u32,
}

impl AkabeiCrypt {
    pub fn new(base: BaseSchema, seed: u32) -> Self {
        Self { base, seed }
    }

    fn get_key(&self, mut hash: u32) -> [u8; 0x20] {
        let mut state = [0; 0x20];
        hash = (hash ^ self.seed) & 0x7FFFFFFF;
        hash = hash << 31 | hash;
        for i in 0..0x20 {
            state[i] = (hash & 0xFF) as u8;
            hash = (hash & 0xFFFFFFFE) << 23 | hash >> 8;
        }
        state
    }
}

impl Crypt for AkabeiCrypt {
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
        stream: Box<dyn Read + Send + Sync + 'a>,
    ) -> Result<Box<dyn ReadDebug + Send + Sync + 'a>> {
        Ok(Box::new(AkabeiCryptReader::new(
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
        Ok(Box::new(AkabeiCryptReader::new(
            stream,
            cur_seg,
            self.get_key(entry.file_hash),
        )))
    }
}

seek_reader_key_impl!(AkabeiCryptReader<T>, [u8; 0x20]);

impl<R: Read> Read for AkabeiCryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.inner.read(buf)?;
        for (i, t) in (&mut buf[..readed]).iter_mut().enumerate() {
            let offset = self.seg_start + self.pos + i as u64;
            *t ^= self.key[(offset & 0x1F) as usize];
        }
        self.pos += readed as u64;
        Ok(readed)
    }
}

seek_crypt_filehash_key_impl!(HaikuoCrypt, HaikuoCryptReader<T>);

impl<R: Read> Read for HaikuoCryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.inner.read(buf)?;
        let key = (self.key ^ (self.key >> 8)) as u8;
        for t in (&mut buf[..readed]).iter_mut() {
            *t ^= key;
        }
        self.pos += readed as u64;
        Ok(readed)
    }
}

seek_crypt_key_impl!(StripeCrypt, StripeCryptReader<T>, u8);

impl<R: Read> Read for StripeCryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.inner.read(buf)?;
        for t in (&mut buf[..readed]).iter_mut() {
            *t ^= self.key;
            w!(*t += 1);
        }
        self.pos += readed as u64;
        Ok(readed)
    }
}

seek_crypt_filehash_key_impl!(ExaCrypt, ExaCryptReader<T>);

impl<R: Read> Read for ExaCryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.inner.read(buf)?;
        let mut shift = ((self.seg_start + self.pos) % 5) as u32;
        for t in (&mut buf[..readed]).iter_mut() {
            *t ^= (self.key >> shift) as u8;
            shift = (shift + 1) % 5;
        }
        self.pos += readed as u64;
        Ok(readed)
    }
}

#[derive(Debug)]
pub struct SmileCrypt {
    base: BaseSchema,
    key_xor: u32,
    first_xor: u8,
    zero_xor: u8,
}

impl SmileCrypt {
    pub fn new(base: BaseSchema, key_xor: u32, first_xor: u8, zero_xor: u8) -> Self {
        Self {
            base,
            key_xor,
            first_xor,
            zero_xor,
        }
    }
}

impl Crypt for SmileCrypt {
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
        stream: Box<dyn Read + Send + Sync + 'a>,
    ) -> Result<Box<dyn ReadDebug + Send + Sync + 'a>> {
        let key = entry.file_hash ^ self.key_xor;
        Ok(Box::new(SmileCryptReader::new(
            stream,
            cur_seg,
            (key, self.first_xor, self.zero_xor),
        )))
    }
    fn decrypt_with_seek<'a>(
        &self,
        entry: &Xp3Entry,
        cur_seg: &Segment,
        stream: Box<dyn ReadSeek + Send + Sync + 'a>,
    ) -> Result<Box<dyn ReadSeek + Send + Sync + 'a>> {
        let key = entry.file_hash ^ self.key_xor;
        Ok(Box::new(SmileCryptReader::new(
            stream,
            cur_seg,
            (key, self.first_xor, self.zero_xor),
        )))
    }
}

seek_reader_key_impl!(SmileCryptReader<T>, (u32, u8, u8));

impl<R: Read> Read for SmileCryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.inner.read(buf)?;
        let (mut hash, first_xor, zero_xor) = self.key;
        let mut key = (hash ^ (hash >> 8) ^ (hash >> 16) ^ (hash >> 24)) as u8;
        if key == 0 {
            key = zero_xor;
        }
        if self.pos == 0 && self.seg_start == 0 && readed > 0 {
            if hash & 0xFF == 0 {
                hash = first_xor as u32;
            }
            buf[0] ^= hash as u8;
        }
        for t in (&mut buf[..readed]).iter_mut() {
            *t ^= key;
        }
        self.pos += readed as u64;
        Ok(readed)
    }
}

seek_crypt_filehash_key_impl!(YuzuCrypt, YuzuCryptReader<T>);

impl<R: Read> Read for YuzuCryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.inner.read(buf)?;
        let hash = self.key ^ 0x1DDB6E7A;
        let mut key = (hash ^ (hash >> 8) ^ (hash >> 16) ^ (hash >> 24)) as u8;
        if key == 0 {
            key = 0xD0;
        }
        for t in (&mut buf[..readed]).iter_mut() {
            *t ^= key;
        }
        self.pos += readed as u64;
        Ok(readed)
    }
}

seek_crypt_filehash_key_u8_impl!(HighRunningCrypt, HighRunningCryptReader<T>);

impl<R: Read> Read for HighRunningCryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.inner.read(buf)?;
        let key = self.key as u64;
        if key != 0 {
            for (i, t) in (&mut buf[..readed]).iter_mut().enumerate() {
                let offset = self.seg_start + self.pos + i as u64;
                if offset % key != 0 {
                    *t ^= self.key;
                }
            }
        }
        self.pos += readed as u64;
        Ok(readed)
    }
}

seek_reader_key_impl!(KissCryptReader<T>, u32);

#[derive(Debug)]
pub struct PuCaCrypt {
    base: BaseSchema,
    hash_table: Vec<u32>,
    key_table: Vec<u8>,
}

impl PuCaCrypt {
    pub fn new(base: BaseSchema, hash_table: Vec<u32>, key_table: Vec<u8>) -> Result<Self> {
        if hash_table.len() != key_table.len() {
            anyhow::bail!(
                "Hash table and key table must have the same length, but got {} and {}",
                hash_table.len(),
                key_table.len()
            );
        }
        Ok(Self {
            base,
            hash_table,
            key_table,
        })
    }
    fn get_key_table(&self, file_hash: u32) -> [u8; 0x400] {
        let mut hash_table = [0u8; 32];
        let mut hash = file_hash;
        for k in (0..32).step_by(4) {
            if hash & 1 != 0 {
                hash |= 0x80000000;
            } else {
                hash &= 0x7FFFFFFF;
            }
            hash_table[k..k + 4].copy_from_slice(&hash.to_le_bytes());
            hash >>= 1;
        }
        let mut key_table = [0u8; 0x400];
        for l in 0..32 {
            for m in 0..32 {
                key_table[l * 32 + m] = (!hash_table[l]) ^ hash_table[m];
            }
        }
        key_table
    }
}

impl Crypt for PuCaCrypt {
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
        stream: Box<dyn Read + Send + Sync + 'a>,
    ) -> Result<Box<dyn ReadDebug + Send + Sync + 'a>> {
        if let Some(pos) = self.hash_table.iter().position(|&h| h == entry.file_hash) {
            Ok(Box::new(PuCaCryptReader::new(
                stream,
                cur_seg,
                self.key_table[pos],
            )))
        } else {
            Ok(Box::new(PuCaCryptReader2::new(
                stream,
                cur_seg,
                self.get_key_table(entry.file_hash),
            )))
        }
    }
    fn decrypt_with_seek<'a>(
        &self,
        entry: &Xp3Entry,
        cur_seg: &Segment,
        stream: Box<dyn ReadSeek + Send + Sync + 'a>,
    ) -> Result<Box<dyn ReadSeek + Send + Sync + 'a>> {
        if let Some(pos) = self.hash_table.iter().position(|&h| h == entry.file_hash) {
            Ok(Box::new(PuCaCryptReader::new(
                stream,
                cur_seg,
                self.key_table[pos],
            )))
        } else {
            Ok(Box::new(PuCaCryptReader2::new(
                stream,
                cur_seg,
                self.get_key_table(entry.file_hash),
            )))
        }
    }
}

seek_reader_key_impl!(PuCaCryptReader<T>, u8);

impl<T: Read> Read for PuCaCryptReader<T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.inner.read(buf)?;
        for t in (&mut buf[..readed]).iter_mut() {
            *t ^= self.key;
        }
        self.pos += readed as u64;
        Ok(readed)
    }
}

seek_reader_key_impl!(PuCaCryptReader2<T>, [u8; 0x400]);

impl<R: Read> Read for PuCaCryptReader2<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.inner.read(buf)?;
        let mut offset = ((self.seg_start + self.pos) & 0x3FF) as usize;
        for t in (&mut buf[..readed]).iter_mut() {
            *t ^= self.key[offset];
            offset = (offset + 1) & 0x3FF;
        }
        self.pos += readed as u64;
        Ok(readed)
    }
}

#[derive(Debug)]
pub struct RhapsodyCrypt {
    base: BaseSchema,
    names: HashMap<u32, String>,
}

impl RhapsodyCrypt {
    pub fn new(
        base: BaseSchema,
        file_list_name: &str,
        file_list_path: Option<&str>,
    ) -> Result<Self> {
        let file_list = if let Some(path) = file_list_path {
            std::fs::read_to_string(path)?
        } else {
            query_filename_list(file_list_name)?
        };
        let mut names = HashMap::new();
        for name in file_list.lines() {
            let name = name.trim();
            if !name.is_empty() {
                names.insert(Self::get_name_hash(name.chars()), name.to_string());
            }
        }
        Ok(Self { base, names })
    }
    fn get_name_hash<T: Iterator<Item = char>>(name: T) -> u32 {
        let mut hash = 0;
        for c in name {
            hash = Self::update_name_hash(hash, c);
        }
        hash
    }
    const fn update_name_hash(hash: u32, c: char) -> u32 {
        let c = c.to_ascii_lowercase() as u32;
        let mut hash = w!(0x1000193u32 * hash ^ (c & 0xFF));
        hash = w!(0x1000193u32 * hash ^ ((c >> 8) & 0xFF));
        hash
    }
    fn get_key(&self, hash: u32) -> [u8; 12] {
        let mut key = [0u8; 12];
        key[0..4].copy_from_slice(&hash.to_le_bytes());
        key[4..8].copy_from_slice(&(0x6E1DA9B2u32).to_le_bytes());
        key[8..12].copy_from_slice(&(0x0040C800u32).to_le_bytes());
        key
    }
}

impl Crypt for RhapsodyCrypt {
    base_schema_impl!();
    fn read_name<'a>(&self, reader: &mut Box<dyn Read + 'a>) -> Result<(String, u64)> {
        use msg_tool_macro::rhapsody_crypt_const_name_hash as hash;
        const PNG_HASH: u32 = hash!(".png");
        const MAP_HASH: u32 = hash!(".map");
        const ASD_HASH: u32 = hash!(".asd");
        const TJS_HASH: u32 = hash!(".tjs");
        const TXT_HASH: u32 = hash!(".txt");
        const KS_HASH: u32 = hash!(".ks");
        const WAV_HASH: u32 = hash!(".wav");
        const JPG_HASH: u32 = hash!(".jpg");
        const OGG_HASH: u32 = hash!(".ogg");
        let key = reader.read_u32()?;
        let name_hash = reader.read_u32()? ^ key;
        if let Some(name) = self.names.get(&name_hash) {
            return Ok((name.clone(), 8));
        }
        let ext_hash = reader.read_u32()? ^ key;
        let mut name = format!("{:08X}", name_hash);
        match ext_hash {
            PNG_HASH => name += ".png",
            MAP_HASH => name += ".map",
            ASD_HASH => name += ".asd",
            TJS_HASH => name += ".tjs",
            TXT_HASH => name += ".txt",
            KS_HASH => name += ".ks",
            WAV_HASH => name += ".wav",
            JPG_HASH => name += ".jpg",
            OGG_HASH => name += ".ogg",
            _ => name += format!(".{:08X}", ext_hash).as_str(),
        };
        Ok((name, 12))
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
        Ok(Box::new(RhapsodyCryptReader::new(
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
        Ok(Box::new(RhapsodyCryptReader::new(
            stream,
            cur_seg,
            self.get_key(entry.file_hash),
        )))
    }
}

seek_reader_key_impl!(RhapsodyCryptReader<T>, [u8; 12]);

impl<R: Read> Read for RhapsodyCryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.inner.read(buf)?;
        let mut offset = ((self.seg_start + self.pos) % 12) as usize;
        for t in (&mut buf[..readed]).iter_mut() {
            *t ^= self.key[offset];
            offset = (offset + 1) % 12;
        }
        self.pos += readed as u64;
        Ok(readed)
    }
}

#[derive(Debug)]
pub struct MadoCrypt {
    base: AkabeiCrypt,
}

impl MadoCrypt {
    pub fn new(base: BaseSchema, seed: u32) -> Self {
        Self {
            base: AkabeiCrypt::new(base, seed),
        }
    }
}

impl AsRef<BaseSchema> for MadoCrypt {
    fn as_ref(&self) -> &BaseSchema {
        &self.base.base
    }
}

impl Crypt for MadoCrypt {
    base_schema2_impl!();
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
        Ok(Box::new(MadoCryptReader::new(
            stream,
            cur_seg,
            self.base.get_key(entry.file_hash),
        )))
    }
    fn decrypt_with_seek<'a>(
        &self,
        entry: &Xp3Entry,
        cur_seg: &Segment,
        stream: Box<dyn ReadSeek + Send + Sync + 'a>,
    ) -> Result<Box<dyn ReadSeek + Send + Sync + 'a>> {
        Ok(Box::new(MadoCryptReader::new(
            stream,
            cur_seg,
            self.base.get_key(entry.file_hash),
        )))
    }
}

seek_reader_key_impl!(MadoCryptReader<T>, [u8; 0x20]);

impl<R: Read> Read for MadoCryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.inner.read(buf)?;
        for (i, t) in (&mut buf[..readed]).iter_mut().enumerate() {
            let offset = self.seg_start + self.pos + i as u64;
            *t ^= self.key[(offset % 0x1F) as usize];
        }
        self.pos += readed as u64;
        Ok(readed)
    }
}

#[derive(Debug)]
pub struct SmxCrypt {
    base: BaseSchema,
    mask: u32,
    key_seq: Vec<u8>,
}

impl SmxCrypt {
    pub fn new(base: BaseSchema, mask: u32, key_seq: &[u8]) -> Result<Self> {
        if key_seq.len() <= mask as usize + 1 {
            anyhow::bail!(
                "Key sequence length must be greater than mask + 1, but got {} and {}",
                key_seq.len(),
                mask
            );
        }
        if key_seq.len() < 2 {
            anyhow::bail!(
                "Key sequence length must be at least 2, but got {}",
                key_seq.len()
            );
        }
        Ok(Self {
            base,
            mask,
            key_seq: key_seq.to_vec(),
        })
    }

    fn generate_key(&self, file_hash: u32) -> Vec<u8> {
        let mut key = vec![0u8; self.key_seq.len() - 1];
        for i in 1..self.key_seq.len() {
            key[i - 1] = (file_hash >> self.key_seq[i]) as u8;
        }
        key
    }
}

impl Crypt for SmxCrypt {
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
        stream: Box<dyn Read + Send + Sync + 'a>,
    ) -> Result<Box<dyn ReadDebug + Send + Sync + 'a>> {
        let start_key = (entry.file_hash >> self.key_seq[0]) as u8;
        Ok(Box::new(SmxCryptReader::new(
            stream,
            cur_seg,
            (self.mask, start_key, self.generate_key(entry.file_hash)),
        )))
    }
    fn decrypt_with_seek<'a>(
        &self,
        entry: &Xp3Entry,
        cur_seg: &Segment,
        stream: Box<dyn ReadSeek + Send + Sync + 'a>,
    ) -> Result<Box<dyn ReadSeek + Send + Sync + 'a>> {
        let start_key = (entry.file_hash >> self.key_seq[0]) as u8;
        Ok(Box::new(SmxCryptReader::new(
            stream,
            cur_seg,
            (self.mask, start_key, self.generate_key(entry.file_hash)),
        )))
    }
}

seek_reader_key_impl!(SmxCryptReader<T>, (u32, u8, Vec<u8>));

impl<R: Read> Read for SmxCryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.inner.read(buf)?;
        let (mask, start_key, key) = &self.key;
        let mask = *mask as u64;
        let mut offset = self.seg_start + self.pos;
        for t in (&mut buf[..readed]).iter_mut() {
            let key = if offset <= 100 {
                *start_key
            } else {
                key[(offset & mask) as usize]
            };
            *t ^= key;
            offset += 1;
        }
        self.pos += readed as u64;
        Ok(readed)
    }
}

seek_crypt_filehash_key_impl!(FestivalCrypt, FestivalCryptReader<T>);

impl<R: Read> Read for FestivalCryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.inner.read(buf)?;
        let key = (!(self.key >> 7)) as u8;
        for t in (&mut buf[..readed]).iter_mut() {
            *t ^= key;
        }
        self.pos += readed as u64;
        Ok(readed)
    }
}

seek_crypt_impl!(PinPointCrypt, PinPointCryptReader<T>);

impl<R: Read> PinPointCryptReader<R> {
    #[inline(always)]
    fn count_set_bits(x: u8) -> u32 {
        let mut bit_count = ((x & 0x55) + ((x >> 1) & 0x55)) as u32;
        bit_count = (bit_count & 0x33) + ((bit_count >> 2) & 0x33);
        ((bit_count & 0xF) + ((bit_count >> 4) & 0xF)) & 0xF
    }
}

impl<R: Read> Read for PinPointCryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.inner.read(buf)?;
        for t in (&mut buf[..readed]).iter_mut() {
            let bit_count = Self::count_set_bits(*t);
            if bit_count > 0 {
                *t = (*t).rotate_left(bit_count);
            }
        }
        self.pos += readed as u64;
        Ok(readed)
    }
}

seek_crypt_filehash_key_impl!(HybridCrypt, HybridCryptReader<T>);

impl<R: Read> Read for HybridCryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.inner.read(buf)?;
        let key = (self.key >> 5) as u8;
        for t in (&mut buf[..readed]).iter_mut() {
            *t ^= key;
        }
        self.pos += readed as u64;
        Ok(readed)
    }
}

#[derive(Debug)]
pub struct NekoWorksCrypt {
    base: BaseSchema,
    key: Vec<u8>,
}

impl NekoWorksCrypt {
    pub fn new(base: BaseSchema, key: Vec<u8>) -> Result<Self> {
        if key.len() < 31 {
            anyhow::bail!("NekoWorksCrypt: key is too small.");
        }
        Ok(Self { base, key })
    }

    fn init_key(&self, mut hash: u32) -> [u8; 31] {
        hash &= 0x7FFFFFFF;
        hash = hash << 31 | hash;
        let mut key = [0; 31];
        key.copy_from_slice(&self.key[..31]);
        for i in 0..31 {
            key[i] ^= hash as u8;
            hash = (hash & 0xFFFFFFFE) << 23 | hash >> 8;
        }
        key
    }
}

impl Crypt for NekoWorksCrypt {
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
        stream: Box<dyn Read + Send + Sync + 'a>,
    ) -> Result<Box<dyn ReadDebug + Send + Sync + 'a>> {
        Ok(Box::new(NekoWorksCryptReader::new(
            stream,
            cur_seg,
            self.init_key(entry.file_hash),
        )))
    }
    fn decrypt_with_seek<'a>(
        &self,
        entry: &Xp3Entry,
        cur_seg: &Segment,
        stream: Box<dyn ReadSeek + Send + Sync + 'a>,
    ) -> Result<Box<dyn ReadSeek + Send + Sync + 'a>> {
        Ok(Box::new(NekoWorksCryptReader::new(
            stream,
            cur_seg,
            self.init_key(entry.file_hash),
        )))
    }
}

seek_reader_key_impl!(NekoWorksCryptReader<T>, [u8; 31]);

impl<R: Read> Read for NekoWorksCryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.inner.read(buf)?;
        let mut offset = ((self.seg_start + self.pos) % 31) as usize;
        for t in (&mut buf[..readed]).iter_mut() {
            *t ^= self.key[offset];
            offset = (offset + 1) % 31;
        }
        self.pos += readed as u64;
        Ok(readed)
    }
}

#[derive(Debug)]
pub struct NinkiSeiyuuCrypt {
    base: BaseSchema,
    tbl2: [u8; 64],
    tbl3: [u8; 64],
}

impl NinkiSeiyuuCrypt {
    pub fn new(base: BaseSchema, key1: u64, key2: u64, key3: u64) -> Self {
        let tbl2 = Self::get_table2(3080);
        let tbl3 = Self::get_table3(3080, key1, key2, key3);
        Self { base, tbl2, tbl3 }
    }
    fn get_table1(seed: u32) -> [u8; 32] {
        let mut key = [0; 32];
        let mut v48 = seed & 0x7FFFFFFF;
        for i in 0..31 {
            key[i] = v48 as u8;
            v48 = (v48 >> 8) | (((v48 as u8) as u32) << 23);
        }
        key
    }
    fn get_table2(seed: u32) -> [u8; 64] {
        let mut key = [0; 64];
        let v51 = seed & 0xFFF;
        let v52 = ((v51 | (v51 << 13)) as u64) | (((v51 >> 19) as u64) << 32);
        let mut v53 = v51 | ((v52 as u32) << 13);
        let mut v54 = (((((v52 as u32) << 7) & 0x1FFFFFFF) as u64) | (v52 >> 19)) as u32;
        for i in 0..61 {
            let v56 = v53 as u8;
            key[i] = v56;
            v53 = ((((v54 as u64) << 32) | (v53 as u64)) >> 8) as u32;
            v54 = (v54 >> 8) | ((v56 as u32) << 21);
        }
        key
    }
    fn get_table3(seed: u32, key1: u64, key2: u64, key3: u64) -> [u8; 64] {
        let mut key = [0; 64];
        let v88 = seed & 0xFFF;
        let v89 = ((v88 | (v88 << 13)) as u64) | (((v88 >> 19) as u64) << 32);
        let mut v90 = ((key1 + key2) ^ ((v88 | ((v89 as u32) << 13)) as u64)) as u32;
        let mut v91 =
            ((((key1 + ((key3 & 0xFFFFFFFF00000000) | (key2 & 0xFFFFFFFF))) >> 32) & 0x1FFFFFFF)
                ^ (((((v89 as u32) << 7) & 0x1FFFFFFF) as u64) | (v89 >> 19))) as u32;
        for i in 0..61 {
            let v93 = v90 as u8;
            key[i] = v93;
            v90 = ((((v91 as u64) << 32) | (v90 as u64)) >> 8) as u32;
            v91 = (v91 >> 8) | ((v93 as u32) << 21);
        }
        key
    }
}

impl Crypt for NinkiSeiyuuCrypt {
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
        stream: Box<dyn Read + Send + Sync + 'a>,
    ) -> Result<Box<dyn ReadDebug + Send + Sync + 'a>> {
        let key = (
            Self::get_table1(entry.file_hash),
            self.tbl2.clone(),
            self.tbl3.clone(),
        );
        Ok(Box::new(NinkiSeiyuuCryptReader::new(stream, cur_seg, key)))
    }
    fn decrypt_with_seek<'a>(
        &self,
        entry: &Xp3Entry,
        cur_seg: &Segment,
        stream: Box<dyn ReadSeek + Send + Sync + 'a>,
    ) -> Result<Box<dyn ReadSeek + Send + Sync + 'a>> {
        let key = (
            Self::get_table1(entry.file_hash),
            self.tbl2.clone(),
            self.tbl3.clone(),
        );
        Ok(Box::new(NinkiSeiyuuCryptReader::new(stream, cur_seg, key)))
    }
}

seek_reader_key_impl!(NinkiSeiyuuCryptReader<T>, ([u8; 32], [u8; 64], [u8; 64]));

impl<R: Read> Read for NinkiSeiyuuCryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.inner.read(buf)?;
        let mut offset1 = ((self.seg_start + self.pos) % 0x1F) as usize;
        let mut offset2 = ((self.seg_start + self.pos) % 0x3D) as usize;
        for t in (&mut buf[..readed]).iter_mut() {
            *t ^= self.key.0[offset1];
            *t = (*t).wrapping_add(self.key.1[offset2] ^ self.key.2[offset2]);
            offset1 = (offset1 + 1) % 0x1F;
            offset2 = (offset2 + 1) % 0x3D;
        }
        self.pos += readed as u64;
        Ok(readed)
    }
}

#[derive(Debug)]
pub struct SyangrilaSmartCrypt {
    base: BaseSchema,
}

impl SyangrilaSmartCrypt {
    pub fn new(base: BaseSchema) -> Self {
        Self { base }
    }

    fn get_key(hash: u32) -> [u8; 5] {
        [
            (hash >> 5) as u8,
            (hash >> 5) as u8,
            (hash >> 7) as u8,
            (hash >> 1) as u8,
            (hash >> 4) as u8,
        ]
    }
}

impl Crypt for SyangrilaSmartCrypt {
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
        stream: Box<dyn Read + Send + Sync + 'a>,
    ) -> Result<Box<dyn ReadDebug + Send + Sync + 'a>> {
        Ok(Box::new(SyangrilaSmartCryptReader::new(
            stream,
            cur_seg,
            Self::get_key(entry.file_hash),
        )))
    }
    fn decrypt_with_seek<'a>(
        &self,
        entry: &Xp3Entry,
        cur_seg: &Segment,
        stream: Box<dyn ReadSeek + Send + Sync + 'a>,
    ) -> Result<Box<dyn ReadSeek + Send + Sync + 'a>> {
        Ok(Box::new(SyangrilaSmartCryptReader::new(
            stream,
            cur_seg,
            Self::get_key(entry.file_hash),
        )))
    }
}

seek_reader_key_impl!(SyangrilaSmartCryptReader<T>, [u8; 5]);

impl<R: Read> Read for SyangrilaSmartCryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.inner.read(buf)?;
        let offset = self.seg_start + self.pos;
        for (i, t) in buf[..readed].iter_mut().enumerate() {
            let tpos = offset + i as u64;
            if tpos <= 0x64 {
                *t ^= self.key[4];
            } else {
                *t ^= self.key[(tpos & 3) as usize];
            }
        }
        self.pos += readed as u64;
        Ok(readed)
    }
}

seek_crypt_filehash_key_u8_impl!(Kano2Crypt, Kano2CryptReader<T>);

impl<R: Read> Read for Kano2CryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.inner.read(buf)?;
        let mut offset = (self.seg_start + self.pos) % 8;
        for t in buf[..readed].iter_mut() {
            if offset == 0 {
                *t ^= self.key;
            }
            offset = (offset + 1) % 8;
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
    assert!(CRYPT_SCHEMA.contains_key(CIS::from_str("PURELY x CATION")));
}

#[test]
fn check_alias_exists() {
    let mut alias = std::collections::HashSet::new();
    for (key, schema) in CRYPT_SCHEMA.iter() {
        if alias.contains(key.as_str()) {
            panic!("Game {} is already used", key);
        }
        alias.insert(key.to_string());
        if let Some(title) = &schema.title {
            for part in title.split("|") {
                let part = part.trim();
                if alias.contains(part) {
                    panic!("Game alias {} in {} is already used", part, key);
                }
                alias.insert(part.to_string());
            }
        }
    }
}

#[test]
fn test_cx_cb_table() {
    for (key, list) in CX_CB_TABLE.iter() {
        println!("Key: {}, List length: {}", key, list.len());
    }
}

#[test]
fn test_altered_pink_key_table() {
    assert_eq!(ALTERED_PINK_KEY_TABLE.len(), 0x100);
}
