//! Emote Multiple Image File (.pimg)
use crate::ext::io::*;
use crate::ext::psb::*;
use crate::scripts::base::*;
use crate::try_option;
use crate::types::*;
use crate::utils::img::*;
use crate::utils::psd::*;
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

struct PImgLayer<'a> {
    data: &'a PsbValueFixed,
    name: &'a str,
    layer_id: i64,
    /// seems is layer type in PSD files
    layer_type: i64,
    left: u32,
    top: u32,
    width: u32,
    height: u32,
    opacity: u8,
    visible: bool,
    type_: i64,
    children: Vec<PImgLayer<'a>>,
}

impl std::fmt::Debug for PImgLayer<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PImgLayer")
            .field("layer_id", &self.layer_id)
            .field("layer_type", &self.layer_type)
            .field("name", &self.name)
            .field("left", &self.left)
            .field("top", &self.top)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("opacity", &self.opacity)
            .field("visible", &self.visible)
            .field("type", &self.type_)
            .field("children", &self.children)
            .finish()
    }
}

impl<'a> PImgLayer<'a> {
    pub fn new(data: &'a PsbValueFixed, layers: &'a PsbValueFixed) -> Result<Self> {
        let layer_id = data["layer_id"]
            .as_i64()
            .ok_or_else(|| anyhow::anyhow!("Layer does not have a valid layer_id"))?;
        let layer_type = data["layer_type"]
            .as_i64()
            .ok_or_else(|| anyhow::anyhow!("Layer does not have a valid layer_type"))?;
        let name = data["name"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Layer does not have a valid name"))?;
        let left = data["left"].as_u32();
        let top = data["top"].as_u32();
        let width = data["width"].as_u32();
        let height = data["height"].as_u32();
        let (left, top, width, height) = if layer_type != 0 {
            (
                left.unwrap_or(0),
                top.unwrap_or(0),
                width.unwrap_or(0),
                height.unwrap_or(0),
            )
        } else {
            (
                left.ok_or_else(|| anyhow::anyhow!("Layer does not have a valid left"))?,
                top.ok_or_else(|| anyhow::anyhow!("Layer does not have a valid top"))?,
                width.ok_or_else(|| anyhow::anyhow!("Layer does not have a valid width"))?,
                height.ok_or_else(|| anyhow::anyhow!("Layer does not have a valid height"))?,
            )
        };
        let opacity = data["opacity"]
            .as_u8()
            .ok_or_else(|| anyhow::anyhow!("Layer does not have a valid opacity"))?;
        let visible = data["visible"].as_i64().unwrap_or(1) != 0;
        let type_ = data["type"]
            .as_i64()
            .ok_or_else(|| anyhow::anyhow!("Layer does not have a valid type"))?;
        let mut children = Vec::new();
        for layer in layers.members() {
            if layer_type == 2 || layer_type == 1 {
                if let Some(parent_id) = layer["group_layer_id"].as_i64() {
                    if parent_id == layer_id {
                        children.push(PImgLayer::new(layer, layers)?);
                    }
                }
            } else if layer_type == 0 {
                if let Some(base_id) = layer["diff_id"].as_i64() {
                    if base_id == layer_id {
                        children.push(PImgLayer::new(layer, layers)?);
                    }
                }
            }
        }
        Ok(Self {
            data,
            layer_id,
            layer_type,
            name,
            left,
            top,
            width,
            height,
            opacity,
            visible,
            type_,
            children,
        })
    }

    fn len(&self) -> usize {
        1 + self.children.iter().map(|c| c.len()).sum::<usize>()
    }

    fn load_img(&self, img: &PImg) -> Result<ImageData> {
        if self.layer_type == 2 || self.layer_type == 1 {
            anyhow::bail!("Group layers do not have image data");
        }
        if self.layer_id == -1 {
            // Generate a empty image
            Ok(ImageData {
                width: self.width,
                height: self.height,
                color_type: ImageColorType::Rgba,
                depth: 8,
                data: vec![0u8; (self.width * self.height * 4) as usize],
            })
        } else {
            let tlg = img.load_img(self.layer_id).map_err(|e| {
                anyhow::anyhow!("Failed to load image for layer_id {}: {}", self.layer_id, e)
            })?;
            let mut img = ImageData {
                width: tlg.width,
                height: tlg.height,
                color_type: match tlg.color {
                    TlgColorType::Bgr24 => ImageColorType::Bgr,
                    TlgColorType::Bgra32 => ImageColorType::Bgra,
                    TlgColorType::Grayscale8 => ImageColorType::Grayscale,
                },
                depth: 8,
                data: tlg.data.clone(),
            };
            convert_to_rgba(&mut img)?;
            Ok(img)
        }
    }

    fn save_to_psd(&self, img: &PImg, psd: &mut PsdWriter, base: &mut ImageData) -> Result<()> {
        if self.children.is_empty() {
            let img = self.load_img(img)?;
            let mut visible = self.visible;
            if !self.data["diff_id"].is_none() {
                visible = false; // Diff layers are always hide by default
            }
            if visible {
                draw_on_img_with_opacity(base, &img, self.left, self.top, self.opacity)?;
            }
            let option = PsdLayerOption {
                visible,
                opacity: self.opacity,
            };
            psd.add_layer(self.name, self.left, self.top, img, Some(option))?;
        } else {
            psd.add_layer_group_end()?;
            if self.layer_type == 0 {
                let img = self.load_img(img)?;
                let visible = self.visible;
                if visible {
                    draw_on_img_with_opacity(base, &img, self.left, self.top, self.opacity)?;
                }
                let option = PsdLayerOption {
                    visible,
                    opacity: self.opacity,
                };
                psd.add_layer(self.name, self.left, self.top, img, Some(option))?;
            }
            for child in &self.children {
                child.save_to_psd(img, psd, base)?;
            }
            let option = if self.layer_type == 0 {
                None
            } else {
                Some(PsdLayerOption {
                    visible: self.visible,
                    opacity: self.opacity,
                })
            };
            psd.add_layer_group(self.name, self.layer_type == 2, option)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
struct PImgLayerRoot<'a> {
    layers: Vec<PImgLayer<'a>>,
}

impl<'a> PImgLayerRoot<'a> {
    pub fn new(layers: &'a PsbValueFixed) -> Result<Self> {
        let mut root_layers = Vec::new();
        for layer in layers.members() {
            if layer["group_layer_id"].is_none() && layer["diff_id"].is_none() {
                root_layers.push(PImgLayer::new(layer, layers)?);
            }
        }
        Ok(Self {
            layers: root_layers,
        })
    }

    fn len(&self) -> usize {
        self.layers.iter().map(|l| l.len()).sum()
    }

    fn save_to_psd(&self, img: &PImg, psd: &mut PsdWriter, base: &mut ImageData) -> Result<()> {
        for layer in &self.layers {
            layer.save_to_psd(img, psd, base)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
/// Emote PImg Script
pub struct PImg {
    psb: VirtualPsbFixed,
    overlay: Option<bool>,
    psd: bool,
    psd_compress: bool,
    zlib_compression_level: u32,
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
            psd: config.emote_pimg_psd,
            psd_compress: config.psd_compress,
            zlib_compression_level: config.zlib_compression_level,
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
        OutputScriptType::Custom
    }

    fn is_output_supported(&self, output: OutputScriptType) -> bool {
        matches!(output, OutputScriptType::Custom)
    }

    fn custom_output_extension<'a>(&'a self) -> &'a str {
        "psd"
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn is_image(&self) -> bool {
        !self.psd
    }

    fn is_multi_image(&self) -> bool {
        !self.psd
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

    fn custom_export(&self, filename: &std::path::Path, encoding: Encoding) -> Result<()> {
        let psb = self.psb.root();
        let width = psb["width"]
            .as_u32()
            .ok_or(anyhow::anyhow!("missing width"))?;
        let height = psb["height"]
            .as_u32()
            .ok_or(anyhow::anyhow!("missing height"))?;
        let mut psd = PsdWriter::new(width, height, ImageColorType::Rgba, 8, encoding)?
            .compress(self.psd_compress)
            .zlib_compression_level(self.zlib_compression_level);
        let mut base = ImageData {
            width,
            height,
            color_type: ImageColorType::Rgba,
            depth: 8,
            data: vec![0u8; (width * height * 4) as usize],
        };
        let layers = PImgLayerRoot::new(&psb["layers"])?;
        if layers.len() != psb["layers"].len() {
            return Err(anyhow::anyhow!("Layer hierarchy is invalid"));
        }
        layers.save_to_psd(self, &mut psd, &mut base)?;
        let file = std::fs::File::create(filename)?;
        let mut writer = std::io::BufWriter::new(file);
        psd.save(base, &mut writer)?;
        Ok(())
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
