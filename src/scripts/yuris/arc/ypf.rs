//! Yu-Ris Archive (.ypf)
use super::pe;
use crate::ext::io::*;
use crate::ext::mutex::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use crate::utils::murmur2::*;
use crate::utils::struct_pack::*;
use crate::utils::threadpool::*;
use anyhow::{Result, anyhow, bail};
use clap::ValueEnum;
use int_enum::IntEnum;
use std::any::Any;
use std::collections::HashMap;
use std::hash::Hasher;
use std::io::{Read, Seek, SeekFrom, Write};
use std::num::NonZeroU64;
use std::ops::DerefMut;
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub struct YpfBuilder {}

impl YpfBuilder {
    pub fn new() -> Self {
        Self {}
    }
}

impl ScriptBuilder for YpfBuilder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Cp932
    }

    fn default_archive_encoding(&self) -> Option<Encoding> {
        Some(Encoding::Cp932)
    }

    fn build_script(
        &self,
        data: Vec<u8>,
        _filename: &str,
        _encoding: Encoding,
        archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script + Send + Sync>> {
        let mut base_offset = 0;
        if data.starts_with(b"MZ") {
            base_offset = pe::get_base_offset(&data)?;
        }
        Ok(Box::new(YPF::new(
            MemReader::new(data),
            archive_encoding,
            config,
            base_offset,
        )?))
    }

    fn build_script_from_file(
        &self,
        filename: &str,
        _encoding: Encoding,
        archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script + Send + Sync>> {
        if filename == "-" {
            let data = crate::utils::files::read_file(filename)?;
            let mut base_offset = 0;
            if data.starts_with(b"MZ") {
                base_offset = pe::get_base_offset(&data)?;
            }
            Ok(Box::new(YPF::new(
                MemReader::new(data),
                archive_encoding,
                config,
                base_offset,
            )?))
        } else {
            let mut file = std::fs::File::open(filename)?;
            let mut base_offset = 0;
            if file.peek_and_equal(b"MZ").is_ok() {
                let mp = pelite::FileMap::open(filename)?;
                base_offset = pe::get_base_offset(&mp)?;
            }
            Ok(Box::new(YPF::new(
                file,
                archive_encoding,
                config,
                base_offset,
            )?))
        }
    }

    fn build_script_from_reader<'a>(
        &self,
        mut reader: Box<dyn ReadSeek + Send + Sync + 'a>,
        _filename: &str,
        _encoding: Encoding,
        archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script + Send + Sync + 'a>> {
        let mut base_offset = 0;
        if reader.peek_and_equal(b"MZ").is_ok() {
            let mut data = Vec::new();
            let pos = reader.stream_position()?;
            reader.read_to_end(&mut data)?;
            reader.seek(SeekFrom::Start(pos))?;
            base_offset = pe::get_base_offset(&data)?;
        }
        Ok(Box::new(YPF::new(
            reader,
            archive_encoding,
            config,
            base_offset,
        )?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ypf", "exe"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::YurisYPF
    }

    fn is_this_format(&self, filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 4 && buf.starts_with(b"YPF\0") {
            return Some(20);
        }
        if buf_len >= 2 && buf.starts_with(b"MZ") {
            let p = std::path::Path::new(filename);
            if p.exists() {
                if let Ok(file) = pelite::FileMap::open(p) {
                    if pe::get_base_offset(&file).is_ok() {
                        return Some(20);
                    }
                }
            }
        }
        None
    }

    fn is_archive(&self) -> bool {
        true
    }

    fn create_archive(
        &self,
        filename: &str,
        files: &[&str],
        encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<Box<dyn Archive>> {
        let f = std::fs::File::create(filename)?;
        let writer = std::io::BufWriter::new(f);
        Ok(Box::new(YPFArchiveWriter::new(
            writer, files, encoding, config,
        )?))
    }
}

#[repr(u8)]
#[derive(Debug, IntEnum, Clone, Copy)]
enum ResourceType {
    Default,
    BMP,
    PNG,
    JPG,
    GIF,
    WAV,
    OGG,
    PSD,
    YCG,
    PSB,
    WAV_,
    OGG_,
    OPUS,
}

impl Default for ResourceType {
    fn default() -> Self {
        Self::Default
    }
}

/// Map file extension to `ResourceType`.
///
/// When `use_new_file_type` is true, ogg/wav are mapped to the newer
/// type values (`OGG_` / `WAV_`); otherwise they use the legacy values.
fn get_file_type(name: &str, use_new_file_type: bool) -> ResourceType {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "bmp" => ResourceType::BMP,
        "png" => ResourceType::PNG,
        "jpg" | "jpeg" => ResourceType::JPG,
        "gif" => ResourceType::GIF,
        "ycg" => ResourceType::YCG,
        "psb" => ResourceType::PSB,
        "wav" => {
            if use_new_file_type {
                ResourceType::WAV_
            } else {
                ResourceType::WAV
            }
        }
        "ogg" => {
            if use_new_file_type {
                ResourceType::OGG_
            } else {
                ResourceType::OGG
            }
        }
        "psd" => ResourceType::PSD,
        "opus" => ResourceType::OPUS,
        _ => ResourceType::Default,
    }
}

#[derive(Clone, Debug)]
struct YPFEntry {
    name_hash: u32,
    name: String,
    typ: ResourceType,
    compressed: bool,
    size: u32,
    compressed_size: u32,
    offset: u64,
    hash: Option<u32>,
}

fn get_info_as_version(info: &Option<Box<dyn Any>>) -> Result<u32> {
    Ok(*info
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("info not found"))?
        .downcast_ref()
        .ok_or_else(|| anyhow::anyhow!("not YSTBHeader"))?)
}

impl StructPack for YPFEntry {
    fn pack<W: Write>(
        &self,
        writer: &mut W,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn std::any::Any>>,
    ) -> Result<()> {
        let version = get_info_as_version(info)?;
        self.name_hash.pack(writer, big, encoding, info)?;
        let table = if version < 500 {
            &NAME_DEFAULT_TABLE
        } else {
            &NAME_V500_TABLE
        };
        let mut name = encode_string(encoding, &self.name, true)?;
        if name.len() > 0xFF {
            bail!("File name can not longer than 255 bytes.");
        }
        let name_len = name.len() as u8;
        let name_len = (table
            .iter()
            .position(|s| *s == name_len)
            .ok_or_else(|| anyhow!("No suitable len found in table"))?
            as u8)
            ^ 0xFF;
        name_len.pack(writer, big, encoding, info)?;
        for num in name.iter_mut() {
            *num ^= match version {
                290 => 64,
                500 => 54,
                _ => 0,
            };
            *num = !(*num);
        }
        writer.write_all(&name)?;
        (self.typ as u8).pack(writer, big, encoding, info)?;
        self.compressed.pack(writer, big, encoding, info)?;
        self.size.pack(writer, big, encoding, info)?;
        self.compressed_size.pack(writer, big, encoding, info)?;
        if version >= 480 {
            self.offset.pack(writer, big, encoding, info)?;
        } else {
            (self.offset as u32).pack(writer, big, encoding, info)?;
        };
        if version >= 473 {
            let hash = self.hash.ok_or_else(|| anyhow!("hash not specified."))?;
            hash.pack(writer, big, encoding, info)?;
        }
        Ok(())
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum NameHashType {
    /// Crc32
    Crc32,
    /// Murmur2
    Murmur2,
}

impl Default for NameHashType {
    fn default() -> Self {
        Self::Murmur2
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum DataHashType {
    /// Adler32
    Adler32,
    /// Murmur2
    Murmur2,
    /// Xxhash32
    Xxh32,
}

impl Default for DataHashType {
    fn default() -> Self {
        Self::Murmur2
    }
}

#[derive(Debug)]
pub struct YPF<'a, T: Read + Seek + std::fmt::Debug + 'a> {
    #[allow(unused)]
    version: u32,
    entries: Vec<YPFEntry>,
    reader: Arc<Mutex<T>>,
    _mark: std::marker::PhantomData<&'a ()>,
}

const NAME_DEFAULT_TABLE: [u8; 256] = [
    0, 1, 2, 72, 4, 5, 53, 7, 8, 11, 10, 9, 16, 19, 14, 15, 12, 25, 18, 13, 20, 27, 22, 23, 24, 17,
    26, 21, 30, 29, 28, 31, 35, 33, 34, 32, 36, 37, 41, 39, 40, 38, 42, 43, 47, 45, 50, 44, 48, 49,
    46, 51, 52, 6, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 3, 73,
    74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97,
    98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116,
    117, 118, 119, 120, 121, 122, 123, 124, 125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135,
    136, 137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148, 149, 150, 151, 152, 153, 154,
    155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173,
    174, 175, 176, 177, 178, 179, 180, 181, 182, 183, 184, 185, 186, 187, 188, 189, 190, 191, 192,
    193, 194, 195, 196, 197, 198, 199, 200, 201, 202, 203, 204, 205, 206, 207, 208, 209, 210, 211,
    212, 213, 214, 215, 216, 217, 218, 219, 220, 221, 222, 223, 224, 225, 226, 227, 228, 229, 230,
    231, 232, 233, 234, 235, 236, 237, 238, 239, 240, 241, 242, 243, 244, 245, 246, 247, 248, 249,
    250, 251, 252, 253, 254, 255,
];

const NAME_V500_TABLE: [u8; 256] = [
    0, 1, 2, 10, 4, 5, 53, 7, 8, 11, 3, 9, 16, 19, 14, 15, 12, 24, 18, 13, 46, 27, 22, 23, 17, 25,
    26, 21, 30, 29, 28, 31, 35, 33, 34, 32, 36, 37, 41, 39, 40, 38, 42, 43, 47, 45, 20, 44, 48, 49,
    50, 51, 52, 6, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73,
    74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97,
    98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116,
    117, 118, 119, 120, 121, 122, 123, 124, 125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135,
    136, 137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148, 149, 150, 151, 152, 153, 154,
    155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173,
    174, 175, 176, 177, 178, 179, 180, 181, 182, 183, 184, 185, 186, 187, 188, 189, 190, 191, 192,
    193, 194, 195, 196, 197, 198, 199, 200, 201, 202, 203, 204, 205, 206, 207, 208, 209, 210, 211,
    212, 213, 214, 215, 216, 217, 218, 219, 220, 221, 222, 223, 224, 225, 226, 227, 228, 229, 230,
    231, 232, 233, 234, 235, 236, 237, 238, 239, 240, 241, 242, 243, 244, 245, 246, 247, 248, 249,
    250, 251, 252, 253, 254, 255,
];

fn detect_hash(name: &[u8], expected: u32) -> Result<NameHashType> {
    let mut hasher = StreamingMurmur2::new(0, name.len() as u32);
    hasher.write(name);
    if hasher.finish() as u32 == expected {
        return Ok(NameHashType::Murmur2);
    }
    if crc32fast::hash(name) == expected {
        return Ok(NameHashType::Crc32);
    }
    bail!("Unknown hash type or checksum/name is invalid/broken")
}

fn detect_data_hash<T: Read + Seek>(
    mut stream: T,
    size: u32,
    expected: u32,
) -> Result<DataHashType> {
    let mut murmur2_hasher = StreamingMurmur2::new(0, size);
    let mut adler32_hasher = adler::Adler32::new();
    let mut xxh32_hasher = Xxh32::new(0);
    let mut buf = [0; 1024];
    loop {
        let readed = stream.read(&mut buf)?;
        if readed == 0 {
            break;
        }
        let b = &buf[..readed];
        murmur2_hasher.write(b);
        adler32_hasher.write(b);
        xxh32_hasher.write(b);
    }
    if murmur2_hasher.finish() as u32 == expected {
        return Ok(DataHashType::Murmur2);
    }
    if adler32_hasher.finish() as u32 == expected {
        return Ok(DataHashType::Adler32);
    }
    if xxh32_hasher.finish() as u32 == expected {
        return Ok(DataHashType::Xxh32);
    }
    bail!("Unknown hash type or checksum/data is invalid/broken")
}

fn cal_name_hash(name: &[u8], typ: NameHashType) -> u32 {
    match typ {
        NameHashType::Crc32 => crc32fast::hash(name),
        NameHashType::Murmur2 => {
            let mut hasher = StreamingMurmur2::new(0, name.len() as u32);
            hasher.write(name);
            hasher.finish() as u32
        }
    }
}

impl<'b, T: Read + Seek + std::fmt::Debug + Send + Sync + 'b> YPF<'b, T> {
    pub fn new(
        mut reader: T,
        archive_encoding: Encoding,
        config: &ExtraConfig,
        base_offset: u64,
    ) -> Result<Self> {
        if base_offset > 0 {
            reader.seek(SeekFrom::Start(base_offset))?;
        }
        let mut header = [0u8; 4];
        reader.read_exact(&mut header)?;
        if &header != b"YPF\0" {
            bail!("Invalid YPF archive header")
        }
        let version = reader.read_u32()?;
        if !matches!(version, 234..=500) {
            bail!("Unsupported YPF engine version: {}", version);
        }
        eprintln!("Yuris YPF engine version: {version}");
        let count = reader.read_u32()?;
        let index_size = reader.read_u32()?;
        let mut entries = Vec::with_capacity(count as usize);
        let table = if version < 500 {
            &NAME_DEFAULT_TABLE
        } else {
            &NAME_V500_TABLE
        };
        let mut hash_type = None;
        {
            let mut index = StreamRegion::new(&mut reader, 0x20, index_size as u64)?;
            for _ in 0..count {
                let hash = index.read_u32()?;
                let length = table[(index.read_u8()? ^ 0xff) as usize];
                let mut name = index.read_exact_vec(length as usize)?;
                for num in name.iter_mut() {
                    *num = !(*num);
                    *num ^= match version {
                        290 => 64,
                        500 => 54,
                        _ => 0,
                    };
                }
                if config.yuris_check_hash {
                    if let Some(hash_type) = hash_type {
                        let thash = cal_name_hash(&name, hash_type);
                        if hash != thash {
                            let name = decode_to_string(archive_encoding, &name, false)?;
                            bail!(
                                "checksum/name is invalid/broken for {name}. expected hash: {hash:08X}, actual: {thash:08X}"
                            );
                        }
                    } else {
                        let typ = detect_hash(&name, hash)?;
                        eprintln!("Detected name hash type: {:?}", typ);
                        hash_type = Some(typ);
                    }
                }
                let name = decode_to_string(archive_encoding, &name, true)?;
                entries.push(YPFEntry {
                    name_hash: hash,
                    name: name.clone(),
                    typ: index
                        .read_u8()?
                        .try_into()
                        .map_err(|e| anyhow!("Unknown entry type for {name}: {}", e))?,
                    compressed: index.read_u8()? != 0,
                    size: index.read_u32()?,
                    compressed_size: index.read_u32()?,
                    offset: if version >= 480 {
                        index.read_u64()?
                    } else {
                        index.read_u32()? as u64
                    },
                    hash: if version >= 473 {
                        Some(index.read_u32()?)
                    } else {
                        None
                    },
                })
            }
        }
        if config.yuris_debug_archive {
            println!("Entries in yuris YPF: {:#?}", entries);
            let _ = std::io::stdout().flush();
        }
        if config.yuris_check_hash {
            let mut data_hash_type = None;
            for entry in &entries {
                let hash = match entry.hash {
                    Some(hash) if hash != 0 => hash,
                    _ => continue,
                };
                let mut stream = StreamRegion::new(
                    &mut reader,
                    entry.offset,
                    entry.offset + entry.compressed_size as u64,
                )?;
                if let Some(hash_type) = data_hash_type {
                    let mut hasher: Box<dyn Hasher> = match hash_type {
                        DataHashType::Adler32 => Box::new(adler::Adler32::new()),
                        DataHashType::Murmur2 => {
                            Box::new(StreamingMurmur2::new(0, entry.compressed_size))
                        }
                        DataHashType::Xxh32 => Box::new(Xxh32::new(0)),
                    };
                    let mut buf = [0; 1024];
                    loop {
                        let readed = stream.read(&mut buf)?;
                        if readed == 0 {
                            break;
                        }
                        hasher.write(&buf[..readed]);
                    }
                    let thash = hasher.finish() as u32;
                    if thash != hash {
                        bail!(
                            "checksum/data is invalid/broken for {}. expected hash: {hash:08X}, actual: {thash:08X}",
                            entry.name
                        );
                    }
                } else {
                    let typ = detect_data_hash(stream, entry.compressed_size, hash)?;
                    eprintln!("Detected data hash type: {:?}", typ);
                    data_hash_type = Some(typ);
                }
            }
        }
        Ok(Self {
            version,
            entries,
            reader: Arc::new(Mutex::new(reader)),
            _mark: std::marker::PhantomData,
        })
    }
}

impl<'b, T: Read + Seek + std::fmt::Debug + Send + Sync + 'b> Script for YPF<'b, T> {
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
        Ok(Box::new(self.entries.iter().map(|s| Ok(s.name.clone()))))
    }

    fn iter_archive_offset<'a>(&'a self) -> Result<Box<dyn Iterator<Item = Result<u64>> + 'a>> {
        Ok(Box::new(self.entries.iter().map(|s| Ok(s.offset))))
    }

    fn open_file<'a>(&'a self, index: usize) -> Result<Box<dyn ArchiveContent + Send + Sync + 'a>> {
        let entry = self
            .entries
            .get(index)
            .ok_or_else(|| anyhow!("index out of bound"))?;
        let mut entry = Entry {
            entry,
            stream: StreamRegion::with_size(
                MutexWrapper::new(self.reader.clone(), entry.offset),
                entry.compressed_size as u64,
            )?,
            cache: Mutex::new(None),
            pos: 0,
            script_type: None,
        };
        let mut buf = [0; 0x20];
        let readed = entry.read(&mut buf)?;
        entry.rewind()?;
        entry.script_type = detect_script_type(&entry.entry.name, readed, &buf);
        Ok(Box::new(entry))
    }
}

fn detect_script_type(_filename: &str, buf_len: usize, buf: &[u8]) -> Option<ScriptType> {
    if buf_len >= 4 {
        if buf.starts_with(b"YSCF") {
            return Some(ScriptType::YurisYSCFG);
        }
        if buf.starts_with(b"YSCM") {
            return Some(ScriptType::YurisYSCM);
        }
        if buf.starts_with(b"YSER") {
            return Some(ScriptType::YurisYSER);
        }
        if buf.starts_with(b"YSLB") {
            return Some(ScriptType::YurisYSLB);
        }
        if buf.starts_with(b"YSTB") {
            return Some(ScriptType::YurisYSTB);
        }
        if buf.starts_with(b"YSTD") {
            return Some(ScriptType::YurisYSTD);
        }
        if buf.starts_with(b"YSTL") {
            return Some(ScriptType::YurisYSTL);
        }
        if buf.starts_with(b"YSVR") {
            return Some(ScriptType::YurisYSVR);
        }
    }
    #[cfg(feature = "yuris-img")]
    if buf_len >= 12 && buf.starts_with(b"YDG\0YU-RIS\0\0") {
        return Some(ScriptType::YurisYDG);
    }
    None
}

#[derive(Debug)]
struct Entry<'a, T: Read + Seek + std::fmt::Debug + Send + Sync + 'a> {
    entry: &'a YPFEntry,
    stream: StreamRegion<MutexWrapper<T>>,
    cache: Mutex<Option<Box<dyn ReadDebug + Send + Sync + 'a>>>,
    pos: u64,
    script_type: Option<ScriptType>,
}

impl<'b, T: Read + Seek + std::fmt::Debug + Send + Sync + 'b> ArchiveContent for Entry<'b, T> {
    fn name(&self) -> &str {
        &self.entry.name
    }

    fn script_type(&self) -> Option<&ScriptType> {
        self.script_type.as_ref()
    }

    fn to_data<'a>(&'a mut self) -> Result<Box<dyn ReadSeek + Send + Sync + 'a>> {
        Ok(Box::new(self))
    }
}

impl<'a, T: Read + Seek + std::fmt::Debug + Send + Sync + 'a> Read for Entry<'a, T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.entry.compressed {
            let mut lock = self.cache.lock().map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::Other, "Failed to lock the mutex")
            })?;
            if let Some(cache) = lock.as_mut() {
                let readed = cache.read(buf)?;
                self.pos += readed as u64;
                return Ok(readed);
            }
            self.stream.rewind()?;
            self.stream.read_and_equal(b"x\xDA")?;
            let mut cache = Box::new(flate2::read::DeflateDecoder::new(self.stream.clone()))
                as Box<dyn ReadDebug + Send + Sync + 'a>;
            if self.pos > 0 {
                cache.skip(self.pos)?;
            }
            let readed = cache.read(buf)?;
            self.pos += readed as u64;
            lock.replace(cache);
            Ok(readed)
        } else {
            self.stream.read(buf)
        }
    }
}

impl<'a, T: Read + Seek + std::fmt::Debug + Send + Sync + 'a> Seek for Entry<'a, T> {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        if self.entry.compressed {
            let new_pos = match pos {
                SeekFrom::Start(p) => p,
                SeekFrom::End(offset) => {
                    if offset < 0 {
                        if (-offset) as u64 > self.entry.size as u64 {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::InvalidInput,
                                "Seek from end exceeds file length",
                            ));
                        }
                        self.entry.size as u64 - (-offset) as u64
                    } else {
                        self.entry.size as u64 + offset as u64
                    }
                }
                SeekFrom::Current(offset) => {
                    if offset < 0 {
                        if (-offset) as u64 > self.pos {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::InvalidInput,
                                "Seek from current exceeds file start",
                            ));
                        }
                        self.pos - (-offset) as u64
                    } else {
                        self.pos + offset as u64
                    }
                }
            };
            let mut lock = self.cache.lock().map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::Other, "Failed to lock the mutex")
            })?;
            if let Some(cache) = lock.as_mut()
                && self.pos <= new_pos
            {
                let to_skip = new_pos - self.pos;
                if to_skip > 0 {
                    cache.skip(to_skip)?;
                }
                self.pos = new_pos;
                Ok(new_pos)
            } else {
                lock.take();
                self.pos = new_pos;
                Ok(new_pos)
            }
        } else {
            self.stream.seek(pos)
        }
    }

    fn stream_position(&mut self) -> std::io::Result<u64> {
        if self.entry.compressed {
            Ok(self.pos)
        } else {
            self.stream.stream_position()
        }
    }
}

pub struct Xxh32 {
    inner: xxhash_rust::xxh32::Xxh32,
}

impl Xxh32 {
    pub fn new(seed: u32) -> Self {
        Self {
            inner: xxhash_rust::xxh32::Xxh32::new(seed),
        }
    }
}

impl Hasher for Xxh32 {
    fn write(&mut self, bytes: &[u8]) {
        self.inner.update(bytes);
    }
    fn finish(&self) -> u64 {
        self.inner.digest() as u64
    }
}

pub struct YPFArchiveWriter<T: Write + Seek> {
    writer: Arc<Mutex<T>>,
    headers: Arc<Mutex<HashMap<String, YPFEntry>>>,
    version: u32,
    compress: bool,
    zopfli: bool,
    compress_level: u32,
    zopfli_iteration_count: NonZeroU64,
    zopfli_iterations_without_improvement: NonZeroU64,
    zopfli_maximum_block_splits: u16,
    runner: ThreadPool<Result<()>>,
    data_hash: DataHashType,
    encoding: Encoding,
}

impl<T: Write + Seek> YPFArchiveWriter<T> {
    /// Creates a new YPF Archive Writer.
    ///
    /// * `writer` - The writer to write the archive to.
    /// * `files` - The list of files to include in the archive.
    /// * `encoding` - The encoding used for the archive.
    /// * `config` - Extra configuration options.
    pub fn new(
        mut writer: T,
        files: &[&str],
        encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<Self> {
        writer.write_all(b"YPF\0")?;
        let version = config.yuris_ypf_version.ok_or_else(|| {
            anyhow!("Version is required. Use --yuris-ypf-version to specify version.")
        })?;
        writer.write_u32(version)?;
        let file_count = files.len() as u32;
        writer.write_u32(file_count)?;
        writer.write_u32(0)?; // placeholder for header size
        writer.write_u128(0)?; // unused
        let mut headers = HashMap::new();
        let info = &Some(Box::new(version) as Box<dyn Any>);
        for file in files {
            let name = encode_string(encoding, file, true)?;
            let mut hasher: Box<dyn Hasher> = match config.yuris_name_hash_type {
                NameHashType::Crc32 => Box::new(crc32fast::Hasher::new()),
                NameHashType::Murmur2 => Box::new(StreamingMurmur2::new(0, name.len() as u32)),
            };
            hasher.write(&name);
            let header = YPFEntry {
                name_hash: hasher.finish() as u32,
                name: file.to_string(),
                typ: get_file_type(file, config.yuris_use_new_file_type),
                compressed: config.yuris_ypf_compress_file,
                size: 0,
                compressed_size: 0,
                offset: 0,
                hash: if version >= 473 { Some(0) } else { None },
            };
            header.pack(&mut writer, false, encoding, info)?;
            headers.insert(file.to_string(), header);
        }
        let header_size = writer.stream_position()?;
        writer.write_u32_at(12, header_size as u32)?;
        Ok(Self {
            writer: Arc::new(Mutex::new(writer)),
            headers: Arc::new(Mutex::new(headers)),
            version,
            compress: config.yuris_ypf_compress_file,
            zopfli: config.yuris_ypf_zopfli,
            compress_level: config.zlib_compression_level,
            zopfli_iteration_count: config.zopfli_iteration_count,
            zopfli_iterations_without_improvement: config.zopfli_iterations_without_improvement,
            zopfli_maximum_block_splits: config.zopfli_maximum_block_splits,
            runner: ThreadPool::new(
                if config.yuris_ypf_compress_file {
                    config.yuris_ypf_workers
                } else {
                    1
                },
                Some("yuris-ypf-writer"),
                false,
            )?,
            encoding,
            data_hash: config.yuris_data_hash_type,
        })
    }

    fn create_hasher(&self, length: u32) -> Box<dyn Hasher + Send + Sync> {
        match self.data_hash {
            DataHashType::Adler32 => Box::new(adler::Adler32::new()),
            DataHashType::Murmur2 => Box::new(StreamingMurmur2::new(0, length)),
            DataHashType::Xxh32 => Box::new(Xxh32::new(0)),
        }
    }

    fn create_hasher2(&self) -> Box<dyn Hasher + Send + Sync> {
        match self.data_hash {
            DataHashType::Adler32 => Box::new(adler::Adler32::new()),
            DataHashType::Murmur2 => Box::new(Murmur2::new(0)),
            DataHashType::Xxh32 => Box::new(Xxh32::new(0)),
        }
    }
}

impl<T: Write + Seek + Send + Sync + 'static> Archive for YPFArchiveWriter<T> {
    fn new_file<'a>(
        &'a mut self,
        name: &str,
        size: Option<u64>,
    ) -> Result<Box<dyn WriteSeek + 'a>> {
        let inner = self.new_file_non_seek(name, size)?;
        Ok(Box::new(Writer {
            inner,
            mem: MemWriter::new(),
        }))
    }

    fn new_file_non_seek<'a>(
        &'a mut self,
        name: &str,
        size: Option<u64>,
    ) -> Result<Box<dyn Write + 'a>> {
        let mut entry = self
            .headers
            .lock_blocking()
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("File '{}' not found in archive", name))?
            .clone();
        if self.compress {
            let (reader, writer) = std::io::pipe()?;
            let file = self.writer.clone();
            let headers = self.headers.clone();
            let compress_level = self.compress_level;
            let name = name.to_owned();
            let zopfli = self.zopfli;
            let iteration_count = self.zopfli_iteration_count;
            let iterations_without_improvement = self.zopfli_iterations_without_improvement;
            let maximum_block_splits = self.zopfli_maximum_block_splits;
            let data_hash = self.data_hash;
            self.runner.execute(
                move |_| {
                    let mut tsize = 0;
                    let mut reader = TrackStream::new(reader, &mut tsize);
                    let mut data = Vec::new();
                    if entry.compressed {
                        let mut compressed = MemWriter::new();
                        compressed.write_all(b"x\xDA")?;
                        if zopfli {
                            let mut encoder = zopfli::DeflateEncoder::new(
                                zopfli::Options {
                                    iteration_count,
                                    iterations_without_improvement,
                                    maximum_block_splits,
                                },
                                zopfli::BlockType::Dynamic,
                                &mut compressed,
                            );
                            std::io::copy(&mut reader, &mut encoder)?;
                            encoder.finish()?;
                        } else {
                            let mut encoder = flate2::write::DeflateEncoder::new(
                                &mut compressed,
                                flate2::Compression::new(compress_level),
                            );
                            std::io::copy(&mut reader, &mut encoder)?;
                            encoder.finish()?;
                        }
                        data = compressed.into_inner();
                    } else {
                        reader.read_to_end(&mut data)?;
                    }
                    entry.size = tsize as u32;
                    entry.compressed_size = data.len() as u32;
                    if let Some(hash) = entry.hash.as_mut() {
                        let mut hasher: Box<dyn Hasher> = match data_hash {
                            DataHashType::Adler32 => Box::new(adler::Adler32::new()),
                            DataHashType::Murmur2 => {
                                Box::new(StreamingMurmur2::new(0, entry.compressed_size))
                            }
                            DataHashType::Xxh32 => Box::new(Xxh32::new(0)),
                        };
                        hasher.write(&data);
                        *hash = hasher.finish() as u32;
                    }
                    let mut writer = file.lock_blocking();
                    entry.offset = writer.seek(SeekFrom::End(0))?;
                    writer.write_all(&data)?;
                    headers.lock_blocking().insert(name, entry);
                    Ok(())
                },
                true,
            )?;
            Ok(Box::new(writer))
        } else {
            let mut writer = self.writer.lock_blocking();
            entry.offset = writer.seek(SeekFrom::End(0))?;
            Ok(Box::new(YPFArchiveFile {
                entry,
                writer: self.writer.clone(),
                pos: 0,
                headers: self.headers.clone(),
                hasher: if let Some(size) = size {
                    self.create_hasher(size as u32)
                } else {
                    self.create_hasher2()
                },
            }))
        }
    }

    fn write_header(&mut self) -> Result<()> {
        self.runner.join();
        for err in self.runner.take_results() {
            err?;
        }
        let mut writer = self.writer.lock_blocking();
        let headers = self.headers.lock_blocking();
        writer.seek(SeekFrom::Start(0x20))?;
        let mut files = headers.iter().map(|(_, d)| d).collect::<Vec<_>>();
        files.sort_by_key(|f| f.offset);
        let info = &Some(Box::new(self.version) as Box<dyn Any>);
        for file in files {
            file.pack(writer.deref_mut(), false, self.encoding, info)?;
        }
        Ok(())
    }
}

struct YPFArchiveFile<T: Write + Seek> {
    entry: YPFEntry,
    writer: Arc<Mutex<T>>,
    pos: usize,
    headers: Arc<Mutex<HashMap<String, YPFEntry>>>,
    hasher: Box<dyn Hasher + Send + Sync>,
}

impl<T: Write + Seek> Write for YPFArchiveFile<T> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut writer = self.writer.lock().map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::Other, "Failed to lock the mutex")
        })?;
        writer.seek(SeekFrom::Start(self.entry.offset + self.pos as u64))?;
        let bytes_written = writer.write(buf)?;
        self.pos += bytes_written;
        self.entry.size = self.entry.size.max(self.pos as u32);
        self.hasher.write(&buf[..bytes_written]);
        Ok(bytes_written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer
            .lock()
            .map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::Other, "Failed to lock the mutex")
            })?
            .flush()
    }
}

impl<T: Write + Seek> Drop for YPFArchiveFile<T> {
    fn drop(&mut self) {
        self.entry.compressed_size = self.entry.size;
        if let Some(hash) = self.entry.hash.as_mut() {
            *hash = self.hasher.finish() as u32;
        }
        self.headers
            .lock_blocking()
            .insert(self.entry.name.clone(), self.entry.clone());
    }
}

struct Writer<'a> {
    inner: Box<dyn Write + 'a>,
    mem: MemWriter,
}

impl std::fmt::Debug for Writer<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Writer").field("mem", &self.mem).finish()
    }
}

impl<'a> Write for Writer<'a> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.mem.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.mem.flush()
    }
}

impl<'a> Seek for Writer<'a> {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        self.mem.seek(pos)
    }

    fn stream_position(&mut self) -> std::io::Result<u64> {
        self.mem.stream_position()
    }

    fn rewind(&mut self) -> std::io::Result<()> {
        self.mem.rewind()
    }
}

impl<'a> Drop for Writer<'a> {
    fn drop(&mut self) {
        let _ = self.inner.write_all(&self.mem.data);
        let _ = self.inner.flush();
    }
}
