//! Circus Archive File (.dat)
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use anyhow::Result;
use std::io::{Read, Seek, SeekFrom};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
/// Circus DAT Archive Builder
pub struct DatArchiveBuilder {}

impl DatArchiveBuilder {
    /// Creates a new instance of `DatArchiveBuilder`.
    pub fn new() -> Self {
        Self {}
    }
}

impl ScriptBuilder for DatArchiveBuilder {
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
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(DatArchive::new(
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
    ) -> Result<Box<dyn Script>> {
        if filename == "-" {
            let data = crate::utils::files::read_file(filename)?;
            Ok(Box::new(DatArchive::new(
                MemReader::new(data),
                archive_encoding,
                config,
            )?))
        } else {
            let f = std::fs::File::open(filename)?;
            let reader = std::io::BufReader::new(f);
            Ok(Box::new(DatArchive::new(reader, archive_encoding, config)?))
        }
    }

    fn build_script_from_reader(
        &self,
        reader: Box<dyn ReadSeek>,
        _filename: &str,
        _encoding: Encoding,
        archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(DatArchive::new(reader, archive_encoding, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["dat"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::CircusDat
    }

    fn is_archive(&self) -> bool {
        true
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        is_this_format(&buf[..buf_len]).ok()
    }
}

#[derive(Debug, Clone)]
struct DatFileHeader {
    name: String,
    offset: u32,
    size: u32,
}

struct Entry<T: Read + Seek> {
    header: DatFileHeader,
    reader: Arc<Mutex<T>>,
    pos: usize,
    script_type: Option<ScriptType>,
}

impl<T: Read + Seek> ArchiveContent for Entry<T> {
    fn name(&self) -> &str {
        &self.header.name
    }

    fn script_type(&self) -> Option<&ScriptType> {
        self.script_type.as_ref()
    }
}

impl<T: Read + Seek> Read for Entry<T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut reader = self.reader.lock().map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to lock mutex: {}", e),
            )
        })?;
        reader.seek(SeekFrom::Start(self.header.offset as u64 + self.pos as u64))?;
        let bytes_read = buf.len().min(self.header.size as usize - self.pos);
        if bytes_read == 0 {
            return Ok(0);
        }
        let bytes_read = reader.read(&mut buf[..bytes_read])?;
        self.pos += bytes_read;
        Ok(bytes_read)
    }
}

impl<T: Read + Seek> Seek for Entry<T> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as usize,
            SeekFrom::End(offset) => {
                if offset < 0 {
                    if (-offset) as usize > self.header.size as usize {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "Seek from end exceeds file length",
                        ));
                    }
                    self.header.size as usize - (-offset) as usize
                } else {
                    self.header.size as usize + offset as usize
                }
            }
            SeekFrom::Current(offset) => {
                if offset < 0 {
                    if (-offset) as usize > self.pos {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "Seek from current exceeds current position",
                        ));
                    }
                    self.pos.saturating_sub((-offset) as usize)
                } else {
                    self.pos + offset as usize
                }
            }
        };
        self.pos = new_pos;
        Ok(self.pos as u64)
    }

    fn stream_position(&mut self) -> std::io::Result<u64> {
        Ok(self.pos as u64)
    }
}

#[derive(Debug)]
/// Extra information for the DAT archive.
pub struct DatExtraInfo {
    /// Maximum length of file names in the DAT archive.
    pub name_len: usize,
}

#[derive(Debug)]
/// Circus DAT Archive
pub struct DatArchive<T: Read + Seek + std::fmt::Debug> {
    reader: Arc<Mutex<T>>,
    entries: Vec<DatFileHeader>,
    name_len: usize,
}

const NAME_LEN: [usize; 3] = [0x24, 0x30, 0x3C];

impl<T: Read + Seek + std::fmt::Debug> DatArchive<T> {
    /// Creates a new `DatArchive` from a reader.
    ///
    /// * `reader` - The reader to read the DAT archive from.
    /// * `encoding` - The encoding to use for string fields.
    /// * `config` - Extra configuration options.
    pub fn new(mut reader: T, encoding: Encoding, _config: &ExtraConfig) -> Result<Self> {
        let (name_len, entries) = Self::read_all_index(&mut reader, encoding)?;
        let reader = Arc::new(Mutex::new(reader));
        Ok(Self {
            reader,
            entries,
            name_len,
        })
    }

    fn read_all_index(reader: &mut T, encoding: Encoding) -> Result<(usize, Vec<DatFileHeader>)> {
        for &name_len in &NAME_LEN {
            match Self::read_index(reader, encoding, name_len) {
                Ok(entries) => return Ok((name_len, entries)),
                Err(_) => continue,
            }
        }
        Err(anyhow::anyhow!("Failed to read DAT index"))
    }

    fn read_index(
        reader: &mut T,
        encoding: Encoding,
        name_len: usize,
    ) -> Result<Vec<DatFileHeader>> {
        reader.rewind()?;
        let mut count = reader.read_u32()?;
        let index_size = (name_len + 4) * count as usize;
        count -= 1;
        let mut entries = Vec::with_capacity(count as usize);
        let mut next_offset = reader.peek_u32_at(4 + name_len)?;
        if (next_offset as usize) < index_size + 4 {
            return Err(anyhow::anyhow!("Invalid next_offset"));
        }
        let first_size = reader.peek_u32_at(name_len)?;
        let second_offset = reader.peek_u32_at(8 + name_len * 2)?;
        if second_offset - next_offset == first_size {
            return Err(anyhow::anyhow!("Invalid second_offset"));
        }
        let file_len = reader.stream_length()?;
        for i in 0..count {
            let name = reader.read_fstring(name_len, encoding, true)?;
            if name.is_empty() {
                return Err(anyhow::anyhow!("Empty file name in DAT archive"));
            }
            let offset = next_offset;
            if i + 1 == count {
                next_offset = file_len as u32;
            } else {
                next_offset = reader.peek_u32_at((name_len + 4) * (i as usize + 2))?;
            }
            if next_offset < offset {
                return Err(anyhow::anyhow!("Invalid offset in DAT archive"));
            }
            let size = next_offset - offset;
            if offset < index_size as u32 || offset + size > file_len as u32 {
                return Err(anyhow::anyhow!("Invalid offset or size in DAT archive"));
            }
            let header = DatFileHeader { name, offset, size };
            entries.push(header);
            reader.seek_relative(4)?;
        }
        Ok(entries)
    }
}

impl<T: Read + Seek + std::fmt::Debug + 'static> Script for DatArchive<T> {
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
        Ok(Box::new(self.entries.iter().map(|e| Ok(e.name.clone()))))
    }

    fn iter_archive_offset<'a>(&'a self) -> Result<Box<dyn Iterator<Item = Result<u64>> + 'a>> {
        Ok(Box::new(self.entries.iter().map(|e| Ok(e.offset as u64))))
    }

    fn open_file<'a>(&'a self, index: usize) -> Result<Box<dyn ArchiveContent + 'a>> {
        if index >= self.entries.len() {
            return Err(anyhow::anyhow!(
                "Index out of bounds: {} (max: {})",
                index,
                self.entries.len()
            ));
        }
        let entry = &self.entries[index];
        let mut entry = Entry {
            header: entry.clone(),
            reader: self.reader.clone(),
            pos: 0,
            script_type: None,
        };
        let mut buf = [0; 32];
        let readed = match entry.read(&mut buf) {
            Ok(readed) => readed,
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Failed to read entry '{}': {}",
                    entry.header.name,
                    e
                ));
            }
        };
        entry.pos = 0;
        entry.script_type = detect_script_type(&buf, readed, &entry.header.name);
        Ok(Box::new(entry))
    }

    fn extra_info<'a>(&'a self) -> Option<Box<dyn AnyDebug + 'a>> {
        Some(Box::new(DatExtraInfo {
            name_len: self.name_len,
        }))
    }
}

fn detect_script_type(_buf: &[u8], _buf_len: usize, _filename: &str) -> Option<ScriptType> {
    #[cfg(feature = "circus-img")]
    if _buf_len >= 4 && _buf.starts_with(b"CRXG") {
        return Some(ScriptType::CircusCrx);
    }
    #[cfg(feature = "circus-audio")]
    if _buf_len >= 4 && _buf.starts_with(b"XPCM") {
        return Some(ScriptType::CircusPcm);
    }
    None
}

fn is_this_format_name_len(buf: &[u8], name_len: usize) -> Result<u8> {
    let mut reader = MemReaderRef::new(buf);
    let count = reader.read_u32()? as usize;
    let index_size = (name_len + 4) * count;
    let mut score = if count > 0 && count < 1000 { 5 } else { 0 };
    let mcount = ((buf.len() - 4) / (name_len + 4)).min(count - 1);
    score += ((mcount / 2).min(10)) as u8;
    if mcount == 0 {
        return Err(anyhow::anyhow!("No entries found in DAT archive"));
    }
    let mut next_offset = reader.cpeek_u32_at(4 + name_len)?;
    if (next_offset as usize) < index_size + 4 {
        return Err(anyhow::anyhow!("Invalid next_offset in DAT archive"));
    }
    let first_size = reader.cpeek_u32_at(name_len)?;
    let second_offset = reader.cpeek_u32_at(8 + name_len * 2)?;
    if second_offset - next_offset == first_size {
        return Err(anyhow::anyhow!("Invalid second_offset in DAT archive"));
    }
    for i in 0..mcount {
        let offset = next_offset;
        if i + 1 == mcount {
            break;
        } else {
            next_offset = reader.cpeek_u32_at((name_len + 4) * (i + 2))?;
        }
        if next_offset < offset {
            return Err(anyhow::anyhow!("Invalid offset in DAT archive"));
        }
        if offset < index_size as u32 {
            return Err(anyhow::anyhow!(
                "Offset is less than index size in DAT archive"
            ));
        }
    }
    Ok(score)
}

/// Checks if the buffer is a valid DAT archive format.
///
/// * `buf` - The buffer to check.
pub fn is_this_format(buf: &[u8]) -> Result<u8> {
    for &name_len in &NAME_LEN {
        match is_this_format_name_len(buf, name_len) {
            Ok(score) => return Ok(score),
            Err(_) => continue,
        }
    }
    Err(anyhow::anyhow!("Not a valid DAT archive format"))
}
