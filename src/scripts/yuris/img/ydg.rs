//! YU-RIS compressed image file (.ydg)
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::img::*;
use crate::utils::struct_pack::*;
use anyhow::Result;
use msg_tool_macro::*;
use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::{Arc, Mutex};

#[derive(StructPack, StructUnpack, Debug, Clone)]
struct YDGHeader {
    /// YDG
    magic: [u8; 4],
    /// YU-RIS
    yuris_magic: [u8; 8],
    /// Seems always 0x64
    _unk: u32,
    /// Header length
    header_size: u32,
    /// YDG file size
    file_size: u32,
    _unk1: u64,
    /// Image width
    width: u16,
    /// Image height
    height: u16,
    #[pack_vec_len(self.header_size - 0x24)]
    #[unpack_vec_len({
        if header_size < 0x24 {
            anyhow::bail!("Header size at least need 0x24 bytes.");
        }
        header_size - 0x24
    })]
    other: Vec<u8>,
    #[pvec(u32)]
    slices: Vec<Slice>,
}

#[derive(StructPack, StructUnpack, Debug, Clone)]
struct Slice {
    /// Slice start offset
    offset: u32,
    /// Slice size
    size: u32,
    x: u16,
    height: u16,
    _unk: u32,
}

#[derive(Debug)]
/// YU-RIS compressed image file (.ydg) builder
pub struct YDGImageBuilder {}

impl YDGImageBuilder {
    pub fn new() -> Self {
        Self {}
    }
}

impl ScriptBuilder for YDGImageBuilder {
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
    ) -> Result<Box<dyn Script + Send + Sync>> {
        Ok(Box::new(YDGImage::new(MemReader::new(buf), config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ydg"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::YurisYDG
    }

    fn is_image(&self) -> bool {
        true
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 12 && buf.starts_with(b"YDG\0YU-RIS\0\0") {
            return Some(50);
        }
        None
    }

    fn can_create_image_file(&self) -> bool {
        true
    }

    fn create_image_file<'a>(
        &'a self,
        mut data: ImageData,
        _filename: &str,
        mut writer: Box<dyn WriteSeek + 'a>,
        _options: &ExtraConfig,
    ) -> Result<()> {
        let mut header = YDGHeader {
            magic: *b"YDG\0",
            yuris_magic: *b"YU-RIS\0\0",
            _unk: 0x64,
            header_size: 0x30,
            file_size: 0,
            _unk1: 0,
            width: data.width as u16,
            height: data.height as u16,
            other: vec![0; 0xC],
            slices: vec![Slice {
                offset: 0,
                size: 0,
                x: 0,
                height: data.height as u16,
                _unk: 0,
            }],
        };
        header.pack(&mut writer, false, Encoding::Utf8, &None)?;
        header.slices[0].offset = writer.stream_position()? as u32;
        match data.color_type {
            ImageColorType::Bgr => {
                convert_bgr_to_rgb(&mut data)?;
            }
            ImageColorType::Bgra => {
                convert_bgra_to_rgba(&mut data)?;
            }
            ImageColorType::Rgb | ImageColorType::Rgba => {}
            ImageColorType::Grayscale => {
                convert_grayscale_to_rgb(&mut data)?;
            }
        };
        let encoder = qoi::Encoder::new(&data.data, data.width, data.height)?;
        encoder.encode_to_stream(&mut writer)?;
        let file_size = writer.stream_position()? as u32;
        header.slices[0].size = file_size - header.slices[0].offset;
        header.file_size = file_size;
        writer.seek(SeekFrom::Start(0))?;
        header.pack(&mut writer, false, Encoding::Utf8, &None)?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct YDGImage<T> {
    inner: Arc<Mutex<T>>,
    header: YDGHeader,
}

impl<T: Read + Seek> YDGImage<T> {
    pub fn new(mut data: T, _config: &ExtraConfig) -> Result<Self> {
        let header = YDGHeader::unpack(&mut data, false, Encoding::Utf8, &None)?;
        if &header.magic != b"YDG\0" {
            anyhow::bail!("Unknown YDG magic: {:?}", header.magic);
        }
        if &header.yuris_magic != b"YU-RIS\0\0" {
            anyhow::bail!("Unknown YU-RIS magic: {:?}", header.yuris_magic);
        }
        Ok(Self {
            inner: Arc::new(Mutex::new(data)),
            header,
        })
    }

    fn load_slice(&self, slice: &Slice) -> Result<ImageData> {
        let mut data = StreamRegion::with_size(
            MutexWrapper::new(self.inner.clone(), slice.offset as u64),
            slice.size as u64,
        )?;
        let mut buf = [0; 12];
        let readed = data.peek(&mut buf)?;
        if readed == 12 && buf.starts_with(b"RIFF") && buf.ends_with(b"WEBP") {
            load_webp(data)
        } else {
            load_qoi(data)
        }
    }
}

impl<T: Read + Seek + std::fmt::Debug> Script for YDGImage<T> {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Custom
    }

    fn is_output_supported(&self, output: OutputScriptType) -> bool {
        matches!(output, OutputScriptType::Custom)
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn is_image(&self) -> bool {
        true
    }

    fn export_image(&self) -> Result<ImageData> {
        let slice = self
            .header
            .slices
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("YDG image has no valid tiles."))?;
        let mut y = 0;
        let mut base = self.load_slice(slice)?;
        convert_to_rgba(&mut base)?;
        let mut base = draw_on_canvas(
            base,
            self.header.width as u32,
            self.header.height as u32,
            slice.x as u32,
            y,
        )?;
        y += slice.height as u32;
        for slice in &self.header.slices[1..] {
            let mut diff = self.load_slice(slice)?;
            convert_to_rgba(&mut diff)?;
            draw_on_image(&mut base, &diff, slice.x as u32, y)?;
            y += slice.height as u32;
        }
        Ok(base)
    }

    fn import_image<'a>(
        &'a self,
        mut data: ImageData,
        _filename: &str,
        mut file: Box<dyn WriteSeek + 'a>,
    ) -> Result<()> {
        if data.depth != 8 {
            anyhow::bail!("Unsupported depth: {}", data.depth);
        }
        let mut header = self.header.clone();
        header.slices.clear();
        header.slices.push(Slice {
            offset: 0,
            size: 0,
            x: 0,
            height: data.height as u16,
            _unk: 0,
        });
        header.pack(&mut file, false, Encoding::Utf8, &None)?;
        header.slices[0].offset = file.stream_position()? as u32;
        if header.width != data.width as u16 || header.height != data.height as u16 {
            eprintln!(
                "WARNING: image size dismatched, expected {}x{}, actually {}x{}.",
                header.width, header.height, data.width, data.height
            );
            crate::COUNTER.inc_warning();
            header.width = data.width as u16;
            header.height = data.height as u16;
        }
        match data.color_type {
            ImageColorType::Bgr => {
                convert_bgr_to_rgb(&mut data)?;
            }
            ImageColorType::Bgra => {
                convert_bgra_to_rgba(&mut data)?;
            }
            ImageColorType::Rgb | ImageColorType::Rgba => {}
            ImageColorType::Grayscale => {
                convert_grayscale_to_rgb(&mut data)?;
            }
        };
        let encoder = qoi::Encoder::new(&data.data, data.width, data.height)?;
        encoder.encode_to_stream(&mut file)?;
        let file_size = file.stream_position()? as u32;
        header.slices[0].size = file_size - header.slices[0].offset;
        header.file_size = file_size;
        file.seek(SeekFrom::Start(0))?;
        header.pack(&mut file, false, Encoding::Utf8, &None)?;
        Ok(())
    }
}
