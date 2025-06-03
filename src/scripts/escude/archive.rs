use super::crypto::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::decode_to_string;
use crate::ext::io::*;
use anyhow::Result;
use std::io::{Read, Seek, SeekFrom};

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

    fn build_script(
        &self,
        data: Vec<u8>,
        encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(EscudeBinArchive::new(
            MemReader::new(data),
            encoding,
            config,
        )?))
    }

    fn build_script_from_file(
        &self,
        filename: &str,
        encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<Box<dyn Script>> {
        if filename == "-" {
            let data = crate::utils::files::read_file(filename)?;
            self.build_script(data, encoding, config)
        } else {
            let f = std::fs::File::open(filename)?;
            let reader = std::io::BufReader::new(f);
            Ok(Box::new(EscudeBinArchive::new(reader, encoding, config)?))
        }
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
}

#[derive(Debug)]
struct BinEntry {
    name_offset: u32,
    data_offset: u32,
    length: u32,
}

struct Entry {
    name: String,
    data: Vec<u8>,
}

impl ArchiveContent for Entry {
    fn name(&self) -> &str {
        &self.name
    }

    fn data(&self) -> &[u8] {
        &self.data
    }

    fn is_script(&self) -> bool {
        self.data.starts_with(b"ESCR1_00")
    }
}

#[derive(Debug)]
pub struct EscudeBinArchive<T: Read + Seek + std::fmt::Debug> {
    reader: T,
    file_count: u32,
    name_tbl_len: u32,
    entries: Vec<BinEntry>,
    encoding: Encoding,
}

impl<T: Read + Seek + std::fmt::Debug> EscudeBinArchive<T> {
    pub fn new(mut reader: T, encoding: Encoding, _config: &ExtraConfig) -> Result<Self> {
        let mut header = [0u8; 8];
        reader.read_exact(&mut header)?;
        if &header != b"ESC-ARC2" {
            return Err(anyhow::anyhow!("Invalid Escude binary script header"));
        }
        reader.seek(SeekFrom::Start(0xC))?;
        let mut crypto_reader = CryptoReader::new(&mut reader)?;
        let file_count = crypto_reader.read_u32()?;
        let name_tbl_len = crypto_reader.read_u32()?;
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
            name_tbl_len,
            entries,
            encoding,
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

    fn iter_archive<'a>(
        &'a mut self,
    ) -> Result<Box<dyn Iterator<Item = Result<Box<dyn ArchiveContent>>> + 'a>> {
        let encoding = self.encoding;
        Ok(Box::new(EscudeBinArchiveIterator {
            entries: self.entries.iter(),
            reader: &mut self.reader,
            encoding,
            file_count: self.file_count,
        }))
    }
}

struct EscudeBinArchiveIterator<'a, T: Iterator<Item = &'a BinEntry>, R: Read + Seek> {
    entries: T,
    reader: &'a mut R,
    encoding: Encoding,
    file_count: u32,
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
        let name = match decode_to_string(self.encoding, name.as_bytes()) {
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
                Err(e) => return Some(Err(anyhow::anyhow!("Failed to unpack LZW data: {}", e))),
            };
        }
        Some(Ok(Box::new(Entry { name, data })))
    }
}
