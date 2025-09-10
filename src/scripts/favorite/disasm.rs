use crate::ext::io::*;
use crate::types::*;
use crate::utils::encoding::*;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::io::{Read, Seek, SeekFrom};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Oper {
    // Byte
    B,
    // Word
    W,
    // Double Word
    D,
    // String
    S,
}

use Oper::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", content = "c")]
pub enum Operand {
    B(u8),
    W(u16),
    D(u32),
    S(String),
}

impl Operand {
    pub fn len(&self, encoding: Encoding) -> Result<usize> {
        Ok(match self {
            Operand::B(_) => 1,
            Operand::W(_) => 2,
            Operand::D(_) => 4,
            Operand::S(s) => {
                let bytes = encode_string(encoding, s, true)?;
                // null terminator + length byte
                bytes.len() + 2
            }
        })
    }
}

const OPS: [(u8, &[Oper]); 49] = [
    (0x00, &[]),
    (0x01, &[B, B]), //unknown
    (0x02, &[D]),    //call function
    (0x03, &[W]),    //unknown
    (0x04, &[]),     //retn?
    (0x05, &[]),     //retn?
    (0x06, &[D]),    //jump?
    (0x07, &[D]),    //cond jump?
    (0x08, &[]),     //unknown
    (0x09, &[]),     //unknown
    (0x0a, &[D]),    //unknown
    (0x0b, &[W]),    //unknown
    (0x0c, &[B]),    //unknown
    (0x0d, &[]),     //empty
    (0x0e, &[S]),    //string
    (0x0f, &[W]),    //unknown
    (0x10, &[B]),    //unknown
    (0x11, &[W]),    //unknown
    (0x12, &[B]),    //unknown
    (0x13, &[]),
    (0x14, &[]),  //unknown
    (0x15, &[W]), //unknown
    (0x16, &[B]), //unknown
    (0x17, &[W]), //unknown
    (0x18, &[B]), //unknown
    (0x19, &[]),  //unknown
    (0x1a, &[]),  //unknown
    (0x1b, &[]),  //unknown
    (0x1c, &[]),  //unknown
    (0x1d, &[]),  //unknown
    (0x1e, &[]),  //unknown
    (0x1f, &[]),  //unknown
    (0x20, &[]),  //unknown
    (0x21, &[]),  //unknown
    (0x22, &[]),  //unknown
    (0x23, &[]),  //unknown
    (0x24, &[]),  //unknown
    (0x25, &[]),  //unknown
    (0x26, &[]),  //unknown
    (0x27, &[]),  //unknown
    (0x33, &[]),
    (0x3f, &[]),
    (0x40, &[]),
    (0xb3, &[]),
    (0xb8, &[]),
    (0xd8, &[]),
    (0xf0, &[]),
    (0x52, &[]),
    (0x9e, &[]),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Func {
    pub pos: u64,
    pub opcode: u8,
    pub operands: Vec<Operand>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Data {
    pub functions: Vec<Func>,
    pub main_script: Vec<Func>,
    pub extra_data: Vec<u8>,
}

impl Data {
    pub fn disasm<R: Read + Seek>(mut reader: R, encoding: Encoding) -> Result<Self> {
        let mut data = Data {
            functions: Vec::new(),
            main_script: Vec::new(),
            extra_data: Vec::new(),
        };
        let script_len = reader.read_u32()? as u64;
        let main_script_data = reader.peek_u32_at(script_len)? as u64;
        {
            let mut target = &mut data.functions;
            let mut pos = reader.stream_position()?;
            while pos < script_len {
                if pos >= main_script_data {
                    target = &mut data.main_script;
                }
                target.push(Self::read_func(&mut reader, encoding)?);
                pos = reader.stream_position()?;
            }
        }
        reader.seek(SeekFrom::Start(script_len + 4))?;
        reader.read_to_end(&mut data.extra_data)?;
        Ok(data)
    }

    fn read_func<R: Read + Seek>(reader: &mut R, encoding: Encoding) -> Result<Func> {
        let pos = reader.stream_position()?;
        let opcode = reader.read_u8()?;
        let operands = if let Some((_, ops)) = OPS.iter().find(|(code, _)| *code == opcode) {
            let mut operands = Vec::with_capacity(ops.len());
            for &op in *ops {
                let operand = match op {
                    B => Operand::B(reader.read_u8()?),
                    W => Operand::W(reader.read_u16()?),
                    D => Operand::D(reader.read_u32()?),
                    S => {
                        let len = reader.read_u8()? as usize;
                        let s = reader.read_cstring()?;
                        if s.as_bytes_with_nul().len() != len {
                            return Err(anyhow::anyhow!(
                                "String length mismatch at {:#x}: expected {}, got {}",
                                pos,
                                len,
                                s.as_bytes_with_nul().len()
                            ));
                        }
                        let s = decode_to_string(encoding, s.as_bytes(), true)?;
                        Operand::S(s)
                    }
                };
                operands.push(operand);
            }
            operands
        } else {
            return Err(anyhow::anyhow!("Unknown opcode: {:#x} at {:#x}", opcode, pos))
        };
        Ok(Func {
            pos,
            opcode,
            operands,
        })
    }
}
