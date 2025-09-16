use super::base::*;
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::struct_pack::*;
use anyhow::Result;
use std::io::{Read, Seek};

#[derive(Debug)]
pub struct PgdGeBuilder {}

impl PgdGeBuilder {
    pub fn new() -> Self {
        Self {}
    }
}

impl ScriptBuilder for PgdGeBuilder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Cp932
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
        Ok(Box::new(PgdGe::new(MemReader::new(buf), config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["pgd"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::SoftpalPgdGe
    }

    fn is_image(&self) -> bool {
        true
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 4 && buf.starts_with(b"GE \0") {
            return Some(20);
        }
        None
    }

    fn can_create_image_file(&self) -> bool {
        true
    }

    fn create_image_file<'a>(
        &'a self,
        data: ImageData,
        mut writer: Box<dyn WriteSeek + 'a>,
        _options: &ExtraConfig,
    ) -> Result<()> {
        let header = PgdGeHeader {
            offset_x: 0,
            offset_y: 0,
            width: data.width,
            height: data.height,
            canvas_width: data.width,
            canvas_height: data.height,
            mode: 3,
        };
        writer.write_all(b"GE \0")?;
        header.pack(&mut writer, false, Encoding::Utf8)?;
        PgdWriter::new(data).with_method(3).pack_ge(&mut writer)?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct PgdGe {
    header: PgdGeHeader,
    data: ImageData,
}

impl PgdGe {
    pub fn new<T: Read + Seek>(mut input: T, _config: &ExtraConfig) -> Result<Self> {
        let mut magic = [0u8; 4];
        input.read_exact(&mut magic)?;
        if &magic != b"GE \0" {
            return Err(anyhow::anyhow!("Not a valid PGD GE image"));
        }
        let header = PgdGeHeader::unpack(&mut input, false, Encoding::Utf8)?;
        let reader = PgdReader::with_ge_header(input, &header)?;
        let data = reader.unpack_ge()?;
        Ok(Self { header, data })
    }
}

impl Script for PgdGe {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Json
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn is_image(&self) -> bool {
        true
    }

    fn export_image(&self) -> Result<ImageData> {
        Ok(self.data.clone())
    }

    fn import_image<'a>(
        &'a self,
        data: ImageData,
        mut file: Box<dyn WriteSeek + 'a>,
    ) -> Result<()> {
        let mut header = self.header.clone();
        if data.height != self.data.height {
            return Err(anyhow::anyhow!(
                "Image height does not match: expected {}, got {}",
                self.data.height,
                data.height
            ));
        }
        if data.width != self.data.width {
            return Err(anyhow::anyhow!(
                "Image width does not match: expected {}, got {}",
                self.data.width,
                data.width
            ));
        }
        header.mode = 3;
        file.write_all(b"GE \0")?;
        header.pack(&mut file, false, Encoding::Utf8)?;
        PgdWriter::new(data).with_method(3).pack_ge(&mut file)?;
        Ok(())
    }
}
