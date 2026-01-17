use crate::ext::io::*;
use crate::types::*;
use crate::utils::encoding::*;
use crate::utils::struct_pack::*;
use anyhow::Result;
use int_enum::IntEnum;
use msg_tool_macro::{StructPack, StructUnpack};
use std::io::{Read, Seek, Write};
use std::ops::{Deref, DerefMut};

#[repr(u8)]
#[derive(Debug, IntEnum, PartialEq, Eq, Clone, Copy)]
pub enum CSCompareType {
    CsctNotEqual,
    CsctEqual,
    CsctLessThan,
    CsctLessEqual,
    CsctGreaterThan,
    CsctGreaterEqual,
}

#[repr(u8)]
#[derive(Debug, IntEnum, PartialEq, Eq, Clone, Copy)]
pub enum CSInstructionCode {
    CsicNew,
    CsicFree,
    CsicLoad,
    CsicStore,
    CsicEnter,
    CsicLeave,
    CsicJump,
    CsicCJump,
    CsicCall,
    CsicReturn,
    CsicElement,
    CsicElementIndirect,
    CsicOperate,
    CsicUniOperate,
    CsicCompare,
}

#[repr(u8)]
#[derive(Debug, IntEnum, PartialEq, Eq, Clone, Copy)]
pub enum CSObjectMode {
    CsomImmediate,
    CsomStack,
    CsomThis,
    CsomGlobal,
    CsomData,
    CsomAuto,
}

#[repr(u8)]
#[derive(Debug, IntEnum, PartialEq, Eq, Clone, Copy)]
pub enum CSOperatorType {
    CsotNop = 0xFF,
    CsotAdd = 0,
    CsotSub,
    CsotMul,
    CsotDiv,
    CsotMod,
    CsotAnd,
    CsotOr,
    CsotXor,
    CsotLogicalAnd,
    CsotLogicalOr,
}

#[repr(u8)]
#[derive(Debug, IntEnum, PartialEq, Eq, Clone, Copy)]
pub enum CSUnaryOperatorType {
    CsuotPlus,
    CsuotNegate,
    CsuotBitnot,
    CsuotLogicalNot,
}

#[repr(u8)]
#[derive(Debug, IntEnum, PartialEq, Eq, Clone, Copy)]
pub enum CSVariableType {
    CsvtObject,
    CsvtReference,
    CsvtArray,
    CsvtHash,
    CsvtInteger,
    CsvtReal,
    CsvtString,
    CsvtInteger64,
    CsvtPointer,
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct EMCFileHeader {
    pub signagure: [u8; 8],
    pub file_id: u32,
    pub _reserved: u32,
    pub format_desc: [u8; 48],
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct EXIHeader {
    #[skip_unpack_if(reader.stream_length()? < 4)]
    pub version: u32,
    #[skip_unpack_if(reader.stream_length()? < 8)]
    pub int_base: u32,
}

#[derive(Clone, Debug, Default, StructPack, StructUnpack)]
pub struct DWordArray {
    #[pvec(u32)]
    pub data: Vec<u32>,
}

impl Deref for DWordArray {
    type Target = Vec<u32>;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl DerefMut for DWordArray {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

#[derive(Clone, Debug)]
pub struct WideString(pub String);

impl StructUnpack for WideString {
    fn unpack<R: Read + Seek>(reader: &mut R, big: bool, encoding: Encoding) -> Result<Self> {
        let length = u32::unpack(reader, big, encoding)? as usize;
        let to_read = length * 2;
        let buf = reader.read_exact_vec(to_read)?;
        let enc = if big {
            Encoding::Utf16BE
        } else {
            Encoding::Utf16LE
        };
        let s = decode_to_string(enc, &buf, true)?;
        Ok(Self(s))
    }
}

impl StructPack for WideString {
    fn pack<W: Write>(&self, writer: &mut W, big: bool, encoding: Encoding) -> Result<()> {
        let enc = if big {
            Encoding::Utf16BE
        } else {
            Encoding::Utf16LE
        };
        let encoded = encode_string(enc, &self.0, false)?;
        let length = (encoded.len() / 2) as u32;
        length.pack(writer, big, encoding)?;
        writer.write_all(&encoded)?;
        Ok(())
    }
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct FunctionNameItem {
    pub addr: u32,
    pub name: WideString,
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct FunctionNameList {
    #[pvec(u32)]
    pub items: Vec<FunctionNameItem>,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum ECSObject {
    Integer(i64),
    Real(f64),
    String(String),
    Array(Vec<ECSObject>),
    ClassInfoObject(String),
    Hash,
    Reference,
    Global(ECSGlobal),
}

impl ECSObject {
    pub fn read_from<R: Read + Seek>(reader: &mut R, int64: bool) -> Result<Self> {
        let typ = reader.read_u32()?;
        let obj_typ = CSVariableType::try_from(typ as u8)
            .map_err(|typ| anyhow::anyhow!("Invalid CSVariableType: {}", typ))?;
        match obj_typ {
            CSVariableType::CsvtObject => {
                let class_name = WideString::unpack(reader, false, Encoding::Utf8)?.0;
                return Ok(ECSObject::ClassInfoObject(class_name));
            }
            CSVariableType::CsvtReference => {
                return Ok(ECSObject::Reference);
            }
            CSVariableType::CsvtArray => {
                let count = reader.read_u32()? as usize;
                let mut items = Vec::with_capacity(count);
                for _ in 0..count {
                    let item = ECSObject::read_from(reader, int64)?;
                    items.push(item);
                }
                return Ok(ECSObject::Array(items));
            }
            CSVariableType::CsvtHash => {
                return Ok(ECSObject::Hash);
            }
            CSVariableType::CsvtInteger => {
                if int64 {
                    let val = reader.read_i64()?;
                    return Ok(ECSObject::Integer(val));
                } else {
                    let val = reader.read_i32()? as i64;
                    return Ok(ECSObject::Integer(val));
                }
            }
            CSVariableType::CsvtReal => {
                let val = reader.read_f64()?;
                return Ok(ECSObject::Real(val));
            }
            CSVariableType::CsvtString => {
                let s = WideString::unpack(reader, false, Encoding::Utf8)?.0;
                return Ok(ECSObject::String(s));
            }
            _ => {
                return Err(anyhow::anyhow!("Unsupported CSVariableType: {:?}", obj_typ));
            }
        }
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct ECSObjectItem {
    pub name: String,
    pub obj: ECSObject,
}

#[derive(Clone, Debug)]
pub struct ECSGlobal(pub Vec<ECSObjectItem>);

impl Deref for ECSGlobal {
    type Target = Vec<ECSObjectItem>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ECSGlobal {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct TaggedRefAddress {
    pub tag: WideString,
    pub refs: DWordArray,
}

#[derive(Clone, Debug, Default, StructPack, StructUnpack)]
pub struct TaggedRefAddressList {
    #[pvec(u32)]
    pub items: Vec<TaggedRefAddress>,
}

impl Deref for TaggedRefAddressList {
    type Target = Vec<TaggedRefAddress>;

    fn deref(&self) -> &Self::Target {
        &self.items
    }
}

impl DerefMut for TaggedRefAddressList {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.items
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct ECSExecutionImageCommandRecord {
    pub code: CSInstructionCode,
    pub addr: u32,
    pub size: u32,
    pub new_addr: u32,
}

#[derive(Clone, Debug)]
pub struct ECSExecutionImageAssembly {
    pub command_list: Vec<ECSExecutionImageCommandRecord>,
}

impl Deref for ECSExecutionImageAssembly {
    type Target = Vec<ECSExecutionImageCommandRecord>;

    fn deref(&self) -> &Self::Target {
        &self.command_list
    }
}

impl DerefMut for ECSExecutionImageAssembly {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.command_list
    }
}

#[test]
fn test_exi_header_unpack() {
    let data = b"\x01\x00\x00\x00";
    let mut cursor = MemReaderRef::new(data);
    let header = EXIHeader::unpack(&mut cursor, false, Encoding::Utf8).unwrap();
    assert_eq!(header.version, 1);
    assert_eq!(header.int_base, 0);
}
