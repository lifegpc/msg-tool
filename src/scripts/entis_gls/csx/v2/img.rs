use super::super::CSXScriptV2FullVer;
use super::super::base::*;
use super::disasm::*;
use super::types::*;
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::struct_pack::*;
use anyhow::Result;
use std::collections::HashMap;
use std::io::{Seek, Write};

const ID_HEADER: u64 = 0x2020726564616568; // header
const ID_IMAGE: u64 = 0x2020206567616D69;
const ID_IMAGE_GLOBAL: u64 = 0x6C626F6C67676D69;
const ID_IMAGE_CONST: u64 = 0x74736E6F63676D69;
const ID_IMAGE_SHARED: u64 = 0x6572616873676D69;
const ID_CLASS_INFO: u64 = 0x666E697373616C63;
const ID_FUNCTION: u64 = 0x6E6F6974636E7566;
const ID_INIT_NAKED_FUNC: u64 = 0x636E666E74696E69;
const ID_FUNC_INFO: u64 = 0x6F666E69636E7566;
const ID_SYMBOL_INFO: u64 = 0x666E696C626D7973;
const ID_GLOBAL: u64 = 0x20206C61626F6C67;
const ID_DATA: u64 = 0x2020202061746164;
const ID_CONST_STRING: u64 = 0x72747374736E6F63;
const ID_LINK_INFO: u64 = 0x20666E696B6E696C;
const ID_LINK_INFO_EX: u64 = 0x343678656B6E696C;
const ID_REF_FUNC: u64 = 0x20636E7566666572;
const ID_REF_CODE: u64 = 0x2065646F63666572;
const ID_REF_CLASS: u64 = 0x7373616C63666572;
const ID_IMPORT_NATIVE_FUNC: u64 = 0x766974616E706D69;

#[derive(Clone, Debug)]
#[allow(unused)]
pub struct ECSExecutionImage {
    file_header: FileHeader,
    section_header: SectionHeader,
    image: MemReader,
    image_global: Option<MemReader>,
    image_const: Option<MemReader>,
    image_shared: Option<MemReader>,
    section_class_info: SectionClassInfo,
    section_function: SectionFunction,
    section_init_naked_func: SectionInitNakedFunc,
    section_func_info: SectionFuncInfo,
    section_symbol_info: Option<SectionSymbolInfo>,
    section_global: Option<SectionGlobal>,
    section_data: Option<SectionData>,
    section_const_string: SectionConstString,
    section_link_info: Option<SectionLinkInfo>,
    section_link_info_ex: Option<SectionLinkInfoEx>,
    section_ref_func: Option<SectionRefFunc>,
    section_ref_code: Option<SectionRefCode>,
    section_ref_class: Option<SectionRefClass>,
    section_import_native_func: SectionImportNativeFunc,
    no_part_label: bool,
}

impl ECSExecutionImage {
    pub fn new(reader: MemReaderRef<'_>, config: &ExtraConfig) -> Result<Self> {
        if let Some(ver) = config.entis_gls_csx_v2_ver {
            match ver {
                CSXScriptV2FullVer::V3 => Self::inner_new(reader, config, 3),
                CSXScriptV2FullVer::V2 => Self::inner_new(reader, config, 2),
            }
        } else {
            match Self::inner_new(reader.clone(), config, 3) {
                Ok(img) => Ok(img),
                Err(_) => Self::inner_new(reader, config, 2),
            }
        }
    }

    fn inner_new(mut reader: MemReaderRef<'_>, config: &ExtraConfig, ver: u32) -> Result<Self> {
        let file_header = FileHeader::unpack(&mut reader, false, Encoding::Utf8, &None)?;
        if file_header.signagure != *b"Entis\x1a\0\0" {
            return Err(anyhow::anyhow!("Invalid EMC file signature"));
        }
        if !file_header.format_desc.starts_with(b"Cotopha Image file") {
            return Err(anyhow::anyhow!("Invalid EMC file format description"));
        }
        let mut section_header = SectionHeader::default();
        section_header.full_ver = ver;
        let len = reader.data.len();
        let mut image = None;
        let mut image_global = None;
        let mut image_const = None;
        let mut image_shared = None;
        let mut section_class_info = None;
        let mut section_function = None;
        let mut section_init_naked_func = None;
        let mut section_func_info = None;
        let mut section_symbol_info = None;
        let mut section_global = None;
        let mut section_data = None;
        let mut section_const_string = None;
        let mut section_link_info = None;
        let mut section_link_info_ex = None;
        let mut section_ref_func = None;
        let mut section_ref_code = None;
        let mut section_ref_class = None;
        let mut section_import_native_func = None;
        while reader.pos < len {
            if len - reader.pos < 16 {
                break;
            }
            let id = reader.read_u64()?;
            if id == 0 {
                break;
            }
            let size = reader.read_u64()?;
            let pos = reader.pos;
            match id {
                ID_HEADER => {
                    let mut mem = StreamRegion::with_size(&mut reader, size)?;
                    section_header = SectionHeader::unpack(&mut mem, false, Encoding::Utf8, &None)?;
                    section_header.full_ver = ver;
                }
                ID_IMAGE => {
                    image = Some(MemReader::new(reader.read_exact_vec(size as usize)?));
                }
                ID_IMAGE_GLOBAL => {
                    image_global = Some(MemReader::new(reader.read_exact_vec(size as usize)?));
                }
                ID_IMAGE_CONST => {
                    image_const = Some(MemReader::new(reader.read_exact_vec(size as usize)?));
                }
                ID_IMAGE_SHARED => {
                    image_shared = Some(MemReader::new(reader.read_exact_vec(size as usize)?));
                }
                ID_CLASS_INFO => {
                    let mut mem = StreamRegion::with_size(&mut reader, size)?;
                    section_class_info = Some(SectionClassInfo::unpack(
                        &mut mem,
                        false,
                        Encoding::Utf8,
                        &Some(Box::new(section_header.clone())),
                    )?);
                    if mem.stream_position()? != size {
                        eprintln!(
                            "WARNING: Some data is not parsed in ECSExecutionImage::CLASS_INFO"
                        );
                        crate::COUNTER.inc_warning();
                    }
                }
                ID_FUNCTION => {
                    let mut mem = StreamRegion::with_size(&mut reader, size)?;
                    section_function = Some(SectionFunction::unpack(
                        &mut mem,
                        false,
                        Encoding::Utf8,
                        &Some(Box::new(section_header.clone())),
                    )?);
                    if mem.stream_position()? != size {
                        eprintln!(
                            "WARNING: Some data is not parsed in ECSExecutionImage::FUNCTION"
                        );
                        crate::COUNTER.inc_warning();
                    }
                }
                ID_INIT_NAKED_FUNC => {
                    let mut mem = StreamRegion::with_size(&mut reader, size)?;
                    section_init_naked_func = Some(SectionInitNakedFunc::unpack(
                        &mut mem,
                        false,
                        Encoding::Utf8,
                        &Some(Box::new(section_header.clone())),
                    )?);
                    if mem.stream_position()? != size {
                        eprintln!(
                            "WARNING: Some data is not parsed in ECSExecutionImage::INIT_NAKED_FUNC"
                        );
                        crate::COUNTER.inc_warning();
                    }
                }
                ID_FUNC_INFO => {
                    let mut mem = StreamRegion::with_size(&mut reader, size)?;
                    section_func_info = Some(SectionFuncInfo::unpack(
                        &mut mem,
                        false,
                        Encoding::Utf8,
                        &Some(Box::new(section_header.clone())),
                    )?);
                    if mem.stream_position()? != size {
                        eprintln!(
                            "WARNING: Some data is not parsed in ECSExecutionImage::FUNC_INFO"
                        );
                        crate::COUNTER.inc_warning();
                    }
                }
                ID_SYMBOL_INFO => {
                    let mut mem = StreamRegion::with_size(&mut reader, size)?;
                    section_symbol_info = Some(SectionSymbolInfo::unpack(
                        &mut mem,
                        false,
                        Encoding::Utf8,
                        &Some(Box::new(section_header.clone())),
                    )?);
                    if mem.stream_position()? != size {
                        eprintln!(
                            "WARNING: Some data is not parsed in ECSExecutionImage::SYMBOL_INFO"
                        );
                        crate::COUNTER.inc_warning();
                    }
                }
                ID_GLOBAL => {
                    let mut mem = StreamRegion::with_size(&mut reader, size)?;
                    section_global = Some(SectionGlobal::unpack(
                        &mut mem,
                        false,
                        Encoding::Utf8,
                        &Some(Box::new(section_header.clone())),
                    )?);
                    if mem.stream_position()? != size {
                        eprintln!("WARNING: Some data is not parsed in ECSExecutionImage::GLOBAL");
                        crate::COUNTER.inc_warning();
                    }
                }
                ID_DATA => {
                    let mut mem = StreamRegion::with_size(&mut reader, size)?;
                    section_data = Some(SectionData::unpack(
                        &mut mem,
                        false,
                        Encoding::Utf8,
                        &Some(Box::new(section_header.clone())),
                    )?);
                    if mem.stream_position()? != size {
                        eprintln!("WARNING: Some data is not parsed in ECSExecutionImage::DATA");
                        crate::COUNTER.inc_warning();
                    }
                }
                ID_CONST_STRING => {
                    let mut mem = StreamRegion::with_size(&mut reader, size)?;
                    section_const_string = Some(SectionConstString::unpack(
                        &mut mem,
                        false,
                        Encoding::Utf8,
                        &Some(Box::new(section_header.clone())),
                    )?);
                    if mem.stream_position()? != size {
                        eprintln!(
                            "WARNING: Some data is not parsed in ECSExecutionImage::CONST_STRING"
                        );
                        crate::COUNTER.inc_warning();
                    }
                }
                ID_LINK_INFO => {
                    let mut mem = StreamRegion::with_size(&mut reader, size)?;
                    section_link_info = Some(SectionLinkInfo::unpack(
                        &mut mem,
                        false,
                        Encoding::Utf8,
                        &Some(Box::new(section_header.clone())),
                    )?);
                    if mem.stream_position()? != size {
                        eprintln!(
                            "WARNING: Some data is not parsed in ECSExecutionImage::LINK_INFO"
                        );
                        crate::COUNTER.inc_warning();
                    }
                }
                ID_LINK_INFO_EX => {
                    let mut mem = StreamRegion::with_size(&mut reader, size)?;
                    section_link_info_ex = Some(SectionLinkInfoEx::unpack(
                        &mut mem,
                        false,
                        Encoding::Utf8,
                        &Some(Box::new(section_header.clone())),
                    )?);
                    if mem.stream_position()? != size {
                        eprintln!(
                            "WARNING: Some data is not parsed in ECSExecutionImage::LINK_INFO_EX"
                        );
                        crate::COUNTER.inc_warning();
                    }
                }
                ID_REF_FUNC => {
                    let mut mem = StreamRegion::with_size(&mut reader, size)?;
                    section_ref_func = Some(SectionRefFunc::unpack(
                        &mut mem,
                        false,
                        Encoding::Utf8,
                        &Some(Box::new(section_header.clone())),
                    )?);
                    if mem.stream_position()? != size {
                        eprintln!(
                            "WARNING: Some data is not parsed in ECSExecutionImage::REF_FUNC"
                        );
                        crate::COUNTER.inc_warning();
                    }
                }
                ID_REF_CODE => {
                    let mut mem = StreamRegion::with_size(&mut reader, size)?;
                    section_ref_code = Some(SectionRefCode::unpack(
                        &mut mem,
                        false,
                        Encoding::Utf8,
                        &Some(Box::new(section_header.clone())),
                    )?);
                    if mem.stream_position()? != size {
                        eprintln!(
                            "WARNING: Some data is not parsed in ECSExecutionImage::REF_CODE"
                        );
                        crate::COUNTER.inc_warning();
                    }
                }
                ID_REF_CLASS => {
                    let mut mem = StreamRegion::with_size(&mut reader, size)?;
                    section_ref_class = Some(SectionRefClass::unpack(
                        &mut mem,
                        false,
                        Encoding::Utf8,
                        &Some(Box::new(section_header.clone())),
                    )?);
                    if mem.stream_position()? != size {
                        eprintln!(
                            "WARNING: Some data is not parsed in ECSExecutionImage::REF_CLASS"
                        );
                        crate::COUNTER.inc_warning();
                    }
                }
                ID_IMPORT_NATIVE_FUNC => {
                    let mut mem = StreamRegion::with_size(&mut reader, size)?;
                    section_import_native_func = Some(SectionImportNativeFunc::unpack(
                        &mut mem,
                        false,
                        Encoding::Utf8,
                        &Some(Box::new(section_header.clone())),
                    )?);
                    if mem.stream_position()? != size {
                        eprintln!(
                            "WARNING: Some data is not parsed in ECSExecutionImage::IMPORT_NATIVE_FUNC"
                        );
                        crate::COUNTER.inc_warning();
                    }
                }
                0 => {
                    break;
                }
                _ => {
                    return Err(anyhow::anyhow!(
                        "Unknown ECSExecutionImage section ID: 0x{:016X}",
                        id
                    ));
                }
            }
            reader.pos = pos + size as usize;
        }
        Ok(Self {
            file_header,
            section_header,
            image: image.ok_or_else(|| anyhow::anyhow!("Missing image data"))?,
            image_global,
            image_const,
            image_shared,
            section_class_info: section_class_info
                .ok_or_else(|| anyhow::anyhow!("Missing class info section"))?,
            section_function: section_function
                .ok_or_else(|| anyhow::anyhow!("Missing function section"))?,
            section_init_naked_func: section_init_naked_func
                .ok_or_else(|| anyhow::anyhow!("Missing init naked func section"))?,
            section_func_info: section_func_info
                .ok_or_else(|| anyhow::anyhow!("Missing func info section"))?,
            section_symbol_info,
            section_global,
            section_data,
            section_const_string: section_const_string
                .ok_or_else(|| anyhow::anyhow!("Missing const string section"))?,
            section_link_info,
            section_link_info_ex,
            section_ref_func,
            section_ref_code,
            section_ref_class,
            section_import_native_func: section_import_native_func
                .ok_or_else(|| anyhow::anyhow!("Missing import native func section"))?,
            no_part_label: config.entis_gls_csx_no_part_label,
        })
    }

    fn fix_image<'a, 'b>(
        assembly: &ECSExecutionImageAssembly,
        disasm: &mut ECSExecutionImageDisassembler<'a>,
        writer: &mut MemWriter,
        commands: &HashMap<u32, &'b ECSExecutionImageCommandRecord>,
    ) -> Result<()> {
        for cmd in assembly.iter() {
            if cmd.code == CsicEnter {
                disasm.stream.pos = cmd.addr as usize + 1;
                let name_length = disasm.stream.read_u32()?;
                if name_length != 0x80000000 {
                    disasm.stream.pos += name_length as usize * 2;
                } else {
                    disasm.stream.pos += 4;
                }
                let num_args = disasm.stream.read_i32()?;
                if num_args == -1 {
                    let _flag = disasm.stream.read_u8()?;
                    let offset = disasm.stream.pos as i64 - cmd.addr as i64;
                    let original_addr = disasm.stream.read_i32()? as i64 + disasm.stream.pos as i64;
                    let target_cmd = commands.get(&(original_addr as u32)).ok_or_else(|| anyhow::anyhow!(
                        "Cannot find target command at address {:08X} for Enter instruction fixup at {:08X}",
                        original_addr as u32,
                        cmd.addr
                    ))?;
                    let new_addr = target_cmd.new_addr as i64 - cmd.new_addr as i64 - offset - 4;
                    writer.write_i32_at(cmd.new_addr as u64 + offset as u64, new_addr as i32)?;
                }
            } else if matches!(cmd.code, CsicJump | CodeJumpOffset32) {
                disasm.stream.pos = cmd.addr as usize + 1;
                let offset = disasm.stream.pos as i64 - cmd.addr as i64;
                let original_addr = disasm.stream.read_i32()? as i64 + disasm.stream.pos as i64;
                let target_cmd = commands.get(&(original_addr as u32)).ok_or_else(|| anyhow::anyhow!(
                    "Cannot find target command at address {:08X} for {:?} instruction fixup at {:08X}",
                    original_addr as u32,
                    cmd.code,
                    cmd.addr
                ))?;
                let new_addr = target_cmd.new_addr as i64 - cmd.new_addr as i64 - offset - 4;
                writer.write_i32_at(cmd.new_addr as u64 + offset as u64, new_addr as i32)?;
            } else if matches!(cmd.code, CsicCJump | CodeCJumpOffset32 | CodeCNJumpOffset32) {
                disasm.stream.pos = cmd.addr as usize + 2;
                let offset = disasm.stream.pos as i64 - cmd.addr as i64;
                let original_addr = disasm.stream.read_i32()? as i64 + disasm.stream.pos as i64;
                let target_cmd = commands.get(&(original_addr as u32)).ok_or_else(|| anyhow::anyhow!(
                    "Cannot find target command at address {:08X} for {:?} instruction fixup at {:08X}",
                    original_addr as u32,
                    cmd.code,
                    cmd.addr
                ))?;
                let new_addr = target_cmd.new_addr as i64 - cmd.new_addr as i64 - offset - 4;
                writer.write_i32_at(cmd.new_addr as u64 + offset as u64, new_addr as i32)?;
            } else if cmd.code == CsicExCall {
                disasm.stream.pos = cmd.addr as usize + 1;
                let _arg_count = disasm.stream.read_i32()?;
                let csom = disasm.read_csom()?;
                let csvt = disasm.read_csvt()?;
                if csom == CsomImmediate && csvt == CsvtInteger {
                    let offset = disasm.stream.pos as i64 - cmd.addr as i64;
                    let addr = disasm.stream.read_u32()?;
                    let target_cmd = commands.get(&addr).ok_or_else(|| anyhow::anyhow!(
                        "Cannot find target command at address {:08X} for ExCall instruction fixup at {:08X}",
                        addr,
                        cmd.addr
                    ))?;
                    let new_addr = target_cmd.new_addr;
                    writer.write_u32_at(cmd.new_addr as u64 + offset as u64, new_addr)?;
                }
            } else if cmd.code == CodeCallImm32 {
                disasm.stream.pos = cmd.addr as usize + 1;
                let offset = disasm.stream.pos as i64 - cmd.addr as i64;
                let addr = disasm.stream.read_u32()?;
                let target_cmd = commands.get(&addr).ok_or_else(|| anyhow::anyhow!(
                    "Cannot find target command at address {:08X} for CallImm32 instruction fixup at {:08X}",
                    addr,
                    cmd.addr
                ))?;
                let new_addr = target_cmd.new_addr;
                writer.write_u32_at(cmd.new_addr as u64 + offset as u64, new_addr)?;
            }
        }
        Ok(())
    }

    fn fix_references(
        &mut self,
        commands: &HashMap<u32, &ECSExecutionImageCommandRecord>,
    ) -> Result<()> {
        let mut list: Vec<u32> = commands.iter().map(|(&k, _)| k).collect();
        list.sort();
        for cmd in self.section_function.prologue.iter_mut() {
            let ocmd = *cmd;
            if let Some(tcmd) = commands.get(&ocmd) {
                *cmd = tcmd.new_addr;
            } else {
                let pre_one_idx = match list.binary_search(&ocmd) {
                    Ok(idx) => idx,
                    Err(idx) => {
                        if idx == 0 {
                            idx
                        } else {
                            idx - 1
                        }
                    }
                };
                let tcmd = &commands[&list[pre_one_idx]];
                if !tcmd.internal || tcmd.size + tcmd.addr < ocmd {
                    return Err(anyhow::anyhow!(
                        "Cannot find target command at address {:08X} for PIF prologue fixup",
                        ocmd
                    ));
                }
                let offset = tcmd.new_addr as i64 - tcmd.addr as i64;
                *cmd = (ocmd as i64 + offset) as u32;
            }
        }
        for cmd in self.section_function.epilogue.iter_mut() {
            let ocmd = *cmd;
            *cmd = commands
                .get(&ocmd)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Cannot find target command at address {:08X} for PIF epilogue fixup",
                        ocmd
                    )
                })?
                .new_addr;
        }
        for func in self.section_function.func_names.iter_mut() {
            let ocmd = func.address;
            func.address = commands
                .get(&ocmd)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Cannot find target command at address {:08X} for function names list fixup",
                        ocmd
                    )
                })?
                .new_addr;
        }
        for cmd in self.section_init_naked_func.naked_prologue.iter_mut() {
            let ocmd = *cmd;
            *cmd = commands
                .get(&ocmd)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Cannot find target command at address {:08X} for NNF prologue fixup",
                        ocmd
                    )
                })?
                .new_addr;
        }
        for cmd in self.section_init_naked_func.naked_epilogue.iter_mut() {
            let ocmd = *cmd;
            *cmd = commands
                .get(&ocmd)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Cannot find target command at address {:08X} for NNF epilogue fixup",
                        ocmd
                    )
                })?
                .new_addr;
        }
        for func in self.section_func_info.functions.iter_mut() {
            let ocmd = func.header.address;
            func.header.address = commands
                .get(&ocmd)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Cannot find target command at address {:08X} for function info list fixup",
                        ocmd
                    )
                })?
                .new_addr;
            if func.header.bytes != u32::MAX {
                let end_ocmd = ocmd + func.header.bytes;
                let end_tcmd = commands
                    .get(&end_ocmd)
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "Cannot find target command at address {:08X} for function info list fixup",
                            end_ocmd
                        )
                    })?.new_addr;
                func.header.bytes = end_tcmd - func.header.address;
            }
        }
        Ok(())
    }

    fn save<'a>(&self, mut writer: Box<dyn Write + 'a>) -> Result<()> {
        self.file_header
            .pack(&mut writer, false, Encoding::Utf8, &None)?;
        if self.section_header.header_size > 0 {
            let mut mem = MemWriter::new();
            self.section_header
                .pack(&mut mem, false, Encoding::Utf8, &None)?;
            writer.write_u64(ID_HEADER)?;
            writer.write_u64(mem.data.len() as u64)?;
            writer.write_all(&mem.into_inner())?;
        }
        writer.write_u64(ID_IMAGE)?;
        writer.write_u64(self.image.data.len() as u64)?;
        writer.write_all(&self.image.data)?;
        if let Some(img_global) = &self.image_global {
            writer.write_u64(ID_IMAGE_GLOBAL)?;
            writer.write_u64(img_global.data.len() as u64)?;
            writer.write_all(&img_global.data)?;
        }
        if let Some(img_const) = &self.image_const {
            writer.write_u64(ID_IMAGE_CONST)?;
            writer.write_u64(img_const.data.len() as u64)?;
            writer.write_all(&img_const.data)?;
        }
        if let Some(img_shared) = &self.image_shared {
            writer.write_u64(ID_IMAGE_SHARED)?;
            writer.write_u64(img_shared.data.len() as u64)?;
            writer.write_all(&img_shared.data)?;
        }
        writer.write_u64(ID_CLASS_INFO)?;
        let mut mem = MemWriter::new();
        self.section_class_info.pack(
            &mut mem,
            false,
            Encoding::Utf8,
            &Some(Box::new(self.section_header.clone())),
        )?;
        writer.write_u64(mem.data.len() as u64)?;
        writer.write_all(&mem.into_inner())?;
        writer.write_u64(ID_FUNCTION)?;
        let mut mem = MemWriter::new();
        self.section_function.pack(
            &mut mem,
            false,
            Encoding::Utf8,
            &Some(Box::new(self.section_header.clone())),
        )?;
        writer.write_u64(mem.data.len() as u64)?;
        writer.write_all(&mem.into_inner())?;
        writer.write_u64(ID_INIT_NAKED_FUNC)?;
        let mut mem = MemWriter::new();
        self.section_init_naked_func.pack(
            &mut mem,
            false,
            Encoding::Utf8,
            &Some(Box::new(self.section_header.clone())),
        )?;
        writer.write_u64(mem.data.len() as u64)?;
        writer.write_all(&mem.into_inner())?;
        writer.write_u64(ID_FUNC_INFO)?;
        let mut mem = MemWriter::new();
        self.section_func_info.pack(
            &mut mem,
            false,
            Encoding::Utf8,
            &Some(Box::new(self.section_header.clone())),
        )?;
        writer.write_u64(mem.data.len() as u64)?;
        writer.write_all(&mem.into_inner())?;
        if let Some(section_symbol_info) = &self.section_symbol_info {
            writer.write_u64(ID_SYMBOL_INFO)?;
            let mut mem = MemWriter::new();
            section_symbol_info.pack(
                &mut mem,
                false,
                Encoding::Utf8,
                &Some(Box::new(self.section_header.clone())),
            )?;
            writer.write_u64(mem.data.len() as u64)?;
            writer.write_all(&mem.into_inner())?;
        }
        if let Some(section_global) = &self.section_global {
            writer.write_u64(ID_GLOBAL)?;
            let mut mem = MemWriter::new();
            section_global.pack(
                &mut mem,
                false,
                Encoding::Utf8,
                &Some(Box::new(self.section_header.clone())),
            )?;
            writer.write_u64(mem.data.len() as u64)?;
            writer.write_all(&mem.into_inner())?;
        }
        if let Some(section_data) = &self.section_data {
            writer.write_u64(ID_DATA)?;
            let mut mem = MemWriter::new();
            section_data.pack(
                &mut mem,
                false,
                Encoding::Utf8,
                &Some(Box::new(self.section_header.clone())),
            )?;
            writer.write_u64(mem.data.len() as u64)?;
            writer.write_all(&mem.into_inner())?;
        }
        writer.write_u64(ID_CONST_STRING)?;
        let mut mem = MemWriter::new();
        self.section_const_string.pack(
            &mut mem,
            false,
            Encoding::Utf8,
            &Some(Box::new(self.section_header.clone())),
        )?;
        writer.write_u64(mem.data.len() as u64)?;
        writer.write_all(&mem.into_inner())?;
        if let Some(section_link_info) = &self.section_link_info {
            writer.write_u64(ID_LINK_INFO)?;
            let mut mem = MemWriter::new();
            section_link_info.pack(
                &mut mem,
                false,
                Encoding::Utf8,
                &Some(Box::new(self.section_header.clone())),
            )?;
            writer.write_u64(mem.data.len() as u64)?;
            writer.write_all(&mem.into_inner())?;
        }
        if let Some(section_link_info_ex) = &self.section_link_info_ex {
            writer.write_u64(ID_LINK_INFO_EX)?;
            let mut mem = MemWriter::new();
            section_link_info_ex.pack(
                &mut mem,
                false,
                Encoding::Utf8,
                &Some(Box::new(self.section_header.clone())),
            )?;
            writer.write_u64(mem.data.len() as u64)?;
            writer.write_all(&mem.into_inner())?;
        }
        if let Some(section_ref_func) = &self.section_ref_func {
            writer.write_u64(ID_REF_FUNC)?;
            let mut mem = MemWriter::new();
            section_ref_func.pack(
                &mut mem,
                false,
                Encoding::Utf8,
                &Some(Box::new(self.section_header.clone())),
            )?;
            writer.write_u64(mem.data.len() as u64)?;
            writer.write_all(&mem.into_inner())?;
        }
        if let Some(section_ref_code) = &self.section_ref_code {
            writer.write_u64(ID_REF_CODE)?;
            let mut mem = MemWriter::new();
            section_ref_code.pack(
                &mut mem,
                false,
                Encoding::Utf8,
                &Some(Box::new(self.section_header.clone())),
            )?;
            writer.write_u64(mem.data.len() as u64)?;
            writer.write_all(&mem.into_inner())?;
        }
        if let Some(section_ref_class) = &self.section_ref_class {
            writer.write_u64(ID_REF_CLASS)?;
            let mut mem = MemWriter::new();
            section_ref_class.pack(
                &mut mem,
                false,
                Encoding::Utf8,
                &Some(Box::new(self.section_header.clone())),
            )?;
            writer.write_u64(mem.data.len() as u64)?;
            writer.write_all(&mem.into_inner())?;
        }
        writer.write_u64(ID_IMPORT_NATIVE_FUNC)?;
        let mut mem = MemWriter::new();
        self.section_import_native_func.pack(
            &mut mem,
            false,
            Encoding::Utf8,
            &Some(Box::new(self.section_header.clone())),
        )?;
        writer.write_u64(mem.data.len() as u64)?;
        writer.write_all(&mem.into_inner())?;
        Ok(())
    }
}

impl ECSImage for ECSExecutionImage {
    fn disasm<'a>(&self, writer: Box<dyn std::io::Write + 'a>) -> Result<()> {
        let mut disasm = ECSExecutionImageDisassembler::new(
            self.image.to_ref(),
            &self.section_function,
            &self.section_func_info,
            &self.section_import_native_func,
            &self.section_class_info,
            &self.section_const_string,
            Some(writer),
        );
        disasm.execute()?;
        Ok(())
    }

    fn export(&self) -> Result<Vec<Message>> {
        let mut messages = Vec::new();
        let mut disasm = ECSExecutionImageDisassembler::new(
            self.image.to_ref(),
            &self.section_function,
            &self.section_func_info,
            &self.section_import_native_func,
            &self.section_class_info,
            &self.section_const_string,
            None,
        );
        disasm.execute()?;
        let assembly = disasm.assembly.clone();
        let mut string_stack = Vec::new();
        let mut stacks = Vec::new();
        let mut index = 0;
        let len = assembly.len();
        while index < len {
            let cmd = &assembly[index];
            if cmd.code == CsicLoad {
                disasm.stream.pos = cmd.addr as usize + 1;
                let csom = disasm.read_csom()?;
                let csvt = disasm.read_csvt()?;
                let is_string = csom == CsomImmediate && csvt == CsvtString;
                if is_string {
                    let s = disasm.get_string_literal()?;
                    string_stack.push(s);
                }
                stacks.push(is_string);
            } else if matches!(
                cmd.code,
                CsicCall
                    | CsicCallMember
                    | CsicCallNativeFunction
                    | CsicCallNativeMember
                    | CsicEnter
                    | CsicElementIndirect
            ) {
                string_stack.clear();
                stacks.clear();
            } else if cmd.code == CsicOperate {
                disasm.stream.pos = cmd.addr as usize + 1;
                let csot = disasm.read_csot()?;
                if csot == CsotAdd {
                    if string_stack.len() >= 2
                        && index >= 2
                        && stacks.len() >= 2
                        && stacks[stacks.len() - 1]
                        && stacks[stacks.len() - 2]
                    {
                        let s2 = string_stack.pop().unwrap();
                        let s1 = string_stack.pop().unwrap();
                        let s = s1 + &s2;
                        string_stack.push(s);
                        stacks.pop();
                        // Remove the two previous load commands and replace with this one
                        index += 1;
                        continue;
                    }
                }
                if let Some(is_str) = stacks.pop() {
                    if is_str && string_stack.is_empty() {
                        return Err(anyhow::anyhow!(
                            "String stack is empty when processing Operate at {:08X}",
                            cmd.addr,
                        ));
                    }
                    if is_str {
                        string_stack.pop();
                    }
                }
            } else if cmd.code == CsicExCall {
                disasm.stream.pos = cmd.addr as usize + 1;
                let arg_count = disasm.stream.read_i32()?;
                let csom = disasm.read_csom()?;
                let csvt = disasm.read_csvt()?;
                if csom == CsomImmediate {
                    let func_name = if csvt == CsvtString {
                        disasm.get_string_literal()?
                    } else if csvt == CsvtInteger {
                        let func_address = disasm.stream.read_u32()?;
                        let func = disasm.func_map.get(&func_address).ok_or_else(|| {
                            anyhow::anyhow!(
                                "Function address 0x{:08X} not found in ExCall",
                                func_address
                            )
                        })?;
                        func.name.0.clone()
                    } else {
                        return Err(anyhow::anyhow!(
                            "Unexpected CSVT for function name in ExCall"
                        ));
                    };
                    if func_name == "WitchWizard::OutMsg" && arg_count == 8 {
                        if string_stack.len() < 2 {
                            return Err(anyhow::anyhow!(
                                "String stack has less than 2 items when processing OutMsg at {:08X}",
                                cmd.addr,
                            ));
                        }
                        if string_stack.len() > 2 {
                            eprintln!(
                                "WARNING: String stack has more than 2 items when processing OutMsg at {:08X}",
                                cmd.addr,
                            );
                            crate::COUNTER.inc_warning();
                        }
                        let name = string_stack[0].clone();
                        let message = string_stack[1].clone();
                        messages.push(Message {
                            name: if name.is_empty() { None } else { Some(name) },
                            message,
                        });
                    }
                }
                string_stack.clear();
                stacks.clear();
            }
            index += 1;
        }
        Ok(messages)
    }

    fn export_multi(&self) -> Result<HashMap<String, Vec<Message>>> {
        let mut key = String::from("global");
        let mut messages: HashMap<String, Vec<Message>> = HashMap::new();
        let mut disasm = ECSExecutionImageDisassembler::new(
            self.image.to_ref(),
            &self.section_function,
            &self.section_func_info,
            &self.section_import_native_func,
            &self.section_class_info,
            &self.section_const_string,
            None,
        );
        disasm.execute()?;
        let assembly = disasm.assembly.clone();
        let mut string_stack = Vec::new();
        let mut stacks = Vec::new();
        let mut index = 0;
        let len = assembly.len();
        while index < len {
            let cmd = &assembly[index];
            if cmd.code == CsicLoad {
                disasm.stream.pos = cmd.addr as usize + 1;
                let csom = disasm.read_csom()?;
                let csvt = disasm.read_csvt()?;
                let is_string = csom == CsomImmediate && csvt == CsvtString;
                if is_string {
                    let s = disasm.get_string_literal()?;
                    string_stack.push(s);
                }
                stacks.push(is_string);
            } else if matches!(
                cmd.code,
                CsicCall
                    | CsicCallMember
                    | CsicCallNativeFunction
                    | CsicCallNativeMember
                    | CsicEnter
                    | CsicElementIndirect
            ) {
                string_stack.clear();
                stacks.clear();
            } else if cmd.code == CsicOperate {
                disasm.stream.pos = cmd.addr as usize + 1;
                let csot = disasm.read_csot()?;
                if csot == CsotAdd {
                    if string_stack.len() >= 2
                        && index >= 2
                        && stacks.len() >= 2
                        && stacks[stacks.len() - 1]
                        && stacks[stacks.len() - 2]
                    {
                        let s2 = string_stack.pop().unwrap();
                        let s1 = string_stack.pop().unwrap();
                        let s = s1 + &s2;
                        string_stack.push(s);
                        stacks.pop();
                        // Remove the two previous load commands and replace with this one
                        index += 1;
                        continue;
                    }
                }
                if let Some(is_str) = stacks.pop() {
                    if is_str && string_stack.is_empty() {
                        return Err(anyhow::anyhow!(
                            "String stack is empty when processing Operate at {:08X}",
                            cmd.addr,
                        ));
                    }
                    if is_str {
                        string_stack.pop();
                    }
                }
            } else if cmd.code == CsicExCall {
                disasm.stream.pos = cmd.addr as usize + 1;
                let arg_count = disasm.stream.read_i32()?;
                let csom = disasm.read_csom()?;
                let csvt = disasm.read_csvt()?;
                if csom == CsomImmediate {
                    let func_name = if csvt == CsvtString {
                        disasm.get_string_literal()?
                    } else if csvt == CsvtInteger {
                        let func_address = disasm.stream.read_u32()?;
                        let func = disasm.func_map.get(&func_address).ok_or_else(|| {
                            anyhow::anyhow!(
                                "Function address 0x{:08X} not found in ExCall",
                                func_address
                            )
                        })?;
                        func.name.0.clone()
                    } else {
                        return Err(anyhow::anyhow!(
                            "Unexpected CSVT for function name in ExCall"
                        ));
                    };
                    if func_name == "WitchWizard::SetPastLabel"
                        && arg_count == 2
                        && !self.no_part_label
                    {
                        if string_stack.is_empty() {
                            return Err(anyhow::anyhow!(
                                "String stack is empty when processing SetPastLabel"
                            ));
                        }
                        if string_stack.len() > 1 {
                            eprintln!(
                                "WARNING: String stack has more than 1 item when processing SetPastLabel at {:08X}",
                                cmd.addr,
                            );
                            crate::COUNTER.inc_warning();
                        }
                        key = string_stack[0].clone();
                    } else if func_name == "WitchWizard::OutMsg" && arg_count == 8 {
                        if string_stack.len() < 2 {
                            return Err(anyhow::anyhow!(
                                "String stack has less than 2 items when processing OutMsg at {:08X}",
                                cmd.addr,
                            ));
                        }
                        if string_stack.len() > 2 {
                            eprintln!(
                                "WARNING: String stack has more than 2 items when processing OutMsg at {:08X}",
                                cmd.addr,
                            );
                            crate::COUNTER.inc_warning();
                        }
                        let name = string_stack[0].clone();
                        let message = string_stack[1].clone();
                        messages
                            .entry(key.clone())
                            .or_insert_with(Vec::new)
                            .push(Message {
                                name: if name.is_empty() { None } else { Some(name) },
                                message,
                            });
                    } else if func_name == "WitchWizard::SetCurrentScriptName" && arg_count == 2 {
                        if string_stack.is_empty() {
                            return Err(anyhow::anyhow!(
                                "String stack is empty when processing SetCurrentScriptName"
                            ));
                        }
                        if string_stack.len() > 1 {
                            eprintln!(
                                "WARNING: String stack has more than 1 item when processing SetCurrentScriptName at {:08X}",
                                cmd.addr,
                            );
                            crate::COUNTER.inc_warning();
                        }
                        key = string_stack[0].clone();
                    }
                }
                string_stack.clear();
                stacks.clear();
            }
            index += 1;
        }
        Ok(messages)
    }

    fn export_all(&self) -> Result<Vec<String>> {
        let mut disasm = ECSExecutionImageDisassembler::new(
            self.image.to_ref(),
            &self.section_function,
            &self.section_func_info,
            &self.section_import_native_func,
            &self.section_class_info,
            &self.section_const_string,
            None,
        );
        disasm.execute()?;
        let mut messages = Vec::new();
        for cmd in disasm.assembly.clone().iter() {
            if cmd.code == CsicLoad {
                disasm.stream.pos = cmd.addr as usize + 1;
                let csom = disasm.read_csom()?;
                let csvt = disasm.read_csvt()?;
                if csom == CsomImmediate && csvt == CsvtString {
                    let s = disasm.get_string_literal()?;
                    messages.push(s);
                }
            }
        }
        Ok(messages)
    }

    fn import<'a>(
        &self,
        _messages: Vec<Message>,
        _file: Box<dyn WriteSeek + 'a>,
        _replacement: Option<&'a ReplacementTable>,
    ) -> Result<()> {
        Err(anyhow::anyhow!("Import not implemented for CSX v2"))
    }

    fn import_multi<'a>(
        &self,
        _messages: HashMap<String, Vec<Message>>,
        _file: Box<dyn WriteSeek + 'a>,
        _replacement: Option<&'a ReplacementTable>,
    ) -> Result<()> {
        Err(anyhow::anyhow!("Import multi not implemented for CSX v2"))
    }

    fn import_all<'a>(&self, messages: Vec<String>, file: Box<dyn WriteSeek + 'a>) -> Result<()> {
        let mut cloned = self.clone();
        let mut mess = messages.into_iter();
        let mut mes = mess.next();
        let mut disasm = ECSExecutionImageDisassembler::new(
            self.image.to_ref(),
            &self.section_function,
            &self.section_func_info,
            &self.section_import_native_func,
            &self.section_class_info,
            &self.section_const_string,
            None,
        );
        disasm.execute()?;
        let mut conststr_map: HashMap<String, u32> = cloned
            .section_const_string
            .strings
            .iter()
            .enumerate()
            .map(|(i, s)| (s.string.0.clone(), i as u32))
            .collect();
        let mut assembly = disasm.assembly.clone();
        let mut new_image = MemWriter::new();
        for cmd in assembly.iter_mut() {
            cmd.new_addr = new_image.pos as u32;
            if cmd.code == CsicLoad {
                disasm.stream.pos = cmd.addr as usize + 1;
                let csom = disasm.read_csom()?;
                let csvt = disasm.read_csvt()?;
                if csom == CsomImmediate && csvt == CsvtString {
                    let s = match mes {
                        Some(s) => s,
                        None => {
                            return Err(anyhow::anyhow!(
                                "Not enough messages for import_all at {:08X}",
                                cmd.addr,
                            ));
                        }
                    };
                    mes = mess.next();
                    let constr_idx = if let Some(&idx) = conststr_map.get(&s) {
                        idx
                    } else {
                        // Add new string to const string section
                        let idx = cloned.section_const_string.strings.len() as u32;
                        cloned.section_const_string.strings.push(ConstStringEntry {
                            string: WideString(s.clone()),
                            refs: DWordArray { data: Vec::new() },
                        });
                        conststr_map.insert(s.clone(), idx);
                        idx
                    };
                    new_image.write_u8(CsicLoad as u8)?;
                    new_image.write_u8(CsomImmediate as u8)?;
                    new_image.write_u8(CsvtString as u8)?;
                    new_image.write_u32(0x80000000)?;
                    new_image.write_u32(constr_idx)?;
                    continue;
                }
            }
            // Copy original command
            new_image.write_from(&mut disasm.stream, cmd.addr as u64, cmd.size as u64)?;
        }
        if mes.is_some() || mess.next().is_some() {
            return Err(anyhow::anyhow!("Too many messages for import_all"));
        }
        let commands: HashMap<u32, &ECSExecutionImageCommandRecord> =
            assembly.iter().map(|c| (c.addr, c)).collect();
        Self::fix_image(&assembly, &mut disasm, &mut new_image, &commands)?;
        cloned.image = MemReader::new(new_image.into_inner());
        cloned.fix_references(&commands)?;
        cloned.save(file)?;
        Ok(())
    }
}
