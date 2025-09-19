use crate::ext::io::*;
use anyhow::Result;
use int_enum::IntEnum;
use std::collections::HashMap;
use std::io::{Read, Write};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Oper {
    P,
    I,
    L,
}

use Oper::*;

const OPS: [(u16, (Option<&'static str>, &'static [Oper])); 210] = [
    (0x0001, (Some("mov"), &[P, P])),
    (0x0002, (Some("add"), &[P, P])),
    (0x0003, (Some("sub"), &[P, P])),
    (0x0004, (Some("mul"), &[P, P])),
    (0x0005, (Some("div"), &[P, P])),
    (0x0006, (Some("binand"), &[P, P])),
    (0x0007, (Some("binor"), &[P, P])),
    (0x0008, (Some("binxor"), &[P, P])),
    (0x0009, (Some("jmp"), &[L])),
    (0x000A, (Some("jz"), &[L, P])),
    (0x000B, (Some("call"), &[L])),
    (0x000C, (Some("eq"), &[P, P])),
    (0x000D, (Some("neq"), &[P, P])),
    (0x000E, (Some("le"), &[P, P])),
    (0x000F, (Some("ge"), &[P, P])),
    (0x0010, (Some("lt"), &[P, P])),
    (0x0011, (Some("gt"), &[P, P])),
    (0x0012, (Some("logor"), &[P, P])),
    (0x0013, (Some("logand"), &[P, P])),
    (0x0014, (Some("not"), &[I])),
    (0x0015, (Some("exit"), &[])),
    (0x0016, (Some("nop"), &[])),
    (0x0017, (Some("syscall"), &[I, I])),
    (0x0018, (Some("ret"), &[])),
    (0x0019, (None, &[])),
    (0x001A, (Some("mod"), &[P, P])),
    (0x001B, (Some("shl"), &[P, P])),
    (0x001C, (Some("sar"), &[P, P])),
    (0x001D, (Some("neg"), &[I])),
    (0x001E, (Some("pop"), &[P])),
    (0x001F, (Some("push"), &[P])),
    (0x0020, (Some("enter"), &[P])),
    (0x0021, (Some("leave"), &[P])),
    (0x0023, (Some("create_message"), &[])),
    (0x0024, (Some("get_message"), &[])),
    (0x0025, (Some("get_message_param"), &[])),
    (0x0028, (Some("se_load"), &[])),
    (0x0029, (Some("se_play"), &[])),
    (0x002A, (Some("se_play_ex"), &[])),
    (0x002B, (Some("se_stop"), &[])),
    (0x002C, (Some("se_set_volume"), &[])),
    (0x002D, (Some("se_get_volume"), &[])),
    (0x002E, (Some("se_unload"), &[])),
    (0x002F, (Some("se_wait"), &[])),
    (0x0030, (Some("set_se_info"), &[])),
    (0x0031, (Some("get_se_ex_volume"), &[])),
    (0x0032, (Some("set_se_ex_volume"), &[])),
    (0x0033, (Some("se_enable"), &[])),
    (0x0034, (Some("is_se_enable"), &[])),
    (0x0035, (Some("se_set_pan"), &[])),
    (0x0036, (Some("se_mute"), &[])),
    (0x0038, (Some("select_init"), &[])),
    (0x0039, (Some("select"), &[])),
    (0x003A, (Some("select_add_choice"), &[])),
    (0x003B, (Some("end_select"), &[])),
    (0x003C, (Some("select_clear"), &[])),
    (0x003D, (Some("select_set_offset"), &[])),
    (0x003E, (Some("select_set_process"), &[])),
    (0x003F, (Some("select_lock"), &[])),
    (0x0040, (Some("get_select_on_key"), &[])),
    (0x0041, (Some("get_select_pull_key"), &[])),
    (0x0042, (Some("get_select_push_key"), &[])),
    (0x0044, (Some("skip_set"), &[])),
    (0x0045, (Some("skip_is"), &[])),
    (0x0046, (Some("auto_set"), &[])),
    (0x0047, (Some("auto_is"), &[])),
    (0x0048, (Some("auto_set_time"), &[])),
    (0x0049, (Some("auto_get_time"), &[])),
    (0x004A, (Some("window_set_mode"), &[])),
    (0x004B, (None, &[])),
    (0x004C, (None, &[])),
    (0x004D, (None, &[])),
    (0x004E, (None, &[])),
    (0x004F, (Some("effect_enable_is"), &[])),
    (0x0050, (Some("cursor_pos_get"), &[])),
    (0x0051, (Some("time_get"), &[])),
    (0x0052, (None, &[])),
    (0x0053, (Some("load_font"), &[])),
    (0x0054, (Some("unload_font"), &[])),
    (0x0055, (Some("set_font_type"), &[])),
    (0x0056, (Some("key_cancel"), &[])),
    (0x0057, (Some("set_font_color"), &[])),
    (0x0058, (Some("load_font_ex"), &[])),
    (0x0059, (None, &[])),
    (0x005A, (None, &[])),
    (0x005B, (Some("lpush"), &[])),
    (0x005C, (Some("lpop"), &[])),
    (0x005D, (None, &[])),
    (0x005E, (None, &[])),
    (0x005F, (Some("set_font_size"), &[])),
    (0x0060, (Some("get_font_size"), &[])),
    (0x0061, (Some("get_font_type"), &[])),
    (0x0062, (Some("set_font_effect"), &[])),
    (0x0063, (Some("get_font_effect"), &[])),
    (0x0064, (Some("get_pull_key"), &[])),
    (0x0065, (Some("get_on_key"), &[])),
    (0x0066, (Some("get_push_key"), &[])),
    (0x0067, (Some("input_clear"), &[])),
    (0x0068, (Some("change_window_size"), &[])),
    (0x0069, (Some("change_aspect_mode"), &[])),
    (0x006A, (Some("aspect_position_enable"), &[])),
    (0x006B, (None, &[])),
    (0x006C, (Some("get_aspect_mode"), &[])),
    (0x006D, (Some("get_monitor_size"), &[])),
    (0x006E, (Some("get_window_pos"), &[])),
    (0x006F, (Some("get_system_metrics"), &[])),
    (0x0070, (Some("set_system_path"), &[])),
    (0x0071, (Some("set_allmosaicthumbnail"), &[])),
    (0x0072, (Some("enable_window_change"), &[])),
    (0x0073, (Some("is_enable_window_change"), &[])),
    (0x0074, (Some("set_cursor"), &[])),
    (0x0075, (Some("set_hide_cursor_time"), &[])),
    (0x0076, (Some("get_hide_cursor_time"), &[])),
    (0x0077, (Some("scene_skip"), &[])),
    (0x0078, (Some("cancel_scene_skip"), &[])),
    (0x0079, (Some("lsize"), &[])),
    (0x007A, (Some("get_async_key"), &[])),
    (0x007B, (Some("get_font_color"), &[])),
    (0x007C, (Some("get_current_date"), &[])),
    (0x007D, (Some("history_skip"), &[])),
    (0x007E, (Some("cancel_history_skip"), &[])),
    (0x007F, (None, &[])),
    (0x0081, (Some("system_btn_set"), &[])),
    (0x0082, (Some("system_btn_release"), &[])),
    (0x0083, (Some("system_btn_enable"), &[])),
    (0x0086, (Some("text_init"), &[])),
    (0x0087, (Some("text_set_icon"), &[])),
    (0x0088, (Some("text"), &[])),
    (0x0089, (Some("text_hide"), &[])),
    (0x008A, (Some("text_show"), &[])),
    (0x008B, (Some("text_set_btn"), &[])),
    (0x008C, (Some("text_uninit"), &[])),
    (0x008D, (Some("text_set_rect"), &[])),
    (0x008E, (Some("text_clear"), &[])),
    (0x008F, (None, &[])),
    (0x0090, (Some("text_get_time"), &[])),
    (0x0091, (Some("text_window_set_alpha"), &[])),
    (0x0092, (Some("text_voice_play"), &[])),
    (0x0093, (None, &[])),
    (0x0094, (Some("text_set_icon_animation_time"), &[])),
    (0x0095, (Some("text_w"), &[])),
    (0x0096, (Some("text_a"), &[])),
    (0x0097, (Some("text_wa"), &[])),
    (0x0098, (Some("text_n"), &[])),
    (0x0099, (Some("text_cat"), &[])),
    (0x009A, (Some("set_history"), &[])),
    (0x009B, (Some("is_text_visible"), &[])),
    (0x009C, (Some("text_set_base"), &[])),
    (0x009D, (Some("enable_voice_cut"), &[])),
    (0x009E, (Some("is_voice_cut"), &[])),
    (0x009F, (None, &[])),
    (0x00A0, (None, &[])),
    (0x00A1, (None, &[])),
    (0x00A2, (Some("text_set_color"), &[])),
    (0x00A3, (Some("text_redraw"), &[])),
    (0x00A4, (Some("set_text_mode"), &[])),
    (0x00A5, (Some("text_init_visualnovelmode"), &[])),
    (0x00A6, (Some("text_set_icon_mode"), &[])),
    (0x00A7, (Some("text_vn_br"), &[])),
    (0x00A8, (None, &[])),
    (0x00A9, (None, &[])),
    (0x00AA, (None, &[])),
    (0x00AB, (None, &[])),
    (0x00AC, (Some("tips_get_str"), &[])),
    (0x00AD, (Some("tips_get_param"), &[])),
    (0x00AE, (Some("tips_reset"), &[])),
    (0x00AF, (Some("tips_search"), &[])),
    (0x00B0, (Some("tips_set_color"), &[])),
    (0x00B1, (Some("tips_stop"), &[])),
    (0x00B2, (Some("tips_get_flag"), &[])),
    (0x00B3, (Some("tips_init"), &[])),
    (0x00B4, (Some("tips_pause"), &[])),
    (0x00B6, (Some("voice_play"), &[])),
    (0x00B7, (Some("voice_stop"), &[])),
    (0x00B8, (Some("voice_set_volume"), &[])),
    (0x00B9, (Some("voice_get_volume"), &[])),
    (0x00BA, (Some("set_voice_info"), &[])),
    (0x00BB, (Some("voice_enable"), &[])),
    (0x00BC, (Some("is_voice_enable"), &[])),
    (0x00BD, (None, &[])),
    (0x00BE, (Some("bgv_play"), &[])),
    (0x00BF, (Some("bgv_stop"), &[])),
    (0x00C0, (Some("bgv_enable"), &[])),
    (0x00C1, (Some("get_voice_ex_volume"), &[])),
    (0x00C2, (Some("set_voice_ex_volume"), &[])),
    (0x00C3, (Some("voice_check_enable"), &[])),
    (0x00C4, (Some("voice_autopan_initialize"), &[])),
    (0x00C5, (Some("voice_autopan_enable"), &[])),
    (0x00C6, (Some("set_voice_autopan"), &[])),
    (0x00C7, (Some("is_voice_autopan_enable"), &[])),
    (0x00C8, (Some("voice_wait"), &[])),
    (0x00C9, (Some("bgv_pause"), &[])),
    (0x00CA, (Some("bgv_mute"), &[])),
    (0x00CB, (Some("set_bgv_volume"), &[])),
    (0x00CC, (Some("get_bgv_volume"), &[])),
    (0x00CD, (Some("set_bgv_auto_volume"), &[])),
    (0x00CE, (Some("voice_mute"), &[])),
    (0x00CF, (Some("voice_call"), &[])),
    (0x00D0, (Some("voice_call_clear"), &[])),
    (0x00D2, (Some("wait"), &[])),
    (0x00D3, (Some("wait_click"), &[])),
    (0x00D4, (Some("wait_sync_begin"), &[])),
    (0x00D5, (Some("wait_sync"), &[])),
    (0x00D6, (Some("wait_sync_end"), &[])),
    (0x00D7, (None, &[])),
    (0x00D8, (Some("wait_clear"), &[])),
    (0x00D9, (Some("wait_click_no_anim"), &[])),
    (0x00DA, (Some("wait_sync_get_time"), &[])),
    (0x00DB, (Some("wait_time_push"), &[])),
    (0x00DC, (Some("wait_time_pop"), &[])),
];
const MOV: u16 = 0x0001;
const CALL: u16 = 0x000B;
const SYSCALL: u16 = 0x0017;
const RET: u16 = 0x0018;
const PUSH: u16 = 0x001F;
const ENTER: u16 = 0x0020;
const SELECT_ADD_CHOICE: u16 = 0x003A;
const TEXT: u16 = 0x0088;
const TEXT_W: u16 = 0x0095;
const TEXT_A: u16 = 0x0096;
const TEXT_WA: u16 = 0x0097;
const TEXT_N: u16 = 0x0098;
const TEXT_CAT: u16 = 0x0099;
pub const CODE_OFFSET: u32 = 0xC;

#[derive(Clone, Copy)]
struct Operand {
    offset: u32,
    raw_value: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, IntEnum)]
#[repr(u32)]
enum OperandType {
    Literal = 0,
    Variable = 4,
    Argument = 8,
    UNK = 0xFF,
}

impl Operand {
    pub fn typ(&self) -> OperandType {
        let typ = (self.raw_value >> 28) & 0xF;
        OperandType::try_from(typ).unwrap_or(OperandType::UNK)
    }

    pub fn raw_type(&self) -> u32 {
        (self.raw_value >> 28) & 0xF
    }

    pub fn value(&self) -> u32 {
        self.raw_value & 0x0FFFFFFF
    }
}

impl std::fmt::Display for Operand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{:08X}", self.raw_value)
    }
}

struct Instruction {
    offset: u32,
    opcode: u16,
    operands: Vec<Operand>,
}

impl Instruction {
    pub fn is_message(&self) -> bool {
        match self.opcode {
            TEXT | TEXT_W | TEXT_A | TEXT_WA | TEXT_N | TEXT_CAT => true,
            SYSCALL => {
                if self.operands.is_empty() {
                    false
                } else {
                    let raw_value = self.operands[0].raw_value;
                    match raw_value {
                        0x20002 | 0x2000F | 0x20010 | 0x20011 | 0x20012 | 0x20013 => true,
                        _ => false,
                    }
                }
            }
            _ => false,
        }
    }
}

struct UserMessageFunction {
    num_args: u32,
    name_arg_index: u32,
    message_arg_index: u32,
}

pub struct Disasm<'a> {
    reader: MemReaderRef<'a>,
    label_offsets: Vec<u32>,
    user_message_functions: HashMap<u32, UserMessageFunction>,
    variables: HashMap<u32, Operand>,
    stack: Vec<Operand>,
    strs: Vec<PalString>,
    pre_is_hover_text_move: bool,
}

#[derive(Debug)]
pub enum StringType {
    Name,
    Message,
    /// Hover text
    Hover,
    /// Label
    Label,
}

impl StringType {
    pub fn is_label(&self) -> bool {
        matches!(self, StringType::Label)
    }
}

#[derive(Debug)]
pub struct PalString {
    pub offset: u32,
    pub typ: StringType,
}

impl<'a> Disasm<'a> {
    pub fn new(data: &'a [u8], label_offsets: &[u32]) -> Result<Self> {
        let mut reader = MemReaderRef::new(data);
        let mut magic = [0; 4];
        reader.read_exact(&mut magic)?;
        if magic != *b"Sv20" {
            return Err(anyhow::anyhow!(
                "Invalid magic number for Softpal script: {:?}",
                magic
            ));
        }
        Ok(Self {
            reader,
            label_offsets: label_offsets.to_vec(),
            user_message_functions: HashMap::new(),
            variables: HashMap::new(),
            stack: Vec::new(),
            strs: Vec::new(),
            pre_is_hover_text_move: false,
        })
    }

    pub fn disassemble<W: Write + ?Sized>(
        mut self,
        mut writer: Option<&mut W>,
    ) -> Result<Vec<PalString>> {
        self.find_user_message_functions()?;
        self.reader.pos = CODE_OFFSET as usize;
        let len = self.reader.data.len();
        while self.reader.pos < len {
            let instr = self.read_instruction()?;
            if let Some(writer) = writer.as_mut() {
                self.write_instruction_to(&instr, writer)?;
            }
            let is_hover_text_move = instr.opcode == MOV
                && instr.operands[0].typ() == OperandType::Variable
                && instr.operands[0].value() == 2
                && instr.operands[1].typ() == OperandType::Literal
                && instr.operands[1].value() < 0xFFFFFFF;
            if instr.is_message() {
                self.handle_message_instruction()?;
            } else if instr.opcode == MOV {
                self.handle_mov_instruction(instr)?;
            } else if instr.opcode == PUSH {
                self.handle_push_instruction(instr)?;
            } else if instr.opcode == CALL {
                self.handle_call_instruction(instr)?;
            } else if instr.opcode == SYSCALL {
                self.handle_syscall_instruction(instr)?;
            } else if instr.opcode == SELECT_ADD_CHOICE {
                self.handle_select_choice_instruction()?;
            } else {
                self.stack.clear();
                self.variables.clear();
            }
            self.pre_is_hover_text_move = is_hover_text_move;
        }
        Ok(self.strs)
    }

    fn read_instruction(&mut self) -> Result<Instruction> {
        let offset = self.reader.pos as u32;
        let opcode = self.reader.read_u32()?;
        if (opcode >> 16) != 1 {
            return Err(anyhow::anyhow!(
                "Invalid opcode format: 0x{:08X} at offset 0x{:08X}",
                opcode,
                offset
            ));
        }
        let opcode = (opcode & 0xFFFF) as u16;
        let (_, (_, opers)) = OPS.iter().find(|(op, _)| *op == opcode).ok_or_else(|| {
            anyhow::anyhow!(
                "Unknown opcode: 0x{:04X} at offset 0x{:08X}",
                opcode,
                offset
            )
        })?;
        let mut operands = Vec::new();
        for _ in *opers {
            let offset = self.reader.pos as u32;
            let raw_value = self.reader.read_u32()?;
            operands.push(Operand { offset, raw_value });
        }
        Ok(Instruction {
            offset,
            opcode,
            operands,
        })
    }

    fn write_instruction_to(&self, instr: &Instruction, writer: &mut dyn Write) -> Result<()> {
        let (_, (name, opers)) =
            OPS.iter()
                .find(|(op, _)| *op == instr.opcode)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Unknown opcode: 0x{:04X} at offset 0x{:08X}",
                        instr.opcode,
                        instr.offset
                    )
                })?;
        if let Some(name) = name {
            write!(writer, "0x{:08X} {}", instr.offset, name)?;
        } else {
            write!(writer, "0x{:08X} 0x{:04X}", instr.offset, instr.opcode)?;
        }
        for i in 0..instr.operands.len() {
            writer.write_all(if i == 0 { b" " } else { b", " })?;
            let value = instr.operands[i].value();
            let mut typ = opers[i];
            if typ == L {
                if instr.operands[i].typ() == OperandType::Literal {
                    write!(writer, "#0x{:08X}", self.label_offsets[value as usize - 1])?;
                } else {
                    typ = P;
                }
            }
            if typ == P {
                match instr.operands[i].typ() {
                    OperandType::Literal => write!(writer, "0x{:08X}", value)?,
                    OperandType::Variable => write!(writer, "var_{}", value)?,
                    OperandType::Argument => write!(writer, "arg_{}", value)?,
                    OperandType::UNK => {
                        write!(writer, "{}:[0x{:08X}]", instr.operands[i].raw_type(), value)?
                    }
                }
            } else if typ == I {
                write!(writer, "0x{:08X}", value)?;
            }
        }
        writeln!(writer)?;
        if instr.opcode == RET {
            writeln!(writer)?;
        }
        Ok(())
    }

    fn find_user_message_functions(&mut self) -> Result<()> {
        let mut current_func_args = None;
        self.reader.pos = CODE_OFFSET as usize;
        let len = self.reader.data.len();
        while self.reader.pos < len {
            let instr = self.read_instruction()?;
            if instr.is_message() {
                if let Some((func_offset, func_num_args)) = current_func_args {
                    if self.stack.len() >= 4 {
                        let _number = self.stack.pop().unwrap();
                        let name = self.stack.pop().unwrap();
                        let message = self.stack.pop().unwrap();
                        if name.typ() == OperandType::Argument
                            && message.typ() == OperandType::Argument
                        {
                            self.user_message_functions.insert(
                                func_offset,
                                UserMessageFunction {
                                    num_args: func_num_args,
                                    name_arg_index: name.value() - 1,
                                    message_arg_index: message.value() - 1,
                                },
                            );
                            current_func_args = None;
                        }
                    }
                }
                self.stack.clear();
                self.variables.clear();
                continue;
            }
            match instr.opcode {
                ENTER => {
                    current_func_args = Some((instr.offset, instr.operands[0].value()));
                    self.stack.clear();
                    self.variables.clear();
                }
                MOV if instr.operands[0].typ() == OperandType::Variable => {
                    self.variables
                        .insert(instr.operands[0].value(), instr.operands[1]);
                }
                PUSH => {
                    if instr.operands[0].typ() == OperandType::Variable
                        && self.variables.contains_key(&instr.operands[0].value())
                    {
                        let var = self.variables.get(&instr.operands[0].value()).unwrap();
                        self.stack.push(*var);
                    } else {
                        self.stack.push(instr.operands[0]);
                    }
                }
                RET => {
                    current_func_args = None;
                    self.stack.clear();
                    self.variables.clear();
                }
                _ => {
                    self.stack.clear();
                    self.variables.clear();
                }
            }
        }
        Ok(())
    }

    fn handle_mov_instruction(&mut self, instr: Instruction) -> Result<()> {
        if instr.operands[0].typ() == OperandType::Variable {
            self.variables
                .insert(instr.operands[0].value(), instr.operands[1]);
        }
        Ok(())
    }

    fn handle_push_instruction(&mut self, instr: Instruction) -> Result<()> {
        if instr.operands[0].typ() == OperandType::Variable
            && self.variables.contains_key(&instr.operands[0].value())
        {
            let var = self.variables.get(&instr.operands[0].value()).unwrap();
            if self.pre_is_hover_text_move && instr.operands[0].value() == 2 {
                self.strs.push(PalString {
                    offset: var.offset,
                    typ: StringType::Hover,
                });
            }
            self.stack.push(*var);
        } else {
            self.stack.push(instr.operands[0]);
        }
        Ok(())
    }

    fn handle_call_instruction(&mut self, instr: Instruction) -> Result<()> {
        self.handle_call_instruction_internal(instr)?;
        self.stack.clear();
        self.variables.clear();
        Ok(())
    }

    fn handle_call_instruction_internal(&mut self, instr: Instruction) -> Result<()> {
        if self.label_offsets.is_empty() || instr.operands[0].typ() != OperandType::Literal {
            return Ok(());
        }
        let target_offset = self.label_offsets[instr.operands[0].value() as usize - 1];
        let message_func = match self.user_message_functions.get(&target_offset) {
            Some(func) => func,
            None => return Ok(()),
        };
        if self.stack.len() < message_func.num_args as usize {
            return Ok(());
        }
        let mut args = Vec::new();
        for _ in 0..message_func.num_args {
            args.push(self.stack.pop().unwrap());
        }
        args.reverse();
        let name = args[message_func.name_arg_index as usize];
        let message = args[message_func.message_arg_index as usize];
        if name.typ() == OperandType::Literal && message.typ() == OperandType::Literal {
            self.strs.push(PalString {
                offset: name.offset,
                typ: StringType::Name,
            });
            self.strs.push(PalString {
                offset: message.offset,
                typ: StringType::Message,
            });
        }
        Ok(())
    }

    fn handle_syscall_instruction(&mut self, instr: Instruction) -> Result<()> {
        match instr.operands[0].raw_value {
            0x60002 => {
                self.handle_select_choice_instruction()?;
            }
            0x20014 => {
                self.handle_another_message()?;
            }
            0xf0002 => {
                self.handle_label()?;
            }
            _ => {
                self.stack.clear();
            }
        }
        Ok(())
    }

    fn handle_message_instruction(&mut self) -> Result<()> {
        self.handle_message_instruction_internal()?;
        self.stack.clear();
        self.variables.clear();
        Ok(())
    }

    fn handle_another_message(&mut self) -> Result<()> {
        if self.stack.len() < 3 {
            return Ok(());
        }
        let _message_id = self.stack.pop().unwrap();
        let name = self.stack.pop().unwrap();
        let message = self.stack.pop().unwrap();
        if name.typ() != OperandType::Literal || message.typ() != OperandType::Literal {
            return Ok(());
        }
        self.strs.push(PalString {
            offset: name.offset,
            typ: StringType::Name,
        });
        self.strs.push(PalString {
            offset: message.offset,
            typ: StringType::Message,
        });
        Ok(())
    }

    fn handle_label(&mut self) -> Result<()> {
        if self.stack.len() < 1 {
            return Ok(());
        }
        let label = self.stack.pop().unwrap();
        if label.typ() != OperandType::Literal {
            return Ok(());
        }
        self.strs.push(PalString {
            offset: label.offset,
            typ: StringType::Label,
        });
        Ok(())
    }

    fn handle_message_instruction_internal(&mut self) -> Result<()> {
        if self.stack.len() < 4 {
            return Ok(());
        }
        let _number = self.stack.pop().unwrap();
        let name = self.stack.pop().unwrap();
        let message = self.stack.pop().unwrap();
        if name.typ() != OperandType::Literal || message.typ() != OperandType::Literal {
            return Ok(());
        }
        self.strs.push(PalString {
            offset: name.offset,
            typ: StringType::Name,
        });
        self.strs.push(PalString {
            offset: message.offset,
            typ: StringType::Message,
        });
        Ok(())
    }

    fn handle_select_choice_instruction(&mut self) -> Result<()> {
        self.handle_select_choice_instruction_internal()?;
        self.stack.clear();
        self.variables.clear();
        Ok(())
    }

    fn handle_select_choice_instruction_internal(&mut self) -> Result<()> {
        if self.stack.len() < 1 {
            return Ok(());
        }
        let choice = self.stack.pop().unwrap();
        self.strs.push(PalString {
            offset: choice.offset,
            typ: StringType::Message,
        });
        Ok(())
    }
}
