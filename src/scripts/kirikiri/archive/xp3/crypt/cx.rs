use super::*;
use anyhow::Result;
use overf::wrapping as w;
use std::path::PathBuf;
use std::sync::Weak;

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
    programs: Vec<Box<dyn ICxProgram>>,
    program_builder: Box<dyn ICxProgramBuilder>,
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
        program_builder: Box<dyn ICxProgramBuilder>,
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

    fn new_program(&self, seed: u32) -> Box<dyn ICxProgram> {
        self.program_builder
            .build(seed, Arc::downgrade(&self.control_block))
    }

    fn generate_program(&self, seed: u32) -> Result<Box<dyn ICxProgram>> {
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

    fn emit_code(&self, program: &mut Box<dyn ICxProgram>, stage: i32) -> bool {
        program.emit_nop(5)
            && program.emit(MovEdiArg, 4)
            && self.emit_body(program, stage)
            && program.emit_nop(5)
            && program.emit(Retn, 1)
    }

    fn emit_body(&self, program: &mut Box<dyn ICxProgram>, stage: i32) -> bool {
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

    fn emit_body2(&self, program: &mut Box<dyn ICxProgram>, stage: i32) -> bool {
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
    fn emit_prolog(&self, program: &mut Box<dyn ICxProgram>) -> bool {
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

    fn emit_even_branch(&self, program: &mut Box<dyn ICxProgram>) -> bool {
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

    fn emit_odd_branch(&self, program: &mut Box<dyn ICxProgram>) -> bool {
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
        stream: Box<dyn Read + 'a>,
    ) -> Result<Box<dyn ReadDebug + 'a>> {
        let key = (
            entry.file_hash,
            Box::new(self.clone()) as Box<dyn ICxEncryption + 'a>,
        );
        Ok(Box::new(CxEncryptionReader::new(stream, cur_seg, key)))
    }
    fn decrypt_with_seek<'a>(
        &self,
        entry: &Xp3Entry,
        cur_seg: &Segment,
        stream: Box<dyn ReadSeek + 'a>,
    ) -> Result<Box<dyn ReadSeek + 'a>> {
        let key = (
            entry.file_hash,
            Box::new(self.clone()) as Box<dyn ICxEncryption + 'a>,
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
    fn build(&self, seed: u32, control_blocks: Weak<Vec<u32>>) -> Box<dyn ICxProgram>;
}

impl ICxProgramBuilder for CxProgramBuilder {
    fn build(&self, seed: u32, control_blocks: Weak<Vec<u32>>) -> Box<dyn ICxProgram> {
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
    key: (u32, Box<dyn ICxEncryption + 'a>),
}

impl<'a, T: Read> CxEncryptionReader<'a, T> {
    pub fn new(inner: T, seg: &Segment, key: (u32, Box<dyn ICxEncryption + 'a>)) -> Self {
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
        program_builder: Box<dyn ICxProgramBuilder>,
        names_section_id: String,
    ) -> Result<Self> {
        let cx = CxEncryption::new_inner(base, schema, filename, program_builder)?;
        Ok(Self {
            base: cx,
            names_section_id,
        })
    }

    fn read_yuzu_names(
        reader: Box<dyn ReadDebug>,
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

fn read_yuzu_names<T>(archive: &mut Xp3Archive, names_section_id: &str, convert: T) -> Result<()>
where
    T: FnOnce(Box<dyn ReadDebug>, u32) -> Result<(HashMap<u32, String>, HashMap<String, String>)>,
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
        stream: Box<dyn Read + 'a>,
    ) -> Result<Box<dyn ReadDebug + 'a>> {
        let key = (
            entry.file_hash,
            Box::new(self.clone()) as Box<dyn ICxEncryption + 'a>,
        );
        Ok(Box::new(CxEncryptionReader::new(stream, cur_seg, key)))
    }
    fn decrypt_with_seek<'a>(
        &self,
        entry: &Xp3Entry,
        cur_seg: &Segment,
        stream: Box<dyn ReadSeek + 'a>,
    ) -> Result<Box<dyn ReadSeek + 'a>> {
        let key = (
            entry.file_hash,
            Box::new(self.clone()) as Box<dyn ICxEncryption + 'a>,
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
    fn build(&self, seed: u32, control_blocks: Weak<Vec<u32>>) -> Box<dyn ICxProgram> {
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
        stream: Box<dyn Read + 'a>,
    ) -> Result<Box<dyn ReadDebug + 'a>> {
        let key = (
            entry.file_hash,
            Box::new(self.clone()) as Box<dyn ICxEncryption + 'a>,
        );
        Ok(Box::new(CxEncryptionReader::new(stream, cur_seg, key)))
    }
    fn decrypt_with_seek<'a>(
        &self,
        entry: &Xp3Entry,
        cur_seg: &Segment,
        stream: Box<dyn ReadSeek + 'a>,
    ) -> Result<Box<dyn ReadSeek + 'a>> {
        let key = (
            entry.file_hash,
            Box::new(self.clone()) as Box<dyn ICxEncryption + 'a>,
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

    fn read_yuzu_names(
        &self,
        mut reader: Box<dyn ReadDebug>,
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
        stream: Box<dyn Read + 'a>,
    ) -> Result<Box<dyn ReadDebug + 'a>> {
        let key = (
            entry.file_hash,
            Box::new(self.clone()) as Box<dyn ICxEncryption + 'a>,
        );
        Ok(Box::new(CxEncryptionReader::new(stream, cur_seg, key)))
    }
    fn decrypt_with_seek<'a>(
        &self,
        entry: &Xp3Entry,
        cur_seg: &Segment,
        stream: Box<dyn ReadSeek + 'a>,
    ) -> Result<Box<dyn ReadSeek + 'a>> {
        let key = (
            entry.file_hash,
            Box::new(self.clone()) as Box<dyn ICxEncryption + 'a>,
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

    fn read_yuzu_names(
        &self,
        mut reader: Box<dyn ReadDebug>,
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
        stream: Box<dyn Read + 'a>,
    ) -> Result<Box<dyn ReadDebug + 'a>> {
        let key = (
            entry.file_hash,
            Box::new(self.clone()) as Box<dyn ICxEncryption + 'a>,
        );
        Ok(Box::new(CxEncryptionReader::new(stream, cur_seg, key)))
    }
    fn decrypt_with_seek<'a>(
        &self,
        entry: &Xp3Entry,
        cur_seg: &Segment,
        stream: Box<dyn ReadSeek + 'a>,
    ) -> Result<Box<dyn ReadSeek + 'a>> {
        let key = (
            entry.file_hash,
            Box::new(self.clone()) as Box<dyn ICxEncryption + 'a>,
        );
        Ok(Box::new(CxEncryptionReader::new(stream, cur_seg, key)))
    }
}
