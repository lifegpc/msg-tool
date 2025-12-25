//! Emote Multiple Image File (.pimg)
use crate::ext::io::*;
use crate::ext::psb::*;
use crate::scripts::base::*;
use crate::try_option;
use crate::types::*;
use crate::utils::img::*;
use anyhow::Result;
use emote_psb::PsbReader;
use libtlg_rs::*;
use std::collections::HashMap;
use std::io::{Read, Seek};
use std::path::Path;

#[derive(Debug)]
/// Emote PImg Script Builder
pub struct PImgBuilder {}

impl PImgBuilder {
    /// Creates a new instance of `PImgBuilder`
    pub const fn new() -> Self {
        Self {}
    }
}

impl ScriptBuilder for PImgBuilder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Utf8
    }

    fn build_script(
        &self,
        buf: Vec<u8>,
        filename: &str,
        _encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(PImg::new(MemReader::new(buf), filename, config)?))
    }

    fn build_script_from_file(
        &self,
        filename: &str,
        _encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        if filename == "-" {
            let data = crate::utils::files::read_file(filename)?;
            Ok(Box::new(PImg::new(MemReader::new(data), filename, config)?))
        } else {
            let f = std::fs::File::open(filename)?;
            let reader = std::io::BufReader::new(f);
            Ok(Box::new(PImg::new(reader, filename, config)?))
        }
    }

    fn build_script_from_reader(
        &self,
        reader: Box<dyn ReadSeek>,
        filename: &str,
        _encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(PImg::new(reader, filename, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["pimg"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::EmotePimg
    }

    fn is_this_format(&self, filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if Path::new(filename)
            .extension()
            .map(|ext| ext.to_ascii_lowercase() == "pimg")
            .unwrap_or(false)
            && buf_len >= 4
            && buf.starts_with(b"PSB\0")
        {
            return Some(255);
        }
        None
    }

    fn is_image(&self) -> bool {
        true
    }
}

#[derive(Debug)]
/// Emote PImg Script
pub struct PImg {
    psb: VirtualPsbFixed,
    overlay: Option<bool>,
}

impl PImg {
    /// Create a new PImg script
    ///
    /// * `reader` - The reader containing the PImg script data
    /// * `filename` - The name of the file
    /// * `config` - Extra configuration options
    pub fn new<R: Read + Seek>(reader: R, _filename: &str, config: &ExtraConfig) -> Result<Self> {
        let psb = PsbReader::open_psb_v2(reader)?.to_psb_fixed();
        Ok(Self {
            psb,
            overlay: config.emote_pimg_overlay,
        })
    }

    fn load_img(&self, layer_id: i64) -> Result<Tlg> {
        let layer_id = layer_id as usize;
        let psb = self.psb.root();
        let reference = &psb[format!("{layer_id}.tlg")];
        let resource_id = reference
            .resource_id()
            .ok_or_else(|| anyhow::anyhow!("Layer {layer_id} does not have a resource ID"))?
            as usize;
        if resource_id >= self.psb.resources().len() {
            return Err(anyhow::anyhow!(
                "Resource ID {resource_id} for layer {layer_id} is out of bounds"
            ));
        }
        let resource = &self.psb.resources()[resource_id];
        Ok(load_tlg(MemReaderRef::new(&resource))?)
    }
}

impl Script for PImg {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Json
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn is_image(&self) -> bool {
        true
    }

    fn is_multi_image(&self) -> bool {
        true
    }

    fn export_multi_image<'a>(
        &'a self,
    ) -> Result<Box<dyn Iterator<Item = Result<ImageDataWithName>> + 'a>> {
        let psb = self.psb.root();
        let overlay = self.overlay.unwrap_or_else(|| {
            psb["layers"]
                .members()
                .all(|layer| layer["group_layer_id"].is_none())
        });
        if !overlay {
            return Ok(Box::new(PImgIter2 {
                pimg: self,
                layers: psb.iter(),
            }));
        }
        let width = psb["width"]
            .as_u32()
            .ok_or(anyhow::anyhow!("missing width"))?;
        let height = psb["height"]
            .as_u32()
            .ok_or(anyhow::anyhow!("missing height"))?;
        if !psb["layers"].is_list() {
            return Err(anyhow::anyhow!("layers is not a list"));
        }
        if psb["layers"].len() == 0 {
            return Ok(Box::new(std::iter::empty()));
        }
        let mut bases = HashMap::new();
        for i in psb["layers"].members() {
            if !i["diff_id"].is_none() {
                continue; // Skip layers with diff_id
            }
            let layer_id = i["layer_id"]
                .as_i64()
                .ok_or(anyhow::anyhow!("missing layer_id"))?;
            let top = i["top"].as_u32().ok_or(anyhow::anyhow!("missing top"))?;
            let left = i["left"].as_u32().ok_or(anyhow::anyhow!("missing left"))?;
            let opacity = i["opacity"]
                .as_u8()
                .ok_or_else(|| anyhow::anyhow!("Layer does not have a valid opacity"))?;
            bases.insert(layer_id, (self.load_img(layer_id)?, top, left, opacity));
        }
        Ok(Box::new(PImgIter {
            pimg: self,
            width,
            height,
            layers: psb["layers"].members(),
            bases,
        }))
    }
}

struct PImgIter<'a> {
    pimg: &'a PImg,
    width: u32,
    height: u32,
    layers: ListIter<'a>,
    bases: HashMap<i64, (Tlg, u32, u32, u8)>,
}

impl<'a> Iterator for PImgIter<'a> {
    type Item = Result<ImageDataWithName>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.layers.next() {
            Some(layer) => {
                let layer_id =
                    try_option!(layer["layer_id"].as_i64().ok_or_else(|| {
                        anyhow::anyhow!("Layer does not have a valid layer_id")
                    }));
                let layer_name = try_option!(
                    layer["name"]
                        .as_str()
                        .ok_or_else(|| { anyhow::anyhow!("Layer does not have a valid name") })
                );
                let width = try_option!(
                    layer["width"]
                        .as_u32()
                        .ok_or_else(|| { anyhow::anyhow!("Layer does not have a valid width") })
                );
                let height = try_option!(
                    layer["height"]
                        .as_u32()
                        .ok_or_else(|| { anyhow::anyhow!("Layer does not have a valid height") })
                );
                let top = try_option!(
                    layer["top"]
                        .as_u32()
                        .ok_or_else(|| { anyhow::anyhow!("Layer does not have a valid top") })
                );
                let left = try_option!(
                    layer["left"]
                        .as_u32()
                        .ok_or_else(|| { anyhow::anyhow!("Layer does not have a valid left") })
                );
                let opacity = try_option!(
                    layer["opacity"]
                        .as_u8()
                        .ok_or_else(|| { anyhow::anyhow!("Layer does not have a valid opacity") })
                );
                if layer["diff_id"].is_none() {
                    let base = &try_option!(self.bases.get(&layer_id).ok_or(anyhow::anyhow!(
                        "Base image for layer_id {} not found",
                        layer_id
                    )))
                    .0;
                    let mut data = ImageData {
                        width: self.width,
                        height: self.height,
                        color_type: match base.color {
                            TlgColorType::Bgr24 => ImageColorType::Bgr,
                            TlgColorType::Bgra32 => ImageColorType::Bgra,
                            TlgColorType::Grayscale8 => ImageColorType::Grayscale,
                        },
                        depth: 8,
                        data: base.data.clone(),
                    };
                    if opacity != 255 {
                        try_option!(apply_opacity(&mut data, opacity));
                    }
                    if self.width != width || self.height != height || top != 0 || left != 0 {
                        data =
                            try_option!(draw_on_canvas(data, self.width, self.height, left, top));
                    }
                    return Some(Ok(ImageDataWithName {
                        name: layer_name.to_string(),
                        data,
                    }));
                } else {
                    let diff_id =
                        try_option!(layer["diff_id"].as_i64().ok_or_else(|| {
                            anyhow::anyhow!("Layer does not have a valid diff_id")
                        }));
                    let (base, base_top, base_left, base_opacity) = try_option!(
                        self.bases
                            .get(&diff_id)
                            .ok_or(anyhow::anyhow!("Base image layer {} not found", diff_id))
                    );
                    let diff = try_option!(self.pimg.load_img(layer_id));
                    if base.color != diff.color {
                        return Some(Err(anyhow::anyhow!(
                            "Color type mismatch for layer_id {}: base color {:?}, diff color {:?}",
                            layer_id,
                            base.color,
                            diff.color
                        )));
                    }
                    let mut base_img = ImageData {
                        width: base.width,
                        height: base.height,
                        color_type: match base.color {
                            TlgColorType::Bgr24 => ImageColorType::Bgr,
                            TlgColorType::Bgra32 => ImageColorType::Bgra,
                            TlgColorType::Grayscale8 => ImageColorType::Grayscale,
                        },
                        depth: 8,
                        data: base.data.clone(),
                    };
                    if base.width != self.width
                        || base.height != self.height
                        || *base_top != 0
                        || *base_left != 0
                    {
                        base_img = try_option!(draw_on_canvas(
                            base_img,
                            self.width,
                            self.height,
                            *base_left,
                            *base_top
                        ));
                    }
                    if *base_opacity != 255 {
                        try_option!(apply_opacity(&mut base_img, *base_opacity));
                    }
                    let diff = ImageData {
                        width: diff.width,
                        height: diff.height,
                        color_type: match diff.color {
                            TlgColorType::Bgr24 => ImageColorType::Bgr,
                            TlgColorType::Bgra32 => ImageColorType::Bgra,
                            TlgColorType::Grayscale8 => ImageColorType::Grayscale,
                        },
                        depth: 8,
                        data: diff.data.clone(),
                    };
                    try_option!(draw_on_img_with_opacity(
                        &mut base_img,
                        &diff,
                        left,
                        top,
                        opacity
                    ));
                    Some(Ok(ImageDataWithName {
                        name: layer_name.to_string(),
                        data: base_img,
                    }))
                }
            }
            None => None,
        }
    }
}

struct PImgIter2<'a> {
    pimg: &'a PImg,
    layers: ObjectIter<'a>,
}

impl<'a> Iterator for PImgIter2<'a> {
    type Item = Result<ImageDataWithName>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.layers.next() {
            Some((k, v)) => {
                if !k.ends_with(".tlg") {
                    return self.next();
                }
                let resource_id = try_option!(
                    v.resource_id()
                        .ok_or_else(|| anyhow::anyhow!("Layer {} does not have a resource ID", k))
                ) as usize;
                let name = k.trim_end_matches(".tlg").to_string();
                if resource_id >= self.pimg.psb.resources().len() {
                    return Some(Err(anyhow::anyhow!(
                        "Resource ID {} for layer {} is out of bounds",
                        resource_id,
                        k
                    )));
                }
                let resource = &self.pimg.psb.resources()[resource_id];
                let tlg = try_option!(load_tlg(MemReaderRef::new(&resource)));
                Some(Ok(ImageDataWithName {
                    name,
                    data: ImageData {
                        width: tlg.width,
                        height: tlg.height,
                        color_type: match tlg.color {
                            TlgColorType::Bgr24 => ImageColorType::Bgr,
                            TlgColorType::Bgra32 => ImageColorType::Bgra,
                            TlgColorType::Grayscale8 => ImageColorType::Grayscale,
                        },
                        depth: 8,
                        data: tlg.data.clone(),
                    },
                }))
            }
            None => None,
        }
    }
}
