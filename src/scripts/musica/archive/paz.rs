use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::blowfish::*;
use crate::utils::encoding::*;
use crate::utils::rc4::*;
use crate::utils::serde_base64bytes::Base64Bytes;
use crate::utils::struct_pack::*;
use crate::utils::xored_stream::*;
use anyhow::Result;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
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
    xor_key: u8,
}

impl Schema {
    pub fn get_type_key(&self, entry: &PazEntry, is_audio: bool) -> Option<&str> {
        if is_audio {
            return self.type_keys.get("ogg").map(|s| s.as_str());
        }
        let mut name = std::path::Path::new(&entry.name)
            .extension()?
            .to_string_lossy()
            .to_lowercase();
        if name == "mpg" || name == "mpeg" {
            name = "avi".to_string();
        }
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

    fn create_archive(
        &self,
        filename: &str,
        files: &[&str],
        encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<Box<dyn Archive>> {
        let file = std::fs::File::create(filename)?;
        let file = std::io::BufWriter::new(file);
        Ok(Box::new(PazArcWriter::new(
            file, files, encoding, filename, config,
        )?))
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

impl PazEntry {
    pub fn is_compressed(&self) -> bool {
        (self.flags & 0x1) != 0
    }

    pub fn set_is_compressed(&mut self, compressed: bool) {
        if compressed {
            self.flags |= 0x1;
        } else {
            self.flags &= !0x1;
        }
    }
}

#[derive(Debug)]
pub struct PazArc {
    stream: Arc<Mutex<MultipleReadStream>>,
    schema: Schema,
    arc_key: ArcKey,
    entries: Vec<PazEntry>,
    archive_encoding: Encoding,
    xor_key: u8,
    is_audio: bool,
    mov_key: Option<Vec<u8>>,
}

const AUDIO_PAZ_NAMES: &[&str] = &["bgm", "se", "voice", "pmbgm", "pmse", "pmvoice"];

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
        let is_audio = AUDIO_PAZ_NAMES.contains(&arc_name.as_str());
        let is_video = arc_name == "mov";
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
        let xor_key = if let Some(xor_key) = config.musica_xor_key {
            xor_key
        } else if schema.xor_key != 0 {
            schema.xor_key
        } else {
            let xor = (index_size >> 24) as u8;
            eprintln!("Detected xor key from index size: {}", xor);
            xor
        };
        if xor_key != 0 {
            let t = xor_key as u32;
            index_size ^= t << 24 | t << 16 | t << 8 | t;
        }
        if index_size & 7 != 0 {
            return Err(anyhow::anyhow!("Invalid PAZ index size"));
        }
        let mut mov_key = None;
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
            if is_video {
                let mut key = index_stream.read_exact_vec(0x100)?;
                if schema.version < 1 {
                    let mut nkey = vec![0u8; 0x100];
                    for i in 0..0x100 {
                        nkey[key[i] as usize] = i as u8;
                    }
                    key = nkey;
                }
                mov_key = Some(key);
            }
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
            is_audio,
            mov_key,
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
            if self.schema.version > 0 && !entry.is_compressed() {
                if let Some(type_key) = self.schema.get_type_key(&entry, self.is_audio) {
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
            }
            if entry.is_compressed() {
                let stream = ZlibDecoder::new(stream);
                return Ok(Box::new(PazFileEntry::new(entry, stream)));
            }
            return Ok(Box::new(PazFileEntry::new(entry, stream)));
        } else if let Some(mov_key) = &self.mov_key {
            if self.schema.version < 1 {
                let stream = TableEncryptedStream::new(stream, mov_key.clone())?;
                if entry.is_compressed() {
                    let stream = ZlibDecoder::new(stream);
                    return Ok(Box::new(PazFileEntry::new(entry, stream)));
                }
                return Ok(Box::new(PazFileEntry::new(entry, stream)));
            }
            let type_key = self
                .schema
                .get_type_key(&entry, self.is_audio)
                .ok_or_else(|| {
                    anyhow::anyhow!("Data decryption key not found for entry '{}'.", entry.name)
                })?;
            let key = format!(
                "{} {:08X} {}",
                entry.name.to_ascii_lowercase(),
                entry.unpacked_size,
                type_key
            );
            let key = encode_string(self.archive_encoding, &key, false)?;
            let mut rkey = mov_key.clone();
            let key_len = key.len();
            for i in 0..0x100 {
                rkey[i] ^= key[i % key_len];
            }
            let mut rc4 = Rc4::new(&rkey);
            let key_block = rc4.generate_block((entry.size as usize).min(0x10000));
            let stream = XoredKeyStream::new(stream, key_block, 0);
            if entry.is_compressed() {
                let stream = ZlibDecoder::new(stream);
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

    fn script_type(&self) -> Option<&ScriptType> {
        let ext_name = std::path::Path::new(&self.entry.name)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        match ext_name.as_str() {
            "sc" => Some(&ScriptType::Musica),
            _ => None,
        }
    }
}

struct TableEncryptedStream<T> {
    inner: T,
    table: Vec<u8>,
}

impl<T> TableEncryptedStream<T> {
    pub fn new(inner: T, table: Vec<u8>) -> Result<Self> {
        if table.len() != 256 {
            return Err(anyhow::anyhow!(
                "Table length must be 256, got {}",
                table.len()
            ));
        }
        Ok(TableEncryptedStream { inner, table })
    }
}

impl<T: Read> Read for TableEncryptedStream<T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.inner.read(buf)?;
        for i in 0..readed {
            buf[i] = self.table[buf[i] as usize];
        }
        Ok(readed)
    }
}

impl<T: Seek> Seek for TableEncryptedStream<T> {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(pos)
    }

    fn rewind(&mut self) -> std::io::Result<()> {
        self.inner.rewind()
    }

    fn stream_position(&mut self) -> std::io::Result<u64> {
        self.inner.stream_position()
    }
}

impl<T: Write> Write for TableEncryptedStream<T> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut encrypted_buf = vec![0u8; buf.len()];
        for i in 0..buf.len() {
            encrypted_buf[i] = self.table[buf[i] as usize];
        }
        self.inner.write(&encrypted_buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

pub struct PazArcWriter<T: Write + Seek> {
    writer: T,
    headers: HashMap<String, PazEntry>,
    encoding: Encoding,
    is_audio: bool,
    mov_key: Option<Vec<u8>>,
    schema: Schema,
    arc_key: ArcKey,
    xor_key: u8,
    compress: bool,
    compress_level: u32,
}

impl<T: Write + Seek> PazArcWriter<T> {
    pub fn new(
        mut writer: T,
        files: &[&str],
        encoding: Encoding,
        filename: &str,
        config: &ExtraConfig,
    ) -> Result<Self> {
        let schema = config.musica_game_title.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "Game title not specified. Use --musica-game-title to specify the game title."
            )
        })?;
        let schema = query_paz_schema(schema).ok_or_else(|| {
            anyhow::anyhow!("Unsupported game title '{}' for PAZ archive", schema)
        })?;
        let arc_name = std::path::Path::new(filename)
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?
            .to_lowercase();
        let is_audio = AUDIO_PAZ_NAMES.contains(&arc_name.as_str());
        let is_video = arc_name == "mov";
        let arc_key = schema.arc_keys.get(&arc_name).ok_or_else(|| {
            anyhow::anyhow!(
                "No ARC key found for archive name '{}' in game schema",
                arc_name
            )
        })?;
        let mov_key = if is_video {
            let mut key = vec![0u8; 0x100];
            for i in 0..0x100 {
                key[i] = i as u8;
            }
            Some(key)
        } else {
            None
        };
        let start_offset = if schema.version > 0 { 0x20 } else { 0 };
        if start_offset > 0 {
            if schema.signature != 0 {
                writer.write_u32(schema.signature)?;
            }
            writer.seek(SeekFrom::Start(start_offset))?;
        }
        let mut entries = HashMap::new();
        for file in files {
            let entry = PazEntry {
                name: file.to_string(),
                offset: 0,
                unpacked_size: 0,
                size: 0,
                aligned_size: 0,
                flags: 0,
            };
            entries.insert(file.to_string(), entry);
        }
        let xor_key = if let Some(xor_key) = config.musica_xor_key {
            xor_key
        } else {
            schema.xor_key
        };
        writer.write_u32(0)?; // Placeholder for index size
        {
            let blowfish: Blowfish<byteorder::LE> = Blowfish::new(&arc_key.index_key)?;
            let stream = XoredStream::new(&mut writer, xor_key);
            let mut index_stream = BlowfishEncryptor::new(blowfish, stream);
            index_stream.write_u32(entries.len() as u32)?;
            if let Some(mov_data) = &mov_key {
                index_stream.write_all(mov_data)?;
            }
            for entry in entries.values() {
                index_stream.write_struct(entry, false, encoding)?;
            }
        }
        let index_end = writer.stream_position()?;
        let index_size = (index_end - start_offset - 4) as u32;
        if xor_key != 0 {
            let mut stream = XoredStream::new(&mut writer, xor_key);
            stream.write_u32_at(start_offset, index_size)?;
        } else {
            writer.write_u32_at(start_offset, index_size)?;
        };
        Ok(PazArcWriter {
            writer,
            headers: entries,
            encoding,
            is_audio,
            mov_key,
            schema: schema.clone(),
            arc_key: arc_key.clone(),
            xor_key,
            compress: config.musica_compress,
            compress_level: config.zlib_compression_level,
        })
    }
}

impl<T: Write + Seek> Archive for PazArcWriter<T> {
    fn new_file<'a>(
        &'a mut self,
        name: &str,
        _size: Option<u64>,
    ) -> Result<Box<dyn WriteSeek + 'a>> {
        let entry = self
            .headers
            .get_mut(name)
            .ok_or_else(|| anyhow::anyhow!("File '{}' not found in PAZ archive headers", name))?;
        if entry.offset != 0 || entry.size != 0 {
            return Err(anyhow::anyhow!(
                "File '{}' already exists in PAZ archive",
                name
            ));
        }
        if let Some(data_key) = &self.arc_key.data_key {
            let blowfish: Blowfish<byteorder::LE> = Blowfish::new(&data_key.bytes)?;
            entry.offset = self.writer.stream_position()?;
            let stream = XoredStream::new(&mut self.writer, self.xor_key);
            let stream = BlowfishEncryptor::new(blowfish, stream);
            let mut type_key = None;
            entry.set_is_compressed(self.compress);
            if self.schema.version > 0 && !self.compress {
                if let Some(tkey) = self.schema.get_type_key(&entry, self.is_audio) {
                    type_key = Some(tkey.to_string());
                }
            }
            let writer = MemDataKeyWriter {
                inner: Box::new(stream),
                cache: MemWriter::new(),
                type_key,
                entry,
                encoding: self.encoding,
                version: self.schema.version,
                compress: self.compress,
                compress_level: self.compress_level,
                compressed_size: 0,
            };
            return Ok(Box::new(writer));
        } else if let Some(mov_key) = &self.mov_key {
            entry.offset = self.writer.stream_position()?;
            let stream = XoredStream::new(&mut self.writer, self.xor_key);
            if self.schema.version < 1 {
                let stream = TableEncryptedStream::new(stream, mov_key.clone())?;
                let writer = MovDataWriter {
                    inner: Box::new(stream),
                    entry,
                };
                return Ok(Box::new(writer));
            }
            let type_key = self
                .schema
                .get_type_key(&entry, self.is_audio)
                .ok_or_else(|| {
                    anyhow::anyhow!("Data decryption key not found for entry '{}'.", entry.name)
                })?;
            let writer = MemMovDataKeyWriter {
                inner: Box::new(stream),
                cache: MemWriter::new(),
                type_key: type_key.to_string(),
                mov_key: mov_key.clone(),
                entry,
                encoding: self.encoding,
            };
            return Ok(Box::new(writer));
        }
        Err(anyhow::anyhow!("Data encryption key not found."))
    }

    fn new_file_non_seek<'a>(
        &'a mut self,
        name: &str,
        size: Option<u64>,
    ) -> Result<Box<dyn Write + 'a>> {
        if let Some(data_key) = &self.arc_key.data_key {
            let size = match size {
                Some(size) => size,
                None => {
                    return Ok(Box::new(self.new_file(name, None)?));
                }
            };
            let entry = self.headers.get_mut(name).ok_or_else(|| {
                anyhow::anyhow!("File '{}' not found in PAZ archive headers", name)
            })?;
            if entry.offset != 0 || entry.size != 0 {
                return Err(anyhow::anyhow!(
                    "File '{}' already exists in PAZ archive",
                    name
                ));
            }
            let blowfish: Blowfish<byteorder::LE> = Blowfish::new(&data_key.bytes)?;
            entry.offset = self.writer.stream_position()?;
            let stream = XoredStream::new(&mut self.writer, self.xor_key);
            let stream = BlowfishEncryptor::new(blowfish, stream);
            if self.schema.version > 0 && !self.compress {
                if let Some(tkey) = self.schema.get_type_key(&entry, self.is_audio) {
                    let key = format!("{} {:08X} {}", entry.name.to_ascii_lowercase(), size, tkey);
                    let key = encode_string(self.encoding, &key, false)?;
                    let mut rc4 = Rc4::new(&key);
                    if self.schema.version >= 2 {
                        let crc = crc32fast::hash(&key);
                        let skip = ((crc >> 12) as i32) & 0xFF;
                        rc4.skip_bytes(skip as usize);
                    }
                    let writer = Rc4Stream::new(stream, rc4);
                    let writer = DateKeyWriter {
                        inner: Box::new(writer),
                        entry,
                    };
                    return Ok(Box::new(writer));
                }
            }
            if self.compress {
                entry.set_is_compressed(true);
                let writer = DataKeyComWriter::new(Box::new(stream), entry, self.compress_level);
                return Ok(Box::new(writer));
            }
            let writer = DateKeyWriter {
                inner: Box::new(stream),
                entry,
            };
            return Ok(Box::new(writer));
        } else if let Some(mov_key) = &self.mov_key {
            let size = match size {
                Some(size) => size,
                None => {
                    return Ok(Box::new(self.new_file(name, None)?));
                }
            };
            let entry = self.headers.get_mut(name).ok_or_else(|| {
                anyhow::anyhow!("File '{}' not found in PAZ archive headers", name)
            })?;
            if entry.offset != 0 || entry.size != 0 {
                return Err(anyhow::anyhow!(
                    "File '{}' already exists in PAZ archive",
                    name
                ));
            }
            entry.offset = self.writer.stream_position()?;
            let stream = XoredStream::new(&mut self.writer, self.xor_key);
            if self.schema.version < 1 {
                let stream = TableEncryptedStream::new(stream, mov_key.clone())?;
                let writer = MovDataWriter {
                    inner: Box::new(stream),
                    entry,
                };
                return Ok(Box::new(writer));
            }
            let type_key = self
                .schema
                .get_type_key(&entry, self.is_audio)
                .ok_or_else(|| {
                    anyhow::anyhow!("Data decryption key not found for entry '{}'.", entry.name)
                })?;
            let key = format!(
                "{} {:08X} {}",
                entry.name.to_ascii_lowercase(),
                size,
                type_key
            );
            let key = encode_string(self.encoding, &key, false)?;
            let mut rkey = mov_key.clone();
            let key_len = key.len();
            for i in 0..0x100 {
                rkey[i] ^= key[i % key_len];
            }
            let mut rc4 = Rc4::new(&rkey);
            let key_block = rc4.generate_block((size as usize).min(0x10000));
            let region = StreamRegion::new(stream, entry.offset, entry.offset + size)?;
            let stream = XoredKeyStream::new(region, key_block, 0);
            let writer = DateKeyWriter {
                inner: Box::new(stream),
                entry,
            };
            return Ok(Box::new(writer));
        }
        Err(anyhow::anyhow!("Data encryption key not found."))
    }

    fn write_header(&mut self) -> Result<()> {
        let start_offset = if self.schema.version > 0 { 0x24 } else { 4 };
        self.writer.seek(SeekFrom::Start(start_offset))?;
        {
            let blowfish: Blowfish<byteorder::LE> = Blowfish::new(&self.arc_key.index_key)?;
            let stream = XoredStream::new(&mut self.writer, self.xor_key);
            let mut index_stream = BlowfishEncryptor::new(blowfish, stream);
            index_stream.write_u32(self.headers.len() as u32)?;
            if let Some(mov_data) = &self.mov_key {
                index_stream.write_all(mov_data)?;
            }
            for entry in self.headers.values() {
                index_stream.write_struct(entry, false, self.encoding)?;
            }
        }
        Ok(())
    }
}

struct DateKeyWriter<'a> {
    inner: Box<dyn Write + 'a>,
    entry: &'a mut PazEntry,
}

impl<'a> Write for DateKeyWriter<'a> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let writed = self.inner.write(buf)?;
        self.entry.size += writed as u32;
        Ok(writed)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

impl<'a> Drop for DateKeyWriter<'a> {
    fn drop(&mut self) {
        self.entry.unpacked_size = self.entry.size;
        self.entry.aligned_size = (self.entry.size + 7) & !7;
    }
}

struct DataKeyComWriter<'a> {
    inner: ZlibEncoder<Box<dyn Write + 'a>>,
    entry: &'a mut PazEntry,
}

impl<'a> DataKeyComWriter<'a> {
    pub fn new(inner: Box<dyn Write + 'a>, entry: &'a mut PazEntry, level: u32) -> Self {
        DataKeyComWriter {
            inner: ZlibEncoder::new(inner, flate2::Compression::new(level)),
            entry,
        }
    }
}

impl<'a> Write for DataKeyComWriter<'a> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

impl<'a> Drop for DataKeyComWriter<'a> {
    fn drop(&mut self) {
        if let Err(e) = self.inner.try_finish() {
            eprintln!(
                "Error finishing compression for PAZ file entry '{}': {}",
                self.entry.name, e
            );
            crate::COUNTER.inc_error();
            return;
        }
        self.entry.size = self.inner.total_out() as u32;
        self.entry.unpacked_size = self.inner.total_in() as u32;
        self.entry.aligned_size = (self.entry.size + 7) & !7;
    }
}

trait MyWriteSeek: Write + Seek {}
impl<T: Write + Seek> MyWriteSeek for T {}

struct MovDataWriter<'a> {
    inner: Box<dyn MyWriteSeek + 'a>,
    entry: &'a mut PazEntry,
}

impl<'a> Write for MovDataWriter<'a> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

impl<'a> Seek for MovDataWriter<'a> {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(pos)
    }

    fn rewind(&mut self) -> std::io::Result<()> {
        self.inner.rewind()
    }

    fn stream_position(&mut self) -> std::io::Result<u64> {
        self.inner.stream_position()
    }
}

impl<'a> Drop for MovDataWriter<'a> {
    fn drop(&mut self) {
        if let Ok(pos) = self.inner.stream_position() {
            self.entry.unpacked_size = (pos - self.entry.offset) as u32;
            self.entry.size = self.entry.unpacked_size;
            self.entry.aligned_size = self.entry.size;
        } else {
            eprintln!(
                "Error getting stream position for PAZ file entry '{}'",
                self.entry.name
            );
            crate::COUNTER.inc_error();
        }
    }
}

struct MemDataKeyWriter<'a> {
    inner: Box<dyn Write + 'a>,
    cache: MemWriter,
    type_key: Option<String>,
    entry: &'a mut PazEntry,
    encoding: Encoding,
    version: u32,
    compress: bool,
    compress_level: u32,
    compressed_size: u64,
}

impl<'a> Write for MemDataKeyWriter<'a> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.cache.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.cache.flush()
    }
}

impl<'a> Seek for MemDataKeyWriter<'a> {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        self.cache.seek(pos)
    }

    fn rewind(&mut self) -> std::io::Result<()> {
        self.cache.rewind()
    }

    fn stream_position(&mut self) -> std::io::Result<u64> {
        self.cache.stream_position()
    }
}

impl<'a> Drop for MemDataKeyWriter<'a> {
    fn drop(&mut self) {
        let data = &self.cache.data;
        self.entry.unpacked_size = data.len() as u32;
        self.entry.size = self.entry.unpacked_size;
        self.entry.aligned_size = (self.entry.size + 7) & !7;
        {
            let mut stream = if let Some(tkey) = &self.type_key {
                let key = format!(
                    "{} {:08X} {}",
                    self.entry.name.to_ascii_lowercase(),
                    self.entry.unpacked_size,
                    tkey
                );
                let key = match encode_string(self.encoding, &key, false) {
                    Ok(key) => key,
                    Err(e) => {
                        eprintln!(
                            "Error encoding key for PAZ file entry '{}': {}",
                            self.entry.name, e
                        );
                        crate::COUNTER.inc_error();
                        return;
                    }
                };
                let mut rc4 = Rc4::new(&key);
                if self.version >= 2 {
                    let crc = crc32fast::hash(&key);
                    let skip = ((crc >> 12) as i32) & 0xFF;
                    rc4.skip_bytes(skip as usize);
                }
                Box::new(Rc4Stream::new(&mut self.inner, rc4)) as Box<dyn Write>
            } else if self.compress {
                let stream = ZlibEncoder::new(
                    TrackStream::new(&mut self.inner, &mut self.compressed_size),
                    flate2::Compression::new(self.compress_level),
                );
                Box::new(stream) as Box<dyn Write>
            } else {
                Box::new(&mut self.inner) as Box<dyn Write>
            };
            if let Err(e) = stream.write_all(&data) {
                eprintln!("Error writing PAZ file entry '{}': {}", self.entry.name, e);
                crate::COUNTER.inc_error();
            }
        }
        if self.compress {
            self.entry.size = self.compressed_size as u32;
            self.entry.aligned_size = (self.entry.size + 7) & !7;
        }
    }
}

struct MemMovDataKeyWriter<'a> {
    inner: Box<dyn MyWriteSeek + 'a>,
    cache: MemWriter,
    type_key: String,
    entry: &'a mut PazEntry,
    encoding: Encoding,
    mov_key: Vec<u8>,
}

impl<'a> Write for MemMovDataKeyWriter<'a> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.cache.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.cache.flush()
    }
}

impl<'a> Seek for MemMovDataKeyWriter<'a> {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        self.cache.seek(pos)
    }

    fn rewind(&mut self) -> std::io::Result<()> {
        self.cache.rewind()
    }

    fn stream_position(&mut self) -> std::io::Result<u64> {
        self.cache.stream_position()
    }
}

impl<'a> Drop for MemMovDataKeyWriter<'a> {
    fn drop(&mut self) {
        let data = &self.cache.data;
        self.entry.unpacked_size = data.len() as u32;
        self.entry.size = self.entry.unpacked_size;
        self.entry.aligned_size = self.entry.size;
        let key = format!(
            "{} {:08X} {}",
            self.entry.name.to_ascii_lowercase(),
            self.entry.unpacked_size,
            self.type_key
        );
        let key = match encode_string(self.encoding, &key, false) {
            Ok(key) => key,
            Err(e) => {
                eprintln!(
                    "Error encoding key for PAZ file entry '{}': {}",
                    self.entry.name, e
                );
                crate::COUNTER.inc_error();
                return;
            }
        };
        let mut rkey = self.mov_key.clone();
        let key_len = key.len();
        for i in 0..0x100 {
            rkey[i] ^= key[i % key_len];
        }
        let mut rc4 = Rc4::new(&rkey);
        let key_block = rc4.generate_block(data.len().min(0x10000));
        let region = match StreamRegion::new(
            &mut self.inner,
            self.entry.offset,
            self.entry.offset + self.entry.size as u64,
        ) {
            Ok(region) => region,
            Err(e) => {
                eprintln!(
                    "Error creating stream region for PAZ file entry '{}': {}",
                    self.entry.name, e
                );
                crate::COUNTER.inc_error();
                return;
            }
        };
        let mut stream = XoredKeyStream::new(region, key_block, 0);
        if let Err(e) = stream.write_all(&data) {
            eprintln!("Error writing PAZ file entry '{}': {}", self.entry.name, e);
            crate::COUNTER.inc_error();
        }
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
