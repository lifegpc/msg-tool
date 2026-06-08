//! Yu-Ris YSTB files
use super::yscm::YSCMData;
use super::yslb::{Label, YSLBData};
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
use std::collections::{BTreeMap, BTreeSet};
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

#[derive(Clone, Debug, StructUnpack, StructPack, Deserialize, Serialize)]
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
    NotEqual,
    Mod,
    LogAnd,
    PerformVarIndexAtion,
    Mul,
    Add,
    Nop,
    Sub,
    Div,
    Equal,
    Less,
    Greater,
    BinAnd,
    PushInt8 { value: i8 },
    PushDouble { value: f64 },
    PushScalarVarVar { index: u16 },
    PushScalarVarStr { index: u16 },
    PushInt32 { value: i32 },
    PushInt64 { value: i64 },
    MString { s: String },
    BinOr,
    ChangeSign,
    Le,
    PrepareVarIndexationVar { index: u16 },
    PrepareVarIndexationStr { index: u16 },
    PushInt16 { value: i16 },
    Ge,
    BinXor,
    ToNumber,
    ToString,
    PushArrayVarVar { index: u16 },
    PushArrayVarStr { index: u16 },
    LogOr,
    Array { data: Vec<YSTBArgDat> },
    String { s: String },
}

impl YSTBArgDat {
    fn to_data(self, encoding: Encoding) -> Result<Vec<u8>> {
        Ok(match self {
            YSTBArgDat::Raw { data } => data.bytes,
            YSTBArgDat::NotEqual => NOTEQUAL_TYPE.into(),
            YSTBArgDat::Mod => MOD_TYPE.into(),
            YSTBArgDat::LogAnd => LOGAND_TYPE.into(),
            YSTBArgDat::PerformVarIndexAtion => PERFORMVARINDEXATION_TYPE.into(),
            YSTBArgDat::Mul => MUL_TYPE.into(),
            YSTBArgDat::Add => ADD_TYPE.into(),
            YSTBArgDat::Nop => NOP_TYPE.into(),
            YSTBArgDat::Sub => SUB_TYPE.into(),
            YSTBArgDat::Div => DIV_TYPE.into(),
            YSTBArgDat::Equal => EQUAL_TYPE.into(),
            YSTBArgDat::Less => LESS_TYPE.into(),
            YSTBArgDat::Greater => GREATER_TYPE.into(),
            YSTBArgDat::BinAnd => BINAND_TYPE.into(),
            YSTBArgDat::PushInt8 { value } => {
                let mut m = MemWriter::new();
                m.write_u8(b'B')?;
                m.write_u16(1)?;
                m.write_i8(value)?;
                m.into_inner()
            }
            YSTBArgDat::PushDouble { value } => {
                let mut m = MemWriter::new();
                m.write_u8(b'F')?;
                m.write_u16(8)?;
                m.write_f64(value)?;
                m.into_inner()
            }
            YSTBArgDat::PushScalarVarVar { index } => {
                let mut m = MemWriter::new();
                m.write_u8(b'H')?;
                m.write_u16(3)?;
                m.write_u8(b'$')?;
                m.write_u16(index)?;
                m.into_inner()
            }
            YSTBArgDat::PushScalarVarStr { index } => {
                let mut m = MemWriter::new();
                m.write_u8(b'H')?;
                m.write_u16(3)?;
                m.write_u8(b'@')?;
                m.write_u16(index)?;
                m.into_inner()
            }
            YSTBArgDat::PushInt32 { value } => {
                let mut m = MemWriter::new();
                m.write_u8(b'I')?;
                m.write_u16(4)?;
                m.write_i32(value)?;
                m.into_inner()
            }
            YSTBArgDat::PushInt64 { value } => {
                let mut m = MemWriter::new();
                m.write_u8(b'L')?;
                m.write_u16(8)?;
                m.write_i64(value)?;
                m.into_inner()
            }
            YSTBArgDat::MString { s } => {
                let mut m = MemWriter::new();
                m.write_u8(b'M')?;
                let d = encode_string(encoding, &s, true)?;
                m.write_u16(d.len() as u16)?;
                m.write_all(&d)?;
                m.into_inner()
            }
            YSTBArgDat::BinOr => BINOR_TYPE.into(),
            YSTBArgDat::ChangeSign => CHANGESIGN_TYPE.into(),
            YSTBArgDat::Le => LE_TYPE.into(),
            YSTBArgDat::PrepareVarIndexationVar { index } => {
                let mut m = MemWriter::new();
                m.write_u8(b'V')?;
                m.write_u16(3)?;
                m.write_u8(b'$')?;
                m.write_u16(index)?;
                m.into_inner()
            }
            YSTBArgDat::PrepareVarIndexationStr { index } => {
                let mut m = MemWriter::new();
                m.write_u8(b'V')?;
                m.write_u16(3)?;
                m.write_u8(b'@')?;
                m.write_u16(index)?;
                m.into_inner()
            }
            YSTBArgDat::PushInt16 { value } => {
                let mut m = MemWriter::new();
                m.write_u8(b'W')?;
                m.write_u16(2)?;
                m.write_i16(value)?;
                m.into_inner()
            }
            YSTBArgDat::Ge => GE_TYPE.into(),
            YSTBArgDat::BinXor => BINXOR_TYPE.into(),
            YSTBArgDat::ToNumber => TONUMBER_TYPE.into(),
            YSTBArgDat::ToString => TOSTRING_TYPE.into(),
            YSTBArgDat::PushArrayVarVar { index } => {
                let mut m = MemWriter::new();
                m.write_u8(b'v')?;
                m.write_u16(3)?;
                m.write_u8(b'$')?;
                m.write_u16(index)?;
                m.into_inner()
            }
            YSTBArgDat::PushArrayVarStr { index } => {
                let mut m = MemWriter::new();
                m.write_u8(b'v')?;
                m.write_u16(3)?;
                m.write_u8(b'@')?;
                m.write_u16(index)?;
                m.into_inner()
            }
            YSTBArgDat::LogOr => LOGOR_TYPE.into(),
            YSTBArgDat::Array { data } => {
                let mut m = MemWriter::new();
                for d in data {
                    m.write_all(&d.to_data(encoding)?)?;
                }
                m.into_inner()
            }
            YSTBArgDat::String { s } => encode_string(encoding, &s, true)?,
        })
    }
}

impl TryFrom<YSTBArgTmp> for YSTBArg {
    type Error = anyhow::Error;
    fn try_from(value: YSTBArgTmp) -> Result<Self> {
        let data = value.data.to_data(value.encoding)?;
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
        let mut list = Vec::new();
        let mut data = value.data.as_slice();
        loop {
            if data.is_empty() {
                break;
            }
            if data.starts_with(NOTEQUAL_TYPE) {
                list.push(YSTBArgDat::NotEqual);
                data = &data[3..];
            } else if data.starts_with(MOD_TYPE) {
                list.push(YSTBArgDat::Mod);
                data = &data[3..];
            } else if data.starts_with(LOGAND_TYPE) {
                list.push(YSTBArgDat::LogAnd);
                data = &data[3..];
            } else if data.starts_with(PERFORMVARINDEXATION_TYPE) {
                list.push(YSTBArgDat::PerformVarIndexAtion);
                data = &data[4..];
            } else if data.starts_with(MUL_TYPE) {
                list.push(YSTBArgDat::Mul);
                data = &data[3..];
            } else if data.starts_with(ADD_TYPE) {
                list.push(YSTBArgDat::Add);
                data = &data[3..];
            } else if data.starts_with(NOP_TYPE) {
                list.push(YSTBArgDat::Nop);
                data = &data[3..];
            } else if data.starts_with(SUB_TYPE) {
                list.push(YSTBArgDat::Sub);
                data = &data[3..];
            } else if data.starts_with(DIV_TYPE) {
                list.push(YSTBArgDat::Div);
                data = &data[3..];
            } else if data.starts_with(EQUAL_TYPE) {
                list.push(YSTBArgDat::Equal);
                data = &data[3..];
            } else if data.starts_with(LESS_TYPE) {
                list.push(YSTBArgDat::Less);
                data = &data[3..];
            } else if data.starts_with(GREATER_TYPE) {
                list.push(YSTBArgDat::Greater);
                data = &data[3..];
            } else if data.starts_with(BINAND_TYPE) {
                list.push(YSTBArgDat::BinAnd);
                data = &data[3..];
            } else if data.starts_with(PUSHINT8_TYPE) {
                if data.len() < 4 {
                    list.push(YSTBArgDat::Raw {
                        data: data.to_vec().into(),
                    });
                    break;
                }
                list.push(YSTBArgDat::PushInt8 {
                    value: i8::from_le_bytes([data[3]]),
                });
                data = &data[4..];
            } else if data.starts_with(PUSHDOUBLE_TYPE) {
                if data.len() < 11 {
                    list.push(YSTBArgDat::Raw {
                        data: data.to_vec().into(),
                    });
                    break;
                }
                list.push(YSTBArgDat::PushDouble {
                    value: f64::from_le_bytes([
                        data[3], data[4], data[5], data[6], data[7], data[8], data[9], data[10],
                    ]),
                });
                data = &data[11..];
            } else if data.starts_with(PUSHSCALARVAR_VAR_TYPE) {
                if data.len() < 6 {
                    list.push(YSTBArgDat::Raw {
                        data: data.to_vec().into(),
                    });
                    break;
                }
                list.push(YSTBArgDat::PushScalarVarVar {
                    index: u16::from_le_bytes([data[4], data[5]]),
                });
                data = &data[6..];
            } else if data.starts_with(PUSHSCALARVAR_STR_TYPE) {
                if data.len() < 6 {
                    list.push(YSTBArgDat::Raw {
                        data: data.to_vec().into(),
                    });
                    break;
                }
                list.push(YSTBArgDat::PushScalarVarStr {
                    index: u16::from_le_bytes([data[4], data[5]]),
                });
                data = &data[6..];
            } else if data.starts_with(PUSHINT32_TYPE) {
                if data.len() < 7 {
                    list.push(YSTBArgDat::Raw {
                        data: data.to_vec().into(),
                    });
                    break;
                }
                list.push(YSTBArgDat::PushInt32 {
                    value: i32::from_le_bytes([data[3], data[4], data[5], data[6]]),
                });
                data = &data[7..];
            } else if data.starts_with(PUSHINT64_TYPE) {
                if data.len() < 11 {
                    list.push(YSTBArgDat::Raw {
                        data: data.to_vec().into(),
                    });
                    break;
                }
                list.push(YSTBArgDat::PushInt64 {
                    value: i64::from_le_bytes([
                        data[3], data[4], data[5], data[6], data[7], data[8], data[9], data[10],
                    ]),
                });
                data = &data[11..];
            } else if data.starts_with(PUSHSTRING_TYPE) {
                if data.len() < 3 {
                    list.push(YSTBArgDat::Raw {
                        data: data.to_vec().into(),
                    });
                    break;
                }
                let len = u16::from_le_bytes([data[1], data[2]]) as usize;
                if data.len() < 3 + len {
                    list.push(YSTBArgDat::Raw {
                        data: data.to_vec().into(),
                    });
                    break;
                }
                if let Ok(s) = decode_to_string(value.encoding, &data[3..3 + len], true) {
                    list.push(YSTBArgDat::MString { s });
                } else {
                    list.push(YSTBArgDat::Raw {
                        data: data[..3 + len].to_vec().into(),
                    });
                }
                data = &data[3 + len..];
            } else if data.starts_with(BINOR_TYPE) {
                list.push(YSTBArgDat::BinOr);
                data = &data[3..];
            } else if data.starts_with(CHANGESIGN_TYPE) {
                list.push(YSTBArgDat::ChangeSign);
                data = &data[3..];
            } else if data.starts_with(LE_TYPE) {
                list.push(YSTBArgDat::Le);
                data = &data[3..];
            } else if data.starts_with(PREPAREVARINDEXATION_VAR_TYPE) {
                if data.len() < 6 {
                    list.push(YSTBArgDat::Raw {
                        data: data.to_vec().into(),
                    });
                    break;
                }
                list.push(YSTBArgDat::PrepareVarIndexationVar {
                    index: u16::from_le_bytes([data[4], data[5]]),
                });
                data = &data[6..];
            } else if data.starts_with(PREPAREVARINDEXATION_STR_TYPE) {
                if data.len() < 6 {
                    list.push(YSTBArgDat::Raw {
                        data: data.to_vec().into(),
                    });
                    break;
                }
                list.push(YSTBArgDat::PrepareVarIndexationStr {
                    index: u16::from_le_bytes([data[4], data[5]]),
                });
                data = &data[6..];
            } else if data.starts_with(PUSHINT16_TYPE) {
                if data.len() < 5 {
                    list.push(YSTBArgDat::Raw {
                        data: data.to_vec().into(),
                    });
                    break;
                }
                list.push(YSTBArgDat::PushInt16 {
                    value: i16::from_le_bytes([data[3], data[4]]),
                });
                data = &data[5..];
            } else if data.starts_with(GE_TYPE) {
                list.push(YSTBArgDat::Ge);
                data = &data[3..];
            } else if data.starts_with(BINXOR_TYPE) {
                list.push(YSTBArgDat::BinXor);
                data = &data[3..];
            } else if data.starts_with(TONUMBER_TYPE) {
                list.push(YSTBArgDat::ToNumber);
                data = &data[3..];
            } else if data.starts_with(TOSTRING_TYPE) {
                list.push(YSTBArgDat::ToString);
                data = &data[3..];
            } else if data.starts_with(PUSHARRAYVAR_VAR_TYPE) {
                if data.len() < 6 {
                    list.push(YSTBArgDat::Raw {
                        data: data.to_vec().into(),
                    });
                    break;
                }
                list.push(YSTBArgDat::PushArrayVarVar {
                    index: u16::from_le_bytes([data[4], data[5]]),
                });
                data = &data[6..];
            } else if data.starts_with(PUSHARRAYVAR_STR_TYPE) {
                if data.len() < 6 {
                    list.push(YSTBArgDat::Raw {
                        data: data.to_vec().into(),
                    });
                    break;
                }
                list.push(YSTBArgDat::PushArrayVarStr {
                    index: u16::from_le_bytes([data[4], data[5]]),
                });
                data = &data[6..];
            } else if data.starts_with(LOGOR_TYPE) {
                list.push(YSTBArgDat::LogOr);
                data = &data[3..];
            } else {
                if list.is_empty() {
                    if !data.contains(&0) && let Ok(s) = decode_to_string(value.encoding, data, true) {
                        list.push(YSTBArgDat::String { s });
                        break;
                    }
                }
                list.push(YSTBArgDat::Raw {
                    data: data.to_vec().into(),
                });
                break;
            }
        }
        if list.len() > 1 {
            return Ok(Self {
                base: value.base.clone(),
                data: YSTBArgDat::Array { data: list },
                encoding: value.encoding,
            });
        }
        if let Some(data) = list.pop() {
            return Ok(Self {
                base: value.base.clone(),
                data,
                encoding: value.encoding,
            });
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

// [opcode] (1 byte) [data size] (2 byte little endian) [data]
const NOTEQUAL_TYPE: &[u8] = b"!\0\0";
const MOD_TYPE: &[u8] = b"%\0\0";
const LOGAND_TYPE: &[u8] = b"&\0\0";
const PERFORMVARINDEXATION_TYPE: &[u8] = b")\x01\0\0";
const MUL_TYPE: &[u8] = b"*\0\0";
const ADD_TYPE: &[u8] = b"+\0\0";
const NOP_TYPE: &[u8] = b",\0\0";
const SUB_TYPE: &[u8] = b"-\0\0";
const DIV_TYPE: &[u8] = b"/\0\0";
const EQUAL_TYPE: &[u8] = b"=\0\0";
const LESS_TYPE: &[u8] = b"<\0\0";
const GREATER_TYPE: &[u8] = b">\0\0";
const BINAND_TYPE: &[u8] = b"A\0\0";
/// then one byte data
const PUSHINT8_TYPE: &[u8] = b"B\x01\0";
/// then eight byte data
const PUSHDOUBLE_TYPE: &[u8] = b"F\x08\0";
const PUSHSCALARVAR_VAR_TYPE: &[u8] = b"H\x03\0$";
const PUSHSCALARVAR_STR_TYPE: &[u8] = b"H\x03\0@";
const PUSHINT32_TYPE: &[u8] = b"I\x04\0";
const PUSHINT64_TYPE: &[u8] = b"L\x08\0";
const PUSHSTRING_TYPE: &[u8] = b"M";
const BINOR_TYPE: &[u8] = b"O\0\0";
const CHANGESIGN_TYPE: &[u8] = b"R\0\0";
const LE_TYPE: &[u8] = b"S\0\0";
const PREPAREVARINDEXATION_VAR_TYPE: &[u8] = b"V\x03\0$";
const PREPAREVARINDEXATION_STR_TYPE: &[u8] = b"V\x03\0@";
const PUSHINT16_TYPE: &[u8] = b"W\x02\0";
const GE_TYPE: &[u8] = b"Z\0\0";
const BINXOR_TYPE: &[u8] = b"^\0\0";
const TONUMBER_TYPE: &[u8] = b"i\0\0";
const TOSTRING_TYPE: &[u8] = b"s\0\0";
const PUSHARRAYVAR_VAR_TYPE: &[u8] = b"v\x03\0$";
const PUSHARRAYVAR_STR_TYPE: &[u8] = b"v\x03\0@";
const LOGOR_TYPE: &[u8] = b"|\0\0";

impl<'a> std::fmt::Debug for YSTBArgData<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut data = self.0;
        let mut first = true;
        loop {
            if data.is_empty() {
                break;
            }
            let is_first = first;
            if first {
                first = false;
            } else {
                f.write_str(" ")?;
            }
            if data.starts_with(NOTEQUAL_TYPE) {
                f.write_str("notequal")?;
                data = &data[3..];
            } else if data.starts_with(MOD_TYPE) {
                f.write_str("mod")?;
                data = &data[3..];
            } else if data.starts_with(LOGAND_TYPE) {
                f.write_str("logand")?;
                data = &data[3..];
            } else if data.starts_with(PERFORMVARINDEXATION_TYPE) {
                f.write_str("performvarindexation")?;
                data = &data[4..];
            } else if data.starts_with(MUL_TYPE) {
                f.write_str("mul")?;
                data = &data[3..];
            } else if data.starts_with(ADD_TYPE) {
                f.write_str("add")?;
                data = &data[3..];
            } else if data.starts_with(NOP_TYPE) {
                f.write_str("nop")?;
                data = &data[3..];
            } else if data.starts_with(SUB_TYPE) {
                f.write_str("sub")?;
                data = &data[3..];
            } else if data.starts_with(DIV_TYPE) {
                f.write_str("div")?;
                data = &data[3..];
            } else if data.starts_with(EQUAL_TYPE) {
                f.write_str("equal")?;
                data = &data[3..];
            } else if data.starts_with(LESS_TYPE) {
                f.write_str("less")?;
                data = &data[3..];
            } else if data.starts_with(GREATER_TYPE) {
                f.write_str("greater")?;
                data = &data[3..];
            } else if data.starts_with(BINAND_TYPE) {
                f.write_str("binand")?;
                data = &data[3..];
            } else if data.starts_with(PUSHINT8_TYPE) {
                if data.len() < 4 {
                    f.write_str(&hex::encode(data))?;
                    break;
                }
                f.write_fmt(format_args!("pushint8({})", i8::from_le_bytes([data[3]])))?;
                data = &data[4..];
            } else if data.starts_with(PUSHDOUBLE_TYPE) {
                if data.len() < 11 {
                    f.write_str(&hex::encode(data))?;
                    break;
                }
                f.write_fmt(format_args!(
                    "pushdouble({})",
                    f64::from_le_bytes([
                        data[3], data[4], data[5], data[6], data[7], data[8], data[9], data[10]
                    ])
                ))?;
                data = &data[11..];
            } else if data.starts_with(PUSHSCALARVAR_VAR_TYPE) {
                if data.len() < 6 {
                    f.write_str(&hex::encode(data))?;
                    break;
                }
                f.write_fmt(format_args!(
                    "pushscalarvar(var[{}])",
                    u16::from_le_bytes([data[4], data[5]])
                ))?;
                data = &data[6..];
            } else if data.starts_with(PUSHSCALARVAR_STR_TYPE) {
                if data.len() < 6 {
                    f.write_str(&hex::encode(data))?;
                    break;
                }
                f.write_fmt(format_args!(
                    "pushscalarvar(str[{}])",
                    u16::from_le_bytes([data[4], data[5]])
                ))?;
                data = &data[6..];
            } else if data.starts_with(PUSHINT32_TYPE) {
                if data.len() < 7 {
                    f.write_str(&hex::encode(data))?;
                    break;
                }
                f.write_fmt(format_args!(
                    "pushint32({})",
                    i32::from_le_bytes([data[3], data[4], data[5], data[6]])
                ))?;
                data = &data[7..];
            } else if data.starts_with(PUSHINT64_TYPE) {
                if data.len() < 11 {
                    f.write_str(&hex::encode(data))?;
                    break;
                }
                f.write_fmt(format_args!(
                    "pushint64({})",
                    i64::from_le_bytes([
                        data[3], data[4], data[5], data[6], data[7], data[8], data[9], data[10]
                    ])
                ))?;
                data = &data[11..];
            } else if data.starts_with(PUSHSTRING_TYPE) {
                if data.len() < 3 {
                    f.write_str(&hex::encode(data))?;
                    break;
                }
                let len = u16::from_le_bytes([data[1], data[2]]) as usize;
                if data.len() < 3 + len {
                    f.write_str(&hex::encode(data))?;
                    break;
                }
                if let Ok(s) = decode_to_string(self.1, &data[3..3 + len], true) {
                    f.write_str(&s)?;
                } else {
                    f.write_str(&hex::encode(&data[..3 + len]))?;
                }
                data = &data[3 + len..];
            } else if data.starts_with(BINOR_TYPE) {
                f.write_str("binor")?;
                data = &data[3..];
            } else if data.starts_with(CHANGESIGN_TYPE) {
                f.write_str("changesign")?;
                data = &data[3..];
            } else if data.starts_with(LE_TYPE) {
                f.write_str("le")?;
                data = &data[3..];
            } else if data.starts_with(PREPAREVARINDEXATION_VAR_TYPE) {
                if data.len() < 6 {
                    f.write_str(&hex::encode(data))?;
                    break;
                }
                f.write_fmt(format_args!(
                    "preparevarindexation(var[{}])",
                    u16::from_le_bytes([data[4], data[5]])
                ))?;
                data = &data[6..];
            } else if data.starts_with(PREPAREVARINDEXATION_STR_TYPE) {
                if data.len() < 6 {
                    f.write_str(&hex::encode(data))?;
                    break;
                }
                f.write_fmt(format_args!(
                    "preparevarindexation(str[{}])",
                    u16::from_le_bytes([data[4], data[5]])
                ))?;
                data = &data[6..];
            } else if data.starts_with(PUSHINT16_TYPE) {
                if data.len() < 5 {
                    f.write_str(&hex::encode(data))?;
                    break;
                }
                f.write_fmt(format_args!(
                    "pushint16({})",
                    i16::from_le_bytes([data[3], data[4]])
                ))?;
                data = &data[5..];
            } else if data.starts_with(GE_TYPE) {
                f.write_str("ge")?;
                data = &data[3..];
            } else if data.starts_with(BINXOR_TYPE) {
                f.write_str("binxor")?;
                data = &data[3..];
            } else if data.starts_with(TONUMBER_TYPE) {
                f.write_str("tonumber")?;
                data = &data[3..];
            } else if data.starts_with(TOSTRING_TYPE) {
                f.write_str("tostring")?;
                data = &data[3..];
            } else if data.starts_with(PUSHARRAYVAR_VAR_TYPE) {
                if data.len() < 6 {
                    f.write_str(&hex::encode(data))?;
                    break;
                }
                f.write_fmt(format_args!(
                    "pusharrayvar(var[{}])",
                    u16::from_le_bytes([data[4], data[5]])
                ))?;
                data = &data[6..];
            } else if data.starts_with(PUSHARRAYVAR_STR_TYPE) {
                if data.len() < 6 {
                    f.write_str(&hex::encode(data))?;
                    break;
                }
                f.write_fmt(format_args!(
                    "pusharrayvar(str[{}])",
                    u16::from_le_bytes([data[4], data[5]])
                ))?;
                data = &data[6..];
            } else if data.starts_with(LOGOR_TYPE) {
                f.write_str("logor")?;
                data = &data[3..];
            } else {
                if is_first {
                    if !data.contains(&0) && let Ok(s) = decode_to_string(self.1, &data, true) {
                        f.write_str(&s)?;
                        break;
                    }
                }
                f.write_str(&hex::encode(data))?;
                break;
            }
        }
        Ok(())
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
    labels: BTreeMap<u32, Label>,
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
            if let Some(archive) = &archive {
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
        let labels = match Self::try_load_yslb(filename, &archive, config, encoding) {
            Ok(labels) => labels,
            Err(e) => {
                eprintln!("WARNING: Failed to load ysl.bin file: {}", e);
                crate::COUNTER.inc_warning();
                BTreeMap::new()
            }
        };
        Ok(Self {
            data,
            com,
            xor_key,
            disasm: config.yuris_ystb_disasm,
            custom_yaml: config.custom_yaml,
            labels,
        })
    }

    fn try_load_yslb(
        filename: &str,
        archive: &Option<&Box<dyn Script>>,
        config: &ExtraConfig,
        encoding: Encoding,
    ) -> Result<BTreeMap<u32, Label>> {
        let yslb = if let Some(path) = config.yuris_ysl_path.as_ref() {
            crate::utils::files::read_file(path)?
        } else {
            let path = std::path::Path::new(filename);
            let pdir = path.parent().unwrap_or_else(|| std::path::Path::new(""));
            let fp = pdir.join("ysl.ybn");
            if let Some(archive) = &archive {
                let mut file = archive.open_file_by_name(&fp.to_string_lossy(), true)?;
                file.data()?
            } else {
                let p = crate::utils::files::get_ignorecase_path(&fp)?;
                crate::utils::files::read_file(&p)?
            }
        };
        if !yslb.starts_with(b"YSLB") {
            anyhow::bail!("Unsupported YSLB file. (ysl.ybn)");
        }
        let mut reader = MemReader::new(yslb);
        reader.pos = 4;
        let labels = YSLBData::unpack(&mut reader, false, encoding, &None)?;
        let path = std::path::Path::new(filename);
        let filename = path
            .file_stem()
            .ok_or_else(|| anyhow::anyhow!("No filename"))?
            .to_string_lossy()
            .into_owned();
        let script_idx_name = String::from_iter(
            filename
                .chars()
                .rev()
                .take(5)
                .collect::<Vec<_>>()
                .iter()
                .rev(),
        );
        let script_idx: u16 = script_idx_name.parse()?;
        let mut map = BTreeMap::new();
        for label in labels.labels {
            if label.script_index == script_idx {
                map.insert(label.offset, label);
            }
        }
        Ok(map)
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

struct YSTBDataSer<'a> {
    data: &'a YSTBData,
    com: &'a YSCMData,
    labels: &'a BTreeMap<u32, Label>,
}

impl<'a> serde::Serialize for YSTBDataSer<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let h = &self.data.header;
        let mut s = serializer.serialize_struct("YSTBData", 4)?;
        s.serialize_field("version", &h.version)?;
        s.serialize_field("reserve0", &h.reserve0)?;
        s.serialize_field(
            "insts",
            &YSTBInstSliceSer {
                insts: &self.data.insts,
                com: self.com,
                labels: self.labels,
            },
        )?;
        s.serialize_field("line_numbers", &self.data.line_numbers)?;
        s.end()
    }
}

struct YSTBInstSliceSer<'a> {
    insts: &'a [YSTBInst],
    com: &'a YSCMData,
    labels: &'a BTreeMap<u32, Label>,
}

impl<'a> serde::Serialize for YSTBInstSliceSer<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeSeq;
        let mut seq = serializer.serialize_seq(Some(self.insts.len()))?;
        for (i, inst) in self.insts.iter().enumerate() {
            let offset = i as u32;
            let opcode_name = self
                .com
                .opcodes
                .get(inst.opcode as usize)
                .map(|m| m.name.as_str());
            let label = self.labels.get(&offset);
            seq.serialize_element(&YSTBInstSer {
                inst,
                opcode_name,
                label,
            })?;
        }
        seq.end()
    }
}

struct YSTBInstSer<'a> {
    inst: &'a YSTBInst,
    opcode_name: Option<&'a str>,
    label: Option<&'a Label>,
}

impl<'a> serde::Serialize for YSTBInstSer<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let nfields = 3 + self.opcode_name.is_some() as usize + self.label.is_some() as usize;
        let mut s = serializer.serialize_struct("YSTBInst", nfields)?;
        s.serialize_field("opcode", &self.inst.opcode)?;
        if let Some(name) = self.opcode_name {
            s.serialize_field("opcode_name", name)?;
        }
        s.serialize_field("unk", &self.inst.unk)?;
        s.serialize_field("args", &self.inst.args)?;
        if let Some(label) = self.label {
            s.serialize_field("label", &label.name)?;
        }
        s.end()
    }
}

impl Script for YSTB {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Json
    }

    fn is_output_supported(&self, _output: OutputScriptType) -> bool {
        true
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

    fn extract_messages(&self) -> Result<Vec<Message>> {
        let mut mes = Vec::new();
        for code in self.data.insts.iter() {
            let meta =
                self.com.opcodes.get(code.opcode as usize).ok_or_else(|| {
                    anyhow::anyhow!("Failed to find op {:x}'s metadata", code.opcode)
                })?;
            if meta.name == "WORD" {
                if code.arg_count != 1 {
                    anyhow::bail!("Bad argument count for WORD.");
                }
                let arg = &code.args[0];
                if arg.typ == 0 && arg.size > 0 {
                    let mut data = decode_to_string(arg.encoding, &arg.data, true)?;
                    let name = if data.starts_with("【")
                        && let Some(end_pos) = data.find("】")
                    {
                        let n = data[3..end_pos].to_owned();
                        data = data[end_pos + 3..].to_owned();
                        Some(n)
                    } else {
                        None
                    };
                    mes.push(Message::new(data, name));
                }
            } else if meta.name == "_"
                && meta.arguments.len() > 0
                && code.arg_count == 1
                && meta.arguments[0].data == 3
            {
                let arg = &code.args[0];
                if arg.data.starts_with(PUSHSTRING_TYPE) {
                    let len = u16::from_le_bytes([arg.data[1], arg.data[2]]);
                    if len as u32 + 3 == arg.size {
                        let data = decode_to_string(
                            arg.encoding,
                            &arg.data[4..arg.size as usize - 1],
                            true,
                        )?;
                        mes.push(Message::new(data, None));
                    }
                }
            } else if meta.name == "GOSUB" && code.arg_count >= 2 {
                let arg0 = &code.args[0];
                let name = format!("{:?}", &YSTBArgData(&arg0.data, arg0.encoding)).trim_matches('"').to_lowercase();
                if name == "es.sel.set" {
                    for arg in &code.args[1..] {
                        if arg.data.starts_with(PUSHSTRING_TYPE) {
                            let len = u16::from_le_bytes([arg.data[1], arg.data[2]]);
                            if len as u32 + 3 == arg.size {
                                let data = decode_to_string(
                                    arg.encoding,
                                    &arg.data[4..arg.size as usize - 1],
                                    true,
                                )?;
                                if !data.is_empty() {
                                    mes.push(Message::new(data, None));
                                }
                            }
                        }
                    }
                } else if name == "es.char.name" && code.arg_count >= 3 {
                    let arg = &code.args[2];
                    if arg.data.starts_with(PUSHSTRING_TYPE) {
                        let len = u16::from_le_bytes([arg.data[1], arg.data[2]]);
                        if len as u32 + 3 == arg.size {
                            let data = decode_to_string(
                                arg.encoding,
                                &arg.data[4..arg.size as usize - 1],
                                true,
                            )?;
                            mes.push(Message::new(data, None));
                        }
                    }
                }
            }
        }
        Ok(mes)
    }

    fn import_messages<'a>(
        &'a self,
        messages: Vec<Message>,
        mut file: Box<dyn WriteSeek + 'a>,
        _filename: &str,
        encoding: Encoding,
        replacement: Option<&'a ReplacementTable>,
    ) -> Result<()> {
        let mut messages_iter = messages.into_iter();

        // Build modified instruction data: (YSTBInstBase, Vec<(YSTBArgBase, Vec<u8>)>)
        let mut inst_data: Vec<(YSTBInstBase, Vec<(YSTBArgBase, Vec<u8>)>)> = Vec::new();

        for code in self.data.insts.iter() {
            let meta = self.com.opcodes.get(code.opcode as usize).ok_or_else(|| {
                anyhow::anyhow!("Failed to find op {:x}'s metadata", code.opcode)
            })?;

            // Default: copy all args as-is
            let mut new_args: Vec<(YSTBArgBase, Vec<u8>)> = code
                .args
                .iter()
                .map(|arg| (arg.base.clone(), arg.data.clone()))
                .collect();

            if meta.name == "WORD" {
                if code.arg_count == 1 {
                    let arg = &code.args[0];
                    if arg.typ == 0 && arg.size > 0 {
                        let mut msg = messages_iter.next().ok_or_else(|| {
                            anyhow::anyhow!("No more messages to import")
                        })?;
                        if let Some(table) = replacement {
                            for (from, to) in &table.map {
                                msg.message = msg.message.replace(from, to);
                            }
                            if let Some(ref name) = msg.name {
                                let mut new_name = name.clone();
                                for (from, to) in &table.map {
                                    new_name = new_name.replace(from, to);
                                }
                                msg.name = Some(new_name);
                            }
                        }
                        let mut text = msg.message;
                        if let Some(name) = msg.name {
                            text = format!("【{}】{}", name, text);
                        }
                        let encoded = encode_string(encoding, &text, true)?;
                        new_args[0].1 = encoded;
                        new_args[0].0.size = new_args[0].1.len() as u32;
                    }
                }
            } else if meta.name == "_"
                && !meta.arguments.is_empty()
                && code.arg_count == 1
                && meta.arguments[0].data == 3
            {
                let arg = &code.args[0];
                if arg.data.starts_with(PUSHSTRING_TYPE) {
                    let len = u16::from_le_bytes([arg.data[1], arg.data[2]]);
                    if len as u32 + 3 == arg.size {
                        let mut msg = messages_iter.next().ok_or_else(|| {
                            anyhow::anyhow!("No more messages to import")
                        })?;
                        if let Some(table) = replacement {
                            for (from, to) in &table.map {
                                msg.message = msg.message.replace(from, to);
                            }
                        }
                        let d = encode_string(encoding, &msg.message, true)?;
                        let mut new_data = Vec::with_capacity(3 + d.len());
                        new_data.push(b'M');
                        new_data.extend_from_slice(&(d.len() as u16).to_le_bytes());
                        new_data.extend_from_slice(&d);
                        new_args[0].1 = new_data;
                        new_args[0].0.size = new_args[0].1.len() as u32;
                    }
                }
            } else if meta.name == "GOSUB" && code.arg_count >= 2 {
                let arg0 = &code.args[0];
                let name = format!(
                    "{:?}",
                    &YSTBArgData(&arg0.data, arg0.encoding)
                )
                .trim_matches('"')
                .to_lowercase();
                if name == "es.sel.set" {
                    for arg_pair in new_args.iter_mut().skip(1) {
                        let data = &arg_pair.1;
                        if data.starts_with(PUSHSTRING_TYPE) {
                            let slen = u16::from_le_bytes([data[1], data[2]]);
                            if slen as u32 + 3 == arg_pair.0.size {
                                let mut msg = messages_iter.next().ok_or_else(|| {
                                    anyhow::anyhow!("No more messages to import")
                                })?;
                                if let Some(table) = replacement {
                                    for (from, to) in &table.map {
                                        msg.message = msg.message.replace(from, to);
                                    }
                                }
                                if !msg.message.is_empty() {
                                    let d = encode_string(encoding, &msg.message, true)?;
                                    let mut new_data = Vec::with_capacity(3 + d.len());
                                    new_data.push(b'M');
                                    new_data.extend_from_slice(
                                        &(d.len() as u16).to_le_bytes(),
                                    );
                                    new_data.extend_from_slice(&d);
                                    arg_pair.1 = new_data;
                                    arg_pair.0.size = arg_pair.1.len() as u32;
                                }
                            }
                        }
                    }
                } else if name == "es.char.name" && code.arg_count >= 3 {
                    // Re-encode arg[1] from original encoding to target encoding
                    let arg1 = &code.args[1];
                    if arg1.data.starts_with(PUSHSTRING_TYPE) {
                        let slen = u16::from_le_bytes([arg1.data[1], arg1.data[2]]);
                        if slen as u32 + 3 == arg1.size {
                            let decoded = decode_to_string(
                                arg1.encoding,
                                &arg1.data[3..arg1.size as usize - 1],
                                true,
                            )?;
                            let d = encode_string(encoding, &decoded, true)?;
                            let mut new_data = Vec::with_capacity(3 + d.len());
                            new_data.push(b'M');
                            new_data.extend_from_slice(&(d.len() as u16).to_le_bytes());
                            new_data.extend_from_slice(&d);
                            new_args[1].1 = new_data;
                            new_args[1].0.size = new_args[1].1.len() as u32;
                        }
                    }
                    // Patch arg[2] with message
                    let arg2 = &code.args[2];
                    if arg2.data.starts_with(PUSHSTRING_TYPE) {
                        let slen = u16::from_le_bytes([arg2.data[1], arg2.data[2]]);
                        if slen as u32 + 3 == arg2.size {
                            let mut msg = messages_iter.next().ok_or_else(|| {
                                anyhow::anyhow!("No more messages to import")
                            })?;
                            if let Some(table) = replacement {
                                for (from, to) in &table.map {
                                    msg.message = msg.message.replace(from, to);
                                }
                            }
                            let d = encode_string(encoding, &msg.message, true)?;
                            let mut new_data = Vec::with_capacity(3 + d.len());
                            new_data.push(b'M');
                            new_data.extend_from_slice(&(d.len() as u16).to_le_bytes());
                            new_data.extend_from_slice(&d);
                            new_args[2].1 = new_data;
                            new_args[2].0.size = new_args[2].1.len() as u32;
                        }
                    }
                }
            }

            inst_data.push((code.base.clone(), new_args));
        }

        // Write binary output (same structure as custom_import)
        let mut f = MemWriter::new();
        f.write_all(b"YSTB")?;

        let inst_entry_count = inst_data.len() as u32;
        let inst_index_size = inst_entry_count * 4;
        let arg_count: usize = inst_data.iter().map(|(_, args)| args.len()).sum();
        let args_index_size = arg_count as u32 * 0xC;

        let mut header = self.data.header.clone();
        header.inst_entry_count = inst_entry_count;
        header.inst_index_size = inst_index_size;
        header.args_index_size = args_index_size;
        header.line_numbers_size = self.data.line_numbers.len() as u32;

        // Pack header (will update args_data_size later)
        header.pack(&mut f, false, encoding, &None)?;

        // Pack instruction bases with correct arg_counts
        for (base, args) in inst_data.iter() {
            let mut b = base.clone();
            b.arg_count = args.len() as u8;
            b.pack(&mut f, false, encoding, &None)?;
        }

        // Pack arg index table and arg data
        let mut cpos = f.pos as u64;
        f.pos += args_index_size as usize;
        let bpos = f.pos as u32;

        for (base, args) in inst_data.iter_mut() {
            let meta = self.com.opcodes.get(base.opcode as usize).ok_or_else(|| {
                anyhow::anyhow!("Failed to find op {:x}'s metadata", base.opcode)
            })?;

            for arg in args.iter_mut() {
                arg.0.size = arg.1.len() as u32;
                f.write_struct_at(cpos, &arg.0, false, encoding, &None)?;
                cpos += 8;

                if arg.0.size == 0
                    || (meta.name == "RETURNCODE"
                        && arg.0.size == 1
                        && arg.1[0] == b'M')
                {
                    f.write_u32_at(cpos, 0)?;
                    cpos += 4;
                    continue;
                }

                let offset = f.pos as u32 - bpos;
                f.write_u32_at(cpos, offset)?;
                cpos += 4;
                f.write_all(&arg.1)?;
            }
        }

        // Update args_data_size and write line_numbers
        header.args_data_size = f.pos as u32 - bpos;
        f.write_all(&self.data.line_numbers)?;

        // Rewrite header with correct args_data_size
        f.pos = 4;
        header.pack(&mut f, false, encoding, &None)?;

        // Apply XOR if needed
        if let Some(xor) = self.xor_key {
            let mut r = MemReader::new(f.into_inner());
            f = MemWriter::new();
            Self::xor(&mut r, &mut f, xor)?;
        }

        file.write_all(&f.data)?;
        Ok(())
    }

    fn custom_export(&self, filename: &std::path::Path, encoding: Encoding) -> Result<()> {
        if !self.disasm {
            let wrapper = YSTBDataSer {
                data: &self.data,
                com: &self.com,
                labels: &self.labels,
            };
            let s = if self.custom_yaml {
                serde_yaml_ng::to_string(&wrapper)?
            } else {
                serde_json::to_string_pretty(&wrapper)?
            };
            let mut f = std::fs::File::create(filename)?;
            let encoded = encode_string(encoding, &s, true)?;
            f.write_all(&encoded)?;
            return Ok(());
        }
        let mut file = MemWriter::new();
        let mut indent = String::new();
        let mut unused_labels = BTreeSet::from_iter(self.labels.keys().cloned());
        for (i, code) in self.data.insts.iter().enumerate() {
            let offset = i as u32;
            let meta =
                self.com.opcodes.get(code.opcode as usize).ok_or_else(|| {
                    anyhow::anyhow!("Failed to find op {:x}'s metadata", code.opcode)
                })?;
            if let Some(lab) = self.labels.get(&offset) {
                writeln!(file, "#{}", lab.name)?;
                unused_labels.remove(&offset);
            }
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
        if !unused_labels.is_empty() {
            eprintln!("WARNING: Some labels not used: {:?}", unused_labels);
            crate::COUNTER.inc_warning();
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
