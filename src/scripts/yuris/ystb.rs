//! Yu-Ris YSTB files
use super::yscm::YSCMData;
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use crate::utils::serde_base64bytes::*;
use crate::utils::struct_pack::*;
use crate::utils::xored_stream::*;
use anyhow::Result;
use msg_tool_macro::*;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::io::{Read, Seek, SeekFrom, Write};
use std::ops::{Deref, DerefMut};

#[derive(Clone, Debug, StructUnpack, StructPack, Deserialize, Serialize)]
struct YSTBHeader {
    version: u32,
    #[serde(skip)]
    inst_entry_count: u32,
    #[serde(skip)]
    inst_index_size: u32,
    #[serde(skip)]
    args_index_size: u32,
    #[serde(skip)]
    args_data_size: u32,
    #[serde(skip)]
    line_numbers_size: u32,
    reserve0: u32,
}

#[derive(Clone, Debug, StructUnpack, StructPack)]
struct YSTBHeaderV2 {
    version: u32,
    code_seg_size: u32,
    args_seg_size: u32,
    args_seg_offset: u32,
    reserved0: u32,
    reserved1: u32,
    reserved2: u32,
}

#[derive(Deserialize, Serialize)]
struct YSTBData {
    #[serde(flatten)]
    header: YSTBHeader,
    insts: Vec<YSTBInst>,
    line_numbers: Base64Bytes,
}

impl std::fmt::Debug for YSTBData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("YSTBData")
            .field("header", &self.header)
            .field("insts", &self.insts)
            .field("line_numbers", &hex::encode(&self.line_numbers.bytes))
            .finish()
    }
}

impl StructUnpack for YSTBData {
    fn unpack<R: Read + Seek>(
        reader: &mut R,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<Self> {
        let header = YSTBHeader::unpack(reader, big, encoding, info)?;
        let insts = reader.read_struct_vec::<YSTBInstBase>(
            header.inst_entry_count as usize,
            big,
            encoding,
            info,
        )?;
        let info = Box::new(header.clone()) as Box<dyn Any>;
        let args = reader.read_struct_vec::<YSTBArg>(
            (header.args_index_size / 0xC) as usize,
            big,
            encoding,
            &Some(info),
        )?;
        let mut args = args.into_iter();
        let insts = insts
            .into_iter()
            .map(|base| {
                let args = args.by_ref().take(base.arg_count as usize).collect();
                YSTBInst { base, args }
            })
            .collect();
        let line_numbers = reader.peek_exact_at_vec(
            0x20 + header.inst_index_size as u64
                + header.args_index_size as u64
                + header.args_data_size as u64,
            header.line_numbers_size as usize,
        )?;
        Ok(Self {
            header,
            insts,
            line_numbers: line_numbers.into(),
        })
    }
}

#[derive(Debug, StructUnpack, StructPack, Deserialize, Serialize)]
struct YSTBInstBase {
    opcode: u8,
    #[serde(skip)]
    arg_count: u8,
    unk: u16,
}

#[derive(Deserialize, Serialize)]
struct YSTBInst {
    #[serde(flatten)]
    base: YSTBInstBase,
    args: Vec<YSTBArg>,
}

impl Deref for YSTBInst {
    type Target = YSTBInstBase;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for YSTBInst {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl std::fmt::Debug for YSTBInst {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("YSTBInst")
            .field("opcode", &self.opcode)
            .field("arg_count", &self.arg_count)
            .field("unk", &self.unk)
            .field("args", &self.args)
            .finish()
    }
}

#[derive(Clone, Debug, StructUnpack, StructPack, Deserialize, Serialize)]
struct YSTBArgBase {
    id: u16,
    typ: u16,
    #[serde(skip)]
    size: u32,
}

struct YSTBArg {
    base: YSTBArgBase,
    data: Vec<u8>,
    encoding: Encoding,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "t")]
enum YSTBArgDat {
    Raw { data: Base64Bytes },
    MString { s: String },
}

impl TryFrom<YSTBArgTmp> for YSTBArg {
    type Error = anyhow::Error;
    fn try_from(value: YSTBArgTmp) -> Result<Self> {
        let data = match value.data {
            YSTBArgDat::Raw { data } => data.bytes,
            YSTBArgDat::MString { s } => {
                let mut m = MemWriter::new();
                m.write_u8(b'M')?;
                let d = encode_string(value.encoding, &s, true)?;
                m.write_u16(d.len() as u16)?;
                m.write_all(&d)?;
                m.into_inner()
            }
        };
        Ok(Self {
            base: value.base,
            data,
            encoding: value.encoding,
        })
    }
}

impl<'a> TryFrom<&'a YSTBArg> for YSTBArgTmp {
    type Error = anyhow::Error;
    fn try_from(value: &'a YSTBArg) -> Result<Self> {
        if value.data.len() >= 5 && value.data.starts_with(b"M") {
            let len = u16::from_le_bytes([value.data[1], value.data[2]]);
            if len as usize == value.data.len() - 3 {
                if let Ok(s) = decode_to_string(value.encoding, &value.data[3..], true) {
                    return Ok(Self {
                        base: value.base.clone(),
                        data: YSTBArgDat::MString { s },
                        encoding: value.encoding,
                    });
                }
            }
        }
        Ok(Self {
            base: value.base.clone(),
            data: YSTBArgDat::Raw {
                data: value.data.clone().into(),
            },
            encoding: value.encoding,
        })
    }
}

#[derive(Deserialize, Serialize)]
struct YSTBArgTmp {
    #[serde(flatten)]
    base: YSTBArgBase,
    data: YSTBArgDat,
    encoding: Encoding,
}

impl<'de> Deserialize<'de> for YSTBArg {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let tmp = YSTBArgTmp::deserialize(deserializer)?;
        tmp.try_into().map_err(serde::de::Error::custom)
    }
}

impl Serialize for YSTBArg {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let tmp: YSTBArgTmp = self.try_into().map_err(serde::ser::Error::custom)?;
        tmp.serialize(serializer)
    }
}

struct YSTBArgData<'a>(&'a [u8], Encoding);

impl<'a> std::fmt::Debug for YSTBArgData<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let data = self.0;
        // 6-byte local variable reference: [48 03 00 40] [var_id]
        if data.len() == 6 && &data[0..4] == b"\x48\x03\x00\x40" {
            let id = u16::from_le_bytes([data[4], data[5]]);
            return write!(f, "var[{:04x}]", id);
        }
        // 6-byte string variable reference: [48 03 00 24] [var_id]
        if data.len() == 6 && &data[0..4] == b"\x48\x03\x00\x24" {
            let id = u16::from_le_bytes([data[4], data[5]]);
            return write!(f, "str[{:04x}]", id);
        }
        // Structured variable ref with embedded M-string label
        // Pattern: [48 03 00 24] [2-byte var_id] [4D len content...] [padding]
        if data.len() >= 9 && &data[0..4] == b"\x48\x03\x00\x24" && data[6] == b'M' {
            let len = u16::from_le_bytes([data[7], data[8]]) as usize;
            if len + 9 <= data.len() {
                if let Ok(s) = decode_to_string(self.1, &data[9..9 + len], true) {
                    return f.write_str(&s);
                }
            }
        }
        // M-string format
        if data.len() >= 5 && data.starts_with(b"M") {
            let len = u16::from_le_bytes([data[1], data[2]]);
            if len as usize == data.len() - 3 {
                if let Ok(s) = decode_to_string(self.1, &data[3..], true) {
                    return f.write_str(&s);
                }
            }
        }
        write!(f, "{}", &hex::encode(data))
    }
}

impl std::fmt::Debug for YSTBArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("YSTBArg")
            .field("id", &self.id)
            .field("type", &self.typ)
            .field("size", &self.size)
            .field("data", &YSTBArgData(&self.data, self.encoding))
            .finish()
    }
}

impl Deref for YSTBArg {
    type Target = YSTBArgBase;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for YSTBArg {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

fn get_info_as_header(info: &Option<Box<dyn Any>>) -> Result<&YSTBHeader> {
    Ok(info
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("info not found"))?
        .downcast_ref()
        .ok_or_else(|| anyhow::anyhow!("not YSTBHeader"))?)
}

impl StructUnpack for YSTBArg {
    fn unpack<R: Read + Seek>(
        reader: &mut R,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<Self> {
        let base = YSTBArgBase::unpack(reader, big, encoding, info)?;
        let offset = u32::unpack(reader, big, encoding, info)?;
        let header = get_info_as_header(info)?;
        let target =
            0x20 + header.inst_index_size as u64 + header.args_index_size as u64 + offset as u64;
        let data = reader.peek_exact_at_vec(target, base.size as usize)?;
        Ok(Self {
            base,
            data,
            encoding,
        })
    }
}

#[derive(Debug)]
pub struct YSTBBuilder {}

impl YSTBBuilder {
    /// Creates a new instance of `YSTBBuilder`
    pub const fn new() -> Self {
        YSTBBuilder {}
    }
}

impl ScriptBuilder for YSTBBuilder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Cp932
    }

    fn build_script(
        &self,
        buf: Vec<u8>,
        filename: &str,
        encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
        archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script + Send + Sync>> {
        Ok(Box::new(YSTB::new(
            MemReader::new(buf),
            filename,
            encoding,
            config,
            archive,
        )?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ybn"]
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 4 && buf.starts_with(b"YSTB") {
            return Some(20);
        }
        None
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::YurisYSTB
    }
}

#[derive(Debug)]
pub struct YSTB {
    data: YSTBData,
    com: YSCMData,
    xor_key: Option<u32>,
    disasm: bool,
    custom_yaml: bool,
}

impl YSTB {
    pub fn new<T: Read + Seek>(
        mut reader: T,
        filename: &str,
        encoding: Encoding,
        config: &ExtraConfig,
        archive: Option<&Box<dyn Script>>,
    ) -> Result<Self> {
        let mut sig = [0; 4];
        reader.read_exact(&mut sig)?;
        if &sig != b"YSTB" {
            anyhow::bail!("Unsupported YSTB file.");
        }
        let mut xor_key = None;
        let data = match YSTBData::unpack(&mut reader, false, encoding, &None) {
            Ok(data) => data,
            Err(err) => {
                let key = Self::get_xor_key(&mut reader)?;
                if key == 0 {
                    return Err(err);
                }
                xor_key = Some(key);
                let mut writer = MemWriter::with_capacity(reader.stream_length()? as usize);
                Self::xor(&mut reader, &mut writer, key)?;
                let mut reader = writer.to_ref();
                reader.pos = 4;
                YSTBData::unpack(&mut reader, false, encoding, &None)?
            }
        };
        // println!("xor_key: {:?}, {:#?}", xor_key, data);
        let yscm = if let Some(path) = config.yuris_ysc_path.as_ref() {
            crate::utils::files::read_file(path)?
        } else {
            let path = std::path::Path::new(filename);
            let pdir = path.parent().unwrap_or_else(|| std::path::Path::new(""));
            let fp = pdir.join("ysc.ybn");
            if let Some(archive) = archive {
                let mut file = archive.open_file_by_name(&fp.to_string_lossy(), true)?;
                file.data()?
            } else {
                let p = crate::utils::files::get_ignorecase_path(&fp)?;
                crate::utils::files::read_file(&p)?
            }
        };
        if !yscm.starts_with(b"YSCM") {
            anyhow::bail!("Unsupported YSCM file. (ysc.ybn)");
        }
        let mut reader = MemReader::new(yscm);
        reader.pos = 4;
        let com = YSCMData::unpack(&mut reader, false, encoding, &None)?;
        Ok(Self {
            data,
            com,
            xor_key,
            disasm: config.yuris_ystb_disasm,
            custom_yaml: config.custom_yaml,
        })
    }

    fn get_xor_key<T: Read + Seek>(reader: &mut T) -> Result<u32> {
        let version = reader.peek_u32_at(4)?;
        reader.seek(SeekFrom::Start(4))?;
        Ok(if matches!(version, 201..300) {
            let header: YSTBHeaderV2 = reader.read_struct(false, Encoding::Cp932, &None)?;
            if (header.code_seg_size as u64) + (header.args_seg_size as u64) < 0x10 {
                0
            } else {
                reader.peek_u32_at(0x2C)?
            }
        } else {
            let header: YSTBHeader = reader.read_struct(false, Encoding::Cp932, &None)?;
            if header.args_data_size == 0 {
                0
            } else {
                reader.peek_u32_at(header.inst_index_size as u64 + 0x28)?
            }
        })
    }

    fn xor<R: Read + Seek, W: Write>(
        mut reader: &mut R,
        writer: &mut W,
        xor_key: u32,
    ) -> Result<()> {
        let key = xor_key.to_le_bytes();
        reader.seek(SeekFrom::Start(4))?;
        writer.write_all(b"YSTB")?;
        let version = reader.peek_u32()?;
        if matches!(version, 201..300) {
            let header: YSTBHeaderV2 = reader.read_struct(false, Encoding::Cp932, &None)?;
            writer.write_struct(&header, false, Encoding::Cp932, &None)?;
            let mut stream = XoredKeyStream::new(
                StreamRegion::with_size(&mut reader, header.code_seg_size as u64)?,
                key.to_vec(),
                0,
            );
            std::io::copy(&mut stream, writer)?;
            stream = XoredKeyStream::new(
                StreamRegion::with_size(&mut reader, header.args_seg_size as u64)?,
                key.to_vec(),
                0,
            );
            std::io::copy(&mut stream, writer)?;
            std::io::copy(reader, writer)?;
        } else {
            let header: YSTBHeader = reader.read_struct(false, Encoding::Cp932, &None)?;
            writer.write_struct(&header, false, Encoding::Cp932, &None)?;
            let mut stream = XoredKeyStream::new(
                StreamRegion::with_size(&mut reader, header.inst_index_size as u64)?,
                key.to_vec(),
                0,
            );
            std::io::copy(&mut stream, writer)?;
            stream = XoredKeyStream::new(
                StreamRegion::with_size(&mut reader, header.args_index_size as u64)?,
                key.to_vec(),
                0,
            );
            std::io::copy(&mut stream, writer)?;
            stream = XoredKeyStream::new(
                StreamRegion::with_size(&mut reader, header.args_data_size as u64)?,
                key.to_vec(),
                0,
            );
            std::io::copy(&mut stream, writer)?;
            stream = XoredKeyStream::new(
                StreamRegion::with_size(&mut reader, header.line_numbers_size as u64)?,
                key.to_vec(),
                0,
            );
            std::io::copy(&mut stream, writer)?;
            std::io::copy(reader, writer)?;
        }
        Ok(())
    }
}

impl Script for YSTB {
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
        if self.disasm {
            "txt"
        } else if self.custom_yaml {
            "yaml"
        } else {
            "json"
        }
    }

    fn custom_export(&self, filename: &std::path::Path, encoding: Encoding) -> Result<()> {
        if !self.disasm {
            let s = if self.custom_yaml {
                serde_yaml_ng::to_string(&self.data)?
            } else {
                serde_json::to_string_pretty(&self.data)?
            };
            let mut f = std::fs::File::create(filename)?;
            let encoded = encode_string(encoding, &s, true)?;
            f.write_all(&encoded)?;
            return Ok(());
        }
        let mut file = MemWriter::new();
        let mut indent = String::new();
        for code in self.data.insts.iter() {
            let meta =
                self.com.opcodes.get(code.opcode as usize).ok_or_else(|| {
                    anyhow::anyhow!("Failed to find op {:x}'s metadata", code.opcode)
                })?;
            if meta.name == "IFEND" || meta.name == "IFBLEND" || meta.name == "LOOPEND" {
                indent.pop();
                indent.pop();
            }
            write!(file, "{}", indent)?;
            if meta.name == "GOSUB" {
                if code.arg_count < 1 {
                    anyhow::bail!("GOSUB at least need one argument.");
                }
                let arg0 = &code.args[0];
                let name = format!("{:?}", &YSTBArgData(&arg0.data, arg0.encoding));
                write!(file, "\\{}(", name.trim_matches('"'))?;
                let mut first = true;
                for arg in &code.args[1..] {
                    write!(
                        file,
                        "{}{:?}",
                        if first {
                            first = false;
                            ""
                        } else {
                            ", "
                        },
                        &YSTBArgData(&arg.data, arg.encoding)
                    )?;
                }
                writeln!(file, ")")?;
            } else {
                write!(file, "{}[", meta.name)?;
                let mut first = true;
                for arg in &code.args {
                    if first {
                        first = false;
                    } else {
                        write!(file, ", ")?;
                    }
                    if meta.arguments.len() > arg.id as usize {
                        write!(file, "{}=", meta.arguments[arg.id as usize].name)?;
                    }
                    write!(file, "{:?}", &YSTBArgData(&arg.data, arg.encoding))?;
                }
                writeln!(file, "]")?;
            }
            if meta.name == "IF" || meta.name == "ELSE" || meta.name == "LOOP" {
                indent += "  ";
            }
        }
        let mut f = std::fs::File::create(filename)?;
        if encoding.is_utf8() {
            f.write_all(&file.data)?;
        } else {
            let s = decode_to_string(Encoding::Utf8, &file.data, true)?;
            let encoded = encode_string(encoding, &s, true)?;
            f.write_all(&encoded)?;
        }
        Ok(())
    }

    fn custom_import<'a>(
        &'a self,
        custom_filename: &'a str,
        mut file: Box<dyn WriteSeek + 'a>,
        encoding: Encoding,
        output_encoding: Encoding,
    ) -> Result<()> {
        if self.disasm {
            anyhow::bail!("Import is not supported for disasm mode.");
        }
        let mut f = MemWriter::new();
        let data = crate::utils::files::read_file(custom_filename)?;
        let data = decode_to_string(output_encoding, &data, true)?;
        let mut data: YSTBData = if self.custom_yaml {
            serde_yaml_ng::from_str(&data)?
        } else {
            serde_json::from_str(&data)?
        };
        f.write_all(b"YSTB")?;
        data.header.line_numbers_size = data.line_numbers.len() as u32;
        data.header.inst_entry_count = data.insts.len() as u32;
        data.header.inst_index_size = data.header.inst_entry_count * 4;
        data.header.pack(&mut f, false, encoding, &None)?;
        for i in data.insts.iter_mut() {
            i.base.arg_count = i.args.len() as u8;
            i.base.pack(&mut f, false, encoding, &None)?;
        }
        let arg_count = data.insts.iter().fold(0, |c, i| c + i.args.len());
        data.header.args_index_size = arg_count as u32 * 0xC;
        let mut cpos = f.pos as u64;
        f.pos += data.header.args_index_size as usize;
        let bpos = f.pos as u32;
        for i in data.insts.iter_mut() {
            let meta =
                self.com.opcodes.get(i.opcode as usize).ok_or_else(|| {
                    anyhow::anyhow!("Failed to find op {:x}'s metadata", i.opcode)
                })?;
            for arg in i.args.iter_mut() {
                arg.base.size = arg.data.len() as u32;
                f.write_struct_at(cpos, &arg.base, false, encoding, &None)?;
                cpos += 8;
                if arg.base.size == 0
                    || (meta.name == "RETURNCODE" && arg.base.size == 1 && arg.data[0] == b'M')
                {
                    f.write_u32_at(cpos, 0)?;
                    cpos += 4;
                    continue;
                }
                let offset = f.pos as u32 - bpos;
                f.write_u32_at(cpos, offset)?;
                cpos += 4;
                f.write_all(&arg.data)?;
            }
        }
        data.header.args_data_size = f.pos as u32 - bpos;
        f.write_all(&data.line_numbers)?;
        f.pos = 4;
        data.header.pack(&mut f, false, encoding, &None)?;
        if let Some(xor) = self.xor_key {
            let mut r = MemReader::new(f.into_inner());
            f = MemWriter::new();
            Self::xor(&mut r, &mut f, xor)?;
        }
        file.write_all(&f.data)?;
        Ok(())
    }
}
