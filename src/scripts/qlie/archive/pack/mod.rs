//! Qlie Pack Archive (.pack)
mod encryption;
mod twister;
mod types;

use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::struct_pack::*;
use anyhow::Result;
use encryption::Encryption;
use std::io::{Read, Seek, SeekFrom};
use std::sync::{Arc, Mutex};
use types::*;

#[derive(Debug)]
pub struct QliePackArchiveBuilder {}

impl QliePackArchiveBuilder {
    pub fn new() -> Self {
        Self {}
    }
}

impl ScriptBuilder for QliePackArchiveBuilder {
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
        Ok(Box::new(QliePackArchive::new(
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
            Ok(Box::new(QliePackArchive::new(
                MemReader::new(data),
                archive_encoding,
                config,
            )?))
        } else {
            let f = std::fs::File::open(filename)?;
            let reader = std::io::BufReader::new(f);
            Ok(Box::new(QliePackArchive::new(
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
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(QliePackArchive::new(
            reader,
            archive_encoding,
            config,
        )?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["pack"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::QliePack
    }

    fn is_this_format(&self, filename: &str, _buf: &[u8], _buf_len: usize) -> Option<u8> {
        // Workround: Check only if filename exists in filesystem.
        // This means that we cannot detect .pack files in another archive.
        // Pack file header is at the end of file, so we cannot check signature here with header buffer.
        match is_this_format(filename) {
            Ok(true) => Some(30),
            _ => None,
        }
    }

    fn is_archive(&self) -> bool {
        true
    }
}

/// Check if the given file is Qlie Pack Archive format
pub fn is_this_format<P: AsRef<std::path::Path> + ?Sized>(path: &P) -> Result<bool> {
    let path = path.as_ref();
    if !path.exists() || !path.is_file() {
        return Ok(false);
    }
    let mut file = std::fs::File::open(path)?;
    file.seek(SeekFrom::End(-0x1C))?;
    let header = QlieHeader::unpack(&mut file, false, Encoding::Utf8, &None)?;
    Ok(header.is_valid())
}

#[derive(Debug)]
pub struct QliePackArchive<T: Read + Seek + std::fmt::Debug> {
    header: QlieHeader,
    encryption: Box<dyn Encryption>,
    reader: Arc<Mutex<T>>,
    qkey: Option<QlieKey>,
    entries: Vec<QlieEntry>,
    common_key: Option<Vec<u8>>,
}

impl<T: Read + Seek + std::fmt::Debug> QliePackArchive<T> {
    pub fn new(mut reader: T, archive_encoding: Encoding, _config: &ExtraConfig) -> Result<Self> {
        reader.seek(SeekFrom::End(-0x1C))?;
        let header = QlieHeader::unpack(&mut reader, false, archive_encoding, &None)?;
        if !header.is_valid() {
            return Err(anyhow::anyhow!("Invalid Qlie Pack Archive header"));
        }
        let file_size = reader.stream_position()?;
        if header.index_offset > file_size - 0x1C {
            return Err(anyhow::anyhow!(
                "Invalid index offset in Qlie Pack Archive header"
            ));
        }
        let major = header.major_version();
        let minor = header.minor_version();
        let encryption = encryption::create_encryption(major, minor)?;
        // Read key
        let mut key = 0;
        let mut qkey = None;
        if major >= 2 {
            reader.seek(SeekFrom::End(-0x440))?;
            let mut qk = QlieKey::unpack(&mut reader, false, archive_encoding, &None)?;
            if qk.hash_size as u64 > file_size || qk.hash_size < 0x44 {
                return Err(anyhow::anyhow!("Invalid Qlie Pack Archive key"));
            }
            if major >= 3 {
                key = encryption.compute_hash(&qk.key[..0x100])? & 0xFFFFFFF;
            }
            encryption::decrypt(&mut qk.signature, key)?;
            if &qk.signature != b"8hr48uky,8ugi8ewra4g8d5vbf5hb5s6" {
                eprintln!(
                    "WARNING: Invalid Qlie Pack Archive key signature, decryption key may be incorrect"
                );
                crate::COUNTER.inc_warning();
            }
            qkey = Some(qk);
        }
        // Read entries
        let mut entries = Vec::new();
        reader.seek(SeekFrom::Start(header.index_offset))?;
        for _ in 0..header.file_count {
            let name_length = reader.read_u16()?;
            let raw_name_length = if encryption.is_unicode() {
                name_length as usize * 2
            } else {
                name_length as usize
            };
            let mut raw_name = reader.read_exact_vec(raw_name_length)?;
            let name = encryption.decrypt_name(&mut raw_name, key as i32, archive_encoding)?;
            let offset = reader.read_u64()?;
            let size = reader.read_u32()?;
            let unpacked_size = reader.read_u32()?;
            let is_packed = reader.read_u32()?;
            let is_encrypted = reader.read_u32()?;
            let hash = reader.read_u32()?;
            let entry = QlieEntry {
                name,
                offset,
                size,
                unpacked_size,
                is_packed,
                is_encrypted,
                hash,
                key,
                common_key: None,
            };
            entries.push(entry);
        }
        let mut common_key = None;
        if major >= 3 && minor >= 1 {
            if let Some(common_key_entry) = entries
                .iter()
                .find(|e| e.name == "pack_keyfile_kfueheish15538fa9or.key")
            {
                reader.seek(SeekFrom::Start(common_key_entry.offset))?;
                let stream = StreamRegion::with_size(&mut reader, common_key_entry.size as u64)?;
                let mut decrypted = encryption.decrypt_entry(Box::new(stream), common_key_entry)?;
                if common_key_entry.is_packed != 0 {
                    decrypted = encryption::decompress(decrypted)?;
                }
                let mut key_data = Vec::new();
                decrypted.read_to_end(&mut key_data)?;
                common_key = Some(encryption::get_common_key(&key_data)?);
            }
        }
        Ok(Self {
            header,
            encryption,
            reader: Arc::new(Mutex::new(reader)),
            qkey,
            entries,
            common_key,
        })
    }
}

impl<T: Read + Seek + std::fmt::Debug + 'static> Script for QliePackArchive<T> {
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

    fn open_file<'a>(&'a self, index: usize) -> Result<Box<dyn ArchiveContent + 'a>> {
        let mut entry = self
            .entries
            .get(index)
            .ok_or_else(|| anyhow::anyhow!("Invalid file index {} for Qlie Pack Archive", index))?
            .clone();
        if self.common_key.is_some() {
            entry.common_key = self.common_key.clone();
        }
        let stream = StreamRegion::with_size(
            MutexWrapper::new(self.reader.clone(), entry.offset),
            entry.size as u64,
        )?;
        let stream_clone = StreamRegion::with_size(
            MutexWrapper::new(self.reader.clone(), entry.offset),
            entry.size as u64,
        )?;
        let mut stream = self.encryption.decrypt_entry(Box::new(stream), &entry)?;
        let mut stream_clone = self
            .encryption
            .decrypt_entry(Box::new(stream_clone), &entry)?;
        if entry.is_packed != 0 {
            stream = encryption::decompress(stream)?;
            stream_clone = encryption::decompress(stream_clone)?;
        }
        let mut entry = QliePackArchiveContent::new(stream, entry);
        let mut header_buffer = [0u8; 1024];
        let readed = stream_clone.read_most(&mut header_buffer)?;
        entry.typ = detect_script_type(&entry.entry.name, &header_buffer, readed);
        Ok(Box::new(entry))
    }
}

fn detect_script_type(_name: &str, buf: &[u8], buf_len: usize) -> Option<ScriptType> {
    if super::super::script::is_this_format(buf, buf_len) {
        Some(ScriptType::Qlie)
    } else {
        None
    }
}

#[derive(Debug)]
struct QliePackArchiveContent<T: Read + std::fmt::Debug> {
    reader: T,
    entry: QlieEntry,
    typ: Option<ScriptType>,
}

impl<T: Read + std::fmt::Debug> QliePackArchiveContent<T> {
    pub fn new(reader: T, entry: QlieEntry) -> Self {
        Self {
            reader,
            entry,
            typ: None,
        }
    }
}

impl<T: Read + std::fmt::Debug> Read for QliePackArchiveContent<T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.reader.read(buf)
    }
}

impl<T: Read + std::fmt::Debug> ArchiveContent for QliePackArchiveContent<T> {
    fn name(&self) -> &str {
        &self.entry.name
    }

    fn script_type(&self) -> Option<&ScriptType> {
        self.typ.as_ref()
    }
}
