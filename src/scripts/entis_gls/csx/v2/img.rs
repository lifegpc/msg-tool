use super::super::base::*;
use super::types::*;
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::struct_pack::*;
use anyhow::Result;
use std::collections::HashMap;
use std::io::Seek;

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
}

impl ECSExecutionImage {
    pub fn new(mut reader: MemReaderRef<'_>, _config: &ExtraConfig) -> Result<Self> {
        let file_header = FileHeader::unpack(&mut reader, false, Encoding::Utf8, &None)?;
        if file_header.signagure != *b"Entis\x1a\0\0" {
            return Err(anyhow::anyhow!("Invalid EMC file signature"));
        }
        if !file_header.format_desc.starts_with(b"Cotopha Image file") {
            return Err(anyhow::anyhow!("Invalid EMC file format description"));
        }
        let mut section_header = SectionHeader::default();
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
        })
    }
}

impl ECSImage for ECSExecutionImage {
    fn disasm<'a>(&self, _writer: Box<dyn std::io::Write + 'a>) -> Result<()> {
        Err(anyhow::anyhow!("Disassembly not implemented for CSX v2"))
    }

    fn export(&self) -> Result<Vec<Message>> {
        Err(anyhow::anyhow!("Export not implemented for CSX v2"))
    }

    fn export_multi(&self) -> Result<HashMap<String, Vec<Message>>> {
        Err(anyhow::anyhow!("Export multi not implemented for CSX v2"))
    }

    fn export_all(&self) -> Result<Vec<String>> {
        Err(anyhow::anyhow!("Export all not implemented for CSX v2"))
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

    fn import_all<'a>(&self, _messages: Vec<String>, _file: Box<dyn WriteSeek + 'a>) -> Result<()> {
        Err(anyhow::anyhow!("Import all not implemented for CSX v2"))
    }
}
