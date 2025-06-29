use crate::ext::io::*;
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

    fn create_archive(
        &self,
        _filename: &str,
        _files: &[&str],
        _encoding: Encoding,
        _config: &ExtraConfig,
    ) -> Result<Box<dyn Archive>> {
        Err(anyhow::anyhow!(
            "This script type does not support creating an archive."
        ))
    }

    fn can_create_file(&self) -> bool {
        false
    }

    fn create_file<'a>(
        &'a self,
        _filename: &'a str,
        _writer: Box<dyn WriteSeek + 'a>,
        _encoding: Encoding,
        _file_encoding: Encoding,
    ) -> Result<()> {
        Err(anyhow::anyhow!(
            "This script type does not support creating directly."
        ))
    }

    fn create_file_filename(
        &self,
        filename: &str,
        output_filename: &str,
        encoding: Encoding,
        file_encoding: Encoding,
    ) -> Result<()> {
        let f = std::fs::File::create(output_filename)?;
        let f = std::io::BufWriter::new(f);
        self.create_file(filename, Box::new(f), encoding, file_encoding)
    }

    #[cfg(feature = "image")]
    fn is_image(&self) -> bool {
        false
    }

    #[cfg(feature = "image")]
    fn can_create_image_file(&self) -> bool {
        false
    }

    #[cfg(feature = "image")]
    fn create_image_file<'a>(
        &'a self,
        _data: ImageData,
        _writer: Box<dyn WriteSeek + 'a>,
        _options: &ExtraConfig,
    ) -> Result<()> {
        Err(anyhow::anyhow!(
            "This script type does not support creating an image file."
        ))
    }

    #[cfg(feature = "image")]
    fn create_image_file_filename(
        &self,
        data: ImageData,
        filename: &str,
        options: &ExtraConfig,
    ) -> Result<()> {
        let f = std::fs::File::create(filename)?;
        let f = std::io::BufWriter::new(f);
        self.create_image_file(data, Box::new(f), options)
    }
}

pub trait ArchiveContent: Read {
    fn name(&self) -> &str;
    fn is_script(&self) -> bool {
        self.script_type().is_some()
    }
    fn script_type(&self) -> Option<&ScriptType> {
        None
    }
    fn data(&mut self) -> Result<Vec<u8>> {
        let mut data = Vec::new();
        self.read_to_end(&mut data)?;
        Ok(data)
    }
    fn to_data<'a>(&'a mut self) -> Result<Box<dyn ReadSeek + 'a>> {
        Ok(Box::new(MemReader::new(self.data()?)))
    }
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

    fn import_messages<'a>(
        &'a self,
        _messages: Vec<Message>,
        _file: Box<dyn WriteSeek + 'a>,
        _encoding: Encoding,
        _replacement: Option<&'a ReplacementTable>,
    ) -> Result<()> {
        if !self.is_archive() {
            return Err(anyhow::anyhow!(
                "This script type does not support importing messages."
            ));
        }
        Ok(())
    }

    fn import_messages_filename(
        &self,
        _messages: Vec<Message>,
        _filename: &str,
        _encoding: Encoding,
        _replacement: Option<&ReplacementTable>,
    ) -> Result<()> {
        let f = std::fs::File::create(_filename)?;
        let f = std::io::BufWriter::new(f);
        self.import_messages(_messages, Box::new(f), _encoding, _replacement)
    }

    fn custom_export(&self, _filename: &std::path::Path, _encoding: Encoding) -> Result<()> {
        Err(anyhow::anyhow!(
            "This script type does not support custom export."
        ))
    }

    fn custom_import<'a>(
        &'a self,
        _custom_filename: &'a str,
        _file: Box<dyn WriteSeek + 'a>,
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
        let f = std::io::BufWriter::new(f);
        self.custom_import(custom_filename, Box::new(f), encoding, output_encoding)
    }

    fn is_archive(&self) -> bool {
        false
    }

    fn iter_archive<'a>(&'a mut self) -> Result<Box<dyn Iterator<Item = Result<String>> + 'a>> {
        Err(anyhow::anyhow!(
            "This script type does not support iterating over archive contents."
        ))
    }

    fn iter_archive_mut<'a>(
        &'a mut self,
    ) -> Result<Box<dyn Iterator<Item = Result<Box<dyn ArchiveContent>>> + 'a>> {
        Ok(Box::new(std::iter::empty()))
    }

    #[cfg(feature = "image")]
    fn is_image(&self) -> bool {
        false
    }

    #[cfg(feature = "image")]
    fn export_image(&self) -> Result<ImageData> {
        Err(anyhow::anyhow!(
            "This script type does not support to export image."
        ))
    }

    #[cfg(feature = "image")]
    fn import_image<'a>(&'a self, _data: ImageData, _file: Box<dyn WriteSeek + 'a>) -> Result<()> {
        Err(anyhow::anyhow!(
            "This script type does not support to import image."
        ))
    }

    #[cfg(feature = "image")]
    fn import_image_filename(&self, data: ImageData, filename: &str) -> Result<()> {
        let f = std::fs::File::create(filename)?;
        let f = std::io::BufWriter::new(f);
        self.import_image(data, Box::new(f))
    }

    #[cfg(feature = "image")]
    fn is_multi_image(&self) -> bool {
        false
    }

    #[cfg(feature = "image")]
    fn export_multi_image<'a>(
        &'a self,
    ) -> Result<Box<dyn Iterator<Item = Result<ImageDataWithName>> + 'a>> {
        Err(anyhow::anyhow!(
            "This script type does not support to export multi image."
        ))
    }

    #[cfg(feature = "image")]
    fn import_multi_image<'a>(
        &'a self,
        _data: Vec<ImageDataWithName>,
        _file: Box<dyn WriteSeek + 'a>,
    ) -> Result<()> {
        Err(anyhow::anyhow!(
            "This script type does not support to import multi image."
        ))
    }

    #[cfg(feature = "image")]
    fn import_multi_image_filename(
        &self,
        data: Vec<ImageDataWithName>,
        filename: &str,
    ) -> Result<()> {
        let f = std::fs::File::create(filename)?;
        let f = std::io::BufWriter::new(f);
        self.import_multi_image(data, Box::new(f))
    }
}

pub trait Archive {
    fn new_file<'a>(&'a mut self, name: &str) -> Result<Box<dyn WriteSeek + 'a>>;
    fn write_header(&mut self) -> Result<()>;
}
