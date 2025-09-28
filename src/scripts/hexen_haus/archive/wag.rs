//! HexenHaus WAG archive (.wag)
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::decode_to_string;
use anyhow::{Result, anyhow};
use std::convert::TryFrom;
use std::io::{Read, Seek, SeekFrom};
use std::sync::{Arc, Mutex};

const WAG_SIGNATURE: &[u8; 4] = b"IAF_";
const OFFSET_TABLE_START: u64 = 0x4A;
const DATA_SIGNATURE: u32 = 0x4154_4144; // 'DATA'
const SECTION_IMAGE: u32 = 0x4447_4D49; // 'IMGD'
const SECTION_NAME: u32 = 0x454E_4E46; // 'FNNE'

#[derive(Debug)]
/// HexenHaus WAG archive builder
pub struct HexenHausWagArchiveBuilder;

impl HexenHausWagArchiveBuilder {
    /// Creates a new `HexenHausWagArchiveBuilder`
    pub const fn new() -> Self {
        HexenHausWagArchiveBuilder
    }
}

impl ScriptBuilder for HexenHausWagArchiveBuilder {
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
        Ok(Box::new(HexenHausWagArchive::new(
            MemReader::new(buf),
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
            return Ok(Box::new(HexenHausWagArchive::new(
                MemReader::new(data),
                archive_encoding,
                config,
            )?));
        }
        let file = std::fs::File::open(filename)?;
        let reader = std::io::BufReader::new(file);
        Ok(Box::new(HexenHausWagArchive::new(
            reader,
            archive_encoding,
            config,
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
        Ok(Box::new(HexenHausWagArchive::new(
            reader,
            archive_encoding,
            config,
        )?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["wag"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::HexenHausWag
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= WAG_SIGNATURE.len() && buf.starts_with(WAG_SIGNATURE) {
            Some(10)
        } else {
            None
        }
    }

    fn is_archive(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone)]
struct HexenHausWagEntry {
    name: String,
    offset: u64,
    size: u32,
}

#[derive(Debug)]
/// HexenHaus WAG archive reader
pub struct HexenHausWagArchive<T: Read + Seek + std::fmt::Debug> {
    reader: Arc<Mutex<T>>,
    file_length: u64,
    entries: Vec<HexenHausWagEntry>,
}

impl<T: Read + Seek + std::fmt::Debug> HexenHausWagArchive<T> {
    /// Creates a new `HexenHausWagArchive`
    pub fn new(mut reader: T, archive_encoding: Encoding, _config: &ExtraConfig) -> Result<Self> {
        reader.seek(SeekFrom::Start(0))?;
        let mut signature = [0u8; 4];
        reader.read_exact(&mut signature)?;
        if signature != *WAG_SIGNATURE {
            return Err(anyhow!("Invalid HexenHaus WAG signature"));
        }

        reader.seek(SeekFrom::Start(6))?;
        let file_count = reader.read_u32()?;
        if file_count == 0 {
            return Err(anyhow!("WAG archive contains no files"));
        }

        let file_length = reader.seek(SeekFrom::End(0))?;
        reader.seek(SeekFrom::Start(0))?;

        let reader = Arc::new(Mutex::new(reader));
        let entry_count = file_count as usize;

        let offset_table_len = entry_count
            .checked_mul(4)
            .ok_or_else(|| anyhow!("Offset table length overflow"))?;
        let offset_table_len_u64 =
            u64::try_from(offset_table_len).map_err(|_| anyhow!("Offset table length overflow"))?;
        let offset_table_end = OFFSET_TABLE_START
            .checked_add(offset_table_len_u64)
            .ok_or_else(|| anyhow!("Offset table exceeds addressable range"))?;
        if offset_table_end > file_length {
            return Err(anyhow!("Offset table extends beyond file length"));
        }

        let mut offsets_raw = vec![0u8; offset_table_len];
        read_decrypted_exact(&reader, OFFSET_TABLE_START, &mut offsets_raw)?;
        if offsets_raw.len() % 4 != 0 {
            return Err(anyhow!("Invalid offset table length"));
        }

        let mut offsets = Vec::with_capacity(entry_count);
        let mut offsets_reader = MemReader::new(offsets_raw);
        while !offsets_reader.is_eof() {
            let offset = offsets_reader.read_u32()?;
            offsets.push(offset as u64);
        }

        let mut entries = Vec::with_capacity(entry_count);
        for offset in offsets {
            if offset
                .checked_add(10)
                .map_or(true, |value| value > file_length)
            {
                continue;
            }
            let mut header_buf = [0u8; 10];
            read_decrypted_exact(&reader, offset, &mut header_buf)?;
            let mut header_reader = MemReaderRef::new(&header_buf);
            let signature = header_reader.read_u32()?;
            if signature != DATA_SIGNATURE {
                continue;
            }
            let section_count = header_reader.read_u32()?;

            let mut entry_name: Option<String> = None;
            let mut data_offset = 0u64;
            let mut data_size = 0u32;
            let mut position = offset
                .checked_add(10)
                .ok_or_else(|| anyhow!("Entry position overflow"))?;

            for _ in 0..section_count {
                if position >= file_length {
                    break;
                }
                let mut section_sig_buf = [0u8; 4];
                read_decrypted_exact(&reader, position, &mut section_sig_buf)?;
                let section_signature = u32::from_le_bytes(section_sig_buf);
                position = position
                    .checked_add(4)
                    .ok_or_else(|| anyhow!("Section position overflow"))?;

                match section_signature {
                    SECTION_IMAGE => {
                        let mut size_buf = [0u8; 4];
                        read_decrypted_exact(&reader, position, &mut size_buf)?;
                        let image_size = u32::from_le_bytes(size_buf);
                        let imgd_start = position
                            .checked_sub(4)
                            .ok_or_else(|| anyhow!("Invalid IMGD start position"))?;
                        data_offset = imgd_start;
                        data_size = image_size
                            .checked_add(0x10)
                            .ok_or_else(|| anyhow!("IMGD section size overflow"))?;
                        position = position
                            .checked_add(4)
                            .ok_or_else(|| anyhow!("Section position overflow"))?;
                        position = position
                            .checked_add(u64::from(image_size))
                            .ok_or_else(|| anyhow!("Section position overflow"))?;
                        position = position
                            .checked_add(2)
                            .ok_or_else(|| anyhow!("Section position overflow"))?;
                    }
                    SECTION_NAME => {
                        let mut name_len_buf = [0u8; 4];
                        read_decrypted_exact(&reader, position, &mut name_len_buf)?;
                        let raw_name_len = u32::from_le_bytes(name_len_buf);
                        position = position
                            .checked_add(4)
                            .ok_or_else(|| anyhow!("Section position overflow"))?;

                        let mut skip_buf = [0u8; 2];
                        read_decrypted_exact(&reader, position, &mut skip_buf)?;
                        position = position
                            .checked_add(2)
                            .ok_or_else(|| anyhow!("Section position overflow"))?;

                        let name_length = raw_name_len.saturating_sub(2) as usize;
                        if name_length > 0 {
                            if position > file_length {
                                break;
                            }
                            let remaining = file_length - position;
                            let name_length_u64 = u64::try_from(name_length)
                                .map_err(|_| anyhow!("Name length overflow"))?;
                            if name_length_u64 > remaining {
                                break;
                            }
                            let mut name_buf = vec![0u8; name_length];
                            read_decrypted_exact(&reader, position, &mut name_buf)?;
                            position = position
                                .checked_add(name_length_u64)
                                .ok_or_else(|| anyhow!("Section position overflow"))?;
                            let name = decode_to_string(archive_encoding, &name_buf, true)?;
                            if !name.is_empty() {
                                entry_name = Some(name);
                            }
                        }

                        let mut skip_tail = [0u8; 2];
                        read_decrypted_exact(&reader, position, &mut skip_tail)?;
                        position = position
                            .checked_add(2)
                            .ok_or_else(|| anyhow!("Section position overflow"))?;
                    }
                    _ => {
                        let mut section_size_buf = [0u8; 4];
                        read_decrypted_exact(&reader, position, &mut section_size_buf)?;
                        let section_size = u32::from_le_bytes(section_size_buf);
                        position = position
                            .checked_add(4)
                            .ok_or_else(|| anyhow!("Section position overflow"))?;
                        position = position
                            .checked_add(u64::from(section_size))
                            .ok_or_else(|| anyhow!("Section position overflow"))?;
                        position = position
                            .checked_add(2)
                            .ok_or_else(|| anyhow!("Section position overflow"))?;
                    }
                }
            }

            if data_size == 0 {
                continue;
            }
            if data_offset
                .checked_add(u64::from(data_size))
                .map_or(true, |end| end > file_length)
            {
                continue;
            }
            if let Some(name) = entry_name {
                if !name.is_empty() {
                    entries.push(HexenHausWagEntry {
                        name,
                        offset: data_offset,
                        size: data_size,
                    });
                }
            }
        }

        if entries.is_empty() {
            return Err(anyhow!("WAG archive contains no readable entries"));
        }

        Ok(HexenHausWagArchive {
            reader,
            file_length,
            entries,
        })
    }

    fn read_decrypted_slice(&self, offset: u64, size: usize) -> Result<Vec<u8>> {
        let requested = u64::try_from(size).map_err(|_| anyhow!("Requested size overflow"))?;
        let length = requested.min(self.file_length.saturating_sub(offset));
        let read_len = usize::try_from(length).map_err(|_| anyhow!("Unable to allocate buffer"))?;
        let mut buf = vec![0u8; read_len];
        if read_len == 0 {
            return Ok(buf);
        }
        read_decrypted_exact(&self.reader, offset, &mut buf)?;
        Ok(buf)
    }
}

impl<T: Read + Seek + std::fmt::Debug + std::any::Any> Script for HexenHausWagArchive<T> {
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
        Ok(Box::new(self.entries.iter().map(|entry| Ok(entry.offset))))
    }

    fn open_file<'a>(&'a self, index: usize) -> Result<Box<dyn ArchiveContent + 'a>> {
        if index >= self.entries.len() {
            return Err(anyhow!(
                "Index out of bounds: {} (total files: {})",
                index,
                self.entries.len()
            ));
        }
        let entry = self.entries[index].clone();
        let header =
            self.read_decrypted_slice(entry.offset, usize::min(entry.size as usize, 16))?;
        let typ = super::detect_script_type(&entry.name, &header);
        Ok(Box::new(WagEntry {
            header: entry,
            reader: self.reader.clone(),
            pos: 0,
            typ,
        }))
    }
}

struct WagEntry<T: Read + Seek> {
    header: HexenHausWagEntry,
    reader: Arc<Mutex<T>>,
    pos: u64,
    typ: Option<ScriptType>,
}

impl<T: Read + Seek> ArchiveContent for WagEntry<T> {
    fn name(&self) -> &str {
        &self.header.name
    }

    fn script_type(&self) -> Option<&ScriptType> {
        self.typ.as_ref()
    }
}

impl<T: Read + Seek> Read for WagEntry<T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut reader = self.reader.lock().map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to lock mutex: {}", e),
            )
        })?;
        reader.seek(SeekFrom::Start(self.header.offset + self.pos))?;
        let total_size = u64::from(self.header.size);
        if self.pos >= total_size {
            return Ok(0);
        }
        let remaining = total_size - self.pos;
        let remaining_usize = match usize::try_from(remaining) {
            Ok(value) => value,
            Err(_) => usize::MAX,
        };
        let to_read = remaining_usize.min(buf.len());
        if to_read == 0 {
            return Ok(0);
        }
        let bytes_read = reader.read(&mut buf[..to_read])?;
        drop(reader);
        for byte in &mut buf[..bytes_read] {
            *byte = byte.rotate_right(4);
        }
        self.pos = self.pos.saturating_add(bytes_read as u64);
        Ok(bytes_read)
    }
}

impl<T: Read + Seek> Seek for WagEntry<T> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset,
            SeekFrom::End(offset) => {
                let size = i64::from(self.header.size);
                let target = size.checked_add(offset).ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Seek from end exceeds file length",
                    )
                })?;
                if target < 0 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Seek from end before start",
                    ));
                }
                target as u64
            }
            SeekFrom::Current(offset) => {
                let current = i64::try_from(self.pos).map_err(|_| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Current position overflow",
                    )
                })?;
                let target = current.checked_add(offset).ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Seek from current caused overflow",
                    )
                })?;
                if target < 0 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Seek from current before start",
                    ));
                }
                target as u64
            }
        };
        self.pos = new_pos;
        Ok(self.pos)
    }

    fn stream_position(&mut self) -> std::io::Result<u64> {
        Ok(self.pos)
    }
}

fn read_decrypted_exact<T: Read + Seek>(
    reader: &Arc<Mutex<T>>,
    offset: u64,
    buf: &mut [u8],
) -> Result<()> {
    if buf.is_empty() {
        return Ok(());
    }
    let mut guard = reader
        .lock()
        .map_err(|e| anyhow!("Failed to lock reader: {}", e))?;
    guard.seek(SeekFrom::Start(offset))?;
    guard.read_exact(buf)?;
    drop(guard);
    for byte in buf.iter_mut() {
        *byte = byte.rotate_right(4);
    }
    Ok(())
}
