//! Softpal PAC archive (.pac)
use super::*;
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use anyhow::{Result, anyhow, ensure};
use std::io::{Read, Seek, SeekFrom};
use std::sync::{Arc, Mutex};

const SOFTPAL_INDEX_OFFSET: u64 = 0x3FE;
const AMUSE_INDEX_OFFSET: u64 = 0x804;
const XOR_KEY: u32 = 0xF7D5859D;

#[derive(Debug, Clone, Copy)]
enum SoftpalPacVariant {
    Softpal,
    Amuse,
}

#[derive(Debug)]
/// Softpal PAC archive builder.
pub struct SoftpalPacBuilder {
    variant: SoftpalPacVariant,
}

impl SoftpalPacBuilder {
    /// Creates a builder for the classic Softpal PAC layout.
    pub fn new() -> Self {
        Self {
            variant: SoftpalPacVariant::Softpal,
        }
    }

    /// Creates a builder for the Amuse Craft PAC layout.
    pub fn new_amuse() -> Self {
        Self {
            variant: SoftpalPacVariant::Amuse,
        }
    }
}

impl ScriptBuilder for SoftpalPacBuilder {
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
        Ok(Box::new(SoftpalPacArchive::new(
            MemReader::new(buf),
            archive_encoding,
            config,
            self.variant,
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
        let file = std::fs::File::open(filename)?;
        let reader = std::io::BufReader::new(file);
        Ok(Box::new(SoftpalPacArchive::new(
            reader,
            archive_encoding,
            config,
            self.variant,
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
        Ok(Box::new(SoftpalPacArchive::new(
            reader,
            archive_encoding,
            config,
            self.variant,
        )?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["pac"]
    }

    fn script_type(&self) -> &'static ScriptType {
        match self.variant {
            SoftpalPacVariant::Softpal => &ScriptType::SoftpalPac,
            SoftpalPacVariant::Amuse => &ScriptType::SoftpalPacAmuse,
        }
    }

    fn is_archive(&self) -> bool {
        true
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        match self.variant {
            SoftpalPacVariant::Softpal => None,
            SoftpalPacVariant::Amuse => {
                if buf_len >= 4 && buf.starts_with(b"PAC ") {
                    Some(10)
                } else {
                    None
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
struct SoftpalPacEntry {
    name: String,
    offset: u32,
    size: u32,
}

#[derive(Debug)]
/// Softpal PAC archive reader.
pub struct SoftpalPacArchive<T: Read + Seek + std::fmt::Debug> {
    reader: Arc<Mutex<T>>,
    entries: Vec<SoftpalPacEntry>,
}

impl<T: Read + Seek + std::fmt::Debug> SoftpalPacArchive<T> {
    fn new(
        mut reader: T,
        archive_encoding: Encoding,
        _config: &ExtraConfig,
        variant: SoftpalPacVariant,
    ) -> Result<Self> {
        let encoding = match archive_encoding {
            Encoding::Auto => Encoding::Cp932,
            other => other,
        };
        let file_len = reader.stream_length()?;
        if let SoftpalPacVariant::Amuse = variant {
            let signature = reader.peek_u32_at(0)?;
            ensure!(
                signature == 0x2043_4150,
                "Invalid Softpal PAC/Amuse signature: {signature:08X}"
            );
        }

        let count_offset = match variant {
            SoftpalPacVariant::Softpal => 0,
            SoftpalPacVariant::Amuse => 8,
        };
        let count = reader.peek_i32_at(count_offset)?;
        ensure!(count >= 0, "Negative entry count: {count}");
        let count = count as usize;

        if count == 0 {
            return Ok(Self {
                reader: Arc::new(Mutex::new(reader)),
                entries: Vec::new(),
            });
        }

        let (index_offset, name_length) = match variant {
            SoftpalPacVariant::Softpal => {
                let mut chosen = None;
                for &candidate in &[0x20usize, 0x10usize] {
                    let first_offset =
                        reader.peek_u32_at(SOFTPAL_INDEX_OFFSET + candidate as u64 + 4)? as u64;
                    let expected = SOFTPAL_INDEX_OFFSET + (candidate as u64 + 8) * count as u64;
                    if first_offset == expected {
                        ensure!(
                            first_offset <= file_len,
                            "First entry offset {first_offset:#X} exceeds archive length {file_len:#X}"
                        );
                        chosen = Some((SOFTPAL_INDEX_OFFSET, candidate));
                        break;
                    }
                }
                chosen.ok_or_else(|| anyhow!("Unsupported Softpal PAC layout"))?
            }
            SoftpalPacVariant::Amuse => {
                let name_length = 0x20usize;
                let first_offset =
                    reader.peek_u32_at(AMUSE_INDEX_OFFSET + name_length as u64 + 4)? as u64;
                let expected = AMUSE_INDEX_OFFSET + (name_length as u64 + 8) * count as u64;
                ensure!(
                    first_offset == expected,
                    "Invalid Softpal PAC/Amuse index layout: expected {expected:#X}, got {first_offset:#X}"
                );
                ensure!(
                    first_offset <= file_len,
                    "First entry offset {first_offset:#X} exceeds archive length {file_len:#X}"
                );
                (AMUSE_INDEX_OFFSET, name_length)
            }
        };

        reader.seek(SeekFrom::Start(index_offset))?;
        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            let name = reader.read_fstring(name_length, encoding, true)?;
            let size = reader.read_u32()?;
            let offset = reader.read_u32()?;
            let end = offset as u64 + size as u64;
            ensure!(
                end <= file_len,
                "Entry '{name}' exceeds archive bounds: offset={offset:#X}, size={size:#X}"
            );
            entries.push(SoftpalPacEntry { name, offset, size });
        }

        Ok(Self {
            reader: Arc::new(Mutex::new(reader)),
            entries,
        })
    }
}

impl<T: Read + Seek + std::fmt::Debug + 'static> Script for SoftpalPacArchive<T> {
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
        Ok(Box::new(
            self.entries.iter().map(|entry| Ok(entry.offset as u64)),
        ))
    }

    fn open_file<'a>(&'a self, index: usize) -> Result<Box<dyn ArchiveContent + 'a>> {
        let entry = self
            .entries
            .get(index)
            .ok_or_else(|| anyhow!("Index out of bounds: {index}"))?;
        let mut buf = [0u8; 16];
        let buflen = self.reader.cpeek_at(entry.offset as u64, &mut buf)?;
        let script_type = detect_script_type(&entry.name, &buf[..buflen]);
        if buflen >= 16 && should_decrypt_entry(&buf) {
            let mut data = vec![0u8; entry.size as usize];
            self.reader.cpeek_exact_at(entry.offset as u64, &mut data)?;
            decrypt_entry(&mut data);
            Ok(Box::new(MemEntry::new(
                entry.name.clone(),
                data,
                script_type,
            )))
        } else {
            Ok(Box::new(PacEntry::new(
                entry.clone(),
                self.reader.clone(),
                script_type,
            )))
        }
    }
}

fn should_decrypt_entry(data: &[u8]) -> bool {
    data.len() > 16 && data[0] == b'$'
}

fn decrypt_entry(data: &mut [u8]) {
    if data.len() <= 16 {
        return;
    }
    let mut shift: u32 = 4;
    for chunk in data[16..].chunks_exact_mut(4) {
        let mut block = [0u8; 4];
        block.copy_from_slice(chunk);
        let rotate = (shift & 7) as u32;
        block[0] = block[0].rotate_left(rotate);
        shift = shift.wrapping_add(1);
        let decrypted = u32::from_le_bytes(block) ^ XOR_KEY;
        chunk.copy_from_slice(&decrypted.to_le_bytes());
    }
}

#[derive(Debug)]
struct MemEntry {
    name: String,
    data: Vec<u8>,
    pos: usize,
    script_type: Option<ScriptType>,
}

impl MemEntry {
    pub fn new(name: String, data: Vec<u8>, script_type: Option<ScriptType>) -> Self {
        Self {
            name,
            data,
            pos: 0,
            script_type,
        }
    }
}

impl ArchiveContent for MemEntry {
    fn name(&self) -> &str {
        &self.name
    }

    fn script_type(&self) -> Option<&ScriptType> {
        self.script_type.as_ref()
    }
}

impl Read for MemEntry {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.pos >= self.data.len() {
            return Ok(0);
        }
        let bytes_to_read = buf.len().min(self.data.len() - self.pos);
        if bytes_to_read == 0 {
            return Ok(0);
        }
        buf[..bytes_to_read].copy_from_slice(&self.data[self.pos..self.pos + bytes_to_read]);
        self.pos += bytes_to_read;
        Ok(bytes_to_read)
    }
}

impl Seek for MemEntry {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let len = self.data.len() as i64;
        let current = self.pos as i64;
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::End(offset) => len + offset,
            SeekFrom::Current(offset) => current + offset,
        };
        if new_pos < 0 || new_pos > len {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Seek position is out of bounds",
            ));
        }
        self.pos = new_pos as usize;
        Ok(self.pos as u64)
    }

    fn stream_position(&mut self) -> std::io::Result<u64> {
        Ok(self.pos as u64)
    }
}

#[derive(Debug)]
struct PacEntry<T: Read + Seek + std::fmt::Debug> {
    header: SoftpalPacEntry,
    pos: u64,
    reader: Arc<Mutex<T>>,
    script_type: Option<ScriptType>,
}

impl<T: Read + Seek + std::fmt::Debug> PacEntry<T> {
    fn new(
        header: SoftpalPacEntry,
        reader: Arc<Mutex<T>>,
        script_type: Option<ScriptType>,
    ) -> Self {
        Self {
            header,
            pos: 0,
            reader,
            script_type,
        }
    }
}

impl<T: Read + Seek + std::fmt::Debug> ArchiveContent for PacEntry<T> {
    fn name(&self) -> &str {
        &self.header.name
    }

    fn script_type(&self) -> Option<&ScriptType> {
        self.script_type.as_ref()
    }
}

impl<T: Read + Seek + std::fmt::Debug> Read for PacEntry<T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.pos >= self.header.size as u64 {
            return Ok(0);
        }
        let bytes_to_read = buf.len().min((self.header.size as u64 - self.pos) as usize);
        if bytes_to_read == 0 {
            return Ok(0);
        }
        let bytes_read = self.reader.cpeek_at(
            self.header.offset as u64 + self.pos,
            &mut buf[..bytes_to_read],
        )?;
        self.pos += bytes_read as u64;
        Ok(bytes_read)
    }
}

impl<T: Read + Seek + std::fmt::Debug> Seek for PacEntry<T> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let len = self.header.size as i64;
        let current = self.pos as i64;
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::End(offset) => len + offset,
            SeekFrom::Current(offset) => current + offset,
        };
        if new_pos < 0 || new_pos > len {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Seek position is out of bounds",
            ));
        }
        self.pos = new_pos as u64;
        Ok(self.pos as u64)
    }

    fn stream_position(&mut self) -> std::io::Result<u64> {
        Ok(self.pos as u64)
    }
}
