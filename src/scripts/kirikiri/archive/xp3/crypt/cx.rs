use super::*;
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Weak;

const S_CTL_BLOCK_SIGNATURE: &[u8] = b" Encryption control block";

#[derive(Debug)]
pub struct CxEncryption {
    mask: u32,
    offset: u32,
    prolog_order: Vec<u8>,
    odd_branch_order: Vec<u8>,
    even_branch_order: Vec<u8>,
    control_block: Arc<Vec<u32>>,
    programs: Vec<CxProgram>,
}

impl CxEncryption {
    pub fn new(schema: &CxSchema, filename: &str) -> Result<Arc<Self>> {
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
        } else {
            return Err(anyhow::anyhow!("TPM file name is required in schema"));
        };
        let control_block = Arc::new(control_block);
        let programs = Vec::with_capacity(0x80);
        let mut obj = Self {
            mask: schema.mask,
            offset: schema.offset,
            prolog_order: schema.prolog_order.bytes.clone(),
            odd_branch_order: schema.odd_branch_order.bytes.clone(),
            even_branch_order: schema.even_branch_order.bytes.clone(),
            control_block: control_block,
            programs,
        };
        for seed in 0..0x80 {
            obj.programs.push(obj.generate_program(seed)?);
        }
        Ok(Arc::new(obj))
    }

    fn new_program(&self, seed: u32) -> CxProgram {
        CxProgram {
            code: Vec::with_capacity(CX_PROGRAM_SIZE),
            control_block: Arc::downgrade(&self.control_block),
            length: 0,
            seed,
        }
    }

    fn generate_program(&self, seed: u32) -> Result<CxProgram> {
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

    fn emit_code(&self, program: &mut CxProgram, stage: i32) -> bool {
        program.emit_nop(5)
            && program.emit(MovEdiArg, 4)
            && self.emit_body(program, stage)
            && program.emit_nop(5)
            && program.emit(Retn, 1)
    }

    fn emit_body(&self, program: &mut CxProgram, stage: i32) -> bool {
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

    fn emit_body2(&self, program: &mut CxProgram, stage: i32) -> bool {
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
    fn emit_prolog(&self, program: &mut CxProgram) -> bool {
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

    fn emit_even_branch(&self, program: &mut CxProgram) -> bool {
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

    fn emit_odd_branch(&self, program: &mut CxProgram) -> bool {
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

    fn get_base_offset(&self, hash: u32) -> u32 {
        (hash & self.mask).wrapping_add(self.offset)
    }

    fn execute_xcode(&self, mut hash: u32) -> Result<(u32, u32)> {
        let seed = hash & 0x7f;
        hash >>= 7;
        let program = &self.programs[seed as usize];
        let ret1 = program.execute(hash)?;
        let ret2 = program.execute(!hash)?;
        Ok((ret1, ret2))
    }

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

impl Crypt for Arc<CxEncryption> {
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
        stream: Box<dyn Read + 'a>,
    ) -> Result<Box<dyn ReadDebug + 'a>> {
        let key = (entry.file_hash, self.clone());
        Ok(Box::new(CxEncryptionReader::new(stream, cur_seg, key)))
    }
    fn decrypt_with_seek<'a>(
        &self,
        entry: &Xp3Entry,
        cur_seg: &Segment,
        stream: Box<dyn ReadSeek + 'a>,
    ) -> Result<Box<dyn ReadSeek + 'a>> {
        let key = (entry.file_hash, self.clone());
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

impl CxProgram {
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

    fn emit_random(&mut self) -> bool {
        let random = self.get_random();
        self.emit_u32(random)
    }

    fn get_random(&mut self) -> u32 {
        let seed = self.seed;
        self.seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
        self.seed ^ (seed << 16) ^ (seed >> 16)
    }
}

impl<R: Read> Read for CxEncryptionReader<R> {
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
