use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use anyhow::Result;
use flate2::read::ZlibDecoder;
use std::io::{Read, Seek, Take};
use std::sync::{Arc, Mutex};
use xp3::XP3Reader;
use xp3::index::file::{IndexSegmentFlag, XP3FileIndex};

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
}

#[derive(Debug)]
/// Kirikiri XP3 Archive
pub struct Xp3Archive<T: Read + Seek + std::fmt::Debug> {
    reader: Arc<Mutex<T>>,
    entries: Vec<(String, XP3FileIndex)>,
}

impl<T: Read + Seek + std::fmt::Debug> Xp3Archive<T> {
    /// Create a new Kirikiri XP3 Archive
    pub fn new(reader: T, _config: &ExtraConfig) -> Result<Self> {
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
        let entry = Entry::new(self.reader.clone(), index);
        Ok(Box::new(entry))
    }
}

struct Entry<T: Read + Seek> {
    reader: Arc<Mutex<T>>,
    index: XP3FileIndex,
    cache: Option<ZlibDecoder<Take<MutexWrapper<T>>>>,
    pos: u64,
    entries_pos: Vec<u64>,
}

impl<T: Read + Seek> Entry<T> {
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
        }
    }
}

impl<T: Read + Seek> ArchiveContent for Entry<T> {
    fn name(&self) -> &str {
        &self.index.info().name()
    }
}

impl<T: Read + Seek> Read for Entry<T> {
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
