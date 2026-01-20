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
    CsctNotEqualPointer,
    CsctEqualPointer,
}

pub use CSCompareType::*;

#[repr(u8)]
#[derive(Debug, IntEnum, PartialEq, Eq, Clone, Copy)]
pub enum CSExtraOperatorType {
    CsxotArrayDim,
    CsxotHashContainer,
    CsxotMoveReference,
}

pub use CSExtraOperatorType::*;

#[repr(u8)]
#[derive(Debug, IntEnum, PartialEq, Eq, Clone, Copy)]
pub enum CSExtraUniOperatorType {
    CsxuotDeselect,
    CsxuotBoolean,
    CsxuotSizeOf,
    CsxuotTypeOf,
    CsxuotStaticCast,
    CsxuotDynamicCast,
    CsxuotDuplicate,
    CsxuotDelete,
    CsxuotDeleteArray,
    CsxuotLoadAddress,
    CsxuotRefAddress,
}

pub use CSExtraUniOperatorType::*;

#[repr(u8)]
#[derive(Debug, IntEnum, PartialEq, Eq, Clone, Copy)]
pub enum CSInstructionCode {
    CsicNew = 0,
    CsicFree = 1,
    CsicLoad = 2,
    CsicStore = 3,
    CsicEnter = 4,
    CsicLeave = 5,
    CsicJump = 6,
    CsicCJump = 7,
    CsicCall = 8,
    CsicReturn = 9,
    CsicElement = 10,
    CsicElementIndirect = 11,
    CsicOperate = 12,
    CsicUniOperate = 13,
    CsicCompare = 14,
    CsicExOperate = 15,
    CsicExUniOperate = 16,
    CsicExCall = 17,
    CsicExReturn = 18,
    CsicCallMember = 19,
    CsicCallNativeMember = 20,
    CsicSwap = 21,
    CsicCreateBufferVSize = 23,
    CsicPointerToObject = 24,
    CsicReferenceForPointer = 26,
    CsicCallNativeFunction = 29,
    // Shell
    CodeLoadMem = 0x80,
    CodeLoadMemBaseImm32,
    CodeLoadMemBaseIndex,
    CodeLoadMemBaseIndexImm32,
    CodeStoreMem = 0x84,
    CodeStoreMemBaseImm32,
    CodeStoreMemBaseIndex,
    CodeStoreMemBaseIndexImm32,
    CodeLoadLocal = 0x88,
    CodeLoadLocalIndexImm32,
    CodeStoreLocal = 0x8A,
    CodeStoreLocalIndexImm32,
    CodeMoveReg = 0x90,
    CodeCvtFloat2Int = 0x92,
    CodeCvtInt2Float = 0x93,
    CodeSrlImm8 = 0x94,
    CodeSraImm8,
    CodeSllImm8,
    CodeMaskMove,
    CodeAddImm32 = 0x98,
    CodeMulImm32,
    CodeAddSPImm32 = 0x9A,
    CodeLoadImm64 = 0x9B,
    CodeNegInt = 0x9C,
    CodeNotInt,
    CodeNegFloat,
    CodeAddReg = 0xA0,
    CodeSubReg,
    CodeMulReg,
    CodeDivReg,
    CodeModReg,
    CodeAndReg,
    CodeOrReg,
    CodeXorReg,
    CodeSrlReg,
    CodeSraReg,
    CodeSllReg,
    CodeMoveSx32Reg = 0xAB,
    CodeMoveSx16Reg,
    CodeMoveSx8Reg,
    CodeFAddReg = 0xB0,
    CodeFSubReg,
    CodeFMulReg,
    CodeFDivReg,
    CodeMul32Reg = 0xB8,
    CodeIMul32Reg,
    CodeDiv32Reg,
    CodeIDiv32Reg,
    CodeMod32Reg,
    CodeIMod32Reg,
    CodeCmpNeReg = 0xC0,
    CodeCmpEqReg,
    CodeCmpLtReg,
    CodeCmpLeReg,
    CodeCmpGtReg,
    CodeCmpGeReg,
    CodeCmpCReg,
    CodeCmpCZReg,
    CodeFCmpNeReg = 0xC8,
    CodeFCmpEqReg,
    CodeFCmpLtReg,
    CodeFCmpLeReg,
    CodeFCmpGtReg,
    CodeFCmpGeReg,
    CodeJumpOffset32 = 0xD0,
    CodeJumpReg = 0xD1,
    CodeCNJumpOffset32 = 0xD2,
    CodeCJumpOffset32,
    CodeCallImm32 = 0xD4,
    CodeCallReg = 0xD5,
    CodeSysCallImm32 = 0xD6,
    CodeSysCallReg = 0xD7,
    CodeReturn = 0xD8,
    CodePushReg = 0xDC,
    CodePopReg,
    CodePushRegs,
    CodePopRegs,
    CodeMemoryHint = 0xE0,
    CodeFloatExtension,
    CodeSIMD64Extension2Op,
    CodeSIMD64Extension3Op,
    CodeSIMD128Extension2Op,
    CodeSIMD128Extension3Op,
    CodeEscape = 0xFD,
    CodeNoOperation = 0xFE,
    CodeSystemReserved = 0xFF,
}

pub use CSInstructionCode::*;
#[allow(unused_imports)]
pub use CodeLoadLocal as CodeLoadLocalImm32;
#[allow(unused_imports)]
pub use CodeLoadMem as CodeLoadMemBase;
#[allow(unused_imports)]
pub use CodeStoreLocal as CodeStoreLocalImm32;
#[allow(unused_imports)]
pub use CodeStoreMem as CodeStoreMemBase;

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

pub use CSObjectMode::*;

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
    CsotShiftRight,
    CsotShiftLeft,
}

pub use CSOperatorType::*;

#[repr(u8)]
#[derive(Debug, IntEnum, PartialEq, Eq, Clone, Copy)]
pub enum CSUnaryOperatorType {
    CsuotPlus,
    CsuotNegate,
    CsuotBitnot,
    CsuotLogicalNot,
}

pub use CSUnaryOperatorType::*;

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
    CsvtClassObject,
    CsvtBoolean,
    CsvtInt8,
    CsvtUint8,
    CsvtInt16,
    CsvtUint16,
    CsvtInt32,
    CsvtUint32,
    CsvtArrayDimension,
    CsvtHashContainer,
    CsvtReal32,
    CsvtReal64,
    CsvtPointerReference,
    CsvtBuffer,
    CsvtFunction,
}

pub use CSVariableType::*;

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct FileHeader {
    pub signagure: [u8; 8],
    pub file_id: u32,
    pub _reserved: u32,
    pub format_desc: [u8; 48],
}

#[derive(Clone, Debug, msg_tool_macro::Default)]
pub struct SectionHeader {
    #[default(3)]
    pub full_ver: u32,
    pub header_size: u32,
    pub version: u32,
    pub int_base: u32,
    pub container_flags: u32,
    pub _reserved: u32,
    pub stack_size: u32,
    pub heap_size: u32,
    pub entry_point: u32,
    pub static_initialize: u32,
    pub resume_prepare: u32,
}

impl StructUnpack for SectionHeader {
    fn unpack<R: Read + Seek>(
        reader: &mut R,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn std::any::Any>>,
    ) -> Result<Self> {
        let full_ver = 3;
        let header_size = reader.stream_length()? as u32;
        let version = if header_size >= 4 {
            u32::unpack(reader, big, encoding, info)?
        } else {
            0
        };
        let int_base = if header_size >= 8 {
            u32::unpack(reader, big, encoding, info)?
        } else {
            0
        };
        let container_flags = if header_size >= 12 {
            u32::unpack(reader, big, encoding, info)?
        } else {
            0
        };
        let _reserved = if header_size >= 16 {
            u32::unpack(reader, big, encoding, info)?
        } else {
            0
        };
        let stack_size = if header_size >= 20 {
            u32::unpack(reader, big, encoding, info)?
        } else {
            0
        };
        let heap_size = if header_size >= 24 {
            u32::unpack(reader, big, encoding, info)?
        } else {
            0
        };
        let entry_point = if header_size >= 28 {
            u32::unpack(reader, big, encoding, info)?
        } else {
            0
        };
        let static_initialize = if header_size >= 32 {
            u32::unpack(reader, big, encoding, info)?
        } else {
            0
        };
        let resume_prepare = if header_size >= 36 {
            u32::unpack(reader, big, encoding, info)?
        } else {
            0
        };
        Ok(Self {
            full_ver,
            header_size,
            version,
            int_base,
            container_flags,
            _reserved,
            stack_size,
            heap_size,
            entry_point,
            static_initialize,
            resume_prepare,
        })
    }
}

impl StructPack for SectionHeader {
    fn pack<W: Write>(
        &self,
        writer: &mut W,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn std::any::Any>>,
    ) -> Result<()> {
        if self.header_size >= 4 {
            self.version.pack(writer, big, encoding, info)?;
        }
        if self.header_size >= 8 {
            self.int_base.pack(writer, big, encoding, info)?;
        }
        if self.header_size >= 12 {
            self.container_flags.pack(writer, big, encoding, info)?;
        }
        if self.header_size >= 16 {
            self._reserved.pack(writer, big, encoding, info)?;
        }
        if self.header_size >= 20 {
            self.stack_size.pack(writer, big, encoding, info)?;
        }
        if self.header_size >= 24 {
            self.heap_size.pack(writer, big, encoding, info)?;
        }
        if self.header_size >= 28 {
            self.entry_point.pack(writer, big, encoding, info)?;
        }
        if self.header_size >= 32 {
            self.static_initialize.pack(writer, big, encoding, info)?;
        }
        if self.header_size >= 36 {
            self.resume_prepare.pack(writer, big, encoding, info)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct WideString(pub String);

impl StructUnpack for WideString {
    fn unpack<R: Read + Seek>(
        reader: &mut R,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn std::any::Any>>,
    ) -> Result<Self> {
        let length = u32::unpack(reader, big, encoding, info)? as usize;
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
    fn pack<W: Write>(
        &self,
        writer: &mut W,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn std::any::Any>>,
    ) -> Result<()> {
        let enc = if big {
            Encoding::Utf16BE
        } else {
            Encoding::Utf16LE
        };
        let encoded = encode_string(enc, &self.0, false)?;
        let length = (encoded.len() / 2) as u32;
        length.pack(writer, big, encoding, info)?;
        writer.write_all(&encoded)?;
        Ok(())
    }
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct BaseClassInfoEntry {
    pub flags: u32,
    pub name: WideString,
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct ECSCastInterface {
    pub cast_object: i32,
    pub var_offset: i32,
    pub var_bounds: i32,
    pub func_offset: i32,
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct BaseClassCastInfoEntry {
    pub name: WideString,
    pub pci: ECSCastInterface,
    pub flags: u32,
}

fn get_version(info: &Option<Box<dyn std::any::Any>>) -> Result<u32> {
    if let Some(boxed) = info {
        if let Some(version) = boxed.downcast_ref::<SectionHeader>() {
            return Ok(version.version);
        }
    }
    Err(anyhow::anyhow!(
        "SectionHeader info not provided for version retrieval"
    ))
}

fn get_int_base(info: &Option<Box<dyn std::any::Any>>) -> Result<u32> {
    if let Some(boxed) = info {
        if let Some(header) = boxed.downcast_ref::<SectionHeader>() {
            return Ok(header.int_base);
        }
    }
    Err(anyhow::anyhow!(
        "SectionHeader info not provided for int_base retrieval"
    ))
}

fn get_full_ver(info: &Option<Box<dyn std::any::Any>>) -> Result<u32> {
    if let Some(boxed) = info {
        if let Some(header) = boxed.downcast_ref::<SectionHeader>() {
            return Ok(header.full_ver);
        }
    }
    Err(anyhow::anyhow!(
        "SectionHeader info not provided for full_ver retrieval"
    ))
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct ClassInfoObject {
    pub class_name: WideString,
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct ArrayObject {
    #[pvec(u32)]
    pub elements: Vec<TypedObject>,
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct Integer64Object {
    mask: i64,
    value: i64,
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct PointerObject {
    ref_type: i32,
    read_only: u8,
    ref_type_object: TypedObject,
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct ArrayDimensionObject {
    element_type_object: TypedObject,
    #[pvec(u32)]
    bounds: Vec<i32>,
    #[pvec(u32)]
    elements: Vec<TypedObject>,
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct HashContainerObject {
    element_type_object: TypedObject,
}

#[derive(Clone, Debug)]
pub enum TypedObject {
    Invalid,
    Object(ClassInfoObject),
    ReferenceV1(Box<TypedObject>),
    Reference,
    Array(ArrayObject),
    Hash,
    Integer(i64),
    Real(f64),
    String(WideString),
    Integer64V3(Integer64Object),
    Integer64(i64),
    PointerV3(Box<PointerObject>),
    Pointer,
    Boolean(i64),
    Int8(i64),
    Uint8(i64),
    Int16(i64),
    Uint16(i64),
    Int32(i64),
    Uint32(i64),
    ArrayDimension(Box<ArrayDimensionObject>),
    HashContainer(Box<HashContainerObject>),
    Real32(f64),
    Real64(f64),
}

impl StructUnpack for TypedObject {
    fn unpack<R: Read + Seek>(
        reader: &mut R,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn std::any::Any>>,
    ) -> Result<Self> {
        let typ = i32::unpack(reader, big, encoding, info)?;
        if typ == -1 {
            return Ok(TypedObject::Invalid);
        }
        let typ = CSVariableType::try_from(typ as u8)
            .map_err(|_| anyhow::anyhow!("Invalid CSVariableType value: {}", typ))?;
        match typ {
            CsvtObject => {
                let obj = ClassInfoObject::unpack(reader, big, encoding, info)?;
                Ok(TypedObject::Object(obj))
            }
            CsvtReference => {
                if get_version(info)? == 1 {
                    let inner = TypedObject::unpack(reader, big, encoding, info)?;
                    Ok(TypedObject::ReferenceV1(Box::new(inner)))
                } else {
                    Ok(TypedObject::Reference)
                }
            }
            CsvtArray => {
                let arr = ArrayObject::unpack(reader, big, encoding, info)?;
                Ok(TypedObject::Array(arr))
            }
            CsvtHash => Ok(TypedObject::Hash),
            CsvtInteger => {
                let value = if get_int_base(info)? == 64 {
                    i64::unpack(reader, big, encoding, info)?
                } else {
                    i32::unpack(reader, big, encoding, info)? as i64
                };
                Ok(TypedObject::Integer(value))
            }
            CsvtReal => {
                let value = f64::unpack(reader, big, encoding, info)?;
                Ok(TypedObject::Real(value))
            }
            CsvtString => {
                let s = WideString::unpack(reader, big, encoding, info)?;
                Ok(TypedObject::String(s))
            }
            CsvtInteger64 => {
                if get_full_ver(info)? == 3 {
                    let obj = Integer64Object::unpack(reader, big, encoding, info)?;
                    Ok(TypedObject::Integer64V3(obj))
                } else {
                    let value = i64::unpack(reader, big, encoding, info)?;
                    Ok(TypedObject::Integer64(value))
                }
            }
            CsvtPointer => {
                if get_full_ver(info)? == 3 {
                    let obj = PointerObject::unpack(reader, big, encoding, info)?;
                    Ok(TypedObject::PointerV3(Box::new(obj)))
                } else {
                    Ok(TypedObject::Pointer)
                }
            }
            CsvtBoolean => {
                let value = i64::unpack(reader, big, encoding, info)?;
                Ok(TypedObject::Boolean(value))
            }
            CsvtInt8 => {
                let value = i64::unpack(reader, big, encoding, info)?;
                Ok(TypedObject::Int8(value))
            }
            CsvtUint8 => {
                let value = i64::unpack(reader, big, encoding, info)?;
                Ok(TypedObject::Uint8(value))
            }
            CsvtInt16 => {
                let value = i64::unpack(reader, big, encoding, info)?;
                Ok(TypedObject::Int16(value))
            }
            CsvtUint16 => {
                let value = i64::unpack(reader, big, encoding, info)?;
                Ok(TypedObject::Uint16(value))
            }
            CsvtInt32 => {
                let value = i64::unpack(reader, big, encoding, info)?;
                Ok(TypedObject::Int32(value))
            }
            CsvtUint32 => {
                let value = i64::unpack(reader, big, encoding, info)?;
                Ok(TypedObject::Uint32(value))
            }
            CsvtArrayDimension => {
                let obj = ArrayDimensionObject::unpack(reader, big, encoding, info)?;
                Ok(TypedObject::ArrayDimension(Box::new(obj)))
            }
            CsvtHashContainer => {
                let obj = HashContainerObject::unpack(reader, big, encoding, info)?;
                Ok(TypedObject::HashContainer(Box::new(obj)))
            }
            CsvtReal32 => {
                let value = f64::unpack(reader, big, encoding, info)?;
                Ok(TypedObject::Real32(value))
            }
            CsvtReal64 => {
                let value = f64::unpack(reader, big, encoding, info)?;
                Ok(TypedObject::Real64(value))
            }
            _ => Err(anyhow::anyhow!(
                "TypedObject unpack for type {:?} not implemented",
                typ
            )),
        }
    }
}

impl StructPack for TypedObject {
    fn pack<W: Write>(
        &self,
        writer: &mut W,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn std::any::Any>>,
    ) -> Result<()> {
        match self {
            TypedObject::Invalid => {
                (-1i32).pack(writer, big, encoding, info)?;
            }
            TypedObject::Object(o) => {
                (CsvtObject as i32).pack(writer, big, encoding, info)?;
                o.pack(writer, big, encoding, info)?;
            }
            TypedObject::ReferenceV1(inner) => {
                (CsvtReference as i32).pack(writer, big, encoding, info)?;
                inner.pack(writer, big, encoding, info)?;
            }
            TypedObject::Reference => {
                (CsvtReference as i32).pack(writer, big, encoding, info)?;
            }
            TypedObject::Array(arr) => {
                (CsvtArray as i32).pack(writer, big, encoding, info)?;
                arr.pack(writer, big, encoding, info)?;
            }
            TypedObject::Hash => {
                (CsvtHash as i32).pack(writer, big, encoding, info)?;
            }
            TypedObject::Integer(value) => {
                (CsvtInteger as i32).pack(writer, big, encoding, info)?;
                if get_int_base(info)? == 64 {
                    value.pack(writer, big, encoding, info)?;
                } else {
                    (*value as i32).pack(writer, big, encoding, info)?;
                }
            }
            TypedObject::Real(value) => {
                (CsvtReal as i32).pack(writer, big, encoding, info)?;
                value.pack(writer, big, encoding, info)?;
            }
            TypedObject::String(s) => {
                (CsvtString as i32).pack(writer, big, encoding, info)?;
                s.pack(writer, big, encoding, info)?;
            }
            TypedObject::Integer64V3(obj) => {
                (CsvtInteger64 as i32).pack(writer, big, encoding, info)?;
                obj.pack(writer, big, encoding, info)?;
            }
            TypedObject::Integer64(value) => {
                (CsvtInteger64 as i32).pack(writer, big, encoding, info)?;
                value.pack(writer, big, encoding, info)?;
            }
            TypedObject::PointerV3(obj) => {
                (CsvtPointer as i32).pack(writer, big, encoding, info)?;
                obj.pack(writer, big, encoding, info)?;
            }
            TypedObject::Pointer => {
                (CsvtPointer as i32).pack(writer, big, encoding, info)?;
            }
            TypedObject::Boolean(value) => {
                (CsvtBoolean as i32).pack(writer, big, encoding, info)?;
                value.pack(writer, big, encoding, info)?;
            }
            TypedObject::Int8(value) => {
                (CsvtInt8 as i32).pack(writer, big, encoding, info)?;
                value.pack(writer, big, encoding, info)?;
            }
            TypedObject::Uint8(value) => {
                (CsvtUint8 as i32).pack(writer, big, encoding, info)?;
                value.pack(writer, big, encoding, info)?;
            }
            TypedObject::Int16(value) => {
                (CsvtInt16 as i32).pack(writer, big, encoding, info)?;
                value.pack(writer, big, encoding, info)?;
            }
            TypedObject::Uint16(value) => {
                (CsvtUint16 as i32).pack(writer, big, encoding, info)?;
                value.pack(writer, big, encoding, info)?;
            }
            TypedObject::Int32(value) => {
                (CsvtInt32 as i32).pack(writer, big, encoding, info)?;
                value.pack(writer, big, encoding, info)?;
            }
            TypedObject::Uint32(value) => {
                (CsvtUint32 as i32).pack(writer, big, encoding, info)?;
                value.pack(writer, big, encoding, info)?;
            }
            TypedObject::ArrayDimension(obj) => {
                (CsvtArrayDimension as i32).pack(writer, big, encoding, info)?;
                obj.pack(writer, big, encoding, info)?;
            }
            TypedObject::HashContainer(obj) => {
                (CsvtHashContainer as i32).pack(writer, big, encoding, info)?;
                obj.pack(writer, big, encoding, info)?;
            }
            TypedObject::Real32(value) => {
                (CsvtReal32 as i32).pack(writer, big, encoding, info)?;
                value.pack(writer, big, encoding, info)?;
            }
            TypedObject::Real64(value) => {
                (CsvtReal64 as i32).pack(writer, big, encoding, info)?;
                value.pack(writer, big, encoding, info)?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct FieldInfoEntry {
    pub name: WideString,
    pub flags: u32,
    pub type_object: TypedObject,
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct TypeInfoEntry {
    pub flags: u32,
    pub type_object: TypedObject,
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct PrototypeInfoEntry {
    pub flags: u32,
    pub name: WideString,
    pub global_name: WideString,
    pub return_type: TypeInfoEntry,
    #[pvec(u32)]
    pub arguments: Vec<TypeInfoEntry>,
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct MethodInfoEntry {
    pub prototype_info: PrototypeInfoEntry,
    pub func_class: WideString,
    pub pointer_data: [u8; 40],
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct ClassInfoEntry {
    pub flags: u32,
    pub name: WideString,
    pub global_name: WideString,
    #[pvec(u32)]
    pub base_class_info: Vec<BaseClassInfoEntry>,
    #[pvec(u32)]
    pub base_class_cast_info: Vec<BaseClassCastInfoEntry>,
    #[pvec(u32)]
    pub field_info: Vec<FieldInfoEntry>,
    #[pvec(u32)]
    pub method_info: Vec<MethodInfoEntry>,
    #[pvec(u32)]
    pub extra_data: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct SectionClassInfo {
    pub names: Vec<WideString>,
    pub infos: Vec<ClassInfoEntry>,
}

impl StructUnpack for SectionClassInfo {
    fn unpack<R: Read + Seek>(
        reader: &mut R,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn std::any::Any>>,
    ) -> Result<Self> {
        let count = u32::unpack(reader, big, encoding, info)? as usize;
        let mut names = Vec::with_capacity(count);
        for _ in 0..count {
            let name = WideString::unpack(reader, big, encoding, info)?;
            names.push(name);
        }
        let mut infos = Vec::with_capacity(count);
        for _ in 0..count {
            let class_info = ClassInfoEntry::unpack(reader, big, encoding, info)?;
            infos.push(class_info);
        }
        Ok(SectionClassInfo { names, infos })
    }
}

impl StructPack for SectionClassInfo {
    fn pack<W: Write>(
        &self,
        writer: &mut W,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn std::any::Any>>,
    ) -> Result<()> {
        let count = self.names.len() as u32;
        if count != self.infos.len() as u32 {
            return Err(anyhow::anyhow!(
                "SectionClassInfo pack error: names count {} does not match infos count {}",
                count,
                self.infos.len()
            ));
        }
        count.pack(writer, big, encoding, info)?;
        for name in &self.names {
            name.pack(writer, big, encoding, info)?;
        }
        for class_info in &self.infos {
            class_info.pack(writer, big, encoding, info)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct FuncNameEntry {
    pub address: u32,
    pub name: WideString,
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

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct SectionFunction {
    pub prologue: DWordArray,
    pub epilogue: DWordArray,
    #[pvec(u32)]
    pub func_names: Vec<FuncNameEntry>,
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct SectionInitNakedFunc {
    pub naked_prologue: DWordArray,
    pub naked_epilogue: DWordArray,
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct FuncEntryHeader {
    pub flags: u32,
    pub address: u32,
    pub bytes: u32,
    pub reserved: u32,
}

#[derive(Clone, Debug, StructPack)]
pub struct FuncInfoEntry {
    pub header: FuncEntryHeader,
    pub name: WideString,
    pub reserved: Vec<u8>,
}

impl StructUnpack for FuncInfoEntry {
    fn unpack<R: Read + Seek>(
        reader: &mut R,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn std::any::Any>>,
    ) -> Result<Self> {
        let header = FuncEntryHeader::unpack(reader, big, encoding, info)?;
        let name = WideString::unpack(reader, big, encoding, info)?;
        let reserved_size = header.reserved as usize;
        let reserved = reader.read_exact_vec(reserved_size)?;
        Ok(Self {
            header,
            name,
            reserved,
        })
    }
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct SectionFuncInfo {
    #[pvec(u32)]
    pub functions: Vec<FuncInfoEntry>,
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct NakedSymbolInfoEntry {
    pub flags: u32,
    pub reserved: u32,
    pub address: u64,
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct SymbolInfoEntry {
    pub info: NakedSymbolInfoEntry,
    pub name: WideString,
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct SectionSymbolInfo {
    #[pvec(u32)]
    pub symbols: Vec<SymbolInfoEntry>,
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct TagedObjectEntry {
    pub tag: WideString,
    pub object: TypedObject,
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct SectionGlobal {
    #[pvec(u32)]
    pub objects: Vec<TagedObjectEntry>,
}

#[derive(Clone, Debug)]
pub enum GlobalObjectEntryObj {
    Object(TypedObject),
    List(Vec<TagedObjectEntry>),
}

#[derive(Clone, Debug)]
pub struct GlobalObjectEntry {
    pub name: WideString,
    pub object: GlobalObjectEntryObj,
}

impl StructUnpack for GlobalObjectEntry {
    fn unpack<R: Read + Seek>(
        reader: &mut R,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn std::any::Any>>,
    ) -> Result<Self> {
        let name = WideString::unpack(reader, big, encoding, info)?;
        let length = i32::unpack(reader, big, encoding, info)?;
        if length >= 0 {
            let obj = reader.read_struct_vec(length as usize, big, encoding, info)?;
            Ok(Self {
                name,
                object: GlobalObjectEntryObj::List(obj),
            })
        } else {
            let obj = TypedObject::unpack(reader, big, encoding, info)?;
            Ok(Self {
                name,
                object: GlobalObjectEntryObj::Object(obj),
            })
        }
    }
}

impl StructPack for GlobalObjectEntry {
    fn pack<W: Write>(
        &self,
        writer: &mut W,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn std::any::Any>>,
    ) -> Result<()> {
        self.name.pack(writer, big, encoding, info)?;
        match &self.object {
            GlobalObjectEntryObj::Object(obj) => {
                (0x80000000u32).pack(writer, big, encoding, info)?;
                obj.pack(writer, big, encoding, info)?;
            }
            GlobalObjectEntryObj::List(list) => {
                let length = list.len() as i32;
                length.pack(writer, big, encoding, info)?;
                for item in list {
                    item.pack(writer, big, encoding, info)?;
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct SectionData {
    #[pvec(u32)]
    pub objects: Vec<GlobalObjectEntry>,
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct ConstStringEntry {
    pub string: WideString,
    pub refs: DWordArray,
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct SectionConstString {
    #[pvec(u32)]
    pub strings: Vec<ConstStringEntry>,
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct TagedDwordArrayEntry {
    pub tag: WideString,
    pub array: DWordArray,
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct TagedDwordArray {
    #[pvec(u32)]
    pub elements: Vec<TagedDwordArrayEntry>,
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct SectionLinkInfo {
    pub ext_global_ref: DWordArray,
    pub ext_data_ref: DWordArray,
    pub imp_global_ref: TagedDwordArray,
    pub imp_data_ref: TagedDwordArray,
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct SectionLinkInfoEx {
    pub flags: u32,
    pub ext_naked_global_ref: DWordArray,
    pub ext_naked_const_ref: DWordArray,
    pub ext_naked_shared_ref: DWordArray,
    pub ext_naked_func_ref: DWordArray,
    pub imp_naked_global_ref: TagedDwordArray,
    pub imp_naked_const_ref: TagedDwordArray,
    pub imp_naked_shared_ref: TagedDwordArray,
    pub imp_naked_func_ref: TagedDwordArray,
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct SectionRefFunc {
    pub refs: TagedDwordArray,
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct SectionRefCode {
    pub refs: DWordArray,
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct SectionRefClass {
    pub refs: DWordArray,
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct SectionNativeFunc {
    #[pvec(u32)]
    pub names: Vec<WideString>,
    pub addresses: DWordArray,
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
pub struct SectionNakedFunc {
    #[pvec(u32)]
    pub names: Vec<WideString>,
    pub addresses: DWordArray,
}

const ID_NATIVE_FUNC: u64 = 0x636E66766974616E;
const ID_NAKED_FUNC: u64 = 0x636E6664656B616E;

#[derive(Clone, Debug)]
pub struct SectionImportNativeFunc {
    pub native_func: SectionNativeFunc,
    pub naked_func: SectionNakedFunc,
}

impl StructUnpack for SectionImportNativeFunc {
    fn unpack<R: Read + Seek>(
        mut reader: &mut R,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn std::any::Any>>,
    ) -> Result<Self> {
        let len = reader.stream_length()?;
        let mut native_func = None;
        let mut naked_func = None;
        while reader.stream_position()? < len {
            let id = u64::unpack(reader, big, encoding, info)?;
            let length = u64::unpack(reader, big, encoding, info)?;
            match id {
                ID_NATIVE_FUNC => {
                    let mut mem = StreamRegion::with_size(&mut reader, length)?;
                    let func = SectionNativeFunc::unpack(&mut mem, big, encoding, info)?;
                    native_func = Some(func);
                }
                ID_NAKED_FUNC => {
                    let mut mem = StreamRegion::with_size(&mut reader, length)?;
                    let func = SectionNakedFunc::unpack(&mut mem, big, encoding, info)?;
                    naked_func = Some(func);
                }
                _ => {
                    return Err(anyhow::anyhow!(
                        "Unknown SectionImportNativeFunc id: {:#X}",
                        id
                    ));
                }
            }
        }
        Ok(Self {
            native_func: native_func
                .ok_or_else(|| anyhow::anyhow!("Missing native_func in SectionImportNativeFunc"))?,
            naked_func: naked_func
                .ok_or_else(|| anyhow::anyhow!("Missing naked_func in SectionImportNativeFunc"))?,
        })
    }
}

impl StructPack for SectionImportNativeFunc {
    fn pack<W: Write>(
        &self,
        mut writer: &mut W,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn std::any::Any>>,
    ) -> Result<()> {
        // Write native_func
        ID_NATIVE_FUNC.pack(&mut writer, big, encoding, info)?;
        let mut native_func_buf = Vec::new();
        self.native_func
            .pack(&mut native_func_buf, big, encoding, info)?;
        (native_func_buf.len() as u64).pack(&mut writer, big, encoding, info)?;
        writer.write_all(&native_func_buf)?;

        // Write naked_func
        ID_NAKED_FUNC.pack(&mut writer, big, encoding, info)?;
        let mut naked_func_buf = Vec::new();
        self.naked_func
            .pack(&mut naked_func_buf, big, encoding, info)?;
        (naked_func_buf.len() as u64).pack(&mut writer, big, encoding, info)?;
        writer.write_all(&naked_func_buf)?;

        Ok(())
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
    pub commands: Vec<ECSExecutionImageCommandRecord>,
}

impl Deref for ECSExecutionImageAssembly {
    type Target = Vec<ECSExecutionImageCommandRecord>;

    fn deref(&self) -> &Self::Target {
        &self.commands
    }
}

impl DerefMut for ECSExecutionImageAssembly {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.commands
    }
}
