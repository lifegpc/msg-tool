//! Softpal script (.src)
mod disasm;

use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use anyhow::Result;
use disasm::*;
use std::io::Read;

#[derive(Debug)]
/// Softpal script builder
pub struct SoftpalScriptBuilder {}

impl SoftpalScriptBuilder {
    /// Create a new Softpal script builder
    pub fn new() -> Self {
        Self {}
    }
}

impl ScriptBuilder for SoftpalScriptBuilder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Cp932
    }

    fn build_script(
        &self,
        buf: Vec<u8>,
        filename: &str,
        encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
        archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(SoftpalScript::new(
            buf, filename, encoding, config, archive,
        )?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["src"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::Softpal
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 4 && buf.starts_with(b"Sv20") {
            return Some(10);
        }
        None
    }
}

#[derive(Debug)]
/// Softpal SRC Script
pub struct SoftpalScript {
    data: MemReader,
    strs: Vec<PalString>,
    texts: MemReader,
    encoding: Encoding,
    label_offsets: Vec<u32>,
}

impl SoftpalScript {
    /// Create a new Softpal script
    pub fn new(
        buf: Vec<u8>,
        filename: &str,
        encoding: Encoding,
        _config: &ExtraConfig,
        archive: Option<&Box<dyn Script>>,
    ) -> Result<Self> {
        let texts = MemReader::new(Self::load_file(filename, archive, "TEXT.DAT")?);
        let points_data = MemReader::new(Self::load_file(filename, archive, "POINT.DAT")?);
        let label_offsets = Self::load_point_data(points_data)?;
        let strs = Disasm::new(&buf, &label_offsets)?.disassemble::<MemWriter>(None)?;
        Ok(Self {
            data: MemReader::new(buf),
            strs,
            encoding,
            texts,
            label_offsets,
        })
    }

    fn load_file(filename: &str, archive: Option<&Box<dyn Script>>, name: &str) -> Result<Vec<u8>> {
        if let Some(archive) = archive {
            Ok(archive
                .open_file_by_name(name, true)
                .map_err(|e| anyhow::anyhow!("Failed to open file {} in archive: {}", name, e))?
                .data()?)
        } else {
            let mut path = std::path::PathBuf::from(filename);
            path.set_file_name(name);
            std::fs::read(path).map_err(|e| anyhow::anyhow!("Failed to read file {}: {}", name, e))
        }
    }

    fn load_point_data(mut data: MemReader) -> Result<Vec<u32>> {
        let mut magic = [0u8; 16];
        data.read_exact(&mut magic)?;
        if magic != *b"$POINT_LIST_****" {
            return Err(anyhow::anyhow!("Invalid point list magic: {:?}", magic));
        }
        let mut label_offsets = Vec::new();
        while !data.is_eof() {
            label_offsets.push(data.read_u32()? + CODE_OFFSET);
        }
        label_offsets.reverse();
        Ok(label_offsets)
    }
}

impl Script for SoftpalScript {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Json
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn is_output_supported(&self, _: OutputScriptType) -> bool {
        true
    }

    fn extract_messages(&self) -> Result<Vec<Message>> {
        let mut messages = Vec::new();
        let mut name = None;
        for str in &self.strs {
            let addr = self.data.cpeek_u32_at(str.offset as u64)?;
            let text = self.texts.cpeek_cstring_at(addr as u64 + 4)?;
            let text =
                decode_to_string(self.encoding, text.as_bytes(), false)?.replace("<br>", "\n");
            match str.typ {
                StringType::Name => {
                    if text.is_empty() {
                        continue; // Skip empty names
                    }
                    name = Some(text);
                }
                StringType::Message => messages.push(Message {
                    name: name.take(),
                    message: text,
                }),
            }
        }
        Ok(messages)
    }

    fn custom_output_extension<'a>(&'a self) -> &'a str {
        "txt"
    }

    fn custom_export(&self, filename: &std::path::Path, _encoding: Encoding) -> Result<()> {
        let mut file = std::fs::File::create(filename)
            .map_err(|e| anyhow::anyhow!("Failed to create file {}: {}", filename.display(), e))?;
        Disasm::new(&self.data.data, &self.label_offsets)?.disassemble(Some(&mut file))?;
        Ok(())
    }
}
