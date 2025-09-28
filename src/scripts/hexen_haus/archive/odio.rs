//! HexenHaus ODIO archive (.bin)
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use anyhow::{Result, anyhow};
use std::io::{Read, Seek, SeekFrom};
use std::sync::{Arc, Mutex};

const ODIO_SIGNATURE: &[u8; 4] = b"ODIO";
const HEADER_CHECK_OFFSET: u64 = 0x0A;
const HEADER_CHECK_VALUE: u32 = 0xCCAE_01FF;
const INDEX_START: u64 = 0x12;
const INDEX_ENTRY_SIZE: u64 = 6;
const ENTRY_HEADER_SIZE: u64 = 0x2C;

#[derive(Debug)]
/// HexenHaus ODIO archive builder
pub struct HexenHausOdioArchiveBuilder;

impl HexenHausOdioArchiveBuilder {
    /// Creates a new `HexenHausOdioArchiveBuilder`
    pub const fn new() -> Self {
        HexenHausOdioArchiveBuilder
    }
}

impl ScriptBuilder for HexenHausOdioArchiveBuilder {
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
        Ok(Box::new(HexenHausOdioArchive::new(
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
            return Ok(Box::new(HexenHausOdioArchive::new(
                MemReader::new(data),
                archive_encoding,
                config,
            )?));
        }
        let file = std::fs::File::open(filename)?;
        let reader = std::io::BufReader::new(file);
        Ok(Box::new(HexenHausOdioArchive::new(
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
        Ok(Box::new(HexenHausOdioArchive::new(
            reader,
            archive_encoding,
            config,
        )?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["bin"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::HexenHausOdio
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= ODIO_SIGNATURE.len() && buf.starts_with(ODIO_SIGNATURE) {
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
struct HexenHausOdioEntry {
    name: String,
    offset: u64,
    size: u64,
}

#[derive(Debug)]
/// HexenHaus ODIO archive reader
pub struct HexenHausOdioArchive<T: Read + Seek + std::fmt::Debug> {
    reader: Arc<Mutex<T>>,
    entries: Vec<HexenHausOdioEntry>,
}

impl<T: Read + Seek + std::fmt::Debug> HexenHausOdioArchive<T> {
    /// Creates a new `HexenHausOdioArchive`
    pub fn new(mut reader: T, _archive_encoding: Encoding, _config: &ExtraConfig) -> Result<Self> {
        reader.seek(SeekFrom::Start(0))?;
        let mut signature = [0u8; 4];
        reader.read_exact(&mut signature)?;
        if signature != *ODIO_SIGNATURE {
            return Err(anyhow!("Invalid HexenHaus ODIO signature"));
        }

        let reserved = reader.read_u32()?;
        if reserved != 0 {
            return Err(anyhow!("Unexpected reserved field in ODIO header"));
        }

        reader.seek(SeekFrom::Start(HEADER_CHECK_OFFSET))?;
        let header_check = reader.read_u32()?;
        if header_check != HEADER_CHECK_VALUE {
            return Err(anyhow!("Invalid HexenHaus ODIO header check value"));
        }

        let file_length = reader.seek(SeekFrom::End(0))?;
        reader.seek(SeekFrom::Start(INDEX_START))?;
        let first_offset = u64::from(reader.read_u32()?);
        if first_offset < INDEX_START {
            return Err(anyhow!("First entry offset precedes index start"));
        }
        if first_offset > file_length {
            return Err(anyhow!("First entry offset exceeds file length"));
        }

        let index_len = first_offset
            .checked_sub(INDEX_START)
            .ok_or_else(|| anyhow!("Invalid index length in ODIO archive"))?;
        if index_len % INDEX_ENTRY_SIZE != 0 {
            return Err(anyhow!("ODIO index length is not aligned"));
        }
        let entry_count_u64 = index_len / INDEX_ENTRY_SIZE;
        let entry_count =
            usize::try_from(entry_count_u64).map_err(|_| anyhow!("ODIO entry count overflow"))?;
        if entry_count == 0 {
            return Err(anyhow!("ODIO archive contains no entries"));
        }

        let mut entries = Vec::with_capacity(entry_count);
        let mut index_offset = INDEX_START;
        let mut next_offset = first_offset;

        for i in 0..entry_count {
            let entry_offset = next_offset;

            index_offset = index_offset
                .checked_add(INDEX_ENTRY_SIZE)
                .ok_or_else(|| anyhow!("Index offset overflow"))?;

            if i + 1 == entry_count {
                next_offset = file_length;
            } else {
                if index_offset + 4 > file_length {
                    return Err(anyhow!("Index offset exceeds file length"));
                }
                reader.seek(SeekFrom::Start(index_offset))?;
                next_offset = u64::from(reader.read_u32()?);
            }

            if entry_offset > file_length {
                return Err(anyhow!("Entry offset exceeds file length"));
            }
            if next_offset > file_length {
                return Err(anyhow!("Entry extends beyond file length"));
            }
            if next_offset < entry_offset {
                return Err(anyhow!("Entry offsets are out of order"));
            }

            let size = next_offset - entry_offset;
            if size == 0 {
                continue;
            }

            let name = format!("{:04}.ogg", i);
            entries.push(HexenHausOdioEntry {
                name,
                offset: entry_offset,
                size,
            });
        }

        if entries.is_empty() {
            return Err(anyhow!("ODIO archive contains no readable entries"));
        }

        reader.seek(SeekFrom::Start(0))?;
        Ok(HexenHausOdioArchive {
            reader: Arc::new(Mutex::new(reader)),
            entries,
        })
    }
}

impl<T: Read + Seek + std::fmt::Debug + std::any::Any> Script for HexenHausOdioArchive<T> {
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
            return Err(anyhow!(
                "Index out of bounds: {} (total files: {})",
                index,
                self.entries.len()
            ));
        }
        let entry = self.entries[index].clone();

        let decrypt = if entry.size >= ENTRY_HEADER_SIZE {
            let mut header = [0u8; 4];
            let mut guard = self
                .reader
                .lock()
                .map_err(|e| anyhow!("Failed to lock reader: {}", e))?;
            guard.seek(SeekFrom::Start(entry.offset))?;
            guard.read_exact(&mut header)?;
            header == *b"ONCE"
        } else {
            false
        };

        let (data_offset, data_size) = if decrypt {
            let data_offset = entry
                .offset
                .checked_add(ENTRY_HEADER_SIZE)
                .ok_or_else(|| anyhow!("Entry data offset overflow"))?;
            let data_size = entry
                .size
                .checked_sub(ENTRY_HEADER_SIZE)
                .ok_or_else(|| anyhow!("Entry data size underflow"))?;
            (data_offset, data_size)
        } else {
            (entry.offset, entry.size)
        };

        Ok(Box::new(OdioEntry {
            name: entry.name,
            reader: self.reader.clone(),
            data_offset,
            data_size,
            pos: 0,
            decrypt,
        }))
    }
}

struct OdioEntry<T: Read + Seek> {
    name: String,
    reader: Arc<Mutex<T>>,
    data_offset: u64,
    data_size: u64,
    pos: u64,
    decrypt: bool,
}

impl<T: Read + Seek> ArchiveContent for OdioEntry<T> {
    fn name(&self) -> &str {
        &self.name
    }
}

impl<T: Read + Seek> Read for OdioEntry<T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let total_size = self.data_size;
        if self.pos >= total_size {
            return Ok(0);
        }

        let remaining = total_size - self.pos;
        let remaining_usize = match usize::try_from(remaining) {
            Ok(value) => value,
            Err(_) => usize::MAX,
        };
        let to_read = remaining_usize.min(buf.len());
        if to_read == 0 {
            return Ok(0);
        }

        let absolute_offset = match self.data_offset.checked_add(self.pos) {
            Some(offset) => offset,
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Read position overflow",
                ));
            }
        };

        let mut guard = self.reader.lock().map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to lock mutex: {}", e),
            )
        })?;
        guard.seek(SeekFrom::Start(absolute_offset))?;
        let bytes_read = guard.read(&mut buf[..to_read])?;
        drop(guard);

        if self.decrypt {
            for byte in &mut buf[..bytes_read] {
                *byte = byte.rotate_right(4);
            }
        }

        self.pos = self.pos.saturating_add(bytes_read as u64);
        Ok(bytes_read)
    }
}

impl<T: Read + Seek> Seek for OdioEntry<T> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset,
            SeekFrom::End(offset) => {
                let size = i64::try_from(self.data_size).map_err(|_| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Data size exceeds seek range",
                    )
                })?;
                let target = size.checked_add(offset).ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Seek from end caused overflow",
                    )
                })?;
                if target < 0 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Seek from end before start",
                    ));
                }
                target as u64
            }
            SeekFrom::Current(offset) => {
                let current = i64::try_from(self.pos).map_err(|_| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Current position overflow",
                    )
                })?;
                let target = current.checked_add(offset).ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Seek from current caused overflow",
                    )
                })?;
                if target < 0 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Seek before start",
                    ));
                }
                target as u64
            }
        };
        self.pos = new_pos;
        Ok(self.pos)
    }

    fn stream_position(&mut self) -> std::io::Result<u64> {
        Ok(self.pos)
    }
}
