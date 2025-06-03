use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::{decode_to_string, encode_string};
use crate::utils::struct_pack::StructPack;
use anyhow::Result;
use std::collections::HashMap;
use std::ffi::CString;
use std::io::Read;
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
}

impl EscudeBinScript {
    pub fn new(data: Vec<u8>, encoding: Encoding, _config: &ExtraConfig) -> Result<Self> {
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
                strings.push(decode_to_string(encoding, &s)?);
            }
        } else {
            for _ in 0..string_count {
                let s = reader.read_cstring()?;
                strings.push(decode_to_string(encoding, s.as_bytes())?);
            }
        }
        Ok(EscudeBinScript { vms, unk1, strings })
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
            .map(|s| Message {
                message: s.to_string(),
                name: None,
            })
            .collect())
    }

    fn import_messages(
        &self,
        messages: Vec<Message>,
        mut writer: Box<dyn WriteSeek>,
        encoding: Encoding,
        replacement: Option<&ReplacementTable>,
    ) -> Result<()> {
        writer.write_all(b"ESCR1_00")?;
        let mut offsets = Vec::with_capacity(messages.len());
        let mut strs = Vec::with_capacity(messages.len());
        let mut len = 0;
        for message in messages {
            offsets.push(len);
            let mut s = message.message;
            if let Some(repl) = replacement {
                for (from, to) in &repl.map {
                    s = s.replace(from, to);
                }
            }
            let encoded = encode_string(encoding, &s, true)?;
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
        s.add("!?｡｢｣､･ｦｧｨｩｪｫｬｭｮｯｰｱｲｳｴｵｶｷｸｹｺｻｼｽｾｿﾀﾁﾂﾃﾄﾅﾆﾇﾈﾉﾊﾋﾌﾍﾎﾏﾐﾑﾒﾓﾔﾕﾖﾗﾘﾙﾚﾛﾜﾝﾞﾟ", "！？　。「」、…をぁぃぅぇぉゃゅょっーあいうえおかきくけこさしすせそたちつてとなにぬねのはひふへほまみむめもやゆよらりるれろわん゛゜")?;
        Ok(s)
    }

    fn add(&mut self, from: &str, to: &str) -> Result<()> {
        let encoding = Encoding::Cp932; // Default encoding, can be changed as needed
        let froms = UnicodeSegmentation::graphemes(from, true);
        let tos = UnicodeSegmentation::graphemes(to, true);
        for (from, to) in froms.zip(tos) {
            let from_bytes = if from == "" {
                vec![0xa0]
            } else {
                encode_string(encoding, from, true)?
            };
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
