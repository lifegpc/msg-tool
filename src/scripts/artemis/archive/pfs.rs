use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::struct_pack::*;
use anyhow::Result;
use msg_tool_macro::*;
use sha1::Digest;
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub struct ArtemisArcBuilder {}

impl ArtemisArcBuilder {
    pub fn new() -> Self {
        ArtemisArcBuilder {}
    }
}

impl ScriptBuilder for ArtemisArcBuilder {
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
        archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(ArtemisArc::new(
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
        Ok(Box::new(ArtemisArc::new(
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
        Ok(Box::new(ArtemisArc::new(
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
        &ScriptType::ArtemisArc
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 3 && (buf.starts_with(b"pf6") || buf.starts_with(b"pf8")) {
            return Some(10);
        }
        None
    }

    fn create_archive(
        &self,
        filename: &str,
        files: &[&str],
        encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<Box<dyn Archive>> {
        let f = std::fs::File::options()
            .write(true)
            .read(true)
            .create(true)
            .truncate(true)
            .open(filename)?;
        Ok(Box::new(ArtemisArcWriter::new(f, files, encoding, config)?))
    }
}

#[derive(Debug, Clone, StructPack, StructUnpack)]
struct PfsEntryHeader {
    #[pstring(u32)]
    name: String,
    _unk: u32,
    offset: u32,
    size: u32,
}

#[derive(Debug)]
pub struct ArtemisArc<T: Read + Seek + std::fmt::Debug> {
    reader: Arc<Mutex<T>>,
    entries: Vec<PfsEntryHeader>,
    xor_key: Option<[u8; 20]>,
    output_ext: Option<String>,
}

impl<T: Read + Seek + std::fmt::Debug> ArtemisArc<T> {
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
                "Invalid Artemis archive magic: {:?}",
                magic
            ));
        }
        let version = reader.read_u8()?;
        if version != b'6' && version != b'8' {
            return Err(anyhow::anyhow!(
                "Unsupported Artemis archive version: {}",
                version
            ));
        }
        let index_size = reader.read_u32()?;
        let file_count = reader.read_u32()?;
        let mut entries = Vec::with_capacity(file_count as usize);
        for _ in 0..file_count {
            let header = reader.read_struct(false, archive_encoding)?;
            entries.push(header);
        }
        let xor_key = if version == b'8' {
            reader.seek(SeekFrom::Start(7))?;
            let mut sha = sha1::Sha1::default();
            let ra = &mut reader;
            let mut r = ra.take(index_size as u64);
            std::io::copy(&mut r, &mut sha)?;
            sha.flush()?;
            let result = sha.finalize();
            let mut xor_key = [0u8; 20];
            xor_key.copy_from_slice(&result);
            Some(xor_key)
        } else {
            None
        };
        let output_ext = std::path::Path::new(filename)
            .extension()
            .filter(|s| *s != "pfs")
            .map(|s| s.to_string_lossy().to_string());
        Ok(ArtemisArc {
            reader: Arc::new(Mutex::new(reader)),
            entries,
            xor_key,
            output_ext,
        })
    }
}

impl<T: Read + Seek + std::fmt::Debug + 'static> Script for ArtemisArc<T> {
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
        let mut entry = Entry {
            header: header.clone(),
            reader: self.reader.clone(),
            pos: 0,
            script_type: None,
            xor_key: self.xor_key.clone(),
        };
        let mut header = [0; 0x20];
        let readed = entry.read(&mut header)?;
        entry.pos = 0;
        entry.script_type = detect_script_type(&header, readed, &entry.header.name);
        Ok(Box::new(entry))
    }

    fn archive_output_ext<'a>(&'a self) -> Option<&'a str> {
        self.output_ext.as_ref().map(|s| s.as_str())
    }
}

struct Entry<T: Read + Seek> {
    header: PfsEntryHeader,
    reader: Arc<Mutex<T>>,
    pos: u64,
    script_type: Option<ScriptType>,
    xor_key: Option<[u8; 20]>,
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
        reader.seek(SeekFrom::Start(self.header.offset as u64 + self.pos))?;
        let bytes_read = buf.len().min(self.header.size as usize - self.pos as usize);
        if bytes_read == 0 {
            return Ok(0);
        }
        let bytes_read = reader.read(&mut buf[..bytes_read])?;
        if let Some(xor_key) = &self.xor_key {
            for i in 0..bytes_read {
                let l = (self.pos + i as u64) % 20;
                buf[i] ^= xor_key[l as usize];
            }
        }
        self.pos += bytes_read as u64;
        Ok(bytes_read)
    }
}

impl<T: Read + Seek> Seek for Entry<T> {
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

fn detect_script_type(buf: &[u8], buf_len: usize, filename: &str) -> Option<ScriptType> {
    if buf_len >= 5 && buf.starts_with(b"ASB\0\0") {
        return Some(ScriptType::ArtemisAsb);
    }
    if super::super::ast::is_this_format(filename, buf, buf_len) {
        return Some(ScriptType::Artemis);
    }
    None
}

pub struct ArtemisArcWriter<T: Write + Seek + Read> {
    writer: T,
    headers: HashMap<String, PfsEntryHeader>,
    encoding: Encoding,
    disable_xor: bool,
    index_size: u32,
}

impl<T: Write + Seek + Read> ArtemisArcWriter<T> {
    pub fn new(
        mut writer: T,
        files: &[&str],
        encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<Self> {
        writer.write_all(if config.artemis_arc_disable_xor {
            b"pf6"
        } else {
            b"pf8"
        })?;
        writer.write_u32(0)?; // Placeholder for index size
        writer.write_u32(files.len() as u32)?;
        let mut headers = HashMap::new();
        for file in files {
            let header = PfsEntryHeader {
                name: file.to_string(),
                _unk: 0,
                offset: 0,
                size: 0,
            };
            header.pack(&mut writer, false, encoding)?;
            headers.insert(file.to_string(), header);
        }
        let size = writer.stream_position()?;
        let index_size = size as u32 - 7;
        writer.write_u32_at(3, index_size)?;
        Ok(ArtemisArcWriter {
            writer,
            headers,
            encoding,
            disable_xor: config.artemis_arc_disable_xor,
            index_size,
        })
    }
}

impl<T: Write + Seek + Read> Archive for ArtemisArcWriter<T> {
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
        let file = ArtemisArcFile {
            header: entry,
            writer: &mut self.writer,
            pos: 0,
        };
        Ok(Box::new(file))
    }

    fn write_header(&mut self) -> Result<()> {
        self.writer.seek(SeekFrom::Start(11))?;
        let mut files = self.headers.values().collect::<Vec<_>>();
        files.sort_by_key(|d| d.offset);
        for file in files.iter() {
            file.pack(&mut self.writer, false, self.encoding)?;
        }
        if !self.disable_xor {
            self.writer.seek(SeekFrom::Start(7))?;
            let mut sha = sha1::Sha1::default();
            let w = &mut self.writer;
            let mut header = w.take(self.index_size as u64);
            std::io::copy(&mut header, &mut sha)?;
            sha.flush()?;
            let result = sha.finalize();
            let mut xor_key = [0u8; 20];
            xor_key.copy_from_slice(&result);
            let mut buf = [0u8; 1024];
            for file in files.iter() {
                self.writer.seek(SeekFrom::Start(file.offset as u64))?;
                let mut pos = 0u32;
                while pos < file.size {
                    let bytes_to_read = (file.size - pos).min(1024) as usize;
                    let bytes_read = self.writer.read(&mut buf[..bytes_to_read])?;
                    if bytes_read == 0 {
                        return Err(anyhow::anyhow!(
                            "Unexpected end of file while reading '{}'",
                            file.name
                        ));
                    }
                    for i in 0..bytes_read {
                        let l = (pos as u64 + i as u64) % 20;
                        buf[i] ^= xor_key[l as usize];
                    }
                    self.writer.seek_relative(-(bytes_read as i64))?;
                    self.writer.write_all(&buf[..bytes_read])?;
                    pos += bytes_read as u32;
                }
            }
        }
        Ok(())
    }
}

pub struct ArtemisArcFile<'a, T: Write + Seek> {
    header: &'a mut PfsEntryHeader,
    writer: &'a mut T,
    pos: u64,
}

impl<'a, T: Write + Seek> Write for ArtemisArcFile<'a, T> {
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

impl<'a, T: Write + Seek> Seek for ArtemisArcFile<'a, T> {
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
