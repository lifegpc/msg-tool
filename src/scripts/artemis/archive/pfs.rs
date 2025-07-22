use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::struct_pack::*;
use anyhow::Result;
use msg_tool_macro::*;
use sha1::Digest;
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
        _filename: &str,
        _encoding: Encoding,
        archive_encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(ArtemisArc::new(
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
    ) -> Result<Box<dyn Script>> {
        let f = std::fs::File::open(filename)?;
        let f = std::io::BufReader::new(f);
        Ok(Box::new(ArtemisArc::new(f, archive_encoding, config)?))
    }

    fn build_script_from_reader(
        &self,
        reader: Box<dyn ReadSeek>,
        _filename: &str,
        _encoding: Encoding,
        archive_encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(ArtemisArc::new(reader, archive_encoding, config)?))
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
}

impl<T: Read + Seek + std::fmt::Debug> ArtemisArc<T> {
    pub fn new(mut reader: T, archive_encoding: Encoding, _config: &ExtraConfig) -> Result<Self> {
        let mut magic = [0; 2];
        reader.read_exact(&mut magic)?;
        if &magic != b"pf" {
            return Err(anyhow::anyhow!(
                "Invalid Artemis archive magic: {:?}",
                magic
            ));
        }
        let version = reader.read_u8()?;
        if version != b'2' && version != b'6' && version != b'8' {
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
        Ok(ArtemisArc {
            reader: Arc::new(Mutex::new(reader)),
            entries,
            xor_key,
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

    fn iter_archive<'a>(&'a mut self) -> Result<Box<dyn Iterator<Item = Result<String>> + 'a>> {
        Ok(Box::new(
            self.entries.iter().map(|header| Ok(header.name.clone())),
        ))
    }

    fn iter_archive_mut<'a>(
        &'a mut self,
    ) -> Result<Box<dyn Iterator<Item = Result<Box<dyn ArchiveContent>>> + 'a>> {
        Ok(Box::new(ArtemisArcIter {
            entries: self.entries.iter(),
            reader: self.reader.clone(),
            xor_key: self.xor_key.clone(),
        }))
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

struct ArtemisArcIter<'a, T: Iterator<Item = &'a PfsEntryHeader>, R: Read + Seek + 'static> {
    entries: T,
    reader: Arc<Mutex<R>>,
    xor_key: Option<[u8; 20]>,
}

impl<'a, T: Iterator<Item = &'a PfsEntryHeader>, R: Read + Seek + 'static> Iterator
    for ArtemisArcIter<'a, T, R>
{
    type Item = Result<Box<dyn ArchiveContent>>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(header) = self.entries.next() {
            let entry = Entry {
                header: header.clone(),
                reader: self.reader.clone(),
                pos: 0,
                script_type: None,
                xor_key: self.xor_key.clone(),
            };
            Some(Ok(Box::new(entry)))
        } else {
            None
        }
    }
}
