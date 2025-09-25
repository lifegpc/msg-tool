//! ExHibit GRP archive extractor.
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use anyhow::{Context, Result};
use std::fmt::Debug;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
/// Builder for ExHibit GRP archives.
pub struct ExHibitGrpArchiveBuilder {}

impl ExHibitGrpArchiveBuilder {
    /// Creates a new builder instance.
    pub const fn new() -> Self {
        Self {}
    }

    fn build_with_reader<T>(
        &self,
        reader: T,
        filename: &str,
        archive_encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<Box<dyn Script>>
    where
        T: Read + Seek + Debug + 'static,
    {
        Ok(Box::new(ExHibitGrpArchive::new(
            reader,
            filename,
            archive_encoding,
            config,
        )?))
    }
}

impl ScriptBuilder for ExHibitGrpArchiveBuilder {
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
        self.build_with_reader(MemReader::new(data), filename, archive_encoding, config)
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
            return Err(anyhow::anyhow!(
                "Reading ExHibit GRP from stdin is not supported; provide a file path."
            ));
        }
        let file = std::fs::File::open(filename)
            .with_context(|| format!("Failed to open '{}'.", filename))?;
        let reader = std::io::BufReader::new(file);
        self.build_with_reader(reader, filename, archive_encoding, config)
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
        self.build_with_reader(reader, filename, archive_encoding, config)
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["grp"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::ExHibitGrp
    }

    fn is_this_format(&self, filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if !matches_grp_name(filename) {
            return None;
        }
        if buf_len >= 4 && buf.starts_with(b"AiFS") {
            return None;
        }
        Some(10)
    }

    fn is_archive(&self) -> bool {
        true
    }
}

#[derive(Clone, Debug)]
struct GrpFileEntry {
    name: String,
    offset: u64,
    size: u64,
}

#[derive(Debug)]
/// ExHibit GRP archive instance.
pub struct ExHibitGrpArchive<T: Read + Seek + Debug> {
    reader: Arc<Mutex<T>>,
    entries: Vec<GrpFileEntry>,
}

impl<T: Read + Seek + Debug> ExHibitGrpArchive<T> {
    fn new(
        mut reader: T,
        filename: &str,
        _archive_encoding: Encoding,
        _config: &ExtraConfig,
    ) -> Result<Self> {
        let mut header = [0u8; 4];
        reader
            .peek_exact_at(0, &mut header)
            .context("Failed to read GRP header.")?;
        if &header == b"AiFS" {
            return Err(anyhow::anyhow!(
                "Input file is a TOC (AiFS) rather than an archive."
            ));
        }

        let path = Path::new(filename);
        let (toc_path, arc_index) = locate_toc_file(path).context("Failed to locate TOC file.")?;

        let archive_size = (&mut reader)
            .stream_length()
            .context("Failed to determine archive size.")?;

        let entries = parse_toc_entries(&toc_path, arc_index, archive_size)
            .with_context(|| format!("Failed to parse TOC '{}'.", toc_path.display()))?;

        Ok(Self {
            reader: Arc::new(Mutex::new(reader)),
            entries,
        })
    }
}

impl<T: Read + Seek + Debug + 'static> Script for ExHibitGrpArchive<T> {
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
            return Err(anyhow::anyhow!(
                "Index out of bounds: {} (max: {}).",
                index,
                self.entries.len()
            ));
        }
        let entry = self.entries[index].clone();
        Ok(Box::new(GrpEntry::new(entry, self.reader.clone())))
    }
}

struct GrpEntry<T: Read + Seek> {
    info: GrpFileEntry,
    reader: Arc<Mutex<T>>,
    pos: u64,
}

impl<T: Read + Seek> GrpEntry<T> {
    fn new(info: GrpFileEntry, reader: Arc<Mutex<T>>) -> Self {
        Self {
            info,
            reader,
            pos: 0,
        }
    }

    fn remaining(&self) -> u64 {
        self.info.size.saturating_sub(self.pos)
    }
}

impl<T: Read + Seek> ArchiveContent for GrpEntry<T> {
    fn name(&self) -> &str {
        &self.info.name
    }
}

impl<T: Read + Seek> Read for GrpEntry<T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if buf.is_empty() || self.pos >= self.info.size {
            return Ok(0);
        }
        let remaining = self.remaining() as usize;
        if remaining == 0 {
            return Ok(0);
        }
        let to_read = buf.len().min(remaining);
        let mut reader = self.reader.lock().map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to lock reader mutex: {}", e),
            )
        })?;
        reader.seek(SeekFrom::Start(self.info.offset + self.pos))?;
        let bytes = reader.read(&mut buf[..to_read])?;
        self.pos = self.pos.checked_add(bytes as u64).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::Other, "Read position overflow.")
        })?;
        Ok(bytes)
    }
}

impl<T: Read + Seek> Seek for GrpEntry<T> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset,
            SeekFrom::End(offset) => {
                let signed = self.info.size as i128 + offset as i128;
                if signed < 0 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Seek before entry start is not allowed.",
                    ));
                }
                signed as u64
            }
            SeekFrom::Current(offset) => {
                let signed = self.pos as i128 + offset as i128;
                if signed < 0 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Seek before entry start is not allowed.",
                    ));
                }
                signed as u64
            }
        };
        if new_pos > self.info.size {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Seek beyond entry size is not allowed.",
            ));
        }
        self.pos = new_pos;
        Ok(self.pos)
    }
}

#[derive(Debug)]
struct NameInfo {
    digits_offset: usize,
    digits_len: usize,
    arc_num: u32,
}

fn matches_grp_name(filename: &str) -> bool {
    Path::new(filename)
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| parse_name_info(name).ok())
        .is_some()
}

fn parse_name_info(name: &str) -> Result<NameInfo> {
    if name.len() < 7 {
        return Err(anyhow::anyhow!(
            "Filename '{}' is too short for GRP pattern.",
            name
        ));
    }
    let prefix = &name[..3];
    if !prefix.eq_ignore_ascii_case("res") {
        return Err(anyhow::anyhow!(
            "Filename '{}' does not start with 'res'.",
            name
        ));
    }
    let suffix = &name[name.len() - 4..];
    if !suffix.eq_ignore_ascii_case(".grp") {
        return Err(anyhow::anyhow!(
            "Filename '{}' does not end with '.grp'.",
            name
        ));
    }
    let digits = &name[3..name.len() - 4];
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return Err(anyhow::anyhow!(
            "Filename '{}' does not contain a numeric sequence.",
            name
        ));
    }
    let arc_num = digits.parse::<u32>().with_context(|| {
        format!(
            "Failed to parse archive number from '{}' (digits '{}').",
            name, digits
        )
    })?;
    Ok(NameInfo {
        digits_offset: 3,
        digits_len: digits.len(),
        arc_num,
    })
}

fn locate_toc_file(path: &Path) -> Result<(PathBuf, u32)> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("Filename contains invalid UTF-8."))?;
    let info = parse_name_info(file_name)?;
    if info.arc_num == 0 {
        return Err(anyhow::anyhow!(
            "Archive '{}' has number 0 and therefore no preceding TOC file.",
            file_name
        ));
    }

    let mut toc_num = info.arc_num as i64 - 1;
    let mut arc_index: u32 = 1;
    while toc_num >= 0 {
        let digits = format!("{:0width$}", toc_num, width = info.digits_len);
        let mut candidate = String::with_capacity(file_name.len());
        candidate.push_str(&file_name[..info.digits_offset]);
        candidate.push_str(&digits);
        candidate.push_str(&file_name[info.digits_offset + info.digits_len..]);
        let candidate_path = path.with_file_name(&candidate);
        if !candidate_path.exists() {
            return Err(anyhow::anyhow!(
                "TOC file '{}' does not exist.",
                candidate_path.display()
            ));
        }
        let mut file = std::fs::File::open(&candidate_path).with_context(|| {
            format!(
                "Failed to open TOC candidate '{}'.",
                candidate_path.display()
            )
        })?;
        let mut header = [0u8; 4];
        file.read_exact(&mut header).with_context(|| {
            format!("Failed to read header from '{}'.", candidate_path.display())
        })?;
        if &header == b"AiFS" {
            return Ok((candidate_path, arc_index));
        }
        toc_num -= 1;
        arc_index = arc_index
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("Archive index overflow while searching TOC."))?;
    }

    Err(anyhow::anyhow!(
        "Unable to locate a TOC (AiFS) file for '{}'.",
        file_name
    ))
}

fn parse_toc_entries(
    toc_path: &Path,
    arc_index: u32,
    archive_size: u64,
) -> Result<Vec<GrpFileEntry>> {
    let file = std::fs::File::open(toc_path)?;
    let mut reader = std::io::BufReader::new(file);
    let toc_len = reader.stream_length()?;
    if toc_len < 0x10 {
        return Err(anyhow::anyhow!("TOC file is too small."));
    }

    reader.seek(SeekFrom::Start(0xC))?;
    let res_count = reader.read_i32()?;
    if res_count <= 0 {
        return Err(anyhow::anyhow!("TOC resource count is invalid."));
    }
    if arc_index as i64 > res_count as i64 {
        return Err(anyhow::anyhow!(
            "Archive index {} is out of range (resource count {}).",
            arc_index,
            res_count
        ));
    }

    let mut index_offset = 0x10u64;
    let mut arc_offset = None;
    for _ in 0..res_count {
        if index_offset + 0x10 > toc_len {
            break;
        }
        reader.seek(SeekFrom::Start(index_offset))?;
        let mut num = reader.read_i32()?;
        if num == 0x0100_0000 {
            index_offset = index_offset
                .checked_add(4)
                .ok_or_else(|| anyhow::anyhow!("Index offset overflow."))?;
            if index_offset + 4 > toc_len {
                break;
            }
            reader.seek(SeekFrom::Start(index_offset))?;
            num = reader.read_i32()?;
        }
        reader.seek(SeekFrom::Start(index_offset + 0xC))?;
        let entry_count = reader.read_u32()?;
        if num == arc_index as i32 {
            arc_offset = Some(index_offset);
            break;
        }
        let step = (entry_count as u64)
            .checked_mul(8)
            .and_then(|v| v.checked_add(0x10))
            .ok_or_else(|| anyhow::anyhow!("Index offset overflow while skipping entries."))?;
        index_offset = index_offset
            .checked_add(step)
            .ok_or_else(|| anyhow::anyhow!("Index offset overflow while iterating."))?;
    }

    let arc_offset =
        arc_offset.ok_or_else(|| anyhow::anyhow!("Archive reference not found in TOC."))?;

    reader.seek(SeekFrom::Start(arc_offset + 4))?;
    let start_index = reader.read_i32()?;
    if start_index < 0 {
        return Err(anyhow::anyhow!("Start index is negative."));
    }
    reader.seek(SeekFrom::Start(arc_offset + 0xC))?;
    let entry_count = reader.read_i32()?;
    if entry_count < 0 {
        return Err(anyhow::anyhow!("Entry count is negative."));
    }
    let entry_count = entry_count as u32;

    let data_offset = arc_offset
        .checked_add(0x10)
        .ok_or_else(|| anyhow::anyhow!("Entry table offset overflow."))?;
    let table_len = (entry_count as u64)
        .checked_mul(8)
        .ok_or_else(|| anyhow::anyhow!("Entry table size overflow."))?;
    if data_offset + table_len > toc_len {
        return Err(anyhow::anyhow!("TOC entry table exceeds file size."));
    }

    let mut entries = Vec::with_capacity(entry_count as usize);
    let mut entry_offset = data_offset;
    for i in 0..entry_count {
        reader.seek(SeekFrom::Start(entry_offset))?;
        let offset = reader.read_u32()? as u64;
        let size = reader.read_u32()? as u64;
        if size != 0 {
            let end = offset
                .checked_add(size)
                .ok_or_else(|| anyhow::anyhow!("Entry size overflow."))?;
            if end > archive_size {
                return Err(anyhow::anyhow!(
                    "Entry {} exceeds archive size (offset {}, size {}).",
                    i,
                    offset,
                    size
                ));
            }
            let index = (start_index as u32)
                .checked_add(i)
                .ok_or_else(|| anyhow::anyhow!("Entry index overflow."))?;
            entries.push(GrpFileEntry {
                name: format!("{:05}.ogg", index),
                offset,
                size,
            });
        }
        entry_offset += 8;
    }

    if entries.is_empty() {
        return Err(anyhow::anyhow!("Archive contains no entries."));
    }

    Ok(entries)
}
