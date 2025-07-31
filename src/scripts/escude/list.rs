use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::{decode_to_string, encode_string};
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
        _archive: Option<&Box<dyn Script>>,
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

    fn can_create_file(&self) -> bool {
        true
    }

    fn create_file<'a>(
        &'a self,
        filename: &'a str,
        writer: Box<dyn WriteSeek + 'a>,
        encoding: Encoding,
        file_encoding: Encoding,
    ) -> Result<()> {
        create_file(filename, writer, encoding, file_encoding)
    }
}

#[derive(Debug)]
pub struct EscudeBinList {
    pub entries: Vec<ListEntry>,
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
            } else if filename == "enum_gfx.bin" {
                for ent in self.entries.iter_mut() {
                    let id = ent.id;
                    if let ListData::Unknown(unk) = &ent.data {
                        let mut reader = MemReader::new(unk.clone());
                        let element_size = if id == 0 {
                            248
                        } else if id == 1 {
                            248
                        } else if id == 2 {
                            248
                        } else if id == 3 {
                            112
                        } else if id == 4 {
                            32
                        } else if id == 9999 {
                            1
                        } else {
                            return Err(anyhow::anyhow!("Unknown enum gfx ID: {}", id));
                        };
                        let len = unk.len();
                        if len % element_size != 0 {
                            return Err(anyhow::anyhow!(
                                "Invalid enum gfx length: {} for ID: {}",
                                len,
                                id
                            ));
                        }
                        let count = len / element_size;
                        let data_entry = match id {
                            0 => ListData::Gfx(EnumGfx::Bgs(
                                reader.read_struct_vec::<BgT>(count, false, encoding)?,
                            )),
                            1 => ListData::Gfx(EnumGfx::Evs(
                                reader.read_struct_vec::<EvT>(count, false, encoding)?,
                            )),
                            2 => ListData::Gfx(EnumGfx::Sts(
                                reader.read_struct_vec::<StT>(count, false, encoding)?,
                            )),
                            3 => ListData::Gfx(EnumGfx::Efxs(
                                reader.read_struct_vec::<EfxT>(count, false, encoding)?,
                            )),
                            4 => ListData::Gfx(EnumGfx::Locs(
                                reader.read_struct_vec::<LocT>(count, false, encoding)?,
                            )),
                            9999 => {
                                // Special case for unknown enum gfx ID
                                ListData::Unknown(unk.clone())
                            }
                            _ => return Err(anyhow::anyhow!("Unknown enum gfx ID: {}", id)),
                        };
                        ent.data = data_entry;
                    }
                }
            } else if filename == "enum_snd.bin" {
                for ent in self.entries.iter_mut() {
                    let id = ent.id;
                    if let ListData::Unknown(unk) = &ent.data {
                        let mut reader = MemReader::new(unk.clone());
                        let element_size = if id == 0 {
                            196
                        } else if id == 1 {
                            128
                        } else if id == 2 {
                            128
                        } else if id == 3 {
                            128
                        } else if id == 9999 {
                            1
                        } else {
                            return Err(anyhow::anyhow!("Unknown enum sound ID: {}", id));
                        };
                        let len = unk.len();
                        if len % element_size != 0 {
                            return Err(anyhow::anyhow!(
                                "Invalid enum sound length: {} for ID: {}",
                                len,
                                id
                            ));
                        }
                        let count = len / element_size;
                        let data_entry = match id {
                            0 => ListData::Snd(EnumSnd::Bgm(
                                reader.read_struct_vec::<BgmT>(count, false, encoding)?,
                            )),
                            1 => ListData::Snd(EnumSnd::Amb(
                                reader.read_struct_vec::<AmbT>(count, false, encoding)?,
                            )),
                            2 => ListData::Snd(EnumSnd::Se(
                                reader.read_struct_vec::<SeT>(count, false, encoding)?,
                            )),
                            3 => ListData::Snd(EnumSnd::Sfx(
                                reader.read_struct_vec::<SfxT>(count, false, encoding)?,
                            )),
                            9999 => {
                                // Special case for unknown enum sound ID
                                ListData::Unknown(unk.clone())
                            }
                            _ => return Err(anyhow::anyhow!("Unknown enum sound ID: {}", id)),
                        };
                        ent.data = data_entry;
                    }
                }
            }
        }
        Ok(())
    }
}

fn create_file<'a>(
    custom_filename: &'a str,
    mut writer: Box<dyn WriteSeek + 'a>,
    encoding: Encoding,
    output_encoding: Encoding,
) -> Result<()> {
    let input = crate::utils::files::read_file(custom_filename)?;
    let s = decode_to_string(output_encoding, &input, true)?;
    let entries: Vec<ListEntry> = serde_json::from_str(&s)
        .map_err(|e| anyhow::anyhow!("Failed to read Escude list from JSON: {}", e))?;
    writer.write_all(b"LIST")?;
    writer.write_u32(0)?; // Placeholder for size
    let mut total_size = 0;
    for entry in entries {
        let cur_pos = writer.stream_position()?;
        writer.write_u32(entry.id)?;
        writer.write_u32(0)?; // Placeholder for size
        entry.data.pack(&mut writer, false, encoding)?;
        let end_pos = writer.stream_position()?;
        let size = (end_pos - cur_pos - 8) as u32; // 8 bytes for id and size
        writer.seek(std::io::SeekFrom::Start(cur_pos + 4))?; // Seek to size position
        writer.write_u32(size)?;
        writer.seek(std::io::SeekFrom::Start(end_pos))?; // Seek to end
        total_size += size + 8;
    }
    writer.seek(std::io::SeekFrom::Start(4))?; // Seek back to size position
    writer.write_u32(total_size)?;
    writer.flush()?;
    Ok(())
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

    fn custom_import<'a>(
        &'a self,
        custom_filename: &'a str,
        writer: Box<dyn WriteSeek + 'a>,
        encoding: Encoding,
        output_encoding: Encoding,
    ) -> Result<()> {
        create_file(custom_filename, writer, encoding, output_encoding)
    }
}

#[derive(Debug, Serialize, Deserialize, StructPack, StructUnpack)]
pub struct ScriptT {
    #[fstring = 64]
    #[fstring_pad = 0x20]
    /// File name
    pub file: String,
    pub source: u32,
    #[fstring = 64]
    #[fstring_pad = 0x20]
    pub title: String,
}

#[derive(Debug, Serialize, Deserialize, StructPack, StructUnpack)]
pub struct NameT {
    #[fstring = 64]
    #[fstring_pad = 0x20]
    /// Name of the character
    pub text: String,
    /// Text color
    pub color: u32,
    #[fstring = 32]
    #[fstring_pad = 0x20]
    /// Face image file name
    pub face: String,
}

#[derive(Debug, Serialize, Deserialize, StructPack, StructUnpack)]
pub struct VarT {
    /// Variable name
    #[fstring = 32]
    #[fstring_pad = 0x20]
    pub name: String,
    /// Variable value
    pub value: u16,
    /// Variable flag
    pub flag: u16,
}

#[derive(Debug, Serialize, Deserialize, StructPack, StructUnpack)]
pub struct SceneT {
    /// The scene script ID
    pub script: u32,
    /// The scene name
    #[fstring = 64]
    #[fstring_pad = 0x20]
    pub name: String,
    /// The scene thumbail image file name
    #[fstring = 32]
    #[fstring_pad = 0x20]
    pub thumbnail: String,
    /// The scene order in the scene (Extra)
    pub order: i32,
}

#[derive(Debug, Serialize, Deserialize, StructPack)]
#[serde(tag = "type", content = "data")]
pub enum EnumScr {
    Scripts(Vec<ScriptT>),
    Names(Vec<NameT>),
    Vars(Vec<VarT>),
    Scenes(Vec<SceneT>),
}

#[derive(Debug, Serialize, Deserialize, StructPack, StructUnpack)]
pub struct BgT {
    /// Background image name
    #[fstring = 32]
    #[fstring_pad = 0x20]
    name: String,
    /// Background image file name
    #[fstring = 64]
    #[fstring_pad = 0x20]
    file: String,
    #[fstring = 128]
    #[fstring_pad = 0x20]
    option: String,
    coverd: u32,
    color: u32,
    id: u32,
    loc: u32,
    order: i32,
    link: u32,
}

#[derive(Debug, Serialize, Deserialize, StructPack, StructUnpack)]
pub struct EvT {
    /// Event image name
    #[fstring = 32]
    #[fstring_pad = 0x20]
    name: String,
    /// Event image file name
    #[fstring = 64]
    #[fstring_pad = 0x20]
    file: String,
    #[fstring = 128]
    #[fstring_pad = 0x20]
    option: String,
    coverd: u32,
    color: u32,
    id: u32,
    loc: u32,
    order: i32,
    link: u32,
}

#[derive(Debug, Serialize, Deserialize, StructPack, StructUnpack)]
pub struct StT {
    #[fstring = 32]
    #[fstring_pad = 0x20]
    name: String,
    #[fstring = 64]
    #[fstring_pad = 0x20]
    file: String,
    #[fstring = 128]
    #[fstring_pad = 0x20]
    option: String,
    coverd: u32,
    color: u32,
    id: u32,
    loc: u32,
    order: i32,
    link: u32,
}

#[derive(Debug, Serialize, Deserialize, StructPack, StructUnpack)]
pub struct EfxT {
    /// Effect image name
    #[fstring = 32]
    #[fstring_pad = 0x20]
    name: String,
    /// Effect image file name
    #[fstring = 64]
    #[fstring_pad = 0x20]
    file: String,
    spot: i32,
    dx: i32,
    dy: i32,
    r#loop: bool,
    #[fvec = 3]
    #[serde(skip, default = "exft_padding")]
    padding: Vec<u8>,
}

fn exft_padding() -> Vec<u8> {
    vec![0; 3]
}

#[derive(Debug, Serialize, Deserialize, StructPack, StructUnpack)]
pub struct Point {
    x: i16,
    y: i16,
}

#[derive(Debug, Serialize, Deserialize, StructPack, StructUnpack)]
pub struct LocT {
    #[fvec = 8]
    pt: Vec<Point>,
}

#[derive(Debug, Serialize, Deserialize, StructPack)]
#[serde(tag = "type", content = "data")]
pub enum EnumGfx {
    Bgs(Vec<BgT>),
    Evs(Vec<EvT>),
    Sts(Vec<StT>),
    Efxs(Vec<EfxT>),
    Locs(Vec<LocT>),
}

#[derive(Debug, Serialize, Deserialize, StructPack, StructUnpack)]
pub struct BgmT {
    #[fstring = 64]
    #[fstring_pad = 0x20]
    pub name: String,
    #[fstring = 64]
    #[fstring_pad = 0x20]
    pub file: String,
    #[fstring = 64]
    #[fstring_pad = 0x20]
    pub title: String,
    pub order: i32,
}

#[derive(Debug, Serialize, Deserialize, StructPack, StructUnpack)]
pub struct AmbT {
    #[fstring = 64]
    #[fstring_pad = 0x20]
    pub name: String,
    #[fstring = 64]
    #[fstring_pad = 0x20]
    pub file: String,
}

#[derive(Debug, Serialize, Deserialize, StructPack, StructUnpack)]
pub struct SeT {
    #[fstring = 64]
    #[fstring_pad = 0x20]
    pub name: String,
    #[fstring = 64]
    #[fstring_pad = 0x20]
    pub file: String,
}

#[derive(Debug, Serialize, Deserialize, StructPack, StructUnpack)]
pub struct SfxT {
    #[fstring = 64]
    #[fstring_pad = 0x20]
    pub name: String,
    #[fstring = 64]
    #[fstring_pad = 0x20]
    pub file: String,
}

#[derive(Debug, Serialize, Deserialize, StructPack)]
#[serde(tag = "type", content = "data")]
pub enum EnumSnd {
    Bgm(Vec<BgmT>),
    Amb(Vec<AmbT>),
    Se(Vec<SeT>),
    Sfx(Vec<SfxT>),
}

#[derive(Debug, Serialize, Deserialize, StructPack)]
#[serde(tag = "type", content = "data")]
pub enum ListData {
    Scr(EnumScr),
    Gfx(EnumGfx),
    Snd(EnumSnd),
    Unknown(Vec<u8>),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListEntry {
    id: u32,
    pub data: ListData,
}
