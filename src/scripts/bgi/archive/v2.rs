use super::bse::*;
use super::dsc::*;
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::encode_string;
use crate::utils::struct_pack::*;
use anyhow::Result;
use msg_tool_macro::*;
use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub struct BgiArchiveBuilder {}

impl BgiArchiveBuilder {
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
pub struct BgiArchive<T: Read + Seek + std::fmt::Debug> {
    reader: Arc<Mutex<T>>,
    file_count: u32,
    entries: Vec<BgiFileHeader>,
    #[cfg(feature = "bgi-img")]
    is_sysgrp_arc: bool,
}

impl<T: Read + Seek + std::fmt::Debug> BgiArchive<T> {
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
            file_count,
            entries,
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

    fn iter_archive<'a>(&'a mut self) -> Result<Box<dyn Iterator<Item = Result<String>> + 'a>> {
        Ok(Box::new(
            self.entries.iter().map(|e| Ok(e.filename.clone())),
        ))
    }

    fn iter_archive_mut<'a>(
        &'a mut self,
    ) -> Result<Box<dyn Iterator<Item = Result<Box<dyn ArchiveContent>>> + 'a>> {
        Ok(Box::new(BgiArchiveIter {
            entries: self.entries.iter(),
            reader: self.reader.clone(),
            base_offset: 16 + (self.file_count as u64 * 0x80),
            #[cfg(feature = "bgi-img")]
            is_sysgrp_arc: self.is_sysgrp_arc,
        }))
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

struct BgiArchiveIter<'a, T: Iterator<Item = &'a BgiFileHeader>, R: Read + Seek> {
    entries: T,
    reader: Arc<Mutex<R>>,
    base_offset: u64,
    #[cfg(feature = "bgi-img")]
    is_sysgrp_arc: bool,
}

impl<'a, T: Iterator<Item = &'a BgiFileHeader>, R: Read + Seek + 'static> Iterator
    for BgiArchiveIter<'a, T, R>
{
    type Item = Result<Box<dyn ArchiveContent>>;

    fn next(&mut self) -> Option<Self::Item> {
        let entry = match self.entries.next() {
            Some(e) => e,
            None => return None,
        };
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
                return Some(Err(anyhow::anyhow!(
                    "Failed to read entry '{}': {}",
                    entry.header.filename,
                    e
                )));
            }
        }
        entry.pos = 0;
        if buf.starts_with(b"DSC FORMAT 1.00") {
            let data = match entry.data() {
                Ok(data) => data,
                Err(e) => {
                    return Some(Err(anyhow::anyhow!(
                        "Failed to read DSC data for '{}': {}",
                        entry.header.filename,
                        e
                    )));
                }
            };
            entry.pos = 0;
            let dsc = match DscDecoder::new(&data) {
                Ok(dsc) => dsc,
                Err(e) => {
                    return Some(Err(anyhow::anyhow!(
                        "Failed to create DSC decoder for '{}': {}",
                        entry.header.filename,
                        e
                    )));
                }
            };
            let decoded = match dsc.unpack() {
                Ok(decoded) => decoded,
                Err(e) => {
                    return Some(Err(anyhow::anyhow!(
                        "Failed to unpack DSC data for '{}': {}",
                        entry.header.filename,
                        e
                    )));
                }
            };
            let reader = MemReader::new(decoded);
            if reader.data.starts_with(b"BSE 1.") {
                match BseReader::new(reader, detect_script_type, &entry.header.filename) {
                    Ok(bse_reader) => {
                        return Some(Ok(Box::new(bse_reader)));
                    }
                    Err(e) => {
                        return Some(Err(anyhow::anyhow!(
                            "Failed to create BSE reader for '{}': {}",
                            entry.header.filename,
                            e
                        )));
                    }
                };
            }
            return Some(Ok(Box::new(MemEntry {
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
            })));
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
                                return Some(Err(anyhow::anyhow!(
                                    "Failed to read BSE data for '{}': {}",
                                    &filename,
                                    e
                                )));
                            }
                        };
                        let dsc = match DscDecoder::new(&data) {
                            Ok(dsc) => dsc,
                            Err(e) => {
                                return Some(Err(anyhow::anyhow!(
                                    "Failed to create DSC decoder for '{}': {}",
                                    &filename,
                                    e
                                )));
                            }
                        };
                        let decoded = match dsc.unpack() {
                            Ok(decoded) => decoded,
                            Err(e) => {
                                return Some(Err(anyhow::anyhow!(
                                    "Failed to unpack DSC data for '{}': {}",
                                    &filename,
                                    e
                                )));
                            }
                        };
                        let reader = MemReader::new(decoded);
                        return Some(Ok(Box::new(MemEntry {
                            name: filename,
                            data: reader,
                            detect,
                        })));
                    }
                    return Some(Ok(Box::new(bse_reader)));
                }
                Err(e) => {
                    return Some(Err(anyhow::anyhow!(
                        "Failed to create BSE reader for '{}': {}",
                        &filename,
                        e
                    )));
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
        Some(Ok(Box::new(entry)))
    }
}
