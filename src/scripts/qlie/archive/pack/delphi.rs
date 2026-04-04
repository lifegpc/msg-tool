use crate::ext::io::*;
use crate::types::*;
use crate::utils::encoding::*;
use crate::utils::struct_pack::*;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Seek};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", content = "value")]
pub enum DelphiValue {
    Uint8(u8),
    Uint16(u16),
    LongDouble([u8; 10]),
    Unk6,
    String(String),
    Unk8,
    Bool(bool),
    ByteString(Vec<u8>),
    StringArray(Vec<String>),
    UnicodeString(String),
}

impl DelphiValue {
    pub fn as_bytes<'a>(&'a self) -> Option<&'a [u8]> {
        match self {
            DelphiValue::ByteString(b) => Some(b),
            _ => None,
        }
    }
}

impl StructUnpack for DelphiValue {
    fn unpack<R: Read + Seek>(
        reader: &mut R,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn std::any::Any>>,
    ) -> Result<Self> {
        let type_id = u8::unpack(reader, big, encoding, info)?;
        match type_id {
            2 => Ok(DelphiValue::Uint8(u8::unpack(reader, big, encoding, info)?)),
            3 => Ok(DelphiValue::Uint16(u16::unpack(
                reader, big, encoding, info,
            )?)),
            5 => Ok(DelphiValue::LongDouble({
                let mut buf = [0u8; 10];
                reader.read_exact(&mut buf)?;
                buf
            })),
            6 | 7 => Ok(DelphiValue::String({
                let slen = u8::unpack(reader, big, encoding, info)? as usize;
                let buf = reader.read_exact_vec(slen)?;
                decode_to_string(encoding, &buf, true)?
            })),
            8 => Ok(DelphiValue::Bool(false)),
            9 => Ok(DelphiValue::Bool(true)),
            10 => Ok(DelphiValue::ByteString({
                let slen = u32::unpack(reader, big, encoding, info)? as usize;
                reader.read_exact_vec(slen)?
            })),
            11 => Ok(DelphiValue::StringArray({
                let mut arr = Vec::new();
                let mut len;
                while {
                    len = u8::unpack(reader, big, encoding, info)?;
                    len > 0
                } {
                    let buf = reader.read_exact_vec(len as usize)?;
                    arr.push(decode_to_string(encoding, &buf, true)?);
                }
                arr
            })),
            18 => Ok(DelphiValue::UnicodeString({
                let slen = u32::unpack(reader, big, encoding, info)? as usize;
                let buf = reader.read_exact_vec(slen * 2)?;
                decode_to_string(
                    if big {
                        Encoding::Utf16BE
                    } else {
                        Encoding::Utf16LE
                    },
                    &buf,
                    true,
                )?
            })),
            _ => Err(anyhow::anyhow!("Unknown Delphi value type: {}", type_id)),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]

pub struct DelphiObject {
    pub type_name: String,
    pub name: String,
    pub properties: HashMap<String, DelphiValue>,
    pub contents: Vec<DelphiObject>,
}

impl StructUnpack for DelphiObject {
    fn unpack<R: Read + Seek>(
        reader: &mut R,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn std::any::Any>>,
    ) -> Result<Self> {
        let type_len = u8::unpack(reader, big, encoding, info)? as usize;
        let type_name = {
            let buf = reader.read_exact_vec(type_len)?;
            decode_to_string(encoding, &buf, true)?
        };
        let name_len = u8::unpack(reader, big, encoding, info)? as usize;
        let name = {
            let buf = reader.read_exact_vec(name_len)?;
            decode_to_string(encoding, &buf, true)?
        };
        let mut properties = HashMap::new();
        let mut keylen;
        while {
            keylen = u8::unpack(reader, big, encoding, info)?;
            keylen > 0
        } {
            let key_buf = reader.read_exact_vec(keylen as usize)?;
            let key = decode_to_string(encoding, &key_buf, true)?;
            let value = DelphiValue::unpack(reader, big, encoding, info)?;
            properties.insert(key, value);
        }
        let mut contents = Vec::new();
        while reader.peek_u8()? != 0 {
            contents.push(DelphiObject::unpack(reader, big, encoding, info)?);
        }
        reader.read_u8()?; // consume the terminating 0
        return Ok(Self {
            type_name,
            name,
            properties,
            contents,
        });
    }
}

pub fn deser_delphi<R: Read + Seek>(reader: &mut R) -> Result<DelphiObject> {
    let sig = reader.read_u32()?;
    if sig != 0x30465054 {
        return Err(anyhow::anyhow!(
            "Invalid Delphi object signature: {:08X}",
            sig
        ));
    }
    Ok(DelphiObject::unpack(reader, false, Encoding::Cp932, &None)?)
}
