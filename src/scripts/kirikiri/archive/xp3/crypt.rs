use super::archive::*;
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use anyhow::Result;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Seek, SeekFrom};

pub trait Crypt: std::fmt::Debug {
    /// Initializes the cryptographic context for the archive.
    fn init(&self, _archive: &mut Xp3Archive) -> Result<()> {
        Ok(())
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
#[serde(rename_all = "PascalCase", tag = "$type")]
enum CryptType {
    NoCrypt,
    FateCrypt,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Schema {
    #[serde(flatten)]
    crypt: CryptType,
    title: Option<String>,
}

impl Schema {
    pub fn create_crypt(&self) -> Box<dyn Crypt> {
        match self.crypt {
            CryptType::NoCrypt => Box::new(NoCrypt::new()),
            CryptType::FateCrypt => Box::new(FateCrypt::new()),
        }
    }
}

include_flate::flate!(static CRYPT_DATA: str from "src/scripts/kirikiri/archive/xp3/crypt.json" with zstd);

lazy_static::lazy_static! {
    static ref CRYPT_SCHEMA: BTreeMap<String, Schema> = {
        serde_json::from_str(&CRYPT_DATA).expect("Failed to parse crypt.json")
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

impl Crypt for NoCrypt {}

#[derive(Debug)]
pub struct FateCrypt {}

impl FateCrypt {
    pub fn new() -> Self {
        Self {}
    }
}

impl Crypt for FateCrypt {
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
        Ok(Box::new(FateCryptReader::new(stream, cur_seg)))
    }

    fn decrypt_with_seek<'a>(
        &self,
        _entry: &Xp3Entry,
        cur_seg: &Segment,
        stream: Box<dyn ReadSeek + 'a>,
    ) -> Result<Box<dyn ReadSeek + 'a>> {
        Ok(Box::new(FateCryptReader::new(stream, cur_seg)))
    }
}

struct FateCryptReader<R: Read> {
    inner: R,
    /// Start offset of the current xp3 entry.
    seg_start: u64,
    seg_size: u64,
    pos: u64,
}

impl<T: Read> FateCryptReader<T> {
    pub fn new(inner: T, seg: &Segment) -> Self {
        Self {
            inner,
            seg_start: seg.offset_in_file,
            seg_size: seg.original_size,
            pos: 0,
        }
    }
}

#[automatically_derived]
impl<T: Read> std::fmt::Debug for FateCryptReader<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FateCryptReader")
            .field("seg_start", &self.seg_start)
            .field("seg_size", &self.seg_size)
            .field("pos", &self.pos)
            .finish()
    }
}

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

impl<T: Read + Seek> Seek for FateCryptReader<T> {
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

#[test]
fn test_deserialize_crypt() {
    for (key, schema) in CRYPT_SCHEMA.iter() {
        println!("Title: {}, Schema: {:?}", key, schema);
    }
}
