//! Yu-Ris Archive (.ypf)
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use crate::utils::murmur2::*;
use anyhow::{Result, anyhow, bail};
use clap::ValueEnum;
use int_enum::IntEnum;
use std::hash::Hasher;
use std::io::{Read, Seek, SeekFrom};
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
        Ok(Box::new(YPF::new(
            MemReader::new(data),
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
    ) -> Result<Box<dyn Script + Send + Sync>> {
        if filename == "-" {
            let data = crate::utils::files::read_file(filename)?;
            Ok(Box::new(YPF::new(
                MemReader::new(data),
                archive_encoding,
                config,
            )?))
        } else {
            let f = std::fs::File::open(filename)?;
            let reader = std::io::BufReader::new(f);
            Ok(Box::new(YPF::new(reader, archive_encoding, config)?))
        }
    }

    fn build_script_from_reader<'a>(
        &self,
        reader: Box<dyn ReadSeek + Send + Sync + 'a>,
        _filename: &str,
        _encoding: Encoding,
        archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script + Send + Sync + 'a>> {
        Ok(Box::new(YPF::new(reader, archive_encoding, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ypf"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::YurisYPF
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 4 && buf.starts_with(b"YPF\0") {
            return Some(20);
        }
        None
    }

    fn is_archive(&self) -> bool {
        true
    }
}

#[repr(u8)]
#[derive(Debug, IntEnum)]
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

#[derive(Debug)]
struct YPFEntry {
    name: String,
    #[allow(unused)]
    typ: ResourceType,
    compressed: bool,
    size: u32,
    compressed_size: u32,
    offset: u64,
    hash: Option<u32>,
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
// #TODO: Whirlpool Relirium has another hash type
pub enum DataHashType {
    /// Adler32
    Adler32,
    /// Murmur2
    Murmur2,
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
    let mut buf = [0; 1024];
    loop {
        let readed = stream.read(&mut buf)?;
        if readed == 0 {
            break;
        }
        let b = &buf[..readed];
        murmur2_hasher.write(b);
        adler32_hasher.write(b);
    }
    if murmur2_hasher.finish() as u32 == expected {
        return Ok(DataHashType::Murmur2);
    }
    if adler32_hasher.finish() as u32 == expected {
        return Ok(DataHashType::Adler32);
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
    pub fn new(mut reader: T, archive_encoding: Encoding, config: &ExtraConfig) -> Result<Self> {
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
