use crate::types::*;
use anyhow::Result;
use std::io::{Read, Seek, Write};

pub trait ReadSeek: Read + Seek + std::fmt::Debug {}

pub trait WriteSeek: Write + Seek {}

impl<T: Read + Seek + std::fmt::Debug> ReadSeek for T {}

impl<T: Write + Seek> WriteSeek for T {}

pub trait ScriptBuilder: std::fmt::Debug {
    fn default_encoding(&self) -> Encoding;

    fn default_archive_encoding(&self) -> Option<Encoding> {
        None
    }

    fn default_patched_encoding(&self) -> Encoding {
        self.default_encoding()
    }

    fn build_script(
        &self,
        buf: Vec<u8>,
        filename: &str,
        encoding: Encoding,
        archive_encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<Box<dyn Script>>;

    fn build_script_from_file(
        &self,
        filename: &str,
        encoding: Encoding,
        archive_encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<Box<dyn Script>> {
        let data = crate::utils::files::read_file(filename)?;
        self.build_script(data, filename, encoding, archive_encoding, config)
    }

    fn build_script_from_reader(
        &self,
        mut reader: Box<dyn ReadSeek>,
        filename: &str,
        encoding: Encoding,
        archive_encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<Box<dyn Script>> {
        let mut data = Vec::new();
        reader
            .read_to_end(&mut data)
            .map_err(|e| anyhow::anyhow!("Failed to read from reader: {}", e))?;
        self.build_script(data, filename, encoding, archive_encoding, config)
    }

    fn extensions(&self) -> &'static [&'static str];

    fn is_this_format(&self, _filename: &str, _buf: &[u8], _buf_len: usize) -> Option<u8> {
        None
    }

    fn script_type(&self) -> &'static ScriptType;

    fn is_archive(&self) -> bool {
        false
    }
}

pub trait ArchiveContent {
    fn name(&self) -> &str;
    fn data(&self) -> &[u8];
    fn is_script(&self) -> bool;
}

pub trait Script: std::fmt::Debug {
    fn default_output_script_type(&self) -> OutputScriptType;

    fn is_output_supported(&self, output: OutputScriptType) -> bool {
        !matches!(output, OutputScriptType::Custom)
    }

    fn custom_output_extension(&self) -> &'static str {
        ""
    }

    fn default_format_type(&self) -> FormatOptions;

    fn extract_messages(&self) -> Result<Vec<Message>> {
        if !self.is_archive() {
            return Err(anyhow::anyhow!(
                "This script type does not support extracting messages."
            ));
        }
        Ok(vec![])
    }

    fn import_messages(
        &self,
        _messages: Vec<Message>,
        _filename: &str,
        _encoding: Encoding,
        _replacement: Option<&ReplacementTable>,
    ) -> Result<()> {
        if !self.is_archive() {
            return Err(anyhow::anyhow!(
                "This script type does not support importing messages."
            ));
        }
        Ok(())
    }

    fn custom_export(&self, _filename: &std::path::Path, _encoding: Encoding) -> Result<()> {
        Err(anyhow::anyhow!(
            "This script type does not support custom export."
        ))
    }

    fn custom_import(
        &self,
        _custom_filename: &str,
        _file: Box<dyn WriteSeek>,
        _encoding: Encoding,
        _output_encoding: Encoding,
    ) -> Result<()> {
        Err(anyhow::anyhow!(
            "This script type does not support custom import."
        ))
    }

    fn custom_import_filename(
        &self,
        custom_filename: &str,
        filename: &str,
        encoding: Encoding,
        output_encoding: Encoding,
    ) -> Result<()> {
        let f = std::fs::File::create(filename)?;
        self.custom_import(custom_filename, Box::new(f), encoding, output_encoding)
    }

    fn is_archive(&self) -> bool {
        false
    }

    fn iter_archive<'a>(
        &'a mut self,
    ) -> Result<Box<dyn Iterator<Item = Result<Box<dyn ArchiveContent>>> + 'a>> {
        Ok(Box::new(std::iter::empty()))
    }
}
