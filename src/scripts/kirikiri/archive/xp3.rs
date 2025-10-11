use super::xp3pack::*;
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use anyhow::Result;
use flate2::read::ZlibDecoder;
use overf::wrapping;
use std::io::{Read, Seek, SeekFrom, Take};
use std::sync::{Arc, Mutex};
use xp3::XP3Reader;
use xp3::index::file::{IndexSegmentFlag, XP3FileIndex};

pub use super::xp3pack::SegmenterConfig;

pub fn parse_segmenter_config(str: &str) -> Result<SegmenterConfig> {
    let parts: Vec<&str> = str.split(':').collect();
    if parts.is_empty() {
        return Ok(SegmenterConfig::default());
    }
    match parts[0].to_lowercase().as_str() {
        "none" => Ok(SegmenterConfig::None),
        "cdc" => {
            if parts.len() != 4 {
                return Err(anyhow::anyhow!(
                    "Invalid FastCDC segmenter config. Expected format: fastcdc,min_size,avg_size,max_size"
                ));
            }
            let min_size = parse_size::parse_size(parts[1])?;
            let avg_size = parse_size::parse_size(parts[2])?;
            let max_size = parse_size::parse_size(parts[3])?;
            if min_size == 0 || avg_size == 0 || max_size == 0 {
                return Err(anyhow::anyhow!(
                    "Invalid FastCDC segmenter config. Sizes must be greater than 0."
                ));
            }
            if !(min_size <= avg_size && avg_size <= max_size) {
                return Err(anyhow::anyhow!(
                    "Invalid FastCDC segmenter config. Expected min_size <= avg_size <= max_size."
                ));
            }
            Ok(SegmenterConfig::FastCdc {
                min_size: min_size as u32,
                avg_size: avg_size as u32,
                max_size: max_size as u32,
            })
        }
        "fixed" => {
            if parts.len() != 2 {
                return Err(anyhow::anyhow!(
                    "Invalid Fixed segmenter config. Expected format: fixed,size"
                ));
            }
            let size = parse_size::parse_size(parts[1])?;
            if size == 0 {
                return Err(anyhow::anyhow!(
                    "Invalid Fixed segmenter config. Size must be greater than 0."
                ));
            }
            Ok(SegmenterConfig::Fixed(size as usize))
        }
        _ => Err(anyhow::anyhow!("Unknown segmenter type: {}", parts[0])),
    }
}

#[derive(Debug)]
/// Builder for Kirikiri XP3 Archive
pub struct Xp3ArchiveBuilder {}

impl Xp3ArchiveBuilder {
    /// Create a new Kirikiri XP3 Archive Builder
    pub fn new() -> Self {
        Self {}
    }
}

impl ScriptBuilder for Xp3ArchiveBuilder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Utf8
    }

    fn default_archive_encoding(&self) -> Option<Encoding> {
        Some(Encoding::Utf8)
    }

    fn build_script(
        &self,
        buf: Vec<u8>,
        _filename: &str,
        _encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(Xp3Archive::new(MemReader::new(buf), config)?))
    }

    fn build_script_from_file(
        &self,
        filename: &str,
        _encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        let file = std::fs::File::open(filename)?;
        Ok(Box::new(Xp3Archive::new(file, config)?))
    }

    fn build_script_from_reader(
        &self,
        reader: Box<dyn ReadSeek>,
        _filename: &str,
        _encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(Xp3Archive::new(reader, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["xp3"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::KirikiriXp3
    }

    fn is_archive(&self) -> bool {
        true
    }

    fn create_archive(
        &self,
        filename: &str,
        files: &[&str],
        _encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<Box<dyn Archive>> {
        Ok(Box::new(Xp3ArchiveWriter::new(filename, files, config)?))
    }
}

#[derive(Debug)]
/// Kirikiri XP3 Archive
pub struct Xp3Archive<T: Read + Seek + std::fmt::Debug> {
    reader: Arc<Mutex<T>>,
    entries: Vec<(String, XP3FileIndex)>,
    decrypt_simple_crypt: bool,
    decompress_mdf: bool,
}

impl<T: Read + Seek + std::fmt::Debug> Xp3Archive<T> {
    /// Create a new Kirikiri XP3 Archive
    pub fn new(reader: T, config: &ExtraConfig) -> Result<Self> {
        let xp3_reader = XP3Reader::open_archive(reader)
            .map_err(|e| anyhow::anyhow!("Failed to open XP3 archive: {:?}", e))?;
        let entries = xp3_reader
            .entries()
            .filter_map(|(i, d)| {
                // Skip garbage files
                if i.find("$$$ This is a protected archive. $$$").is_some()
                    || (i.to_lowercase().ends_with(".nene") && d.info().file_size() == 0)
                {
                    None
                } else {
                    Some((i.clone(), d.clone()))
                }
            })
            .collect();
        Ok(Self {
            reader: Arc::new(Mutex::new(xp3_reader.close().1)),
            entries,
            decrypt_simple_crypt: config.xp3_simple_crypt,
            decompress_mdf: config.xp3_mdf_decompress,
        })
    }
}

impl<T: Read + Seek + std::fmt::Debug + 'static> Script for Xp3Archive<T> {
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
            self.entries.iter().map(|entry| Ok(entry.0.clone())),
        ))
    }

    fn open_file<'a>(&'a self, index: usize) -> Result<Box<dyn ArchiveContent + 'a>> {
        let index = self
            .entries
            .iter()
            .nth(index)
            .ok_or(anyhow::anyhow!("Index out of bounds: {}", index))?
            .1
            .clone();
        let mut entry = Entry::new(self.reader.clone(), index);
        let mut header = [0u8; 16];
        let header_len = entry.read(&mut header)?;
        entry.rewind()?;
        entry.script_type = detect_script_type(entry.index.info().name(), &header, header_len);
        if self.decrypt_simple_crypt
            && header_len >= 5
            && header[0] == 0xFE
            && header[1] == 0xFE
            && header[3] == 0xFF
            && header[4] == 0xFE
        {
            let crypt = header[2];
            if crypt == 2 {
                let index = entry.index.clone();
                return Ok(Box::new(SimpleCryptZlib::new(entry, index)?));
            }
            if matches!(crypt, 0 | 1) {
                let index = entry.index.clone();
                return Ok(Box::new(SimpleCrypt::new(entry, index, crypt)?));
            }
        }
        if self.decompress_mdf
            && header_len >= 4
            && &header[0..4] == b"mdf\0"
            && entry.index.info().file_size() > 8
        {
            let index = entry.index.clone();
            return Ok(Box::new(MdfEntry::new(entry, index)?));
        }
        Ok(Box::new(entry))
    }
}

fn detect_script_type(filename: &str, buf: &[u8], buf_len: usize) -> Option<ScriptType> {
    #[cfg(feature = "kirikiri-img")]
    if buf_len >= 11 && libtlg_rs::is_valid_tlg(buf) {
        return Some(ScriptType::KirikiriTlg);
    }
    if buf_len >= 8 && (buf.starts_with(b"TJS/ns0\0") || buf.starts_with(b"TJS/4s0\0")) {
        return Some(ScriptType::KirikiriTjsNs0);
    }
    if buf_len >= 8 && buf.starts_with(b"TJS2100\0") {
        return Some(ScriptType::KirikiriTjs2);
    }
    let extension = std::path::Path::new(filename)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    match extension.as_str() {
        "ks" => Some(ScriptType::Kirikiri),
        "scn" => Some(ScriptType::KirikiriScn),
        #[cfg(feature = "emote-img")]
        "dref" => Some(ScriptType::EmoteDref),
        #[cfg(feature = "emote-img")]
        "pimg" => Some(ScriptType::EmotePimg),
        _ => None,
    }
}

#[derive(Debug)]
struct Entry<T: Read + Seek + std::fmt::Debug> {
    reader: Arc<Mutex<T>>,
    index: XP3FileIndex,
    cache: Option<ZlibDecoder<Take<MutexWrapper<T>>>>,
    pos: u64,
    entries_pos: Vec<u64>,
    script_type: Option<ScriptType>,
}

impl<T: Read + Seek + std::fmt::Debug> Entry<T> {
    fn new(reader: Arc<Mutex<T>>, index: XP3FileIndex) -> Self {
        let mut pos = 0;
        let entries_pos = index
            .segments()
            .iter()
            .map(|seg| {
                let p = pos;
                pos += seg.original_size();
                p
            })
            .collect();
        Self {
            reader,
            index,
            cache: None,
            pos: 0,
            entries_pos,
            script_type: None,
        }
    }
}

impl<T: Read + Seek + std::fmt::Debug> ArchiveContent for Entry<T> {
    fn name(&self) -> &str {
        &self.index.info().name()
    }

    fn to_data<'a>(&'a mut self) -> Result<Box<dyn ReadSeek + 'a>> {
        Ok(Box::new(self))
    }

    fn script_type(&self) -> Option<&ScriptType> {
        self.script_type.as_ref()
    }
}

impl<T: Read + Seek + std::fmt::Debug> Read for Entry<T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.pos >= self.index.info().file_size() {
            self.cache.take();
            return Ok(0);
        }
        if let Some(cache) = self.cache.as_mut() {
            let readed = cache.read(buf)?;
            if readed > 0 {
                self.pos += readed as u64;
                return Ok(readed);
            }
            self.cache.take();
        }
        let seg_index = match self.entries_pos.binary_search(&self.pos) {
            Ok(i) => i,
            Err(i) => {
                if i == 0 {
                    0
                } else {
                    i - 1
                }
            }
        };
        let seg = &self.index.segments()[seg_index];
        let start_pos = seg.data_offset();
        let seg_pos = self.entries_pos[seg_index];
        let skip_pos = self.pos - seg_pos;
        let read_size = seg.saved_size();
        match seg.flag() {
            IndexSegmentFlag::UnCompressed => {
                let mut lock = MutexWrapper::new(self.reader.clone(), start_pos + skip_pos);
                let readed = (&mut lock).take(read_size - skip_pos).read(buf)?;
                self.pos += readed as u64;
                Ok(readed)
            }
            IndexSegmentFlag::Compressed => {
                let mut cache = ZlibDecoder::new(
                    MutexWrapper::new(self.reader.clone(), start_pos).take(read_size),
                );
                if skip_pos != 0 {
                    let mut e = EmptyWriter::new();
                    std::io::copy(&mut (&mut cache).take(skip_pos), &mut e)?; // skip
                }
                let readed = cache.read(buf)?;
                self.pos += readed as u64;
                self.cache = Some(cache);
                Ok(readed)
            }
        }
    }
}

impl<T: Read + Seek + std::fmt::Debug> Seek for Entry<T> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let new_pos = match pos {
            SeekFrom::Start(p) => p,
            SeekFrom::End(offset) => {
                if offset < 0 {
                    if (-offset) as u64 > self.index.info().file_size() {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "Seek from end exceeds file length",
                        ));
                    }
                    self.index.info().file_size() - (-offset) as u64
                } else {
                    self.index.info().file_size() + offset as u64
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
        if let Some(cache) = self.cache.as_mut() {
            let old_seg_index = match self.entries_pos.binary_search(&self.pos) {
                Ok(i) => i,
                Err(i) => {
                    if i == 0 {
                        0
                    } else {
                        i - 1
                    }
                }
            };
            let new_seg_index = match self.entries_pos.binary_search(&new_pos) {
                Ok(i) => i,
                Err(i) => {
                    if i == 0 {
                        0
                    } else {
                        i - 1
                    }
                }
            };
            if old_seg_index != new_seg_index {
                self.cache.take();
            } else {
                if new_pos >= self.pos {
                    let skip_pos = new_pos - self.pos;
                    let mut e = EmptyWriter::new();
                    std::io::copy(&mut cache.take(skip_pos), &mut e)?; // skip
                } else {
                    self.cache.take();
                }
            }
        }
        self.pos = new_pos;
        Ok(self.pos)
    }

    fn rewind(&mut self) -> std::io::Result<()> {
        self.pos = 0;
        self.cache.take();
        Ok(())
    }

    fn stream_position(&mut self) -> std::io::Result<u64> {
        Ok(self.pos)
    }
}

struct SimpleCryptZlib<T: Read + Seek + std::fmt::Debug> {
    inner: PrefixStream<ZlibDecoder<StreamRegion<Entry<T>>>>,
    index: XP3FileIndex,
}

impl<T: Read + Seek + std::fmt::Debug> SimpleCryptZlib<T> {
    fn new(mut entry: Entry<T>, index: XP3FileIndex) -> Result<Self> {
        entry.seek(SeekFrom::Start(0x15))?;
        let entry = StreamRegion::new(entry, 0x15, index.info().file_size())?;
        let inner = PrefixStream::new(vec![0xFF, 0xFE], ZlibDecoder::new(entry));
        Ok(Self { inner, index })
    }
}

impl<T: Read + Seek + std::fmt::Debug> ArchiveContent for SimpleCryptZlib<T> {
    fn name(&self) -> &str {
        &self.index.info().name()
    }
}

impl<T: Read + Seek + std::fmt::Debug> Read for SimpleCryptZlib<T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}

#[derive(Debug)]
struct SimpleCryptInner<T: Read + Seek + std::fmt::Debug> {
    inner: StreamRegion<Entry<T>>,
    crypt: u8,
}

impl<T: Read + Seek + std::fmt::Debug> SimpleCryptInner<T> {
    fn new(mut entry: Entry<T>, crypt: u8) -> Result<Self> {
        entry.seek(SeekFrom::Start(5))?;
        let size = entry.index.info().file_size();
        let entry = StreamRegion::new(entry, 5, size)?;
        Ok(Self {
            inner: entry,
            crypt,
        })
    }
}

impl<T: Read + Seek + std::fmt::Debug> Read for SimpleCryptInner<T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.inner.read(buf)?;
        match self.crypt {
            0 => {
                for b in &mut buf[..readed] {
                    let ch = *b as u16;
                    if ch >= 20 {
                        *b = wrapping! {ch ^ (((ch & 0xfe) << 8) ^ 1)} as u8;
                    }
                }
            }
            1 => {
                for b in &mut buf[..readed] {
                    let mut ch = *b as u32;
                    ch = wrapping! {((ch & 0xaaaaaaaa) >> 1) | ((ch & 0x55555555) << 1)};
                    *b = ch as u8;
                }
            }
            _ => {}
        }
        Ok(readed)
    }
}

impl<T: Read + Seek + std::fmt::Debug> Seek for SimpleCryptInner<T> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(pos)
    }

    fn rewind(&mut self) -> std::io::Result<()> {
        self.inner.rewind()
    }

    fn stream_position(&mut self) -> std::io::Result<u64> {
        self.inner.stream_position()
    }
}

#[derive(Debug)]
struct SimpleCrypt<T: Read + Seek + std::fmt::Debug> {
    inner: PrefixStream<SimpleCryptInner<T>>,
    index: XP3FileIndex,
}

impl<T: Read + Seek + std::fmt::Debug> SimpleCrypt<T> {
    fn new(entry: Entry<T>, index: XP3FileIndex, crypt: u8) -> Result<Self> {
        let inner = PrefixStream::new(vec![0xFF, 0xFE], SimpleCryptInner::new(entry, crypt)?);
        Ok(Self { inner, index })
    }
}

impl<T: Read + Seek + std::fmt::Debug> ArchiveContent for SimpleCrypt<T> {
    fn name(&self) -> &str {
        &self.index.info().name()
    }

    fn to_data<'a>(&'a mut self) -> Result<Box<dyn ReadSeek + 'a>> {
        Ok(Box::new(self))
    }
}

impl<T: Read + Seek + std::fmt::Debug> Read for SimpleCrypt<T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}

impl<T: Read + Seek + std::fmt::Debug> Seek for SimpleCrypt<T> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(pos)
    }

    fn rewind(&mut self) -> std::io::Result<()> {
        self.inner.rewind()
    }

    fn stream_position(&mut self) -> std::io::Result<u64> {
        self.inner.stream_position()
    }
}

#[derive(Debug)]
struct MdfEntry<T: Read + Seek + std::fmt::Debug> {
    inner: ZlibDecoder<StreamRegion<Entry<T>>>,
    index: XP3FileIndex,
}

impl<T: Read + Seek + std::fmt::Debug> MdfEntry<T> {
    fn new(mut entry: Entry<T>, index: XP3FileIndex) -> Result<Self> {
        entry.seek(SeekFrom::Start(8))?;
        let entry = StreamRegion::new(entry, 8, index.info().file_size())?;
        let inner = ZlibDecoder::new(entry);
        Ok(Self { inner, index })
    }
}

impl<T: Read + Seek + std::fmt::Debug> ArchiveContent for MdfEntry<T> {
    fn name(&self) -> &str {
        &self.index.info().name()
    }
}

impl<T: Read + Seek + std::fmt::Debug> Read for MdfEntry<T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}
