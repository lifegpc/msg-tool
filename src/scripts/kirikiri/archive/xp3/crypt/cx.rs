use super::*;
use crate::ext::mutex::MutexExt;
use crate::utils::files::*;
use crate::utils::struct_pack::*;
use anyhow::Result;
use chacha20::ChaCha20Legacy;
use serde::{Deserializer, de};
use std::collections::HashSet;
use std::ops::Index;
use std::path::PathBuf;
use std::sync::{Mutex, Weak};

const S_CTL_BLOCK_SIGNATURE: &[u8] = b" Encryption control block";

macro_rules! base_schema_impl {
    () => {
        fn hash_after_crypt(&self) -> bool {
            AsRef::<BaseSchema>::as_ref(self.as_ref()).hash_after_crypt
        }
        fn startup_tjs_not_encrypted(&self) -> bool {
            AsRef::<BaseSchema>::as_ref(self.as_ref()).startup_tjs_not_encrypted
        }
        fn obfuscated_index(&self) -> bool {
            AsRef::<BaseSchema>::as_ref(self.as_ref()).obfuscated_index
        }
    };
}

#[derive(Debug)]
pub struct CxEncryption {
    mask: u32,
    offset: u32,
    prolog_order: Vec<u8>,
    odd_branch_order: Vec<u8>,
    even_branch_order: Vec<u8>,
    control_block: Arc<Vec<u32>>,
    programs: Vec<Box<dyn ICxProgram + Send + Sync>>,
    program_builder: Box<dyn ICxProgramBuilder + Send + Sync>,
    base: BaseSchema,
}

trait ICxEncryption: std::fmt::Debug {
    fn get_base_offset(&self, hash: u32) -> u32;
    fn inner_decrypt(
        &self,
        mut key: u32,
        mut offset: u64,
        buffer: &mut [u8],
        mut pos: usize,
        mut count: usize,
    ) -> Result<()> {
        let base_offset = self.get_base_offset(key);
        if offset < base_offset as u64 {
            let base_length = ((base_offset as u64 - offset) as usize).min(count);
            self.decode(key, offset, buffer, pos, base_length)?;
            offset += base_length as u64;
            pos += base_length;
            count -= base_length;
        }
        if count > 0 {
            key = (key >> 16) ^ key;
            self.decode(key, offset, buffer, pos, count)?;
        }
        Ok(())
    }
    fn decode(
        &self,
        key: u32,
        offset: u64,
        buffer: &mut [u8],
        pos: usize,
        count: usize,
    ) -> Result<()>;
}

impl CxEncryption {
    pub fn new(base: BaseSchema, schema: &CxSchema, filename: &str) -> Result<Arc<Self>> {
        Ok(Arc::new(Self::new_inner(
            base,
            schema,
            filename,
            Box::new(CxProgramBuilder::default()),
        )?))
    }
    fn new_inner(
        base: BaseSchema,
        schema: &CxSchema,
        filename: &str,
        program_builder: Box<dyn ICxProgramBuilder + Send + Sync>,
    ) -> Result<Self> {
        if schema.prolog_order.len() != 3 {
            return Err(anyhow::anyhow!("Prolog order must have 3 elements"));
        }
        if schema.odd_branch_order.len() != 6 {
            return Err(anyhow::anyhow!("Odd branch order must have 6 elements"));
        }
        if schema.even_branch_order.len() != 8 {
            return Err(anyhow::anyhow!("Even branch order must have 8 elements"));
        }
        let control_block = if let Some(tpm_path) = &schema.tpm_file_name {
            Self::read_tpm(tpm_path, filename)?
        } else if let Some(control_block_name) = &schema.control_block_name {
            CX_CB_TABLE
                .get(control_block_name)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Control block not found in cx_cb.pck: {}",
                        control_block_name
                    )
                })?
                .clone()
        } else {
            return Err(anyhow::anyhow!(
                "TPM file name or control block is required in schema"
            ));
        };
        let control_block = Arc::new(control_block);
        let programs = Vec::with_capacity(0x80);
        let mut obj = Self {
            base,
            mask: schema.mask,
            offset: schema.offset,
            prolog_order: schema.prolog_order.bytes.clone(),
            odd_branch_order: schema.odd_branch_order.bytes.clone(),
            even_branch_order: schema.even_branch_order.bytes.clone(),
            control_block: control_block,
            programs,
            program_builder,
        };
        for seed in 0..0x80 {
            obj.programs.push(obj.generate_program(seed)?);
        }
        Ok(obj)
    }

    fn new_program(&self, seed: u32) -> Box<dyn ICxProgram + Send + Sync> {
        self.program_builder
            .build(seed, Arc::downgrade(&self.control_block))
    }

    fn generate_program(&self, seed: u32) -> Result<Box<dyn ICxProgram + Send + Sync>> {
        let mut program = self.new_program(seed);
        for stage in (1..=5).rev() {
            if self.emit_code(&mut program, stage) {
                return Ok(program);
            }
            program.clear();
        }
        Err(anyhow::anyhow!("Overly large CxEncryption bytecode"))
    }

    fn read_tpm(tpm_path: &str, filename: &str) -> Result<Vec<u32>> {
        let pfile = Self::get_tpm_path(tpm_path, filename)?;
        let tpm = std::fs::read(&pfile)?;
        let mut begin = 0;
        let end = (tpm.len() - 0x1000) & !0x3;
        while begin < end {
            if &tpm[begin..begin + S_CTL_BLOCK_SIGNATURE.len()] == S_CTL_BLOCK_SIGNATURE {
                let mut control_block = Vec::with_capacity(0x400);
                let mut reader = MemReaderRef::new(&tpm[begin..]);
                for _ in 0..0x400 {
                    control_block.push(!reader.read_u32()?);
                }
                return Ok(control_block);
            }
            begin += 4;
        }
        Err(anyhow::anyhow!(
            "Control block signature not found in TPM file: {}",
            pfile.display()
        ))
    }

    fn get_tpm_path(tpm_path: &str, filename: &str) -> Result<PathBuf> {
        let pb = PathBuf::from(filename);
        let pdir = pb
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Invalid TPM path"))?;
        let pfile = pdir.join(tpm_path);
        if pfile.is_file() {
            return Ok(pfile);
        }
        let pfile = pdir.join("..").join(tpm_path);
        if pfile.is_file() {
            return Ok(pfile);
        }
        Err(anyhow::anyhow!("TPM file not found: {}", tpm_path))
    }

    fn emit_code(&self, program: &mut Box<dyn ICxProgram + Send + Sync>, stage: i32) -> bool {
        program.emit_nop(5)
            && program.emit(MovEdiArg, 4)
            && self.emit_body(program, stage)
            && program.emit_nop(5)
            && program.emit(Retn, 1)
    }

    fn emit_body(&self, program: &mut Box<dyn ICxProgram + Send + Sync>, stage: i32) -> bool {
        if stage == 1 {
            return self.emit_prolog(program);
        }
        if !program.emit(PushEbx, 1) {
            return false;
        }
        if (program.get_random() & 1) != 0 {
            if !self.emit_body(program, stage - 1) {
                return false;
            }
        } else {
            if !self.emit_body2(program, stage - 1) {
                return false;
            }
        }
        if !program.emit(MovEbxEax, 2) {
            return false;
        }
        if (program.get_random() & 1) != 0 {
            if !self.emit_body(program, stage - 1) {
                return false;
            }
        } else {
            if !self.emit_body2(program, stage - 1) {
                return false;
            }
        }
        self.emit_odd_branch(program) && program.emit(PopEbx, 1)
    }

    fn emit_body2(&self, program: &mut Box<dyn ICxProgram + Send + Sync>, stage: i32) -> bool {
        if stage == 1 {
            return self.emit_prolog(program);
        }
        let r = if (program.get_random() & 1) != 0 {
            self.emit_body(program, stage - 1)
        } else {
            self.emit_body2(program, stage - 1)
        };
        r && self.emit_even_branch(program)
    }
    fn emit_prolog(&self, program: &mut Box<dyn ICxProgram + Send + Sync>) -> bool {
        match self.prolog_order[(program.get_random() % 3) as usize] {
            2 => {
                program.emit_nop(5)
                    && program.emit(MovEaxImmed, 2)
                    && {
                        let random = program.get_random() & 0x3ff;
                        program.emit_u32(random)
                    }
                    && program.emit(MovEaxIndirect, 0)
            }
            1 => program.emit(MovEaxEdi, 2),
            0 => program.emit(MovEaxImmed, 1) && program.emit_random(),
            _ => true,
        }
    }

    fn emit_even_branch(&self, program: &mut Box<dyn ICxProgram + Send + Sync>) -> bool {
        match self.even_branch_order[(program.get_random() & 7) as usize] {
            0 => program.emit(NotEax, 2),
            1 => program.emit(DecEax, 1),
            2 => program.emit(NegEax, 2),
            3 => program.emit(IncEax, 1),
            4 => {
                program.emit_nop(5)
                    && program.emit(AndEaxImmed, 1)
                    && program.emit_u32(0x3ff)
                    && program.emit(MovEaxIndirect, 3)
            }
            5 => {
                program.emit(PushEbx, 1)
                    && program.emit(MovEbxEax, 2)
                    && program.emit(AndEbxImmed, 2)
                    && program.emit_u32(0xaaaaaaaa)
                    && program.emit(AndEaxImmed, 1)
                    && program.emit_u32(0x55555555)
                    && program.emit(ShrEbx1, 2)
                    && program.emit(ShlEax1, 2)
                    && program.emit(OrEaxEbx, 2)
                    && program.emit(PopEbx, 1)
            }
            6 => program.emit(XorEaxImmed, 1) && program.emit_random(),
            7 => {
                let mut r = if (program.get_random() & 1) != 0 {
                    program.emit(AddEaxImmed, 1)
                } else {
                    program.emit(SubEaxImmed, 1)
                };
                r = r && program.emit_random();
                r
            }
            _ => true,
        }
    }

    fn emit_odd_branch(&self, program: &mut Box<dyn ICxProgram + Send + Sync>) -> bool {
        match self.odd_branch_order[(program.get_random() % 6) as usize] {
            0 => {
                program.emit(PushEcx, 1)
                    && program.emit(MovEcxEbx, 2)
                    && program.emit(AndEcx0F, 3)
                    && program.emit(ShrEaxCl, 2)
                    && program.emit(PopEcx, 1)
            }
            1 => {
                program.emit(PushEcx, 1)
                    && program.emit(MovEcxEbx, 2)
                    && program.emit(AndEcx0F, 3)
                    && program.emit(ShlEaxCl, 2)
                    && program.emit(PopEcx, 1)
            }
            2 => program.emit(AddEaxEbx, 2),
            3 => program.emit(NegEax, 2) && program.emit(AddEaxEbx, 2),
            4 => program.emit(ImulEaxEbx, 3),
            5 => program.emit(SubEaxEbx, 2),
            _ => true,
        }
    }

    fn execute_xcode(&self, mut hash: u32) -> Result<(u32, u32)> {
        let seed = hash & 0x7f;
        hash >>= 7;
        let program = &self.programs[seed as usize];
        let ret1 = program.execute(hash)?;
        let ret2 = program.execute(!hash)?;
        Ok((ret1, ret2))
    }
}

impl AsRef<BaseSchema> for CxEncryption {
    fn as_ref(&self) -> &BaseSchema {
        &self.base
    }
}

impl ICxEncryption for CxEncryption {
    fn get_base_offset(&self, hash: u32) -> u32 {
        (hash & self.mask).wrapping_add(self.offset)
    }

    fn decode(
        &self,
        key: u32,
        offset: u64,
        buffer: &mut [u8],
        pos: usize,
        count: usize,
    ) -> Result<()> {
        let ret = self.execute_xcode(key)?;
        let key1 = ret.1 >> 16;
        let mut key2 = ret.1 & 0xffff;
        let mut key3 = (ret.0 & 0xFF) as u8;
        if key1 == key2 {
            key2 = key2.wrapping_add(1);
        }
        if key3 == 0 {
            key3 = 1;
        }
        if (key2 as u64) >= offset && (key2 as u64) < offset + (count as u64) {
            buffer[pos + key2 as usize - offset as usize] ^= ((ret.0 >> 16) & 0xFF) as u8;
        }
        if (key1 as u64) >= offset && (key1 as u64) < offset + (count as u64) {
            buffer[pos + key1 as usize - offset as usize] ^= ((ret.0 >> 8) & 0xFF) as u8;
        }
        for i in 0..count {
            buffer[pos + i] ^= key3;
        }
        Ok(())
    }
}

macro_rules! icx_enc_arc_impl {
    ($t:ident) => {
        impl ICxEncryption for Arc<$t> {
            fn get_base_offset(&self, hash: u32) -> u32 {
                self.as_ref().get_base_offset(hash)
            }
            fn inner_decrypt(
                &self,
                key: u32,
                offset: u64,
                buffer: &mut [u8],
                pos: usize,
                count: usize,
            ) -> Result<()> {
                self.as_ref().inner_decrypt(key, offset, buffer, pos, count)
            }
            fn decode(
                &self,
                key: u32,
                offset: u64,
                buffer: &mut [u8],
                pos: usize,
                count: usize,
            ) -> Result<()> {
                self.as_ref().decode(key, offset, buffer, pos, count)
            }
        }
    };
}

macro_rules! icx_enc_impl {
    ($t:ident) => {
        impl ICxEncryption for $t {
            fn get_base_offset(&self, hash: u32) -> u32 {
                self.base.get_base_offset(hash)
            }
            fn inner_decrypt(
                &self,
                key: u32,
                offset: u64,
                buffer: &mut [u8],
                pos: usize,
                count: usize,
            ) -> Result<()> {
                self.base.inner_decrypt(key, offset, buffer, pos, count)
            }
            fn decode(
                &self,
                key: u32,
                offset: u64,
                buffer: &mut [u8],
                pos: usize,
                count: usize,
            ) -> Result<()> {
                self.base.decode(key, offset, buffer, pos, count)
            }
        }
    };
}

icx_enc_arc_impl!(CxEncryption);

impl Crypt for Arc<CxEncryption> {
    base_schema_impl!();
    fn decrypt_supported(&self) -> bool {
        true
    }
    fn decrypt_seek_supported(&self) -> bool {
        true
    }
    fn decrypt<'a>(
        &self,
        entry: &Xp3Entry,
        cur_seg: &Segment,
        stream: Box<dyn Read + Send + Sync + 'a>,
    ) -> Result<Box<dyn ReadDebug + Send + Sync + 'a>> {
        let key = (
            entry.file_hash,
            Box::new(self.clone()) as Box<dyn ICxEncryption + Send + Sync + 'a>,
        );
        Ok(Box::new(CxEncryptionReader::new(stream, cur_seg, key)))
    }
    fn decrypt_with_seek<'a>(
        &self,
        entry: &Xp3Entry,
        cur_seg: &Segment,
        stream: Box<dyn ReadSeek + Send + Sync + 'a>,
    ) -> Result<Box<dyn ReadSeek + Send + Sync + 'a>> {
        let key = (
            entry.file_hash,
            Box::new(self.clone()) as Box<dyn ICxEncryption + Send + Sync + 'a>,
        );
        Ok(Box::new(CxEncryptionReader::new(stream, cur_seg, key)))
    }
}

const CX_PROGRAM_SIZE: usize = 0x80;

#[derive(Debug)]
struct CxProgram {
    code: Vec<u32>,
    control_block: Weak<Vec<u32>>,
    length: usize,
    seed: u32,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, int_enum::IntEnum)]
enum CxByteCode {
    Nop,
    Retn,
    MovEdiArg,
    PushEbx,
    PopEbx,
    PushEcx,
    PopEcx,
    MovEaxEbx,
    MovEbxEax,
    MovEcxEbx,
    MovEaxControlBlock,
    MovEaxEdi,
    MovEaxIndirect,
    AddEaxEbx,
    SubEaxEbx,
    ImulEaxEbx,
    AndEcx0F,
    ShrEbx1,
    ShlEax1,
    ShrEaxCl,
    ShlEaxCl,
    OrEaxEbx,
    NotEax,
    NegEax,
    DecEax,
    IncEax,
    Immed = 0x100,
    MovEaxImmed,
    AndEbxImmed,
    AndEaxImmed,
    XorEaxImmed,
    AddEaxImmed,
    SubEaxImmed,
}

use CxByteCode::*;

#[derive(Debug, Default)]
struct Context {
    eax: u32,
    ebx: u32,
    ecx: u32,
    edi: u32,
    stack: Vec<u32>,
}

#[derive(Debug)]
struct CxProgramBuilder {}

impl Default for CxProgramBuilder {
    fn default() -> Self {
        Self {}
    }
}

trait ICxProgramBuilder: std::fmt::Debug {
    fn build(&self, seed: u32, control_blocks: Weak<Vec<u32>>)
    -> Box<dyn ICxProgram + Send + Sync>;
}

impl ICxProgramBuilder for CxProgramBuilder {
    fn build(
        &self,
        seed: u32,
        control_blocks: Weak<Vec<u32>>,
    ) -> Box<dyn ICxProgram + Send + Sync> {
        Box::new(CxProgram {
            code: Vec::with_capacity(CX_PROGRAM_SIZE),
            control_block: control_blocks,
            length: 0,
            seed,
        })
    }
}

trait ICxProgram: std::fmt::Debug {
    fn execute(&self, hash: u32) -> Result<u32>;
    fn clear(&mut self);
    fn emit_nop(&mut self, count: usize) -> bool;
    fn emit(&mut self, bytecode: CxByteCode, length: usize) -> bool;
    fn emit_u32(&mut self, x: u32) -> bool;
    fn emit_random(&mut self) -> bool {
        let random = self.get_random();
        self.emit_u32(random)
    }
    fn get_random(&mut self) -> u32;
}

impl ICxProgram for CxProgram {
    fn execute(&self, hash: u32) -> Result<u32> {
        let mut context = Context::default();
        let mut iterator = self.code.iter();
        let mut immed = 0u32;
        while let Some(code) = iterator.next() {
            let code = *code;
            const IMMED: u32 = Immed as u32;
            if IMMED == (code & IMMED) {
                immed = *iterator.next().ok_or_else(|| {
                    anyhow::anyhow!("Incomplete IMMED bytecode in CxEncryption program")
                })?;
            }
            let bytecode = CxByteCode::try_from(code).map_err(|_| {
                anyhow::anyhow!("Invalid bytecode in CxEncryption program: {:#X}", code)
            })?;
            match bytecode {
                Nop => {}
                Immed => {}
                MovEdiArg => {
                    context.edi = hash;
                }
                PushEbx => {
                    context.stack.push(context.ebx);
                }
                PopEbx => {
                    context.ebx = context.stack.pop().ok_or_else(|| {
                        anyhow::anyhow!("Stack underflow in CxEncryption program")
                    })?;
                }
                PushEcx => {
                    context.stack.push(context.ecx);
                }
                PopEcx => {
                    context.ecx = context.stack.pop().ok_or_else(|| {
                        anyhow::anyhow!("Stack underflow in CxEncryption program")
                    })?;
                }
                MovEbxEax => {
                    context.ebx = context.eax;
                }
                MovEaxEdi => {
                    context.eax = context.edi;
                }
                MovEcxEbx => {
                    context.ecx = context.ebx;
                }
                MovEaxEbx => {
                    context.eax = context.ebx;
                }
                AndEcx0F => {
                    context.ecx &= 0x0F;
                }
                ShrEbx1 => {
                    context.ebx >>= 1;
                }
                ShlEax1 => {
                    context.eax <<= 1;
                }
                ShrEaxCl => {
                    context.eax >>= context.ecx;
                }
                ShlEaxCl => {
                    context.eax <<= context.ecx;
                }
                OrEaxEbx => {
                    context.eax |= context.ebx;
                }
                NotEax => {
                    context.eax = !context.eax;
                }
                NegEax => {
                    context.eax = context.eax.wrapping_neg();
                }
                DecEax => {
                    context.eax = context.eax.wrapping_sub(1);
                }
                IncEax => {
                    context.eax = context.eax.wrapping_add(1);
                }
                AddEaxEbx => {
                    context.eax = context.eax.wrapping_add(context.ebx);
                }
                SubEaxEbx => {
                    context.eax = context.eax.wrapping_sub(context.ebx);
                }
                ImulEaxEbx => {
                    context.eax = context.eax.wrapping_mul(context.ebx);
                }
                AddEaxImmed => {
                    context.eax = context.eax.wrapping_add(immed);
                }
                SubEaxImmed => {
                    context.eax = context.eax.wrapping_sub(immed);
                }
                AndEbxImmed => {
                    context.ebx &= immed;
                }
                AndEaxImmed => {
                    context.eax &= immed;
                }
                XorEaxImmed => {
                    context.eax ^= immed;
                }
                MovEaxImmed => {
                    context.eax = immed;
                }
                MovEaxIndirect => {
                    let control_block = self
                        .control_block
                        .upgrade()
                        .ok_or_else(|| anyhow::anyhow!("Control block has been dropped"))?;
                    if context.eax as usize >= control_block.len() {
                        return Err(anyhow::anyhow!(
                            "Control block index out of bounds in CxEncryption program: {}",
                            context.eax
                        ));
                    }
                    context.eax = !control_block[context.eax as usize];
                }
                Retn => {
                    if context.stack.len() != 0 {
                        return Err(anyhow::anyhow!(
                            "Stack not empty at RETN in CxEncryption program"
                        ));
                    }
                    return Ok(context.eax);
                }
                _ => {
                    return Err(anyhow::anyhow!(
                        "Unsupported bytecode in CxEncryption program: {:?}",
                        bytecode
                    ));
                }
            }
        }
        Err(anyhow::anyhow!(
            "CxEncryption program without RETN bytecode"
        ))
    }

    fn clear(&mut self) {
        self.length = 0;
        self.code.clear();
    }

    fn emit_nop(&mut self, count: usize) -> bool {
        if self.length + count > CX_PROGRAM_SIZE {
            return false;
        }
        self.length += count;
        return true;
    }

    fn emit(&mut self, bytecode: CxByteCode, length: usize) -> bool {
        if self.length + length > CX_PROGRAM_SIZE {
            return false;
        }
        self.code.push(bytecode as u32);
        self.length += length;
        return true;
    }

    fn emit_u32(&mut self, x: u32) -> bool {
        if self.length + 4 > CX_PROGRAM_SIZE {
            return false;
        }
        self.code.push(x);
        self.length += 4;
        return true;
    }

    fn get_random(&mut self) -> u32 {
        let seed = self.seed;
        self.seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
        self.seed ^ (seed << 16) ^ (seed >> 16)
    }
}

#[derive(msg_tool_macro::MyDebug)]
struct CxEncryptionReader<'a, T> {
    #[skip_fmt]
    inner: T,
    seg_start: u64,
    seg_size: u64,
    pos: u64,
    key: (u32, Box<dyn ICxEncryption + Send + Sync + 'a>),
}

impl<'a, T: Read> CxEncryptionReader<'a, T> {
    pub fn new(
        inner: T,
        seg: &Segment,
        key: (u32, Box<dyn ICxEncryption + Send + Sync + 'a>),
    ) -> Self {
        Self {
            inner,
            seg_start: seg.offset_in_file,
            seg_size: seg.original_size,
            pos: 0,
            key,
        }
    }
}

impl<'a, T: Read + Seek> Seek for CxEncryptionReader<'a, T> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let new_pos: i64 = match pos {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::End(offset) => self.seg_size as i64 + offset,
            SeekFrom::Current(offset) => self.pos as i64 + offset,
        };
        let offset = new_pos - self.pos as i64;
        if offset != 0 {
            self.inner.seek(SeekFrom::Current(offset))?;
            self.pos = new_pos as u64;
        }
        Ok(self.pos)
    }
}

impl<'a, R: Read> Read for CxEncryptionReader<'a, R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let offset = self.seg_start + self.pos;
        let count = self.inner.read(buf)?;
        if count == 0 {
            return Ok(0);
        }
        let key = self.key.0;
        let cx = &self.key.1;
        if let Err(e) = cx.inner_decrypt(key, offset, buf, 0, count) {
            return Err(std::io::Error::new(std::io::ErrorKind::Other, e));
        }
        self.pos += count as u64;
        Ok(count)
    }
}

#[derive(Debug)]
pub struct SenrenCxCrypt {
    base: CxEncryption,
    names_section_id: String,
}

impl AsRef<BaseSchema> for SenrenCxCrypt {
    fn as_ref(&self) -> &BaseSchema {
        self.base.as_ref()
    }
}

impl SenrenCxCrypt {
    pub fn new(
        base: BaseSchema,
        schema: &CxSchema,
        filename: &str,
        names_section_id: String,
    ) -> Result<Arc<Self>> {
        Ok(Arc::new(Self::new_inner(
            base,
            schema,
            filename,
            Box::new(CxProgramBuilder::default()),
            names_section_id,
        )?))
    }
    fn new_inner(
        base: BaseSchema,
        schema: &CxSchema,
        filename: &str,
        program_builder: Box<dyn ICxProgramBuilder + Send + Sync>,
        names_section_id: String,
    ) -> Result<Self> {
        let cx = CxEncryption::new_inner(base, schema, filename, program_builder)?;
        Ok(Self {
            base: cx,
            names_section_id,
        })
    }

    fn read_yuzu_names<'a>(
        reader: Box<dyn ReadDebug + 'a>,
        unpacked_size: u32,
    ) -> Result<(HashMap<u32, String>, HashMap<String, String>)> {
        let mut decoded = MemWriter::with_capacity(unpacked_size as usize);
        {
            let mut decoder = flate2::read::ZlibDecoder::new(reader);
            std::io::copy(&mut decoder, &mut decoded)?;
        }
        let decoded = decoded.into_inner();
        let mut reader = MemReader::new(decoded);
        let mut hash_map = HashMap::new();
        let mut md5_map = HashMap::new();
        let mut dir_offset = 0u64;
        while !reader.is_eof() {
            let _entry_sign = reader.read_u32()?;
            let mut entry_size = reader.read_u64()?;
            dir_offset += 12 + entry_size;
            let hash = reader.read_u32()?;
            let name_len = reader.read_u16()?;
            entry_size -= 6;
            if (name_len as u64) * 2 <= entry_size {
                let name = reader.read_exact_vec((name_len) as usize * 2)?;
                let name = decode_to_string(Encoding::Utf16LE, &name, true)?;
                if !hash_map.contains_key(&hash) {
                    hash_map.insert(hash, name.clone());
                }
                let encoded = encode_string(Encoding::Utf16LE, &name.to_ascii_lowercase(), true)?;
                let md5 = format!("{:x}", md5::compute(encoded));
                md5_map.insert(md5, name);
            }
            reader.pos = dir_offset as usize;
            md5_map.insert("$".into(), "startup.tjs".into());
        }
        Ok((hash_map, md5_map))
    }
}

fn read_yuzu_names<'a, T>(
    archive: &mut Xp3Archive<'a>,
    names_section_id: &str,
    convert: T,
) -> Result<()>
where
    T: FnOnce(
        Box<dyn ReadDebug + 'a>,
        u32,
    ) -> Result<(HashMap<u32, String>, HashMap<String, String>)>,
{
    if let Some(section) = archive.extras.iter().find(|s| s.tag == names_section_id) {
        let mut sreader = MemReaderRef::new(&section.data);
        let offset = sreader.read_u64()? + archive.base_offset;
        let unpacked_size = sreader.read_u32()?;
        let packed_size = sreader.read_u32()?;
        let index_stream =
            MutexWrapper::new(archive.inner.clone(), offset).take(packed_size as u64);
        let (hash_map, md5_map) = convert(Box::new(index_stream), unpacked_size)?;
        for entry in archive.entries.iter_mut() {
            if let Some(name) = hash_map.get(&entry.file_hash) {
                entry.name = name.clone();
            } else if let Some(name) = md5_map.get(&entry.name) {
                entry.name = name.clone();
            }
        }
    }
    archive.extras.retain(|s| s.tag != names_section_id);
    Ok(())
}

icx_enc_impl!(SenrenCxCrypt);
icx_enc_arc_impl!(SenrenCxCrypt);

impl Crypt for Arc<SenrenCxCrypt> {
    base_schema_impl!();
    fn init(&self, archive: &mut Xp3Archive) -> Result<()> {
        default_init_crypt(archive)?;
        read_yuzu_names(
            archive,
            &self.names_section_id,
            SenrenCxCrypt::read_yuzu_names,
        )
    }
    fn decrypt_supported(&self) -> bool {
        true
    }
    fn decrypt_seek_supported(&self) -> bool {
        true
    }
    fn decrypt<'a>(
        &self,
        entry: &Xp3Entry,
        cur_seg: &Segment,
        stream: Box<dyn Read + Send + Sync + 'a>,
    ) -> Result<Box<dyn ReadDebug + Send + Sync + 'a>> {
        let key = (
            entry.file_hash,
            Box::new(self.clone()) as Box<dyn ICxEncryption + Send + Sync + 'a>,
        );
        Ok(Box::new(CxEncryptionReader::new(stream, cur_seg, key)))
    }
    fn decrypt_with_seek<'a>(
        &self,
        entry: &Xp3Entry,
        cur_seg: &Segment,
        stream: Box<dyn ReadSeek + Send + Sync + 'a>,
    ) -> Result<Box<dyn ReadSeek + Send + Sync + 'a>> {
        let key = (
            entry.file_hash,
            Box::new(self.clone()) as Box<dyn ICxEncryption + Send + Sync + 'a>,
        );
        Ok(Box::new(CxEncryptionReader::new(stream, cur_seg, key)))
    }
}

#[derive(Debug)]
struct CxProgramNana {
    base: CxProgram,
    random_seed: u32,
}

impl CxProgramNana {
    fn new(seed: u32, control_blocks: Weak<Vec<u32>>, random_seed: u32) -> Self {
        Self {
            base: CxProgram {
                code: Vec::with_capacity(CX_PROGRAM_SIZE),
                control_block: control_blocks,
                length: 0,
                seed,
            },
            random_seed,
        }
    }
}

impl ICxProgram for CxProgramNana {
    fn execute(&self, hash: u32) -> Result<u32> {
        self.base.execute(hash)
    }
    fn clear(&mut self) {
        self.base.clear();
    }
    fn emit(&mut self, bytecode: CxByteCode, length: usize) -> bool {
        self.base.emit(bytecode, length)
    }
    fn emit_nop(&mut self, count: usize) -> bool {
        self.base.emit_nop(count)
    }
    fn emit_u32(&mut self, x: u32) -> bool {
        self.base.emit_u32(x)
    }
    fn get_random(&mut self) -> u32 {
        let mut s = self.base.seed ^ (self.base.seed << 17);
        s ^= (s << 18) | (s >> 15);
        self.base.seed = !s;
        let mut r = self.random_seed ^ (self.random_seed << 13);
        r ^= r >> 17;
        self.random_seed = r ^ (r << 5);
        self.base.seed ^ self.random_seed
    }
}

#[derive(Debug)]
struct CxProgramNanaBuilder {
    random_seed: u32,
}

impl CxProgramNanaBuilder {
    fn new(random_seed: u32) -> Self {
        Self { random_seed }
    }
}

impl ICxProgramBuilder for CxProgramNanaBuilder {
    fn build(
        &self,
        seed: u32,
        control_blocks: Weak<Vec<u32>>,
    ) -> Box<dyn ICxProgram + Send + Sync> {
        Box::new(CxProgramNana::new(seed, control_blocks, self.random_seed))
    }
}

#[derive(Debug)]
pub struct CabbageCxCrypt {
    base: SenrenCxCrypt,
}

impl AsRef<BaseSchema> for CabbageCxCrypt {
    fn as_ref(&self) -> &BaseSchema {
        self.base.as_ref()
    }
}

impl CabbageCxCrypt {
    pub fn new(
        base: BaseSchema,
        schema: &CxSchema,
        filename: &str,
        names_section_id: String,
        random_seed: u32,
    ) -> Result<Arc<Self>> {
        Ok(Arc::new(Self {
            base: SenrenCxCrypt::new_inner(
                base,
                schema,
                filename,
                Box::new(CxProgramNanaBuilder::new(random_seed)),
                names_section_id,
            )?,
        }))
    }
}

icx_enc_impl!(CabbageCxCrypt);
icx_enc_arc_impl!(CabbageCxCrypt);

impl Crypt for Arc<CabbageCxCrypt> {
    base_schema_impl!();
    fn init(&self, archive: &mut Xp3Archive) -> Result<()> {
        default_init_crypt(archive)?;
        read_yuzu_names(
            archive,
            &self.base.names_section_id,
            SenrenCxCrypt::read_yuzu_names,
        )
    }
    fn decrypt_supported(&self) -> bool {
        true
    }
    fn decrypt_seek_supported(&self) -> bool {
        true
    }
    fn decrypt<'a>(
        &self,
        entry: &Xp3Entry,
        cur_seg: &Segment,
        stream: Box<dyn Read + Send + Sync + 'a>,
    ) -> Result<Box<dyn ReadDebug + Send + Sync + 'a>> {
        let key = (
            entry.file_hash,
            Box::new(self.clone()) as Box<dyn ICxEncryption + Send + Sync + 'a>,
        );
        Ok(Box::new(CxEncryptionReader::new(stream, cur_seg, key)))
    }
    fn decrypt_with_seek<'a>(
        &self,
        entry: &Xp3Entry,
        cur_seg: &Segment,
        stream: Box<dyn ReadSeek + Send + Sync + 'a>,
    ) -> Result<Box<dyn ReadSeek + Send + Sync + 'a>> {
        let key = (
            entry.file_hash,
            Box::new(self.clone()) as Box<dyn ICxEncryption + Send + Sync + 'a>,
        );
        Ok(Box::new(CxEncryptionReader::new(stream, cur_seg, key)))
    }
}

#[derive(Debug)]
struct NanaDecryptor {
    state: [u32; 27],
    seed: u64,
}

impl NanaDecryptor {
    fn new(key: &[u32], seed1: u32, seed2: u32) -> Self {
        let mut state = [0u32; 27];
        let seed = (seed2 as u64) << 32 | (seed1 as u64);
        let mut s = [0u32; 3];
        let mut k = key[0];
        s[0] = key[1];
        s[1] = key[2];
        s[2] = key[3];
        state[0] = k;
        let mut dst = 1;
        for i in 0..26usize {
            let src = i % 3;
            let m = s[src].rotate_right(8);
            let n = (i as u32) ^ k.wrapping_add(m);
            k = n ^ k.rotate_left(3);
            state[dst] = k;
            dst += 1;
            s[src] = n;
        }
        Self { state, seed }
    }

    fn decrypt(&self, data: &mut [u8]) {
        let mut i = 0;
        let mut offset = 0;
        let mut length = data.len();
        while length > 0 {
            offset += 1;
            let mut key = self.transform_key(offset ^ self.seed);
            let count = std::cmp::min(length, 8);
            for _ in 0..count {
                data[i] ^= (key & 0xFF) as u8;
                key >>= 8;
                i += 1;
            }
            length -= count;
        }
    }

    fn transform_key(&self, key: u64) -> u64 {
        let mut lo = (key & 0xFFFFFFFF) as u32;
        let mut hi = (key >> 32) as u32;
        for i in 0..27 {
            hi = hi.rotate_right(8);
            hi = hi.wrapping_add(lo);
            hi ^= self.state[i];
            lo = lo.rotate_left(3);
            lo ^= hi;
        }
        (hi as u64) << 32 | (lo as u64)
    }
}

#[derive(Debug)]
pub struct NanaCxCrypt {
    base: SenrenCxCrypt,
    decryptor: NanaDecryptor,
}

impl AsRef<BaseSchema> for NanaCxCrypt {
    fn as_ref(&self) -> &BaseSchema {
        self.base.as_ref()
    }
}

impl NanaCxCrypt {
    pub fn new(
        base: BaseSchema,
        schema: &CxSchema,
        filename: &str,
        names_section_id: String,
        random_seed: u32,
        yuz_key: &[u32],
    ) -> Result<Arc<Self>> {
        if yuz_key.len() != 6 {
            return Err(anyhow::anyhow!(
                "Invalid Yuzu keys for NanaCxCrypt: expected 6, got {}",
                yuz_key.len()
            ));
        }
        let cx = SenrenCxCrypt::new_inner(
            base,
            schema,
            filename,
            Box::new(CxProgramNanaBuilder::new(random_seed)),
            names_section_id,
        )?;
        let decryptor = NanaDecryptor::new(yuz_key, yuz_key[4], yuz_key[5]);
        Ok(Arc::new(Self {
            base: cx,
            decryptor,
        }))
    }

    fn read_yuzu_names<'a>(
        &self,
        mut reader: Box<dyn ReadDebug + 'a>,
        unpacked_size: u32,
    ) -> Result<(HashMap<u32, String>, HashMap<String, String>)> {
        let mut prefix = Vec::with_capacity(0x100);
        (&mut reader).take(0x100).read_to_end(&mut prefix)?;
        self.decryptor.decrypt(&mut prefix);
        let reader = Box::new(PrefixStream::new(prefix, reader));
        SenrenCxCrypt::read_yuzu_names(reader, unpacked_size)
    }
}

icx_enc_impl!(NanaCxCrypt);
icx_enc_arc_impl!(NanaCxCrypt);

impl Crypt for Arc<NanaCxCrypt> {
    base_schema_impl!();
    fn init(&self, archive: &mut Xp3Archive) -> Result<()> {
        default_init_crypt(archive)?;
        read_yuzu_names(
            archive,
            &self.base.names_section_id,
            |reader, unpacked_size| self.read_yuzu_names(reader, unpacked_size),
        )
    }
    fn decrypt_supported(&self) -> bool {
        true
    }
    fn decrypt_seek_supported(&self) -> bool {
        true
    }
    fn decrypt<'a>(
        &self,
        entry: &Xp3Entry,
        cur_seg: &Segment,
        stream: Box<dyn Read + Send + Sync + 'a>,
    ) -> Result<Box<dyn ReadDebug + Send + Sync + 'a>> {
        let key = (
            entry.file_hash,
            Box::new(self.clone()) as Box<dyn ICxEncryption + Send + Sync + 'a>,
        );
        Ok(Box::new(CxEncryptionReader::new(stream, cur_seg, key)))
    }
    fn decrypt_with_seek<'a>(
        &self,
        entry: &Xp3Entry,
        cur_seg: &Segment,
        stream: Box<dyn ReadSeek + Send + Sync + 'a>,
    ) -> Result<Box<dyn ReadSeek + Send + Sync + 'a>> {
        let key = (
            entry.file_hash,
            Box::new(self.clone()) as Box<dyn ICxEncryption + Send + Sync + 'a>,
        );
        Ok(Box::new(CxEncryptionReader::new(stream, cur_seg, key)))
    }
}

#[derive(Debug)]
struct YuzDecryptor {
    state: [u8; 64],
}

impl YuzDecryptor {
    fn new(key1: &[u32], key2: &[u32], seed1: u32, seed2: u32) -> Self {
        let mut state = [0u8; 64];
        for i in 0..4 {
            state[i * 4..i * 4 + 4].copy_from_slice(&key2[i].to_le_bytes());
        }
        for i in 0..8 {
            state[i * 4 + 16..i * 4 + 20].copy_from_slice(&key1[i].to_le_bytes());
        }
        let t: u32 = !0;
        state[48..52].copy_from_slice(&t.to_le_bytes());
        state[52..56].copy_from_slice(&t.to_le_bytes());
        state[56..60].copy_from_slice(&(!seed1).to_le_bytes());
        state[60..64].copy_from_slice(&(!seed2).to_le_bytes());
        Self { state }
    }

    fn decrypt(&self, data: &mut [u8]) {
        let mut state1 = [0u8; 64];
        let mut state2 = [0u8; 64];
        let mut i = 0;
        let mut offset: u64 = 0;
        let mut length = data.len();
        while length > 0 {
            state1.copy_from_slice(&self.state);
            state1[48..56].copy_from_slice(&(!offset).to_le_bytes());
            offset += 1;
            Self::transform_state(&state1, &mut state2, 8);
            let count = length.min(0x40);
            for j in 0..count {
                data[i] ^= state2[j];
                i += 1;
            }
            length -= count;
        }
    }

    fn transform_state(state: &[u8], target: &mut [u8], length: usize) {
        let mut tmp = [0u32; 16];
        for i in 0..16 {
            tmp[i] = !u32::from_le_bytes([
                state[i * 4],
                state[i * 4 + 1],
                state[i * 4 + 2],
                state[i * 4 + 3],
            ]);
        }
        if length > 0 {
            for _ in 0..((length - 1) >> 1) + 1 {
                let mut t1 = w!(tmp[4] + tmp[0]);
                let mut t2 = (t1 ^ tmp[12]).rotate_left(16);
                let mut t3 = w!(t2 + tmp[8]);
                let mut t4 = (tmp[4] ^ t3).rotate_left(12);
                let mut t5 = w!(t4 + t1);
                let mut t6 = (t5 ^ t2).rotate_left(8);
                tmp[12] = t6;
                w!(t6 += t3);
                tmp[4] = (t4 ^ t6).rotate_left(7);
                t4 = (w!(tmp[5] + tmp[1]) ^ tmp[13]).rotate_left(16);
                t3 = (tmp[5] ^ w!(t4 + tmp[9])).rotate_left(12);
                t2 = w!(t3 + tmp[5] + tmp[1]);
                tmp[13] = (t2 ^ t4).rotate_left(8);
                w!(tmp[9] += tmp[13] + t4);
                tmp[5] = (t3 ^ tmp[9]).rotate_left(7);
                t4 = (w!(tmp[6] + tmp[2]) ^ tmp[14]).rotate_left(16);
                w!(tmp[10] += t4);
                t1 = (tmp[6] ^ tmp[10]).rotate_left(12);
                t3 = w!(t1 + tmp[6] + tmp[2]);
                tmp[14] = (t3 ^ t4).rotate_left(8);
                tmp[6] = (t1 ^ w!(tmp[14] + tmp[10])).rotate_left(7);
                w!(tmp[10] += tmp[14]);
                t4 = w!(tmp[7] + tmp[3]) ^ tmp[15];
                w!(tmp[3] += tmp[7]);
                t4 = t4.rotate_left(16);
                w!(tmp[11] += t4);
                t1 = (tmp[7] ^ tmp[11]).rotate_left(12);
                t4 ^= w!(t1 + tmp[3]);
                w!(tmp[3] += t1);
                t4 = t4.rotate_left(8);
                w!(tmp[11] += t4);
                t1 = (t1 ^ tmp[11]).rotate_left(7);
                w!(t5 += tmp[5]);
                w!(t2 += tmp[6]);
                t4 = (t5 ^ t4).rotate_left(16);
                w!(tmp[10] += t4);
                tmp[5] = (tmp[5] ^ tmp[10]).rotate_left(12);
                tmp[0] = w!(tmp[5] + t5);
                t4 = (tmp[0] ^ t4).rotate_left(8);
                tmp[15] = t4;
                w!(tmp[10] += t4);
                tmp[5] = (tmp[5] ^ tmp[10]).rotate_left(7);
                tmp[12] = (tmp[12] ^ t2).rotate_left(16);
                w!(tmp[11] += tmp[12]);
                t4 = (tmp[11] ^ tmp[6]).rotate_left(12);
                tmp[1] = w!(t4 + t2);
                tmp[12] = (tmp[12] ^ tmp[1]).rotate_left(8);
                w!(tmp[11] += tmp[12]);
                tmp[6] = (t4 ^ tmp[11]).rotate_left(7);
                w!(t3 += t1);
                t4 = (tmp[13] ^ t3).rotate_left(16);
                t2 = w!(t4 + t6);
                t1 = (t2 ^ t1).rotate_left(12);
                tmp[2] = w!(t1 + t3);
                tmp[13] = (t4 ^ tmp[2]).rotate_left(8);
                tmp[8] = w!(tmp[13] + t2);
                tmp[7] = (tmp[8] ^ t1).rotate_left(7);
                t6 = (tmp[14] ^ w!(tmp[4] + tmp[3])).rotate_left(16);
                t1 = (tmp[4] ^ w!(t6 + tmp[9])).rotate_left(12);
                w!(tmp[3] += t1 + tmp[4]);
                t3 = (t6 ^ tmp[3]).rotate_left(8);
                w!(tmp[9] += t3 + t6);
                tmp[4] = (t1 ^ tmp[9]).rotate_left(7);
                tmp[14] = t3;
            }
        }
        let mut pos = 0;
        for i in 0..16 {
            let d =
                !u32::from_le_bytes([state[pos], state[pos + 1], state[pos + 2], state[pos + 3]]);
            let d = w!(tmp[i] + d);
            target[pos..pos + 4].copy_from_slice(&d.to_le_bytes());
            pos += 4;
        }
    }
}

#[derive(Debug)]
pub struct RiddleCxCrypt {
    base: SenrenCxCrypt,
    decryptor: YuzDecryptor,
    key1: u32,
    key2: u32,
}

impl AsRef<BaseSchema> for RiddleCxCrypt {
    fn as_ref(&self) -> &BaseSchema {
        self.base.as_ref()
    }
}

impl RiddleCxCrypt {
    pub fn new(
        base: BaseSchema,
        schema: &CxSchema,
        filename: &str,
        names_section_id: String,
        random_seed: u32,
        yuz_key: &[u32],
        key1: u32,
        key2: u32,
    ) -> Result<Arc<Self>> {
        if yuz_key.len() != 6 {
            return Err(anyhow::anyhow!(
                "Invalid Yuzu keys for RiddleCxCrypt: expected 6, got {}",
                yuz_key.len()
            ));
        }
        let cx = SenrenCxCrypt::new_inner(
            base,
            schema,
            filename,
            Box::new(CxProgramNanaBuilder::new(random_seed)),
            names_section_id,
        )?;
        let control_block = cx.base.control_block.as_ref();
        let decryptor = YuzDecryptor::new(&control_block, yuz_key, yuz_key[4], yuz_key[5]);
        Ok(Arc::new(Self {
            base: cx,
            decryptor,
            key1,
            key2,
        }))
    }

    fn get_key_from_hash(&self, key: u32) -> u64 {
        let lo = key ^ self.key2;
        let mut hi = (key << 13) ^ key;
        hi ^= hi >> 17;
        hi ^= (hi << 5) ^ self.key1;
        ((hi as u64) << 32) | (lo as u64)
    }

    fn read_yuzu_names<'a>(
        &self,
        mut reader: Box<dyn ReadDebug + 'a>,
        unpacked_size: u32,
    ) -> Result<(HashMap<u32, String>, HashMap<String, String>)> {
        let mut prefix = Vec::with_capacity(0x100);
        (&mut reader).take(0x100).read_to_end(&mut prefix)?;
        self.decryptor.decrypt(&mut prefix);
        let reader = Box::new(PrefixStream::new(prefix, reader));
        SenrenCxCrypt::read_yuzu_names(reader, unpacked_size)
    }
}

impl ICxEncryption for RiddleCxCrypt {
    fn get_base_offset(&self, hash: u32) -> u32 {
        self.base.get_base_offset(hash)
    }
    fn inner_decrypt(
        &self,
        key: u32,
        offset: u64,
        buffer: &mut [u8],
        pos: usize,
        count: usize,
    ) -> Result<()> {
        if offset < 8 && count > 0 {
            let mut key = self.get_key_from_hash(key);
            key >>= offset << 3;
            let first_chunk = count.min(8 - offset as usize);
            for i in 0..first_chunk {
                buffer[pos + i] ^= (key & 0xFF) as u8;
                key >>= 8;
            }
        }
        self.base.inner_decrypt(key, offset, buffer, pos, count)
    }
    fn decode(
        &self,
        key: u32,
        offset: u64,
        buffer: &mut [u8],
        pos: usize,
        count: usize,
    ) -> Result<()> {
        self.base.decode(key, offset, buffer, pos, count)
    }
}
icx_enc_arc_impl!(RiddleCxCrypt);

impl Crypt for Arc<RiddleCxCrypt> {
    base_schema_impl!();
    fn init(&self, archive: &mut Xp3Archive) -> Result<()> {
        default_init_crypt(archive)?;
        read_yuzu_names(
            archive,
            &self.base.names_section_id,
            |reader, unpacked_size| self.read_yuzu_names(reader, unpacked_size),
        )
    }
    fn decrypt_supported(&self) -> bool {
        true
    }
    fn decrypt_seek_supported(&self) -> bool {
        true
    }
    fn decrypt<'a>(
        &self,
        entry: &Xp3Entry,
        cur_seg: &Segment,
        stream: Box<dyn Read + Send + Sync + 'a>,
    ) -> Result<Box<dyn ReadDebug + Send + Sync + 'a>> {
        let key = (
            entry.file_hash,
            Box::new(self.clone()) as Box<dyn ICxEncryption + Send + Sync + 'a>,
        );
        Ok(Box::new(CxEncryptionReader::new(stream, cur_seg, key)))
    }
    fn decrypt_with_seek<'a>(
        &self,
        entry: &Xp3Entry,
        cur_seg: &Segment,
        stream: Box<dyn ReadSeek + Send + Sync + 'a>,
    ) -> Result<Box<dyn ReadSeek + Send + Sync + 'a>> {
        let key = (
            entry.file_hash,
            Box::new(self.clone()) as Box<dyn ICxEncryption + Send + Sync + 'a>,
        );
        Ok(Box::new(CxEncryptionReader::new(stream, cur_seg, key)))
    }
}

#[derive(Debug)]
pub struct HxCryptLite {
    base: CxEncryption,
    header_key: Option<Vec<u8>>,
    header_split_position: u64,
    file_crypt_flag: bool,
}

impl HxCryptLite {
    pub fn new(
        base: BaseSchema,
        schema: &CxSchema,
        filename: &str,
        header_key: Option<Vec<u8>>,
        header_split_position: u64,
        file_crypt_flag: bool,
        random_type: i32,
    ) -> Result<Arc<Self>> {
        if let Some(key) = header_key.as_ref() {
            if key.len() < 8 {
                anyhow::bail!("header_key is too small.");
            }
        }
        Ok(Arc::new(Self {
            base: CxEncryption::new_inner(
                base,
                schema,
                filename,
                Box::new(HxProgramLiteBuilder::new(random_type)),
            )?,
            header_key,
            header_split_position,
            file_crypt_flag,
        }))
    }
}

impl AsRef<BaseSchema> for HxCryptLite {
    fn as_ref(&self) -> &BaseSchema {
        self.base.as_ref()
    }
}

#[derive(Debug)]
struct HxProgramLite {
    base: CxProgram,
    random_type: i32,
    random_block: [u32; 0x270],
    block_position: usize,
}

impl HxProgramLite {
    pub fn new(seed: u32, control_block: Weak<Vec<u32>>, random_method: i32) -> Self {
        let block_position = 0x270;
        let mut block = [0; 0x270];
        block[0] = seed;
        for i in 1..0x270 {
            block[i] =
                ((block[i - 1] ^ (block[i - 1] >> 0x1E)) * 0x6C078965).wrapping_add(i as u32);
        }
        Self {
            base: CxProgram {
                code: Vec::with_capacity(CX_PROGRAM_SIZE),
                control_block,
                length: 0,
                seed,
            },
            random_type: random_method,
            random_block: block,
            block_position,
        }
    }

    fn get_random_new(&mut self) -> u32 {
        if self.block_position == 0x270 {
            self.transform_block();
        }
        let s0 = self.random_block[self.block_position];
        let s1 = (s0 >> 11) ^ s0;
        let s2 = ((s1 & 0xFF3A58AD) << 7) ^ s1;
        let s3 = ((s2 & 0xFFFFDF8C) << 15) ^ s2;
        let s4 = (s3 >> 18) ^ s3;
        self.block_position += 1;
        s4
    }

    fn transform_block(&mut self) {
        self.block_position = 0;
        let block = &mut self.random_block;
        // 0-0xE2
        for i in 0..0xE3 {
            let s0 = if (block[i + 1] & 1) != 0 {
                0x9908B0DFu32
            } else {
                0
            };
            let s1 = (((block[i] ^ block[i + 1]) & 0x7FFFFFFE) ^ block[i]) >> 1;
            let s2 = s0 ^ s1 ^ block[i + 0x18D];
            block[i] = s2;
        }
        // 0xE3-0x26E
        for i in 0..0x18C {
            let s0 = if (block[i + 1 + 0xE3] & 1) != 0 {
                0x9908B0DFu32
            } else {
                0
            };
            let s1 =
                (((block[i + 0xE3] ^ block[i + 1 + 0xE3]) & 0x7FFFFFFE) ^ block[i + 0xE3]) >> 1;
            let s2 = s0 ^ s1 ^ block[i];
            block[i + 0xE3] = s2;
        }
        // 0x26F
        let s0 = if (block[0] & 1) != 0 {
            0x9908B0DFu32
        } else {
            0
        };
        let s1 = (((block[0x26F] ^ block[0]) & 0x7FFFFFFE) ^ block[0x26F]) >> 1;
        let s2 = s0 ^ s1 ^ block[0x18C];
        block[0x26F] = s2;
    }
}

impl ICxProgram for HxProgramLite {
    fn execute(&self, hash: u32) -> Result<u32> {
        self.base.execute(hash)
    }
    fn clear(&mut self) {
        self.base.clear();
    }
    fn emit(&mut self, bytecode: CxByteCode, length: usize) -> bool {
        self.base.emit(bytecode, length)
    }
    fn emit_nop(&mut self, count: usize) -> bool {
        self.base.emit_nop(count)
    }
    fn emit_u32(&mut self, x: u32) -> bool {
        self.base.emit_u32(x)
    }
    fn get_random(&mut self) -> u32 {
        if self.random_type == 0 {
            self.base.get_random()
        } else {
            self.get_random_new()
        }
    }
}

#[derive(Debug)]
struct HxProgramLiteBuilder {
    random_method: i32,
}

impl HxProgramLiteBuilder {
    pub fn new(random_method: i32) -> Self {
        Self { random_method }
    }
}

impl ICxProgramBuilder for HxProgramLiteBuilder {
    fn build(
        &self,
        seed: u32,
        control_blocks: Weak<Vec<u32>>,
    ) -> Box<dyn ICxProgram + Send + Sync> {
        Box::new(HxProgramLite::new(seed, control_blocks, self.random_method))
    }
}

struct HxFileDecryptor {
    split_pos1: u64,
    split_pos2: u64,
    key: u32,
    key1: u8,
    key2: u8,
}

impl HxFileDecryptor {
    fn new(key: u64, file_key_flag: bool) -> Self {
        let key_ptr = key.to_le_bytes();
        let mut global_key = key_ptr[0] as u32;
        let mut key1 = key_ptr[1];
        let mut key2 = key_ptr[2];
        let split_pos1 = u16::from_le_bytes([key_ptr[6], key_ptr[7]]) as u64;
        let mut split_pos2 = u16::from_le_bytes([key_ptr[4], key_ptr[5]]) as u64;
        if split_pos1 == split_pos2 {
            split_pos2 += 1;
        }
        if global_key == 0 {
            global_key = 1;
        }
        global_key = global_key.wrapping_mul(0x01010101);
        if file_key_flag {
            key1 = 0;
            key2 = 0;
        }
        Self {
            split_pos1,
            split_pos2,
            key: global_key,
            key1,
            key2,
        }
    }

    fn decrypt(&self, data: &mut [u8], offset: u64, pos: usize, count: usize) {
        if count == 0 {
            return;
        }
        let key = self.key.to_le_bytes();
        let mut key_pos = (offset & 3) as usize;
        for i in 0..count {
            data[pos + i] ^= key[key_pos];
            key_pos = (key_pos + 1) & 3;
        }
        let count = count as u64;
        if self.split_pos1 >= offset && self.split_pos1 < offset + count {
            data[(self.split_pos1 - offset) as usize + pos] ^= self.key1;
        }
        if self.split_pos2 >= offset && self.split_pos2 < offset + count {
            data[(self.split_pos2 - offset) as usize + pos] ^= self.key2;
        }
    }
}

struct HxHeaderDecryptor {
    key: [u8; 8],
    pos: u64,
}

impl HxHeaderDecryptor {
    fn new(hash: u32, key: &[u8], pos: u64) -> Self {
        let key_ptr = [
            u32::from_le_bytes([key[0], key[1], key[2], key[3]]),
            u32::from_le_bytes([key[4], key[5], key[6], key[7]]),
        ];
        let s0 = hash ^ key_ptr[1];
        let s1 = hash ^ (hash << 13);
        let s2 = s1 ^ (s1 >> 17);
        let s3 = s2 ^ (s2 << 5) ^ key_ptr[0];
        let key = ((s3 as u64) << 32) | (s0 as u64);
        Self {
            key: key.to_le_bytes(),
            pos,
        }
    }

    fn decrypt(&self, data: &mut [u8], offset: u64, pos: usize, count: usize) {
        let mut start_pos = offset;
        if start_pos <= self.pos {
            start_pos = self.pos;
        }
        let mut end_pos = offset + count as u64;
        if end_pos >= self.pos + 8 {
            end_pos = self.pos + 8;
        }
        if start_pos >= end_pos {
            return;
        }
        let dlen = end_pos - start_pos;
        let key_start_index = start_pos - self.pos;
        let data_start_index = start_pos - offset + pos as u64;
        for i in 0..dlen {
            data[(data_start_index + i) as usize] ^= self.key[(key_start_index + i) as usize];
        }
    }
}

impl ICxEncryption for HxCryptLite {
    fn get_base_offset(&self, _hash: u32) -> u32 {
        _hash
    }
    fn inner_decrypt(
        &self,
        hash: u32,
        offset: u64,
        buffer: &mut [u8],
        pos: usize,
        count: usize,
    ) -> Result<()> {
        if let Some(key) = self.header_key.as_ref() {
            let dec = HxHeaderDecryptor::new(hash, &key, self.header_split_position);
            dec.decrypt(buffer, offset, pos, count);
        }
        let ret1 = self.base.execute_xcode(hash)?;
        let ret2 = self.base.execute_xcode(hash ^ (hash >> 16))?;
        let key1 = ((ret1.1 as u64) << 32) | (ret1.0 as u64);
        let key2 = ((ret2.1 as u64) << 32) | (ret2.0 as u64);
        let split_pos = (self.base.offset + (hash & self.base.mask)) as u64;
        let dec1 = HxFileDecryptor::new(key1, self.file_crypt_flag);
        let dec2 = HxFileDecryptor::new(key2, self.file_crypt_flag);
        if split_pos > offset {
            if split_pos < offset + count as u64 {
                let blen1 = split_pos - offset;
                let blen2 = offset + count as u64 - split_pos;
                dec1.decrypt(buffer, offset, pos, blen1 as usize);
                dec2.decrypt(buffer, offset + blen1, pos + blen1 as usize, blen2 as usize);
            } else {
                dec1.decrypt(buffer, offset, pos, count);
            }
        } else {
            dec2.decrypt(buffer, offset, pos, count);
        }
        Ok(())
    }
    fn decode(
        &self,
        _key: u32,
        _offset: u64,
        _buffer: &mut [u8],
        _pos: usize,
        _count: usize,
    ) -> Result<()> {
        Ok(())
    }
}

icx_enc_arc_impl!(HxCryptLite);

impl Crypt for Arc<HxCryptLite> {
    base_schema_impl!();
    fn decrypt_supported(&self) -> bool {
        true
    }
    fn decrypt_seek_supported(&self) -> bool {
        true
    }
    fn decrypt<'a>(
        &self,
        entry: &Xp3Entry,
        cur_seg: &Segment,
        stream: Box<dyn Read + Send + Sync + 'a>,
    ) -> Result<Box<dyn ReadDebug + Send + Sync + 'a>> {
        let key = (
            entry.file_hash,
            Box::new(self.clone()) as Box<dyn ICxEncryption + Send + Sync + 'a>,
        );
        Ok(Box::new(CxEncryptionReader::new(stream, cur_seg, key)))
    }
    fn decrypt_with_seek<'a>(
        &self,
        entry: &Xp3Entry,
        cur_seg: &Segment,
        stream: Box<dyn ReadSeek + Send + Sync + 'a>,
    ) -> Result<Box<dyn ReadSeek + Send + Sync + 'a>> {
        let key = (
            entry.file_hash,
            Box::new(self.clone()) as Box<dyn ICxEncryption + Send + Sync + 'a>,
        );
        Ok(Box::new(CxEncryptionReader::new(stream, cur_seg, key)))
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
struct FileHash([u8; 32]);

impl std::fmt::Debug for FileHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FileHash(")?;
        write!(f, "{}", hex::encode(self.0))?;
        write!(f, ")")
    }
}

impl std::fmt::Display for FileHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

impl<'a> TryFrom<&'a [u8]> for FileHash {
    type Error = anyhow::Error;
    fn try_from(value: &'a [u8]) -> Result<Self> {
        Ok(Self(value.try_into()?))
    }
}

impl<'a> TryFrom<&'a str> for FileHash {
    type Error = anyhow::Error;
    fn try_from(value: &'a str) -> Result<Self> {
        Self::try_from(hex::decode(value)?.as_slice())
    }
}

impl<'de> Deserialize<'de> for FileHash {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let bytes = hex::decode(&s).map_err(de::Error::custom)?;
        let arr = bytes.try_into().map_err(|bytes: Vec<u8>| {
            de::Error::custom(format!(
                "FileHash length mismatch: expected 32 bytes, got {}",
                bytes.len()
            ))
        })?;
        Ok(FileHash(arr))
    }
}

impl FileHash {
    fn to_string(&self) -> String {
        hex::encode(&self.0)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
struct PathHash(u64);

impl std::fmt::Debug for PathHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PathHash({:#x})", self.0)
    }
}

impl std::fmt::Display for PathHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(&self.0.to_be_bytes()))
    }
}

impl<'de> Deserialize<'de> for PathHash {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let bytes = hex::decode(&s).map_err(de::Error::custom)?;
        let arr: [u8; 8] = bytes.try_into().map_err(|bytes: Vec<u8>| {
            de::Error::custom(format!(
                "PathHash length mismatch: expected 8 bytes, got {}",
                bytes.len()
            ))
        })?;
        Ok(PathHash(u64::from_be_bytes(arr)))
    }
}

impl<'a> TryFrom<&'a [u8]> for PathHash {
    type Error = anyhow::Error;
    fn try_from(value: &'a [u8]) -> Result<Self> {
        let arr: [u8; 8] = value.try_into()?;
        Ok(PathHash(u64::from_be_bytes(arr)))
    }
}

impl<'a> TryFrom<&'a str> for PathHash {
    type Error = anyhow::Error;
    fn try_from(value: &'a str) -> Result<Self> {
        Self::try_from(hex::decode(value)?.as_slice())
    }
}

impl PathHash {
    fn to_string(&self) -> String {
        hex::encode(&self.0.to_be_bytes())
    }
}

#[derive(Clone, Deserialize)]
struct KeyPackage {
    description: String,
    sku: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CxdecDb {
    #[allow(unused)]
    file_hash_salt: String,
    /// xp3 filename -> path hash -> file hash -> file name
    file_list: HashMap<String, HashMap<PathHash, HashMap<FileHash, Option<String>>>>,
    #[serde(default)]
    key_packages: Vec<KeyPackage>,
    #[allow(unused)]
    path_hash_salt: String,
    path_mapping: HashMap<PathHash, Option<String>>,
    project_name: String,
}

impl std::fmt::Debug for CxdecDb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CxdecDb").finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub struct HxCrypt {
    base: CxEncryption,
    key: [u8; 32],
    nonce: [u8; 16],
    filter_key: u64,
    file_mapping: HashMap<FileHash, String>,
    path_mapping: HashMap<PathHash, String>,
    info_map: Mutex<HashMap<String, HxEntry>>,
}

#[derive(Clone)]
pub struct IndexKey {
    key: [u8; 32],
    nonce: [u8; 16],
}

impl std::fmt::Debug for IndexKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IndexKey")
            .field("key", &hex::encode(&self.key))
            .field("nonce", &hex::encode(&self.nonce))
            .finish()
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct IndexKeyTmp {
    key: String,
    nonce: String,
}

impl<'de> Deserialize<'de> for IndexKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use base64::{Engine, engine::general_purpose::STANDARD};
        let s = IndexKeyTmp::deserialize(deserializer)?;
        let bytes = STANDARD.decode(&s.key).map_err(de::Error::custom)?;
        let key: [u8; 32] = bytes.try_into().map_err(|bytes: Vec<u8>| {
            de::Error::custom(format!(
                "Index key length mismatch: expected 32 bytes, got {}",
                bytes.len()
            ))
        })?;
        let hbytes = STANDARD.decode(&s.nonce).map_err(de::Error::custom)?;
        let nonce: [u8; 16] = hbytes.try_into().map_err(|bytes: Vec<u8>| {
            de::Error::custom(format!(
                "Index key nonce length mismatch: expected 16 bytes, got {}",
                bytes.len()
            ))
        })?;
        Ok(Self { key, nonce })
    }
}

impl HxCrypt {
    pub fn new(
        base: BaseSchema,
        cx: &CxSchema,
        index_key: Option<&IndexKey>,
        filter_key: u64,
        random_type: i32,
        file_list_name: Option<&str>,
        file_list_path: Option<&str>,
        index_key_dict: &HashMap<String, IndexKey>,
        filename: &str,
    ) -> Result<Self> {
        let mut index_key = if let Some(fkey) = index_key {
            Some(fkey.clone())
        } else {
            None
        };
        let p = std::path::Path::new(filename);
        let b = p
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("Failed to get file name from path."))?;
        let s: &str = &b.to_string_lossy();
        if let Some(ind) = index_key_dict.get(s) {
            index_key = Some(ind.clone())
        }
        let index_key = index_key.ok_or_else(|| anyhow::anyhow!("Can not find index key."))?;
        let (file_map, path_map) = if let Some(path) = file_list_path {
            let data = std::fs::read(path)?;
            let data = decode_to_string(Encoding::Utf8, &data, true)?;
            Self::read_names(&data, s)?
        } else if let Some(name) = file_list_name {
            let flist = query_filename_list(name)?;
            Self::read_names(&flist, s)?
        } else {
            let pdir = p.parent().map(|s| s.to_owned()).unwrap_or_default();
            if let Some(k) = Self::try_default_name(&pdir.join("filelist.json"), s)? {
                k
            } else if let Some(k) = Self::try_default_name(&pdir.join("filelist.lst"), s)? {
                k
            } else {
                (HashMap::new(), HashMap::new())
            }
        };
        Ok(Self {
            base: CxEncryption::new_inner(
                base,
                cx,
                filename,
                Box::new(HxProgramBuilder::new(random_type)),
            )?,
            key: index_key.key,
            nonce: index_key.nonce,
            filter_key,
            file_mapping: file_map,
            path_mapping: path_map,
            info_map: Mutex::new(HashMap::new()),
        })
    }

    fn try_default_name<P: AsRef<std::path::Path>>(
        s: &P,
        b: &str,
    ) -> Result<Option<(HashMap<FileHash, String>, HashMap<PathHash, String>)>> {
        let n = match get_ignorecase_path(s) {
            Ok(s) => s,
            Err(_) => return Ok(None),
        };
        if !n.exists() {
            return Ok(None);
        }
        let s = std::fs::read(&n)?;
        let data = decode_to_string(Encoding::Utf8, &s, true)?;
        let names = Self::read_names(&data, b)?;
        eprintln!(
            "Read {} file entries and {} directory entries from filelist {}.",
            names.0.len(),
            names.1.len(),
            n.display()
        );
        Ok(Some(names))
    }

    fn read_names(
        s: &str,
        basename: &str,
    ) -> Result<(HashMap<FileHash, String>, HashMap<PathHash, String>)> {
        if let Ok(s) = serde_json::from_str::<CxdecDb>(&s) {
            let path_map: HashMap<_, _> = s
                .path_mapping
                .iter()
                .filter_map(|(k, v)| match v {
                    Some(v) => Some((k.clone(), v.clone())),
                    None => None,
                })
                .collect();
            let file_map: HashMap<_, _> = if let Some(s) = s.file_list.get(basename) {
                s.iter()
                    .map(|s| s.1)
                    .flatten()
                    .filter_map(|(k, v)| match v {
                        Some(v) => Some((k.clone(), v.clone())),
                        None => None,
                    })
                    .collect()
            } else {
                HashMap::new()
            };
            return Ok((file_map, path_map));
        }
        let mut file_map = HashMap::new();
        let mut path_map = HashMap::new();
        for line in s.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let mut iter = line.splitn(2, ':');
            let key = match iter.next() {
                Some(v) => v,
                None => continue,
            };
            let value = match iter.next() {
                Some(v) => v,
                None => continue,
            };
            if key.len() == 16 {
                let key = PathHash::try_from(key)?;
                path_map.insert(key, value.to_string());
            } else if key.len() == 64 {
                let key = FileHash::try_from(key)?;
                file_map.insert(key, value.to_string());
            }
        }
        Ok((file_map, path_map))
    }

    fn create_chacha20_crypt(&self) -> Result<ChaCha20Legacy> {
        use chacha20::{KeyIvInit, cipher::StreamCipherSeek};
        let mut nonce = [0; 8];
        nonce.copy_from_slice(&self.nonce[..8]);
        let mut crypt = ChaCha20Legacy::new((&self.key).into(), (&nonce).into());
        crypt.try_seek(64)?;
        Ok(crypt)
    }

    fn read_index<T: Read + Seek>(&self, mut stream: T) -> Result<()> {
        use chacha20::cipher::StreamCipher;
        let len = stream.stream_length()?;
        let mut crypt = self.create_chacha20_crypt()?;
        let tlen = len as usize - 16;
        let mut buf = Vec::with_capacity(tlen);
        stream.seek(SeekFrom::Start(16))?;
        stream.read_to_end(&mut buf)?;
        crypt.try_apply_keystream(&mut buf)?;
        let mut stream = flate2::read::ZlibDecoder::new(MemReaderRef::new(&buf[4..]));
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf)?;
        let mut reader = MemReader::new(buf);
        let root_obj = TjsValue::unpack(&mut reader, true, Encoding::Utf16LE, &None)?;
        if !root_obj.is_array() {
            anyhow::bail!("Index object is not an array.");
        }
        let mut info_map = self.info_map.lock_blocking();
        info_map.clear();
        let set = create_garbage_filename_set("xp3hnp");
        for i in (0..root_obj.len()).step_by(2) {
            let path_hash = PathHash::try_from(
                root_obj[i]
                    .as_bytes()
                    .ok_or_else(|| anyhow::anyhow!("path hash is not bytes."))?,
            )?;
            let dir_obj = &root_obj[i + 1];
            if !dir_obj.is_array() {
                anyhow::bail!("dir object at index {} is not array.", i + 1);
            }
            let (path_name, path_is_hash) = if let Some(n) = self.path_mapping.get(&path_hash) {
                (n.to_owned(), false)
            } else {
                (path_hash.to_string() + "/", true)
            };
            for j in (0..dir_obj.len()).step_by(2) {
                let entry_hash = FileHash::try_from(
                    dir_obj[j]
                        .as_bytes()
                        .ok_or_else(|| anyhow::anyhow!("entry hash is not bytes."))?,
                )?;
                let entry_obj = &dir_obj[j + 1];
                if !entry_obj.is_array() {
                    anyhow::bail!("Entry object at index {},{} is not array.", i + 1, j + 1);
                }
                if entry_obj.len() < 2 {
                    anyhow::bail!("Entry object at index {},{} is too small.", i + 1, j + 1);
                }
                let entry_id = entry_obj[0]
                    .as_u64()
                    .ok_or_else(|| anyhow::anyhow!("Entry id is not int."))?;
                let entry_key = entry_obj[1]
                    .as_u64()
                    .ok_or_else(|| anyhow::anyhow!("Entry key is not int."))?;
                let (name, name_is_hash) = if let Some(n) = self.file_mapping.get(&entry_hash) {
                    (n.to_owned(), false)
                } else {
                    (entry_hash.to_string(), true)
                };
                let uname = Self::get_unicode_name(entry_id as u32);
                let entry = HxEntry {
                    path: path_name.clone(),
                    name,
                    id: entry_id,
                    key: entry_key,
                    name_is_hash,
                    path_is_hash,
                    is_garbage: set.contains(&entry_hash),
                };
                info_map.insert(uname, entry);
            }
        }
        Ok(())
    }

    fn get_unicode_name(mut hash: u32) -> String {
        let mut buf = [0u16; 4];
        let mut i = 0;
        loop {
            buf[i] = ((hash & 0x3FFF) + 0x5000) as u16;
            hash >>= 14;
            i += 1;
            if hash == 0 {
                break;
            }
        }
        let s = String::from_utf16_lossy(&buf[..i]);
        s
    }

    fn create_filter_key(&self, entry_key: u64, header_key_seed: u64) -> Result<HxFilterKey> {
        let key0 = entry_key as u32;
        let key1 = (entry_key >> 32) as u32;
        let k0 = self.base.execute_xcode(key0)?;
        let file_key_0 = (k0.0 as u64) | ((k0.1 as u64) << 32);
        let k1 = self.base.execute_xcode(key1)?;
        let file_key_1 = (k1.0 as u64) | ((k1.1 as u64) << 32);
        let split_position =
            (self.base.offset as u64 + ((entry_key >> 16) & self.base.mask as u64)) & 0xffffffff;
        let mut header_key = [0u8; 16];
        let k3 = self.base.execute_xcode(header_key_seed as u32)?;
        let mut v5 = (k3.0 as u64) | ((k3.1 as u64) << 32);
        v5 = !v5;
        let mut writer = MemWriterRef::new(&mut header_key);
        writer.write_u64_be(v5)?;
        let k3 = self.base.execute_xcode(v5 as u32)?;
        v5 = (k3.0 as u64) | ((k3.1 as u64) << 32);
        v5 = !v5;
        writer.write_u64_be(v5)?;
        Ok(HxFilterKey {
            key: [file_key_0, file_key_1],
            header_key,
            split_position,
            has_header_key: true,
            flag: false,
        })
    }
}

impl AsRef<CxEncryption> for HxCrypt {
    fn as_ref(&self) -> &CxEncryption {
        &self.base
    }
}

struct CopyStream<'a> {
    inner: Box<dyn Read + Send + Sync + 'a>,
}

impl<'a> CopyStream<'a> {
    pub fn new(stream: Box<dyn Read + Send + Sync + 'a>) -> Self {
        Self { inner: stream }
    }
}

impl<'a> std::fmt::Debug for CopyStream<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CopyStream").finish_non_exhaustive()
    }
}

impl<'a> Read for CopyStream<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}

impl Crypt for HxCrypt {
    base_schema_impl!();
    fn init(&self, archive: &mut Xp3Archive) -> Result<()> {
        if let Some(hxv4) = archive.extras.iter().find(|x| x.tag == "Hxv4") {
            let mut reader = MemReaderRef::new(&hxv4.data);
            let offset = reader.read_u64()? + archive.base_offset;
            let size = reader.read_u32()?;
            let _flags = reader.read_u16()?;
            let stream = StreamRegion::with_size(
                MutexWrapper::new(archive.inner.clone(), offset),
                size as u64,
            )?;
            self.read_index(stream)?;
            let info_map = self.info_map.lock_blocking();
            for entry in archive.entries.iter_mut() {
                if let Some(info) = info_map.get(&entry.name) {
                    if info.is_garbage {
                        continue;
                    }
                    entry.name = format!("{}{}", info.path, info.name);
                    let info = info.clone();
                    entry.extra = Some(Arc::new(Box::new(info)))
                }
            }
            archive.entries.retain(|x| {
                x.extra.is_some() || !info_map.get(&x.name).is_some_and(|x| x.is_garbage)
            });
        }
        archive.extras.retain(|x| x.tag != "Hxv4");
        Ok(())
    }
    fn decrypt_supported(&self) -> bool {
        true
    }
    fn decrypt_seek_supported(&self) -> bool {
        true
    }
    fn decrypt<'a>(
        &self,
        entry: &Xp3Entry,
        cur_seg: &Segment,
        stream: Box<dyn Read + Send + Sync + 'a>,
    ) -> Result<Box<dyn ReadDebug + Send + Sync + 'a>> {
        let info = match entry.extra.as_ref() {
            Some(info) => info,
            None => return Ok(Box::new(CopyStream::new(stream))),
        };
        let info = info
            .as_any()
            .downcast_ref::<HxEntry>()
            .ok_or_else(|| anyhow::anyhow!("extra info is not hx entry."))?;
        let mut entry_key = info.key;
        if (info.id & 0x100000000) == 0 {
            entry_key ^= self.filter_key;
        }
        let header_key = !entry_key;
        let key = self.create_filter_key(entry_key, header_key)?;
        let filter = HxFilter::new(key);
        let key = (
            entry.file_hash,
            Box::new(filter) as Box<dyn ICxEncryption + Send + Sync + 'a>,
        );
        Ok(Box::new(CxEncryptionReader::new(stream, cur_seg, key)))
    }
    fn decrypt_with_seek<'a>(
        &self,
        entry: &Xp3Entry,
        cur_seg: &Segment,
        stream: Box<dyn ReadSeek + Send + Sync + 'a>,
    ) -> Result<Box<dyn ReadSeek + Send + Sync + 'a>> {
        let info = match entry.extra.as_ref() {
            Some(info) => info,
            None => return Ok(stream),
        };
        let info = info
            .as_any()
            .downcast_ref::<HxEntry>()
            .ok_or_else(|| anyhow::anyhow!("extra info is not hx entry."))?;
        let mut entry_key = info.key;
        if (info.id & 0x100000000) == 0 {
            entry_key ^= self.filter_key;
        }
        let header_key = !entry_key;
        let key = self.create_filter_key(entry_key, header_key)?;
        let filter = HxFilter::new(key);
        let key = (
            entry.file_hash,
            Box::new(filter) as Box<dyn ICxEncryption + Send + Sync + 'a>,
        );
        Ok(Box::new(CxEncryptionReader::new(stream, cur_seg, key)))
    }
}

#[derive(Debug)]
enum TjsValue {
    Void,
    Str(String),
    ByteArray(Vec<u8>),
    Int(i64),
    #[allow(unused)]
    Double(f64),
    Array(Vec<TjsValue>),
    Dict(HashMap<String, TjsValue>),
}

impl TjsValue {
    fn is_array(&self) -> bool {
        matches!(self, Self::Array(_))
    }

    fn len(&self) -> usize {
        match self {
            Self::Str(s) => s.len(),
            Self::ByteArray(arr) => arr.len(),
            Self::Array(arr) => arr.len(),
            Self::Dict(dict) => dict.len(),
            _ => 0,
        }
    }

    fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::ByteArray(arr) => Some(arr),
            _ => None,
        }
    }

    fn as_u64(&self) -> Option<u64> {
        match self {
            Self::Int(i) => Some(*i as u64),
            _ => None,
        }
    }
}

const VOID: TjsValue = TjsValue::Void;

impl Index<usize> for TjsValue {
    type Output = TjsValue;
    fn index(&self, index: usize) -> &Self::Output {
        match self {
            Self::Array(arr) => arr.get(index).unwrap_or(&VOID),
            _ => &VOID,
        }
    }
}

fn unpack_string<R: Read + Seek>(reader: &mut R, big: bool, encoding: Encoding) -> Result<String> {
    let len = u32::unpack(reader, big, encoding, &None)? as usize;
    let tlen = if encoding.is_utf16le() { len * 2 } else { len };
    let mut buf = vec![0u8; tlen];
    reader.read_exact(&mut buf)?;
    let s = decode_to_string(encoding, &buf, true)?;
    Ok(s)
}

impl StructUnpack for TjsValue {
    fn unpack<R: Read + Seek>(
        reader: &mut R,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn std::any::Any>>,
    ) -> Result<Self> {
        let typ = u8::unpack(reader, big, encoding, info)?;
        Ok(match typ {
            0 => Self::Void,
            2 => Self::Str(unpack_string(reader, big, encoding)?),
            3 => {
                let len = u32::unpack(reader, big, encoding, info)?;
                let data = reader.read_exact_vec(len as usize)?;
                Self::ByteArray(data)
            }
            4 => {
                let num = i64::unpack(reader, big, encoding, info)?;
                Self::Int(num)
            }
            5 => {
                let num = f64::unpack(reader, big, encoding, info)?;
                Self::Double(num)
            }
            0x81 => {
                let len = u32::unpack(reader, big, encoding, info)?;
                let mut arr = Vec::with_capacity(len as usize);
                for _ in 0..len {
                    arr.push(Self::unpack(reader, big, encoding, info)?);
                }
                Self::Array(arr)
            }
            0xC1 => {
                let len = u32::unpack(reader, big, encoding, info)?;
                let mut dict = HashMap::with_capacity(len as usize);
                for _ in 0..len {
                    let name = unpack_string(reader, big, encoding)?;
                    let obj = Self::unpack(reader, big, encoding, info)?;
                    dict.insert(name, obj);
                }
                Self::Dict(dict)
            }
            _ => anyhow::bail!("Unknown type id: {typ:02x}."),
        })
    }
}

#[derive(Clone, Debug)]
struct HxEntry {
    path: String,
    name: String,
    id: u64,
    key: u64,
    name_is_hash: bool,
    path_is_hash: bool,
    is_garbage: bool,
}

impl AnyDebug for HxEntry {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct HxSplitMix64 {
    state: u64,
}

impl HxSplitMix64 {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }
}

trait IRng: std::fmt::Debug {
    fn next(&mut self) -> u64;
}

impl IRng for HxSplitMix64 {
    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Xoroshiro128PlusPlus {
    state: [u64; 2],
}

impl Xoroshiro128PlusPlus {
    pub fn new(state: [u64; 2]) -> Self {
        assert!(
            state[0] != 0 || state[1] != 0,
            "Initial state cannot be all zeros."
        );
        Self { state }
    }
}

impl IRng for Xoroshiro128PlusPlus {
    fn next(&mut self) -> u64 {
        let s0 = self.state[0];
        let mut s1 = self.state[1];
        let result = s0.wrapping_add(s1).rotate_left(17).wrapping_add(s0);
        s1 ^= s0;
        self.state[0] = s0.rotate_left(49) ^ s1 ^ (s1 << 21);
        self.state[1] = s1.rotate_left(28);
        result
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Xoroshiro128StarStar {
    state: [u64; 2],
}

impl Xoroshiro128StarStar {
    pub fn new(state: [u64; 2]) -> Self {
        assert!(
            state[0] != 0 || state[1] != 0,
            "Initial state cannot be all zeros."
        );
        Self { state }
    }
}

impl IRng for Xoroshiro128StarStar {
    fn next(&mut self) -> u64 {
        let s0 = self.state[0];
        let mut s1 = self.state[1];
        let result = s0.wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        s1 ^= s0;
        self.state[0] = s0.rotate_left(24) ^ s1 ^ (s1 << 16);
        self.state[1] = s1.rotate_left(37);
        result
    }
}

#[derive(Debug)]
struct HxProgram {
    base: CxProgram,
    rng: Box<dyn IRng + Send + Sync>,
}

impl HxProgram {
    pub fn new(seed: u32, control_block: Weak<Vec<u32>>, random_method: i32) -> Self {
        let initial_seed = (seed as u64) | (!(seed as u64) << 32);
        let mut seeder = HxSplitMix64::new(initial_seed);
        let seed1 = seeder.next();
        let seed2 = seeder.next();
        let xoroshiro_seed = [seed1, seed2];
        Self {
            base: CxProgram {
                code: Vec::new(),
                control_block,
                length: 0,
                seed,
            },
            rng: if random_method == 0 {
                Box::new(Xoroshiro128PlusPlus::new(xoroshiro_seed))
            } else {
                Box::new(Xoroshiro128StarStar::new(xoroshiro_seed))
            },
        }
    }
}

impl ICxProgram for HxProgram {
    fn execute(&self, hash: u32) -> Result<u32> {
        self.base.execute(hash)
    }
    fn clear(&mut self) {
        self.base.clear();
    }
    fn emit(&mut self, bytecode: CxByteCode, length: usize) -> bool {
        self.base.emit(bytecode, length)
    }
    fn emit_nop(&mut self, count: usize) -> bool {
        self.base.emit_nop(count)
    }
    fn emit_u32(&mut self, x: u32) -> bool {
        self.base.emit_u32(x)
    }
    fn get_random(&mut self) -> u32 {
        self.rng.next() as u32
    }
}

#[derive(Debug)]
struct HxProgramBuilder {
    random_method: i32,
}

impl HxProgramBuilder {
    pub fn new(random_method: i32) -> Self {
        Self { random_method }
    }
}

impl ICxProgramBuilder for HxProgramBuilder {
    fn build(
        &self,
        seed: u32,
        control_blocks: Weak<Vec<u32>>,
    ) -> Box<dyn ICxProgram + Send + Sync> {
        Box::new(HxProgram::new(seed, control_blocks, self.random_method))
    }
}

struct HxFilterKey {
    key: [u64; 2],
    header_key: [u8; 16],
    split_position: u64,
    has_header_key: bool,
    flag: bool,
}

#[derive(Clone)]
struct HxFilterSpanDecryptor {
    first_decrypt_key: u32,
    key1: u8,
    key2: u8,
    span_position: [u64; 2],
}

impl std::fmt::Debug for HxFilterSpanDecryptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HxFilterSpanDecryptor")
            .field(
                "firstDecryptKey",
                &format_args!("{:#x}", &self.first_decrypt_key),
            )
            .field("key1", &format_args!("{:#x}", &self.key1))
            .field("key2", &format_args!("{:#x}", &self.key2))
            .field(
                "spanPosition",
                &format_args!("{:#x}, {:#x}", self.span_position[0], self.span_position[1]),
            )
            .finish()
    }
}

impl HxFilterSpanDecryptor {
    pub fn new(key: u64, flag: bool) -> Self {
        let decrypt_key_bytes = ((key >> 8) & 0xFFFF) as u32;
        let mut first_decrypt_key = (key & 0xFF) as u32;
        let mut span_position = [(key >> 48) & 0xFFFF, (key >> 32) & 0xFFFF];
        if span_position[0] == span_position[1] {
            span_position[1] = span_position[1].wrapping_add(1);
        }
        if !flag && first_decrypt_key == 0 {
            first_decrypt_key = 0xA5;
        }
        first_decrypt_key = first_decrypt_key.wrapping_mul(0x01010101);
        let (key1, key2) = if flag {
            (0, 0)
        } else {
            (
                (decrypt_key_bytes & 0xFF) as u8,
                ((decrypt_key_bytes >> 8) & 0xFF) as u8,
            )
        };
        Self {
            first_decrypt_key,
            key1,
            key2,
            span_position,
        }
    }

    pub fn decrypt(&self, position: u64, data: &mut [u8]) {
        if data.is_empty() {
            return;
        }
        let key_bytes = self.first_decrypt_key.to_le_bytes();
        for (i, byte) in data.iter_mut().enumerate() {
            let key_index = ((position as usize) + i) & 3;
            *byte ^= key_bytes[key_index];
        }
        let data_len = data.len() as u64;
        if self.key1 != 0 {
            let pos1 = self.span_position[0];
            if pos1 >= position && pos1 < position + data_len {
                let index = (pos1 - position) as usize;
                data[index] ^= self.key1;
            }
        }
        if self.key2 != 0 {
            let pos2 = self.span_position[1];
            if pos2 >= position && pos2 < position + data_len {
                let index = (pos2 - position) as usize;
                data[index] ^= self.key2;
            }
        }
    }
}

struct HxFilter {
    span_decryptors: [HxFilterSpanDecryptor; 2],
    split_position: u64,
    header_key: [u8; 16],
    has_header_key: bool,
}

impl HxFilter {
    pub fn new(key: HxFilterKey) -> HxFilter {
        HxFilter {
            span_decryptors: [
                HxFilterSpanDecryptor::new(key.key[0], key.flag),
                HxFilterSpanDecryptor::new(key.key[1], key.flag),
            ],
            split_position: key.split_position,
            header_key: key.header_key,
            has_header_key: key.has_header_key,
        }
    }

    fn decrypt_header(&self, position: u64, buffer: &mut [u8]) {
        let header_len = self.header_key.len() as u64;
        let overlap_start = position;
        let overlap_end = (position + buffer.len() as u64).min(header_len);
        if overlap_start >= overlap_end {
            return;
        }
        for i in overlap_start..overlap_end {
            let buffer_index = (i - position) as usize;
            let key_index = i as usize;
            buffer[buffer_index] ^= self.header_key[key_index];
        }
    }

    pub fn decrypt(&self, position: u64, buffer: &mut [u8]) {
        if buffer.is_empty() {
            return;
        }
        if self.has_header_key {
            self.decrypt_header(position, buffer);
        }
        let buffer_len = buffer.len() as u64;
        let buffer_end_pos = position + buffer_len;
        if buffer_end_pos <= self.split_position {
            self.span_decryptors[0].decrypt(position, buffer);
        } else if position >= self.split_position {
            self.span_decryptors[1].decrypt(position, buffer);
        } else {
            let split_index = (self.split_position - position) as usize;
            let (part1, part2) = buffer.split_at_mut(split_index);
            self.span_decryptors[0].decrypt(position, part1);
            self.span_decryptors[1].decrypt(self.split_position, part2);
        }
    }
}

impl std::fmt::Debug for HxFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HxFilter")
            .field("spanDecryptors", &self.span_decryptors)
            .field(
                "splitPosition",
                &format_args!("{:#x}", &self.split_position),
            )
            .field("hasHeaderKey", &self.has_header_key)
            .field("headerKey", &hex::encode(self.header_key))
            .finish()
    }
}

impl ICxEncryption for HxFilter {
    fn get_base_offset(&self, _hash: u32) -> u32 {
        0
    }
    fn decode(
        &self,
        _key: u32,
        _offset: u64,
        _buffer: &mut [u8],
        _pos: usize,
        _count: usize,
    ) -> Result<()> {
        Ok(())
    }
    fn inner_decrypt(
        &self,
        _key: u32,
        offset: u64,
        buffer: &mut [u8],
        pos: usize,
        count: usize,
    ) -> Result<()> {
        self.decrypt(offset, &mut buffer[pos..pos + count]);
        Ok(())
    }
}

fn calculate_file_hash(pathname: &str, file_hash_salt: &str) -> FileHash {
    use blake2::{Blake2s256, Digest};
    let mut hasher = Blake2s256::new();
    (pathname.to_lowercase() + file_hash_salt)
        .encode_utf16()
        .for_each(|b| {
            hasher.update(&b.to_le_bytes());
        });
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    FileHash(hash)
}

fn create_garbage_filename_set(file_hash_salt: &str) -> HashSet<FileHash> {
    let mut set = HashSet::new();
    set.insert(calculate_file_hash("$$$ This is a protected archive. $$$ 著作者はこのアーカイブが正規の利用方法以外の方法で展開されることを望んでいません。 $$$ This is a protected archive. $$$ 著作者はこのアーカイブが正規の利用方法以外の方法で展開されることを望んでいません。 $$$ This is a protected archive. $$$ 著作者はこのアーカイブが正規の利用方法以外の方法で展開されることを望んでいません。 $$$ Warning! Extracting this archive may infringe on author's rights. 警告 このアーカイブを展開することにより、あなたは著作者の権利を侵害するおそれがあります。.txt", file_hash_salt));
    set
}

#[test]
fn test_filehash_deserialize() {
    assert_eq!(
        FileHash([
            0x0, 0x1, 0x2, 0x3, 0x4, 0x5, 0x6, 0x7, 0x8, 0x9, 0xa, 0xb, 0xc, 0xd, 0xe, 0xf, 0x0,
            0x1, 0x2, 0x3, 0x4, 0x5, 0x6, 0x7, 0x8, 0x9, 0xa, 0xb, 0xc, 0xd, 0xe, 0xf
        ]),
        serde_json::from_str(
            "\"000102030405060708090a0b0c0d0e0f000102030405060708090a0b0c0d0e0F\""
        )
        .unwrap()
    );
}
