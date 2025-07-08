use super::list::{EnumScr, EscudeBinList, ListData, NameT};
use super::ops::base::CustomOps;
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::{decode_to_string, encode_string};
use crate::utils::struct_pack::StructPack;
use anyhow::Result;
use int_enum::IntEnum;
use std::collections::{BTreeSet, HashMap};
use std::ffi::CString;
use std::io::{Read, Seek, SeekFrom};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug)]
pub struct EscudeBinScriptBuilder {}

impl EscudeBinScriptBuilder {
    pub const fn new() -> Self {
        EscudeBinScriptBuilder {}
    }
}

impl ScriptBuilder for EscudeBinScriptBuilder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Cp932
    }

    fn build_script(
        &self,
        data: Vec<u8>,
        _filename: &str,
        encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(EscudeBinScript::new(data, encoding, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["bin"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::Escude
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len > 8 && buf.starts_with(b"ESCR1_00") {
            return Some(255);
        }
        None
    }
}

#[derive(Debug)]
pub struct EscudeBinScript {
    vms: Vec<u8>,
    unk1: u32,
    strings: Vec<String>,
    names: Option<HashMap<usize, String>>,
}

fn load_enum_script(
    filename: &str,
    encoding: Encoding,
    config: &ExtraConfig,
) -> Result<Vec<NameT>> {
    let buf = crate::utils::files::read_file(filename)?;
    let scr = EscudeBinList::new(buf, filename, encoding, config)?;
    for scr in scr.entries {
        match scr.data {
            ListData::Scr(scr) => match scr {
                EnumScr::Names(names) => return Ok(names),
                _ => {}
            },
            _ => {}
        }
    }
    Err(anyhow::anyhow!(
        "Failed to find name table in Escude enum script",
    ))
}

impl EscudeBinScript {
    pub fn new(data: Vec<u8>, encoding: Encoding, config: &ExtraConfig) -> Result<Self> {
        let mut reader = MemReader::new(data);
        let mut magic = [0u8; 8];
        reader.read_exact(&mut magic)?;
        if &magic != b"ESCR1_00" {
            return Err(anyhow::anyhow!(
                "Invalid Escude binary script magic: {:?}",
                magic
            ));
        }
        let string_count = reader.read_u32()?;
        let mut offsets = Vec::with_capacity(string_count as usize);
        for _ in 0..string_count {
            offsets.push(reader.read_u32()?);
        }
        let vm_count = reader.read_u32()?;
        let mut vms = Vec::with_capacity(vm_count as usize);
        vms.resize(vm_count as usize, 0);
        reader.read_exact(&mut vms)?;
        let unk1 = reader.read_u32()?;
        let mut strings = Vec::with_capacity(string_count as usize);
        if encoding.is_jis() {
            let replaces = StrReplacer::new()?;
            for _ in 0..string_count {
                let s = reader.read_cstring()?;
                let s = replaces.replace(s.as_bytes())?;
                strings.push(decode_to_string(encoding, &s, true)?);
            }
        } else {
            for _ in 0..string_count {
                let s = reader.read_cstring()?;
                strings.push(decode_to_string(encoding, s.as_bytes(), true)?);
            }
        }
        let names = match &config.escude_enum_scr {
            Some(loc) => match load_enum_script(loc, encoding, config) {
                Ok(list) => {
                    let mut names = HashMap::new();
                    let mut vm = VM::new(&vms);
                    vm.vars.insert(1, 1);
                    vm.vars.insert(132, 0);
                    vm.vars.insert(133, 0);
                    vm.vars.insert(134, 0);
                    vm.vars.insert(1001, 0);
                    vm.vars.insert(1003, 0);
                    for i in 135..140 {
                        vm.vars.insert(i, 1);
                    }
                    let _ = vm.run(Some(Box::new(super::ops::panicon::PaniconOps::new())));
                    for (index, name) in vm.names.iter() {
                        if let Some(name) = list.get(*name as usize) {
                            names.insert(*index as usize, name.text.clone());
                        }
                    }
                    Some(names)
                }
                Err(e) => {
                    eprintln!(
                        "WARN: Failed to load Escude enum script from {}: {}",
                        loc, e
                    );
                    crate::COUNTER.inc_warning();
                    None
                }
            },
            None => None,
        };
        Ok(EscudeBinScript {
            vms,
            unk1,
            strings,
            names,
        })
    }
}

impl Script for EscudeBinScript {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Json
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn extract_messages(&self) -> Result<Vec<Message>> {
        Ok(self
            .strings
            .iter()
            .enumerate()
            .map(|(i, s)| Message {
                message: s.replace("<r>", "\n"),
                name: self.names.as_ref().map(|n| n.get(&i).cloned()).flatten(),
            })
            .collect())
    }

    fn import_messages<'a>(
        &'a self,
        messages: Vec<Message>,
        mut writer: Box<dyn WriteSeek + 'a>,
        encoding: Encoding,
        replacement: Option<&'a ReplacementTable>,
    ) -> Result<()> {
        writer.write_all(b"ESCR1_00")?;
        let mut offsets = Vec::with_capacity(messages.len());
        let mut strs = Vec::with_capacity(messages.len());
        let mut len = 0;
        for message in messages {
            offsets.push(len);
            let mut s = message.message.replace("\n", "<r>");
            if let Some(repl) = replacement {
                for (from, to) in &repl.map {
                    s = s.replace(from, to);
                }
            }
            let encoded = encode_string(encoding, &s, false)?;
            len += encoded.len() as u32 + 1;
            strs.push(CString::new(encoded)?);
        }
        writer.write_u32(offsets.len() as u32)?;
        offsets.pack(&mut writer, false, encoding)?;
        writer.write_u32(self.vms.len() as u32)?;
        writer.write_all(&self.vms)?;
        writer.write_u32(self.unk1)?;
        for s in strs {
            writer.write_all(s.as_bytes_with_nul())?;
        }
        Ok(())
    }

    fn is_archive(&self) -> bool {
        false
    }
}

struct StrReplacer {
    pub replacements: HashMap<Vec<u8>, Vec<u8>>,
}

enum JisStr {
    Single(u8),
    Double(u8, u8),
}

impl StrReplacer {
    pub fn new() -> Result<Self> {
        let mut s = StrReplacer {
            replacements: HashMap::new(),
        };
        // 0xa0 to 0xde: Half-width katakana in CP932
        let half_width_katakana = "！？　。「」、…をぁぃぅぇぉゃゅょっーあいうえおかきくけこさしすせそたちつてとなにぬねのはひふへほまみむめもやゆよらりるれろわん゛゜";
        let mut bytes: Vec<u8> = (0xa0..=0xde).collect();
        bytes.insert(0, 0x21);
        bytes.insert(1, 0x3f);
        s.add(&bytes, half_width_katakana)?;
        Ok(s)
    }

    fn add(&mut self, from: &[u8], to: &str) -> Result<()> {
        let encoding = Encoding::Cp932; // Default encoding, can be changed as needed
        let tos = UnicodeSegmentation::graphemes(to, true);
        for (from, to) in from.into_iter().zip(tos) {
            let from_bytes = vec![from.clone()];
            let to_bytes = encode_string(encoding, to, true)?;
            self.replacements.insert(from_bytes, to_bytes);
        }
        Ok(())
    }

    pub fn replace(&self, input: &[u8]) -> Result<Vec<u8>> {
        let mut result = Vec::new();
        let mut reader = MemReaderRef::new(input);
        while let Ok(byte) = reader.read_u8() {
            if byte < 0x80 || (byte >= 0xa0 && byte <= 0xdf) {
                result.push(JisStr::Single(byte));
            } else if (byte >= 0x81 && byte <= 0x9f) || (byte >= 0xe0 && byte <= 0xef) {
                let next_byte = reader.read_u8()?;
                if next_byte < 0x40 || next_byte > 0xfc {
                    return Err(anyhow::anyhow!("Invalid JIS encoding sequence"));
                }
                result.push(JisStr::Double(byte, next_byte));
            } else {
                return Err(anyhow::anyhow!("Invalid byte in JIS encoding: {}", byte));
            }
        }
        let mut output = Vec::new();
        for item in result {
            match item {
                JisStr::Single(byte) => {
                    let vec = vec![byte];
                    if let Some(replacement) = self.replacements.get(&vec) {
                        output.extend_from_slice(replacement);
                    } else {
                        output.push(byte);
                    }
                }
                JisStr::Double(byte1, byte2) => {
                    let key = vec![byte1, byte2];
                    if let Some(replacement) = self.replacements.get(&key) {
                        output.extend_from_slice(replacement);
                    } else {
                        output.push(byte1);
                        output.push(byte2);
                    }
                }
            }
        }
        Ok(output)
    }
}

#[repr(u8)]
#[derive(Debug, IntEnum)]
enum BaseOp {
    End,
    Jump,
    JumpZ,
    Call,
    Ret,
    Push,
    Pop,
    Str,
    SetVar,
    GetVar,
    SetFlag,
    GetFlag,
    Neg,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Not,
    And,
    Or,
    Xor,
    Shr,
    Shl,
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
    LNot,
    LAnd,
    LOr,
    FileLine,
}

pub trait ReadParam<T> {
    fn read_param(&mut self) -> Result<T>;
}

#[derive(Debug)]
pub struct VM<'a, T: std::fmt::Debug> {
    pub reader: MemReaderRef<'a>,
    pub data: Vec<T>,
    pub stack: Vec<u64>,
    pub strs: Vec<T>,
    pub vars: HashMap<T, T>,
    pub flags: HashMap<T, bool>,
    pub mess: BTreeSet<T>,
    pub names: HashMap<T, T>,
}

impl ReadParam<i32> for MemReaderRef<'_> {
    fn read_param(&mut self) -> Result<i32> {
        Ok(self.read_i32()?)
    }
}

impl<'a, T> VM<'a, T>
where
    MemReaderRef<'a>: ReadParam<T>,
    T: TryInto<u64>
        + Default
        + Eq
        + Ord
        + Copy
        + std::fmt::Debug
        + std::fmt::Display
        + std::hash::Hash
        + From<u8>
        + std::ops::Neg<Output = T>
        + std::ops::Add<Output = T>
        + std::ops::Sub<Output = T>
        + std::ops::Mul<Output = T>
        + std::ops::Div<Output = T>
        + std::ops::Rem<Output = T>
        + std::ops::Not<Output = T>
        + std::ops::BitAnd<Output = T>
        + std::ops::BitOr<Output = T>
        + std::ops::BitXor<Output = T>
        + std::ops::Shr<Output = T>
        + std::ops::Shl<Output = T>,
    anyhow::Error: From<<T as TryInto<u64>>::Error>,
{
    pub fn new(data: &'a [u8]) -> Self {
        VM {
            reader: MemReaderRef::new(data),
            data: Vec::new(),
            stack: Vec::new(),
            strs: Vec::new(),
            vars: HashMap::new(),
            flags: HashMap::new(),
            mess: BTreeSet::new(),
            names: HashMap::new(),
        }
    }

    pub fn pop_data(&mut self) -> Result<T> {
        self.data
            .pop()
            .ok_or_else(|| anyhow::anyhow!("No data to pop"))
    }

    fn pop_stack(&mut self) -> Result<u64> {
        self.stack
            .pop()
            .ok_or_else(|| anyhow::anyhow!("No stack to pop"))
    }

    pub fn run(&mut self, mut custom_ops: Option<Box<dyn CustomOps<T>>>) -> Result<()> {
        loop {
            if self.reader.is_eof() {
                break;
            }
            let op = self.reader.read_u8()?;
            if let Ok(op) = BaseOp::try_from(op) {
                // println!("Op code: {op:?}");
                match op {
                    BaseOp::End => break,
                    BaseOp::Jump => {
                        let offset: T = self.reader.read_param()?;
                        let offset: u64 = offset.try_into()?;
                        self.reader.seek(SeekFrom::Start(offset))?;
                    }
                    BaseOp::JumpZ => {
                        let offset: T = self.reader.read_param()?;
                        let offset: u64 = offset.try_into()?;
                        if self.pop_data()? == Default::default() {
                            self.reader.seek(SeekFrom::Start(offset))?;
                        }
                    }
                    BaseOp::Call => {
                        let offset: T = self.reader.read_param()?;
                        let offset: u64 = offset.try_into()?;
                        let pos = self.reader.stream_position()?;
                        self.stack.push(pos);
                        self.reader.seek(SeekFrom::Start(offset))?;
                    }
                    BaseOp::Ret => {
                        if self.stack.is_empty() {
                            let code = self.reader.read_u8()?;
                            if code == 0 && self.reader.is_eof() {
                                break;
                            }
                        }
                        let stack = self.pop_stack()?;
                        self.reader.seek(SeekFrom::Start(stack))?;
                    }
                    BaseOp::Push => {
                        let d = self.reader.read_param()?;
                        self.data.push(d);
                    }
                    BaseOp::Pop => {
                        self.pop_data()?;
                    }
                    BaseOp::Str => {
                        let param = self.reader.read_param()?;
                        self.strs.push(param);
                        self.data.push(param);
                    }
                    BaseOp::SetVar => {
                        let value = self.pop_data()?;
                        let index = self.pop_data()?;
                        self.vars.insert(index, value);
                        self.data.push(value);
                    }
                    BaseOp::GetVar => {
                        let index = self.pop_data()?;
                        let value = self
                            .vars
                            .get(&index)
                            .ok_or_else(|| anyhow::anyhow!("Variable not found: {}", index))?;
                        self.data.push(*value);
                    }
                    BaseOp::SetFlag => {
                        let value = self.pop_data()?;
                        let index = self.pop_data()?;
                        let flag = value != Default::default();
                        self.flags.insert(index, flag);
                        self.data.push(value);
                    }
                    BaseOp::GetFlag => {
                        let index = self.pop_data()?;
                        let flag = self.flags.get(&index).cloned().unwrap_or(false);
                        self.data
                            .push(if flag { T::from(1u8) } else { T::from(0u8) });
                    }
                    BaseOp::Neg => {
                        let value = -self.pop_data()?;
                        self.data.push(value);
                    }
                    BaseOp::Add => {
                        let b = self.pop_data()?;
                        let a = self.pop_data()?;
                        self.data.push(a + b);
                    }
                    BaseOp::Sub => {
                        let b = self.pop_data()?;
                        let a = self.pop_data()?;
                        self.data.push(a - b);
                    }
                    BaseOp::Mul => {
                        let b = self.pop_data()?;
                        let a = self.pop_data()?;
                        self.data.push(a * b);
                    }
                    BaseOp::Div => {
                        let b = self.pop_data()?;
                        if b == Default::default() {
                            return Err(anyhow::anyhow!("Division by zero"));
                        }
                        let a = self.pop_data()?;
                        self.data.push(a / b);
                    }
                    BaseOp::Mod => {
                        let b = self.pop_data()?;
                        if b == Default::default() {
                            return Err(anyhow::anyhow!("Division by zero"));
                        }
                        let a = self.pop_data()?;
                        self.data.push(a % b);
                    }
                    BaseOp::Not => {
                        let value = self.pop_data()?;
                        self.data.push(!value);
                    }
                    BaseOp::And => {
                        let b = self.pop_data()?;
                        let a = self.pop_data()?;
                        self.data.push(a & b);
                    }
                    BaseOp::Or => {
                        let b = self.pop_data()?;
                        let a = self.pop_data()?;
                        self.data.push(a | b);
                    }
                    BaseOp::Xor => {
                        let b = self.pop_data()?;
                        let a = self.pop_data()?;
                        self.data.push(a ^ b);
                    }
                    BaseOp::Shr => {
                        let b = self.pop_data()?;
                        let a = self.pop_data()?;
                        self.data.push(a >> b);
                    }
                    BaseOp::Shl => {
                        let b = self.pop_data()?;
                        let a = self.pop_data()?;
                        self.data.push(a << b);
                    }
                    BaseOp::Eq => {
                        let b = self.pop_data()?;
                        let a = self.pop_data()?;
                        self.data
                            .push(if a == b { T::from(1u8) } else { T::from(0u8) });
                    }
                    BaseOp::Ne => {
                        let b = self.pop_data()?;
                        let a = self.pop_data()?;
                        self.data
                            .push(if a != b { T::from(1u8) } else { T::from(0u8) });
                    }
                    // Original code may contains undefined behavior for these operations
                    BaseOp::Gt => {
                        let a = self.pop_data()?;
                        let b = self.pop_data()?;
                        self.data
                            .push(if a > b { T::from(1u8) } else { T::from(0u8) });
                    }
                    BaseOp::Ge => {
                        let a = self.pop_data()?;
                        let b = self.pop_data()?;
                        self.data
                            .push(if a >= b { T::from(1u8) } else { T::from(0u8) });
                    }
                    BaseOp::Lt => {
                        let a = self.pop_data()?;
                        let b = self.pop_data()?;
                        self.data
                            .push(if a < b { T::from(1u8) } else { T::from(0u8) });
                    }
                    BaseOp::Le => {
                        let a = self.pop_data()?;
                        let b = self.pop_data()?;
                        self.data
                            .push(if a <= b { T::from(1u8) } else { T::from(0u8) });
                    }
                    BaseOp::LNot => {
                        let value = self.pop_data()?;
                        self.data.push(if value == Default::default() {
                            T::from(1u8)
                        } else {
                            T::from(0u8)
                        });
                    }
                    BaseOp::LAnd => {
                        let b = self.pop_data()? != Default::default();
                        let a = self.pop_data()? != Default::default();
                        self.data
                            .push(if a && b { T::from(1u8) } else { T::from(0u8) });
                    }
                    BaseOp::LOr => {
                        let b = self.pop_data()? != Default::default();
                        let a = self.pop_data()? != Default::default();
                        self.data
                            .push(if a || b { T::from(1u8) } else { T::from(0u8) });
                    }
                    BaseOp::FileLine => {
                        let _: T = self.reader.read_param()?;
                    }
                }
                continue;
            }
            if let Some(ops) = &mut custom_ops {
                let nbreak = ops.run(self, op)?;
                if nbreak {
                    break;
                }
            } else {
                return Err(anyhow::anyhow!("Unknown operation: {}", op));
            }
        }
        Ok(())
    }

    pub fn skip_n_params(&mut self, n: u64, nbreak: bool) -> Result<bool> {
        for _ in 0..n {
            self.pop_data()?;
        }
        Ok(nbreak)
    }

    pub fn skip_params(&mut self, nbreak: bool) -> Result<bool> {
        let count: T = self.reader.read_param()?;
        let count: u64 = count.try_into()?;
        self.skip_n_params(count, nbreak)
    }

    pub fn read_params(&mut self, ncount: Option<u64>) -> Result<Vec<T>> {
        let count = match ncount {
            Some(count) => count,
            None => {
                let count: T = self.reader.read_param()?;
                count.try_into()?
            }
        };
        let data_len = self.data.len();
        if (data_len as u64) < count {
            return Err(anyhow::anyhow!(
                "Not enough data to read {} parameters, only {} parameters available",
                count,
                data_len
            ));
        }
        let mut params = Vec::with_capacity(count as usize);
        params.resize(count as usize, Default::default());
        params.copy_from_slice(&self.data[data_len - count as usize..]);
        self.data.truncate(data_len - count as usize);
        Ok(params)
    }
}
