use crate::ext::io::*;
use anyhow::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Oper {
    /// Byte
    B,
    /// Integer
    I,
    /// Address
    A,
    /// String
    S,
    /// Text
    T,
}

use Oper::*;

pub struct Opcodes {
    pub r#yield: u8,
    pub add: u8,
    pub escape_sequence: u8,
    pub message1: u8,
    pub message2: u8,
    pub push_int: u8,
    pub push_string: u8,
    pub syscall: u8,
    pub line_number: u8,
    pub nop1: u8,
    pub nop2: u8,
    pub is_message1_obfuscated: bool,
}

pub struct Syscalls {
    pub exec: i32,
    pub exec_set_character_name: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlikyStringType {
    Internal,
    Message,
    Name,
}

#[derive(Debug, Clone)]
pub struct SlikyString {
    pub start: u64,
    pub len: u64,
    pub typ: SlikyStringType,
}

#[derive(Debug, Clone)]
pub enum Obj {
    Byte(u8),
    Int(i32),
    Str(SlikyString),
}

pub trait Disasm: std::fmt::Debug {
    fn stream(&self) -> &MemReader;
    fn stream_mut(&mut self) -> &mut MemReader;
    fn opcodes(&self) -> &'static Opcodes;
    fn operands(&self) -> &'static [(u8, &'static [Oper])];
    fn syscalls(&self) -> &'static [Syscalls];
    fn code_offset(&self) -> u32;
    fn big_endian_addresses(&self) -> &[u32];
    fn push_big_endian_addresses(&mut self, addr: u32);
    fn little_endian_addresses(&self) -> &[u32];
    fn read_header(&mut self) -> Result<()>;
    fn read_instruction(&mut self) -> Result<(u8, Vec<Obj>)> {
        let opcode = self.stream_mut().read_u8()?;
        let mut operands = Vec::new();
        if let Some((_, ops)) = self.operands().iter().find(|(op, _)| *op == opcode) {
            for &oper in *ops {
                operands.push(self.read_operand(oper)?);
            }
        }
        Ok((opcode, operands))
    }
    fn read_operand(&mut self, oper: Oper) -> Result<Obj> {
        match oper {
            B => Ok(Obj::Byte(self.stream_mut().read_u8()?)),
            I => Ok(Obj::Int(self.stream_mut().read_i32_be()?)),
            A => {
                self.push_big_endian_addresses(self.stream().pos as u32);
                Ok(Obj::Int(self.stream_mut().read_i32_be()?))
            }
            S | T => {
                let start = self.stream().pos as u64;
                let s = self.stream_mut().read_cstring()?;
                Ok(Obj::Str(SlikyString {
                    start,
                    len: s.as_bytes_with_nul().len() as u64,
                    typ: SlikyStringType::Internal,
                }))
            }
        }
    }
    fn read_code(&mut self) -> Result<Vec<SlikyString>> {
        let mut stack: Vec<Obj> = Vec::new();
        let mut message_start_offset = None;
        let mut in_ruby = false;
        let mut texts = Vec::new();
        self.stream_mut().pos = self.code_offset() as usize;
        while !self.stream().is_eof() {
            let instr_offset = self.stream().pos as u64;
            let (opcode, operands) = self.read_instruction()?;
            // message instr
            let opcodes = self.opcodes();
            if opcode == opcodes.message1 || opcode == opcodes.message2 {
                if message_start_offset.is_none() {
                    message_start_offset = Some(instr_offset);
                }
            } else if opcode == opcodes.escape_sequence {
                if let Some(Obj::Byte(b)) = operands.get(0) {
                    if *b == 0x1 {
                        in_ruby = true;
                    }
                }
            } else if opcode == opcodes.r#yield && in_ruby {
                in_ruby = false;
            } else if opcode == opcodes.push_int
                && self.stream().cpeek_u8_at(instr_offset + 5)? == opcodes.line_number
            {
                // Skip
            } else if opcode == opcodes.line_number
                || opcode == opcodes.nop1
                || opcode == opcodes.nop2
            {
                // Skip
            } else {
                if let Some(start) = message_start_offset {
                    let start = start as u64;
                    let text = SlikyString {
                        start,
                        len: instr_offset - start,
                        typ: SlikyStringType::Message,
                    };
                    texts.push(text);
                }
                message_start_offset = None;
                in_ruby = false;
            }
            // name instr
            if opcode == opcodes.push_int || opcode == opcodes.push_string {
                if !operands.is_empty() {
                    stack.push(operands[0].clone());
                }
            } else if opcode == opcodes.add && stack.len() >= 2 {
                let value1 = stack.pop().unwrap();
                let value2 = stack.pop().unwrap();
                if let (Obj::Int(i1), Obj::Int(i2)) = (value1, value2) {
                    stack.push(Obj::Int(i1 + i2));
                }
            } else if opcode == opcodes.syscall && stack.len() >= 3 {
                let func_id = stack.pop().unwrap();
                let exec_id = stack.pop().unwrap();
                let name = stack.pop().unwrap();
                if let (Obj::Int(func_id), Obj::Int(exec_id), Obj::Str(name)) =
                    (func_id, exec_id, name)
                {
                    for syscall in self.syscalls() {
                        if func_id == syscall.exec && exec_id == syscall.exec_set_character_name {
                            texts.push(SlikyString {
                                start: name.start - 1,
                                len: name.len + 1,
                                typ: SlikyStringType::Name,
                            });
                        }
                    }
                }
                stack.clear();
            } else {
                stack.clear();
            }
        }
        Ok(texts)
    }
}

pub const PLUS_OPCODES: Opcodes = Opcodes {
    r#yield: 0x00,
    add: 0x34,
    escape_sequence: 0x1c,
    message1: 0x0a,
    message2: 0x0b,
    push_int: 0x32,
    push_string: 0x33,
    syscall: 0x18,
    line_number: 0xff,
    nop1: 0xfc,
    nop2: 0xfd,
    is_message1_obfuscated: true,
};

const PLUS_OPERANDS: [(u8, &[Oper]); 53] = [
    (0x00, &[]),  // yield
    (0x01, &[]),  // ret
    (0x02, &[]),  // ldglob1.i8
    (0x03, &[]),  // ldglob2.i16
    (0x04, &[]),  // ldglob3.var
    (0x05, &[]),  // ldglob4.var
    (0x06, &[]),  // ldloc.var
    (0x07, &[]),  // ldglob5.i8
    (0x08, &[]),  // ldglob5.i16
    (0x09, &[]),  // ldglob5.i32
    (0x0A, &[S]), // message
    (0x0B, &[T]), // message
    (0x0C, &[]),  // stglob1.i8
    (0x0D, &[]),  // stglob2.i16
    (0x0E, &[]),  // stglob3.var
    (0x0F, &[]),  // stglob4.var
    (0x10, &[]),  // stloc.var
    (0x11, &[]),  // stglob5.i8
    (0x12, &[]),  // stglob5.i16
    (0x13, &[]),  // stglob5.i32
    (0x14, &[A]), // jz
    (0x15, &[A]), // jmp
    (0x16, &[A]), // libreg
    (0x17, &[]),  // libcall
    (0x18, &[]),  // syscall
    (0x19, &[I]), // msgid
    (0x1A, &[I]), // msgid2
    (0x1B, &[A]), // choice
    (0x1C, &[B]), // escape sequence
    (0x32, &[I]), // ldc.i4
    (0x33, &[S]), // ldstr
    (0x34, &[]),  // add
    (0x35, &[]),  // sub
    (0x36, &[]),  // mul
    (0x37, &[]),  // div
    (0x38, &[]),  // mod
    (0x39, &[]),  // rand
    (0x3A, &[]),  // logand
    (0x3B, &[]),  // logor
    (0x3C, &[]),  // binand
    (0x3D, &[]),  // binor
    (0x3E, &[]),  // lt
    (0x3F, &[]),  // gt
    (0x40, &[]),  // le
    (0x41, &[]),  // ge
    (0x42, &[]),  // eq
    (0x43, &[]),  // neq
    (0xFA, &[]),
    (0xFB, &[]),
    (0xFC, &[]),
    (0xFD, &[]),
    (0xFE, &[]),
    (0xFF, &[]),
];

const PLUS_SYSCALLS: [Syscalls; 2] = [
    Syscalls {
        exec: 29,
        exec_set_character_name: 11,
    },
    Syscalls {
        exec: 29,
        exec_set_character_name: 15,
    },
];

#[derive(Debug)]
pub struct PlusDisasm {
    stream: MemReader,
    num_messages: u32,
    num_special_messages: u32,
    code_offset: u32,
    big_endian_addresses: Vec<u32>,
    little_endian_addresses: Vec<u32>,
}

impl PlusDisasm {
    pub fn new(mut stream: MemReader) -> Result<Self> {
        let num_messages = stream.read_u32()?;
        let num_special_messages = stream.read_u32()?;
        let code_offset = 8 + (num_messages + num_special_messages) * 4;
        Ok(Self {
            stream,
            num_messages,
            num_special_messages,
            code_offset,
            big_endian_addresses: Vec::new(),
            little_endian_addresses: Vec::new(),
        })
    }
}

impl Disasm for PlusDisasm {
    fn stream(&self) -> &MemReader {
        &self.stream
    }
    fn stream_mut(&mut self) -> &mut MemReader {
        &mut self.stream
    }
    fn opcodes(&self) -> &'static Opcodes {
        &PLUS_OPCODES
    }
    fn operands(&self) -> &'static [(u8, &'static [Oper])] {
        &PLUS_OPERANDS
    }
    fn syscalls(&self) -> &'static [Syscalls] {
        &PLUS_SYSCALLS
    }
    fn code_offset(&self) -> u32 {
        self.code_offset
    }
    fn big_endian_addresses(&self) -> &[u32] {
        &self.big_endian_addresses
    }
    fn push_big_endian_addresses(&mut self, addr: u32) {
        self.big_endian_addresses.push(addr);
    }
    fn little_endian_addresses(&self) -> &[u32] {
        &self.little_endian_addresses
    }
    fn read_header(&mut self) -> Result<()> {
        for i in 0..self.num_messages + self.num_special_messages {
            self.little_endian_addresses.push(8 + i * 4);
        }
        self.stream.pos = self.code_offset as usize;
        Ok(())
    }
}

const AI6_WIN_OPCODES: Opcodes = Opcodes {
    r#yield: 0x00,
    add: 0x34,
    escape_sequence: 0x1b,
    message1: 0x0a,
    message2: 0x0b,
    push_int: 0x32,
    push_string: 0x33,
    syscall: 0x18,
    line_number: 0xff,
    nop1: 0xfc,
    nop2: 0xfd,
    is_message1_obfuscated: false,
};

const AI6_WIN_OPERANDS: [(u8, &[Oper]); 48] = [
    (0x00, &[]),  // yield
    (0x01, &[]),  // ret
    (0x02, &[]),  // ldglob1.i8
    (0x03, &[]),  // ldglob2.i16
    (0x04, &[]),  // ldglob3.var
    (0x05, &[]),  // ldglob4.var
    (0x06, &[]),  // ldloc.var
    (0x07, &[]),  // ldglob5.i8
    (0x08, &[]),  // ldglob5.i16
    (0x09, &[]),  // ldglob5.i32
    (0x0A, &[S]), // message
    (0x0B, &[S]), // message
    (0x0C, &[]),  // stglob1.i8
    (0x0D, &[]),  // stglob2.i16
    (0x0E, &[]),  // stglob3.var
    (0x0F, &[]),  // stglob4.var
    (0x10, &[]),  // stloc.var
    (0x11, &[]),  // stglob5.i8
    (0x12, &[]),  // stglob5.i16
    (0x13, &[]),  // stglob5.i32
    (0x14, &[A]), // jz
    (0x15, &[A]), // jmp
    (0x16, &[A]), // libreg
    (0x17, &[]),  // libcall
    (0x18, &[]),  // syscall
    (0x19, &[I]), // msgid
    (0x1A, &[A]), // choice
    (0x1B, &[B]), // escape sequence
    (0x32, &[I]), // ldc.i4
    (0x33, &[S]), // ldstr
    (0x34, &[]),  // add
    (0x35, &[]),  // sub
    (0x36, &[]),  // mul
    (0x37, &[]),  // div
    (0x38, &[]),  // mod
    (0x39, &[]),  // rand
    (0x3A, &[]),  // logand
    (0x3B, &[]),  // logor
    (0x3C, &[]),  // binand
    (0x3D, &[]),  // binor
    (0x3E, &[]),  // lt
    (0x3F, &[]),  // gt
    (0x40, &[]),  // le
    (0x41, &[]),  // ge
    (0x42, &[]),  // eq
    (0x43, &[]),  // neq
    (0xFE, &[]),
    (0xFF, &[]),
];

const AI6_WIN_SYSCALLS: [Syscalls; 1] = [Syscalls {
    exec: 31,
    exec_set_character_name: 15,
}];

#[derive(Debug)]
pub struct Ai6WinDisasm {
    stream: MemReader,
    num_messages: u32,
    code_offset: u32,
    big_endian_addresses: Vec<u32>,
    little_endian_addresses: Vec<u32>,
}

impl Ai6WinDisasm {
    pub fn new(mut stream: MemReader) -> Result<Self> {
        let num_messages = stream.read_u32()?;
        let code_offset = 4 + num_messages * 4;
        Ok(Self {
            stream,
            num_messages,
            code_offset,
            big_endian_addresses: Vec::new(),
            little_endian_addresses: Vec::new(),
        })
    }
}

impl Disasm for Ai6WinDisasm {
    fn stream(&self) -> &MemReader {
        &self.stream
    }
    fn stream_mut(&mut self) -> &mut MemReader {
        &mut self.stream
    }
    fn opcodes(&self) -> &'static Opcodes {
        &AI6_WIN_OPCODES
    }
    fn operands(&self) -> &'static [(u8, &'static [Oper])] {
        &AI6_WIN_OPERANDS
    }
    fn syscalls(&self) -> &'static [Syscalls] {
        &AI6_WIN_SYSCALLS
    }
    fn code_offset(&self) -> u32 {
        self.code_offset
    }
    fn big_endian_addresses(&self) -> &[u32] {
        &self.big_endian_addresses
    }
    fn push_big_endian_addresses(&mut self, addr: u32) {
        self.big_endian_addresses.push(addr);
    }
    fn little_endian_addresses(&self) -> &[u32] {
        &self.little_endian_addresses
    }
    fn read_header(&mut self) -> Result<()> {
        for i in 0..self.num_messages {
            self.little_endian_addresses.push(4 + i * 4);
        }
        self.stream.pos = self.code_offset as usize;
        Ok(())
    }
}
