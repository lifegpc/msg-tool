use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use anyhow::Result;

fn try_parse(buf: &[u8]) -> Result<u8> {
    let mut reader = MemReaderRef::new(buf);
    let width = reader.read_u16()?;
    let height = reader.read_u16()?;
    let bpp = reader.read_u16()?;
    let _flag = reader.read_u16()?;
    let padding = reader.read_u64()?;
    if padding != 0 {
        return Err(anyhow::anyhow!("Invalid padding: {}", padding));
    }
    if width == 0 || height == 0 {
        return Err(anyhow::anyhow!("Invalid dimensions: {}x{}", width, height));
    }
    if width > 4096 || height > 4096 {
        return Err(anyhow::anyhow!(
            "Dimensions too large: {}x{}",
            width,
            height
        ));
    }
    if bpp != 8 && bpp != 24 && bpp != 32 {
        return Err(anyhow::anyhow!("Unsupported BPP: {}", bpp));
    }
    Ok(1)
}

#[derive(Debug)]
pub struct BgiImageBuilder {}

impl BgiImageBuilder {
    pub const fn new() -> Self {
        BgiImageBuilder {}
    }
}

impl ScriptBuilder for BgiImageBuilder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Cp932
    }

    fn build_script(
        &self,
        data: Vec<u8>,
        _filename: &str,
        _encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(BgiImage::new(data, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::BGIImg
    }

    fn is_image(&self) -> bool {
        true
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 0x10 {
            return try_parse(&buf[0..0x10]).ok();
        }
        None
    }
}

#[derive(Debug)]
pub struct BgiImage {
    data: MemReader,
    width: u32,
    height: u32,
    color_type: ImageColorType,
    is_scrambled: bool,
}

impl BgiImage {
    pub fn new(buf: Vec<u8>, _config: &ExtraConfig) -> Result<Self> {
        let mut reader = MemReader::new(buf);
        let width = reader.read_u16()? as u32;
        let height = reader.read_u16()? as u32;
        let bpp = reader.read_u16()?;
        let color_type = match bpp {
            8 => ImageColorType::Grayscale,
            24 => ImageColorType::Bgr,
            32 => ImageColorType::Bgra,
            _ => return Err(anyhow::anyhow!("Unsupported BPP: {}", bpp)),
        };
        let flag = reader.read_u16()?;
        let padding = reader.read_u64()?;
        if padding != 0 {
            return Err(anyhow::anyhow!("Invalid padding: {}", padding));
        }
        let is_scrambled = flag != 0;

        Ok(BgiImage {
            data: reader,
            width,
            height,
            color_type,
            is_scrambled,
        })
    }
}

impl Script for BgiImage {
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
        let stride = self.width as usize * ((self.color_type.bbp(8) as usize + 7) / 8);
        let buf_size = stride * self.height as usize;
        if self.is_scrambled {
            return Err(anyhow::anyhow!("Scrambled images are not supported"));
        }
        let mut data = Vec::with_capacity(buf_size);
        data.resize(buf_size, 0);
        self.data.cpeek_extract_at(0x10, &mut data)?;
        Ok(ImageData {
            width: self.width,
            height: self.height,
            color_type: self.color_type,
            depth: 8,
            data,
        })
    }
}
