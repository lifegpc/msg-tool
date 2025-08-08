use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use anyhow::Result;
use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub struct CrmArchiveBuilder {}

impl CrmArchiveBuilder {
    pub fn new() -> Self {
        Self {}
    }
}

impl ScriptBuilder for CrmArchiveBuilder {
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
        Ok(Box::new(CrmArchive::new(
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
            Ok(Box::new(CrmArchive::new(
                MemReader::new(data),
                archive_encoding,
                config,
            )?))
        } else {
            let f = std::fs::File::open(filename)?;
            let reader = std::io::BufReader::new(f);
            Ok(Box::new(CrmArchive::new(reader, archive_encoding, config)?))
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
        Ok(Box::new(CrmArchive::new(reader, archive_encoding, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["crm"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::CircusCrm
    }

    fn is_archive(&self) -> bool {
        true
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 4 && buf.starts_with(b"CRXB") {
            return Some(10);
        }
        None
    }
}

#[derive(Debug, Clone)]
struct CrmFileHeader {
    offset: u32,
    size: u32,
    name: String,
}

struct Entry<T: Read + Seek> {
    header: CrmFileHeader,
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
pub struct CrmArchive<T: Read + Seek + std::fmt::Debug> {
    reader: Arc<Mutex<T>>,
    entries: Vec<CrmFileHeader>,
}

impl<T: Read + Seek + std::fmt::Debug> CrmArchive<T> {
    pub fn new(mut reader: T, encoding: Encoding, _config: &ExtraConfig) -> Result<Self> {
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;
        if &magic != b"CRXB" {
            return Err(anyhow::anyhow!("Invalid CRM archive magic: {:?}", magic));
        }
        reader.seek_relative(4)?;
        let count = reader.read_u32()? as usize;
        reader.seek_relative(4)?;
        let mut entries = Vec::with_capacity(count);
        let file_len = reader.stream_length()?;
        let mut offset_map = BTreeMap::new();
        for _ in 0..count {
            let offset = reader.read_u32()?;
            reader.seek_relative(4)?;
            let name = reader.read_fstring(0x18, encoding, true)?;
            offset_map.insert(offset, name);
        }
        let mut next_iter = offset_map.keys().skip(1);
        for (offset, name) in &offset_map {
            let size = if let Some(next) = next_iter.next() {
                *next
            } else {
                file_len as u32
            } - offset;
            entries.push(CrmFileHeader {
                offset: *offset,
                size,
                name: name.clone(),
            });
        }
        Ok(Self {
            reader: Arc::new(Mutex::new(reader)),
            entries,
        })
    }
}

impl<T: Read + Seek + std::fmt::Debug + 'static> Script for CrmArchive<T> {
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
}

fn detect_script_type(_buf: &[u8], _buf_len: usize, _filename: &str) -> Option<ScriptType> {
    #[cfg(feature = "circus-img")]
    if _buf_len >= 4 && _buf.starts_with(b"CRXG") {
        return Some(ScriptType::CircusCrx);
    }
    None
}
