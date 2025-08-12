use crate::ext::io::*;
use crate::types::*;
use crate::utils::encoding::*;
use anyhow::Result;
use std::any::Any;

pub trait Disasm: Sized {
    fn disassmble(self) -> Result<(Vec<usize>, Vec<Ws2DString>)>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Oper {
    /// Byte
    B,
    /// Word
    H,
    /// Int
    I,
    /// Address
    A,
    /// Float
    F,
    /// String
    S,
    /// Array of operands (*)
    ARR,
}
use Oper::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringType {
    Name,
    Message,
    Internal,
}

#[derive(Debug, Clone)]
pub struct Ws2DString {
    pub text: String,
    pub offset: usize,
    pub len: usize,
    pub typ: StringType,
}

struct DisasmBase<'a> {
    reader: MemReaderRef<'a>,
    opers: &'a [(u8, &'static [Oper])],
    addresses: Vec<usize>,
    texts: Vec<Ws2DString>,
    encoding: Encoding,
}

impl<'a> DisasmBase<'a> {
    pub fn new(data: &'a [u8], opers: &'a [(u8, &'static [Oper])], encoding: Encoding) -> Self {
        DisasmBase {
            reader: MemReaderRef::new(data),
            opers,
            addresses: Vec::new(),
            texts: Vec::new(),
            encoding,
        }
    }

    fn read_instruction(&mut self) -> Result<(u8, Vec<Box<dyn Any>>)> {
        let opcode = self.reader.read_u8()?;
        let opers = self
            .opers
            .iter()
            .find(|&&(op, _)| op == opcode)
            .ok_or_else(|| anyhow::anyhow!("Unknown opcode: {opcode}"))?;
        let operands = self.read_operands(opers.1)?;
        Ok((opcode, operands))
    }

    fn read_operands(&mut self, opers: &[Oper]) -> Result<Vec<Box<dyn Any>>> {
        let mut operands = Vec::new();
        let mut i = 0;
        let oper_len = opers.len();
        while i < oper_len {
            let oper = opers[i];
            if i < oper_len - 1 && opers[i + 1] == ARR {
                i += 1;
                let count = self.reader.read_u8()?;
                for _ in 0..count {
                    operands.push(self.read_operand(oper)?);
                }
            } else {
                let operand = self.read_operand(oper)?;
                operands.push(operand);
            }
            i += 1;
        }
        Ok(operands)
    }

    fn read_operand(&mut self, oper: Oper) -> Result<Box<dyn Any>> {
        match oper {
            B => {
                let value = self.reader.read_u8()?;
                Ok(Box::new(value))
            }
            H => {
                let value = self.reader.read_i16()?;
                Ok(Box::new(value))
            }
            I => {
                let value = self.reader.read_i32()?;
                Ok(Box::new(value))
            }
            A => {
                let pos = self.reader.pos;
                let address = self.reader.read_i32()?;
                self.addresses.push(pos);
                Ok(Box::new(address))
            }
            F => {
                let value = self.reader.read_f32()?;
                Ok(Box::new(value))
            }
            S => {
                let offset = self.reader.pos;
                let s = self.reader.read_cstring()?;
                let decoded = decode_to_string(self.encoding, s.as_bytes(), false)?;
                let len = s.as_bytes_with_nul().len();
                let str = Ws2DString {
                    text: decoded,
                    offset,
                    len,
                    typ: StringType::Internal,
                };
                Ok(Box::new(str))
            }
            _ => {
                // Handle other operand types as needed
                Err(anyhow::anyhow!("Unsupported operand type: {:?}", oper))
            }
        }
    }

    fn handle_choice_screen(&mut self, operands: &mut Vec<Box<dyn Any>>) -> Result<()> {
        if operands.len() < 1 {
            return Err(anyhow::anyhow!("Invalid operands for choice screen"));
        }
        let first = operands.remove(0);
        let num_choices = first
            .downcast::<u8>()
            .map_err(|_| anyhow::anyhow!("Invalid choice count"))?;
        for _ in 0..*num_choices {
            let mut opers = self.read_operands(&[H, S, B, H])?;
            let range = opers.remove(1);
            let mut range = range
                .downcast::<Ws2DString>()
                .map_err(|_| anyhow::anyhow!("Invalid range operand"))?;
            if range.len > 1 {
                range.typ = StringType::Message;
                self.texts.push(*range);
            }
            self.read_instruction()?;
        }
        Ok(())
    }

    fn handle_message(&mut self, operands: &mut Vec<Box<dyn Any>>) -> Result<()> {
        if operands.len() < 3 {
            return Err(anyhow::anyhow!("Invalid operands for message"));
        }
        let range = operands.remove(2);
        let mut range = range
            .downcast::<Ws2DString>()
            .map_err(|_| anyhow::anyhow!("Invalid range operand"))?;
        if range.len > 1 {
            range.typ = StringType::Message;
            self.texts.push(*range);
        }
        Ok(())
    }

    fn handle_name(&mut self, operands: &mut Vec<Box<dyn Any>>) -> Result<()> {
        if operands.len() < 1 {
            return Err(anyhow::anyhow!("Invalid operands for name"));
        }
        let name = operands.remove(0);
        let mut name = name
            .downcast::<Ws2DString>()
            .map_err(|_| anyhow::anyhow!("Invalid name operand"))?;
        if name.len > 1 {
            name.typ = StringType::Name;
            self.texts.push(*name);
        }
        Ok(())
    }
}

impl<'a> Disasm for DisasmBase<'a> {
    fn disassmble(mut self) -> Result<(Vec<usize>, Vec<Ws2DString>)> {
        let maxlen = self.reader.data.len() - 8;
        while self.reader.pos < maxlen {
            let (opcode, mut operands) = self.read_instruction()?;
            match opcode {
                0x0F => self.handle_choice_screen(&mut operands)?,
                0x14 => self.handle_message(&mut operands)?,
                0x15 => self.handle_name(&mut operands)?,
                _ => {}
            }
            for oper in operands {
                if let Ok(str) = oper.downcast::<Ws2DString>() {
                    if str.len > 1 {
                        self.texts.push(*str);
                    }
                }
            }
        }
        self.texts.sort_by_key(|s| s.offset);
        Ok((self.addresses, self.texts))
    }
}

const V1_OPS: [(u8, &'static [Oper]); 103] = [
    (0x00, &[]),
    (0x01, &[B, H, F, A, A]),
    (0x02, &[A]),
    (0x04, &[S]),
    (0x05, &[]),
    (0x06, &[A]),
    (0x07, &[S]),
    (0x08, &[B]),
    (0x09, &[B, H, F]),
    (0x0A, &[H, F]),
    (0x0B, &[H, B]),
    (0x0C, &[H, B, H, ARR]),
    (0x0D, &[H, H, F]),
    (0x0E, &[H, H, B]),
    (0x0F, &[B]),
    (0x11, &[S, F]),
    (0x12, &[S, B, S]),
    (0x13, &[]),
    (0x14, &[I, S, S]),
    (0x15, &[S]),
    (0x16, &[B]),
    (0x17, &[]),
    (0x18, &[B, S]),
    (0x19, &[]),
    (0x1A, &[S]),
    (0x1B, &[B]),
    (0x1C, &[S, S, H]),
    (0x1D, &[H]),
    (0x1E, &[S, S, F, F, H, H, B]),
    (0x1F, &[S, F]),
    (0x20, &[S, F, H]),
    (0x21, &[S, H, H, H]),
    (0x22, &[S, B]),
    (0x28, &[S, S, F, F, H, H, B, H, H, B]),
    (0x29, &[S, F]),
    (0x2A, &[S, F, H]),
    (0x2B, &[S]),
    (0x2C, &[S]),
    (0x2D, &[S, B]),
    (0x2E, &[]),
    (0x2F, &[S, H, F]),
    (0x32, &[S]),
    (0x33, &[S, S, B, B]),
    (0x34, &[S, S, B, B]),
    (0x35, &[S, S, B, B, B]),
    (0x36, &[S, F, F, F, F, F, F, F, B, B]),
    (0x37, &[S]),
    (0x38, &[S, B]),
    (0x39, &[S, B, B, H, ARR]),
    (0x3A, &[S, B, B]),
    (0x3B, &[S, S, H, H, H, F, F, F, F, F, F, F, F]),
    (0x3C, &[S]),
    (0x3D, &[H]),
    (0x3E, &[]),
    (0x3F, &[S, ARR]),
    (0x40, &[S, S, B]),
    (0x41, &[S, B]),
    (0x42, &[S, H]),
    (0x43, &[S]),
    (0x44, &[S, S, B]),
    (0x45, &[S, H, F, F, F, F]),
    (0x46, &[S, H, B, F, F, F, F]),
    (0x47, &[S, S, H, B, B, F, F, F, F, F, H, F]),
    (0x48, &[S, S, H, B, B, S]),
    (0x49, &[S, S, S]),
    (0x4A, &[S, S]),
    (0x4B, &[S, H, H, F, F, F, F]),
    (0x4C, &[S, H, H, B, F, F, F, F]),
    (0x4D, &[S, S, H, H, B, B, F, F, F, F, F, H, F]),
    (0x4E, &[S, S, H, H, B, B, S]),
    (0x4F, &[S, S, H, S]),
    (0x50, &[S, S, H]),
    (0x51, &[S, S, H, F, B]),
    (0x52, &[S, S, F, H, F, B, S]),
    (0x53, &[S, S]),
    (0x54, &[S, S, S]),
    (0x55, &[S, S]),
    (
        0x56,
        &[
            S, B, H, F, F, F, F, F, F, F, F, F, F, F, B, F, F, F, F, B, H, S, H, S, S, F,
        ],
    ),
    (0x57, &[S, H]),
    (0x58, &[S, S]),
    (0x59, &[S, S, H]),
    (0x5A, &[S, H, ARR]),
    (0x5B, &[S, H, B]),
    (0x5C, &[S]),
    (0x5D, &[S, S, B]),
    (0x5E, &[S, F, F]),
    (0x64, &[B]),
    (0x65, &[H, B, F, F, B, S]),
    (0x66, &[S]),
    (0x67, &[B, B, H, F, F, F, F, F, B]),
    (0x68, &[B]),
    (0x6E, &[S, S]),
    (0x6F, &[S]),
    (0x70, &[S, H]),
    (0x71, &[]),
    (0x72, &[S, H, H, S]),
    (0x73, &[S, S, H]),
    (0xFA, &[]),
    (0xFB, &[B]),
    (0xFC, &[H]),
    (0xFD, &[]),
    (0xFE, &[S]),
    (0xFF, &[]),
];

const V2_OPS: [(u8, &'static [Oper]); 134] = [
    (0x00, &[]),
    (0x01, &[B, H, F, A, A]),
    (0x02, &[A]),
    (0x04, &[S]),
    (0x05, &[]),
    (0x06, &[A]),
    (0x07, &[S]),
    (0x08, &[B]),
    (0x09, &[B, H, F]),
    (0x0A, &[H, F]),
    (0x0B, &[H, B]),
    (0x0C, &[H, B, H, ARR]),
    (0x0D, &[H, H, F]),
    (0x0E, &[H, H, B]),
    (0x0F, &[B]),
    (0x11, &[S, F]),
    (0x12, &[S, B, S]),
    (0x13, &[]),
    (0x14, &[I, S, S]),
    (0x15, &[S]),
    (0x16, &[B]),
    (0x17, &[]),
    (0x18, &[B, S]),
    (0x19, &[]),
    (0x1A, &[S]),
    (0x1B, &[B]),
    (0x1C, &[S, S, H, B]),
    (0x1D, &[H]),
    (0x1E, &[S, S, F, F, H, H, B]),
    (0x1F, &[S, F]),
    (0x20, &[S, F, H]),
    (0x21, &[S, H, H, H]),
    (0x22, &[S, B]),
    (0x28, &[S, S, F, F, H, H, B, H, H, B]),
    (0x29, &[S, F]),
    (0x2A, &[S, F, H]),
    (0x2B, &[S]),
    (0x2C, &[S]),
    (0x2D, &[S, B]),
    (0x2E, &[]),
    (0x2F, &[S, H, F]),
    (0x32, &[S]),
    (0x33, &[S, S, B, B]),
    (0x34, &[S, S, B, B]),
    (0x35, &[S, S, B, B, B]),
    (0x36, &[S, F, F, F, F, F, F, F, B, B]),
    (0x37, &[S]),
    (0x38, &[S, B]),
    (0x39, &[S, B, B, H, ARR]),
    (0x3A, &[S, B, B]),
    (0x3B, &[S, S, H, H, H, F, F, F, F, F, F, F, F]),
    (0x3C, &[S]),
    (0x3D, &[H]),
    (0x3E, &[]),
    (0x3F, &[S, ARR]),
    (0x40, &[S, S, B]),
    (0x41, &[S, B]),
    (0x42, &[S, H]),
    (0x43, &[S]),
    (0x44, &[S, S, B]),
    (0x45, &[S, H, F, F, F, F]),
    (0x46, &[S, H, B, F, F, F, F]),
    (0x47, &[S, S, H, B, B, F, F, F, F, F, H, F]),
    (0x48, &[S, S, H, B, B, S]),
    (0x49, &[S, S, S]),
    (0x4A, &[S, S]),
    (0x4B, &[S, H, H, F, F, F, F]),
    (0x4C, &[S, H, H, B, F, F, F, F]),
    (0x4D, &[S, S, H, H, B, B, F, F, F, F, F, H, F]),
    (0x4E, &[S, S, H, H, B, B, S]),
    (0x4F, &[S, S, H, S]),
    (0x50, &[S, S, H]),
    (0x51, &[S, S, H, F, B]),
    (0x52, &[S, S, F, H, F, B, S]),
    (0x53, &[S, S]),
    (0x54, &[S, S, S]),
    (0x55, &[S, S]),
    (
        0x56,
        &[
            S, B, H, F, F, F, F, F, F, F, F, F, F, F, B, F, F, F, F, B, H, S, H, S, S, F,
        ],
    ),
    (0x57, &[S, H]),
    (0x58, &[S, S]),
    (0x59, &[S, S, H]),
    (0x5A, &[S, H, ARR]),
    (0x5B, &[S, H, B]),
    (0x5C, &[S]),
    (0x5D, &[S, S, B]),
    (0x5E, &[S, F, F]),
    (0x5F, &[S]),
    (0x60, &[H, H, H, H]),
    (0x61, &[B, F, F, F, F]),
    (0x62, &[S]),
    (0x63, &[S, B]),
    (0x64, &[B]),
    (0x65, &[H, B, F, F, B, S]),
    (0x66, &[S]),
    (0x67, &[B, B, H, F, F, F, F, F, B]),
    (0x68, &[B]),
    (0x69, &[S, B, B, F, F, F, F, F, H, F]),
    (0x6A, &[S, H, B, B, S]),
    (0x6E, &[S, S]),
    (0x6F, &[S]),
    (0x70, &[S, H]),
    (0x71, &[]),
    (0x72, &[S, H, H, S]),
    (0x73, &[S, S, H]),
    (0x74, &[S, S]),
    (0x75, &[S, S]),
    (0x78, &[S, S, B, B]),
    (0x79, &[S, S, F]),
    (0x7A, &[S, S, F, B, B, S]),
    (0x7B, &[S, S]),
    (0x7C, &[S, S, F]),
    (0x7D, &[S, F]),
    (0x7E, &[S]),
    (0xC8, &[]),
    (0xC9, &[S, S, H, H, H]),
    (0xCA, &[S, S]),
    (0xCB, &[S, B, B]),
    (0xCC, &[]),
    (0xCD, &[S, S, S, S, S, F, B]),
    (0xCE, &[B]),
    (0xCF, &[S, S, F]),
    (0xD0, &[S, H]),
    (0xD1, &[S, H]),
    (0xD2, &[S]),
    (0xD3, &[S]),
    (0xD4, &[S, H, H]),
    (0xF8, &[]),
    (0xF9, &[B, S]),
    (0xFA, &[]),
    (0xFB, &[B]),
    (0xFC, &[H]),
    (0xFD, &[]),
    (0xFE, &[S]),
    (0xFF, &[]),
];

const V3_OPS: [(u8, &'static [Oper]); 165] = [
    (0x00, &[]),
    (0x01, &[B, H, F, A, A]),
    (0x02, &[A]),
    (0x04, &[S]),
    (0x05, &[]),
    (0x06, &[A]),
    (0x07, &[S]),
    (0x08, &[B]),
    (0x09, &[B, H, F]),
    (0x0A, &[H, F]),
    (0x0B, &[H, B]),
    (0x0C, &[H, B, H, ARR]),
    (0x0D, &[H, H, F]),
    (0x0E, &[H, H, B]),
    (0x0F, &[B]),
    (0x11, &[S, B, F]),
    (0x12, &[S, B, S]),
    (0x13, &[]),
    (0x14, &[I, S, S, B]),
    (0x15, &[S, B]),
    (0x16, &[B, B]),
    (0x17, &[]),
    (0x18, &[B, S]),
    (0x19, &[]),
    (0x1A, &[S]),
    (0x1B, &[B]),
    (0x1C, &[S, S, H, B]),
    (0x1D, &[H]),
    (0x1E, &[S, S, F, F, H, H, B, F]),
    (0x1F, &[S, F]),
    (0x20, &[S, F, H]),
    (0x21, &[S, H, H, H]),
    (0x22, &[S, B]),
    (0x28, &[S, S, F, F, H, H, B, H, H, B, F]),
    (0x29, &[S, F]),
    (0x2A, &[S, F, H]),
    (0x2B, &[S]),
    (0x2C, &[S]),
    (0x2D, &[S, B]),
    (0x2E, &[]),
    (0x2F, &[S, H, F]),
    (0x32, &[S]),
    (0x33, &[S, S, B, B]),
    (0x34, &[S, S, B, B]),
    (0x35, &[S, S, B, B, B]),
    (0x36, &[S, F, F, F, F, F, F, F, B, B]),
    (0x37, &[S]),
    (0x38, &[S, B]),
    (0x39, &[S, B, B, H, ARR]),
    (0x3A, &[S, B, B]),
    (0x3B, &[S, S, H, H, H, F, F, F, F, F, F, F, F]),
    (0x3C, &[S]),
    (0x3D, &[H]),
    (0x3E, &[]),
    (0x3F, &[S, ARR]),
    (0x40, &[S, S, B]),
    (0x41, &[S, B]),
    (0x42, &[S, H]),
    (0x43, &[S]),
    (0x44, &[S, S, B]),
    (0x45, &[S, H, F, F, F, F]),
    (0x46, &[S, H, B, F, F, F, F]),
    (0x47, &[S, S, H, B, B, F, F, F, F, F, H, F]),
    (0x48, &[S, S, H, B, B, S]),
    (0x49, &[S, S, S]),
    (0x4A, &[S, S]),
    (0x4B, &[S, H, H, F, F, F, F]),
    (0x4C, &[S, H, H, B, F, F, F, F]),
    (0x4D, &[S, S, H, H, B, B, F, F, F, F, F, H, F]),
    (0x4E, &[S, S, H, H, B, B, S]),
    (0x4F, &[S, S, H, S]),
    (0x50, &[S, S, H]),
    (0x51, &[S, S, H, F, B]),
    (0x52, &[S, S, F, H, F, B, S]),
    (0x53, &[S, S]),
    (0x54, &[S, S, S]),
    (0x55, &[S, S]),
    (
        0x56,
        &[
            S, B, H, F, F, F, F, F, F, F, F, F, F, F, B, F, F, F, F, B, H, S, H, S, S, F,
        ],
    ),
    (0x57, &[S, H]),
    (0x58, &[S, S]),
    (0x59, &[S, S, H]),
    (0x5A, &[S, H, ARR]),
    (0x5B, &[S, H, B]),
    (0x5C, &[S]),
    (0x5D, &[S, S, B]),
    (0x5E, &[S, F, F]),
    (0x5F, &[S]),
    (0x60, &[H, H, H, H]),
    (0x61, &[B, F, F, F, F]),
    (0x62, &[S]),
    (0x63, &[S, B]),
    (0x64, &[B]),
    (0x65, &[H, B, F, F, B, S]),
    (0x66, &[S]),
    (0x67, &[B, B, H, F, F, F, F, F, B]),
    (0x68, &[B]),
    (0x69, &[S, B, B, F, F, F, F, F, H, F]),
    (0x6A, &[S, H, B, B, S]),
    (0x6B, &[S, S]),
    (0x6C, &[S, F, F]),
    (0x6E, &[S, S]),
    (0x6F, &[S]),
    (0x70, &[S, H]),
    (0x71, &[]),
    (0x72, &[S, H, H, S]),
    (0x73, &[S, S, H]),
    (0x74, &[S, S]),
    (0x75, &[S, S]),
    (0x78, &[S, S, B, B, B]),
    (0x79, &[S, S, F]),
    (0x7A, &[S, S, F, B, B, S]),
    (0x7B, &[S, S]),
    (0x7C, &[S, S, F]),
    (0x7D, &[S, F]),
    (0x7E, &[S]),
    (0x7F, &[S, F, F, F, F, F]),
    (0x80, &[S]),
    (0x81, &[S, B, S, F, F, B]),
    (0x82, &[S, S, F]),
    (0x83, &[S, S, F, F]),
    (0x84, &[S, S, S, F, H, F]),
    (0x85, &[S, S, B, F]),
    (0x86, &[S, F, F, F]),
    (0x87, &[S, F]),
    (0x88, &[S, S, S, F, H, F]),
    (0x8C, &[S, S, S, B, B]),
    (0x8D, &[I, S, S, B, B, S]),
    (0x8E, &[I, S, S, B, B, S]),
    (0x8F, &[S, S]),
    (0x90, &[S]),
    (0x96, &[H, F, F, F, F]),
    (0x97, &[H, B, F, F, F, F]),
    (0x98, &[S, H, B, B, F, F, F, F, F, H, F]),
    (0x99, &[S, H, B, B, S]),
    (0x9A, &[]),
    (0x9B, &[S]),
    (0x9C, &[S, S]),
    (0x9D, &[S]),
    (0x9E, &[S, B]),
    (0x9F, &[S, B]),
    (0xC8, &[]),
    (0xC9, &[S, S, H, H, H, H]),
    (0xCA, &[S, S]),
    (0xCB, &[S, B, B]),
    (0xCC, &[]),
    (0xCD, &[S, S, S, S, S, F, B]),
    (0xCE, &[B]),
    (0xCF, &[S, S, F]),
    (0xD0, &[S, H]),
    (0xD1, &[S, H]),
    (0xD2, &[S]),
    (0xD3, &[S]),
    (0xD4, &[S, H, H]),
    (0xE6, &[I, I]),
    (0xE7, &[]),
    (0xE8, &[]),
    (0xF0, &[B]),
    (0xF8, &[]),
    (0xF9, &[B, S]),
    (0xFA, &[]),
    (0xFB, &[B]),
    (0xFC, &[H]),
    (0xFD, &[]),
    (0xFE, &[S]),
    (0xFF, &[]),
];

const OPS: [&[(u8, &'static [Oper])]; 3] = [&V1_OPS, &V2_OPS, &V3_OPS];

pub fn disassmble(data: &[u8], encoding: Encoding) -> Result<(Vec<usize>, Vec<Ws2DString>)> {
    for op in &OPS {
        let disasm = DisasmBase::new(data, op, encoding);
        match disasm.disassmble() {
            Ok(result) => return Ok(result),
            Err(_) => continue, // Try the next version if this one fails
        }
    }
    Err(anyhow::anyhow!(
        "Failed to disassemble the data with all known versions"
    ))
}
