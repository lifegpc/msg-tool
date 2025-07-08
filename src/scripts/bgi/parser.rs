use crate::ext::io::*;
use crate::types::*;
use crate::utils::encoding::decode_to_string;
use anyhow::Result;
use std::collections::HashMap;
use std::io::{Seek, SeekFrom};

#[allow(unused)]
pub enum Inst {
    /// short
    H,
    /// int
    I,
    /// code offset
    C,
    /// message offset
    M,
    /// name offset
    N,
    /// string offset
    Z,
}

use Inst::*;

const V0_INSTS: [(u16, &'static [Inst]); 160] = [
    (0x0010, &[I, I, M]),
    (0x0012, &[Z, Z]),
    (0x0013, &[Z]),
    (0x0014, &[Z]),
    (0x0018, &[I, I, I, I, I]),
    (0x0019, &[I, I, I, I]), // untested
    (0x001A, &[I, I, I]),
    (0x001B, &[Z, I, I, I]), // untested
    (0x001F, &[I]),          // untested
    (0x0022, &[I]),          // untested
    (0x0024, &[I, I, I, I, I]),
    (0x0025, &[I, I]),
    (0x0028, &[Z, I]),
    (0x0029, &[Z, Z, I]),
    (0x002A, &[I]),
    (0x002B, &[Z, I]),
    (0x002C, &[Z, I, I, I, I, I, I, I, I]),
    (0x002D, &[Z, I, I, I, I, I, I, I, I]),
    (0x002E, &[I, I, I, I, I]),
    (0x0030, &[Z, I]),    // untested
    (0x0031, &[Z, I, I]), // untested
    (0x0032, &[I]),       // untested
    (0x0033, &[I]),       // untested
    (0x0034, &[I, I]),
    (0x0035, &[I]),
    (0x0036, &[I]),
    (0x0038, &[I, Z, I, I, I, I, I]),
    (0x0039, &[I, I]),
    (0x003A, &[I, Z, I, I, I, I, I, I, I, I]), // untested
    (0x003B, &[I, I, I, I, I, I]),
    (0x003C, &[I, I, I, I, I, I, I, I, I, I]),
    (0x003D, &[I, I, I, I, I, I, I, I, I, I, I]),
    (0x003F, &[I]),
    (0x0040, &[I, I, Z, I, I]),
    (0x0041, &[I, I, Z, I, I]),
    (0x0042, &[I, I, Z, I]),
    (0x0043, &[I, I, Z, I]),
    (0x0044, &[I, I, Z, I]),
    (0x0045, &[I, I, Z, I]),
    (0x0046, &[I, Z, I]),
    (0x0047, &[I, Z, I]),
    (0x0048, &[I, I]),
    (0x0049, &[I, I]),
    (0x004A, &[I, Z, I]),
    (0x004C, &[Z, I]), // untested
    (0x004D, &[Z, I]), // untested
    (0x004E, &[I]),    // untested
    (0x004F, &[I]),    // untested
    (0x0050, &[Z, I]),
    (0x0051, &[Z, Z, I]),
    (0x0052, &[I]),
    (0x0053, &[Z, I]),
    (0x0054, &[Z, I, I]),
    (0x0060, &[I, I, I, I, I]),
    (0x0061, &[I, I]), // untested
    (0x0062, &[I, I, I, I, I, I]),
    (0x0065, &[I]),
    (0x0066, &[I, I]),
    (0x0067, &[I]),
    (0x0068, &[I]),
    (0x0069, &[I]),
    (0x006A, &[I]),
    (0x006B, &[I]), // untested
    (0x006C, &[I]), // untested
    (0x006E, &[I, I, I]),
    (0x006F, &[I]),
    (0x0070, &[I, Z, I]),
    (0x0071, &[I]),
    (0x0072, &[I, I, I]),
    (0x0073, &[I, I, I]),
    (0x0074, &[I, Z, I]),
    (0x0075, &[I]),
    (0x0076, &[I, I, I]),
    (0x0078, &[I, Z, I]), // untested
    (0x0079, &[I]),       // untested
    (0x007A, &[I, I, I]), // untested
    (0x0080, &[I, Z, I, I]),
    (0x0081, &[Z]),
    (0x0082, &[I]),
    (0x0083, &[I]),
    (0x0084, &[I, Z, I]), // untested
    (0x0085, &[Z]),
    (0x0086, &[I]),
    (0x0087, &[I]),
    (0x0088, &[Z]),
    (0x008C, &[I]),
    (0x008D, &[I]), // untested
    (0x008E, &[I]), // untested
    (0x0090, &[I]), // untested
    (0x0091, &[I]), // untested
    (0x0092, &[I]),
    (0x0093, &[I]),
    (0x0094, &[I]),
    (0x0098, &[I, I]),
    (0x0099, &[I, I]),
    (0x009A, &[I, I]), // untested
    (0x009B, &[I, I]), // untested
    (0x009C, &[I, I]), // untested
    (0x009D, &[I, I]), // untested
    (0x00A0, &[C]),
    (0x00A1, &[I, C]),    // untested
    (0x00A2, &[I, C]),    // untested
    (0x00A3, &[I, I, C]), // untested
    (0x00A4, &[I, I, C]),
    (0x00A5, &[I, I, C]),
    (0x00A6, &[I, I, C]),
    (0x00A7, &[I, I, C]), // untested
    (0x00A8, &[I, I, C]),
    (0x00AC, &[C]), // untested
    (0x00AE, &[I]),
    (0x00C0, &[Z]),
    (0x00C1, &[Z]),
    (0x00C4, &[I]),
    (0x00C8, &[Z]),
    (0x00CA, &[I]), // untested
    (0x00D4, &[I]), // untested
    (0x00D8, &[I]),
    (0x00D9, &[I]),
    (0x00DA, &[I]),
    (0x00DB, &[I]),
    (0x00DC, &[I]),
    (0x00F8, &[Z]),    // untested
    (0x00F9, &[Z, I]), // untested
    (0x00FE, &[H]),
    (0x0110, &[Z, Z]),
    (0x0111, &[I]),
    (0x0120, &[I]),
    (0x0121, &[I]),
    (0x0128, &[Z, I, I]),
    (0x012A, &[I, I]),
    (0x0134, &[I, I]), // untested
    (0x0135, &[I]),    // untested
    (0x0136, &[I]),    // untested
    (0x0138, &[I, Z, I, I, I, I, Z, I, I, I]),
    (0x013B, &[I, I, I, I, I, I, I, I]),
    (0x0140, &[I, I, Z, I, I, I, I]), // untested
    (0x0141, &[I, I, Z, I, I, I, I]), // untested
    (0x0142, &[I, I, Z, I, I, I]),    // untested
    (0x0143, &[I, I, Z, I, I, I]),    // untested
    (0x0144, &[I, I, Z, I, I, I]),    // untested
    (0x0145, &[I, I, Z, I, I, I]),    // untested
    (0x0146, &[I, Z, I, I, I]),       // untested
    (0x0147, &[I, Z, I, I, I]),       // untested
    (0x0148, &[I, I]),
    (0x0149, &[I, I]),
    (0x014B, &[Z, I, I, Z]),
    (0x0150, &[Z, I, I]),
    (0x0151, &[Z, I, I, I]), // untested
    (0x0152, &[I, I]),
    (0x0153, &[I, I, I]), // untested
    (0x016E, &[I, I, I, I, I, I]),
    (0x016F, &[I, I, I, I, I, I, I]), // untested
    (0x0170, &[I, Z, Z, I, I]),
    (0x01C0, &[Z, Z]),
    (0x01C1, &[Z, Z]),          // untested
    (0x0249, &[Z]),             // untested
    (0x024C, &[Z, Z, I, I, I]), // untested
    (0x024D, &[Z]),             // untested
    (0x024E, &[Z, Z]),          // untested
    (0x024F, &[Z]),             // untested
];

const V1_INSTS: [(u32, &'static [Inst]); 12] = [
    (0x0000, &[I]),
    (0x0001, &[C]),
    (0x0002, &[I]),
    //    (0x0003, &[M]),
    (0x0008, &[I]),
    (0x0009, &[I]),
    (0x000A, &[I]),
    (0x0017, &[I]),
    (0x0019, &[I]),
    (0x003F, &[I]),
    (0x007B, &[I, I, I]),
    (0x007E, &[I]),
    (0x007F, &[I, I]),
];

lazy_static::lazy_static! {
    pub static ref V0_INSTS_MAP: HashMap<u16, &'static [Inst]> = HashMap::from(V0_INSTS);
    pub static ref V1_INSTS_MAP: HashMap<u32, &'static [Inst]> = HashMap::from(V1_INSTS);
}

#[derive(Debug, Clone)]
pub enum BGIStringType {
    Name,
    Message,
    Internal,
}

#[derive(Debug, Clone)]
pub struct BGIString {
    pub offset: usize,
    pub address: usize,
    pub typ: BGIStringType,
}

impl BGIString {
    pub fn is_internal(&self) -> bool {
        matches!(self.typ, BGIStringType::Internal)
    }
}

pub struct V0Parser<'a> {
    buf: MemReaderRef<'a>,
    largest_code_address_pperand_encountered: usize,
    pub strings: Vec<BGIString>,
}

impl<'a> V0Parser<'a> {
    pub fn new(buf: MemReaderRef<'a>) -> Self {
        V0Parser {
            buf,
            largest_code_address_pperand_encountered: 0,
            strings: Vec::new(),
        }
    }

    fn read_code_address(&mut self) -> Result<()> {
        let address = self.buf.read_u32()?;
        self.largest_code_address_pperand_encountered = std::cmp::max(
            self.largest_code_address_pperand_encountered,
            address as usize,
        );
        Ok(())
    }

    fn read_string_address(&mut self, typ: BGIStringType) -> Result<()> {
        let offset = self.buf.pos;
        let address = self.buf.read_u32()? as usize;
        self.strings.push(BGIString {
            offset,
            address,
            typ,
        });
        Ok(())
    }

    fn skip_inline_string(&mut self) -> Result<()> {
        self.buf.read_cstring()?;
        Ok(())
    }

    fn read_oper_00a9(&mut self) -> Result<()> {
        let count = self.buf.read_u32()?;
        for _ in 0..count {
            self.read_code_address()?;
        }
        Ok(())
    }

    fn read_oper_00b0(&mut self) -> Result<()> {
        let count = self.buf.read_u32()?;
        for _ in 0..count {
            self.skip_inline_string()?;
        }
        Ok(())
    }

    fn read_oper_00b4(&mut self) -> Result<()> {
        // untested
        let count = self.buf.read_u32()?;
        for _ in 0..count {
            self.skip_inline_string()?;
        }
        Ok(())
    }

    fn read_oper_00fd(&mut self) -> Result<()> {
        // untested
        let count = self.buf.read_u32()?;
        for _ in 0..count {
            self.skip_inline_string()?;
            self.read_code_address()?;
        }
        Ok(())
    }

    fn read_opers(&mut self, templ: &'static [Inst]) -> Result<()> {
        for t in templ.iter() {
            match t {
                H => {
                    self.buf.read_i16()?;
                }
                I => {
                    self.buf.read_i32()?;
                }
                C => {
                    self.read_code_address()?;
                }
                M => {
                    self.read_string_address(BGIStringType::Message)?;
                }
                Z => {
                    self.skip_inline_string()?;
                }
                N => {
                    self.read_string_address(BGIStringType::Name)?;
                }
            }
        }
        Ok(())
    }

    pub fn disassemble(&mut self) -> Result<()> {
        loop {
            let opcode = self.buf.read_u16()?;
            if opcode == 0x00a9 {
                self.read_oper_00a9()?;
            } else if opcode == 0x00b0 {
                self.read_oper_00b0()?;
            } else if opcode == 0x00b4 {
                self.read_oper_00b4()?;
            } else if opcode == 0x00fd {
                self.read_oper_00fd()?;
            } else if let Some(templ) = V0_INSTS_MAP.get(&opcode) {
                self.read_opers(templ)?;
            }
            if opcode == 0x00c2 && self.largest_code_address_pperand_encountered < self.buf.pos {
                break;
            }
        }
        Ok(())
    }
}

struct StackItem {
    pub offset: usize,
    pub value: usize,
}

pub struct V1Parser<'a> {
    buf: MemReaderRef<'a>,
    largest_code_address_pperand_encountered: usize,
    stacks: Vec<StackItem>,
    encoding: Encoding,
    pub offset: usize,
    pub strings: Vec<BGIString>,
}

impl<'a> V1Parser<'a> {
    pub fn new(mut buf: MemReaderRef<'a>, encoding: Encoding) -> Result<Self> {
        if !buf.data.starts_with(b"BurikoCompiledScriptVer1.00\0") {
            return Err(anyhow::anyhow!("Invalid BGI script"));
        }
        if buf.data.len() < 32 {
            return Err(anyhow::anyhow!("Buffer too small"));
        }
        let offset = 28 + buf.peek_u32_at(28)? as u64;
        buf.seek(SeekFrom::Start(offset))?;
        Ok(V1Parser {
            buf,
            largest_code_address_pperand_encountered: 0,
            stacks: Vec::new(),
            encoding,
            offset: offset as usize,
            strings: Vec::new(),
        })
    }

    fn read_code_address(&mut self) -> Result<()> {
        let address = self.buf.read_u32()?;
        self.largest_code_address_pperand_encountered = std::cmp::max(
            self.largest_code_address_pperand_encountered,
            address as usize,
        );
        Ok(())
    }

    fn read_string_address(&mut self, typ: BGIStringType) -> Result<()> {
        let offset = self.buf.pos;
        let address = self.buf.read_u32()? as usize;
        self.strings.push(BGIString {
            offset,
            address,
            typ,
        });
        Ok(())
    }

    fn skip_inline_string(&mut self) -> Result<()> {
        self.buf.read_cstring()?;
        Ok(())
    }

    fn read_opers(&mut self, templ: &'static [Inst]) -> Result<()> {
        for t in templ.iter() {
            match t {
                H => {
                    self.buf.read_i16()?;
                }
                I => {
                    self.buf.read_i32()?;
                }
                C => {
                    self.read_code_address()?;
                }
                M => {
                    self.read_string_address(BGIStringType::Message)?;
                }
                Z => {
                    self.skip_inline_string()?;
                }
                N => {
                    self.read_string_address(BGIStringType::Name)?;
                }
            }
        }
        Ok(())
    }

    fn read_push_string_address_operand(&mut self) -> Result<()> {
        let offset = self.buf.pos;
        let address = self.buf.read_u32()? as usize;
        self.stacks.push(StackItem {
            offset,
            value: address,
        });
        Ok(())
    }

    pub fn is_empty_string(&self, address: usize) -> Result<bool> {
        let start = self.offset + address;
        let data = self.buf.cpeek_u8_at(start)?;
        Ok(data == 0)
    }

    pub fn read_string_at_address(&mut self, address: usize) -> Result<String> {
        let start = self.offset + address;
        let buf = self.buf.peek_cstring_at(start)?;
        // Sometimes string has private use area characters, so we disable strict checking
        Ok(decode_to_string(self.encoding, buf.as_bytes(), false)?)
    }

    pub fn handle_user_function_call(&mut self) -> Result<()> {
        let item = match self.stacks.pop() {
            Some(item) => item,
            None => return Ok(()),
        };
        self.strings.push(BGIString {
            offset: item.offset,
            address: item.value,
            typ: BGIStringType::Internal,
        });
        let funcname = self.read_string_at_address(item.value)?;
        if funcname == "_SelectEx" || funcname == "_SelectExtend" {
            self.handle_choice_screen()?;
        }
        Ok(())
    }

    pub fn handle_message(&mut self) -> Result<()> {
        let item = self
            .stacks
            .pop()
            .ok_or(anyhow::anyhow!("Stack underflow"))?;
        match self.stacks.pop() {
            Some(stack) => {
                self.strings.push(BGIString {
                    offset: stack.offset,
                    address: stack.value,
                    typ: if self.is_empty_string(stack.value)? {
                        BGIStringType::Internal
                    } else {
                        BGIStringType::Name
                    },
                });
            }
            None => {}
        }
        self.strings.push(BGIString {
            offset: item.offset,
            address: item.value,
            typ: if self.is_empty_string(item.value)? {
                BGIStringType::Internal
            } else {
                BGIStringType::Message
            },
        });
        Ok(())
    }

    pub fn handle_choice_screen(&mut self) -> Result<()> {
        let mut choices = Vec::new();
        loop {
            match self.stacks.pop() {
                Some(stack) => {
                    choices.insert(0, stack);
                }
                None => break,
            }
        }
        for choice in choices {
            self.strings.push(BGIString {
                offset: choice.offset,
                address: choice.value,
                typ: BGIStringType::Message,
            });
        }
        Ok(())
    }

    pub fn disassemble(&mut self) -> Result<()> {
        loop {
            let opcode = self.buf.read_u32()?;
            if opcode == 0x0003 {
                self.read_push_string_address_operand()?;
            } else if opcode == 0x001c {
                self.handle_user_function_call()?;
            } else if opcode == 0x0140 || opcode == 0x0143 {
                self.handle_message()?;
            } else if opcode == 0x0160 {
                self.handle_choice_screen()?;
            } else if let Some(templ) = V1_INSTS_MAP.get(&opcode) {
                self.read_opers(templ)?;
            }
            if (opcode == 0x001b || opcode == 0x00f4)
                && self.largest_code_address_pperand_encountered < self.buf.pos - self.offset
            {
                break;
            }
            if opcode == 0x007e || opcode == 0x007f || opcode == 0x00fe {
                self.output_internal_strings();
            }
        }
        self.output_internal_strings();
        Ok(())
    }

    pub fn output_internal_strings(&mut self) {
        loop {
            match self.stacks.pop() {
                Some(stack) => {
                    self.strings.push(BGIString {
                        offset: stack.offset,
                        address: stack.value,
                        typ: BGIStringType::Internal,
                    });
                }
                None => break,
            }
        }
    }
}
