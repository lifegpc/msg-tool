use crate::ext::io::*;
use crate::types::*;
use crate::utils::encoding::*;
use crate::utils::struct_pack::*;
use anyhow::Result;
use int_enum::IntEnum;
use msg_tool_macro::{StructPack, StructUnpack};
use std::io::{Read, Seek, Write};

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
pub use CodeLoadLocal as CodeLoadLocalImm32;
pub use CodeLoadMem as CodeLoadMemBase;
pub use CodeStoreLocal as CodeStoreLocalImm32;
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
