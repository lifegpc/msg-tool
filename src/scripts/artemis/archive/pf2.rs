//! Artemis Engine PF2 Archive (pf2)
use super::detect_script_type;
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::struct_pack::*;
use anyhow::Result;
use msg_tool_macro::*;
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
/// The builder for Artemis PF2 archive scripts.
pub struct ArtemisPf2Builder {}

impl ArtemisPf2Builder {
    /// Creates a new instance of `ArtemisPf2Builder`.
    pub fn new() -> Self {
        ArtemisPf2Builder {}
    }
}

impl ScriptBuilder for ArtemisPf2Builder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Cp932
    }

    fn default_archive_encoding(&self) -> Option<Encoding> {
        Some(Encoding::Cp932)
    }

    fn build_script(
        &self,
        buf: Vec<u8>,
        filename: &str,
        _encoding: Encoding,
        archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(ArtemisPf2::new(
            MemReader::new(buf),
            archive_encoding,
            config,
            filename,
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
        let f = std::fs::File::open(filename)?;
        let f = std::io::BufReader::new(f);
        Ok(Box::new(ArtemisPf2::new(
            f,
            archive_encoding,
            config,
            filename,
        )?))
    }

    fn build_script_from_reader(
        &self,
        reader: Box<dyn ReadSeek>,
        filename: &str,
        _encoding: Encoding,
        archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(ArtemisPf2::new(
            reader,
            archive_encoding,
            config,
            filename,
        )?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        gen_artemis_arc_ext!()
    }

    fn is_archive(&self) -> bool {
        true
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::ArtemisPf2
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 3 && buf.starts_with(b"pf2") {
            return Some(20);
        }
        None
    }

    fn create_archive(
        &self,
        filename: &str,
        files: &[&str],
        encoding: Encoding,
        _config: &ExtraConfig,
    ) -> Result<Box<dyn Archive>> {
        let f = std::fs::File::options()
            .write(true)
            .read(true)
            .create(true)
            .truncate(true)
            .open(filename)?;
        Ok(Box::new(ArtemisPf2Writer::new(f, files, encoding)?))
    }
}

#[derive(Debug, Clone, StructPack, StructUnpack)]
struct Pf2EntryHeader {
    #[pstring(u32)]
    name: String,
    // real path str len (?)
    _unk1: u32,
    _unk2: u32,
    _unk3: u32,
    offset: u32,
    size: u32,
}

#[derive(Debug)]
/// The Artemis PF2 archive script.
pub struct ArtemisPf2<T: Read + Seek + std::fmt::Debug> {
    reader: Arc<Mutex<T>>,
    entries: Vec<Pf2EntryHeader>,
    output_ext: Option<String>,
}

impl<T: Read + Seek + std::fmt::Debug> ArtemisPf2<T> {
    /// Creates a new Artemis PF2 archive script.
    ///
    /// * `reader` - The reader for the archive.
    /// * `archive_encoding` - The encoding used for the archive.
    /// * `config` - Extra configuration options.
    /// * `filename` - The name of the archive file.
    pub fn new(
        mut reader: T,
        archive_encoding: Encoding,
        _config: &ExtraConfig,
        filename: &str,
    ) -> Result<Self> {
        let mut magic = [0; 2];
        reader.read_exact(&mut magic)?;
        if &magic != b"pf" {
            return Err(anyhow::anyhow!(
                "Invalid Artemis PF2 archive magic: {:?}",
                magic
            ));
        }
        let version = reader.read_u8()?;
        if version != b'2' {
            return Err(anyhow::anyhow!(
                "Unsupported Artemis PF2 archive version: {}",
                version
            ));
        }
        let _index_size = reader.read_u32()?;
        let _reserved = reader.read_u32()?;
        let file_count = reader.read_u32()?;
        let mut entries = Vec::with_capacity(file_count as usize);
        for _ in 0..file_count {
            let header = reader.read_struct(false, archive_encoding, &None)?;
            entries.push(header);
        }
        let output_ext = std::path::Path::new(filename)
            .extension()
            .filter(|s| *s != "pfs")
            .map(|s| s.to_string_lossy().to_string());
        Ok(ArtemisPf2 {
            reader: Arc::new(Mutex::new(reader)),
            entries,
            output_ext,
        })
    }
}

impl<T: Read + Seek + std::fmt::Debug + 'static> Script for ArtemisPf2<T> {
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
            self.entries.iter().map(|header| Ok(header.name.clone())),
        ))
    }

    fn iter_archive_offset<'a>(&'a self) -> Result<Box<dyn Iterator<Item = Result<u64>> + 'a>> {
        Ok(Box::new(
            self.entries.iter().map(|header| Ok(header.offset as u64)),
        ))
    }

    fn open_file<'a>(&'a self, index: usize) -> Result<Box<dyn ArchiveContent + 'a>> {
        if index >= self.entries.len() {
            return Err(anyhow::anyhow!(
                "Index out of bounds: {} (max: {})",
                index,
                self.entries.len()
            ));
        }
        let header = &self.entries[index];
        let mut entry = Pf2Entry {
            header: header.clone(),
            reader: self.reader.clone(),
            pos: 0,
            script_type: None,
        };
        let mut header_buf = [0; 0x20];
        let readed = entry.read(&mut header_buf)?;
        entry.pos = 0;
        entry.script_type = detect_script_type(&header_buf, readed, &entry.header.name);
        Ok(Box::new(entry))
    }

    fn archive_output_ext<'a>(&'a self) -> Option<&'a str> {
        self.output_ext.as_deref()
    }
}

struct Pf2Entry<T: Read + Seek> {
    header: Pf2EntryHeader,
    reader: Arc<Mutex<T>>,
    pos: u64,
    script_type: Option<ScriptType>,
}

impl<T: Read + Seek> ArchiveContent for Pf2Entry<T> {
    fn name(&self) -> &str {
        &self.header.name
    }

    fn script_type(&self) -> Option<&ScriptType> {
        self.script_type.as_ref()
    }
}

impl<T: Read + Seek> Read for Pf2Entry<T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut reader = self.reader.lock().map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to lock mutex: {}", e),
            )
        })?;
        reader.seek(SeekFrom::Start(self.header.offset as u64 + self.pos))?;
        let remaining = (self.header.size as u64).saturating_sub(self.pos);
        if remaining == 0 {
            return Ok(0);
        }
        let bytes_to_read = buf.len().min(remaining as usize);
        let bytes_read = reader.read(&mut buf[..bytes_to_read])?;
        self.pos += bytes_read as u64;
        Ok(bytes_read)
    }
}

impl<T: Read + Seek> Seek for Pf2Entry<T> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset,
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

/// The Artemis PF2 archive writer.
pub struct ArtemisPf2Writer<T: Write + Seek + Read> {
    writer: T,
    headers: HashMap<String, Pf2EntryHeader>,
    encoding: Encoding,
    index_size: u32,
}

impl<T: Write + Seek + Read> ArtemisPf2Writer<T> {
    /// Creates a new Artemis PF2 archive writer.
    ///
    /// * `writer` - The writer for the archive.
    /// * `files` - The list of files to include in the archive.
    /// * `encoding` - The encoding used for the archive.
    pub fn new(mut writer: T, files: &[&str], encoding: Encoding) -> Result<Self> {
        writer.write_all(b"pf2")?;
        writer.write_u32(0)?; // Placeholder for index size
        writer.write_u32(0)?; // Reserved field at offset 0x07
        writer.write_u32(files.len() as u32)?;
        let mut headers = HashMap::new();
        for file in files {
            let header = Pf2EntryHeader {
                name: file.to_string(),
                _unk1: 0x10,
                _unk2: 0,
                _unk3: 0,
                offset: 0,
                size: 0,
            };
            header.pack(&mut writer, false, encoding, &None)?;
            headers.insert(file.to_string(), header);
        }
        let size = writer.stream_position()?;
        let index_size = size as u32 - 7;
        writer.write_u32_at(3, index_size)?;
        writer.write_u32_at(7, 0)?;
        Ok(ArtemisPf2Writer {
            writer,
            headers,
            encoding,
            index_size,
        })
    }
}

impl<T: Write + Seek + Read> Archive for ArtemisPf2Writer<T> {
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
        let file = ArtemisPf2File {
            header: entry,
            writer: &mut self.writer,
            pos: 0,
        };
        Ok(Box::new(file))
    }

    fn write_header(&mut self) -> Result<()> {
        self.writer.seek(SeekFrom::Start(15))?;
        let mut files = self.headers.values().collect::<Vec<_>>();
        files.sort_by_key(|d| d.offset);
        for file in files.iter() {
            file.pack(&mut self.writer, false, self.encoding, &None)?;
        }
        self.writer.write_u32_at(3, self.index_size)?;
        self.writer.write_u32_at(7, 0)?;
        Ok(())
    }
}

/// The Artemis PF2 archive file writer.
pub struct ArtemisPf2File<'a, T: Write + Seek> {
    header: &'a mut Pf2EntryHeader,
    writer: &'a mut T,
    pos: u64,
}

impl<'a, T: Write + Seek> Write for ArtemisPf2File<'a, T> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.writer
            .seek(SeekFrom::Start(self.header.offset as u64 + self.pos))?;
        let bytes_written = self.writer.write(buf)?;
        self.pos += bytes_written as u64;
        self.header.size = self.header.size.max(self.pos as u32);
        Ok(bytes_written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

impl<'a, T: Write + Seek> Seek for ArtemisPf2File<'a, T> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset,
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
