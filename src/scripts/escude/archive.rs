use super::crypto::*;
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::{decode_to_string, encode_string};
use anyhow::Result;
use std::collections::HashMap;
use std::ffi::CString;
use std::io::{Read, Seek, SeekFrom, Write};

#[derive(Debug)]
pub struct EscudeBinArchiveBuilder {}

impl EscudeBinArchiveBuilder {
    pub const fn new() -> Self {
        EscudeBinArchiveBuilder {}
    }
}

impl ScriptBuilder for EscudeBinArchiveBuilder {
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
        Ok(Box::new(EscudeBinArchive::new(
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
            Ok(Box::new(EscudeBinArchive::new(
                MemReader::new(data),
                archive_encoding,
                config,
            )?))
        } else {
            let f = std::fs::File::open(filename)?;
            let reader = std::io::BufReader::new(f);
            Ok(Box::new(EscudeBinArchive::new(
                reader,
                archive_encoding,
                config,
            )?))
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
        Ok(Box::new(EscudeBinArchive::new(
            reader,
            archive_encoding,
            config,
        )?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["bin"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::EscudeArc
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len > 8 && buf.starts_with(b"ESC-ARC2") {
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
        let archive = EscudeBinArchiveWriter::new(writer, files, encoding, config)?;
        Ok(Box::new(archive))
    }
}

#[derive(Debug)]
struct BinEntry {
    name_offset: u32,
    data_offset: u32,
    length: u32,
}

struct Entry {
    name: String,
    data: MemReader,
}

impl Read for Entry {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.data.read(buf)
    }
}

impl ArchiveContent for Entry {
    fn name(&self) -> &str {
        &self.name
    }

    fn script_type(&self) -> Option<&ScriptType> {
        if self.data.data.starts_with(b"ESCR1_00") {
            Some(&ScriptType::Escude)
        } else if self.data.data.starts_with(b"LIST") {
            Some(&ScriptType::EscudeList)
        } else {
            None
        }
    }

    fn data(&mut self) -> Result<Vec<u8>> {
        Ok(self.data.data.clone())
    }

    fn to_data<'a>(&'a mut self) -> Result<Box<dyn ReadSeek + 'a>> {
        Ok(Box::new(&mut self.data))
    }
}

#[derive(Debug)]
pub struct EscudeBinArchive<T: Read + Seek + std::fmt::Debug> {
    reader: T,
    file_count: u32,
    entries: Vec<BinEntry>,
    archive_encoding: Encoding,
}

impl<T: Read + Seek + std::fmt::Debug> EscudeBinArchive<T> {
    pub fn new(mut reader: T, archive_encoding: Encoding, _config: &ExtraConfig) -> Result<Self> {
        let mut header = [0u8; 8];
        reader.read_exact(&mut header)?;
        if &header != b"ESC-ARC2" {
            return Err(anyhow::anyhow!("Invalid Escude binary script header"));
        }
        reader.seek(SeekFrom::Start(0xC))?;
        let mut crypto_reader = CryptoReader::new(&mut reader)?;
        let file_count = crypto_reader.read_u32()?;
        let _name_tbl_len = crypto_reader.read_u32()?;
        let mut entries = Vec::with_capacity(file_count as usize);
        for _ in 0..file_count {
            let name_offset = crypto_reader.read_u32()?;
            let data_offset = crypto_reader.read_u32()?;
            let length = crypto_reader.read_u32()?;
            entries.push(BinEntry {
                name_offset,
                data_offset,
                length,
            });
        }
        Ok(EscudeBinArchive {
            reader,
            file_count,
            entries,
            archive_encoding,
        })
    }
}

impl<T: Read + Seek + std::fmt::Debug> Script for EscudeBinArchive<T> {
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
        Ok(Box::new(EscudeBinArchiveIter {
            entries: self.entries.iter(),
            reader: &mut self.reader,
            file_count: self.file_count,
            archive_encoding: self.archive_encoding,
        }))
    }

    fn iter_archive_mut<'a>(
        &'a mut self,
    ) -> Result<Box<dyn Iterator<Item = Result<Box<dyn ArchiveContent>>> + 'a>> {
        Ok(Box::new(EscudeBinArchiveIterator {
            entries: self.entries.iter(),
            reader: &mut self.reader,
            file_count: self.file_count,
            archive_encoding: self.archive_encoding,
        }))
    }
}

struct EscudeBinArchiveIter<'a, T: Iterator<Item = &'a BinEntry>, R: Read + Seek> {
    entries: T,
    reader: &'a mut R,
    file_count: u32,
    archive_encoding: Encoding,
}

impl<'a, T: Iterator<Item = &'a BinEntry>, R: Read + Seek> Iterator
    for EscudeBinArchiveIter<'a, T, R>
{
    type Item = Result<String>;

    fn next(&mut self) -> Option<Self::Item> {
        let entry = match self.entries.next() {
            Some(entry) => entry,
            None => return None,
        };
        let name_offset = entry.name_offset as usize + self.file_count as usize * 12 + 0x14;
        let name = match self.reader.peek_cstring_at(name_offset) {
            Ok(name) => name,
            Err(e) => return Some(Err(e.into())),
        };
        let name = match decode_to_string(self.archive_encoding, name.as_bytes(), true) {
            Ok(name) => name,
            Err(e) => return Some(Err(e.into())),
        };
        Some(Ok(name))
    }
}

struct EscudeBinArchiveIterator<'a, T: Iterator<Item = &'a BinEntry>, R: Read + Seek> {
    entries: T,
    reader: &'a mut R,
    file_count: u32,
    archive_encoding: Encoding,
}

impl<'a, T: Iterator<Item = &'a BinEntry>, R: Read + Seek> Iterator
    for EscudeBinArchiveIterator<'a, T, R>
{
    type Item = Result<Box<dyn ArchiveContent>>;

    fn next(&mut self) -> Option<Self::Item> {
        let entry = match self.entries.next() {
            Some(entry) => entry,
            None => return None,
        };
        let name = match self
            .reader
            .peek_cstring_at(entry.name_offset as usize + self.file_count as usize * 12 + 0x14)
        {
            Ok(name) => name,
            Err(e) => return Some(Err(e.into())),
        };
        let name = match decode_to_string(self.archive_encoding, name.as_bytes(), true) {
            Ok(name) => name,
            Err(e) => return Some(Err(e.into())),
        };
        let mut data = match self
            .reader
            .peek_at_vec(entry.data_offset as usize, entry.length as usize)
        {
            Ok(data) => data,
            Err(e) => return Some(Err(e.into())),
        };
        if data.starts_with(b"acp") {
            let mut decoder = match super::lzw::LZWDecoder::new(&data) {
                Ok(decoder) => decoder,
                Err(e) => return Some(Err(anyhow::anyhow!("Failed to create LZW decoder: {}", e))),
            };
            data = match decoder.unpack() {
                Ok(unpacked_data) => unpacked_data,
                Err(e) => return Some(Err(e)),
            };
        }
        Some(Ok(Box::new(Entry {
            name,
            data: MemReader::new(data),
        })))
    }
}

pub struct EscudeBinArchiveWriter<T: Write + Seek> {
    writer: T,
    headers: HashMap<String, BinEntry>,
    name_tbl_len: u32,
    fake: bool,
}

impl<T: Write + Seek> EscudeBinArchiveWriter<T> {
    pub fn new(
        mut writer: T,
        files: &[&str],
        encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<Self> {
        writer.write_all(b"ESC-ARC2")?;
        let header_len = 0xC + 0xC * files.len();
        let header = vec![0u8; header_len];
        writer.write_all(&header)?;
        let mut headers = HashMap::new();
        for file in files {
            let f = file.to_string();
            let encoded = encode_string(encoding, file, true)?;
            let encoded = CString::new(encoded)?;
            let name_offset = writer.stream_position()? as u32;
            writer.write_all(encoded.as_bytes_with_nul())?;
            headers.insert(
                f,
                BinEntry {
                    name_offset,
                    data_offset: 0,
                    length: 0,
                },
            );
        }
        let name_tbl_len = writer.stream_position()? as u32 - header_len as u32 - 0x8;
        Ok(EscudeBinArchiveWriter {
            writer,
            headers,
            name_tbl_len,
            fake: config.escude_fake_compress,
        })
    }
}

impl<T: Write + Seek> Archive for EscudeBinArchiveWriter<T> {
    fn new_file<'a>(&'a mut self, name: &str) -> Result<Box<dyn WriteSeek + 'a>> {
        let entry = self
            .headers
            .get_mut(name)
            .ok_or_else(|| anyhow::anyhow!("File '{}' not found in archive", name))?;
        if entry.data_offset != 0 {
            return Err(anyhow::anyhow!("File '{}' already exists in archive", name));
        }
        entry.data_offset = self.writer.stream_position()? as u32;
        Ok(Box::new(EscudeBinArchiveFileWithLzw::new(
            entry,
            &mut self.writer,
            self.fake,
        )?))
    }

    fn write_header(&mut self) -> Result<()> {
        self.writer.seek(SeekFrom::Start(0x8))?;
        let mut crypto = CryptoWriter::new(&mut self.writer)?;
        let file_count = self.headers.len() as u32;
        crypto.write_u32(file_count)?;
        crypto.write_u32(self.name_tbl_len)?;
        let mut entries: Vec<_> = self.headers.values().collect();
        entries.sort_by(|a, b| a.name_offset.cmp(&b.name_offset));
        for entry in entries {
            let name_offset = entry.name_offset - file_count * 12 - 0x14;
            crypto.write_u32(name_offset)?;
            crypto.write_u32(entry.data_offset)?;
            crypto.write_u32(entry.length)?;
        }
        Ok(())
    }
}

pub struct EscudeBinArchiveFileWithLzw<'a, T: Write + Seek> {
    writer: EscudeBinArchiveFile<'a, T>,
    buf: MemWriter,
    fake: bool,
}

impl<'a, T: Write + Seek> EscudeBinArchiveFileWithLzw<'a, T> {
    fn new(header: &'a mut BinEntry, writer: &'a mut T, fake: bool) -> Result<Self> {
        let writer = EscudeBinArchiveFile {
            header,
            writer,
            pos: 0,
        };
        Ok(EscudeBinArchiveFileWithLzw {
            writer,
            buf: MemWriter::new(),
            fake,
        })
    }
}

impl<'a, T: Write + Seek> Write for EscudeBinArchiveFileWithLzw<'a, T> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buf.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.buf.flush()
    }
}

impl<'a, T: Write + Seek> Seek for EscudeBinArchiveFileWithLzw<'a, T> {
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

impl<'a, T: Write + Seek> Drop for EscudeBinArchiveFileWithLzw<'a, T> {
    fn drop(&mut self) {
        let buf = self.buf.as_slice();
        let encoder = super::lzw::LZWEncoder::new();
        let data = match encoder.encode(buf, self.fake) {
            Ok(data) => data,
            Err(e) => {
                eprintln!("Failed to encode LZW data: {}", e);
                crate::COUNTER.inc_error();
                return;
            }
        };
        match self.writer.write_all(&data) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Failed to write LZW data: {}", e);
                crate::COUNTER.inc_error();
            }
        }
    }
}

pub struct EscudeBinArchiveFile<'a, T: Write + Seek> {
    header: &'a mut BinEntry,
    writer: &'a mut T,
    pos: usize,
}

impl<'a, T: Write + Seek> Write for EscudeBinArchiveFile<'a, T> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.writer.seek(SeekFrom::Start(
            self.header.data_offset as u64 + self.pos as u64,
        ))?;
        let written = self.writer.write(buf)?;
        self.pos += written;
        self.header.length = self.header.length.max(self.pos as u32);
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

impl<'a, T: Write + Seek> Seek for EscudeBinArchiveFile<'a, T> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as usize,
            SeekFrom::End(offset) => {
                if offset < 0 {
                    if (-offset) as usize > self.header.length as usize {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "Seek from end exceeds file length",
                        ));
                    }
                    self.header.length as usize - (-offset) as usize
                } else {
                    self.header.length as usize + offset as usize
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
