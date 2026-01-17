use super::disasm::*;
use super::types::*;
use crate::ext::io::*;
use crate::types::*;
use crate::utils::struct_pack::*;
use anyhow::Result;
use std::io::Write;

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct ECSExecutionImage {
    file_header: EMCFileHeader,
    exi_header: Option<Vec<u8>>,
    header: Option<EXIHeader>,
    image: MemReader,
    pif_prologue: DWordArray,
    pif_epilogue: DWordArray,
    function_list: FunctionNameList,
    csg_global: ECSGlobal,
    csg_data: ECSGlobal,
    ext_const_str: Option<TaggedRefAddressList>,
    ext_global_ref: DWordArray,
    ext_data_ref: DWordArray,
    imp_global_ref: TaggedRefAddressList,
    imp_data_ref: TaggedRefAddressList,
}

impl ECSExecutionImage {
    pub fn new(mut reader: MemReader) -> Result<Self> {
        let file_header = EMCFileHeader::unpack(&mut reader, false, Encoding::Utf8)?;
        // if file_header.signagure != *b"Entis\x1a\0\0" {
        //     return Err(anyhow::anyhow!("Invalid EMC file signature"));
        // }
        let len = reader.data.len();
        let mut exi_header = None;
        let mut header = None;
        let mut image = None;
        let mut pif_prologue = None;
        let mut pif_epilogue = None;
        let mut function_list = None;
        let mut csg_global = None;
        let mut int64 = false;
        let mut csg_data = None;
        let mut ext_const_str = None;
        let mut ext_global_ref = DWordArray::default();
        let mut ext_data_ref = DWordArray::default();
        let mut imp_global_ref = TaggedRefAddressList::default();
        let mut imp_data_ref = TaggedRefAddressList::default();
        while reader.pos < len {
            if len - reader.pos < 16 {
                break;
            }
            let id = reader.read_u64()?;
            if id == 0 {
                break;
            }
            let size = reader.read_u64()?;
            match id {
                // header
                0x2020726564616568 => {
                    let buf = reader.read_exact_vec(size as usize)?;
                    {
                        let mut sread = MemReaderRef::new(&buf);
                        header = Some(EXIHeader::unpack(&mut sread, false, Encoding::Utf8)?);
                    }
                    exi_header = Some(buf);
                    if let Some(hdr) = &header {
                        if hdr.int_base == 64 {
                            int64 = true;
                        }
                    }
                }
                // image
                0x2020206567616D69 => {
                    image = Some(MemReader::new(reader.read_exact_vec(size as usize)?));
                }
                // function
                0x6E6F6974636E7566 => {
                    pif_prologue = Some(DWordArray::unpack(&mut reader, false, Encoding::Utf8)?);
                    pif_epilogue = Some(DWordArray::unpack(&mut reader, false, Encoding::Utf8)?);
                    function_list = Some(FunctionNameList::unpack(
                        &mut reader,
                        false,
                        Encoding::Utf8,
                    )?);
                }
                // global
                0x20206C61626F6C67 => {
                    let count = reader.read_u32()?;
                    let mut items = Vec::with_capacity(count as usize);
                    for _ in 0..count {
                        let name = WideString::unpack(&mut reader, false, Encoding::Utf16LE)?.0;
                        let obj = ECSObject::read_from(&mut reader, int64)?;
                        items.push(ECSObjectItem { name, obj });
                    }
                    csg_global = Some(ECSGlobal(items));
                }
                // data
                0x2020202061746164 => {
                    let count = reader.read_u32()?;
                    let mut items = Vec::with_capacity(count as usize);
                    for _ in 0..count {
                        let name = WideString::unpack(&mut reader, false, Encoding::Utf16LE)?.0;
                        let length = reader.read_i32()?;
                        let obj = if length >= 0 {
                            let mut datas = Vec::with_capacity(length as usize);
                            for _ in 0..length {
                                let name =
                                    WideString::unpack(&mut reader, false, Encoding::Utf16LE)?.0;
                                let obj = ECSObject::read_from(&mut reader, int64)?;
                                datas.push(ECSObjectItem { name, obj });
                            }
                            ECSObject::Global(ECSGlobal(datas))
                        } else {
                            ECSObject::read_from(&mut reader, int64)?
                        };
                        items.push(ECSObjectItem { name, obj });
                    }
                    csg_data = Some(ECSGlobal(items));
                }
                // conststr
                0x72747374736E6F63 => {
                    ext_const_str = Some(TaggedRefAddressList::unpack(
                        &mut reader,
                        false,
                        Encoding::Utf8,
                    )?);
                }
                // linkinf
                0x20666E696B6E696C => {
                    ext_global_ref = DWordArray::unpack(&mut reader, false, Encoding::Utf8)?;
                    ext_data_ref = DWordArray::unpack(&mut reader, false, Encoding::Utf8)?;
                    imp_global_ref =
                        TaggedRefAddressList::unpack(&mut reader, false, Encoding::Utf8)?;
                    imp_data_ref =
                        TaggedRefAddressList::unpack(&mut reader, false, Encoding::Utf8)?;
                    if !ext_global_ref.is_empty()
                        || !ext_data_ref.is_empty()
                        || !imp_global_ref.is_empty()
                        || !imp_data_ref.is_empty()
                    {
                        eprintln!(
                            "Warning: External/global references(linker data) are not supported and will be ignored. This may cause script rebuild errors."
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
        }
        Ok(Self {
            file_header,
            exi_header,
            header,
            image: image.ok_or_else(|| anyhow::anyhow!("Missing image data"))?,
            pif_prologue: pif_prologue.ok_or_else(|| anyhow::anyhow!("Missing PIF prologue"))?,
            pif_epilogue: pif_epilogue.ok_or_else(|| anyhow::anyhow!("Missing PIF epilogue"))?,
            function_list: function_list.ok_or_else(|| anyhow::anyhow!("Missing function list"))?,
            csg_global: csg_global.ok_or_else(|| anyhow::anyhow!("Missing CSG global"))?,
            csg_data: csg_data.ok_or_else(|| anyhow::anyhow!("Missing CSG data"))?,
            ext_const_str,
            ext_global_ref,
            ext_data_ref,
            imp_global_ref,
            imp_data_ref,
        })
    }

    pub fn disasm<'a>(&self, writer: Box<dyn Write + 'a>) -> Result<()> {
        let mut disasm = ECSExecutionImageDisassembler::new(
            self.image.to_ref(),
            self.ext_const_str.as_ref(),
            Some(writer),
        );
        disasm.execute()?;
        Ok(())
    }
}
