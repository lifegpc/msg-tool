//! Yu-Ris YSTB files
use super::yscm::YSCMData;
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use crate::utils::struct_pack::*;
use crate::utils::xored_stream::*;
use anyhow::Result;
use msg_tool_macro::*;
use std::any::Any;
use std::io::{Read, Seek, SeekFrom, Write};
use std::ops::{Deref, DerefMut};

#[derive(Clone, Debug, StructUnpack, StructPack)]
struct YSTBHeader {
    version: u32,
    inst_entry_count: u32,
    inst_index_size: u32,
    args_index_size: u32,
    args_data_size: u32,
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

struct YSTBData {
    header: YSTBHeader,
    insts: Vec<YSTBInst>,
    line_numbers: Vec<u8>,
}

impl std::fmt::Debug for YSTBData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("YSTBData")
            .field("header", &self.header)
            .field("insts", &self.insts)
            .field("line_numbers", &hex::encode(&self.line_numbers))
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
            line_numbers,
        })
    }
}

#[derive(Debug, StructUnpack, StructPack)]
struct YSTBInstBase {
    opcode: u8,
    arg_count: u8,
    unk: u16,
}

struct YSTBInst {
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

#[derive(Debug, StructUnpack, StructPack)]
struct YSTBArgBase {
    id: u16,
    typ: u16,
    size: u32,
}

struct YSTBArg {
    base: YSTBArgBase,
    data: Vec<u8>,
    encoding: Encoding,
}

struct YSTBArgData<'a>(&'a [u8], Encoding);

impl<'a> std::fmt::Debug for YSTBArgData<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0.len() >= 5 && self.0.starts_with(b"M") {
            let len = u16::from_le_bytes([self.0[1], self.0[2]]);
            if len as usize == self.0.len() - 3 {
                if let Ok(s) = decode_to_string(self.1, &self.0[3..], true) {
                    return f.write_str(&s);
                }
            }
        }
        write!(f, "{}", &hex::encode(self.0))
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
    #[allow(unused)]
    xor_key: Option<u32>,
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
        Ok(Self { data, com, xor_key })
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
        writer.write_all(b"YSCM")?;
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
        "txt"
    }

    fn custom_export(&self, filename: &std::path::Path, encoding: Encoding) -> Result<()> {
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
}
