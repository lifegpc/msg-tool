//! Circus Archive File (.pck/.dat)
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use crate::utils::struct_pack::*;
use anyhow::Result;
use msg_tool_macro::*;
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
/// Circus PCK Archive Builder
pub struct PckArchiveBuilder {}

impl PckArchiveBuilder {
    /// Creates a new instance of `PckArchiveBuilder`.
    pub const fn new() -> Self {
        Self {}
    }
}

impl ScriptBuilder for PckArchiveBuilder {
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
        Ok(Box::new(PckArchive::new(
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
            Ok(Box::new(PckArchive::new(
                MemReader::new(data),
                archive_encoding,
                config,
            )?))
        } else {
            let f = std::fs::File::open(filename)?;
            let reader = std::io::BufReader::new(f);
            Ok(Box::new(PckArchive::new(reader, archive_encoding, config)?))
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
        Ok(Box::new(PckArchive::new(reader, archive_encoding, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["pck", "dat"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::CircusPck
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
        Ok(Box::new(PckArchiveWriter::new(
            writer, files, encoding, config,
        )?))
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        is_this_format(&buf[..buf_len]).ok()
    }
}

#[derive(Debug, Clone, StructPack, StructUnpack)]
struct PckFileHeader {
    #[fstring = 0x38]
    name: String,
    offset: u32,
    size: u32,
}

struct Entry<T: Read + Seek> {
    header: PckFileHeader,
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
/// PCK Archive
pub struct PckArchive<T: Read + Seek + std::fmt::Debug> {
    reader: Arc<Mutex<T>>,
    entries: Vec<PckFileHeader>,
}

impl<T: Read + Seek + std::fmt::Debug> PckArchive<T> {
    /// Creates a new `PckArchive` from a reader.
    ///
    /// * `reader` - The reader to read the PCK archive from.
    /// * `archive_encoding` - The encoding to use for string fields in the archive.
    /// * `config` - Extra configuration options.
    pub fn new(mut reader: T, archive_encoding: Encoding, _config: &ExtraConfig) -> Result<Self> {
        let file_count = reader.read_u32()?;
        // (offset, size)
        let mut offset_list = Vec::with_capacity(file_count as usize);
        for _ in 0..file_count {
            let offset = reader.read_u32()?;
            let size = reader.read_u32()?;
            offset_list.push((offset, size));
        }
        for i in 1..file_count as usize {
            let (prev_offset, prev_size) = offset_list[i - 1];
            let offset = offset_list[i].0;
            if prev_offset + prev_size > offset {
                return Err(anyhow::anyhow!(
                    "PckArchive: Overlapping entries detected at index {}: previous entry ends at {}, current entry starts at {}",
                    i - 1,
                    prev_offset + prev_size,
                    offset
                ));
            }
        }
        let mut entries = Vec::with_capacity(file_count as usize);
        for (i, (offset, size)) in offset_list.into_iter().enumerate() {
            let header: PckFileHeader = reader.read_struct(false, archive_encoding, &None)?;
            if header.offset != offset {
                return Err(anyhow::anyhow!(
                    "PckArchive: Header offset mismatch at entry {}: expected {}, got {}",
                    i,
                    offset,
                    header.offset
                ));
            }
            if header.size != size {
                return Err(anyhow::anyhow!(
                    "PckArchive: Header size mismatch at entry {}: expected {}, got {}",
                    i,
                    size,
                    header.size
                ));
            }
            entries.push(header);
        }
        Ok(Self {
            reader: Arc::new(Mutex::new(reader)),
            entries,
        })
    }
}

impl<T: Read + Seek + std::fmt::Debug + 'static> Script for PckArchive<T> {
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
    #[cfg(feature = "circus-audio")]
    if _buf_len >= 4 && _buf.starts_with(b"XPCM") {
        return Some(ScriptType::CircusPcm);
    }
    None
}

/// PCK Archive Writer
pub struct PckArchiveWriter<T: Write + Seek> {
    writer: T,
    headers: HashMap<String, PckFileHeader>,
    encoding: Encoding,
}

impl<T: Write + Seek> PckArchiveWriter<T> {
    /// Creates a new `PckArchiveWriter` for writing a PCK archive.
    ///
    /// * `writer` - The writer to write the PCK archive to.
    /// * `files` - A list of file names to include in the archive.
    /// * `encoding` - The encoding to use for string fields in the archive.
    /// * `config` - Extra configuration options.
    pub fn new(
        mut writer: T,
        files: &[&str],
        encoding: Encoding,
        _config: &ExtraConfig,
    ) -> Result<Self> {
        let file_count = files.len() as u32;
        writer.write_u32(file_count)?;
        let mut headers = HashMap::new();
        for _ in 0..file_count {
            writer.write_u32(0)?; // Placeholder for offset
            writer.write_u32(0)?; // Placeholder for size
        }
        for file in files {
            let header = PckFileHeader {
                name: file.to_string(),
                offset: 0,
                size: 0,
            };
            header.pack(&mut writer, false, encoding, &None)?;
            headers.insert(file.to_string(), header);
        }
        Ok(PckArchiveWriter {
            writer,
            headers,
            encoding,
        })
    }
}

impl<T: Write + Seek> Archive for PckArchiveWriter<T> {
    fn new_file<'a>(
        &'a mut self,
        name: &str,
        _size: Option<u64>,
    ) -> Result<Box<dyn WriteSeek + 'a>> {
        let entry = self
            .headers
            .get_mut(name)
            .ok_or_else(|| anyhow::anyhow!("File '{}' not found in archive", name))?;
        if entry.offset != 0 || entry.size != 0 {
            return Err(anyhow::anyhow!("File '{}' already exists in archive", name));
        }
        self.writer.seek(SeekFrom::End(0))?;
        entry.offset = self.writer.stream_position()? as u32;
        let file = PckArchiveFile {
            header: entry,
            writer: &mut self.writer,
            pos: 0,
        };
        Ok(Box::new(file))
    }

    fn write_header(&mut self) -> Result<()> {
        self.writer.seek(SeekFrom::Start(0x4))?;
        let mut files = self.headers.iter().map(|(_, d)| d).collect::<Vec<_>>();
        files.sort_by_key(|f| f.offset);
        for file in files.iter() {
            self.writer.write_u32(file.offset)?;
            self.writer.write_u32(file.size)?;
        }
        for file in files {
            file.pack(&mut self.writer, false, self.encoding, &None)?;
        }
        Ok(())
    }
}

/// PCK Archive File
pub struct PckArchiveFile<'a, T: Write + Seek> {
    header: &'a mut PckFileHeader,
    writer: &'a mut T,
    pos: usize,
}

impl<'a, T: Write + Seek> Write for PckArchiveFile<'a, T> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.writer
            .seek(SeekFrom::Start(self.header.offset as u64 + self.pos as u64))?;
        let bytes_written = self.writer.write(buf)?;
        self.pos += bytes_written;
        self.header.size = self.header.size.max(self.pos as u32);
        Ok(bytes_written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

impl<'a, T: Write + Seek> Seek for PckArchiveFile<'a, T> {
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
}

/// Checks if the buffer is a valid PCK archive format.
pub fn is_this_format(buf: &[u8]) -> Result<u8> {
    let mut reader = MemReaderRef::new(buf);
    let count = reader.read_u32()? as usize;
    let mut score = if count > 0 && count < 0x40000 { 5 } else { 0 };
    let avail_count = ((buf.len() - 4) / 0x8).min(count);
    score += ((avail_count / 2).min(10)) as u8;
    if avail_count == 0 {
        return Err(anyhow::anyhow!("No valid entries found in PCK archive"));
    }
    let mut prev_off = reader.read_u32()?;
    let mut prev_size = reader.read_u32()?;
    let mut index = 1;
    while index < avail_count {
        let off = reader.read_u32()?;
        let size = reader.read_u32()?;
        if off < prev_off || prev_off + prev_size != off {
            return Err(anyhow::anyhow!("Invalid offset."));
        }
        prev_off = off;
        prev_size = size;
        index += 1;
    }
    Ok(score)
}
