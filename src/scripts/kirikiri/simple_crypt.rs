use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use anyhow::Result;
use overf::wrapping;
use std::io::Read;

#[derive(Debug)]
pub struct SimpleCryptBuilder {}

impl SimpleCryptBuilder {
    pub fn new() -> Self {
        Self {}
    }
}

impl ScriptBuilder for SimpleCryptBuilder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Utf8
    }

    fn build_script(
        &self,
        buf: Vec<u8>,
        filename: &str,
        _encoding: Encoding,
        _archive_encoding: Encoding,
        _config: &ExtraConfig,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(SimpleCrypt::new(buf, filename)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::KirikiriSimpleCrypt
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 5
            && buf[0] == 0xfe
            && buf[1] == 0xfe
            && (buf[2] == 0 || buf[2] == 1 || buf[2] == 2)
            && buf[3] == 0xff
            && buf[4] == 0xfe
        {
            Some(10)
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct SimpleCrypt {
    /// Crypt mode
    crypt: u8,
    data: MemReader,
    ext: String,
}

impl SimpleCrypt {
    pub fn new(buf: Vec<u8>, filename: &str) -> Result<Self> {
        let mut reader = MemReader::new(buf);
        let mut header = [0u8; 5];
        reader.read_exact(&mut header)?;
        if header[0] != 0xfe
            || header[1] != 0xfe
            || (header[2] != 0 && header[2] != 1 && header[2] != 2)
            || header[3] != 0xff
            || header[4] != 0xfe
        {
            return Err(anyhow::anyhow!("Invalid SimpleCrypt header"));
        }
        Ok(Self {
            crypt: header[2],
            data: reader,
            ext: std::path::Path::new(filename)
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string(),
        })
    }

    pub fn unpack(crypt: u8, data: MemReaderRef) -> Result<Vec<u8>> {
        match crypt {
            0 => Self::unpack_mode0(data),
            1 => Self::unpack_mode1(data),
            2 => Self::unpack_mode2(data),
            _ => Err(anyhow::anyhow!("Unsupported SimpleCrypt mode: {}", crypt)),
        }
    }

    fn unpack_mode0(input: MemReaderRef) -> Result<Vec<u8>> {
        let mut data = Vec::with_capacity(input.data.len() - 3);
        data.push(0xff);
        data.push(0xfe);
        data.extend_from_slice(&input.data[5..]);
        for i in 2..data.len() {
            let ch = data[i] as u16;
            if ch >= 20 {
                data[i] = wrapping! {ch ^ (((ch & 0xfe) << 8) ^ 1)} as u8;
            }
        }
        Ok(data)
    }

    fn unpack_mode1(input: MemReaderRef) -> Result<Vec<u8>> {
        let mut data = Vec::with_capacity(input.data.len() - 3);
        data.push(0xff);
        data.push(0xfe);
        data.extend_from_slice(&input.data[5..]);
        for i in 2..data.len() {
            let mut ch = data[i] as u32;
            ch = wrapping! {((ch & 0xaaaaaaaa) >> 1) | ((ch & 0x55555555) << 1)};
            data[i] = ch as u8;
        }
        Ok(data)
    }

    fn unpack_mode2(mut reader: MemReaderRef) -> Result<Vec<u8>> {
        reader.pos = 5;
        let compressed = reader.read_u64()?;
        debug_assert!(compressed + 5 == reader.data.len() as u64);
        let uncompressed = reader.read_u64()?;
        let mut stream = flate2::Decompress::new(false);
        let mut data = Vec::with_capacity(uncompressed as usize + 2);
        data.push(0xff);
        data.push(0xfe);
        data.resize(uncompressed as usize + 2, 0);
        stream.decompress(
            &reader.data[reader.pos..],
            &mut data[2..],
            flate2::FlushDecompress::Finish,
        )?;
        Ok(data)
    }
}

impl Script for SimpleCrypt {
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
        &self.ext
    }

    fn custom_export(&self, filename: &std::path::Path, _encoding: Encoding) -> Result<()> {
        let data = Self::unpack(self.crypt, self.data.to_ref())?;
        let mut writer = crate::utils::files::write_file(filename)?;
        writer.write_all(&data)?;
        Ok(())
    }
}
