//! Basic traits and types for script.
use crate::ext::io::*;
use crate::types::*;
use anyhow::Result;
use std::collections::HashMap;
use std::io::{Read, Seek, Write};

/// A trait for reading and seeking in a stream.
pub trait ReadSeek: Read + Seek + std::fmt::Debug {}

/// A trait for writing and seeking in a stream.
pub trait WriteSeek: Write + Seek {}

/// A trait for types that can be displayed in debug format and are also support downcasting.
pub trait AnyDebug: std::fmt::Debug + std::any::Any {}

/// A trait for reading in a stream with debug format.
pub trait ReadDebug: Read + std::fmt::Debug {}

impl<T: Read + Seek + std::fmt::Debug> ReadSeek for T {}

impl<T: Read + std::fmt::Debug> ReadDebug for T {}

impl<T: Write + Seek> WriteSeek for T {}

impl<T: std::fmt::Debug + std::any::Any> AnyDebug for T {}

/// A trait for script builders.
pub trait ScriptBuilder: std::fmt::Debug {
    /// Returns the default encoding for the script.
    fn default_encoding(&self) -> Encoding;

    /// Returns the default encoding for the archive.
    /// If None, the default encoding should be used.
    fn default_archive_encoding(&self) -> Option<Encoding> {
        None
    }

    /// Returns the default encoding for script files when patching scripts.
    fn default_patched_encoding(&self) -> Encoding {
        self.default_encoding()
    }

    /// Builds a script from the given buffer.
    ///
    /// * `buf` - The buffer containing the script data.
    /// * `filename` - The name of the file from which the script was read.
    /// * `encoding` - The encoding of the script data.
    /// * `archive_encoding` - The encoding of the archive, if applicable.
    /// * `config` - Additional configuration options.
    /// * `archive` - An optional archive to which the script belongs.
    fn build_script(
        &self,
        buf: Vec<u8>,
        filename: &str,
        encoding: Encoding,
        archive_encoding: Encoding,
        config: &ExtraConfig,
        archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>>;

    /// Builds a script from a file.
    ///
    /// * `filename` - The name of the file to read.
    /// * `encoding` - The encoding of the script data.
    /// * `archive_encoding` - The encoding of the archive, if applicable.
    /// * `config` - Additional configuration options.
    /// * `archive` - An optional archive to which the script belongs.
    fn build_script_from_file(
        &self,
        filename: &str,
        encoding: Encoding,
        archive_encoding: Encoding,
        config: &ExtraConfig,
        archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        let data = crate::utils::files::read_file(filename)?;
        self.build_script(data, filename, encoding, archive_encoding, config, archive)
    }

    /// Builds a script from a reader.
    ///
    /// * `reader` - A reader with seek capabilities.
    /// * `filename` - The name of the file from which the script was read.
    /// * `encoding` - The encoding of the script data.
    /// * `archive_encoding` - The encoding of the archive, if applicable.
    /// * `config` - Additional configuration options.
    /// * `archive` - An optional archive to which the script belongs.
    fn build_script_from_reader(
        &self,
        mut reader: Box<dyn ReadSeek>,
        filename: &str,
        encoding: Encoding,
        archive_encoding: Encoding,
        config: &ExtraConfig,
        archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        let mut data = Vec::new();
        reader
            .read_to_end(&mut data)
            .map_err(|e| anyhow::anyhow!("Failed to read from reader: {}", e))?;
        self.build_script(data, filename, encoding, archive_encoding, config, archive)
    }

    /// Returns the extensions supported by this script builder.
    fn extensions(&self) -> &'static [&'static str];

    /// Checks if the given filename and buffer match this script format.
    /// * `filename` - The name of the file to check.
    /// * `buf` - The buffer containing the script data.
    /// * `buf_len` - The length of the buffer.
    ///
    /// Returns a score (0-255) indicating how well the format matches.
    /// A higher score means a better match.
    fn is_this_format(&self, _filename: &str, _buf: &[u8], _buf_len: usize) -> Option<u8> {
        None
    }

    /// Returns the script type associated with this builder.
    fn script_type(&self) -> &'static ScriptType;

    /// Returns true if this script is an archive.
    fn is_archive(&self) -> bool {
        false
    }

    /// Creates an archive with the given files.
    ///
    /// * `filename` - The path of the archive file to create.
    /// * `files` - A list of files to include in the archive.
    /// * `encoding` - The encoding to use for the archive.
    /// * `config` - Additional configuration options.
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

    /// Returns true if this script type can create from a file directly.
    fn can_create_file(&self) -> bool {
        false
    }

    /// Creates a new script file.
    ///
    /// * `filename` - The path to the input file.
    /// * `writer` - A writer with seek capabilities to write the script data.
    /// * `encoding` - The encoding to use for the script data.
    /// * `file_encoding` - The encoding of the file.
    /// * `config` - Additional configuration options.
    fn create_file<'a>(
        &'a self,
        _filename: &'a str,
        _writer: Box<dyn WriteSeek + 'a>,
        _encoding: Encoding,
        _file_encoding: Encoding,
        _config: &ExtraConfig,
    ) -> Result<()> {
        Err(anyhow::anyhow!(
            "This script type does not support creating directly."
        ))
    }

    /// Creates a new script file with the given filename.
    ///
    /// * `filename` - The path to the input file.
    /// * `output_filename` - The path to the output file.
    /// * `encoding` - The encoding to use for the script data.
    /// * `file_encoding` - The encoding of the file.
    /// * `config` - Additional configuration options.
    fn create_file_filename(
        &self,
        filename: &str,
        output_filename: &str,
        encoding: Encoding,
        file_encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<()> {
        let f = std::fs::File::create(output_filename)?;
        let f = std::io::BufWriter::new(f);
        self.create_file(filename, Box::new(f), encoding, file_encoding, config)
    }

    /// Returns true if this script is an image.
    #[cfg(feature = "image")]
    fn is_image(&self) -> bool {
        false
    }

    /// Returns true if this script type can create from an image file directly.
    #[cfg(feature = "image")]
    fn can_create_image_file(&self) -> bool {
        false
    }

    /// Creates an image file from the given data.
    ///
    /// * `data` - The image data to write.
    /// * `writer` - A writer with seek capabilities to write the image data.
    /// * `options` - Additional configuration options.
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

    /// Creates an image file from the given data to the specified filename.
    ///
    /// * `data` - The image data to write.
    /// * `filename` - The path to the output file.
    /// * `options` - Additional configuration options.
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

/// A trait to present the file in an archive.
pub trait ArchiveContent: Read {
    /// Returns the name of the file in the archive.
    fn name(&self) -> &str;
    /// Returns true if the file is a script.
    fn is_script(&self) -> bool {
        self.script_type().is_some()
    }
    /// Returns the script type if the file is a script.
    fn script_type(&self) -> Option<&ScriptType> {
        None
    }
    /// Returns the data of the file as a vector of bytes.
    fn data(&mut self) -> Result<Vec<u8>> {
        let mut data = Vec::new();
        self.read_to_end(&mut data)?;
        Ok(data)
    }
    /// Returns a reader that supports reading and seeking.
    fn to_data<'a>(&'a mut self) -> Result<Box<dyn ReadSeek + 'a>> {
        Ok(Box::new(MemReader::new(self.data()?)))
    }
}

/// A trait for script types.
pub trait Script: std::fmt::Debug + std::any::Any {
    /// Returns the default output script type for this script.
    fn default_output_script_type(&self) -> OutputScriptType;

    /// Checks if the given output script type is supported by this script.
    fn is_output_supported(&self, output: OutputScriptType) -> bool {
        !matches!(output, OutputScriptType::Custom)
    }

    /// Returns the output extension for this script when exporting with custom output.
    fn custom_output_extension<'a>(&'a self) -> &'a str {
        ""
    }

    /// Returns the default format options for this script.
    fn default_format_type(&self) -> FormatOptions;

    /// Returns true if this script can contains multiple message files.
    fn multiple_message_files(&self) -> bool {
        false
    }

    /// Extract messages from this script.
    fn extract_messages(&self) -> Result<Vec<Message>> {
        if !self.is_archive() {
            return Err(anyhow::anyhow!(
                "This script type does not support extracting messages."
            ));
        }
        Ok(vec![])
    }

    /// Extract multiple messages from this script.
    fn extract_multiple_messages(&self) -> Result<HashMap<String, Vec<Message>>> {
        if !self.multiple_message_files() {
            return Err(anyhow::anyhow!(
                "This script type does not support extracting multiple message files."
            ));
        }
        Ok(HashMap::new())
    }

    /// Import messages into this script.
    ///
    /// * `messages` - The messages to import.
    /// * `file` - A writer with seek capabilities to write the patched scripts.
    /// * `filename` - The path of the file to write the patched scripts.
    /// * `encoding` - The encoding to use for the patched scripts.
    /// * `replacement` - An optional replacement table for message replacements.
    fn import_messages<'a>(
        &'a self,
        _messages: Vec<Message>,
        _file: Box<dyn WriteSeek + 'a>,
        _filename: &str,
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

    /// Import multiple messages into this script.
    ///
    /// * `messages` - A map of filenames to messages to import.
    /// * `file` - A writer with seek capabilities to write the patched scripts.
    /// * `filename` - The path of the file to write the patched scripts.
    /// * `encoding` - The encoding to use for the patched scripts.
    /// * `replacement` - An optional replacement table for message replacements.s
    fn import_multiple_messages<'a>(
        &'a self,
        _messages: HashMap<String, Vec<Message>>,
        _file: Box<dyn WriteSeek + 'a>,
        _filename: &str,
        _encoding: Encoding,
        _replacement: Option<&'a ReplacementTable>,
    ) -> Result<()> {
        if !self.multiple_message_files() {
            return Err(anyhow::anyhow!(
                "This script type does not support importing multiple message files."
            ));
        }
        Ok(())
    }

    /// Import messages into this script.
    ///
    /// * `messages` - The messages to import.
    /// * `filename` - The path of the file to write the patched scripts.
    /// * `encoding` - The encoding to use for the patched scripts.
    /// * `replacement` - An optional replacement table for message replacements.
    fn import_messages_filename(
        &self,
        messages: Vec<Message>,
        filename: &str,
        encoding: Encoding,
        replacement: Option<&ReplacementTable>,
    ) -> Result<()> {
        let f = std::fs::File::create(filename)?;
        let f = std::io::BufWriter::new(f);
        self.import_messages(messages, Box::new(f), filename, encoding, replacement)
    }

    /// Import multiple messages into this script.
    ///
    /// * `messages` - A map of filenames to messages to import.
    /// * `filename` - The path of the file to write the patched scripts.
    /// * `encoding` - The encoding to use for the patched scripts.
    /// * `replacement` - An optional replacement table for message replacements.
    fn import_multiple_messages_filename(
        &self,
        messages: HashMap<String, Vec<Message>>,
        filename: &str,
        encoding: Encoding,
        replacement: Option<&ReplacementTable>,
    ) -> Result<()> {
        let f = std::fs::File::create(filename)?;
        let f = std::io::BufWriter::new(f);
        self.import_multiple_messages(messages, Box::new(f), filename, encoding, replacement)
    }

    /// Exports data from this script.
    ///
    /// * `filename` - The path of the file to write the exported data.
    /// * `encoding` - The encoding to use for the exported data.
    fn custom_export(&self, _filename: &std::path::Path, _encoding: Encoding) -> Result<()> {
        Err(anyhow::anyhow!(
            "This script type does not support custom export."
        ))
    }

    /// Imports data into this script.
    ///
    /// * `custom_filename` - The path of the file to import.
    /// * `file` - A writer with seek capabilities to write the patched scripts.
    /// * `encoding` - The encoding of the patched scripts.
    /// * `output_encoding` - The encoding to use for the imported file.
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

    /// Imports data into this script.
    ///
    /// * `custom_filename` - The path of the file to import.
    /// * `filename` - The path of the file to write the patched scripts.
    /// * `encoding` - The encoding of the patched scripts.
    /// * `output_encoding` - The encoding to use for the imported file.
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

    /// Returns true if this script is an archive.
    fn is_archive(&self) -> bool {
        false
    }

    /// Returns an iterator over archive filenames.
    fn iter_archive_filename<'a>(
        &'a self,
    ) -> Result<Box<dyn Iterator<Item = Result<String>> + 'a>> {
        Err(anyhow::anyhow!(
            "This script type does not support iterating over archive filenames."
        ))
    }

    /// Returns an iterator over archive offsets.
    fn iter_archive_offset<'a>(&'a self) -> Result<Box<dyn Iterator<Item = Result<u64>> + 'a>> {
        Err(anyhow::anyhow!(
            "This script type does not support iterating over archive offsets."
        ))
    }

    /// Opens a file in the archive by its index.
    fn open_file<'a>(&'a self, _index: usize) -> Result<Box<dyn ArchiveContent + 'a>> {
        Err(anyhow::anyhow!(
            "This script type does not support opening files."
        ))
    }

    /// Opens a file in the archive by its name.
    ///
    /// * `name` - The name of the file to open.
    /// * `ignore_case` - If true, the name comparison will be case-insensitive.
    fn open_file_by_name<'a>(
        &'a self,
        name: &str,
        ignore_case: bool,
    ) -> Result<Box<dyn ArchiveContent + 'a>> {
        for (i, fname) in self.iter_archive_filename()?.enumerate() {
            if let Ok(fname) = fname {
                if fname == name || (ignore_case && fname.eq_ignore_ascii_case(name)) {
                    return self.open_file(i);
                }
            }
        }
        Err(anyhow::anyhow!(
            "File with name '{}' not found in archive.",
            name
        ))
    }

    /// Opens a file in the archive by its offset.
    fn open_file_by_offset<'a>(&'a self, offset: u64) -> Result<Box<dyn ArchiveContent + 'a>> {
        for (i, off) in self.iter_archive_offset()?.enumerate() {
            if let Ok(off) = off {
                if off == offset {
                    return self.open_file(i);
                }
            }
        }
        Err(anyhow::anyhow!(
            "File with offset '{}' not found in archive.",
            offset
        ))
    }

    /// Returns output extension for archive output folder.
    fn archive_output_ext<'a>(&'a self) -> Option<&'a str> {
        None
    }

    #[cfg(feature = "image")]
    /// Returns true if this script type is an image.
    fn is_image(&self) -> bool {
        false
    }

    #[cfg(feature = "image")]
    /// Exports the image data from this script.
    fn export_image(&self) -> Result<ImageData> {
        Err(anyhow::anyhow!(
            "This script type does not support to export image."
        ))
    }

    #[cfg(feature = "image")]
    /// Imports an image into this script.
    ///
    /// * `data` - The image data to import.
    /// * `file` - A writer with seek capabilities to write the patched scripts.
    fn import_image<'a>(&'a self, _data: ImageData, _file: Box<dyn WriteSeek + 'a>) -> Result<()> {
        Err(anyhow::anyhow!(
            "This script type does not support to import image."
        ))
    }

    #[cfg(feature = "image")]
    /// Imports an image into this script.
    ///
    /// * `data` - The image data to import.
    /// * `filename` - The path of the file to write the patched scripts.
    fn import_image_filename(&self, data: ImageData, filename: &str) -> Result<()> {
        let f = std::fs::File::create(filename)?;
        let f = std::io::BufWriter::new(f);
        self.import_image(data, Box::new(f))
    }

    #[cfg(feature = "image")]
    /// Returns true if this script is contains multiple images.
    fn is_multi_image(&self) -> bool {
        false
    }

    #[cfg(feature = "image")]
    /// Exports multiple images from this script.
    fn export_multi_image<'a>(
        &'a self,
    ) -> Result<Box<dyn Iterator<Item = Result<ImageDataWithName>> + 'a>> {
        Err(anyhow::anyhow!(
            "This script type does not support to export multi image."
        ))
    }

    #[cfg(feature = "image")]
    /// Imports multiple images into this script.
    ///
    /// * `data` - A vector of image data with names to import.
    /// * `file` - A writer with seek capabilities to write the patched scripts.
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
    /// Imports multiple images into this script.
    ///
    /// * `data` - A vector of image data with names to import.
    /// * `filename` - The path of the file to write the patched scripts.
    fn import_multi_image_filename(
        &self,
        data: Vec<ImageDataWithName>,
        filename: &str,
    ) -> Result<()> {
        let f = std::fs::File::create(filename)?;
        let f = std::io::BufWriter::new(f);
        self.import_multi_image(data, Box::new(f))
    }

    /// Returns the extra information for this script.
    fn extra_info<'a>(&'a self) -> Option<Box<dyn AnyDebug + 'a>> {
        None
    }
}

/// A trait for creating archives.
pub trait Archive {
    /// Creates a new file in the archive.
    ///
    /// size is optional, if provided, size must be exactly the size of the file to be created.
    fn new_file<'a>(&'a mut self, name: &str, size: Option<u64>)
    -> Result<Box<dyn WriteSeek + 'a>>;
    /// Creates a new file in the archive that does not require seeking.
    ///
    /// size is optional, if provided, size must be exactly the size of the file to be created.
    fn new_file_non_seek<'a>(
        &'a mut self,
        name: &str,
        size: Option<u64>,
    ) -> Result<Box<dyn Write + 'a>> {
        self.new_file(name, size)
            .map(|f| Box::new(f) as Box<dyn Write + 'a>)
    }
    /// Writes the header of the archive. (Must be called after writing all files.)
    fn write_header(&mut self) -> Result<()>;
}
