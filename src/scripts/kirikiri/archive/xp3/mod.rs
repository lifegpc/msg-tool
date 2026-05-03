mod archive;
#[allow(dead_code)]
mod consts;
mod crypt;
mod read;
mod reader;
mod segmenter;
mod writer;

use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use anyhow::Result;
use consts::ZSTD_SIGNATURE;
use crypt::Crypt;
pub use crypt::get_supported_games;
pub use crypt::get_supported_games_with_title;
use flate2::read::ZlibDecoder;
use overf::wrapping;
pub use segmenter::SegmenterConfig;
use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::{Arc, Mutex};
use writer::Xp3ArchiveWriter;
use zstd::stream::read::Decoder as ZstdDecoder;

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
        filename: &str,
        _encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script + Send + Sync>> {
        Ok(Box::new(Xp3Archive::new(
            MemReader::new(buf),
            config,
            filename,
        )?))
    }

    fn build_script_from_file(
        &self,
        filename: &str,
        _encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script + Send + Sync>> {
        let file = std::fs::File::open(filename)?;
        Ok(Box::new(Xp3Archive::new(file, config, filename)?))
    }

    fn build_script_from_reader<'a>(
        &self,
        reader: Box<dyn ReadSeek + Send + Sync + 'a>,
        filename: &str,
        _encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script + Send + Sync + 'a>> {
        Ok(Box::new(Xp3Archive::new(reader, config, filename)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["xp3", "bin", "dat"]
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

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 11 && buf.starts_with(consts::XP3_MAGIC) {
            return Some(100);
        }
        None
    }
}

#[derive(Debug)]
/// Kirikiri XP3 Archive
pub struct Xp3Archive<'a> {
    archive: archive::Xp3Archive<'a>,
    decrypt_simple_crypt: bool,
    decompress_mdf: bool,
    force_extract: bool,
    force_decrypt: bool,
}

impl<'a> Xp3Archive<'a> {
    pub fn new<T: Read + Seek + std::fmt::Debug + Send + Sync + 'a>(
        stream: T,
        config: &ExtraConfig,
        filename: &str,
    ) -> Result<Self> {
        let mut archive = archive::Xp3Archive::new(stream, config, filename)?;
        if config.xp3_debug_archive {
            println!("Debug info for {}:\n{:#?}", filename, archive);
            // Try flush stdout.
            let _ = std::io::stdout().flush();
        }
        archive.entries.retain(|entry| {
            let i = &entry.name;
            !(i.find("$$$ This is a protected archive. $$$").is_some()
                // Fate/stay night has spelling mistake. We also filter it.
                || i.find("$$$ This is a protectet archive. $$$").is_some()
                || (i.to_lowercase().ends_with(".nene") && entry.original_size == 0))
        });
        Ok(Self {
            archive,
            decrypt_simple_crypt: config.xp3_simple_crypt,
            decompress_mdf: config.xp3_mdf_decompress,
            force_extract: config.xp3_force_extract,
            force_decrypt: config.xp3_force_decrypt,
        })
    }
}

impl<'b> Script for Xp3Archive<'b> {
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
            self.archive
                .entries
                .iter()
                .map(|entry| Ok(entry.name.clone())),
        ))
    }

    fn open_file<'a>(&'a self, index: usize) -> Result<Box<dyn ArchiveContent + Send + Sync + 'a>> {
        let index = self
            .archive
            .entries
            .iter()
            .nth(index)
            .ok_or(anyhow::anyhow!("Index out of bounds: {}", index))?
            .clone();
        let crypt = self.archive.crypt.clone();
        let skip_decrypt = index.is_encrypted() && !crypt.decrypt_supported();
        if skip_decrypt {
            if !self.force_extract {
                return Err(anyhow::anyhow!(
                    "The archive is encrypted with a method that is not supported by the current crypt implementation. You may need to specify a game title by using --xp3-game-title <title>."
                ));
            }
        }
        let mut entry = Entry::new(
            self.archive.inner.clone(),
            index,
            self.archive.base_offset,
            crypt,
            skip_decrypt,
            self.force_decrypt,
        );
        let mut header = [0u8; 16];
        let header_len = entry.read(&mut header)?;
        entry.rewind()?;
        entry.script_type = detect_script_type(&entry.index.name, &header, header_len);
        if self
            .archive
            .crypt
            .need_filter(&entry.index.name, &header, header_len)
        {
            if self.archive.crypt.filter_seek_supported() {
                let index = entry.index.clone();
                let mut result = self.archive.crypt.filter_with_seek(entry)?;
                let header_len = result.read(&mut header)?;
                result.rewind()?;
                let script_type = detect_script_type(&index.name, &header, header_len);
                return Ok(Box::new(CustomFilterWithSeekEntry {
                    inner: result,
                    index,
                    script_type,
                }));
            } else {
                let index = entry.index.clone();
                let mut result = self.archive.crypt.filter(entry)?;
                let header_len = result.read(&mut header)?;
                let script_type = detect_script_type(&index.name, &header, header_len);
                let prefix = header[..header_len].to_vec();
                return Ok(Box::new(CustomFilterEntry {
                    inner: PrefixStream::new(prefix, result),
                    index,
                    script_type,
                }));
            }
        }
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
            && entry.index.original_size > 8
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

struct Entry<'a> {
    reader: Arc<Mutex<Box<dyn ReadSeek + Send + Sync + 'a>>>,
    index: archive::Xp3Entry,
    crypt: Arc<Box<dyn Crypt + Send + Sync>>,
    /// used to cache segment reader that can't seek. Such as decompressor reader or some decrypter reader.
    cache: Option<Box<dyn Read + Send + Sync + 'a>>,
    /// used to store decrypted stream of current segment when the cryptor support seek when decrypting.
    crypt_stream: Option<Box<dyn ReadSeek + Send + Sync + 'a>>,
    pos: u64,
    base_offset: u64,
    entries_pos: Vec<u64>,
    script_type: Option<ScriptType>,
    skip_decrypt: bool,
    force_decrypt: bool,
}

#[automatically_derived]
impl<'a> std::fmt::Debug for Entry<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Entry")
            .field("reader", &self.reader)
            .field("index", &self.index)
            .field("crypt", &self.crypt)
            .field("cache", &self.cache.is_some())
            .field("crypt_stream", &self.crypt_stream)
            .field("pos", &self.pos)
            .field("base_offset", &self.base_offset)
            .field("entries_pos", &self.entries_pos)
            .field("script_type", &self.script_type)
            .field("skip_decrypt", &self.skip_decrypt)
            .finish()
    }
}

impl<'a> Entry<'a> {
    fn new(
        reader: Arc<Mutex<Box<dyn ReadSeek + Send + Sync + 'a>>>,
        index: archive::Xp3Entry,
        base_offset: u64,
        crypt: Arc<Box<dyn Crypt + Send + Sync>>,
        skip_decrypt: bool,
        force_decrypt: bool,
    ) -> Self {
        let mut pos = 0;
        let entries_pos = index
            .segments
            .iter()
            .map(|seg| {
                let p = pos;
                pos += seg.original_size;
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
            base_offset,
            crypt,
            crypt_stream: None,
            skip_decrypt,
            force_decrypt,
        }
    }

    fn new2(
        reader: Arc<Mutex<Box<dyn ReadSeek + Send + Sync + 'a>>>,
        index: archive::Xp3Entry,
        base_offset: u64,
        crypt: Arc<Box<dyn Crypt + Send + Sync>>,
    ) -> Self {
        Self::new(reader, index, base_offset, crypt, false, false)
    }
}

impl<'b> ArchiveContent for Entry<'b> {
    fn name(&self) -> &str {
        &self.index.name
    }

    fn to_data<'a>(&'a mut self) -> Result<Box<dyn ReadSeek + Send + Sync + 'a>> {
        Ok(Box::new(self))
    }

    fn script_type(&self) -> Option<&ScriptType> {
        self.script_type.as_ref()
    }
}

impl<'a> Read for Entry<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.pos >= self.index.original_size {
            self.cache.take();
            self.crypt_stream.take();
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
        if let Some(crypt_stream) = self.crypt_stream.as_mut() {
            let readed = crypt_stream.read(buf)?;
            if readed > 0 {
                self.pos += readed as u64;
                return Ok(readed);
            }
            self.crypt_stream.take();
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
        let seg = &self.index.segments[seg_index];
        let start_pos = seg.start + self.base_offset;
        let seg_pos = self.entries_pos[seg_index];
        let skip_pos = self.pos - seg_pos;
        let read_size = seg.archived_size;
        if !self.skip_decrypt
            && (self.index.is_encrypted() || (self.force_decrypt && self.crypt.decrypt_supported()))
        {
            if seg.is_compressed || !self.crypt.decrypt_seek_supported() {
                let mut cache: Box<dyn Read + Send + Sync> = if seg.is_compressed {
                    let mut inner =
                        MutexWrapper::new(self.reader.clone(), start_pos).take(read_size);
                    let decompressed = if inner.peek_and_equal(ZSTD_SIGNATURE).is_ok() {
                        Box::new(ZstdDecoder::new(inner)?) as Box<dyn Read + Send + Sync>
                    } else {
                        Box::new(ZlibDecoder::new(inner)) as Box<dyn Read + Send + Sync>
                    };
                    let decrypted =
                        self.crypt
                            .decrypt(&self.index, seg, decompressed)
                            .map_err(|e| {
                                std::io::Error::new(
                                    std::io::ErrorKind::Other,
                                    format!("Decryption failed: {}", e),
                                )
                            })?;
                    Box::new(decrypted) as Box<dyn Read + Send + Sync>
                } else {
                    let inner = MutexWrapper::new(self.reader.clone(), start_pos).take(read_size);
                    let decrypted = self
                        .crypt
                        .decrypt(&self.index, seg, Box::new(inner))
                        .map_err(|e| {
                            std::io::Error::new(
                                std::io::ErrorKind::Other,
                                format!("Decryption failed: {}", e),
                            )
                        })?;
                    Box::new(decrypted) as Box<dyn Read + Send + Sync>
                };
                if skip_pos != 0 {
                    let mut e = EmptyWriter::new();
                    std::io::copy(&mut (&mut cache).take(skip_pos), &mut e)?; // skip
                }
                let readed = cache.read(buf)?;
                self.pos += readed as u64;
                self.cache = Some(cache);
                return Ok(readed);
            } else {
                let inner = MutexWrapper::new(self.reader.clone(), start_pos).take(read_size);
                let mut decrypted = self
                    .crypt
                    .decrypt_with_seek(&self.index, seg, Box::new(inner))
                    .map_err(|e| {
                        std::io::Error::new(
                            std::io::ErrorKind::Other,
                            format!("Decryption failed: {}", e),
                        )
                    })?;
                if skip_pos != 0 {
                    let mut e = EmptyWriter::new();
                    std::io::copy(&mut (&mut decrypted).take(skip_pos), &mut e)?; // skip
                }
                let readed = decrypted.read(buf)?;
                self.pos += readed as u64;
                self.crypt_stream = Some(decrypted);
                return Ok(readed);
            }
        }
        if seg.is_compressed {
            let mut inner = MutexWrapper::new(self.reader.clone(), start_pos).take(read_size);
            let mut cache = if inner.peek_and_equal(ZSTD_SIGNATURE).is_ok() {
                Box::new(ZstdDecoder::new(inner)?) as Box<dyn Read + Send + Sync>
            } else {
                Box::new(ZlibDecoder::new(inner)) as Box<dyn Read + Send + Sync>
            };
            if skip_pos != 0 {
                let mut e = EmptyWriter::new();
                std::io::copy(&mut (&mut cache).take(skip_pos), &mut e)?; // skip
            }
            let readed = cache.read(buf)?;
            self.pos += readed as u64;
            self.cache = Some(cache);
            Ok(readed)
        } else {
            let mut lock = MutexWrapper::new(self.reader.clone(), start_pos + skip_pos);
            let readed = (&mut lock).take(read_size - skip_pos).read(buf)?;
            self.pos += readed as u64;
            Ok(readed)
        }
    }
}

impl<'a> Seek for Entry<'a> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let new_pos = match pos {
            SeekFrom::Start(p) => p,
            SeekFrom::End(offset) => {
                if offset < 0 {
                    if (-offset) as u64 > self.index.original_size {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "Seek from end exceeds file length",
                        ));
                    }
                    self.index.original_size - (-offset) as u64
                } else {
                    self.index.original_size + offset as u64
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
        if let Some(crypt_stream) = self.crypt_stream.as_mut() {
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
                self.crypt_stream.take();
            } else {
                let offset = new_pos as i64 - self.pos as i64;
                crypt_stream.seek(SeekFrom::Current(offset))?;
            }
        }
        self.pos = new_pos;
        Ok(self.pos)
    }

    fn rewind(&mut self) -> std::io::Result<()> {
        self.pos = 0;
        self.cache.take();
        self.crypt_stream.take();
        Ok(())
    }

    fn stream_position(&mut self) -> std::io::Result<u64> {
        Ok(self.pos)
    }
}

struct SimpleCryptZlib<'a> {
    inner: PrefixStream<ZlibDecoder<StreamRegion<Entry<'a>>>>,
    index: archive::Xp3Entry,
}

impl<'a> SimpleCryptZlib<'a> {
    fn new(mut entry: Entry<'a>, index: archive::Xp3Entry) -> Result<Self> {
        entry.seek(SeekFrom::Start(0x15))?;
        let entry = StreamRegion::new(entry, 0x15, index.original_size)?;
        let inner = PrefixStream::new(vec![0xFF, 0xFE], ZlibDecoder::new(entry));
        Ok(Self { inner, index })
    }
}

impl<'a> ArchiveContent for SimpleCryptZlib<'a> {
    fn name(&self) -> &str {
        &self.index.name
    }
}

impl<'a> Read for SimpleCryptZlib<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}

#[derive(Debug)]
struct SimpleCryptInner<'a> {
    inner: StreamRegion<Entry<'a>>,
    crypt: u8,
}

impl<'a> SimpleCryptInner<'a> {
    fn new(mut entry: Entry<'a>, crypt: u8) -> Result<Self> {
        entry.seek(SeekFrom::Start(5))?;
        let size = entry.index.original_size;
        let entry = StreamRegion::new(entry, 5, size)?;
        Ok(Self {
            inner: entry,
            crypt,
        })
    }
}

impl<'a> Read for SimpleCryptInner<'a> {
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

impl<'a> Seek for SimpleCryptInner<'a> {
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
struct SimpleCrypt<'a> {
    inner: PrefixStream<SimpleCryptInner<'a>>,
    index: archive::Xp3Entry,
}

impl<'a> SimpleCrypt<'a> {
    fn new(entry: Entry<'a>, index: archive::Xp3Entry, crypt: u8) -> Result<Self> {
        let inner = PrefixStream::new(vec![0xFF, 0xFE], SimpleCryptInner::new(entry, crypt)?);
        Ok(Self { inner, index })
    }
}

impl<'b> ArchiveContent for SimpleCrypt<'b> {
    fn name(&self) -> &str {
        &self.index.name
    }

    fn to_data<'a>(&'a mut self) -> Result<Box<dyn ReadSeek + Send + Sync + 'a>> {
        Ok(Box::new(self))
    }
}

impl<'a> Read for SimpleCrypt<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}

impl<'a> Seek for SimpleCrypt<'a> {
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
struct MdfEntry<'a> {
    inner: ZlibDecoder<StreamRegion<Entry<'a>>>,
    index: archive::Xp3Entry,
}

impl<'a> MdfEntry<'a> {
    fn new(mut entry: Entry<'a>, index: archive::Xp3Entry) -> Result<Self> {
        entry.seek(SeekFrom::Start(8))?;
        let entry = StreamRegion::new(entry, 8, index.original_size)?;
        let inner = ZlibDecoder::new(entry);
        Ok(Self { inner, index })
    }
}

impl<'a> ArchiveContent for MdfEntry<'a> {
    fn name(&self) -> &str {
        &self.index.name
    }
}

impl<'a> Read for MdfEntry<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}

#[derive(Debug)]
struct CustomFilterEntry<'a> {
    inner: PrefixStream<Box<dyn ReadDebug + Send + Sync + 'a>>,
    index: archive::Xp3Entry,
    script_type: Option<ScriptType>,
}

impl<'a> Read for CustomFilterEntry<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}

impl<'a> ArchiveContent for CustomFilterEntry<'a> {
    fn name(&self) -> &str {
        &self.index.name
    }

    fn script_type(&self) -> Option<&ScriptType> {
        self.script_type.as_ref()
    }
}

#[derive(Debug)]
struct CustomFilterWithSeekEntry<'a> {
    inner: Box<dyn ReadSeek + Send + Sync + 'a>,
    index: archive::Xp3Entry,
    script_type: Option<ScriptType>,
}

impl<'a> Read for CustomFilterWithSeekEntry<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}

impl<'a> Seek for CustomFilterWithSeekEntry<'a> {
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

impl<'b> ArchiveContent for CustomFilterWithSeekEntry<'b> {
    fn name(&self) -> &str {
        &self.index.name
    }

    fn script_type(&self) -> Option<&ScriptType> {
        self.script_type.as_ref()
    }

    fn to_data<'a>(&'a mut self) -> Result<Box<dyn ReadSeek + Send + Sync + 'a>> {
        Ok(Box::new(self))
    }
}
