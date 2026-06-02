//! Yu-Ris YSVR(Variables) file (.ybn)
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use crate::utils::serde_base64bytes::*;
use crate::utils::struct_pack::*;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::io::{Read, Seek, Write};

#[derive(Debug, Serialize, Deserialize)]
struct YSVRData {
    version: u32,
    variables: Vec<Variable>,
}

impl StructUnpack for YSVRData {
    fn unpack<R: Read + Seek>(
        reader: &mut R,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<Self> {
        let version = u32::unpack(reader, big, encoding, info)?;
        let ninfo = Box::new(version) as Box<dyn Any>;
        let count = u16::unpack(reader, big, encoding, info)?;
        let variables = reader.read_struct_vec(count as usize, big, encoding, &Some(ninfo))?;
        Ok(Self { version, variables })
    }
}

impl StructPack for YSVRData {
    fn pack<W: Write>(
        &self,
        writer: &mut W,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<()> {
        self.version.pack(writer, big, encoding, info)?;
        let ninfo = Box::new(self.version) as Box<dyn Any>;
        let count = self.variables.len() as u16;
        count.pack(writer, big, encoding, info)?;
        let info = &Some(ninfo);
        for variable in &self.variables {
            variable.pack(writer, big, encoding, info)?;
        }
        Ok(())
    }
}

fn get_info_as_version(info: &Option<Box<dyn Any>>) -> Result<u32> {
    Ok(*info
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("info not found"))?
        .downcast_ref()
        .ok_or_else(|| anyhow::anyhow!("not YSVR version"))?)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum VariableValue {
    None,
    Int(i64),
    Double(f64),
    ByteString { raw: Base64Bytes },
    MString(String),
}

impl StructUnpack for VariableValue {
    fn unpack<R: Read + Seek>(
        reader: &mut R,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<Self> {
        let var_type = *info
            .as_ref()
            .and_then(|i| i.downcast_ref::<u8>())
            .ok_or_else(|| anyhow::anyhow!("VariableValue: type info missing"))?;
        match var_type {
            0 => Ok(Self::None),
            1 => {
                let v = i64::unpack(reader, big, encoding, info)?;
                Ok(Self::Int(v))
            }
            2 => {
                let v = f64::unpack(reader, big, encoding, info)?;
                Ok(Self::Double(v))
            }
            3 => {
                let len = u16::unpack(reader, big, encoding, info)? as usize;
                let mut buf = vec![0u8; len];
                reader.read_exact(&mut buf)?;
                if buf.starts_with(b"M") && buf.len() >= 3 {
                    let len = u16::from_le_bytes([buf[1], buf[2]]);
                    if buf.len() >= len as usize + 3 {
                        if let Ok(s) = decode_to_string(encoding, &buf[3..], true) {
                            return Ok(Self::MString(s));
                        }
                    }
                }
                Ok(Self::ByteString { raw: buf.into() })
            }
            _ => anyhow::bail!(
                "Unknown variable type: {} at {}",
                var_type,
                reader.stream_position()?
            ),
        }
    }
}

impl StructPack for VariableValue {
    fn pack<W: Write>(
        &self,
        writer: &mut W,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<()> {
        match self {
            Self::Int(v) => v.pack(writer, big, encoding, info)?,
            Self::Double(v) => v.pack(writer, big, encoding, info)?,
            Self::ByteString { raw } => {
                let len = raw.len() as u16;
                len.pack(writer, big, encoding, info)?;
                writer.write_all(&raw)?;
            }
            Self::MString(s) => {
                let encoded = encode_string(encoding, &s, true)?;
                let len = encoded.len() as u16;
                (len + 3).pack(writer, big, encoding, info)?;
                writer.write_u8(b'M')?;
                writer.write_u16(len)?;
                writer.write_all(&encoded)?;
            }
            Self::None => {}
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Variable {
    scope: u8,
    // #TODO: Better version handle
    #[serde(skip_serializing_if = "Option::is_none")]
    unk: Option<u8>,
    script_id: u16,
    variable_index: u16,
    dimension_sizes: Vec<u32>,
    value: VariableValue,
}

impl StructUnpack for Variable {
    fn unpack<R: Read + Seek>(
        reader: &mut R,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<Self> {
        let version = get_info_as_version(info)?;

        let scope = reader.read_u8()?;
        let unk = if version >= 500 {
            Some(reader.read_u8()?)
        } else {
            None
        };
        let script_id = reader.read_u16()?;
        let variable_index = reader.read_u16()?;
        let variable_type = reader.read_u8()?;
        let num_dimensions = reader.read_u8()?;
        let mut dimension_sizes = Vec::with_capacity(num_dimensions as usize);
        for _ in 0..num_dimensions {
            dimension_sizes.push(reader.read_u32()?);
        }
        let value_info = Box::new(variable_type) as Box<dyn Any>;
        let value = VariableValue::unpack(reader, big, encoding, &Some(value_info))?;

        Ok(Self {
            scope,
            unk,
            script_id,
            variable_index,
            dimension_sizes,
            value,
        })
    }
}

impl StructPack for Variable {
    fn pack<W: Write>(
        &self,
        writer: &mut W,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<()> {
        let version = get_info_as_version(info)?;

        writer.write_u8(self.scope)?;
        if version >= 500 {
            writer.write_u8(self.unk.unwrap_or(0))?;
        }
        writer.write_u16(self.script_id)?;
        writer.write_u16(self.variable_index)?;
        writer.write_u8(match &self.value {
            VariableValue::None => 0,
            VariableValue::Int(_) => 1,
            VariableValue::Double(_) => 2,
            VariableValue::ByteString { .. } => 3,
            VariableValue::MString(_) => 3,
        })?;
        writer.write_u8(self.dimension_sizes.len() as u8)?;
        for &size in &self.dimension_sizes {
            writer.write_u32(size)?;
        }
        self.value.pack(writer, big, encoding, &None)?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct YSVRBuilder {}

impl YSVRBuilder {
    /// Creates a new instance of `YSVRBuilder`
    pub const fn new() -> Self {
        YSVRBuilder {}
    }
}

impl ScriptBuilder for YSVRBuilder {
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
        Ok(Box::new(YSVR::new(MemReader::new(buf), encoding, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ybn"]
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 4 && buf.starts_with(b"YSVR") {
            return Some(20);
        }
        None
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::YurisYSVR
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
pub struct YSVR {
    data: YSVRData,
    custom_yaml: bool,
}

impl YSVR {
    pub fn new<T: Read + Seek>(
        mut reader: T,
        encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<Self> {
        let mut sig = [0; 4];
        reader.read_exact(&mut sig)?;
        if &sig != b"YSVR" {
            anyhow::bail!("Unsupported YSVR file.");
        }
        let data = YSVRData::unpack(&mut reader, false, encoding, &None)?;
        Ok(Self {
            data,
            custom_yaml: config.custom_yaml,
        })
    }
}

impl Script for YSVR {
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
    let data: YSVRData = if yaml {
        serde_yaml_ng::from_str(&s).map_err(|e| anyhow::anyhow!("Failed to parse YAML: {}", e))?
    } else {
        serde_json::from_str(&s).map_err(|e| anyhow::anyhow!("Failed to parse JSON: {}", e))?
    };
    writer.write_all(b"YSVR")?;
    data.pack(&mut writer, false, encoding, &None)?;
    Ok(())
}
