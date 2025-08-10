//! Buriko General Interpreter/Ethornell Audio File (Ogg/Vorbis)
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use anyhow::Result;
use std::io::{Read, Seek, SeekFrom, Write};

#[derive(Debug)]
/// Builder for BGI Audio scripts.
pub struct BgiAudioBuilder {}

impl BgiAudioBuilder {
    /// Creates a new instance of `BgiAudioBuilder`.
    pub fn new() -> Self {
        Self {}
    }
}

impl ScriptBuilder for BgiAudioBuilder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Utf8
    }

    fn build_script(
        &self,
        buf: Vec<u8>,
        _filename: &str,
        _encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(BgiAudio::new(MemReader::new(buf), config)?))
    }

    fn build_script_from_file(
        &self,
        filename: &str,
        _encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        let file = std::fs::File::open(filename)?;
        let f = std::io::BufReader::new(file);
        Ok(Box::new(BgiAudio::new(f, config)?))
    }

    fn build_script_from_reader(
        &self,
        reader: Box<dyn ReadSeek>,
        _filename: &str,
        _encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(BgiAudio::new(reader, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::BGIAudio
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 8 && buf[4..].starts_with(b"bw  ") {
            Some(10)
        } else {
            None
        }
    }
}

#[derive(Debug)]
/// BGI Audio script.
pub struct BgiAudio {
    data: MemReader,
}

impl BgiAudio {
    /// Creates a new instance of `BgiAudio` from a reader.
    ///
    /// * `reader` - The reader to read the audio data from.
    /// * `config` - Extra configuration options.
    pub fn new<R: Read + Seek>(mut reader: R, _config: &ExtraConfig) -> Result<Self> {
        let offset = reader.read_u32()?;
        let len = reader.stream_length()?;
        if (offset as u64) > len {
            return Err(anyhow::anyhow!("Invalid offset in BGI audio file"));
        }
        let mut magic = [0; 4];
        reader.read_exact(&mut magic)?;
        if magic != *b"bw  " {
            return Err(anyhow::anyhow!(
                "Invalid magic in BGI audio file: {:?}",
                magic
            ));
        }
        reader.seek(SeekFrom::Start(offset as u64))?;
        let mut data = Vec::new();
        reader.read_to_end(&mut data)?;
        Ok(Self {
            data: MemReader::new(data),
        })
    }
}

impl Script for BgiAudio {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Custom
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn is_output_supported(&self, output: OutputScriptType) -> bool {
        matches!(output, OutputScriptType::Custom)
    }

    fn custom_output_extension<'a>(&'a self) -> &'a str {
        "ogg"
    }

    fn custom_export(&self, filename: &std::path::Path, _encoding: Encoding) -> Result<()> {
        let mut writer = std::fs::File::create(filename)?;
        writer.write_all(&self.data.data)?;
        writer.flush()?;
        Ok(())
    }
}
