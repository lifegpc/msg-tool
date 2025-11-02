use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::blowfish::*;
use crate::utils::encoding::*;
use crate::utils::rc4::*;
use crate::utils::serde_base64bytes::Base64Bytes;
use crate::utils::struct_pack::*;
use crate::utils::xored_stream::XoredStream;
use anyhow::Result;
use msg_tool_macro::{StructPack, StructUnpack};
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::{Arc, Mutex};

include_flate::flate!(static PAZ_DATA: str from "src/scripts/musica/archive/paz.json" with zstd);

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ArcKey {
    index_key: Base64Bytes,
    data_key: Option<Base64Bytes>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Schema {
    version: u32,
    arc_keys: HashMap<String, ArcKey>,
    type_keys: HashMap<String, String>,
    /// PAZ file signature
    signature: u32,
}

impl Schema {
    pub fn get_type_key(&self, entry: &PazEntry) -> Option<&str> {
        let name = std::path::Path::new(&entry.name)
            .extension()?
            .to_string_lossy()
            .to_lowercase();
        self.type_keys.get(&name).map(|s| s.as_str())
    }
}

lazy_static::lazy_static! {
    static ref PAZ_SCHEMA: BTreeMap<String, Schema> = {
        serde_json::from_str(&PAZ_DATA).expect("Failed to parse paz.json")
    };
}

/// Get the supported game titles for PAZ archives.
pub fn get_supported_games() -> Vec<&'static str> {
    PAZ_SCHEMA.keys().map(|s| s.as_str()).collect()
}

fn query_paz_schema(game: &str) -> Option<&'static Schema> {
    PAZ_SCHEMA.get(game)
}

fn query_paz_schema_by_signature(signature: u32) -> Option<(&'static str, &'static Schema)> {
    for (game, schema) in PAZ_SCHEMA.iter() {
        if schema.signature == signature {
            return Some((game.as_str(), schema));
        }
    }
    None
}

#[derive(Debug)]
pub struct PazArcBuilder {}

impl PazArcBuilder {
    pub fn new() -> Self {
        PazArcBuilder {}
    }
}

impl ScriptBuilder for PazArcBuilder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Cp932
    }

    fn default_archive_encoding(&self) -> Option<Encoding> {
        Some(Encoding::Cp932)
    }

    fn build_script(
        &self,
        buf: Vec<u8>,
        filename: &str,
        _encoding: Encoding,
        archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(PazArc::new(
            MemReader::new(buf),
            filename,
            archive_encoding,
            config,
        )?))
    }

    fn build_script_from_file(
        &self,
        filename: &str,
        _encoding: Encoding,
        archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        let f = std::fs::File::open(filename)?;
        let f = std::io::BufReader::new(f);
        Ok(Box::new(PazArc::new(
            f,
            filename,
            archive_encoding,
            config,
        )?))
    }

    fn build_script_from_reader(
        &self,
        reader: Box<dyn ReadSeek>,
        filename: &str,
        _encoding: Encoding,
        archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(PazArc::new(
            reader,
            filename,
            archive_encoding,
            config,
        )?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["paz"]
    }

    fn is_archive(&self) -> bool {
        true
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::MusicaPaz
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 4 {
            let sign = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
            if let Some(_) = query_paz_schema_by_signature(sign) {
                return Some(10);
            }
        }
        None
    }
}

#[derive(Debug, StructPack, StructUnpack, Clone)]
struct PazEntry {
    #[cstring]
    name: String,
    offset: u64,
    unpacked_size: u32,
    size: u32,
    aligned_size: u32,
    flags: u32,
}

#[derive(Debug)]
pub struct PazArc {
    stream: Arc<Mutex<MultipleReadStream>>,
    schema: Schema,
    arc_key: ArcKey,
    entries: Vec<PazEntry>,
    archive_encoding: Encoding,
    xor_key: u8,
}

impl PazArc {
    pub fn new<T: ReadSeek + 'static>(
        reader: T,
        filename: &str,
        archive_encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<Self> {
        let mut stream = MultipleReadStream::new();
        stream.add_stream(reader)?;
        for suffix in b'A'..=b'Z' {
            let arc_filename = format!("{}{}", filename, suffix as char);
            if let Ok(f) = std::fs::File::open(&arc_filename) {
                let f = std::io::BufReader::new(f);
                stream.add_stream_boxed(Box::new(f))?;
            } else {
                break;
            }
        }
        let schema = if let Some(title) = &config.musica_game_title {
            let schema = query_paz_schema(title).ok_or_else(|| {
                anyhow::anyhow!("Unsupported game title '{}' for PAZ archive", title)
            })?;
            let sig = stream.read_u32()?;
            if schema.signature != 0 && schema.signature != sig {
                eprintln!(
                    "Warning: PAZ signature {:08X} does not match expected signature {:08X} for game '{}'",
                    sig, schema.signature, title
                );
                crate::COUNTER.inc_warning();
            }
            schema
        } else {
            let sig = stream.read_u32()?;
            let (game, schema) = query_paz_schema_by_signature(sig).ok_or_else(|| {
                anyhow::anyhow!(
                    "Unknown PAZ signature {:08X}. Please specify the game title in the config.",
                    sig
                )
            })?;
            eprintln!("Detected PAZ archive for game '{}'", game);
            schema
        };
        let arc_name = std::path::Path::new(filename)
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?
            .to_lowercase();
        let arc_key = schema.arc_keys.get(&arc_name).ok_or_else(|| {
            anyhow::anyhow!(
                "No ARC key found for archive name '{}' in game schema",
                arc_name
            )
        })?;
        let mut start_offset = if schema.version > 0 { 0x20 } else { 0 };
        stream.seek(SeekFrom::Start(start_offset))?;
        let mut index_size = stream.read_u32()?;
        start_offset += 4;
        let xor_key = (index_size >> 24) as u8;
        if xor_key != 0 {
            let t = xor_key as u32;
            index_size ^= t << 24 | t << 16 | t << 8 | t;
        }
        if index_size & 7 != 0 {
            return Err(anyhow::anyhow!("Invalid PAZ index size"));
        }
        let entries = {
            let blowfish: Blowfish<byteorder::LE> = Blowfish::new(&arc_key.index_key)?;
            let mut index_stream: Box<dyn ReadSeek> = Box::new(StreamRegion::new(
                &mut stream,
                start_offset,
                start_offset + index_size as u64,
            )?);
            if xor_key != 0 {
                index_stream = Box::new(XoredStream::new(index_stream, xor_key));
            }
            let mut index_stream = BlowfishDecryptor::new(blowfish.clone(), index_stream);
            let count = index_stream.read_u32()?;
            let mut entries = Vec::with_capacity(count as usize);
            for _ in 0..count {
                let entry: PazEntry = index_stream.read_struct(false, archive_encoding)?;
                entries.push(entry);
            }
            entries
        };
        Ok(PazArc {
            stream: Arc::new(Mutex::new(stream)),
            schema: schema.clone(),
            arc_key: arc_key.clone(),
            entries,
            archive_encoding,
            xor_key,
        })
    }
}

impl Script for PazArc {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Json
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn is_archive(&self) -> bool {
        true
    }

    fn iter_archive_filename<'a>(
        &'a self,
    ) -> Result<Box<dyn Iterator<Item = Result<String>> + 'a>> {
        Ok(Box::new(
            self.entries.iter().map(|entry| Ok(entry.name.clone())),
        ))
    }

    fn iter_archive_offset<'a>(&'a self) -> Result<Box<dyn Iterator<Item = Result<u64>> + 'a>> {
        Ok(Box::new(self.entries.iter().map(|entry| Ok(entry.offset))))
    }

    fn open_file<'a>(&'a self, index: usize) -> Result<Box<dyn ArchiveContent + 'a>> {
        if index >= self.entries.len() {
            return Err(anyhow::anyhow!("Index out of bounds"));
        }
        let entry = self.entries[index].clone();
        let stream = XoredStream::new(
            StreamRegion::new(
                MutexWrapper::new(self.stream.clone(), entry.offset),
                entry.offset,
                entry.offset + entry.aligned_size as u64,
            )?,
            self.xor_key,
        );
        if let Some(data_key) = &self.arc_key.data_key {
            let blowfish: Blowfish<byteorder::LE> = Blowfish::new(&data_key.bytes)?;
            let stream = StreamRegion::new(
                BlowfishDecryptor::new(blowfish, stream),
                0,
                entry.size as u64,
            )?;
            if let Some(type_key) = self.schema.get_type_key(&entry) {
                let key = format!(
                    "{} {:08X} {}",
                    entry.name.to_ascii_lowercase(),
                    entry.unpacked_size,
                    type_key
                );
                let key = encode_string(self.archive_encoding, &key, false)?;
                let mut rc4 = Rc4::new(&key);
                if self.schema.version >= 2 {
                    let crc = crc32fast::hash(&key);
                    let skip = ((crc >> 12) as i32) & 0xFF;
                    rc4.skip_bytes(skip as usize);
                }
                let stream = Rc4Stream::new(stream, rc4);
                return Ok(Box::new(PazFileEntry::new(entry, stream)));
            }
            return Ok(Box::new(PazFileEntry::new(entry, stream)));
        }
        Err(anyhow::anyhow!("Data decryption key not found."))
    }
}

#[derive(Debug)]
struct PazFileEntry<T: Read> {
    entry: PazEntry,
    stream: T,
}

impl<T: Read> PazFileEntry<T> {
    pub fn new(entry: PazEntry, stream: T) -> Self {
        PazFileEntry { entry, stream }
    }
}

impl<T: Read> Read for PazFileEntry<T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.stream.read(buf)
    }
}

impl<T: Seek + Read> Seek for PazFileEntry<T> {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        self.stream.seek(pos)
    }

    fn rewind(&mut self) -> std::io::Result<()> {
        self.stream.rewind()
    }

    fn stream_position(&mut self) -> std::io::Result<u64> {
        self.stream.stream_position()
    }
}

impl<T: Read> ArchiveContent for PazFileEntry<T> {
    fn name(&self) -> &str {
        &self.entry.name
    }
}

#[test]
fn test_deserialize_paz() {
    for (game, schema) in PAZ_SCHEMA.iter() {
        println!("Game: {}", game);
        println!("Version: {}", schema.version);
        for (arc_name, arc_key) in schema.arc_keys.iter() {
            println!("  Arc Name: {}", arc_name);
            println!("    Index Key: {:02X?}", arc_key.index_key.bytes);
            if let Some(data_key) = &arc_key.data_key {
                println!("    Data Key: {:02X?}", data_key.bytes);
            } else {
                println!("    Data Key: None");
            }
        }
        for (type_name, type_key) in schema.type_keys.iter() {
            println!("  Type Name: {}, Type Key: {}", type_name, type_key);
        }
        println!("Signature: {:08X}", schema.signature);
    }
}
