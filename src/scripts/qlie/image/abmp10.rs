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

    fn can_create_file(&self) -> bool {
        true
    }

    fn create_file<'a>(
        &'a self,
        filename: &'a str,
        writer: Box<dyn WriteSeek + 'a>,
        encoding: Encoding,
        file_encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<()> {
        create_file(filename, writer, encoding, file_encoding, config)
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
    fn write_to<T: Write + Seek>(
        &self,
        data: &mut T,
        encoding: Encoding,
        img: &AbmpImage,
    ) -> Result<()>;
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

    fn write_to<T: Write + Seek>(
        &self,
        data: &mut T,
        encoding: Encoding,
        img: &AbmpImage,
    ) -> Result<()> {
        data.write_fstring(&self.tag, 0x10, encoding, 0, false)?;
        let res = img
            .resources
            .get(self.data.index)
            .ok_or_else(|| anyhow::anyhow!("Resource index {} out of bounds", self.data.index))?;
        data.write_u32(res.len() as u32)?;
        data.write_all(res)?;
        Ok(())
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

    fn write_to<T: Write + Seek>(
        &self,
        data: &mut T,
        encoding: Encoding,
        img: &AbmpImage,
    ) -> Result<()> {
        data.write_fstring("abimage10", 0x10, encoding, 0, false)?;
        data.write_u8(self.datas.len() as u8)?;
        for res in &self.datas {
            res.write_to(data, encoding, img)?;
        }
        Ok(())
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

    fn write_to<T: Write + Seek>(
        &self,
        data: &mut T,
        encoding: Encoding,
        img: &AbmpImage,
    ) -> Result<()> {
        data.write_fstring("absound10", 0x10, encoding, 0, false)?;
        data.write_u8(self.datas.len() as u8)?;
        for res in &self.datas {
            res.write_to(data, encoding, img)?;
        }
        Ok(())
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

    fn write_to<T: Write + Seek>(
        &self,
        data: &mut T,
        encoding: Encoding,
        img: &AbmpImage,
    ) -> Result<()> {
        data.write_fstring("abimgdat15", 0x10, encoding, 0, false)?;
        data.write_u32(self.version)?;
        let name_length = self.name.encode_utf16().count() as u16;
        let name = encode_string(Encoding::Utf16LE, &self.name, true)?;
        if name.len() != (name_length as usize) * 2 {
            anyhow::bail!("Name length mismatch when writing AbImgData15");
        }
        data.write_u16(name_length)?;
        data.write_all(&name)?;
        let internal_name = encode_string(encoding, &self.internal_name, true)?;
        data.write_u16(internal_name.len() as u16)?;
        data.write_all(&internal_name)?;
        data.write_u8(self.typ)?;
        let param_size = if self.version == 2 { 0x1d } else { 0x11 };
        if self.param.len() != param_size {
            anyhow::bail!("Param size mismatch when writing AbImgData15");
        }
        data.write_all(&self.param)?;
        let res = img
            .resources
            .get(self.data.index)
            .ok_or_else(|| anyhow::anyhow!("Resource index {} out of bounds", self.data.index))?;
        data.write_u32(res.len() as u32)?;
        data.write_all(res)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
/// tag: abimgdat14
struct AbImgData14 {
    name: String,
    internal_name: String,
    typ: u8,
    param: Vec<u8>,
    data: ResourceRef,
}

impl AbmpRes for AbImgData14 {
    fn read_from<T: Read + Seek>(
        data: &mut T,
        encoding: Encoding,
        img: &mut AbmpImage,
    ) -> Result<Self>
    where
        Self: Sized,
    {
        let tag = data.read_fstring(0x10, encoding, true)?;
        if tag != "abimgdat14" {
            anyhow::bail!("Invalid AbImgData14 tag: {}", tag);
        }
        let name_length = data.read_u16()? as usize;
        let name = data.read_fstring(name_length, encoding, false)?;
        let internal_name_length = data.read_u16()? as usize;
        let internal_name = data.read_fstring(internal_name_length, encoding, false)?;
        let typ = data.read_u8()?;
        let param = data.read_exact_vec(0x4C)?;
        let size = data.read_u32()?;
        let resource = data.read_exact_vec(size as usize)?;
        img.resources.push(resource);
        let index = img.resources.len() - 1;
        let mut nname = if !name.is_empty() {
            name.clone()
        } else if !internal_name.is_empty() {
            internal_name.clone()
        } else {
            format!("abimage14_{index}")
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
        Ok(AbImgData14 {
            name,
            internal_name,
            typ,
            param,
            data: ResourceRef { index },
        })
    }

    fn write_to<T: Write + Seek>(
        &self,
        data: &mut T,
        encoding: Encoding,
        img: &AbmpImage,
    ) -> Result<()> {
        data.write_fstring("abimgdat14", 0x10, encoding, 0, false)?;
        let name = encode_string(encoding, &self.name, true)?;
        data.write_u16(name.len() as u16)?;
        data.write_all(&name)?;
        let internal_name = encode_string(encoding, &self.internal_name, true)?;
        data.write_u16(internal_name.len() as u16)?;
        data.write_all(&internal_name)?;
        data.write_u8(self.typ)?;
        if self.param.len() != 0x4C {
            anyhow::bail!("Param size mismatch when writing AbImgData14");
        }
        data.write_all(&self.param)?;
        let res = img
            .resources
            .get(self.data.index)
            .ok_or_else(|| anyhow::anyhow!("Resource index {} out of bounds", self.data.index))?;
        data.write_u32(res.len() as u32)?;
        data.write_all(res)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
/// tag: abimgdat13
struct AbImgData13 {
    name: String,
    internal_name: String,
    typ: u8,
    param: Vec<u8>,
    data: ResourceRef,
}

impl AbmpRes for AbImgData13 {
    fn read_from<T: Read + Seek>(
        data: &mut T,
        encoding: Encoding,
        img: &mut AbmpImage,
    ) -> Result<Self>
    where
        Self: Sized,
    {
        let tag = data.read_fstring(0x10, encoding, true)?;
        if tag != "abimgdat13" {
            anyhow::bail!("Invalid AbImgData13 tag: {}", tag);
        }
        let name_length = data.read_u16()? as usize;
        let name = data.read_fstring(name_length, encoding, false)?;
        let internal_name_length = data.read_u16()? as usize;
        let internal_name = data.read_fstring(internal_name_length, encoding, false)?;
        let typ = data.read_u8()?;
        let param = data.read_exact_vec(0xC)?;
        let size = data.read_u32()?;
        let resource = data.read_exact_vec(size as usize)?;
        img.resources.push(resource);
        let index = img.resources.len() - 1;
        let mut nname = if !name.is_empty() {
            name.clone()
        } else if !internal_name.is_empty() {
            internal_name.clone()
        } else {
            format!("abimage13_{index}")
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
        Ok(AbImgData13 {
            name,
            internal_name,
            typ,
            param,
            data: ResourceRef { index },
        })
    }

    fn write_to<T: Write + Seek>(
        &self,
        data: &mut T,
        encoding: Encoding,
        img: &AbmpImage,
    ) -> Result<()> {
        data.write_fstring("abimgdat13", 0x10, encoding, 0, false)?;
        let name = encode_string(encoding, &self.name, true)?;
        data.write_u16(name.len() as u16)?;
        data.write_all(&name)?;
        let internal_name = encode_string(encoding, &self.internal_name, true)?;
        data.write_u16(internal_name.len() as u16)?;
        data.write_all(&internal_name)?;
        data.write_u8(self.typ)?;
        if self.param.len() != 0xC {
            anyhow::bail!("Param size mismatch when writing AbImgData13");
        }
        data.write_all(&self.param)?;
        let res = img
            .resources
            .get(self.data.index)
            .ok_or_else(|| anyhow::anyhow!("Resource index {} out of bounds", self.data.index))?;
        data.write_u32(res.len() as u32)?;
        data.write_all(res)?;
        Ok(())
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

    fn write_to<T: Write + Seek>(
        &self,
        data: &mut T,
        encoding: Encoding,
        img: &AbmpImage,
    ) -> Result<()> {
        data.write_fstring("absnddat12", 0x10, encoding, 0, false)?;
        data.write_u32(self.version)?;
        let name_length = self.name.encode_utf16().count() as u16;
        let name = encode_string(Encoding::Utf16LE, &self.name, true)?;
        if name.len() != (name_length as usize) * 2 {
            anyhow::bail!("Name length mismatch when writing AbSndData12");
        }
        data.write_u16(name_length)?;
        data.write_all(&name)?;
        let internal_name = encode_string(encoding, &self.internal_name, true)?;
        data.write_u16(internal_name.len() as u16)?;
        data.write_all(&internal_name)?;
        let res = img
            .resources
            .get(self.data.index)
            .ok_or_else(|| anyhow::anyhow!("Resource index {} out of bounds", self.data.index))?;
        data.write_u32(res.len() as u32)?;
        data.write_all(res)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
/// tag: absnddat11
struct AbSndData11 {
    name: String,
    internal_name: String,
    data: ResourceRef,
}

impl AbmpRes for AbSndData11 {
    fn read_from<T: Read + Seek>(
        data: &mut T,
        encoding: Encoding,
        img: &mut AbmpImage,
    ) -> Result<Self>
    where
        Self: Sized,
    {
        let tag = data.read_fstring(0x10, encoding, true)?;
        if tag != "absnddat11" {
            anyhow::bail!("Invalid AbSndData11 tag: {}", tag);
        }
        let name_length = data.read_u16()? as usize;
        let name = data.read_fstring(name_length, encoding, false)?;
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
            format!("absnddat11_{index}")
        };
        img.resource_filenames.push(nname);
        Ok(AbSndData11 {
            name,
            internal_name,
            data: ResourceRef { index },
        })
    }

    fn write_to<T: Write + Seek>(
        &self,
        data: &mut T,
        encoding: Encoding,
        img: &AbmpImage,
    ) -> Result<()> {
        data.write_fstring("absnddat11", 0x10, encoding, 0, false)?;
        let name = encode_string(encoding, &self.name, true)?;
        data.write_u16(name.len() as u16)?;
        data.write_all(&name)?;
        let internal_name = encode_string(encoding, &self.internal_name, true)?;
        data.write_u16(internal_name.len() as u16)?;
        data.write_all(&internal_name)?;
        let res = img
            .resources
            .get(self.data.index)
            .ok_or_else(|| anyhow::anyhow!("Resource index {} out of bounds", self.data.index))?;
        data.write_u32(res.len() as u32)?;
        data.write_all(res)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "@type")]
enum AbmpResource {
    Data(AbData),
    Image10(AbImage10),
    ImgData15(AbImgData15),
    ImgData14(AbImgData14),
    ImgData13(AbImgData13),
    Sound10(AbSound10),
    SndData12(AbSndData12),
    SndData11(AbSndData11),
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
            "abimgdat14" => Ok(AbmpResource::ImgData14(AbImgData14::read_from(
                data, encoding, img,
            )?)),
            "abimgdat13" => Ok(AbmpResource::ImgData13(AbImgData13::read_from(
                data, encoding, img,
            )?)),
            "absound10" => Ok(AbmpResource::Sound10(AbSound10::read_from(
                data, encoding, img,
            )?)),
            "absnddat11" => Ok(AbmpResource::SndData11(AbSndData11::read_from(
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

    fn write_to<T: Write + Seek>(
        &self,
        data: &mut T,
        encoding: Encoding,
        img: &AbmpImage,
    ) -> Result<()> {
        match self {
            AbmpResource::Data(res) => res.write_to(data, encoding, img),
            AbmpResource::Image10(res) => res.write_to(data, encoding, img),
            AbmpResource::ImgData15(res) => res.write_to(data, encoding, img),
            AbmpResource::ImgData14(res) => res.write_to(data, encoding, img),
            AbmpResource::ImgData13(res) => res.write_to(data, encoding, img),
            AbmpResource::Sound10(res) => res.write_to(data, encoding, img),
            AbmpResource::SndData12(res) => res.write_to(data, encoding, img),
            AbmpResource::SndData11(res) => res.write_to(data, encoding, img),
        }
    }
}

/// Qlie Abmp10/11/12 image
#[derive(Clone, Debug, Serialize, Deserialize)]
struct AbmpImage {
    /// Valid version: 10, 11, 12
    version: u8,
    datas: Vec<AbmpResource>,
    extra: Vec<u8>,
    #[serde(skip)]
    resources: Vec<Vec<u8>>,
    /// Just used for dump
    #[serde(skip)]
    resource_filenames: Vec<String>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Resource {
    path: String,
    #[serde(skip_serializing_if = "is_false", default)]
    ambp10: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct AbmpImage2 {
    version: u8,
    datas: Vec<AbmpResource>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    extra: Vec<u8>,
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
            extra: Vec::new(),
        };
        let len = reader.stream_length()?;
        let mut pos = reader.stream_position()?;
        while pos < len - 16 {
            let data = AbmpResource::read_from(reader, encoding, &mut img)?;
            img.datas.push(data);
            pos = reader.stream_position()?;
        }
        if pos < len {
            img.extra = reader.read_exact_vec((len - pos) as usize)?;
        }
        Ok(img)
    }

    pub fn dump_to<T: Write + Seek>(&self, mut writer: T, encoding: Encoding) -> Result<()> {
        writer.write_fstring(
            &format!("abmp1{}", (self.version - 10 + b'0') as char),
            16,
            encoding,
            0,
            false,
        )?;
        for data in &self.datas {
            data.write_to(&mut writer, encoding, self)?;
        }
        writer.write_all(&self.extra)?;
        Ok(())
    }

    fn to_image2(&self) -> AbmpImage2 {
        AbmpImage2 {
            version: self.version,
            datas: self.datas.clone(),
            resources: Vec::new(),
            extra: self.extra.clone(),
        }
    }

    fn from_image2(img: &AbmpImage2) -> Self {
        AbmpImage {
            version: img.version,
            datas: img.datas.clone(),
            resources: Vec::new(),
            resource_filenames: Vec::new(),
            extra: img.extra.clone(),
        }
    }
}

#[derive(Debug)]
pub struct Abmp10Image {
    img: AbmpImage,
    encoding: Encoding,
    config: ExtraConfig,
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
            encoding,
            config: config.clone(),
        })
    }

    fn output_resource(
        &self,
        folder_path: &std::path::PathBuf,
        path: String,
        data: &[u8],
        encoding: Encoding,
    ) -> Result<Resource> {
        let mut res = Resource {
            path,
            ambp10: false,
        };
        if self.config.qlie_abmp10_process_abmp10
            && data.len() > 6
            && data.starts_with(b"abmp1")
            && data[5] >= b'0'
            && data[5] <= b'2'
        {
            res.ambp10 = true;
            let another = Abmp10Image::new(MemReaderRef::new(data), self.encoding, &self.config)?;
            let mut np = std::path::PathBuf::from(&res.path);
            np.set_extension(another.custom_output_extension());
            res.path = np.to_string_lossy().to_string();
            let path = folder_path.join(&res.path);
            make_sure_dir_exists(&path)?;
            another.custom_export(&path, encoding)?;
        } else {
            let path = folder_path.join(&res.path);
            make_sure_dir_exists(&path)?;
            std::fs::write(&path, data)?;
        }
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
        if self.config.custom_yaml {
            "yaml"
        } else {
            "json"
        }
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
            let res = self.output_resource(&base_path, res_name, res, encoding)?;
            img.resources.push(res);
        }
        let s = if self.config.custom_yaml {
            serde_yaml_ng::to_string(&img)?
        } else {
            serde_json::to_string_pretty(&img)?
        };
        let s = encode_string(encoding, &s, false)?;
        file.write_all(&s)?;
        Ok(())
    }

    fn custom_import<'a>(
        &'a self,
        custom_filename: &'a str,
        file: Box<dyn WriteSeek + 'a>,
        encoding: Encoding,
        output_encoding: Encoding,
    ) -> Result<()> {
        create_file(
            custom_filename,
            file,
            encoding,
            output_encoding,
            &self.config,
        )
    }
}

fn create_file<'a>(
    filename: &str,
    mut writer: Box<dyn WriteSeek + 'a>,
    encoding: Encoding,
    file_encoding: Encoding,
    config: &ExtraConfig,
) -> Result<()> {
    let data = crate::utils::files::read_file(filename)?;
    let s = decode_to_string(file_encoding, &data, true)?;
    let img2: AbmpImage2 = if config.custom_yaml {
        serde_yaml_ng::from_str(&s)?
    } else {
        serde_json::from_str(&s)?
    };
    let mut img = AbmpImage::from_image2(&img2);
    let mut base_path = std::path::PathBuf::from(filename);
    base_path.set_extension("");
    for res in &img2.resources {
        let path = base_path.join(&res.path);
        let buf = if res.ambp10 {
            let mut mem = MemWriter::new();
            create_file(
                &path.to_string_lossy(),
                Box::new(&mut mem),
                encoding,
                file_encoding,
                config,
            )?;
            mem.into_inner()
        } else {
            crate::utils::files::read_file(&path)?
        };
        img.resources.push(buf);
    }
    img.dump_to(&mut writer, encoding)?;
    Ok(())
}
