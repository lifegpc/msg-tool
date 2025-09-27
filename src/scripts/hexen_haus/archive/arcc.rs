//! HexenHaus ARCC archive (.arc)
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::decode_to_string;
use anyhow::Result;
use std::io::{Read, Seek, SeekFrom};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
/// HexenHaus ARCC archive builder
pub struct HexenHausArccArchiveBuilder;

impl HexenHausArccArchiveBuilder {
    /// Creates a new `HexenHausArccArchiveBuilder`
    pub const fn new() -> Self {
        HexenHausArccArchiveBuilder
    }
}

impl ScriptBuilder for HexenHausArccArchiveBuilder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Cp932
    }

    fn default_archive_encoding(&self) -> Option<Encoding> {
        Some(Encoding::Cp932)
    }

    fn build_script(
        &self,
        buf: Vec<u8>,
        _filename: &str,
        _encoding: Encoding,
        archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(HexenHausArccArchive::new(
            MemReader::new(buf),
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
            return Ok(Box::new(HexenHausArccArchive::new(
                MemReader::new(data),
                archive_encoding,
                config,
            )?));
        }
        let file = std::fs::File::open(filename)?;
        let reader = std::io::BufReader::new(file);
        Ok(Box::new(HexenHausArccArchive::new(
            reader,
            archive_encoding,
            config,
        )?))
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
        Ok(Box::new(HexenHausArccArchive::new(
            reader,
            archive_encoding,
            config,
        )?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["arc"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::HexenHausArcc
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 4 && buf.starts_with(b"ARCC") {
            Some(10)
        } else {
            None
        }
    }

    fn is_archive(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone)]
struct HexenHausArccEntry {
    name: String,
    offset: u64,
    size: u32,
}

#[derive(Debug)]
/// HexenHaus ARCC archive
pub struct HexenHausArccArchive<T: Read + Seek + std::fmt::Debug> {
    reader: Arc<Mutex<T>>,
    entries: Vec<HexenHausArccEntry>,
}

impl<T: Read + Seek + std::fmt::Debug> HexenHausArccArchive<T> {
    /// Creates a new `HexenHausArccArchive`
    pub fn new(mut reader: T, archive_encoding: Encoding, _config: &ExtraConfig) -> Result<Self> {
        reader.seek(SeekFrom::Start(0))?;
        let mut signature = [0u8; 4];
        reader.read_exact(&mut signature)?;
        if signature != *b"ARCC" {
            return Err(anyhow::anyhow!("Invalid HexenHaus ARCC signature"));
        }
        reader.seek(SeekFrom::Start(0))?;
        let reader = Arc::new(Mutex::new(reader));

        let file_count = reader.cpeek_u32_at(0x14)?;
        let entry_count = file_count as usize;

        let mut index_offset = 0x2a_u64;
        let mut tag = [0u8; 4];
        reader.cpeek_exact_at(index_offset, &mut tag)?;
        if &tag != b"NAME" {
            return Err(anyhow::anyhow!("Missing NAME section in ARCC archive"));
        }
        let addr_offset = reader.cpeek_u64_at(index_offset + 4)?;
        index_offset += 0x0e;

        reader.cpeek_exact_at(index_offset, &mut tag)?;
        if &tag != b"NIDX" {
            return Err(anyhow::anyhow!("Missing NIDX section in ARCC archive"));
        }
        index_offset += 4;
        for _ in 0..entry_count {
            let _ = reader.cpeek_u32_at(index_offset + 2)?;
            index_offset += 8;
        }

        reader.cpeek_exact_at(index_offset, &mut tag)?;
        if &tag != b"EIDX" {
            return Err(anyhow::anyhow!("Missing EIDX section in ARCC archive"));
        }
        index_offset += 4 + 8 * file_count as u64;

        reader.cpeek_exact_at(index_offset, &mut tag)?;
        if &tag != b"CINF" {
            return Err(anyhow::anyhow!("Missing CINF section in ARCC archive"));
        }
        index_offset += 4;

        let mut entries = Vec::with_capacity(entry_count);
        for _ in 0..entry_count {
            index_offset += 6;
            let name_len = reader.cpeek_u16_at(index_offset)? as usize;
            let mut name_buf = vec![0u8; name_len];
            if name_len > 0 {
                reader.cpeek_exact_at(index_offset + 4, &mut name_buf)?;
                decrypt_name(&mut name_buf);
            }
            index_offset += 6 + name_len as u64;
            let name = decode_to_string(archive_encoding, &name_buf, true)?;
            entries.push(HexenHausArccEntry {
                name,
                offset: 0,
                size: 0,
            });
        }

        let mut addr_offset = addr_offset;
        reader.cpeek_exact_at(addr_offset, &mut tag)?;
        if &tag != b"ADDR" {
            return Err(anyhow::anyhow!("Missing ADDR section in ARCC archive"));
        }
        addr_offset += 4;
        for entry in &mut entries {
            entry.offset = reader.cpeek_u64_at(addr_offset + 2)?;
            addr_offset += 12;
        }

        for entry in &mut entries {
            if reader.cpeek_and_equal_at(entry.offset, b"FILE").is_err() {
                continue;
            }
            entry.size = reader.cpeek_u32_at(entry.offset + 0x18)?;
            entry.offset += 0x22;
        }

        entries.retain(|entry| entry.size > 0);
        if entries.is_empty() {
            return Err(anyhow::anyhow!("ARCC archive contains no files"));
        }

        Ok(HexenHausArccArchive { reader, entries })
    }
}

impl<T: Read + Seek + std::fmt::Debug + std::any::Any> Script for HexenHausArccArchive<T> {
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
            return Err(anyhow::anyhow!(
                "Index out of bounds: {} (total files: {})",
                index,
                self.entries.len()
            ));
        }
        let entry = &self.entries[index];
        let header = self
            .reader
            .cpeek_at_vec(entry.offset, (entry.size as usize).min(16))?;
        Ok(Box::new(Entry {
            reader: self.reader.clone(),
            header: entry.clone(),
            pos: 0,
            typ: super::detect_script_type(&entry.name, &header),
        }))
    }
}

struct Entry<T: Read + Seek> {
    header: HexenHausArccEntry,
    reader: Arc<Mutex<T>>,
    pos: u64,
    typ: Option<ScriptType>,
}

impl<T: Read + Seek> ArchiveContent for Entry<T> {
    fn name(&self) -> &str {
        &self.header.name
    }

    fn script_type(&self) -> Option<&ScriptType> {
        self.typ.as_ref()
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
        reader.seek(SeekFrom::Start(self.header.offset + self.pos))?;
        let bytes_read = buf.len().min(self.header.size as usize - self.pos as usize);
        if bytes_read == 0 {
            return Ok(0);
        }
        let bytes_read = reader.read(&mut buf[..bytes_read])?;
        self.pos += bytes_read as u64;
        Ok(bytes_read)
    }
}

impl<T: Read + Seek> Seek for Entry<T> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as u64,
            SeekFrom::End(offset) => {
                if offset < 0 {
                    if (-offset) as u64 > self.header.size as u64 {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "Seek from end exceeds file length",
                        ));
                    }
                    self.header.size as u64 - (-offset) as u64
                } else {
                    self.header.size as u64 + offset as u64
                }
            }
            SeekFrom::Current(offset) => {
                if offset < 0 {
                    if (-offset) as u64 > self.pos {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "Seek from current exceeds current position",
                        ));
                    }
                    self.pos.saturating_sub((-offset) as u64)
                } else {
                    self.pos + offset as u64
                }
            }
        };
        self.pos = new_pos;
        Ok(self.pos)
    }

    fn stream_position(&mut self) -> std::io::Result<u64> {
        Ok(self.pos)
    }
}

fn decrypt_name(buf: &mut [u8]) {
    for byte in buf.iter_mut() {
        *byte ^= 0x69;
    }
}
