//! CatSystem2 Archive File (.int)
use super::twister::MersenneTwister;
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::crc32::CRC32NORMAL_TABLE;
use crate::utils::encoding::{decode_to_string, encode_string};
use anyhow::Result;
use blowfish::Blowfish;
use blowfish::cipher::KeyInit;
use std::io::{Read, Seek, SeekFrom};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
/// Builder for CatSystem2 Archive scripts.
pub struct CSIntArcBuilder {}

impl CSIntArcBuilder {
    /// Creates a new instance of `CSIntArcBuilder`.
    pub fn new() -> Self {
        CSIntArcBuilder {}
    }
}

impl ScriptBuilder for CSIntArcBuilder {
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
        Ok(Box::new(CSIntArc::new(
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
            Ok(Box::new(CSIntArc::new(
                MemReader::new(data),
                archive_encoding,
                config,
                filename,
            )?))
        } else {
            let f = std::fs::File::open(filename)?;
            let reader = std::io::BufReader::new(f);
            Ok(Box::new(CSIntArc::new(
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
        Ok(Box::new(CSIntArc::new(
            reader,
            archive_encoding,
            config,
            filename,
        )?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["int"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::CatSystemInt
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 4 && buf.starts_with(b"KIF\0") {
            return Some(10);
        }
        None
    }

    fn is_archive(&self) -> bool {
        true
    }
}

fn detect_script_type(buf: &[u8], buf_len: usize, _filename: &str) -> Option<&'static ScriptType> {
    #[cfg(feature = "cat-system-img")]
    if buf_len >= 4 && buf.starts_with(b"HG-3") {
        return Some(&ScriptType::CatSystemHg3);
    }
    if buf_len >= 8 && buf.starts_with(b"CatScene") {
        return Some(&ScriptType::CatSystem);
    }
    if buf_len >= 4 && buf.starts_with(b"CSTL") {
        return Some(&ScriptType::CatSystemCstl);
    }
    None
}

#[derive(Clone, Debug)]
struct CSIntFileHeader {
    name: String,
    offset: u32,
    size: u32,
}

struct Entry<T: Read + Seek> {
    header: CSIntFileHeader,
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

struct MemEntry {
    name: String,
    data: MemReader,
}

impl Read for MemEntry {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.data.read(buf)
    }
}

impl ArchiveContent for MemEntry {
    fn name(&self) -> &str {
        &self.name
    }

    fn script_type(&self) -> Option<&ScriptType> {
        detect_script_type(&self.data.data, self.data.data.len(), &self.name)
    }

    fn data(&mut self) -> Result<Vec<u8>> {
        Ok(self.data.data.clone())
    }

    fn to_data<'a>(&'a mut self) -> Result<Box<dyn ReadSeek + 'a>> {
        Ok(Box::new(&mut self.data))
    }
}

#[derive(Debug)]
/// CatSystem2 Archive script.
pub struct CSIntArc<T: Read + Seek + std::fmt::Debug> {
    reader: Arc<Mutex<T>>,
    encrypt: Option<Blowfish>,
    entries: Vec<CSIntFileHeader>,
}

const NAME_SIZES: [usize; 2] = [0x20, 0x40];

impl<T: Read + Seek + std::fmt::Debug> CSIntArc<T> {
    /// Creates a new instance of `CSIntArc` from a reader.
    ///
    /// * `reader` - The reader to read the archive from.
    /// * `archive_encoding` - The encoding used for the archive.
    /// * `config` - Extra configuration options.
    /// * `filename` - The name of the file.
    pub fn new(
        mut reader: T,
        archive_encoding: Encoding,
        config: &ExtraConfig,
        _filename: &str,
    ) -> Result<Self> {
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;
        if &magic != b"KIF\0" {
            return Err(anyhow::anyhow!(
                "Invalid magic number for CatSystem2 archive"
            ));
        }
        let entry_count = reader.read_u32()?;
        let mut keybuf = [0u8; 12];
        reader.read_exact(&mut keybuf)?;
        if &keybuf == b"__key__.dat\0" {
            let key = match &config.cat_system_int_encrypt_password {
                Some(password) => Self::get_key(password)?,
                None => {
                    return Err(anyhow::anyhow!(
                        "CatSystem2 archive requires encryption password. Please use --cat-system-int-encrypt-password option."
                    ));
                }
            };
            eprintln!("Using CatSystem2 archive encryption key: {key:08X}");
            let seed = reader.peek_u32_at(0x4C)?;
            let mut twister = MersenneTwister::new(seed);
            let blowfish_key = twister.rand().to_le_bytes();
            let encrypt = match Blowfish::new_from_slice(&blowfish_key) {
                Ok(bf) => bf,
                Err(e) => {
                    return Err(anyhow::anyhow!("Failed to create Blowfish cipher: {}", e));
                }
            };
            let mut entries = Vec::with_capacity(entry_count as usize - 1);
            let mut name_buf = [0u8; 0x40];
            reader.seek(SeekFrom::Start(0x50))?;
            for i in 1..entry_count {
                reader.read_exact(&mut name_buf)?;
                let offset = reader.read_u32()? + i;
                let size = reader.read_u32()?;
                let decryped = encrypt.decrypt([offset, size]);
                twister.s_rand(key + i);
                let name_key = twister.rand();
                let name = Self::decrypt_name(&mut name_buf, name_key, archive_encoding)?;
                let entry = CSIntFileHeader {
                    name,
                    offset: decryped[0],
                    size: decryped[1],
                };
                entries.push(entry);
            }
            return Ok(CSIntArc {
                reader: Arc::new(Mutex::new(reader)),
                encrypt: Some(encrypt),
                entries: entries,
            });
        }
        let file_size = reader.seek(SeekFrom::End(0))?;
        let mut entries = Vec::with_capacity(entry_count as usize);
        for size in NAME_SIZES {
            reader.seek(SeekFrom::Start(0x8))?;
            for _ in 0..entry_count {
                let name = reader.read_fstring(size, archive_encoding, true)?;
                if name.is_empty() {
                    entries.clear();
                    break;
                }
                let current_offset = reader.stream_position()?;
                let offset = reader.read_u32()?;
                let size = reader.read_u32()?;
                if offset as u64 <= current_offset
                    || !((offset as u64) < file_size
                        && size as u64 <= file_size
                        && offset as u64 <= file_size as u64 - size as u64)
                {
                    entries.clear();
                    break;
                }
                let entry = CSIntFileHeader { name, offset, size };
                entries.push(entry);
            }
            if !entries.is_empty() {
                return Ok(CSIntArc {
                    reader: Arc::new(Mutex::new(reader)),
                    encrypt: None,
                    entries,
                });
            }
        }
        Err(anyhow::anyhow!(
            "Failed to parse archives. Maybe another name length is used? (expected 0x20 or 0x40)",
        ))
    }

    fn decrypt_name(name: &mut [u8; 0x40], key: u32, encoding: Encoding) -> Result<String> {
        let mut k = ((key >> 24) + (key >> 16) + (key >> 8) + key) & 0xFF;
        let mut i = 0;
        while i < 0x40 && name[i] != 0 {
            let v = name[i];
            if v.is_ascii_alphabetic() {
                let mut j = if v.is_ascii_lowercase() {
                    b'z' - v
                } else {
                    b'Z' - v + 26
                } as i8;
                j -= (k % 0x34) as i8;
                if j < 0 {
                    j += 0x34;
                }
                j = 0x33 - j;
                name[i] = if j < 26 {
                    b'z' - j as u8
                } else {
                    b'Z' - (j as u8 - 26)
                };
            }
            k += 1;
            i += 1;
        }
        decode_to_string(encoding, &name[..i], true)
    }

    fn get_key(password: &str) -> Result<u32> {
        let bytes = encode_string(Encoding::Cp932, password, true)?;
        let mut key = 0xFFFFFFFF;
        for &c in bytes.iter() {
            key = !CRC32NORMAL_TABLE[((key >> 24) ^ c as u32) as usize] ^ (key << 8);
        }
        Ok(key)
    }
}

impl<T: Read + Seek + std::fmt::Debug + 'static> Script for CSIntArc<T> {
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
        if let Some(encrypt) = &self.encrypt {
            let mut data = entry.data()?;
            entry.pos = 0;
            for i in 0..data.len() / 8 {
                let j = i * 8;
                let l = data[j] as u32
                    | (data[j + 1] as u32) << 8
                    | (data[j + 2] as u32) << 16
                    | (data[j + 3] as u32) << 24;
                let r = data[j + 4] as u32
                    | (data[j + 5] as u32) << 8
                    | (data[j + 6] as u32) << 16
                    | (data[j + 7] as u32) << 24;
                let result = encrypt.decrypt([l, r]);
                data[j..j + 4].copy_from_slice(&result[0].to_le_bytes());
                data[j + 4..j + 8].copy_from_slice(&result[1].to_le_bytes());
            }
            return Ok(Box::new(MemEntry {
                name: entry.header.name.clone(),
                data: MemReader::new(data),
            }));
        }
        let mut buf = [0u8; 32];
        let buf_len = match entry.read(&mut buf) {
            Ok(len) => len,
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Failed to read entry '{}': {}",
                    entry.header.name,
                    e
                ));
            }
        };
        entry.pos = 0;
        entry.script_type = detect_script_type(&buf, buf_len, &entry.header.name).copied();
        Ok(Box::new(entry))
    }
}
