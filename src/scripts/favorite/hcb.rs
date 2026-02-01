//! Favorite HCB script (.hcb)
use std::io::Write;

use super::disasm::*;
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use crate::utils::str::*;
use anyhow::Result;

#[derive(Debug)]
/// Favorite HCB script builder
pub struct HcbScriptBuilder {}

impl HcbScriptBuilder {
    /// Create a new HcbScriptBuilder
    pub fn new() -> Self {
        Self {}
    }
}

impl ScriptBuilder for HcbScriptBuilder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Cp932
    }

    fn build_script(
        &self,
        buf: Vec<u8>,
        _filename: &str,
        encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(HcbScript::new(buf, encoding, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["hcb"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::Favorite
    }
}

#[derive(Debug)]
pub struct HcbScript {
    data: Data,
    reader: MemReader,
    custom_yaml: bool,
    filter_ascii: bool,
    encoding: Encoding,
}

impl HcbScript {
    pub fn new(buf: Vec<u8>, encoding: Encoding, config: &ExtraConfig) -> Result<Self> {
        let reader = MemReader::new(buf);
        let data = Data::disasm(reader.to_ref(), encoding)?;
        Ok(Self {
            data,
            reader,
            custom_yaml: config.custom_yaml,
            filter_ascii: config.favorite_hcb_filter_ascii,
            encoding,
        })
    }
}

impl Script for HcbScript {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Json
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn is_output_supported(&self, _: OutputScriptType) -> bool {
        true
    }

    fn custom_output_extension<'a>(&'a self) -> &'a str {
        if self.custom_yaml { "yaml" } else { "json" }
    }

    fn extract_messages(&self) -> Result<Vec<Message>> {
        let messages = self
            .data
            .extract_messages(self.filter_ascii)
            .into_iter()
            .map(|(speaker, message)| Message::new(message, speaker))
            .collect();
        Ok(messages)
    }

    fn import_messages<'a>(
        &'a self,
        messages: Vec<Message>,
        file: Box<dyn WriteSeek + 'a>,
        _filename: &str,
        encoding: Encoding,
        replacement: Option<&'a ReplacementTable>,
    ) -> Result<()> {
        let mut mess = messages.iter();
        let mut mes = mess.next();
        let mut patcher =
            BinaryPatcher::new(self.reader.to_ref(), file, |pos| Ok(pos), |pos| Ok(pos))?;
        let mut need_pacth_addresses = Vec::new();
        let mut new_need_patch_addresses = Vec::new();
        let thread_start_callid = self
            .data
            .sys_imports
            .iter()
            .position(|s| s == "ThreadStart")
            .map(|i| i as u16)
            .unwrap_or(u16::MAX);
        let mut func_index = 0;
        let func_len = self.data.functions.len();
        while func_index < func_len {
            let func = &self.data.functions[func_index];
            let mut cur_pos = func.pos + 1;
            if matches!(func.opcode, 0x02 | 0x06 | 0x07) {
                need_pacth_addresses.push(cur_pos);
            }
            if func.opcode == 0x03 {
                let syscall_id = if let Some(Operand::W(id)) = func.operands.get(0) {
                    *id
                } else {
                    anyhow::bail!("Invalid syscall operand at function index {}", func_index);
                };
                if syscall_id == thread_start_callid {
                    if func_index == 0 {
                        anyhow::bail!("ThreadStart syscall cannot be at function index 0");
                    }
                    let pre_func = &self.data.functions[func_index - 1];
                    if pre_func.opcode == 0x0a {
                        need_pacth_addresses.push(pre_func.pos + 1);
                    }
                }
            }
            for operand in &func.operands {
                if let Operand::S(s) = operand {
                    if self.filter_ascii && s.chars().all(|c| c.is_ascii()) {
                        continue;
                    }
                    let m = match mes {
                        Some(m) => m,
                        None => {
                            return Err(anyhow::anyhow!(
                                "Not enough messages to import. Missing message: {}",
                                s
                            ));
                        }
                    };
                    let mut message = m.message.clone();
                    if let Some(table) = replacement {
                        for (k, v) in &table.map {
                            message = message.replace(k, v);
                        }
                    }
                    patcher.copy_up_to(cur_pos)?;
                    let ori_len = operand.len(self.encoding)? as u64;
                    let mut s = encode_string(encoding, &message, true)?;
                    s.push(0); // null-terminated
                    let len = s.len();
                    if len > 255 {
                        return Err(anyhow::anyhow!(
                            "Message too long to import in functions section (max 255 bytes): {}",
                            message
                        ));
                    }
                    patcher.replace_bytes_with_write(ori_len, |writer| {
                        writer.write_u8(len as u8)?;
                        writer.write_all(&s)?;
                        Ok(())
                    })?;
                    mes = mess.next();
                }
                cur_pos += operand.len(self.encoding)? as u64;
            }
            func_index += 1;
        }
        func_index = 0;
        let func_len = self.data.main_script.len();
        'outer: while func_index < func_len {
            let func = &self.data.main_script[func_index];
            let mut cur_pos = func.pos + 1;
            if matches!(func.opcode, 0x02 | 0x06 | 0x07) {
                need_pacth_addresses.push(cur_pos);
            }
            if func.opcode == 0x03 {
                let syscall_id = if let Some(Operand::W(id)) = func.operands.get(0) {
                    *id
                } else {
                    anyhow::bail!("Invalid syscall operand at function index {}", func_index);
                };
                if syscall_id == thread_start_callid {
                    if func_index == 0 {
                        anyhow::bail!("ThreadStart syscall cannot be at function index 0");
                    }
                    let pre_func = &self.data.main_script[func_index - 1];
                    if pre_func.opcode == 0x0a {
                        need_pacth_addresses.push(pre_func.pos + 1);
                    }
                }
            }
            for operand in &func.operands {
                if let Operand::S(s) = operand {
                    if self.filter_ascii && s.chars().all(|c| c.is_ascii()) {
                        continue;
                    }
                    let m = match mes {
                        Some(m) => m,
                        None => {
                            return Err(anyhow::anyhow!(
                                "Not enough messages to import. Missing message: {}",
                                s
                            ));
                        }
                    };
                    let mut message = m.message.clone();
                    if let Some(table) = replacement {
                        for (k, v) in &table.map {
                            message = message.replace(k, v);
                        }
                    }
                    mes = mess.next();
                    patcher.copy_up_to(cur_pos)?;
                    let ori_len = operand.len(self.encoding)? as u64;
                    let mut s = encode_string(encoding, &message, true)?;
                    s.push(0); // null-terminated
                    let len = s.len();
                    if len > 255 {
                        if func.opcode != 0x0e {
                            anyhow::bail!(
                                "Message too long to import in main script functions section (max 255 bytes): {}",
                                message
                            );
                        }
                        let cur = message.as_str();
                        let (mut s, mut remaining) =
                            truncate_string_with_enter(cur, 254, encoding)?;
                        s.push(0); // null-terminated
                        let len = s.len();
                        patcher.replace_bytes_with_write(ori_len, |writer| {
                            writer.write_u8(len as u8)?;
                            writer.write_all(&s)?;
                            Ok(())
                        })?;
                        let mut new_funcs = Vec::new();
                        func_index += 1;
                        loop {
                            let toper = &self.data.main_script[func_index];
                            new_funcs.push(toper.clone());
                            func_index += 1;
                            if matches!(toper.opcode, 0x02 | 0x06 | 0x07) {
                                need_pacth_addresses.push(toper.pos + 1);
                            }
                            if toper.opcode == 0x03 {
                                let syscall_id = if let Some(Operand::W(id)) = toper.operands.get(0)
                                {
                                    *id
                                } else {
                                    anyhow::bail!(
                                        "Invalid syscall operand at function index {}",
                                        func_index
                                    );
                                };
                                if syscall_id == thread_start_callid {
                                    if func_index == 0 {
                                        anyhow::bail!(
                                            "ThreadStart syscall cannot be at function index 0"
                                        );
                                    }
                                    let pre_func = &self.data.main_script[func_index - 1];
                                    if pre_func.opcode == 0x0a {
                                        need_pacth_addresses.push(pre_func.pos + 1);
                                    }
                                }
                            }
                            // Copy until the next call opcode
                            if toper.opcode == 0x02 {
                                break;
                            }
                        }
                        cur_pos = self.data.main_script[func_index].pos;
                        patcher.copy_up_to(cur_pos)?;
                        let mut mem = MemWriter::new();
                        while let Some(remain) = remaining {
                            let (mut s, rem) = truncate_string_with_enter(remain, 254, encoding)?;
                            s.push(0); // null-terminated
                            let len = s.len();
                            remaining = rem;
                            mem.write_u8(0x0e)?; // pushstring
                            mem.write_u8(len as u8)?;
                            mem.write_all(&s)?;
                            let mut tindex = 0;
                            let tlen = new_funcs.len();
                            while tindex < tlen {
                                let toper = &new_funcs[tindex];
                                mem.write_u8(toper.opcode)?;
                                if matches!(toper.opcode, 0x02 | 0x06 | 0x07) {
                                    let addr_pos = mem.pos;
                                    let base_pos = patcher.output.stream_position()?;
                                    let addr = base_pos + addr_pos as u64;
                                    let data = toper
                                        .operands
                                        .iter()
                                        .find_map(|operand| {
                                            if let Operand::D(v) = operand {
                                                Some(*v)
                                            } else {
                                                None
                                            }
                                        })
                                        .ok_or(anyhow::anyhow!(
                                            "Unexpected operand type in function re-write."
                                        ))?;
                                    new_need_patch_addresses.push((addr, data));
                                }
                                if toper.opcode == 0x03 {
                                    let syscall_id =
                                        if let Some(Operand::W(id)) = toper.operands.get(0) {
                                            *id
                                        } else {
                                            anyhow::bail!(
                                                "Invalid syscall operand at function index {}",
                                                func_index
                                            );
                                        };
                                    if syscall_id == thread_start_callid {
                                        if tindex == 0 {
                                            anyhow::bail!(
                                                "ThreadStart syscall cannot be at function index 0"
                                            );
                                        }
                                        let pre_func = &new_funcs[tindex - 1];
                                        if pre_func.opcode == 0x0a {
                                            let addr_pos = mem.pos - 5; // 1 for opcode, 4 for operand
                                            let base_pos = patcher.output.stream_position()?;
                                            let addr = base_pos + addr_pos as u64;
                                            let data = pre_func
                                                .operands
                                                .get(0)
                                                .and_then(|operand| {
                                                    if let Operand::D(v) = operand {
                                                        Some(*v)
                                                    } else {
                                                        None
                                                    }
                                                })
                                                .ok_or(anyhow::anyhow!(
                                                    "Unexpected operand type in function re-write."
                                                ))?;
                                            new_need_patch_addresses.push((addr, data));
                                        }
                                    }
                                }
                                for operand in &toper.operands {
                                    match operand {
                                        Operand::B(v) => mem.write_u8(*v)?,
                                        Operand::W(v) => mem.write_u16(*v)?,
                                        Operand::D(v) => mem.write_u32(*v)?,
                                        Operand::F(v) => mem.write_f32(*v)?,
                                        _ => {
                                            return Err(anyhow::anyhow!(
                                                "Unexpected operand type in function re-write."
                                            ));
                                        }
                                    }
                                }
                                tindex += 1;
                            }
                        }
                        let new_data = mem.into_inner();
                        patcher.replace_bytes(0, &new_data)?;
                        continue 'outer;
                    }
                    patcher.replace_bytes_with_write(ori_len, |writer| {
                        writer.write_u8(len as u8)?;
                        writer.write_all(&s)?;
                        Ok(())
                    })?;
                }
                cur_pos += operand.len(self.encoding)? as u64;
            }
            func_index += 1;
        }
        patcher.copy_up_to(self.reader.data.len() as u64)?;
        for addr in need_pacth_addresses {
            patcher.patch_u32_address(addr)?;
        }
        for (addr, data) in new_need_patch_addresses {
            let new_data = patcher.map_offset(data as u64)? as u32;
            patcher.output.write_u32_at(addr, new_data)?;
        }
        let script_len = self.reader.cpeek_u32_at(0)? as u64;
        let new_script_len = patcher.map_offset(script_len)?;
        patcher.patch_u32(0, new_script_len as u32)?;
        // fix main script data position
        patcher.patch_u32_address(script_len)?;
        Ok(())
    }

    fn custom_export(&self, filename: &std::path::Path, encoding: Encoding) -> Result<()> {
        let s = if self.custom_yaml {
            serde_yaml_ng::to_string(&self.data)?
        } else {
            serde_json::to_string_pretty(&self.data)?
        };
        let e = encode_string(encoding, &s, false)?;
        let mut writer = crate::utils::files::write_file(filename)?;
        writer.write_all(&e)?;
        Ok(())
    }
}
