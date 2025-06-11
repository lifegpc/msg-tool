use super::crypto::*;
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::encode_string;
use crate::utils::struct_pack::*;
use anyhow::Result;
use msg_tool_macro::*;
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom, Write};

#[derive(Debug)]
pub struct ItufuruArchiveBuilder {}

impl ItufuruArchiveBuilder {
    pub const fn new() -> Self {
        ItufuruArchiveBuilder {}
    }
}

impl ScriptBuilder for ItufuruArchiveBuilder {
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
        Ok(Box::new(ItufuruArchive::new(
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
            Ok(Box::new(ItufuruArchive::new(
                MemReader::new(data),
                archive_encoding,
                config,
            )?))
        } else {
            let f = std::fs::File::open(filename)?;
            let reader = std::io::BufReader::new(f);
            Ok(Box::new(ItufuruArchive::new(
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
        Ok(Box::new(ItufuruArchive::new(
            reader,
            archive_encoding,
            config,
        )?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["scd"]
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 4 && buf.starts_with(b"SCR\0") {
            Some(1)
        } else {
            None
        }
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::YaneuraoItufuruArc
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
        let archive = ItufuruArchiveWriter::new(writer, files, encoding, config)?;
        Ok(Box::new(archive))
    }
}

#[derive(Debug, StructPack, StructUnpack)]
struct ItufuruFileHeader {
    #[fstring = 12]
    file_name: String,
    offset: u32,
}

#[derive(Debug, StructPack)]
struct CustomHeader {
    #[fstring = 12]
    file_name: String,
    offset: u32,
    #[skip_pack]
    size: u32,
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
        Some(&ScriptType::YaneuraoItufuru)
    }

    fn data(&mut self) -> Result<Vec<u8>> {
        Ok(self.data.data.clone())
    }

    fn to_data<'a>(&'a mut self) -> Result<Box<dyn ReadSeek + 'a>> {
        Ok(Box::new(&mut self.data))
    }
}

#[derive(Debug)]
pub struct ItufuruArchive<T: Read + Seek + std::fmt::Debug> {
    reader: Crypto<T>,
    first_file_offset: u32,
    files: Vec<CustomHeader>,
}

impl<T: Read + Seek + std::fmt::Debug> ItufuruArchive<T> {
    pub fn new(mut reader: T, archive_encoding: Encoding, _config: &ExtraConfig) -> Result<Self> {
        let mut header = [0u8; 4];
        reader.read_exact(&mut header)?;
        if &header != b"SCR\0" {
            return Err(anyhow::anyhow!("Invalid Itufuru archive header"));
        }
        let file_count = reader.read_u32()?;
        let first_file_offset = reader.read_u32()?;
        reader.read_u32()?; // Skip unused field
        let mut reader = Crypto::new(reader, 0xA5);
        let mut tfiles = Vec::with_capacity(file_count as usize);
        for _ in 0..file_count {
            let file = ItufuruFileHeader::unpack(&mut reader, false, archive_encoding)?;
            tfiles.push(file);
        }
        let mut files = Vec::with_capacity(tfiles.len());
        if !tfiles.is_empty() {
            for i in 0..tfiles.len() - 1 {
                let file = CustomHeader {
                    file_name: tfiles[i].file_name.clone(),
                    offset: tfiles[i].offset,
                    size: tfiles[i + 1].offset - tfiles[i].offset,
                };
                files.push(file);
            }
            let last_file = &tfiles[tfiles.len() - 1];
            let file = CustomHeader {
                file_name: last_file.file_name.clone(),
                offset: last_file.offset,
                size: reader.seek(SeekFrom::End(0))? as u32 - last_file.offset - first_file_offset,
            };
            files.push(file);
        }
        Ok(ItufuruArchive {
            reader,
            first_file_offset,
            files,
        })
    }
}

impl<T: Read + Seek + std::fmt::Debug> Script for ItufuruArchive<T> {
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
        Ok(Box::new(
            self.files.iter().map(|s| Ok(s.file_name.to_owned())),
        ))
    }

    fn iter_archive_mut<'a>(
        &'a mut self,
    ) -> Result<Box<dyn Iterator<Item = Result<Box<dyn ArchiveContent>>> + 'a>> {
        Ok(Box::new(ItufuruArchiveIter {
            entries: self.files.iter(),
            reader: &mut self.reader,
            first_file_offset: self.first_file_offset,
        }))
    }
}

struct ItufuruArchiveIter<'a, T: Iterator<Item = &'a CustomHeader>, R: Read + Seek> {
    entries: T,
    reader: &'a mut R,
    first_file_offset: u32,
}

impl<'a, T: Iterator<Item = &'a CustomHeader>, R: Read + Seek> Iterator
    for ItufuruArchiveIter<'a, T, R>
{
    type Item = Result<Box<dyn ArchiveContent>>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(entry) = self.entries.next() {
            let file_offset = entry.offset as usize;
            match self.reader.peek_extract_at_vec(
                file_offset + self.first_file_offset as usize,
                entry.size as usize,
            ) {
                Ok(data) => {
                    let name = entry.file_name.clone();
                    Some(Ok(Box::new(Entry {
                        name,
                        data: MemReader::new(data),
                    })))
                }
                Err(e) => Some(Err(anyhow::anyhow!(
                    "Failed to read file {}: {}",
                    entry.file_name,
                    e
                ))),
            }
        } else {
            None
        }
    }
}

pub struct ItufuruArchiveWriter<T: Write + Seek> {
    writer: T,
    headers: HashMap<String, CustomHeader>,
    first_file_offset: u32,
    encoding: Encoding,
}

impl<T: Write + Seek> ItufuruArchiveWriter<T> {
    pub fn new(
        mut writer: T,
        files: &[&str],
        encoding: Encoding,
        _config: &ExtraConfig,
    ) -> Result<Self> {
        writer.write_all(b"SCR\0")?;
        let file_count = files.len() as u32;
        writer.write_u32(file_count)?;
        let first_file_offset = 0x10 + file_count * 16; // 16 bytes per file header
        writer.write_u32(first_file_offset)?;
        writer.write_u32(0)?; // Unused field
        let mut headers = HashMap::new();
        for file in files {
            headers.insert(
                file.to_string(),
                CustomHeader {
                    file_name: file.to_string(),
                    offset: 0,
                    size: 0,
                },
            );
        }
        let mut crypto = Crypto::new(&mut writer, 0xA5);
        for (_, header) in headers.iter() {
            header.pack(&mut crypto, false, encoding)?;
        }
        Ok(ItufuruArchiveWriter {
            writer,
            headers,
            first_file_offset,
            encoding,
        })
    }
}

impl<T: Write + Seek> Archive for ItufuruArchiveWriter<T> {
    fn new_file<'a>(&'a mut self, name: &str) -> Result<Box<dyn WriteSeek + 'a>> {
        let entry = self
            .headers
            .get_mut(name)
            .ok_or_else(|| anyhow::anyhow!("File '{}' not found in archive", name))?;
        if entry.size != 0 {
            return Err(anyhow::anyhow!("File '{}' already exists in archive", name));
        }
        entry.offset = self.writer.stream_position()? as u32 - self.first_file_offset;
        Ok(Box::new(ItufuruArchiveWriterEntry::new(
            &mut self.writer,
            entry,
            self.first_file_offset,
        )))
    }
    fn write_header(&mut self) -> Result<()> {
        let mut crypto = Crypto::new(&mut self.writer, 0xA5);
        let mut entries = self.headers.values().collect::<Vec<_>>();
        entries.sort_by_key(|h| h.offset);
        crypto.seek(SeekFrom::Start(16))?;
        for entry in entries.iter() {
            entry.pack(&mut crypto, false, self.encoding)?;
        }
        Ok(())
    }
}

pub struct ItufuruArchiveWriterEntry<'a, T: Write + Seek> {
    writer: Crypto<&'a mut T>,
    header: &'a mut CustomHeader,
    first_file_offset: u32,
    pos: usize,
}

impl<'a, T: Write + Seek> ItufuruArchiveWriterEntry<'a, T> {
    fn new(writer: &'a mut T, header: &'a mut CustomHeader, first_file_offset: u32) -> Self {
        let writer = Crypto::new(writer, 0xA5);
        ItufuruArchiveWriterEntry {
            writer,
            header,
            first_file_offset,
            pos: 0,
        }
    }
}

impl<'a, T: Write + Seek> Write for ItufuruArchiveWriterEntry<'a, T> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.writer.seek(SeekFrom::Start(
            self.header.offset as u64 + self.first_file_offset as u64 + self.pos as u64,
        ))?;
        let written = self.writer.write(buf)?;
        self.pos += written;
        self.header.size = self.header.size.max(self.pos as u32);
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

impl<'a, T: Write + Seek> Seek for ItufuruArchiveWriterEntry<'a, T> {
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
