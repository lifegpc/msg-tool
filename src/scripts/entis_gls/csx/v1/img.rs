use super::disasm::*;
use super::types::*;
use crate::ext::io::*;
use crate::ext::vec::*;
use crate::scripts::base::*;
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
    lf: String,
}

impl ECSExecutionImage {
    pub fn new(mut reader: MemReader, config: &ExtraConfig) -> Result<Self> {
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
            lf: config.entis_gls_csx_lf.clone(),
        })
    }

    fn fix_image<'a, 'b>(
        assembly: &ECSExecutionImageAssembly,
        mut reader: MemReaderRef<'a>,
        writer: &mut MemWriter,
        commands: &HashMap<u32, &'b ECSExecutionImageCommandRecord>,
    ) -> Result<()> {
        for cmd in assembly.iter() {
            // Fix Enter Try Catch address offsets
            if cmd.code == CsicEnter {
                reader.pos = cmd.addr as usize + 1;
                let name_length = reader.read_u32()?;
                if name_length != 0x80000000 {
                    reader.pos += name_length as usize * 2;
                } else {
                    reader.pos += 4;
                }
                let num_args = reader.read_i32()?;
                if num_args == -1 {
                    let _flag = reader.read_u8()?;
                    let offset = reader.pos as i64 - cmd.addr as i64;
                    let original_addr = reader.read_i32()? as i64 + reader.pos as i64;
                    let target_cmd = commands.get(&(original_addr as u32)).ok_or_else(|| anyhow::anyhow!(
                        "Cannot find target command at address {:08X} for Enter instruction fixup at {:08X}",
                        original_addr as u32,
                        cmd.addr
                    ))?;
                    let new_addr = target_cmd.new_addr as i64 - cmd.new_addr as i64 - offset - 4;
                    writer.write_i32_at(cmd.new_addr as u64 + offset as u64, new_addr as i32)?;
                }
            } else if cmd.code == CsicJump {
                // Fix Jump address offsets
                reader.pos = cmd.addr as usize + 1;
                let offset = reader.pos as i64 - cmd.addr as i64;
                let original_addr = reader.read_i32()? as i64 + reader.pos as i64;
                let target_cmd = commands.get(&(original_addr as u32)).ok_or_else(|| anyhow::anyhow!(
                    "Cannot find target command at address {:08X} for Jump instruction fixup at {:08X}",
                    original_addr as u32,
                    cmd.addr
                ))?;
                let new_addr = target_cmd.new_addr as i64 - cmd.new_addr as i64 - offset - 4;
                writer.write_i32_at(cmd.new_addr as u64 + offset as u64, new_addr as i32)?;
            } else if cmd.code == CsicCJump {
                // Fix CJump address offsets
                reader.pos = cmd.addr as usize + 2;
                let offset = reader.pos as i64 - cmd.addr as i64;
                let original_addr = reader.read_i32()? as i64 + reader.pos as i64;
                let target_cmd = commands.get(&(original_addr as u32)).ok_or_else(|| anyhow::anyhow!(
                    "Cannot find target command at address {:08X} for CJump instruction fixup at {:08X}",
                    original_addr as u32,
                    cmd.addr
                ))?;
                let new_addr = target_cmd.new_addr as i64 - cmd.new_addr as i64 - offset - 4;
                writer.write_i32_at(cmd.new_addr as u64 + offset as u64, new_addr as i32)?;
            }
        }
        Ok(())
    }

    fn fix_references(
        &mut self,
        commands: &HashMap<u32, &ECSExecutionImageCommandRecord>,
    ) -> Result<()> {
        for cmd in self.pif_prologue.iter_mut() {
            let ocmd = *cmd;
            *cmd = commands
                .get(&ocmd)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Cannot find target command at address {:08X} for PIF prologue fixup",
                        ocmd
                    )
                })?
                .new_addr;
        }
        for cmd in self.pif_epilogue.iter_mut() {
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
        for func in self.function_list.iter_mut() {
            let ocmd = func.addr;
            func.addr = commands
                .get(&ocmd)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Cannot find target command at address {:08X} for function list fixup",
                        ocmd
                    )
                })?
                .new_addr;
        }
        Ok(())
    }

    fn save<'a>(&self, mut writer: Box<dyn Write + 'a>) -> Result<()> {
        self.file_header.pack(&mut writer, false, Encoding::Utf8)?;
        if let Some(exi_header) = &self.exi_header {
            writer.write_u64(0x2020726564616568)?; // header
            writer.write_u64(exi_header.len() as u64)?;
            writer.write_all(&exi_header)?;
        }
        writer.write_u64(0x2020206567616D69)?; // image
        writer.write_u64(self.image.data.len() as u64)?;
        writer.write_all(&self.image.data)?;
        writer.write_u64(0x6E6F6974636E7566)?; // function
        let mut mem = MemWriter::new();
        self.pif_prologue.pack(&mut mem, false, Encoding::Utf8)?;
        self.pif_epilogue.pack(&mut mem, false, Encoding::Utf8)?;
        self.function_list.pack(&mut mem, false, Encoding::Utf8)?;
        let data = mem.into_inner();
        writer.write_u64(data.len() as u64)?;
        writer.write_all(&data)?;
        writer.write_u64(0x20206C61626F6C67)?; // global
        let mut mem = MemWriter::new();
        let int64 = if let Some(hdr) = &self.header {
            hdr.int_base == 64
        } else {
            false
        };
        mem.write_u32(self.csg_global.len() as u32)?;
        for item in self.csg_global.iter() {
            WideString(item.name.clone()).pack(&mut mem, false, Encoding::Utf16LE)?;
            item.obj.write_to(&mut mem, int64)?;
        }
        let data = mem.into_inner();
        writer.write_u64(data.len() as u64)?;
        writer.write_all(&data)?;
        writer.write_u64(0x2020202061746164)?; // data
        let mut mem = MemWriter::new();
        mem.write_u32(self.csg_data.len() as u32)?;
        for item in self.csg_data.iter() {
            WideString(item.name.clone()).pack(&mut mem, false, Encoding::Utf16LE)?;
            match &item.obj {
                ECSObject::Global(g) => {
                    mem.write_i32(g.len() as i32)?;
                    for data_item in g.iter() {
                        WideString(data_item.name.clone()).pack(
                            &mut mem,
                            false,
                            Encoding::Utf16LE,
                        )?;
                        data_item.obj.write_to(&mut mem, int64)?;
                    }
                }
                _ => {
                    mem.write_u32(0x80000000)?;
                    item.obj.write_to(&mut mem, int64)?;
                }
            }
        }
        let data = mem.into_inner();
        writer.write_u64(data.len() as u64)?;
        writer.write_all(&data)?;
        if let Some(ext_const_str) = &self.ext_const_str {
            writer.write_u64(0x72747374736E6F63)?; // conststr
            let mut mem = MemWriter::new();
            ext_const_str.pack(&mut mem, false, Encoding::Utf8)?;
            let data = mem.into_inner();
            writer.write_u64(data.len() as u64)?;
            writer.write_all(&data)?;
        }
        writer.write_u64(0x20666E696B6E696C)?; // linkinf
        let mut mem = MemWriter::new();
        self.ext_global_ref.pack(&mut mem, false, Encoding::Utf8)?;
        self.ext_data_ref.pack(&mut mem, false, Encoding::Utf8)?;
        self.imp_global_ref.pack(&mut mem, false, Encoding::Utf8)?;
        self.imp_data_ref.pack(&mut mem, false, Encoding::Utf8)?;
        let data = mem.into_inner();
        writer.write_u64(data.len() as u64)?;
        writer.write_all(&data)?;
        Ok(())
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
                    string_stack.push(text);
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
                        if string_stack[0].starts_with("@") {
                            string_stack.clear();
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
                        if string_stack[0] == "心の声" {
                            string_stack.clear();
                            // 傻逼旁白
                            continue;
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
                    messages.push(Message::new(message.replace(&self.lf, "\n"), name.take()));
                    message.clear();
                }
                string_stack.clear();
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
                    string_stack.push(text);
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
                            eprintln!(
                                "Skipping control string at 0x{:08x}: {}",
                                cmd.addr, string_stack[0]
                            );
                            string_stack.clear();
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
                        if string_stack[0] == "心の声" {
                            // 傻逼旁白
                            string_stack.clear();
                            continue;
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
                        .push(Message::new(message.replace(&self.lf, "\n"), name.take()));
                    message.clear();
                }
                pre_is_mess = is_mess;
                string_stack.clear();
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

    pub fn import<'a>(
        &self,
        messages: Vec<Message>,
        file: Box<dyn WriteSeek + 'a>,
        replacement: Option<&'a ReplacementTable>,
    ) -> Result<()> {
        let mut cloned = self.clone();
        let mut disasm = ECSExecutionImageDisassembler::new(
            self.image.to_ref(),
            self.ext_const_str.as_ref(),
            None,
        );
        disasm.execute()?;
        let mut assembly = disasm.assembly.clone();
        let mut index = 0;
        let mut dumped_index = 0;
        let mut new_image = MemWriter::new();
        let mut pre_is_mess = false;
        let mut first_mess_index = None;
        let mut last_mess_index = None;
        let mut message_iter = messages.into_iter();
        let mut mess = message_iter.next();
        while index < assembly.len() {
            let cmd = assembly[index].clone();
            if cmd.code == CsicCall {
                disasm.stream.pos = cmd.addr as usize + 1;
                let csom = disasm.read_csom()?;
                let num_args = disasm.stream.read_i32()?;
                let func_name = WideString::unpack(&mut disasm.stream, false, Encoding::Utf16LE)?.0;
                let mut is_mess = false;
                if csom == CsomAuto && num_args == 1 && func_name == "Mess" {
                    is_mess = true;
                    if first_mess_index.is_none() {
                        first_mess_index = Some(index);
                    }
                    last_mess_index = Some(index);
                }
                if pre_is_mess && !is_mess {
                    let first_index = first_mess_index
                        .ok_or(anyhow::anyhow!("Internal error: first_mess_index is None"))?;
                    let last_index = last_mess_index
                        .ok_or(anyhow::anyhow!("Internal error: last_mess_index is None"))?;
                    // Load string
                    let pre_index = first_index - 1;
                    while dumped_index < pre_index {
                        let tcmd = &mut assembly[dumped_index];
                        tcmd.new_addr = new_image.pos as u32;
                        // Copy original instruction
                        new_image.write_from(
                            &mut disasm.stream,
                            tcmd.addr as u64,
                            tcmd.size as u64,
                        )?;
                        dumped_index += 1;
                    }
                    // Free
                    let post_index = last_index + 1;
                    let mut message = mess
                        .take()
                        .ok_or_else(|| {
                            anyhow::anyhow!("Not enough messages to import for Mess command.")
                        })?
                        .message;
                    mess = message_iter.next();
                    if let Some(repl) = replacement {
                        for (k, v) in repl.map.iter() {
                            message = message.replace(k, v);
                        }
                    }
                    for i in (first_index..=last_index).step_by(3) {
                        let tcmd = assembly[i - 1].clone();
                        disasm.stream.pos = tcmd.addr as usize + 1;
                        let lcsom = disasm.read_csom()?;
                        let lcsvt = disasm.read_csvt()?;
                        if lcsom != CsomImmediate || lcsvt != CsvtString {
                            return Err(anyhow::anyhow!(
                                "Invalid load command before Mess at {:08X}.",
                                tcmd.addr
                            ));
                        }
                    }
                    let mes_list: Vec<_> = message
                        .replace("\n", &self.lf)
                        .split(&self.lf)
                        .map(|s| s.to_string())
                        .collect();
                    let mut new_assembly = Vec::new();
                    let mes_count = mes_list.len();
                    let mut tmp_index = pre_index;
                    for i in 0..mes_count {
                        let mut mes = mes_list[i].clone();
                        if i < mes_count - 1 {
                            mes.push_str(&self.lf);
                        }
                        let mut tcmd = if tmp_index <= post_index {
                            let data = assembly[tmp_index].clone();
                            tmp_index += 1;
                            if data.code != CsicLoad {
                                return Err(anyhow::anyhow!(
                                    "Internal error: expected Load command at {:08X}.",
                                    data.addr
                                ));
                            }
                            data
                        } else {
                            ECSExecutionImageCommandRecord {
                                code: CsicLoad,
                                addr: u32::MAX,
                                size: 0,
                                new_addr: 0,
                            }
                        };
                        tcmd.new_addr = new_image.pos as u32;
                        new_image.write_u8(CsicLoad.into())?;
                        new_image.write_u8(CsomImmediate.into())?;
                        new_image.write_u8(CsvtString.into())?;
                        WideString(mes).pack(&mut new_image, false, Encoding::Utf8)?;
                        new_assembly.push(tcmd);
                        let mut tcmd = if tmp_index <= post_index {
                            let data = assembly[tmp_index].clone();
                            tmp_index += 1;
                            if data.code != CsicCall {
                                return Err(anyhow::anyhow!(
                                    "Expected Call command at {:08X}.",
                                    data.addr
                                ));
                            }
                            data
                        } else {
                            ECSExecutionImageCommandRecord {
                                code: CsicCall,
                                addr: u32::MAX,
                                size: 0,
                                new_addr: 0,
                            }
                        };
                        tcmd.new_addr = new_image.pos as u32;
                        new_image.write_u8(CsicCall.into())?;
                        new_image.write_u8(CsomAuto.into())?;
                        new_image.write_i32(1)?; // num_args
                        WideString("Mess".to_string()).pack(
                            &mut new_image,
                            false,
                            Encoding::Utf16LE,
                        )?;
                        new_assembly.push(tcmd);
                        let mut tcmd = if tmp_index <= post_index {
                            let data = assembly[tmp_index].clone();
                            tmp_index += 1;
                            if data.code != CsicFree {
                                return Err(anyhow::anyhow!(
                                    "Expected Free command at {:08X}.",
                                    data.addr
                                ));
                            }
                            data
                        } else {
                            ECSExecutionImageCommandRecord {
                                code: CsicFree,
                                addr: u32::MAX,
                                size: 0,
                                new_addr: 0,
                            }
                        };
                        tcmd.new_addr = new_image.pos as u32;
                        new_image.write_u8(CsicFree.into())?;
                        new_assembly.push(tcmd);
                    }
                    let ori_count = post_index - pre_index + 1;
                    let new_count = new_assembly.len();
                    dumped_index += new_count;
                    index = (index as isize + (new_count as isize - ori_count as isize)) as usize;
                    last_mess_index = None;
                    first_mess_index = None;
                    assembly.splice(pre_index..post_index + 1, new_assembly);
                }
                if csom == CsomAuto && num_args == 2 && func_name == "Talk" {
                    if index < 2 {
                        return Err(anyhow::anyhow!(
                            "No enough load command at {:08x}.",
                            cmd.addr
                        ));
                    }
                    let pre_index = index - 2;
                    while dumped_index < pre_index {
                        let tcmd = &mut assembly[dumped_index];
                        tcmd.new_addr = new_image.pos as u32;
                        // Copy original instruction
                        new_image.write_from(
                            &mut disasm.stream,
                            tcmd.addr as u64,
                            tcmd.size as u64,
                        )?;
                        dumped_index += 1;
                    }
                    let tcmd = &mut assembly[pre_index];
                    tcmd.new_addr = new_image.pos as u32;
                    disasm.stream.pos = tcmd.addr as usize + 1;
                    let lcsom = disasm.read_csom()?;
                    let lcsvt = disasm.read_csvt()?;
                    if lcsom != CsomImmediate || lcsvt != CsvtString {
                        return Err(anyhow::anyhow!(
                            "Invalid load command before Talk at {:08X}.",
                            tcmd.addr
                        ));
                    }
                    let original_name = disasm.get_string_literal()?;
                    let name = if original_name == "心の声" {
                        original_name
                    } else {
                        let mut name = mess.as_mut().map(|s| s.name.take()).flatten().ok_or(
                            anyhow::anyhow!("No available name for Talk at {:08X}.", cmd.addr),
                        )?;
                        if let Some(repl) = replacement {
                            for (k, v) in repl.map.iter() {
                                name = name.replace(k, v);
                            }
                        }
                        name
                    };
                    new_image.write_u8(CsicLoad.into())?;
                    new_image.write_u8(lcsom.into())?;
                    new_image.write_u8(lcsvt.into())?;
                    WideString(name).pack(&mut new_image, false, Encoding::Utf8)?;
                    dumped_index += 1;
                    while dumped_index <= index {
                        let tcmd = &mut assembly[dumped_index];
                        tcmd.new_addr = new_image.pos as u32;
                        // Copy original instruction
                        new_image.write_from(
                            &mut disasm.stream,
                            tcmd.addr as u64,
                            tcmd.size as u64,
                        )?;
                        dumped_index += 1;
                    }
                } else if csom == CsomAuto && num_args == 2 && func_name == "AddSelect" {
                    if index < 2 {
                        return Err(anyhow::anyhow!(
                            "No enough load command at {:08x}.",
                            cmd.addr
                        ));
                    }
                    let pre_index = index - 2;
                    while dumped_index < pre_index {
                        let tcmd = &mut assembly[dumped_index];
                        tcmd.new_addr = new_image.pos as u32;
                        // Copy original instruction
                        new_image.write_from(
                            &mut disasm.stream,
                            tcmd.addr as u64,
                            tcmd.size as u64,
                        )?;
                        dumped_index += 1;
                    }
                    let tcmd = &mut assembly[pre_index];
                    tcmd.new_addr = new_image.pos as u32;
                    disasm.stream.pos = tcmd.addr as usize + 1;
                    let lcsom = disasm.read_csom()?;
                    let lcsvt = disasm.read_csvt()?;
                    if lcsom != CsomImmediate || lcsvt != CsvtString {
                        return Err(anyhow::anyhow!(
                            "Invalid load command before AddSelect at {:08X}.",
                            tcmd.addr
                        ));
                    }
                    let mut message = mess
                        .take()
                        .ok_or_else(|| {
                            anyhow::anyhow!(
                                "No available message for AddSelect at {:08X}.",
                                cmd.addr
                            )
                        })?
                        .message;
                    mess = message_iter.next();
                    if let Some(repl) = replacement {
                        for (k, v) in repl.map.iter() {
                            message = message.replace(k, v);
                        }
                    }
                    new_image.write_u8(CsicLoad.into())?;
                    new_image.write_u8(lcsom.into())?;
                    new_image.write_u8(lcsvt.into())?;
                    WideString(message).pack(&mut new_image, false, Encoding::Utf8)?;
                    dumped_index += 1;
                    while dumped_index <= index {
                        let tcmd = &mut assembly[dumped_index];
                        tcmd.new_addr = new_image.pos as u32;
                        // Copy original instruction
                        new_image.write_from(
                            &mut disasm.stream,
                            tcmd.addr as u64,
                            tcmd.size as u64,
                        )?;
                        dumped_index += 1;
                    }
                } else if csom == CsomAuto && num_args == 1 && func_name == "SceneTitle" {
                    if index < 1 {
                        return Err(anyhow::anyhow!(
                            "No enough load command at {:08x}.",
                            cmd.addr
                        ));
                    }
                    let pre_index = index - 1;
                    while dumped_index < pre_index {
                        let tcmd = &mut assembly[dumped_index];
                        tcmd.new_addr = new_image.pos as u32;
                        // Copy original instruction
                        new_image.write_from(
                            &mut disasm.stream,
                            tcmd.addr as u64,
                            tcmd.size as u64,
                        )?;
                        dumped_index += 1;
                    }
                    let tcmd = &mut assembly[pre_index];
                    tcmd.new_addr = new_image.pos as u32;
                    disasm.stream.pos = tcmd.addr as usize + 1;
                    let lcsom = disasm.read_csom()?;
                    let lcsvt = disasm.read_csvt()?;
                    if lcsom != CsomImmediate || lcsvt != CsvtString {
                        return Err(anyhow::anyhow!(
                            "Invalid load command before SceneTitle at {:08X}.",
                            tcmd.addr
                        ));
                    }
                    let mut message = mess
                        .take()
                        .ok_or_else(|| {
                            anyhow::anyhow!(
                                "No available message for SceneTitle at {:08X}.",
                                cmd.addr
                            )
                        })?
                        .message;
                    mess = message_iter.next();
                    if let Some(repl) = replacement {
                        for (k, v) in repl.map.iter() {
                            message = message.replace(k, v);
                        }
                    }
                    new_image.write_u8(CsicLoad.into())?;
                    new_image.write_u8(lcsom.into())?;
                    new_image.write_u8(lcsvt.into())?;
                    WideString(message).pack(&mut new_image, false, Encoding::Utf8)?;
                    dumped_index += 1;
                    while dumped_index <= index {
                        let tcmd = &mut assembly[dumped_index];
                        tcmd.new_addr = new_image.pos as u32;
                        // Copy original instruction
                        new_image.write_from(
                            &mut disasm.stream,
                            tcmd.addr as u64,
                            tcmd.size as u64,
                        )?;
                        dumped_index += 1;
                    }
                }
                pre_is_mess = is_mess;
            }
            index += 1;
        }
        while dumped_index < assembly.len() {
            let tcmd = &mut assembly[dumped_index];
            tcmd.new_addr = new_image.pos as u32;
            // Copy original instruction
            new_image.write_from(&mut disasm.stream, tcmd.addr as u64, tcmd.size as u64)?;
            dumped_index += 1;
        }
        if mess.is_some() || message_iter.next().is_some() {
            return Err(anyhow::anyhow!("Too many messages to import."));
        }
        let commands: HashMap<u32, &ECSExecutionImageCommandRecord> =
            assembly.iter().map(|c| (c.addr, c)).collect();
        Self::fix_image(&assembly, disasm.stream.clone(), &mut new_image, &commands)?;
        cloned.image = MemReader::new(new_image.into_inner());
        cloned.fix_references(&commands)?;
        cloned.save(file)?;
        Ok(())
    }

    pub fn import_multi<'a>(
        &self,
        mut messages: HashMap<String, Vec<Message>>,
        file: Box<dyn WriteSeek + 'a>,
        replacement: Option<&'a ReplacementTable>,
    ) -> Result<()> {
        let mut cloned = self.clone();
        let mut key = String::from("global");
        let mut disasm = ECSExecutionImageDisassembler::new(
            self.image.to_ref(),
            self.ext_const_str.as_ref(),
            None,
        );
        disasm.execute()?;
        let mut assembly = disasm.assembly.clone();
        let mut index = 0;
        let mut dumped_index = 0;
        let mut new_image = MemWriter::new();
        let mut pre_is_enter = false;
        let mut pre_enter_name = String::new();
        let mut pre_is_mess = false;
        let mut first_mess_index = None;
        let mut last_mess_index = None;
        while index < assembly.len() {
            let cmd = assembly[index].clone();
            let is_enter = cmd.code == CsicEnter;
            if cmd.code == CsicCall {
                disasm.stream.pos = cmd.addr as usize + 1;
                let csom = disasm.read_csom()?;
                let num_args = disasm.stream.read_i32()?;
                let func_name = WideString::unpack(&mut disasm.stream, false, Encoding::Utf16LE)?.0;
                let mut is_mess = false;
                if csom == CsomAuto && num_args == 1 && func_name == "Mess" {
                    is_mess = true;
                    if first_mess_index.is_none() {
                        first_mess_index = Some(index);
                    }
                    last_mess_index = Some(index);
                }
                if pre_is_mess && !is_mess {
                    let first_index = first_mess_index
                        .ok_or(anyhow::anyhow!("Internal error: first_mess_index is None"))?;
                    let last_index = last_mess_index
                        .ok_or(anyhow::anyhow!("Internal error: last_mess_index is None"))?;
                    // Load string
                    let pre_index = first_index - 1;
                    while dumped_index < pre_index {
                        let tcmd = &mut assembly[dumped_index];
                        tcmd.new_addr = new_image.pos as u32;
                        // Copy original instruction
                        new_image.write_from(
                            &mut disasm.stream,
                            tcmd.addr as u64,
                            tcmd.size as u64,
                        )?;
                        dumped_index += 1;
                    }
                    // Free
                    let post_index = last_index + 1;
                    let mut message = messages
                        .get_mut(&key)
                        .and_then(|messages| messages.pop_first())
                        .ok_or(anyhow::anyhow!(
                            "No available message for Mess at {:08X}.",
                            cmd.addr
                        ))?
                        .message;
                    if let Some(repl) = replacement {
                        for (k, v) in repl.map.iter() {
                            message = message.replace(k, v);
                        }
                    }
                    for i in (first_index..=last_index).step_by(3) {
                        let tcmd = assembly[i - 1].clone();
                        disasm.stream.pos = tcmd.addr as usize + 1;
                        let lcsom = disasm.read_csom()?;
                        let lcsvt = disasm.read_csvt()?;
                        if lcsom != CsomImmediate || lcsvt != CsvtString {
                            return Err(anyhow::anyhow!(
                                "Invalid load command before Mess at {:08X}.",
                                tcmd.addr
                            ));
                        }
                    }
                    let mes_list: Vec<_> = message
                        .replace("\n", &self.lf)
                        .split(&self.lf)
                        .map(|s| s.to_string())
                        .collect();
                    let mut new_assembly = Vec::new();
                    let mes_count = mes_list.len();
                    let mut tmp_index = pre_index;
                    for i in 0..mes_count {
                        let mut mes = mes_list[i].clone();
                        if i < mes_count - 1 {
                            mes.push_str(&self.lf);
                        }
                        let mut tcmd = if tmp_index <= post_index {
                            let data = assembly[tmp_index].clone();
                            tmp_index += 1;
                            if data.code != CsicLoad {
                                return Err(anyhow::anyhow!(
                                    "Internal error: expected Load command at {:08X}.",
                                    data.addr
                                ));
                            }
                            data
                        } else {
                            ECSExecutionImageCommandRecord {
                                code: CsicLoad,
                                addr: u32::MAX,
                                size: 0,
                                new_addr: 0,
                            }
                        };
                        tcmd.new_addr = new_image.pos as u32;
                        new_image.write_u8(CsicLoad.into())?;
                        new_image.write_u8(CsomImmediate.into())?;
                        new_image.write_u8(CsvtString.into())?;
                        WideString(mes).pack(&mut new_image, false, Encoding::Utf8)?;
                        new_assembly.push(tcmd);
                        let mut tcmd = if tmp_index <= post_index {
                            let data = assembly[tmp_index].clone();
                            tmp_index += 1;
                            if data.code != CsicCall {
                                return Err(anyhow::anyhow!(
                                    "Expected Call command at {:08X}.",
                                    data.addr
                                ));
                            }
                            data
                        } else {
                            ECSExecutionImageCommandRecord {
                                code: CsicCall,
                                addr: u32::MAX,
                                size: 0,
                                new_addr: 0,
                            }
                        };
                        tcmd.new_addr = new_image.pos as u32;
                        new_image.write_u8(CsicCall.into())?;
                        new_image.write_u8(CsomAuto.into())?;
                        new_image.write_i32(1)?; // num_args
                        WideString("Mess".to_string()).pack(
                            &mut new_image,
                            false,
                            Encoding::Utf16LE,
                        )?;
                        new_assembly.push(tcmd);
                        let mut tcmd = if tmp_index <= post_index {
                            let data = assembly[tmp_index].clone();
                            tmp_index += 1;
                            if data.code != CsicFree {
                                return Err(anyhow::anyhow!(
                                    "Expected Free command at {:08X}.",
                                    data.addr
                                ));
                            }
                            data
                        } else {
                            ECSExecutionImageCommandRecord {
                                code: CsicFree,
                                addr: u32::MAX,
                                size: 0,
                                new_addr: 0,
                            }
                        };
                        tcmd.new_addr = new_image.pos as u32;
                        new_image.write_u8(CsicFree.into())?;
                        new_assembly.push(tcmd);
                    }
                    let ori_count = post_index - pre_index + 1;
                    let new_count = new_assembly.len();
                    dumped_index += new_count;
                    index = (index as isize + (new_count as isize - ori_count as isize)) as usize;
                    last_mess_index = None;
                    first_mess_index = None;
                    assembly.splice(pre_index..post_index + 1, new_assembly);
                }
                if csom == CsomAuto && num_args == 2 && func_name == "Talk" {
                    if index < 2 {
                        return Err(anyhow::anyhow!(
                            "No enough load command at {:08x}.",
                            cmd.addr
                        ));
                    }
                    let pre_index = index - 2;
                    while dumped_index < pre_index {
                        let tcmd = &mut assembly[dumped_index];
                        tcmd.new_addr = new_image.pos as u32;
                        // Copy original instruction
                        new_image.write_from(
                            &mut disasm.stream,
                            tcmd.addr as u64,
                            tcmd.size as u64,
                        )?;
                        dumped_index += 1;
                    }
                    let tcmd = &mut assembly[pre_index];
                    tcmd.new_addr = new_image.pos as u32;
                    disasm.stream.pos = tcmd.addr as usize + 1;
                    let lcsom = disasm.read_csom()?;
                    let lcsvt = disasm.read_csvt()?;
                    if lcsom != CsomImmediate || lcsvt != CsvtString {
                        return Err(anyhow::anyhow!(
                            "Invalid load command before Talk at {:08X}.",
                            tcmd.addr
                        ));
                    }
                    let original_name = disasm.get_string_literal()?;
                    let name = if original_name == "心の声" {
                        original_name
                    } else {
                        let mut name = messages
                            .get_mut(&key)
                            .and_then(|messages| messages.first_mut().map(|m| m.name.take()))
                            .flatten()
                            .ok_or(anyhow::anyhow!(
                                "No available name message for Talk at {:08X}.",
                                cmd.addr
                            ))?;
                        if let Some(repl) = replacement {
                            for (k, v) in repl.map.iter() {
                                name = name.replace(k, v);
                            }
                        }
                        name
                    };
                    new_image.write_u8(CsicLoad.into())?;
                    new_image.write_u8(lcsom.into())?;
                    new_image.write_u8(lcsvt.into())?;
                    WideString(name).pack(&mut new_image, false, Encoding::Utf8)?;
                    dumped_index += 1;
                    while dumped_index <= index {
                        let tcmd = &mut assembly[dumped_index];
                        tcmd.new_addr = new_image.pos as u32;
                        // Copy original instruction
                        new_image.write_from(
                            &mut disasm.stream,
                            tcmd.addr as u64,
                            tcmd.size as u64,
                        )?;
                        dumped_index += 1;
                    }
                } else if csom == CsomAuto && num_args == 2 && func_name == "AddSelect" {
                    if index < 2 {
                        return Err(anyhow::anyhow!(
                            "No enough load command at {:08x}.",
                            cmd.addr
                        ));
                    }
                    let pre_index = index - 2;
                    while dumped_index < pre_index {
                        let tcmd = &mut assembly[dumped_index];
                        tcmd.new_addr = new_image.pos as u32;
                        // Copy original instruction
                        new_image.write_from(
                            &mut disasm.stream,
                            tcmd.addr as u64,
                            tcmd.size as u64,
                        )?;
                        dumped_index += 1;
                    }
                    let tcmd = &mut assembly[pre_index];
                    tcmd.new_addr = new_image.pos as u32;
                    disasm.stream.pos = tcmd.addr as usize + 1;
                    let lcsom = disasm.read_csom()?;
                    let lcsvt = disasm.read_csvt()?;
                    if lcsom != CsomImmediate || lcsvt != CsvtString {
                        return Err(anyhow::anyhow!(
                            "Invalid load command before AddSelect at {:08X}.",
                            tcmd.addr
                        ));
                    }
                    let mut message = messages
                        .get_mut(&key)
                        .and_then(|messages| messages.pop_first())
                        .ok_or(anyhow::anyhow!(
                            "No available message for AddSelect at {:08X}.",
                            cmd.addr
                        ))?
                        .message;
                    if let Some(repl) = replacement {
                        for (k, v) in repl.map.iter() {
                            message = message.replace(k, v);
                        }
                    }
                    new_image.write_u8(CsicLoad.into())?;
                    new_image.write_u8(lcsom.into())?;
                    new_image.write_u8(lcsvt.into())?;
                    WideString(message).pack(&mut new_image, false, Encoding::Utf8)?;
                    dumped_index += 1;
                    while dumped_index <= index {
                        let tcmd = &mut assembly[dumped_index];
                        tcmd.new_addr = new_image.pos as u32;
                        // Copy original instruction
                        new_image.write_from(
                            &mut disasm.stream,
                            tcmd.addr as u64,
                            tcmd.size as u64,
                        )?;
                        dumped_index += 1;
                    }
                } else if csom == CsomAuto && num_args == 0 && func_name == "ScenarioEnter" {
                    if pre_is_enter {
                        key = pre_enter_name.clone();
                    } else {
                        key = "global".to_string();
                    }
                } else if csom == CsomAuto && num_args == 1 && func_name == "SceneTitle" {
                    if index < 1 {
                        return Err(anyhow::anyhow!(
                            "No enough load command at {:08x}.",
                            cmd.addr
                        ));
                    }
                    let pre_index = index - 1;
                    while dumped_index < pre_index {
                        let tcmd = &mut assembly[dumped_index];
                        tcmd.new_addr = new_image.pos as u32;
                        // Copy original instruction
                        new_image.write_from(
                            &mut disasm.stream,
                            tcmd.addr as u64,
                            tcmd.size as u64,
                        )?;
                        dumped_index += 1;
                    }
                    let tcmd = &mut assembly[pre_index];
                    tcmd.new_addr = new_image.pos as u32;
                    disasm.stream.pos = tcmd.addr as usize + 1;
                    let lcsom = disasm.read_csom()?;
                    let lcsvt = disasm.read_csvt()?;
                    if lcsom != CsomImmediate || lcsvt != CsvtString {
                        return Err(anyhow::anyhow!(
                            "Invalid load command before SceneTitle at {:08X}.",
                            tcmd.addr
                        ));
                    }
                    let mut message = messages
                        .get_mut(&key)
                        .and_then(|messages| messages.pop_first())
                        .ok_or(anyhow::anyhow!(
                            "No available message for SceneTitle at {:08X}.",
                            cmd.addr
                        ))?
                        .message;
                    if let Some(repl) = replacement {
                        for (k, v) in repl.map.iter() {
                            message = message.replace(k, v);
                        }
                    }
                    new_image.write_u8(CsicLoad.into())?;
                    new_image.write_u8(lcsom.into())?;
                    new_image.write_u8(lcsvt.into())?;
                    WideString(message).pack(&mut new_image, false, Encoding::Utf8)?;
                    dumped_index += 1;
                    while dumped_index <= index {
                        let tcmd = &mut assembly[dumped_index];
                        tcmd.new_addr = new_image.pos as u32;
                        // Copy original instruction
                        new_image.write_from(
                            &mut disasm.stream,
                            tcmd.addr as u64,
                            tcmd.size as u64,
                        )?;
                        dumped_index += 1;
                    }
                }
                pre_is_mess = is_mess;
            } else if is_enter {
                disasm.stream.pos = cmd.addr as usize + 1;
                let original_name =
                    WideString::unpack(&mut disasm.stream, false, Encoding::Utf16LE)?.0;
                let num_args = disasm.stream.read_i32()?;
                if num_args == 0 {
                    pre_enter_name = original_name.clone();
                }
            }
            pre_is_enter = is_enter;
            index += 1;
        }
        while dumped_index < assembly.len() {
            let tcmd = &mut assembly[dumped_index];
            tcmd.new_addr = new_image.pos as u32;
            // Copy original instruction
            new_image.write_from(&mut disasm.stream, tcmd.addr as u64, tcmd.size as u64)?;
            dumped_index += 1;
        }
        for (s, mes) in messages {
            if !mes.is_empty() {
                return Err(anyhow::anyhow!(
                    "Not all messages were used for key '{}', {} remaining.",
                    s,
                    mes.len()
                ));
            }
        }
        let commands: HashMap<u32, &ECSExecutionImageCommandRecord> =
            assembly.iter().map(|c| (c.addr, c)).collect();
        Self::fix_image(&assembly, disasm.stream.clone(), &mut new_image, &commands)?;
        cloned.image = MemReader::new(new_image.into_inner());
        cloned.fix_references(&commands)?;
        cloned.save(file)?;
        Ok(())
    }

    pub fn import_all<'a>(
        &self,
        messages: Vec<String>,
        file: Box<dyn WriteSeek + 'a>,
    ) -> Result<()> {
        let mut cloned = self.clone();
        let mut mess = messages.into_iter();
        let mut mes = mess.next();
        let mut disasm = ECSExecutionImageDisassembler::new(
            self.image.to_ref(),
            self.ext_const_str.as_ref(),
            None,
        );
        disasm.execute()?;
        let mut assembly = disasm.assembly.clone();
        let mut new_image = MemWriter::new();
        for cmd in assembly.iter_mut() {
            cmd.new_addr = new_image.pos as u32;
            if cmd.code == CsicLoad {
                disasm.stream.pos = cmd.addr as usize + 1;
                let csom = disasm.read_csom()?;
                let csvt = disasm.read_csvt()?;
                if csom == CsomImmediate && csvt == CsvtString {
                    let code: u8 = CsicLoad.into();
                    let csom: u8 = csom.into();
                    let csvt: u8 = csvt.into();
                    let s = match mes.take() {
                        Some(v) => WideString(v),
                        None => {
                            return Err(anyhow::anyhow!(
                                "Not enough messages to import, ran out at instruction address {:08X}",
                                cmd.addr
                            ));
                        }
                    };
                    mes = mess.next();
                    new_image.write_u8(code)?;
                    new_image.write_u8(csom)?;
                    new_image.write_u8(csvt)?;
                    s.pack(&mut new_image, false, Encoding::Utf8)?;
                    continue;
                }
            }
            // Copy original instruction
            new_image.write_from(&mut disasm.stream, cmd.addr as u64, cmd.size as u64)?;
        }
        let commands: HashMap<u32, &ECSExecutionImageCommandRecord> =
            assembly.iter().map(|c| (c.addr, c)).collect();
        Self::fix_image(&assembly, disasm.stream.clone(), &mut new_image, &commands)?;
        cloned.image = MemReader::new(new_image.into_inner());
        cloned.fix_references(&commands)?;
        cloned.save(file)?;
        Ok(())
    }
}
