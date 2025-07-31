use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::img::*;
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
        _archive: Option<&Box<dyn Script>>,
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

    fn can_create_image_file(&self) -> bool {
        true
    }

    fn create_image_file<'a>(
        &'a self,
        data: ImageData,
        writer: Box<dyn WriteSeek + 'a>,
        options: &ExtraConfig,
    ) -> Result<()> {
        create_image(data, writer, options.bgi_img_scramble.unwrap_or(false))
    }
}

#[derive(Debug)]
pub struct BgiImage {
    data: MemReader,
    width: u32,
    height: u32,
    color_type: ImageColorType,
    is_scrambled: bool,
    opt_is_scrambled: Option<bool>,
}

fn create_image<'a>(
    mut data: ImageData,
    mut writer: Box<dyn WriteSeek + 'a>,
    scrambled: bool,
) -> Result<()> {
    writer.write_u16(data.width as u16)?;
    writer.write_u16(data.height as u16)?;
    if data.depth != 8 {
        return Err(anyhow::anyhow!("Unsupported image depth: {}", data.depth));
    }
    match data.color_type {
        ImageColorType::Bgr => {}
        ImageColorType::Bgra => {}
        ImageColorType::Grayscale => {}
        ImageColorType::Rgb => {
            convert_rgb_to_bgr(&mut data)?;
        }
        ImageColorType::Rgba => {
            convert_rgba_to_bgra(&mut data)?;
        }
    }
    let bpp = data.color_type.bpp(8);
    writer.write_u16(bpp)?;
    let flag = if scrambled { 1 } else { 0 };
    writer.write_u16(flag)?;
    writer.write_u64(0)?; // Padding
    let stride = data.width as usize * ((data.color_type.bpp(8) as usize + 7) / 8);
    let buf_size = stride * data.height as usize;
    if scrambled {
        let bpp = data.color_type.bpp(1) as usize;
        for i in 0..bpp {
            let mut dst = i;
            let mut incr = 0u8;
            let mut h = data.height;
            while h > 0 {
                for _ in 0..data.width {
                    writer.write_u8(data.data[dst].wrapping_sub(incr))?;
                    incr = data.data[dst];
                    dst += bpp;
                }
                h -= 1;
                if h == 0 {
                    break;
                }
                dst += stride;
                let mut pos = dst;
                for _ in 0..data.width {
                    pos -= bpp;
                    writer.write_u8(data.data[pos].wrapping_sub(incr))?;
                    incr = data.data[pos];
                }
                h -= 1;
            }
        }
    } else {
        // PNG sometimes return more padding data than expected
        // We will write only the required size
        writer.write_all(&data.data[..buf_size])?;
    }
    Ok(())
}

impl BgiImage {
    pub fn new(buf: Vec<u8>, config: &ExtraConfig) -> Result<Self> {
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
            opt_is_scrambled: config.bgi_img_scramble,
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
        let stride = self.width as usize * ((self.color_type.bpp(8) as usize + 7) / 8);
        let buf_size = stride * self.height as usize;
        let mut data = Vec::with_capacity(buf_size);
        data.resize(buf_size, 0);
        if self.is_scrambled {
            let mut reader = self.data.to_ref();
            reader.pos = 0x10;
            let bpp = self.color_type.bpp(1) as usize;
            for i in 0..bpp {
                let mut dst = i;
                let mut incr = 0u8;
                let mut h = self.height;
                while h > 0 {
                    for _ in 0..self.width {
                        incr = incr.wrapping_add(reader.read_u8()?);
                        data[dst] = incr;
                        dst += bpp;
                    }
                    h -= 1;
                    if h == 0 {
                        break;
                    }
                    dst += stride;
                    let mut pos = dst;
                    for _ in 0..self.width {
                        pos -= bpp;
                        incr = incr.wrapping_add(reader.read_u8()?);
                        data[pos] = incr;
                    }
                    h -= 1;
                }
            }
        } else {
            self.data.cpeek_exact_at(0x10, &mut data)?;
        }
        Ok(ImageData {
            width: self.width,
            height: self.height,
            color_type: self.color_type,
            depth: 8,
            data,
        })
    }

    fn import_image<'a>(&'a self, data: ImageData, file: Box<dyn WriteSeek + 'a>) -> Result<()> {
        create_image(
            data,
            file,
            self.opt_is_scrambled.unwrap_or(self.is_scrambled),
        )
    }
}
