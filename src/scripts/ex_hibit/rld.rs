//! ExHibit Script File (.rld)
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use crate::utils::struct_pack::*;
use anyhow::Result;
use msg_tool_macro::*;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{Read, Seek, Write};

#[derive(Debug)]
/// Builder for ExHibit RLD script files
pub struct RldScriptBuilder {}

impl RldScriptBuilder {
    /// Creates a new instance of `RldScriptBuilder`
    pub fn new() -> Self {
        Self {}
    }
}

impl ScriptBuilder for RldScriptBuilder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Cp932
    }

    fn build_script(
        &self,
        buf: Vec<u8>,
        filename: &str,
        encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(RldScript::new(buf, filename, encoding, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["rld"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::ExHibit
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 4 && buf.starts_with(b"\0DLR") {
            return Some(10);
        }
        None
    }
}

#[derive(Debug)]
struct XorKey {
    xor_key: u32,
    keys: [u32; 0x100],
}

#[derive(Debug, StructPack, StructUnpack)]
struct Header {
    ver: u32,
    offset: u32,
    count: u32,
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
struct Op {
    op: u16,
    init_count: u8,
    unk: u8,
}

impl PartialEq<u16> for Op {
    fn eq(&self, other: &u16) -> bool {
        self.op == *other
    }
}

impl Op {
    pub fn str_count(&self) -> u8 {
        self.unk & 0xF
    }
}

#[derive(Clone, Debug)]
struct OpExt {
    op: Op,
    strs: Vec<String>,
    ints: Vec<u32>,
}

impl<'de> Deserialize<'de> for OpExt {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct OpExtHelper {
            op: u16,
            unk: u8,
            strs: Vec<String>,
            ints: Vec<u32>,
        }

        let helper = OpExtHelper::deserialize(deserializer)?;
        let init_count = helper.ints.len() as u8;
        let str_count = helper.strs.len() as u8;
        let unk = (helper.unk << 4) | (str_count & 0xF);

        Ok(OpExt {
            op: Op {
                op: helper.op,
                init_count,
                unk,
            },
            strs: helper.strs,
            ints: helper.ints,
        })
    }
}

impl Serialize for OpExt {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("OpExt", 4)?;
        state.serialize_field("op", &self.op.op)?;
        state.serialize_field("unk", &((self.op.unk & 0xF0) >> 4))?;
        state.serialize_field("strs", &self.strs)?;
        state.serialize_field("ints", &self.ints)?;
        state.end()
    }
}

impl StructPack for OpExt {
    fn pack<W: Write>(&self, writer: &mut W, big: bool, encoding: Encoding) -> Result<()> {
        self.op.op.pack(writer, big, encoding)?;
        let init_count = self.ints.len() as u8;
        init_count.pack(writer, big, encoding)?;
        let unk = (self.op.unk & 0xF0) | (self.strs.len() as u8 & 0xF);
        unk.pack(writer, big, encoding)?;
        for i in &self.ints {
            i.pack(writer, big, encoding)?;
        }
        for s in &self.strs {
            let encoded = encode_string(encoding, s, true)?;
            writer.write_all(&encoded)?;
            writer.write_u8(0)?; // Null terminator for C-style strings
        }
        Ok(())
    }
}

impl StructUnpack for OpExt {
    fn unpack<R: Read + Seek>(reader: &mut R, big: bool, encoding: Encoding) -> Result<Self> {
        let op = Op::unpack(reader, big, encoding)?;
        let mut ints = Vec::with_capacity(op.init_count as usize);
        for _ in 0..op.init_count {
            let i = u32::unpack(reader, big, encoding)?;
            ints.push(i);
        }
        let mut strs = Vec::with_capacity(op.str_count() as usize);
        for _ in 0..op.str_count() {
            let s = reader.read_cstring()?;
            let s = decode_to_string(encoding, s.as_bytes(), true)?;
            strs.push(s);
        }
        Ok(Self { op, strs, ints })
    }
}

#[derive(Debug)]
/// ExHibit RLD script file
pub struct RldScript {
    data: MemReader,
    decrypted: bool,
    xor_key: Option<XorKey>,
    header: Header,
    _flag: u32,
    _tag: Option<String>,
    ops: Vec<OpExt>,
    is_def_chara: bool,
    name_table: Option<BTreeMap<u32, String>>,
    custom_yaml: bool,
}

impl RldScript {
    /// Creates a new `RldScript`
    ///
    /// * `buf` - The buffer containing the RLD script data
    /// * `filename` - The name of the file
    /// * `encoding` - The encoding of the script
    /// * `config` - Extra configuration options
    pub fn new(
        buf: Vec<u8>,
        filename: &str,
        encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<Self> {
        let mut reader = MemReader::new(buf);
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;
        if &magic != b"\0DLR" {
            return Err(anyhow::anyhow!("Invalid RLD script magic: {:?}", magic));
        }
        let is_def = std::path::Path::new(filename)
            .file_stem()
            .map(|s| s.to_ascii_lowercase() == "def")
            .unwrap_or(false);
        let is_def_chara = std::path::Path::new(filename)
            .file_stem()
            .map(|s| s.to_ascii_lowercase() == "defchara")
            .unwrap_or(false);
        let xor_key = if is_def {
            if let Some(xor_key) = config.ex_hibit_rld_def_xor_key {
                let keys = config
                    .ex_hibit_rld_def_keys
                    .as_deref()
                    .cloned()
                    .ok_or(anyhow::anyhow!("No keys provided for def RLD script"))?;
                Some(XorKey {
                    xor_key,
                    keys: keys,
                })
            } else {
                None
            }
        } else {
            if let Some(xor_key) = config.ex_hibit_rld_xor_key {
                let keys = config
                    .ex_hibit_rld_keys
                    .as_deref()
                    .cloned()
                    .ok_or(anyhow::anyhow!("No keys provided for RLD script"))?;
                Some(XorKey {
                    xor_key,
                    keys: keys,
                })
            } else {
                None
            }
        };
        let header = Header::unpack(&mut reader, false, encoding)?;
        let mut decrypted = false;
        if let Some(key) = &xor_key {
            Self::xor(&mut reader.data, key);
            decrypted = true;
        }
        let flag = reader.read_u32()?;
        let tag = if flag == 1 {
            let s = reader.read_cstring()?;
            Some(decode_to_string(encoding, s.as_bytes(), true)?)
        } else {
            None
        };
        reader.pos = header.offset as usize;
        let mut ops = Vec::with_capacity(header.count as usize);
        for _ in 0..header.count {
            let op = OpExt::unpack(&mut reader, false, encoding)?;
            ops.push(op);
        }
        let name_table = if is_def_chara {
            None
        } else {
            match Self::try_load_name_table(filename, encoding, config) {
                Ok(table) => Some(table),
                Err(e) => {
                    eprintln!("WARN: Failed to load name table: {}", e);
                    crate::COUNTER.inc_warning();
                    None
                }
            }
        };
        Ok(Self {
            data: reader,
            decrypted,
            xor_key,
            header,
            _flag: flag,
            _tag: tag,
            ops,
            is_def_chara,
            name_table,
            custom_yaml: config.custom_yaml,
        })
    }

    fn try_load_name_table(
        filename: &str,
        encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<BTreeMap<u32, String>> {
        let mut pb = std::path::Path::new(filename).to_path_buf();
        pb.set_file_name("defChara.rld");
        let f = crate::utils::files::read_file(&pb)?;
        let f = Self::new(f, &pb.to_string_lossy(), encoding, config)?;
        Ok(f.name_table()?)
    }

    fn xor(data: &mut Vec<u8>, key: &XorKey) {
        let mut end = data.len().min(0xFFCF);
        end -= end % 4;
        let mut ri = 0;
        for i in (0x10..end).step_by(4) {
            let en_temp = u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]);
            let temp_key = key.keys[ri & 0xFF] ^ key.xor_key;
            let de_temp = (en_temp ^ temp_key).to_le_bytes();
            data[i] = de_temp[0];
            data[i + 1] = de_temp[1];
            data[i + 2] = de_temp[2];
            data[i + 3] = de_temp[3];
            ri += 1;
        }
    }

    fn name_table(&self) -> Result<BTreeMap<u32, String>> {
        let mut names = BTreeMap::new();
        for op in &self.ops {
            if op.op == 48 {
                if op.strs.is_empty() {
                    return Err(anyhow::anyhow!("Op 48 has no strings"));
                }
                let name = op.strs[0].clone();
                let data: Vec<_> = name.split(",").collect();
                if data.len() < 4 {
                    return Err(anyhow::anyhow!("Op 48 has invalid data: {}", name));
                }
                let id = data[0].parse::<u32>()?;
                let name = data[3].to_string();
                names.insert(id, name);
            }
        }
        Ok(names)
    }

    fn write_script<W: Write + Seek>(
        &self,
        mut writer: W,
        encoding: Encoding,
        ops: &[OpExt],
    ) -> Result<()> {
        writer.write_all(&self.data.data[..self.header.offset as usize])?;
        let op_count = ops.len() as u32;
        if op_count != self.header.count {
            writer.write_u32_at(12, op_count)?;
        }
        for op in ops {
            op.pack(&mut writer, false, encoding)?;
        }
        if self.data.data.len() > self.data.pos {
            writer.write_all(&self.data.data[self.data.pos..])?;
        }
        Ok(())
    }
}

impl Script for RldScript {
    fn default_output_script_type(&self) -> OutputScriptType {
        if self.is_def_chara {
            return OutputScriptType::Custom;
        }
        OutputScriptType::Json
    }

    fn is_output_supported(&self, output: OutputScriptType) -> bool {
        if self.is_def_chara {
            return matches!(output, OutputScriptType::Custom);
        }
        true
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn custom_output_extension<'a>(&'a self) -> &'a str {
        if self.custom_yaml { "yaml" } else { "json" }
    }

    fn extract_messages(&self) -> Result<Vec<Message>> {
        let mut messages = Vec::new();
        for op in &self.ops {
            if op.op == 28 {
                if op.strs.len() < 2 {
                    return Err(anyhow::anyhow!("Op 28 has less than 2 strings"));
                }
                let name = if op.strs[0] == "*" {
                    if op.ints.is_empty() {
                        return Err(anyhow::anyhow!("Op 28 has no integers"));
                    }
                    let id = op.ints[0];
                    self.name_table
                        .as_ref()
                        .and_then(|table| table.get(&id).cloned())
                } else if op.strs[0] == "$noname$" {
                    None
                } else {
                    Some(op.strs[0].clone())
                };
                let text = op.strs[1].clone();
                messages.push(Message {
                    name,
                    message: text,
                });
            } else if op.op == 21 || op.op == 191 {
                eprintln!("{op:?}");
            }
        }
        Ok(messages)
    }

    fn import_messages<'a>(
        &'a self,
        messages: Vec<Message>,
        mut file: Box<dyn WriteSeek + 'a>,
        _filename: &str,
        encoding: Encoding,
        replacement: Option<&'a ReplacementTable>,
    ) -> Result<()> {
        let mut ops = self.ops.clone();
        let mut mes = messages.iter();
        let mut mess = mes.next();
        for op in ops.iter_mut() {
            if op.op == 28 {
                let m = match mess {
                    Some(m) => m,
                    None => return Err(anyhow::anyhow!("Not enough messages.")),
                };
                if op.strs.len() < 2 {
                    return Err(anyhow::anyhow!("Op 28 has less than 2 strings"));
                }
                if op.strs[0] != "*" && op.strs[0] != "$noname$" {
                    let mut name = match &m.name {
                        Some(name) => name.clone(),
                        None => {
                            return Err(anyhow::anyhow!("Message has no name"));
                        }
                    };
                    if let Some(replacement) = replacement {
                        for (k, v) in &replacement.map {
                            name = name.replace(k, v);
                        }
                    }
                    op.strs[0] = name;
                }
                let mut message = m.message.clone();
                if let Some(replacement) = replacement {
                    for (k, v) in &replacement.map {
                        message = message.replace(k, v);
                    }
                }
                op.strs[1] = message;
                mess = mes.next();
            }
        }
        if mess.is_some() || mes.next().is_some() {
            return Err(anyhow::anyhow!("Too many messages provided."));
        }
        if self.decrypted {
            let mut writer = MemWriter::new();
            self.write_script(&mut writer, encoding, &ops)?;
            if let Some(key) = &self.xor_key {
                Self::xor(&mut writer.data, key);
            }
            file.write_all(&writer.data)?;
        } else {
            self.write_script(&mut file, encoding, &ops)?;
        }
        Ok(())
    }

    fn custom_export(&self, filename: &std::path::Path, encoding: Encoding) -> Result<()> {
        let s = if self.is_def_chara {
            let names = self.name_table()?;
            if self.custom_yaml {
                serde_yaml_ng::to_string(&names)
                    .map_err(|e| anyhow::anyhow!("Failed to serialize to YAML: {}", e))?
            } else {
                serde_json::to_string(&names)
                    .map_err(|e| anyhow::anyhow!("Failed to serialize to JSON: {}", e))?
            }
        } else {
            if self.custom_yaml {
                serde_yaml_ng::to_string(&self.ops)
                    .map_err(|e| anyhow::anyhow!("Failed to serialize to YAML: {}", e))?
            } else {
                serde_json::to_string(&self.ops)
                    .map_err(|e| anyhow::anyhow!("Failed to serialize to JSON: {}", e))?
            }
        };
        let s = encode_string(encoding, &s, false)?;
        let mut file = std::fs::File::create(filename)?;
        file.write_all(&s)?;
        Ok(())
    }

    fn custom_import<'a>(
        &'a self,
        custom_filename: &'a str,
        mut file: Box<dyn WriteSeek + 'a>,
        encoding: Encoding,
        output_encoding: Encoding,
    ) -> Result<()> {
        let f = crate::utils::files::read_file(custom_filename)?;
        let s = decode_to_string(output_encoding, &f, true)?;
        let ops: Vec<OpExt> = if self.is_def_chara {
            let mut ops = self.ops.clone();
            let names: BTreeMap<u32, String> = if self.custom_yaml {
                serde_yaml_ng::from_str(&s)
                    .map_err(|e| anyhow::anyhow!("Failed to parse YAML: {}", e))?
            } else {
                serde_json::from_str(&s)
                    .map_err(|e| anyhow::anyhow!("Failed to parse JSON: {}", e))?
            };
            for op in ops.iter_mut() {
                if op.op == 48 {
                    if op.strs.is_empty() {
                        return Err(anyhow::anyhow!("Op 48 has no strings"));
                    }
                    let name = op.strs[0].clone();
                    let data: Vec<_> = name.split(",").collect();
                    if data.len() < 4 {
                        return Err(anyhow::anyhow!("Op 48 has invalid data: {}", name));
                    }
                    let id = data[0].parse::<u32>()?;
                    let name = names
                        .get(&id)
                        .cloned()
                        .unwrap_or_else(|| data[3].to_string());
                    let mut data = data.iter().map(|s| s.to_string()).collect::<Vec<_>>();
                    data[3] = name;
                    op.strs[0] = data.join(",");
                }
            }
            ops
        } else {
            if self.custom_yaml {
                serde_yaml_ng::from_str(&s)
                    .map_err(|e| anyhow::anyhow!("Failed to parse YAML: {}", e))?
            } else {
                serde_json::from_str(&s)
                    .map_err(|e| anyhow::anyhow!("Failed to parse JSON: {}", e))?
            }
        };
        if self.decrypted {
            let mut writer = MemWriter::new();
            self.write_script(&mut writer, encoding, &ops)?;
            if let Some(key) = &self.xor_key {
                Self::xor(&mut writer.data, key);
            }
            file.write_all(&writer.data)?;
        } else {
            self.write_script(&mut file, encoding, &ops)?;
        }
        Ok(())
    }
}

/// Load the keys from a file
pub fn load_keys(path: Option<&String>) -> Result<Option<Box<[u32; 0x100]>>> {
    if let Some(path) = path {
        let f = crate::utils::files::read_file(path)?;
        let mut reader = MemReader::new(f);
        let mut keys = [0u32; 0x100];
        for i in 0..0x100 {
            keys[i] = reader.read_u32()?;
        }
        Ok(Some(Box::new(keys)))
    } else {
        Ok(None)
    }
}

#[test]
fn test_ser() {
    let op = OpExt {
        op: Op {
            op: 28,
            init_count: 1,
            unk: 0x10 | 2,
        },
        strs: vec!["name".to_string(), "message".to_string()],
        ints: vec![123],
    };
    let json = serde_json::to_string(&op).unwrap();
    assert_eq!(
        json,
        r#"{"op":28,"unk":1,"strs":["name","message"],"ints":[123]}"#
    );
}

#[test]
fn test_de_ser() {
    let json = r#"{"op":28,"unk":1,"strs":["name","message"],"ints":[123]}"#;
    let op: OpExt = serde_json::from_str(json).unwrap();
    assert_eq!(op.op.op, 28);
    assert_eq!(op.op.init_count, 1);
    assert_eq!(op.op.unk, 0x10 | 2);
    assert_eq!(op.strs[0], "name");
    assert_eq!(op.strs[1], "message");
    assert_eq!(op.ints[0], 123);
}
