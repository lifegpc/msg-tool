use super::disasm::*;
use super::types::*;
use crate::ext::io::*;
use crate::types::*;
use crate::utils::struct_pack::*;
use anyhow::Result;
use std::collections::HashMap;
use std::io::Write;

use CSInstructionCode::*;
use CSObjectMode::*;
use CSVariableType::*;

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

    pub fn export(&self) -> Result<Vec<Message>> {
        let mut disasm = ECSExecutionImageDisassembler::new(
            self.image.to_ref(),
            self.ext_const_str.as_ref(),
            None,
        );
        disasm.execute()?;
        let mut messages = Vec::new();
        let assembly = disasm.assembly.clone();
        let mut name = None;
        let mut pre_is_mess = false;
        let mut string_stack = Vec::new();
        let mut message = String::new();
        for cmd in assembly.iter() {
            if cmd.code == CsicLoad {
                disasm.stream.pos = cmd.addr as usize + 1;
                let csom = disasm.read_csom()?;
                let csvt = disasm.read_csvt()?;
                if csom == CsomImmediate && csvt == CsvtString {
                    let text = disasm.get_string_literal()?;
                    string_stack.insert(0, text);
                    if string_stack.len() > 8 {
                        string_stack.pop();
                    }
                }
            } else if cmd.code == CsicCall {
                disasm.stream.pos = cmd.addr as usize + 1;
                let _csom = disasm.read_csom()?;
                let num_args = disasm.stream.read_i32()?;
                let func_name = WideString::unpack(&mut disasm.stream, false, Encoding::Utf16LE)?.0;
                let mut is_mess = false;
                if num_args == 1 {
                    if func_name == "SceneTitle" {
                        if string_stack.is_empty() {
                            return Err(anyhow::anyhow!(
                                "SceneTitle call without string argument at {:08X}",
                                cmd.addr
                            ));
                        }
                        messages.push(Message::new(string_stack[0].clone(), None));
                    } else if func_name == "Mess" {
                        is_mess = true;
                        if string_stack.is_empty() {
                            return Err(anyhow::anyhow!(
                                "Mess call without string argument at {:08X}",
                                cmd.addr
                            ));
                        }
                        message.push_str(string_stack[0].as_str());
                    }
                } else if num_args == 2 {
                    if func_name == "Talk" {
                        if string_stack.is_empty() {
                            return Err(anyhow::anyhow!(
                                "Talk call without string argument at {:08X}",
                                cmd.addr
                            ));
                        }
                        name = Some(string_stack[0].clone());
                    } else if func_name == "AddSelect" {
                        if string_stack.is_empty() {
                            return Err(anyhow::anyhow!(
                                "AddSelect call without string argument at {:08X}",
                                cmd.addr
                            ));
                        }
                        messages.push(Message::new(string_stack[0].clone(), None));
                    }
                }
                if pre_is_mess && !is_mess {
                    messages.push(Message::new(message.clone(), name.take()));
                    message.clear();
                }
                pre_is_mess = is_mess;
            }
        }
        Ok(messages)
    }

    pub fn export_multi(&self) -> Result<HashMap<String, Vec<Message>>> {
        let mut key = String::from("global");
        let mut messages = HashMap::new();
        let mut disasm = ECSExecutionImageDisassembler::new(
            self.image.to_ref(),
            self.ext_const_str.as_ref(),
            None,
        );
        disasm.execute()?;
        let assembly = disasm.assembly.clone();
        let mut name = None;
        let mut pre_is_mess = false;
        let mut pre_is_enter = false;
        let mut string_stack = Vec::new();
        let mut message = String::new();
        let mut pre_enter_name = String::new();
        for cmd in assembly.iter() {
            let is_enter = cmd.code == CsicEnter;
            if cmd.code == CsicLoad {
                disasm.stream.pos = cmd.addr as usize + 1;
                let csom = disasm.read_csom()?;
                let csvt = disasm.read_csvt()?;
                if csom == CsomImmediate && csvt == CsvtString {
                    let text = disasm.get_string_literal()?;
                    string_stack.insert(0, text);
                    if string_stack.len() > 8 {
                        string_stack.pop();
                    }
                }
            } else if cmd.code == CsicCall {
                disasm.stream.pos = cmd.addr as usize + 1;
                let csom = disasm.read_csom()?;
                let num_args = disasm.stream.read_i32()?;
                let func_name = WideString::unpack(&mut disasm.stream, false, Encoding::Utf16LE)?.0;
                let mut is_mess = false;
                if num_args == 1 {
                    if func_name == "SceneTitle" {
                        if string_stack.is_empty() {
                            return Err(anyhow::anyhow!(
                                "SceneTitle call without string argument at {:08X}",
                                cmd.addr
                            ));
                        }
                        messages
                            .entry(key.clone())
                            .or_insert_with(|| Vec::new())
                            .push(Message::new(string_stack[0].clone(), None));
                    } else if func_name == "Mess" {
                        is_mess = true;
                        if string_stack.is_empty() {
                            return Err(anyhow::anyhow!(
                                "Mess call without string argument at {:08X}",
                                cmd.addr
                            ));
                        }
                        if string_stack[0].starts_with("@") {
                            println!(
                                "Skipping message with special tag at {:08X}: {}",
                                cmd.addr, string_stack[0]
                            );
                            continue;
                        }
                        message.push_str(string_stack[0].as_str());
                    }
                } else if num_args == 2 {
                    if func_name == "Talk" {
                        if string_stack.is_empty() {
                            return Err(anyhow::anyhow!(
                                "Talk call without string argument at {:08X}",
                                cmd.addr
                            ));
                        }
                        name = Some(string_stack[0].clone());
                    } else if func_name == "AddSelect" {
                        if string_stack.is_empty() {
                            return Err(anyhow::anyhow!(
                                "AddSelect call without string argument at {:08X}",
                                cmd.addr
                            ));
                        }
                        messages
                            .entry(key.clone())
                            .or_insert_with(|| Vec::new())
                            .push(Message::new(string_stack[0].clone(), None));
                    }
                } else if num_args == 0 && csom == CsomAuto && func_name == "ScenarioEnter" {
                    if pre_is_enter {
                        key = pre_enter_name.clone();
                    } else {
                        key = "global".to_string();
                    }
                }
                if pre_is_mess && !is_mess {
                    messages
                        .entry(key.clone())
                        .or_insert_with(|| Vec::new())
                        .push(Message::new(message.clone(), name.take()));
                    message.clear();
                }
                pre_is_mess = is_mess;
            } else if is_enter {
                disasm.stream.pos = cmd.addr as usize + 1;
                let name = WideString::unpack(&mut disasm.stream, false, Encoding::Utf16LE)?.0;
                let num_args = disasm.stream.read_i32()?;
                if num_args == 0 {
                    pre_enter_name = name.clone();
                }
            }
            pre_is_enter = is_enter;
        }
        Ok(messages)
    }

    pub fn export_all(&self) -> Result<Vec<String>> {
        let mut disasm = ECSExecutionImageDisassembler::new(
            self.image.to_ref(),
            self.ext_const_str.as_ref(),
            None,
        );
        disasm.execute()?;
        let mut messages = Vec::new();
        let assembly = disasm.assembly.clone();
        for cmd in assembly.iter() {
            if cmd.code == CsicLoad {
                disasm.stream.pos = cmd.addr as usize + 1;
                let csom = disasm.read_csom()?;
                let csvt = disasm.read_csvt()?;
                if csom == CsomImmediate && csvt == CsvtString {
                    let text = disasm.get_string_literal()?;
                    messages.push(text);
                }
            }
        }
        Ok(messages)
    }
}
