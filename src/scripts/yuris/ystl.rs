//! Yu-Ris YSTL(file list) file (.ybn)
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use crate::utils::struct_pack::*;
use anyhow::Result;
use chrono::TimeZone;
use chrono::Timelike;
use chrono::{DateTime, Local, Utc};
use msg_tool_macro::*;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::io::{Read, Seek, Write};
use std::ops::{Deref, DerefMut};

#[derive(Debug, Serialize, Deserialize)]
struct YSTLData {
    version: u32,
    entries: Vec<YSTLEntry>,
}

impl StructUnpack for YSTLData {
    fn unpack<R: Read + Seek>(
        reader: &mut R,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<Self> {
        let version = u32::unpack(reader, big, encoding, info)?;
        let ninfo = Box::new(version) as Box<dyn Any>;
        let count = u32::unpack(reader, big, encoding, info)?;
        let entries = reader.read_struct_vec(count as usize, big, encoding, &Some(ninfo))?;
        Ok(Self { version, entries })
    }
}

impl StructPack for YSTLData {
    fn pack<W: Write>(
        &self,
        writer: &mut W,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<()> {
        self.version.pack(writer, big, encoding, info)?;
        let ninfo = Box::new(self.version) as Box<dyn Any>;
        let count = self.entries.len() as u32;
        count.pack(writer, big, encoding, info)?;
        let info = &Some(ninfo);
        for (i, entry) in self.entries.iter().enumerate() {
            if entry.seq == i as u32 {
                entry.pack(writer, big, encoding, info)?;
            } else {
                let mut entry = entry.clone();
                entry.seq = i as u32;
                entry.pack(writer, big, encoding, info)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(transparent)]
struct FileSystemTime(DateTime<Local>);

impl Deref for FileSystemTime {
    type Target = DateTime<Local>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for FileSystemTime {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl std::fmt::Display for FileSystemTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl StructUnpack for FileSystemTime {
    fn unpack<R: Read + Seek>(
        reader: &mut R,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn std::any::Any>>,
    ) -> Result<Self> {
        let high = u32::unpack(reader, big, encoding, info)?;
        let low = u32::unpack(reader, big, encoding, info)?;
        let time = low as u64 | ((high as u64) << 32);
        const FILETIME_OFFSET: u64 = 116_444_736_000_000_000;
        if time < FILETIME_OFFSET {
            anyhow::bail!("Time to small.");
        }
        let intervals_since_1970 = time - FILETIME_OFFSET;
        let seconds = (intervals_since_1970 / 10_000_000) as i64;
        let nsecs = ((intervals_since_1970 % 10_000_000) * 100) as u32;
        let time = Utc
            .timestamp_opt(seconds, nsecs)
            .single()
            .ok_or_else(|| anyhow::anyhow!("Time is not existed or ambiguous."))?;
        let time = time.with_timezone(&Local);
        Ok(Self(time))
    }
}

impl StructPack for FileSystemTime {
    fn pack<W: Write>(
        &self,
        writer: &mut W,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn std::any::Any>>,
    ) -> Result<()> {
        let time = self.0.with_timezone(&Utc);
        let tseconds = time.timestamp();
        let nsecs = time.nanosecond() / 100;
        const FILETIME_OFFSET: u64 = 116_444_736_000_000_000;
        let seconds = (tseconds as u64)
            .checked_mul(10_000_000)
            .ok_or_else(|| anyhow::anyhow!("Too big time"))?
            .checked_add(nsecs as u64)
            .ok_or_else(|| anyhow::anyhow!("Too big time"))?
            .checked_add(FILETIME_OFFSET)
            .ok_or_else(|| anyhow::anyhow!("Too big time"))?;
        let high = (seconds >> 32) as u32;
        let low = seconds as u32;
        high.pack(writer, big, encoding, info)?;
        low.pack(writer, big, encoding, info)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, StructUnpack, StructPack)]
struct YSTLEntry {
    seq: u32,
    #[pstring(u32)]
    path: String,
    modification_time: FileSystemTime,
    num_variables: u32,
    num_labels: u32,
    // TODO: version may need more check
    #[skip_pack_if(get_info_as_version(__info)? < 300)]
    #[skip_unpack_if(get_info_as_version(__info)? < 300)]
    num_texts: u32,
}

fn get_info_as_version(info: &Option<Box<dyn Any>>) -> Result<u32> {
    Ok(*info
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("info not found"))?
        .downcast_ref()
        .ok_or_else(|| anyhow::anyhow!("not YSTBHeader"))?)
}

#[derive(Debug)]
pub struct YSTLBuilder {}

impl YSTLBuilder {
    /// Creates a new instance of `YSTLBuilder`
    pub const fn new() -> Self {
        YSTLBuilder {}
    }
}

impl ScriptBuilder for YSTLBuilder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Cp932
    }

    fn build_script(
        &self,
        buf: Vec<u8>,
        _filename: &str,
        encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script + Send + Sync>> {
        Ok(Box::new(YSTL::new(MemReader::new(buf), encoding, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ybn"]
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 4 && buf.starts_with(b"YSTL") {
            return Some(20);
        }
        None
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::YurisYSTL
    }

    fn can_create_file(&self) -> bool {
        true
    }

    fn create_file<'a>(
        &'a self,
        filename: &'a str,
        writer: Box<dyn WriteSeek + 'a>,
        encoding: Encoding,
        file_encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<()> {
        create_file(
            filename,
            writer,
            encoding,
            file_encoding,
            config.custom_yaml,
        )
    }
}

#[derive(Debug)]
pub struct YSTL {
    data: YSTLData,
    custom_yaml: bool,
}

impl YSTL {
    pub fn new<T: Read + Seek>(
        mut reader: T,
        encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<Self> {
        let mut sig = [0; 4];
        reader.read_exact(&mut sig)?;
        if &sig != b"YSTL" {
            anyhow::bail!("Unsupported YSTL file.");
        }
        let data = YSTLData::unpack(&mut reader, false, encoding, &None)?;
        Ok(Self {
            data,
            custom_yaml: config.custom_yaml,
        })
    }
}

impl Script for YSTL {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Custom
    }

    fn is_output_supported(&self, output: OutputScriptType) -> bool {
        matches!(output, OutputScriptType::Custom)
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn custom_output_extension(&self) -> &'static str {
        if self.custom_yaml { "yaml" } else { "json" }
    }

    fn custom_export(&self, filename: &std::path::Path, encoding: Encoding) -> Result<()> {
        let s = if self.custom_yaml {
            serde_yaml_ng::to_string(&self.data)
                .map_err(|e| anyhow::anyhow!("Failed to serialize to YAML: {}", e))?
        } else {
            serde_json::to_string_pretty(&self.data)
                .map_err(|e| anyhow::anyhow!("Failed to serialize to JSON: {}", e))?
        };
        let mut writer = crate::utils::files::write_file(filename)?;
        let s = encode_string(encoding, &s, false)?;
        writer.write_all(&s)?;
        writer.flush()?;
        Ok(())
    }

    fn custom_import<'a>(
        &'a self,
        custom_filename: &'a str,
        file: Box<dyn WriteSeek + 'a>,
        encoding: Encoding,
        output_encoding: Encoding,
    ) -> Result<()> {
        create_file(
            custom_filename,
            file,
            encoding,
            output_encoding,
            self.custom_yaml,
        )
    }
}

fn create_file<'a>(
    custom_filename: &'a str,
    mut writer: Box<dyn WriteSeek + 'a>,
    encoding: Encoding,
    output_encoding: Encoding,
    yaml: bool,
) -> Result<()> {
    let input = crate::utils::files::read_file(custom_filename)?;
    let s = decode_to_string(output_encoding, &input, true)?;
    let mut data: YSTLData = if yaml {
        serde_yaml_ng::from_str(&s).map_err(|e| anyhow::anyhow!("Failed to parse YAML: {}", e))?
    } else {
        serde_json::from_str(&s).map_err(|e| anyhow::anyhow!("Failed to parse JSON: {}", e))?
    };
    writer.write_all(b"YSTL")?;
    writer.write_u32(data.version)?;
    writer.write_u32(data.entries.len() as u32)?;
    let info = Box::new(data.version) as Box<dyn Any>;
    let info = &Some(info);
    for (i, entry) in data.entries.iter_mut().enumerate() {
        entry.seq = i as u32;
        entry.pack(&mut writer, false, encoding, info)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_unpack_file_system_time() {
        let mut reader = MemReaderRef::new(b" \x0c\xd8\x01\x00k`\xdd");
        let ts = FileSystemTime::unpack(&mut reader, false, Encoding::Cp932, &None).unwrap();
        println!("{}", ts);
        let utc = ts.to_utc();
        let t = serde_json::to_string(&utc).unwrap();
        assert_eq!(t, "\"2022-01-18T04:07:10Z\"");
    }
    #[test]
    fn test_pack_file_system_time() {
        let ts: FileSystemTime = serde_json::from_str("\"2022-01-18T13:07:10+09:00\"").unwrap();
        let mut buf = [0; 8];
        let mut writer = MemWriterRef::new(&mut buf);
        ts.pack(&mut writer, false, Encoding::Cp932, &None).unwrap();
        assert_eq!(&buf, b" \x0c\xd8\x01\x00k`\xdd");
    }
}
