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
pub struct PckArchiveBuilder {}

impl PckArchiveBuilder {
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
pub struct PckArchive<T: Read + Seek + std::fmt::Debug> {
    reader: Arc<Mutex<T>>,
    entries: Vec<PckFileHeader>,
}

impl<T: Read + Seek + std::fmt::Debug> PckArchive<T> {
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
            let header: PckFileHeader = reader.read_struct(false, archive_encoding)?;
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

    fn iter_archive<'a>(&'a mut self) -> Result<Box<dyn Iterator<Item = Result<String>> + 'a>> {
        Ok(Box::new(self.entries.iter().map(|e| Ok(e.name.clone()))))
    }

    fn iter_archive_mut<'a>(
        &'a mut self,
    ) -> Result<Box<dyn Iterator<Item = Result<Box<dyn ArchiveContent>>> + 'a>> {
        Ok(Box::new(PckArchiveIter {
            entries: self.entries.iter(),
            reader: self.reader.clone(),
        }))
    }
}

fn detect_script_type(_buf: &[u8], _buf_len: usize, _filename: &str) -> Option<ScriptType> {
    #[cfg(feature = "circus-img")]
    if _buf_len >= 4 && _buf.starts_with(b"CRXG") {
        return Some(ScriptType::CircusCrx);
    }
    None
}

struct PckArchiveIter<'a, T: Iterator<Item = &'a PckFileHeader>, R: Read + Seek> {
    entries: T,
    reader: Arc<Mutex<R>>,
}

impl<'a, T: Iterator<Item = &'a PckFileHeader>, R: Read + Seek + 'static> Iterator
    for PckArchiveIter<'a, T, R>
{
    type Item = Result<Box<dyn ArchiveContent>>;

    fn next(&mut self) -> Option<Self::Item> {
        let entry = match self.entries.next() {
            Some(e) => e,
            None => return None,
        };
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
                return Some(Err(anyhow::anyhow!(
                    "Failed to read entry '{}': {}",
                    entry.header.name,
                    e
                )));
            }
        };
        entry.pos = 0;
        entry.script_type = detect_script_type(&buf, readed, &entry.header.name);
        Some(Ok(Box::new(entry)))
    }
}

pub struct PckArchiveWriter<T: Write + Seek> {
    writer: T,
    headers: HashMap<String, PckFileHeader>,
    encoding: Encoding,
}

impl<T: Write + Seek> PckArchiveWriter<T> {
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
            header.pack(&mut writer, false, encoding)?;
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
    fn new_file<'a>(&'a mut self, name: &str) -> Result<Box<dyn WriteSeek + 'a>> {
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
            file.pack(&mut self.writer, false, self.encoding)?;
        }
        Ok(())
    }
}

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
