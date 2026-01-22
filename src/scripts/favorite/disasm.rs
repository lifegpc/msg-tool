use crate::ext::io::*;
use crate::types::*;
use crate::utils::encoding::*;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::{Read, Seek, SeekFrom};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Oper {
    // Byte
    B,
    // Word
    W,
    // Double Word
    D,
    // String
    S,
    // Float
    F,
}

use Oper::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", content = "c")]
pub enum Operand {
    B(u8),
    W(u16),
    D(u32),
    S(String),
    F(f32),
}

impl Operand {
    pub fn len(&self, encoding: Encoding) -> Result<usize> {
        Ok(match self {
            Operand::B(_) => 1,
            Operand::W(_) => 2,
            Operand::D(_) => 4,
            Operand::S(s) => {
                let bytes = encode_string(encoding, s, true)?;
                // null terminator + length byte
                bytes.len() + 2
            }
            Operand::F(_) => 4,
        })
    }
}

const OPS: [(u8, &[Oper]); 49] = [
    (0x00, &[]),     //noop
    (0x01, &[B, B]), //initstack
    (0x02, &[D]),    //call
    (0x03, &[W]),    //syscall
    (0x04, &[]),     //ret
    (0x05, &[]),     //ret2
    (0x06, &[D]),    //jmp
    (0x07, &[D]),    //jmpcond
    (0x08, &[]),     //pushtrue
    (0x09, &[]),     //pushfalse
    (0x0a, &[D]),    //pushint
    (0x0b, &[W]),    //pushint
    (0x0c, &[B]),    //pushint
    (0x0d, &[F]),    //pushfloat * unused
    (0x0e, &[S]),    //pushstring
    (0x0f, &[W]),    //pushglobal
    (0x10, &[B]),    //pushstack
    (0x11, &[W]),    //unknown
    (0x12, &[B]),    //unknown
    (0x13, &[]),     //pushtop
    (0x14, &[]),     //pushtmp
    (0x15, &[W]),    //popglobal
    (0x16, &[B]),    //copystack
    (0x17, &[W]),    //unknown
    (0x18, &[B]),    //unknown
    (0x19, &[]),     //neg
    (0x1a, &[]),     //add
    (0x1b, &[]),     //sub
    (0x1c, &[]),     //mul
    (0x1d, &[]),     //div
    (0x1e, &[]),     //mod
    (0x1f, &[]),     //test
    (0x20, &[]),     //logand
    (0x21, &[]),     //logor
    (0x22, &[]),     //eq
    (0x23, &[]),     //neq
    (0x24, &[]),     //gt
    (0x25, &[]),     //le
    (0x26, &[]),     //lt
    (0x27, &[]),     //ge
    (0x33, &[]),
    (0x3f, &[]),
    (0x40, &[]),
    (0xb3, &[]),
    (0xb8, &[]),
    (0xd8, &[]),
    (0xf0, &[]),
    (0x52, &[]),
    (0x9e, &[]),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Func {
    pub pos: u64,
    pub opcode: u8,
    pub operands: Vec<Operand>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Data {
    pub functions: Vec<Func>,
    pub main_script: Vec<Func>,
    pub extra_data: Vec<u8>,
    #[serde(skip)]
    speak_func_indices: HashSet<u32>,
    #[serde(skip)]
    func_pos_map: HashMap<u64, usize>,
    #[serde(skip)]
    speaker_names: HashMap<usize, Vec<String>>,
}

impl Data {
    pub fn disasm<R: Read + Seek>(mut reader: R, encoding: Encoding) -> Result<Self> {
        let mut data = Data {
            functions: Vec::new(),
            main_script: Vec::new(),
            extra_data: Vec::new(),
            speak_func_indices: HashSet::new(),
            func_pos_map: HashMap::new(),
            speaker_names: HashMap::new(),
        };
        let script_len = reader.read_u32()? as u64;
        let main_script_data = reader.peek_u32_at(script_len)? as u64;
        {
            let mut target = &mut data.functions;
            let mut pos = reader.stream_position()?;
            while pos < script_len {
                if pos >= main_script_data {
                    target = &mut data.main_script;
                }
                target.push(Self::read_func(&mut reader, encoding)?);
                pos = reader.stream_position()?;
            }
        }
        reader.seek(SeekFrom::Start(script_len + 4))?;
        reader.read_to_end(&mut data.extra_data)?;

        data.index_functions();
        data.find_speak_functions();
        data.collect_speaker_names();

        Ok(data)
    }

    fn index_functions(&mut self) {
        for (idx, func) in self.functions.iter().enumerate() {
            if func.opcode == 0x01 {
                self.func_pos_map.insert(func.pos, idx);
            }
        }
    }

    fn find_speak_functions(&mut self) {
        for (idx, func) in self.functions.iter().enumerate() {
            if func.opcode == 0x01 {
                // SPEAK functions have initstack with (3, 0) or (5, 0) parameters
                if let (Some(Operand::B(arg_count)), Some(Operand::B(0))) =
                    (func.operands.first(), func.operands.get(1))
                {
                    if *arg_count == 3 || *arg_count == 5 {
                        self.speak_func_indices.insert(idx as u32);
                    }
                }
            }
        }
    }

    fn collect_speaker_names(&mut self) {
        let func_starts: Vec<usize> = self
            .functions
            .iter()
            .enumerate()
            .filter(|(_, f)| f.opcode == 0x01)
            .map(|(i, _)| i)
            .collect();

        for &speak_idx in &self.speak_func_indices {
            let speak_idx = speak_idx as usize;

            let start_pos = func_starts.iter().position(|&s| s == speak_idx);
            if let Some(pos) = start_pos {
                let end = func_starts.get(pos + 1).copied().unwrap_or(self.functions.len());
                let names: Vec<String> = (speak_idx..end)
                    .filter(|&i| self.functions[i].opcode == 0x0e)
                    .filter_map(|i| match self.functions[i].operands.first() {
                        Some(Operand::S(s)) if !s.trim().is_empty() => Some(s.clone()),
                        _ => None,
                    })
                    .collect();

                if !names.is_empty() {
                    self.speaker_names.insert(speak_idx, names);
                }
            }
        }
    }

    fn get_speaker(&self, func_idx: usize) -> Option<String> {
        let names = self.speaker_names.get(&func_idx)?;

        // Prefer names without '？' prefix, take the last one (usually the "known" name)
        if let Some(name) = names.iter().filter(|n| !n.contains('？')).last() {
            return Some(name.trim().to_string());
        }

        // If all names have '？', strip it from the last one
        names.last().and_then(|name| {
            let cleaned = name.trim().trim_start_matches('？').trim();
            if !cleaned.is_empty() {
                Some(cleaned.to_string())
            } else {
                None
            }
        })
    }

    pub fn extract_messages(&self, filter_ascii: bool) -> Vec<(Option<String>, String)> {
        let mut messages = Vec::new();

        // Extract strings from functions section (no speakers)
        for func in &self.functions {
            if func.opcode == 0x0e {
                if let Some(Operand::S(s)) = func.operands.first() {
                    if !(filter_ascii && s.chars().all(|c| c.is_ascii())) {
                        messages.push((None, s.clone()));
                    }
                }
            }
        }

        // Process main_script, track SPEAK calls for speaker names
        let mut current_speaker: Option<String> = None;

        for func in &self.main_script {
            if func.opcode == 0x02 {
                if let Some(Operand::D(call_target)) = func.operands.first() {
                    if let Some(&func_idx) = self.func_pos_map.get(&(*call_target as u64)) {
                        if self.speak_func_indices.contains(&(func_idx as u32)) {
                            current_speaker = self.get_speaker(func_idx);
                        }
                    }
                }
            } else if func.opcode == 0x0e {
                if let Some(Operand::S(s)) = func.operands.first() {
                    if !(filter_ascii && s.chars().all(|c| c.is_ascii())) {
                        messages.push((current_speaker.clone(), s.clone()));
                    }
                }
            }
        }

        messages
    }

    fn read_func<R: Read + Seek>(reader: &mut R, encoding: Encoding) -> Result<Func> {
        let pos = reader.stream_position()?;
        let opcode = reader.read_u8()?;
        let operands = if let Some((_, ops)) = OPS.iter().find(|(code, _)| *code == opcode) {
            let mut operands = Vec::with_capacity(ops.len());
            for &op in *ops {
                let operand = match op {
                    B => Operand::B(reader.read_u8()?),
                    W => Operand::W(reader.read_u16()?),
                    D => Operand::D(reader.read_u32()?),
                    S => {
                        let len = reader.read_u8()? as usize;
                        let s = reader.read_cstring()?;
                        if s.as_bytes_with_nul().len() != len {
                            return Err(anyhow::anyhow!(
                                "String length mismatch at {:#x}: expected {}, got {}",
                                pos,
                                len,
                                s.as_bytes_with_nul().len()
                            ));
                        }
                        let s = decode_to_string(encoding, s.as_bytes(), true)?;
                        Operand::S(s)
                    }
                    F => Operand::F(reader.read_f32()?),
                };
                operands.push(operand);
            }
            operands
        } else {
            return Err(anyhow::anyhow!(
                "Unknown opcode: {:#x} at {:#x}",
                opcode,
                pos
            ));
        };
        Ok(Func {
            pos,
            opcode,
            operands,
        })
    }
}
