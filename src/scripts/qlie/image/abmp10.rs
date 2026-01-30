//! Qlie Abmp10/11/12 image (.b)
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use crate::utils::files::*;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::io::{Read, Seek, Write};

#[derive(Debug)]
/// Qlie Abmp10/11/12 image builder
pub struct Abmp10ImageBuilder {}

impl Abmp10ImageBuilder {
    pub fn new() -> Self {
        Self {}
    }
}

impl ScriptBuilder for Abmp10ImageBuilder {
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
        Ok(Box::new(Abmp10Image::new(
            MemReader::new(buf),
            encoding,
            config,
        )?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["b"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::QlieAbmp10
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 6 && buf.starts_with(b"abmp1") {
            let v = buf[5];
            if v >= b'0' && v <= b'2' {
                return Some(25);
            }
        }
        None
    }
}

trait AbmpRes {
    fn read_from<T: Read + Seek>(
        data: &mut T,
        encoding: Encoding,
        img: &mut AbmpImage,
    ) -> Result<Self>
    where
        Self: Sized;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ResourceRef {
    index: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct AbData {
    /// abdataxx xx = version
    tag: String,
    data: ResourceRef,
}

impl AbmpRes for AbData {
    fn read_from<T: Read + Seek>(
        data: &mut T,
        encoding: Encoding,
        img: &mut AbmpImage,
    ) -> Result<Self>
    where
        Self: Sized,
    {
        let tag = data.read_fstring(0x10, encoding, true)?;
        if !tag.starts_with("abdata") {
            anyhow::bail!("Invalid AbData tag: {}", tag);
        }
        let size = data.read_u32()?;
        let resource = data.read_exact_vec(size as usize)?;
        img.resources.push(resource);
        let index = img.resources.len() - 1;
        img.resource_filenames.push(format!("{tag}_{index}"));
        Ok(AbData {
            tag,
            data: ResourceRef { index },
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
/// tag: abimage10
struct AbImage10 {
    datas: Vec<AbmpResource>,
}

impl AbmpRes for AbImage10 {
    fn read_from<T: Read + Seek>(
        data: &mut T,
        encoding: Encoding,
        img: &mut AbmpImage,
    ) -> Result<Self>
    where
        Self: Sized,
    {
        let tag = data.read_fstring(0x10, encoding, true)?;
        if tag != "abimage10" {
            anyhow::bail!("Invalid AbImage10 tag: {}", tag);
        }
        let mut datas = Vec::new();
        let count = data.read_u8()?;
        for _ in 0..count {
            let data = AbmpResource::read_from(data, encoding, img)?;
            datas.push(data);
        }
        Ok(AbImage10 { datas })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
/// tag: absound10
struct AbSound10 {
    datas: Vec<AbmpResource>,
}

impl AbmpRes for AbSound10 {
    fn read_from<T: Read + Seek>(
        data: &mut T,
        encoding: Encoding,
        img: &mut AbmpImage,
    ) -> Result<Self>
    where
        Self: Sized,
    {
        let tag = data.read_fstring(0x10, encoding, true)?;
        if tag != "absound10" {
            anyhow::bail!("Invalid AbSound10 tag: {}", tag);
        }
        let mut datas = Vec::new();
        let count = data.read_u8()?;
        for _ in 0..count {
            let data = AbmpResource::read_from(data, encoding, img)?;
            datas.push(data);
        }
        Ok(AbSound10 { datas })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
/// tag: abimgdat15
struct AbImgData15 {
    version: u32,
    name: String,
    internal_name: String,
    typ: u8,
    param: Vec<u8>,
    data: ResourceRef,
}

impl AbmpRes for AbImgData15 {
    fn read_from<T: Read + Seek>(
        data: &mut T,
        encoding: Encoding,
        img: &mut AbmpImage,
    ) -> Result<Self>
    where
        Self: Sized,
    {
        let tag = data.read_fstring(0x10, encoding, true)?;
        if tag != "abimgdat15" {
            anyhow::bail!("Invalid AbImgData15 tag: {}", tag);
        }
        let version = data.read_u32()?;
        let name_length = data.read_u16()? as usize * 2;
        let name = data.read_fstring(name_length, Encoding::Utf16LE, false)?;
        let internal_name_length = data.read_u16()? as usize;
        let internal_name = data.read_fstring(internal_name_length, encoding, false)?;
        let typ = data.read_u8()?;
        let param_size = if version == 2 { 0x1d } else { 0x11 };
        let param = data.read_exact_vec(param_size)?;
        let size = data.read_u32()?;
        let resource = data.read_exact_vec(size as usize)?;
        img.resources.push(resource);
        let index = img.resources.len() - 1;
        let mut nname = if !name.is_empty() {
            name.clone()
        } else if !internal_name.is_empty() {
            internal_name.clone()
        } else {
            format!("abimage15_{index}")
        };
        match typ {
            0 => nname.push_str(".bmp"),
            1 => nname.push_str(".jpg"),
            3 => nname.push_str(".png"),
            4 => nname.push_str(".m"),
            5 => nname.push_str(".argb"),
            6 => nname.push_str(".b"),
            7 => nname.push_str(".ogv"),
            8 => nname.push_str(".mdl"),
            _ => {}
        }
        img.resource_filenames.push(nname);
        Ok(AbImgData15 {
            version,
            name,
            internal_name,
            typ,
            param,
            data: ResourceRef { index },
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
/// tag: absnddat12
struct AbSndData12 {
    version: u32,
    name: String,
    internal_name: String,
    data: ResourceRef,
}

impl AbmpRes for AbSndData12 {
    fn read_from<T: Read + Seek>(
        data: &mut T,
        encoding: Encoding,
        img: &mut AbmpImage,
    ) -> Result<Self>
    where
        Self: Sized,
    {
        let tag = data.read_fstring(0x10, encoding, true)?;
        if tag != "absnddat12" {
            anyhow::bail!("Invalid AbSndData12 tag: {}", tag);
        }
        let version = data.read_u32()?;
        let name_length = data.read_u16()? as usize * 2;
        let name = data.read_fstring(name_length, Encoding::Utf16LE, false)?;
        let internal_name_length = data.read_u16()? as usize;
        let internal_name = data.read_fstring(internal_name_length, encoding, false)?;
        let size = data.read_u32()?;
        let resource = data.read_exact_vec(size as usize)?;
        img.resources.push(resource);
        let index = img.resources.len() - 1;
        let nname = if !name.is_empty() {
            name.clone()
        } else if !internal_name.is_empty() {
            internal_name.clone()
        } else {
            format!("absnddat12_{index}")
        };
        img.resource_filenames.push(nname);
        Ok(AbSndData12 {
            version,
            name,
            internal_name,
            data: ResourceRef { index },
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "@type")]
enum AbmpResource {
    Data(AbData),
    Image10(AbImage10),
    ImgData15(AbImgData15),
    Sound10(AbSound10),
    SndData12(AbSndData12),
}

impl AbmpRes for AbmpResource {
    fn read_from<T: Read + Seek>(
        data: &mut T,
        encoding: Encoding,
        img: &mut AbmpImage,
    ) -> Result<Self> {
        let tag = data.peek_fstring(0x10, encoding, true)?;
        if tag.starts_with("abdata") {
            return Ok(AbmpResource::Data(AbData::read_from(data, encoding, img)?));
        }
        match tag.as_str() {
            "abimage10" => Ok(AbmpResource::Image10(AbImage10::read_from(
                data, encoding, img,
            )?)),
            "abimgdat15" => Ok(AbmpResource::ImgData15(AbImgData15::read_from(
                data, encoding, img,
            )?)),
            "absound10" => Ok(AbmpResource::Sound10(AbSound10::read_from(
                data, encoding, img,
            )?)),
            "absnddat12" => Ok(AbmpResource::SndData12(AbSndData12::read_from(
                data, encoding, img,
            )?)),
            _ => {
                anyhow::bail!("Unknown Abmp resource tag: {}", tag);
            }
        }
    }
}

/// Qlie Abmp10/11/12 image
#[derive(Clone, Debug, Serialize, Deserialize)]
struct AbmpImage {
    /// Valid version: 10, 11, 12
    version: u8,
    datas: Vec<AbmpResource>,
    #[serde(skip)]
    resources: Vec<Vec<u8>>,
    /// Just used for dump
    #[serde(skip)]
    resource_filenames: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Resource {
    path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct AbmpImage2 {
    version: u8,
    datas: Vec<AbmpResource>,
    resources: Vec<Resource>,
}

impl AbmpImage {
    pub fn new_from<T: Read + Seek>(reader: &mut T, encoding: Encoding) -> Result<Self> {
        let magic = reader.read_fstring(16, encoding, true)?;
        if !magic.starts_with("abmp1") {
            anyhow::bail!("Not a valid Abmp image");
        }
        let version = magic.as_bytes()[5] - b'0' + 10;
        let mut img = AbmpImage {
            version,
            datas: Vec::new(),
            resources: Vec::new(),
            resource_filenames: Vec::new(),
        };
        let len = reader.stream_length()?;
        while reader.stream_position()? < len {
            let data = AbmpResource::read_from(reader, encoding, &mut img)?;
            img.datas.push(data);
        }
        Ok(img)
    }

    fn to_image2(&self) -> AbmpImage2 {
        AbmpImage2 {
            version: self.version,
            datas: self.datas.clone(),
            resources: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct Abmp10Image {
    img: AbmpImage,
    custom_yaml: bool,
}

impl Abmp10Image {
    pub fn new<T: Read + Seek>(
        mut data: T,
        encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<Self> {
        let img = AbmpImage::new_from(&mut data, encoding)?;
        Ok(Abmp10Image {
            img,
            custom_yaml: config.custom_yaml,
        })
    }

    fn output_resource(
        &self,
        folder_path: &std::path::PathBuf,
        path: String,
        data: &[u8],
    ) -> Result<Resource> {
        let res = Resource { path };
        let path = folder_path.join(&res.path);
        make_sure_dir_exists(&path)?;
        std::fs::write(&path, data)?;
        Ok(res)
    }
}

impl Script for Abmp10Image {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Custom
    }

    fn is_output_supported(&self, output: OutputScriptType) -> bool {
        matches!(output, OutputScriptType::Custom)
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn custom_output_extension<'a>(&'a self) -> &'a str {
        if self.custom_yaml { "yaml" } else { "json" }
    }

    fn custom_export(&self, filename: &std::path::Path, encoding: Encoding) -> Result<()> {
        let file = std::fs::File::create(filename)?;
        let mut file = std::io::BufWriter::new(file);
        let mut img = self.img.to_image2();
        let mut base_path = filename.to_path_buf();
        base_path.set_extension("");
        for (res, res_name) in self
            .img
            .resources
            .iter()
            .zip(self.img.resource_filenames.iter())
        {
            let res_name = sanitize_path(res_name);
            let res = self.output_resource(&base_path, res_name, res)?;
            img.resources.push(res);
        }
        let s = if self.custom_yaml {
            serde_yaml_ng::to_string(&img)?
        } else {
            serde_json::to_string_pretty(&img)?
        };
        let s = encode_string(encoding, &s, false)?;
        file.write_all(&s)?;
        Ok(())
    }
}
