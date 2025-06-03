use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::encode_string;
use crate::utils::struct_pack::*;
use anyhow::Result;
use msg_tool_macro::*;
use serde::{Deserialize, Serialize};
use std::io::{Read, Seek, Write};

#[derive(Debug)]
pub struct EscudeBinListBuilder {}

impl EscudeBinListBuilder {
    pub const fn new() -> Self {
        EscudeBinListBuilder {}
    }
}

impl ScriptBuilder for EscudeBinListBuilder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Cp932
    }

    fn build_script(
        &self,
        data: Vec<u8>,
        filename: &str,
        encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(EscudeBinList::new(
            data, filename, encoding, config,
        )?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["bin"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::EscudeList
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len > 4 && buf.starts_with(b"LIST") {
            return Some(255);
        }
        None
    }
}

#[derive(Debug)]
pub struct EscudeBinList {
    entries: Vec<ListEntry>,
}

impl EscudeBinList {
    pub fn new(
        data: Vec<u8>,
        filename: &str,
        encoding: Encoding,
        _config: &ExtraConfig,
    ) -> Result<Self> {
        let mut reader = MemReader::new(data);
        let mut magic = [0; 4];
        reader.read_exact(&mut magic)?;
        if &magic != b"LIST" {
            return Err(anyhow::anyhow!("Invalid Escude list file format"));
        }
        let wsize = reader.read_u32()?;
        let mut entries = Vec::new();
        loop {
            let current = reader.stream_position()?;
            if current as usize >= wsize as usize + 8 {
                break;
            }
            let id = reader.read_u32()?;
            let size = reader.read_u32()?;
            let data = reader.read_exact_vec(size as usize)?;
            entries.push(ListEntry {
                id: id,
                data: ListData::Unknown(data),
            });
        }
        let mut s = EscudeBinList { entries };
        match s.try_decode(filename, encoding) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("WARN: Failed to decode Escude list: {}", e);
                crate::COUNTER.inc_warning();
            }
        }
        Ok(s)
    }

    pub fn try_decode(&mut self, filename: &str, encoding: Encoding) -> Result<()> {
        let filename = std::path::Path::new(filename);
        if let Some(filename) = filename.file_name() {
            let filename = filename.to_ascii_lowercase();
            if filename == "enum_scr.bin" {
                for ent in self.entries.iter_mut() {
                    let id = ent.id;
                    if let ListData::Unknown(unk) = &ent.data {
                        let mut reader = MemReader::new(unk.clone());
                        let element_size = if id == 0 {
                            132
                        } else if id == 1 {
                            100
                        } else if id == 2 {
                            36
                        } else if id == 3 {
                            104
                        } else if id == 9999 {
                            1
                        } else {
                            return Err(anyhow::anyhow!("Unknown enum source ID: {}", id));
                        };
                        let len = unk.len();
                        if len % element_size != 0 {
                            return Err(anyhow::anyhow!(
                                "Invalid enum source length: {} for ID: {}",
                                len,
                                id
                            ));
                        }
                        let count = len / element_size;
                        let data_entry = match id {
                            0 => ListData::Scr(EnumScr::Scripts(
                                reader.read_struct_vec::<ScriptT>(count, false, encoding)?,
                            )),
                            1 => ListData::Scr(EnumScr::Names(
                                reader.read_struct_vec::<NameT>(count, false, encoding)?,
                            )),
                            2 => ListData::Scr(EnumScr::Vars(
                                reader.read_struct_vec::<VarT>(count, false, encoding)?,
                            )),
                            3 => ListData::Scr(EnumScr::Scenes(
                                reader.read_struct_vec::<SceneT>(count, false, encoding)?,
                            )),
                            9999 => {
                                // Special case for unknown enum source ID
                                ListData::Unknown(unk.clone())
                            }
                            _ => return Err(anyhow::anyhow!("Unknown enum source ID: {}", id)),
                        };
                        ent.data = data_entry;
                    }
                }
            }
        }
        Ok(())
    }
}

impl Script for EscudeBinList {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Custom
    }

    fn is_output_supported(&self, output: OutputScriptType) -> bool {
        matches!(output, OutputScriptType::Custom)
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn custom_output_extension(&self) -> &'static str {
        "json"
    }

    fn custom_export(&self, filename: &std::path::Path, encoding: Encoding) -> Result<()> {
        let s = serde_json::to_string_pretty(&self.entries)
            .map_err(|e| anyhow::anyhow!("Failed to write Escude list to JSON: {}", e))?;
        let mut writer = crate::utils::files::write_file(filename)?;
        let s = encode_string(encoding, &s, false)?;
        writer.write_all(&s)?;
        writer.flush()?;
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize, StructPack, StructUnpack)]
struct ScriptT {
    #[fstring = 64]
    /// File name
    pub file: String,
    pub source: u32,
    #[fstring = 64]
    pub title: String,
}

#[derive(Debug, Serialize, Deserialize, StructPack, StructUnpack)]
struct NameT {
    #[fstring = 64]
    /// Name of the character
    pub text: String,
    /// Text color
    pub color: u32,
    #[fstring = 32]
    /// Face image file name
    pub face: String,
}

#[derive(Debug, Serialize, Deserialize, StructPack, StructUnpack)]
struct VarT {
    /// Variable name
    #[fstring = 32]
    pub name: String,
    /// Variable value
    pub value: u16,
    /// Variable flag
    pub flag: u16,
}

#[derive(Debug, Serialize, Deserialize, StructPack, StructUnpack)]
struct SceneT {
    /// The scene script ID
    pub script: u32,
    /// The scene name
    #[fstring = 64]
    pub name: String,
    /// The scene thumbail image file name
    #[fstring = 32]
    pub thumbnail: String,
    /// The scene order in the scene (Extra)
    pub order: i32,
}

#[derive(Debug, Serialize, Deserialize, StructPack)]
#[serde(tag = "type", content = "data")]
enum EnumScr {
    Scripts(Vec<ScriptT>),
    Names(Vec<NameT>),
    Vars(Vec<VarT>),
    Scenes(Vec<SceneT>),
}

#[derive(Debug, Serialize, Deserialize, StructPack)]
#[serde(tag = "type", content = "data")]
enum ListData {
    Scr(EnumScr),
    Unknown(Vec<u8>),
}

#[derive(Debug, Serialize, Deserialize)]
struct ListEntry {
    id: u32,
    data: ListData,
}
