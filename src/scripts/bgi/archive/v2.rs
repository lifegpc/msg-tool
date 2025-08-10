//! Buriko General Interpreter/Ethornell Archive File Version 2 (.arc)
use super::bse::*;
use super::dsc::*;
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::encode_string;
use crate::utils::struct_pack::*;
use anyhow::Result;
use msg_tool_macro::*;
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
/// Builder for BGI Archive Version 2
pub struct BgiArchiveBuilder {}

impl BgiArchiveBuilder {
    /// Creates a new instance of `BgiArchiveBuilder`.
    pub const fn new() -> Self {
        BgiArchiveBuilder {}
    }
}

impl ScriptBuilder for BgiArchiveBuilder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Cp932
    }

    fn default_archive_encoding(&self) -> Option<Encoding> {
        Some(Encoding::Cp932)
    }

    fn build_script(
        &self,
        data: Vec<u8>,
        filename: &str,
        _encoding: Encoding,
        archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(BgiArchive::new(
            MemReader::new(data),
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
        if filename == "-" {
            let data = crate::utils::files::read_file(filename)?;
            Ok(Box::new(BgiArchive::new(
                MemReader::new(data),
                archive_encoding,
                config,
                filename,
            )?))
        } else {
            let f = std::fs::File::open(filename)?;
            let reader = std::io::BufReader::new(f);
            Ok(Box::new(BgiArchive::new(
                reader,
                archive_encoding,
                config,
                filename,
            )?))
        }
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
        Ok(Box::new(BgiArchive::new(
            reader,
            archive_encoding,
            config,
            filename,
        )?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["arc"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::BGIArcV2
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 12 && buf.starts_with(b"BURIKO ARC20") {
            return Some(255);
        }
        None
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
        Ok(Box::new(BgiArchiveWriter::new(
            writer, files, encoding, config,
        )?))
    }
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
struct BgiFileHeader {
    #[fstring = 0x60]
    filename: String,
    offset: u32,
    size: u32,
    #[fvec = 8]
    _unk: Vec<u8>,
    #[fvec = 16]
    _padding: Vec<u8>,
}

struct Entry<T: Read + Seek> {
    header: BgiFileHeader,
    reader: Arc<Mutex<T>>,
    pos: usize,
    base_offset: u64,
    script_type: Option<ScriptType>,
}

impl<T: Read + Seek> ArchiveContent for Entry<T> {
    fn name(&self) -> &str {
        &self.header.filename
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
        reader.seek(SeekFrom::Start(
            self.base_offset + self.header.offset as u64 + self.pos as u64,
        ))?;
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

struct MemEntry<F: Fn(&[u8], usize, &str) -> Option<&'static ScriptType>> {
    name: String,
    data: MemReader,
    detect: F,
}

impl<F: Fn(&[u8], usize, &str) -> Option<&'static ScriptType>> Read for MemEntry<F> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.data.read(buf)
    }
}

impl<F: Fn(&[u8], usize, &str) -> Option<&'static ScriptType>> ArchiveContent for MemEntry<F> {
    fn name(&self) -> &str {
        &self.name
    }

    fn script_type(&self) -> Option<&ScriptType> {
        (self.detect)(&self.data.data, self.data.data.len(), &self.name)
    }

    fn data(&mut self) -> Result<Vec<u8>> {
        Ok(self.data.data.clone())
    }

    fn to_data<'a>(&'a mut self) -> Result<Box<dyn ReadSeek + 'a>> {
        Ok(Box::new(&mut self.data))
    }
}

#[derive(Debug)]
/// BGI Archive Version 2
pub struct BgiArchive<T: Read + Seek + std::fmt::Debug> {
    reader: Arc<Mutex<T>>,
    entries: Vec<BgiFileHeader>,
    base_offset: u64,
    #[cfg(feature = "bgi-img")]
    is_sysgrp_arc: bool,
}

impl<T: Read + Seek + std::fmt::Debug> BgiArchive<T> {
    /// Creates a new BGI Archive from a reader.
    ///
    /// * `reader` - The reader to read the archive from.
    /// * `archive_encoding` - The encoding used for the archive.
    /// * `config` - Extra configuration options.
    /// * `filename` - The name of the archive file (used for detecting sysgrp.arc).
    pub fn new(
        mut reader: T,
        archive_encoding: Encoding,
        _config: &ExtraConfig,
        _filename: &str,
    ) -> Result<Self> {
        let mut header = [0u8; 12];
        reader.read_exact(&mut header)?;
        if !header.starts_with(b"BURIKO ARC20") {
            return Err(anyhow::anyhow!("Invalid BGI archive header"));
        }

        let file_count = reader.read_u32()?;
        let mut entries = Vec::with_capacity(file_count as usize);
        for _ in 0..file_count {
            let entry = BgiFileHeader::unpack(&mut reader, false, archive_encoding)?;
            entries.push(entry);
        }

        #[cfg(feature = "bgi-img")]
        let is_sysgrp_arc = _config.bgi_is_sysgrp_arc.unwrap_or_else(|| {
            std::path::Path::new(&_filename.to_lowercase())
                .file_name()
                .map(|f| f == "sysgrp.arc")
                .unwrap_or(false)
        });

        Ok(BgiArchive {
            reader: Arc::new(Mutex::new(reader)),
            entries,
            base_offset: 16 + (file_count as u64 * 0x80),
            #[cfg(feature = "bgi-img")]
            is_sysgrp_arc,
        })
    }
}

impl<T: Read + Seek + std::fmt::Debug + 'static> Script for BgiArchive<T> {
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
            self.entries.iter().map(|e| Ok(e.filename.clone())),
        ))
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
            base_offset: self.base_offset,
            script_type: None,
        };
        let mut buf = [0u8; 32];
        match entry.read(&mut buf) {
            Ok(_) => {}
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Failed to read entry '{}': {}",
                    entry.header.filename,
                    e
                ));
            }
        }
        entry.pos = 0;
        if buf.starts_with(b"DSC FORMAT 1.00") {
            let data = match entry.data() {
                Ok(data) => data,
                Err(e) => {
                    return Err(anyhow::anyhow!(
                        "Failed to read DSC data for '{}': {}",
                        entry.header.filename,
                        e
                    ));
                }
            };
            entry.pos = 0;
            let dsc = match DscDecoder::new(&data) {
                Ok(dsc) => dsc,
                Err(e) => {
                    return Err(anyhow::anyhow!(
                        "Failed to create DSC decoder for '{}': {}",
                        entry.header.filename,
                        e
                    ));
                }
            };
            let decoded = match dsc.unpack() {
                Ok(decoded) => decoded,
                Err(e) => {
                    return Err(anyhow::anyhow!(
                        "Failed to unpack DSC data for '{}': {}",
                        entry.header.filename,
                        e
                    ));
                }
            };
            let reader = MemReader::new(decoded);
            if reader.data.starts_with(b"BSE 1.") {
                match BseReader::new(reader, detect_script_type, &entry.header.filename) {
                    Ok(bse_reader) => {
                        return Ok(Box::new(bse_reader));
                    }
                    Err(e) => {
                        return Err(anyhow::anyhow!(
                            "Failed to create BSE reader for '{}': {}",
                            entry.header.filename,
                            e
                        ));
                    }
                };
            }
            return Ok(Box::new(MemEntry {
                name: entry.header.filename.clone(),
                data: reader,
                #[cfg(feature = "bgi-img")]
                detect: if self.is_sysgrp_arc {
                    detect_script_type_sysgrp
                } else {
                    detect_script_type
                },
                #[cfg(not(feature = "bgi-img"))]
                detect: detect_script_type,
            }));
        }
        if buf.starts_with(b"BSE 1.") {
            let filename = entry.header.filename.clone();
            #[cfg(feature = "bgi-img")]
            let detect = if self.is_sysgrp_arc {
                detect_script_type_sysgrp
            } else {
                detect_script_type
            };
            #[cfg(not(feature = "bgi-img"))]
            let detect = detect_script_type;
            match BseReader::new(entry, detect, &filename) {
                Ok(mut bse_reader) => {
                    if bse_reader.is_dsc() {
                        let data = match bse_reader.data() {
                            Ok(data) => data,
                            Err(e) => {
                                return Err(anyhow::anyhow!(
                                    "Failed to read BSE data for '{}': {}",
                                    &filename,
                                    e
                                ));
                            }
                        };
                        let dsc = match DscDecoder::new(&data) {
                            Ok(dsc) => dsc,
                            Err(e) => {
                                return Err(anyhow::anyhow!(
                                    "Failed to create DSC decoder for '{}': {}",
                                    &filename,
                                    e
                                ));
                            }
                        };
                        let decoded = match dsc.unpack() {
                            Ok(decoded) => decoded,
                            Err(e) => {
                                return Err(anyhow::anyhow!(
                                    "Failed to unpack DSC data for '{}': {}",
                                    &filename,
                                    e
                                ));
                            }
                        };
                        let reader = MemReader::new(decoded);
                        return Ok(Box::new(MemEntry {
                            name: filename,
                            data: reader,
                            detect,
                        }));
                    }
                    return Ok(Box::new(bse_reader));
                }
                Err(e) => {
                    return Err(anyhow::anyhow!(
                        "Failed to create BSE reader for '{}': {}",
                        &filename,
                        e
                    ));
                }
            };
        }
        #[cfg(feature = "bgi-img")]
        if self.is_sysgrp_arc {
            entry.script_type = Some(ScriptType::BGIImg);
        } else {
            entry.script_type =
                detect_script_type(&buf, buf.len(), &entry.header.filename).cloned();
        }
        #[cfg(not(feature = "bgi-img"))]
        {
            entry.script_type =
                detect_script_type(&buf, buf.len(), &entry.header.filename).cloned();
        }
        Ok(Box::new(entry))
    }
}

fn detect_script_type(buf: &[u8], buf_len: usize, filename: &str) -> Option<&'static ScriptType> {
    if buf_len >= 28 && buf.starts_with(b"BurikoCompiledScriptVer1.00\0") {
        return Some(&ScriptType::BGI);
    }
    #[cfg(feature = "bgi-img")]
    if buf_len >= 16 && buf.starts_with(b"CompressedBG___") {
        return Some(&ScriptType::BGICbg);
    }
    #[cfg(feature = "bgi-audio")]
    if buf_len >= 8 && buf[4..].starts_with(b"bw  ") {
        return Some(&ScriptType::BGIAudio);
    }
    let filename = filename.to_lowercase();
    if filename.ends_with("._bp") {
        return Some(&ScriptType::BGIBp);
    } else if filename.ends_with("._bsi") {
        return Some(&ScriptType::BGIBsi);
    }
    None
}

#[cfg(feature = "bgi-img")]
fn detect_script_type_sysgrp(
    _buf: &[u8],
    _buf_len: usize,
    _filename: &str,
) -> Option<&'static ScriptType> {
    Some(&ScriptType::BGIImg)
}

/// BGI Archive Writer for Version 2
pub struct BgiArchiveWriter<T: Write + Seek> {
    writer: T,
    headers: HashMap<String, BgiFileHeader>,
    compress_file: bool,
    encoding: Encoding,
    min_len: usize,
}

impl<T: Write + Seek> BgiArchiveWriter<T> {
    /// Creates a new BGI Archive Writer.
    ///
    /// * `writer` - The writer to write the archive to.
    /// * `files` - The list of files to include in the archive.
    /// * `encoding` - The encoding used for the archive.
    /// * `config` - Extra configuration options.
    pub fn new(
        mut writer: T,
        files: &[&str],
        encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<Self> {
        writer.write_all(b"BURIKO ARC20")?;
        let file_count = files.len();
        writer.write_u32(file_count as u32)?;
        let mut headers = HashMap::new();
        for file in files {
            let header = BgiFileHeader {
                filename: file.to_string(),
                offset: 0,
                size: 0,
                _unk: vec![0; 8],
                _padding: vec![0; 16],
            };
            header.pack(&mut writer, false, encoding)?;
            headers.insert(file.to_string(), header);
        }
        Ok(BgiArchiveWriter {
            writer,
            headers,
            compress_file: config.bgi_compress_file,
            encoding,
            min_len: config.bgi_compress_min_len,
        })
    }
}

impl<T: Write + Seek> Archive for BgiArchiveWriter<T> {
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
        let file = BgiArchiveFile {
            header: entry,
            writer: &mut self.writer,
            pos: 0,
        };
        Ok(if self.compress_file {
            Box::new(BgiArchiveFileWithDsc::new(file, self.min_len))
        } else {
            Box::new(file)
        })
    }

    fn write_header(&mut self) -> Result<()> {
        self.writer.seek(SeekFrom::Start(0x10))?;
        let base_offset = self.headers.len() as u32 * 0x80 + 16;
        let mut files = self.headers.iter_mut().map(|(_, d)| d).collect::<Vec<_>>();
        files.sort_by_key(|f| f.offset);
        for file in files {
            file.offset -= base_offset;
            file.pack(&mut self.writer, false, self.encoding)?;
        }
        Ok(())
    }
}

/// BGI Archive File Writer (Not compressed)
pub struct BgiArchiveFile<'a, T: Write + Seek> {
    header: &'a mut BgiFileHeader,
    writer: &'a mut T,
    pos: usize,
}

impl<'a, T: Write + Seek> Write for BgiArchiveFile<'a, T> {
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

impl<'a, T: Write + Seek> Seek for BgiArchiveFile<'a, T> {
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

/// BGI Archive File Writer with DSC compression
pub struct BgiArchiveFileWithDsc<'a, T: Write + Seek> {
    writer: BgiArchiveFile<'a, T>,
    buf: MemWriter,
    min_len: usize,
}

impl<'a, T: Write + Seek> BgiArchiveFileWithDsc<'a, T> {
    /// Creates a new BGI Archive File Writer with DSC compression.
    ///
    /// * `writer` - The writer to write the archive file to.
    /// * `min_len` - The minimum length for LZSS compression.
    pub fn new(writer: BgiArchiveFile<'a, T>, min_len: usize) -> Self {
        BgiArchiveFileWithDsc {
            writer,
            buf: MemWriter::new(),
            min_len,
        }
    }
}

impl<'a, T: Write + Seek> Write for BgiArchiveFileWithDsc<'a, T> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buf.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.buf.flush()
    }
}

impl<'a, T: Write + Seek> Seek for BgiArchiveFileWithDsc<'a, T> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.buf.seek(pos)
    }

    fn stream_position(&mut self) -> std::io::Result<u64> {
        self.buf.stream_position()
    }

    fn rewind(&mut self) -> std::io::Result<()> {
        self.buf.rewind()
    }
}

impl<'a, T: Write + Seek> Drop for BgiArchiveFileWithDsc<'a, T> {
    fn drop(&mut self) {
        let buf = self.buf.as_slice();
        let encoder = DscEncoder::new(&mut self.writer, self.min_len);
        if let Err(e) = encoder.pack(&buf) {
            eprintln!("Failed to write DSC data: {}", e);
            crate::COUNTER.inc_error();
        }
    }
}
