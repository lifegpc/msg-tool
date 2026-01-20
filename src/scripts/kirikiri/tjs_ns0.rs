//! Kirikiri TJS NS0 binary encoded script
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::{decode_to_string, encode_string};
use crate::utils::struct_pack::*;
use anyhow::Result;
use msg_tool_macro::*;
use overf::wrapping;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{Read, Seek, Write};

#[derive(Debug)]
/// Kirikiri TJS NS0 Script Builder
pub struct TjsNs0Builder {}

impl TjsNs0Builder {
    /// Creates a new instance of `TjsNs0Builder`
    pub fn new() -> Self {
        Self {}
    }
}

impl ScriptBuilder for TjsNs0Builder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Utf16LE
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
        Ok(Box::new(TjsNs0::new(buf, filename, encoding, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["pbd", "tjs"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::KirikiriTjsNs0
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 8 && (buf.starts_with(b"TJS/ns0\0") || buf.starts_with(b"TJS/4s0\0")) {
            return Some(100);
        }
        None
    }

    fn can_create_file(&self) -> bool {
        true
    }

    fn create_file<'a>(
        &'a self,
        filename: &'a str,
        mut writer: Box<dyn WriteSeek + 'a>,
        encoding: Encoding,
        file_encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<()> {
        let s = crate::utils::files::read_file(filename)?;
        let s = decode_to_string(file_encoding, &s, true)?;
        let data: TjsValue = if config.custom_yaml {
            serde_yaml_ng::from_str(&s)?
        } else {
            serde_json::from_str(&s)?
        };
        let header = Header {
            magic: *b"TJS/",
            check: *b"ns0\0",
            seed: u32::from_le_bytes(*b"TJS\0"),
            crypt: 0,
            iv_len: 0,
        };
        let mut checker = ByteChecker::new(header.seed);
        header.pack(&mut writer, false, encoding, &None)?;
        data.pack(&mut checker, &mut writer, false, encoding)?;
        let checksum = checker.final_check();
        writer.write_u32(checksum)?;
        writer.flush()?;
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum TjsValue {
    Void(()),
    Int(i64),
    Double(f64),
    Str(String),
    Array(Vec<TjsValue>),
    Dict(BTreeMap<String, TjsValue>),
}

fn unpack_string<R: Read + Seek>(reader: &mut R, big: bool, encoding: Encoding) -> Result<String> {
    let len = u32::unpack(reader, big, encoding, &None)? as usize;
    let tlen = if encoding.is_utf16le() { len * 2 } else { len };
    let mut buf = vec![0u8; tlen];
    reader.read_exact(&mut buf)?;
    let s = decode_to_string(encoding, &buf, true)?;
    Ok(s)
}

fn pack_string<W: Write>(s: &str, writer: &mut W, big: bool, encoding: Encoding) -> Result<()> {
    let encoded = encode_string(encoding, s, false)?;
    let len = if encoding.is_utf16le() {
        (encoded.len() / 2) as u32
    } else {
        encoded.len() as u32
    };
    len.pack(writer, big, encoding, &None)?;
    writer.write_all(&encoded)?;
    Ok(())
}

impl TjsValue {
    fn pack<W: Write>(
        &self,
        checker: &mut ByteChecker,
        writer: &mut W,
        big: bool,
        encoding: Encoding,
    ) -> Result<()> {
        match self {
            Self::Void(()) => {
                let typ_byte = 0;
                let check_byte = checker.get_seed(typ_byte);
                let typ = ((check_byte as u16) << 8) | (typ_byte as u16);
                typ.pack(writer, big, encoding, &None)?;
            }
            Self::Str(s) => {
                let typ_byte = 2;
                let check_byte = checker.get_seed(typ_byte);
                let typ = ((check_byte as u16) << 8) | (typ_byte as u16);
                typ.pack(writer, big, encoding, &None)?;
                pack_string(s, writer, big, encoding)?;
            }
            Self::Int(i) => {
                let typ_byte = 4;
                let check_byte = checker.get_seed(typ_byte);
                let typ = ((check_byte as u16) << 8) | (typ_byte as u16);
                typ.pack(writer, big, encoding, &None)?;
                i.pack(writer, big, encoding, &None)?;
            }
            Self::Double(f) => {
                let typ_byte = 5;
                let check_byte = checker.get_seed(typ_byte);
                let typ = ((check_byte as u16) << 8) | (typ_byte as u16);
                typ.pack(writer, big, encoding, &None)?;
                f.pack(writer, big, encoding, &None)?;
            }
            Self::Array(arr) => {
                let typ_byte = 0x81;
                let check_byte = checker.get_seed(typ_byte);
                let typ = ((check_byte as u16) << 8) | (typ_byte as u16);
                typ.pack(writer, big, encoding, &None)?;
                let arr_len = arr.len() as u32;
                arr_len.pack(writer, big, encoding, &None)?;
                for item in arr {
                    item.pack(checker, writer, big, encoding)?;
                }
            }
            Self::Dict(dict) => {
                let typ_byte = 0xC1;
                let check_byte = checker.get_seed(typ_byte);
                let typ = ((check_byte as u16) << 8) | (typ_byte as u16);
                typ.pack(writer, big, encoding, &None)?;
                let dict_len = dict.len() as u32;
                dict_len.pack(writer, big, encoding, &None)?;
                for (key, value) in dict {
                    pack_string(key, writer, big, encoding)?;
                    value.pack(checker, writer, big, encoding)?;
                }
            }
        }
        Ok(())
    }

    fn unpack<R: Read + Seek>(
        checker: &mut ByteChecker,
        reader: &mut R,
        big: bool,
        encoding: Encoding,
    ) -> Result<Self> {
        let typ = u16::unpack(reader, big, encoding, &None)?;
        let typ_byte = (typ & 0xff) as u8;
        let check_byte = (typ >> 8) as u8;
        let expected_check = checker.get_seed(typ_byte);
        if check_byte != expected_check {
            return Err(anyhow::anyhow!(
                "TJS/ns0 byte check failed: expected {}, got {} at pos {}",
                expected_check,
                check_byte,
                reader.stream_position()? - 1
            ));
        }
        Ok(match typ_byte {
            0 => TjsValue::Void(()),
            2 => TjsValue::Str(unpack_string(reader, big, encoding)?),
            4 => TjsValue::Int(i64::unpack(reader, big, encoding, &None)?),
            5 => TjsValue::Double(f64::unpack(reader, big, encoding, &None)?),
            0x81 => {
                let arr_len = u32::unpack(reader, big, encoding, &None)? as usize;
                let mut arr = Vec::with_capacity(arr_len);
                for _ in 0..arr_len {
                    arr.push(TjsValue::unpack(checker, reader, big, encoding)?);
                }
                TjsValue::Array(arr)
            }
            0xC1 => {
                let kv_len = u32::unpack(reader, big, encoding, &None)? as usize;
                let mut dict = BTreeMap::new();
                for _ in 0..kv_len {
                    let key = unpack_string(reader, big, encoding)?;
                    let value = TjsValue::unpack(checker, reader, big, encoding)?;
                    dict.insert(key, value);
                }
                TjsValue::Dict(dict)
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Unsupported TJS/ns0 value type: {} at pos {}",
                    typ_byte,
                    reader.stream_position()? - 2
                ));
            }
        })
    }
}

#[derive(Debug)]
/// Kirikiri TJS NS0 Script
pub struct TjsNs0 {
    data: TjsValue,
    custom_yaml: bool,
    header: Header,
}

struct ByteChecker {
    seed: u32,
}

impl ByteChecker {
    pub fn new(seed: u32) -> Self {
        Self { seed }
    }

    fn calculate_round(seed: &mut [u8; 4]) {
        let a = seed[0] ^ wrapping!(seed[0] * 2);
        let mut b = a;
        wrapping! {
        b >>= 2;
        b ^= seed[2];
        b >>= 3;
        b ^= seed[2];
        b ^= a;
        }

        seed[0] = seed[1];
        seed[1] = seed[2];
        seed[2] = b;
    }

    pub fn get_seed(&mut self, type_code: u8) -> u8 {
        let mut s = self.seed.to_le_bytes();
        if type_code == 0 {
            return s[2];
        }
        Self::calculate_round(&mut s);
        self.seed = u32::from_le_bytes(s);
        return s[2];
    }

    pub fn final_check(&mut self) -> u32 {
        let mut s = self.seed.to_le_bytes();
        Self::calculate_round(&mut s);
        Self::calculate_round(&mut s);
        Self::calculate_round(&mut s);
        let tmp = s[0];
        s[0] = s[2];
        s[2] = tmp;
        u32::from_le_bytes(s)
    }
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
struct Header {
    magic: [u8; 4],
    check: [u8; 4],
    seed: u32,
    crypt: u16,
    iv_len: u16,
}

impl TjsNs0 {
    /// Creates a new `TjsNs0` script from the given buffer and filename
    ///
    /// * `buf` - The buffer containing the TJS/ns0 data
    /// * `filename` - The name of the file
    /// * `encoding` - The encoding to use for strings
    /// * `config` - Extra configuration options
    pub fn new(
        buf: Vec<u8>,
        _filename: &str,
        encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<Self> {
        let mut reader = MemReader::new(buf);
        let header = Header::unpack(&mut reader, false, encoding, &None)?;
        if &header.magic != b"TJS/" {
            return Err(anyhow::anyhow!("Not a valid TJS/ns0 file"));
        }
        if header.check[1] != b's' || header.check[2] != b'0' || header.check[3] != 0 {
            return Err(anyhow::anyhow!("Not a valid TJS/ns0 file"));
        }
        if header.crypt != 0 {
            return Err(anyhow::anyhow!("Encrypted TJS/ns0 files are not supported"));
        }
        if header.iv_len != 0 {
            return Err(anyhow::anyhow!("TJS/ns0 files with IV are not supported"));
        }
        let mut reader = match header.check[0] {
            b'n' => reader,
            b'4' => {
                let decompressed = lz4::block::decompress(&reader.data[reader.pos..], None)?;
                MemReader::new(decompressed)
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Unsupported compression method in TJS/ns0 file"
                ));
            }
        };
        let mut checker = ByteChecker::new(header.seed);
        let data = TjsValue::unpack(&mut checker, &mut reader, false, encoding)?;
        let expected_checksum = checker.final_check();
        let actual_checksum = reader.read_u32()?;
        if expected_checksum != actual_checksum {
            return Err(anyhow::anyhow!(
                "TJS/ns0 checksum mismatch: expected {:08X}, got {:08X}",
                expected_checksum,
                actual_checksum
            ));
        }
        Ok(Self {
            data,
            custom_yaml: config.custom_yaml,
            header,
        })
    }
}

impl Script for TjsNs0 {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Custom
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn is_output_supported(&self, output: OutputScriptType) -> bool {
        matches!(output, OutputScriptType::Custom)
    }

    fn custom_output_extension<'a>(&'a self) -> &'a str {
        if self.custom_yaml { "yaml" } else { "json" }
    }

    fn custom_export(&self, filename: &std::path::Path, encoding: Encoding) -> Result<()> {
        let s = if self.custom_yaml {
            serde_yaml_ng::to_string(&self.data)?
        } else {
            serde_json::to_string_pretty(&self.data)?
        };
        let s = encode_string(encoding, &s, false)?;
        let mut writer = crate::utils::files::write_file(filename)?;
        writer.write_all(&s)?;
        Ok(())
    }

    fn custom_import<'a>(
        &'a self,
        custom_filename: &'a str,
        mut file: Box<dyn WriteSeek + 'a>,
        encoding: Encoding,
        output_encoding: Encoding,
    ) -> Result<()> {
        let s = crate::utils::files::read_file(custom_filename)?;
        let s = decode_to_string(output_encoding, &s, true)?;
        let data: TjsValue = if self.custom_yaml {
            serde_yaml_ng::from_str(&s)?
        } else {
            serde_json::from_str(&s)?
        };
        let mut header = self.header.clone();
        header.check = *b"ns0\0";
        let mut checker = ByteChecker::new(header.seed);
        header.pack(&mut file, false, encoding, &None)?;
        data.pack(&mut checker, &mut file, false, encoding)?;
        let checksum = checker.final_check();
        file.write_u32(checksum)?;
        file.flush()?;
        Ok(())
    }
}
