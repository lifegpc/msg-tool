use super::disasm::*;
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use anyhow::Result;
use std::cell::RefCell;
use std::io::Write;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug)]
/// Sliky mes script builder
pub struct MesBuilder {}

impl MesBuilder {
    /// Create a new Sliky mes script builder
    pub fn new() -> Self {
        Self {}
    }
}

impl ScriptBuilder for MesBuilder {
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
        Ok(Box::new(Mes::new(buf, encoding, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["mes"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::Silky
    }
}

struct TextParser<'a> {
    data: Vec<&'a str>,
    typ: SlikyStringType,
    opcodes: &'static Opcodes,
    encoding: Encoding,
    pos: usize,
}

impl<'a> TextParser<'a> {
    fn new(
        s: &'a str,
        typ: SlikyStringType,
        opcodes: &'static Opcodes,
        encoding: Encoding,
    ) -> Self {
        let data = s.graphemes(true).collect();
        Self {
            data,
            typ,
            opcodes,
            encoding,
            pos: 0,
        }
    }

    fn parse(mut self) -> Result<Vec<u8>> {
        match self.typ {
            SlikyStringType::Internal => Err(anyhow::anyhow!(
                "Internal strings cannot be parsed from text."
            )),
            SlikyStringType::Name => {
                let mut m = MemWriter::new();
                m.write_u8(self.opcodes.push_string)?;
                let s = encode_string(self.encoding, &self.data.join(""), false)?;
                m.write_all(&s)?;
                m.write_u8(0)?;
                Ok(m.into_inner())
            }
            SlikyStringType::Message => {
                let mut m = MemWriter::new();
                let mut in_ruby = false;
                let mut in_normal_text = false;
                while let Some(c) = self.next() {
                    match c {
                        "[" => {
                            if in_ruby {
                                return Err(anyhow::anyhow!("Nested ruby tags are not allowed."));
                            }
                            if in_normal_text {
                                m.write_u8(0)?;
                                in_normal_text = false;
                            }
                            in_ruby = true;
                            m.write_u8(self.opcodes.escape_sequence)?;
                            m.write_u8(1)?; // ruby start
                            m.write_u8(self.opcodes.message2)?;
                        }
                        "]" => {
                            if !in_ruby {
                                return Err(anyhow::anyhow!("Unmatched closing ruby tag."));
                            }
                            in_ruby = false;
                            m.write_u8(0)?;
                            m.write_u8(self.opcodes.r#yield)?;
                        }
                        "\n" => {
                            if in_ruby {
                                return Err(anyhow::anyhow!("Newline inside ruby is not allowed."));
                            }
                            if in_normal_text {
                                m.write_u8(0)?;
                                in_normal_text = false;
                            }
                            m.write_u8(self.opcodes.escape_sequence)?;
                            m.write_u8(0)?; // new line
                        }
                        _ => {
                            if !in_ruby && !in_normal_text {
                                in_normal_text = true;
                                m.write_u8(self.opcodes.message2)?;
                            }
                            let s = encode_string(self.encoding, c, false)?;
                            m.write_all(&s)?;
                        }
                    }
                }
                if in_ruby {
                    m.write_u8(0)?;
                    m.write_u8(self.opcodes.r#yield)?;
                }
                if in_normal_text {
                    m.write_u8(0)?;
                }
                Ok(m.into_inner())
            }
        }
    }

    fn next(&mut self) -> Option<&'a str> {
        if self.pos < self.data.len() {
            let c = self.data[self.pos];
            self.pos += 1;
            Some(c)
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct Mes {
    disasm: RefCell<Box<dyn Disasm>>,
    encoding: Encoding,
    texts: Vec<SlikyString>,
}

impl Mes {
    pub fn new(buf: Vec<u8>, encoding: Encoding, _config: &ExtraConfig) -> Result<Self> {
        let reader = MemReader::new(buf);
        let num_message = reader.cpeek_u32()?;
        let code_offset = 4 + num_message as u64 * 4;
        let first_line_offset = reader.cpeek_u32_at(4)? as u64 + code_offset;
        let mut disasm: Box<dyn Disasm> = if reader.cpeek_u8_at(first_line_offset)? == 0x19
            && reader.cpeek_u32_at(first_line_offset + 1)? == 0
        {
            Box::new(Ai6WinDisasm::new(reader)?)
        } else {
            Box::new(PlusDisasm::new(reader)?)
        };
        disasm.read_header()?;
        let texts = disasm.read_code()?;
        Ok(Self {
            disasm: RefCell::new(disasm),
            encoding,
            texts,
        })
    }

    fn code_to_text(&self, str: &SlikyString) -> Result<String> {
        let mut disasm = self.disasm.try_borrow_mut()?;
        let mut result = String::new();
        disasm.stream_mut().pos = str.start as usize;
        let end = str.start as usize + str.len as usize;
        let opcodes = disasm.opcodes();
        while disasm.stream().pos < end {
            let (opcode, operands) = disasm.read_instruction()?;
            if opcode == opcodes.push_string
                || (opcode == opcodes.message1 && !opcodes.is_message1_obfuscated)
                || opcode == opcodes.message2
            {
                if let Some(Obj::Str(s)) = operands.get(0) {
                    let s = disasm.stream().cpeek_fstring_at(
                        s.start,
                        s.len as usize,
                        self.encoding,
                        true,
                    )?;
                    result.push_str(&s);
                }
            } else if opcode == opcodes.message1 && opcodes.is_message1_obfuscated {
                if let Some(Obj::Str(s)) = operands.get(0) {
                    let mut deobfuscated = vec![0u8; (s.len as usize - 1) * 2];
                    let mut input_idx = 0;
                    let mut output_idx = 0;
                    let tlen = s.len - 1;
                    while input_idx < tlen {
                        let b = disasm.stream().cpeek_u8_at(s.start + input_idx)?;
                        input_idx += 1;
                        if matches!(b, 0x81..0xA0 | 0xE0..0xF0) {
                            deobfuscated[output_idx] = b;
                            output_idx += 1;
                            deobfuscated[output_idx] =
                                disasm.stream().cpeek_u8_at(s.start + input_idx)?;
                            input_idx += 1;
                            output_idx += 1;
                        } else {
                            let c = b as i32 - 0x7D62;
                            deobfuscated[output_idx] = (c >> 8) as u8;
                            output_idx += 1;
                            deobfuscated[output_idx] = (c & 0xFF) as u8;
                            output_idx += 1;
                        }
                    }
                    deobfuscated.truncate(output_idx);
                    let s = decode_to_string(self.encoding, &deobfuscated, true)?;
                    result.push_str(&s);
                }
            } else if opcode == opcodes.escape_sequence {
                if let Some(Obj::Byte(e)) = operands.get(0) {
                    match e {
                        // new line
                        0 => result.push('\n'),
                        // ruby
                        1 => result.push_str("["),
                        _ => {
                            return Err(anyhow::anyhow!("Unknown escape sequence: {}", e));
                        }
                    }
                }
            } else if opcode == opcodes.r#yield {
                result.push_str("]");
            }
        }
        Ok(result)
    }
}

impl Script for Mes {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Json
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn extract_messages(&self) -> Result<Vec<Message>> {
        let mut messages = Vec::new();
        let mut name = None;
        for t in self.texts.iter() {
            match t.typ {
                SlikyStringType::Internal => {}
                SlikyStringType::Name => {
                    name = Some(self.code_to_text(t)?);
                }
                SlikyStringType::Message => {
                    let message = self.code_to_text(t)?;
                    messages.push(Message {
                        name: name.take(),
                        message,
                    });
                }
            }
        }
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
        let opcodes = self.disasm.try_borrow()?.opcodes();
        let mut inp = self.disasm.try_borrow()?.stream().clone();
        inp.pos = 0;
        let mut patcher = BinaryPatcher::new(inp.to_ref(), file, |add| Ok(add), |add| Ok(add))?;
        let mut mess = messages.iter();
        let mut mes = mess.next();
        for text in &self.texts {
            patcher.copy_up_to(text.start)?;
            match text.typ {
                // Ignore internal strings
                SlikyStringType::Internal => {}
                SlikyStringType::Name => {
                    let m = match mes {
                        Some(m) => m,
                        None => {
                            return Err(anyhow::anyhow!("Not enough messages"));
                        }
                    };
                    let mut name = match &m.name {
                        Some(n) => n.to_string(),
                        None => {
                            return Err(anyhow::anyhow!("Message name is missing"));
                        }
                    };
                    if let Some(repl) = replacement {
                        for (k, v) in &repl.map {
                            name = name.replace(k, v);
                        }
                    }
                    let data =
                        TextParser::new(&name, SlikyStringType::Name, opcodes, encoding).parse()?;
                    patcher.replace_bytes(text.len, &data)?;
                }
                SlikyStringType::Message => {
                    let m = match mes {
                        Some(m) => m,
                        None => {
                            return Err(anyhow::anyhow!("Not enough messages"));
                        }
                    };
                    let mut message = m.message.to_string();
                    if let Some(repl) = replacement {
                        for (k, v) in &repl.map {
                            message = message.replace(k, v);
                        }
                    }
                    let data =
                        TextParser::new(&message, SlikyStringType::Message, opcodes, encoding)
                            .parse()?;
                    patcher.replace_bytes(text.len, &data)?;
                    mes = mess.next();
                }
            }
        }
        if mes.is_some() || mess.next().is_some() {
            return Err(anyhow::anyhow!("Too many messages"));
        }
        patcher.copy_up_to(inp.data.len() as u64)?;
        let code_offset = self.disasm.try_borrow()?.code_offset();
        for &address_offset in self.disasm.try_borrow()?.little_endian_addresses() {
            let orig_address = inp.cpeek_u32_at(address_offset as u64)? as u64;
            let orig_offset = orig_address + code_offset as u64;
            let new_offset = patcher.map_offset(orig_offset)?;
            let new_address = new_offset - code_offset as u64;
            patcher.patch_u32(address_offset as u64, new_address as u32)?;
        }
        for &address_offset in self.disasm.try_borrow()?.big_endian_addresses() {
            let orig_address = inp.cpeek_u32_be_at(address_offset as u64)? as u64;
            let orig_offset = orig_address + code_offset as u64;
            let new_offset = patcher.map_offset(orig_offset)?;
            let new_address = new_offset - code_offset as u64;
            patcher.patch_u32_be(address_offset as u64, new_address as u32)?;
        }
        Ok(())
    }
}

#[test]
fn test_text_parser() {
    let opcodes = &PLUS_OPCODES;
    let parser = TextParser::new(
        "Hello, [world]s\nThis is a test.",
        SlikyStringType::Message,
        opcodes,
        Encoding::Utf8,
    );
    let data = parser.parse().unwrap();
    assert_eq!(
        data,
        vec![
            opcodes.message2,
            b'H',
            b'e',
            b'l',
            b'l',
            b'o',
            b',',
            b' ',
            0,
            opcodes.escape_sequence,
            1,
            opcodes.message2,
            b'w',
            b'o',
            b'r',
            b'l',
            b'd',
            0,
            opcodes.r#yield,
            opcodes.message2,
            b's',
            0,
            opcodes.escape_sequence,
            0,
            opcodes.message2,
            b'T',
            b'h',
            b'i',
            b's',
            b' ',
            b'i',
            b's',
            b' ',
            b'a',
            b' ',
            b't',
            b'e',
            b's',
            b't',
            b'.',
            0
        ]
    );
}
