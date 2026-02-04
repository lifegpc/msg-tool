//! A simple PSD reader/writer
mod compression;
mod types;

use crate::ext::io::*;
use crate::types::*;
use crate::utils::bit_stream::*;
use crate::utils::encoding::*;
use crate::utils::img::*;
use crate::utils::struct_pack::*;
use anyhow::Result;
use compression::*;
use std::collections::BTreeMap;
use std::io::{Read, Seek, Write};
use std::ops::Deref;
use types::*;

pub use types::{
    AdditionalLayerInfo, IMAGE_RESOURCE_SIGNATURE, LAYER_ID_KEY, LAYER_NAME_SOURCE_SETTING_KEY,
    LayerID, LayerNameSourceSetting,
};

#[derive(Debug, Clone, msg_tool_macro::Default)]
pub struct PsdLayerOption {
    #[default(true)]
    /// Whether the layer is visible.
    pub visible: bool,
    #[default(255)]
    /// The opacity of the layer (0-255).
    pub opacity: u8,
    /// Additional layer information.
    pub additional_info: Vec<AdditionalLayerInfo>,
}

impl PsdLayerOption {
    fn to_flags(&self) -> u8 {
        let mut flags = 0u8;
        if !self.visible {
            flags |= 0b0000_0010;
        }
        flags
    }
}

/// A simple PSD writer.
pub struct PsdWriter {
    psd: PsdFile,
    color_type: ImageColorType,
    compress: bool,
    zlib_compression_level: u32,
    encoding: Encoding,
}

fn encode_unicode_layer(name: &str) -> Result<AdditionalLayerInfo> {
    let layer = UnicodeLayer {
        name: UnicodeString(name.to_string()),
    };
    let mut data = MemWriter::new();
    layer.pack(&mut data, true, Encoding::Utf16BE, &None)?;
    Ok(AdditionalLayerInfo {
        signature: *IMAGE_RESOURCE_SIGNATURE,
        key: *UNICODE_LAYER_KEY,
        data: data.into_inner(),
    })
}

impl PsdWriter {
    /// Creates a new PSD writer with the specified dimensions, color type, and bit depth.
    pub fn new(
        width: u32,
        height: u32,
        color_type: ImageColorType,
        depth: u8,
        encoding: Encoding,
    ) -> Result<Self> {
        let color_type = match color_type {
            ImageColorType::Bgr => ImageColorType::Rgb,
            ImageColorType::Bgra => ImageColorType::Rgba,
            _ => color_type,
        };
        let depth = match depth {
            1 | 8 | 16 | 32 => depth,
            _ => anyhow::bail!("Unsupported bit depth: {}", depth),
        };
        let psd = PsdFile {
            header: PsdHeader {
                signature: *PSD_SIGNATURE,
                version: 1,
                reserved: [0; 6],
                channels: color_type.bpp(1),
                height,
                width,
                depth: depth as u16,
                color_mode: if color_type == ImageColorType::Grayscale {
                    1
                } else {
                    3
                },
            },
            color_mode_data: ColorModeData { data: vec![] },
            image_resource: ImageResourceSection { resources: vec![] },
            layer_and_mask_info: LayerAndMaskInfo {
                layer_info: LayerInfo {
                    layer_count: 0,
                    layer_records: vec![],
                    channel_image_data: vec![],
                },
                global_layer_mask_info: Some(GlobalLayerMaskInfo {
                    overlays_color_space: 0,
                    overlays_color_components: [0; 4],
                    opacity: 0,
                    kind: 128,
                    filler: vec![0],
                }),
                tagged_blocks: vec![],
            },
            image_data: ImageDataSection {
                compression: 0,
                image_data: vec![],
            },
        };
        Ok(Self {
            psd,
            color_type,
            compress: true,
            zlib_compression_level: 6,
            encoding,
        })
    }

    /// Sets whether to compress image data in the PSD file.
    pub fn compress(mut self, compress: bool) -> Self {
        self.compress = compress;
        self
    }

    /// Sets the zlib compression level for the PSD file.
    pub fn zlib_compression_level(mut self, level: u32) -> Self {
        self.zlib_compression_level = level;
        self
    }

    /// Add a layer to the PSD file.
    ///
    /// * `name` - The name of the layer.
    /// * `x` - The x position of the layer.
    /// * `y` - The y position of the layer.
    /// * `data` - The image data of the layer.
    /// * `option` - The options for the layer.
    pub fn add_layer(
        &mut self,
        name: &str,
        x: u32,
        y: u32,
        mut data: ImageData,
        option: Option<PsdLayerOption>,
    ) -> Result<()> {
        if data.color_type == ImageColorType::Bgr {
            convert_bgr_to_rgb(&mut data)?;
        }
        if data.color_type == ImageColorType::Bgra {
            convert_bgra_to_rgba(&mut data)?;
        }
        let length = data.width as u32 * data.height as u32;
        let mut channel_ids = Vec::new();
        if data.color_type == ImageColorType::Grayscale {
            channel_ids.push(0);
        } else {
            channel_ids.push(0); // R
            channel_ids.push(1); // G
            channel_ids.push(2); // B
            if data.color_type == ImageColorType::Rgba {
                channel_ids.push(-1); // Alpha
            }
        }
        let flags = if let Some(opt) = &option {
            opt.to_flags()
        } else {
            0
        };
        let opacity = if let Some(opt) = &option {
            opt.opacity
        } else {
            255
        };
        let mut layer_base = LayerRecordBase {
            top: y as i32,
            left: x as i32,
            bottom: (y + data.height) as i32,
            right: (x + data.width) as i32,
            channels: data.color_type.bpp(1) as u16,
            channel_infos: Vec::new(),
            blend_mode_signature: *IMAGE_RESOURCE_SIGNATURE,
            blend_mode_key: *b"norm",
            opacity,
            clipping: 0,
            flags,
            filler: 0,
        };
        let mut channel_ranges = Vec::new();
        for _ in 0..layer_base.channels {
            channel_ranges.push(ChannelRange {
                source_range: 0xFFFF,
                dest_range: 0xFFFF,
            });
        }
        let layer_blending_ranges = LayerBlendingRanges {
            gray_blend_dest: 0xFFFF,
            gray_blend_source: 0xFFFF,
            channel_ranges,
        };
        let mut image_data = Vec::new();
        for i in 0..layer_base.channels {
            let mut d = Vec::with_capacity(length as usize);
            for y in 0..data.height {
                for x in 0..data.width {
                    let index =
                        (y * data.width + x) as usize * layer_base.channels as usize + i as usize;
                    d.push(data.data[index]);
                }
            }
            if self.compress {
                for y in 0..data.height {
                    let ind = y as usize * data.width as usize;
                    let mut pre = d[ind];
                    for x in 1..data.width as usize {
                        let cur = d[ind + x];
                        d[ind + x] = cur.wrapping_sub(pre);
                        pre = cur;
                    }
                }
                let mut data = Vec::new();
                let mut enc = flate2::write::ZlibEncoder::new(
                    &mut data,
                    flate2::Compression::new(self.zlib_compression_level),
                );
                enc.write_all(&d)?;
                enc.finish()?;
                d = data;
            }
            let cinfo = ChannelInfo {
                channel_id: channel_ids[i as usize],
                length: d.len() as u32 + 2, // +2 for compression method
            };
            layer_base.channel_infos.push(cinfo);
            let compression = if self.compress { 3 } else { 0 };
            image_data.push(ChannelImageData {
                compression,
                image_data: d,
            });
        }
        let encoded = encode_string(self.encoding, &name, false)?;
        let mut infos = vec![encode_unicode_layer(name)?];
        if let Some(opt) = option {
            infos.extend(opt.additional_info);
        }
        let layer = LayerRecord {
            base: layer_base,
            layer_mask: None,
            layer_blending_ranges,
            layer_name: PascalString4(encoded),
            infos,
        };
        self.psd
            .layer_and_mask_info
            .layer_info
            .layer_records
            .push(layer);

        // Update layer count
        self.psd.layer_and_mask_info.layer_info.layer_count += 1;

        self.psd
            .layer_and_mask_info
            .layer_info
            .channel_image_data
            .extend(image_data);
        Ok(())
    }

    /// Adds the start of a layer group to the PSD file.
    pub fn add_layer_group(
        &mut self,
        name: &str,
        is_closed: bool,
        option: Option<PsdLayerOption>,
    ) -> Result<()> {
        let type_info = SectionDividerSetting {
            typ: if is_closed { 2 } else { 1 },
        };
        let mut data = MemWriter::new();
        type_info.pack(&mut data, true, self.encoding, &None)?;
        let encoded = encode_string(self.encoding, &name, false)?;
        let flags = if let Some(opt) = &option {
            opt.to_flags()
        } else {
            0
        };
        let opacity = if let Some(opt) = &option {
            opt.opacity
        } else {
            255
        };
        let mut infos = vec![
            AdditionalLayerInfo {
                signature: *IMAGE_RESOURCE_SIGNATURE,
                key: *SECTION_DIVIDER_SETTING_KEY,
                data: data.into_inner(),
            },
            encode_unicode_layer(name)?,
        ];
        if let Some(opt) = option {
            infos.extend(opt.additional_info);
        }
        let layer = LayerRecord {
            base: LayerRecordBase {
                top: 0,
                left: 0,
                bottom: 0,
                right: 0,
                channels: 0,
                channel_infos: vec![],
                blend_mode_signature: *IMAGE_RESOURCE_SIGNATURE,
                blend_mode_key: *b"pass",
                opacity,
                clipping: 0,
                flags,
                filler: 0,
            },
            layer_mask: None,
            layer_blending_ranges: LayerBlendingRanges {
                gray_blend_dest: 0xFFFF,
                gray_blend_source: 0xFFFF,
                channel_ranges: vec![],
            },
            layer_name: PascalString4(encoded),
            infos,
        };
        self.psd
            .layer_and_mask_info
            .layer_info
            .layer_records
            .push(layer);
        self.psd.layer_and_mask_info.layer_info.layer_count += 1;
        Ok(())
    }

    /// Adds the end of a layer group to the PSD file.
    pub fn add_layer_group_end(&mut self) -> Result<()> {
        let type_info = SectionDividerSetting { typ: 3 };
        let mut data = MemWriter::new();
        type_info.pack(&mut data, true, self.encoding, &None)?;
        let layer = LayerRecord {
            base: LayerRecordBase {
                top: 0,
                left: 0,
                bottom: 0,
                right: 0,
                channels: 0,
                channel_infos: vec![],
                blend_mode_signature: *IMAGE_RESOURCE_SIGNATURE,
                blend_mode_key: *b"norm",
                opacity: 255,
                clipping: 0,
                flags: 0,
                filler: 0,
            },
            layer_mask: None,
            layer_blending_ranges: LayerBlendingRanges {
                gray_blend_dest: 0xFFFF,
                gray_blend_source: 0xFFFF,
                channel_ranges: vec![],
            },
            layer_name: PascalString4(b"</Layer group>".to_vec()),
            infos: vec![AdditionalLayerInfo {
                signature: *IMAGE_RESOURCE_SIGNATURE,
                key: *SECTION_DIVIDER_SETTING_KEY,
                data: data.into_inner(),
            }],
        };
        self.psd
            .layer_and_mask_info
            .layer_info
            .layer_records
            .push(layer);
        self.psd.layer_and_mask_info.layer_info.layer_count += 1;
        Ok(())
    }

    /// Saves the PSD file to the specified writer with the given encoding.
    ///
    /// * `data` - The final composite image data to be saved in the PSD file.
    pub fn save<T: Write>(&mut self, data: ImageData, mut writer: T) -> Result<()> {
        if data.color_type == ImageColorType::Bgr {
            convert_bgr_to_rgb(&mut data.clone())?;
        }
        if data.color_type == ImageColorType::Bgra {
            convert_bgra_to_rgba(&mut data.clone())?;
        }
        if self.color_type != data.color_type {
            anyhow::bail!(
                "Image color type does not match PSD color type: {:?} != {:?}",
                self.color_type,
                data.color_type
            );
        }
        if data.width != self.psd.header.width || data.height != self.psd.header.height {
            anyhow::bail!(
                "Image dimensions do not match PSD dimensions: {}x{} != {}x{}",
                data.width,
                data.height,
                self.psd.header.width,
                self.psd.header.height
            );
        }

        // Convert interleaved data to planar data (RRR...GGG...BBB...)
        let channels = self.psd.header.channels as usize;
        let width = self.psd.header.width as usize;
        let height = self.psd.header.height as usize;
        let total_pixels = width * height;
        let expected_len = total_pixels * channels; // assuming 8-bit depth

        if data.data.len() != expected_len {
            anyhow::bail!("Data length mismatch for planar conversion");
        }

        let mut planar_data = Vec::with_capacity(expected_len);
        for c in 0..channels {
            for i in 0..total_pixels {
                // Interleaved index: pixel_index * channels + channel_index
                let val = data.data[i * channels + c];
                planar_data.push(val);
            }
        }
        if self.compress {
            // RLE compression for planar data
            let mut compressed = MemWriter::new();
            // reserve 2 bytes per scanline for lengths
            for _ in 0..(channels * height) {
                compressed.write_u16_be(0)?; // placeholder for lengths
            }
            for c in 0..channels {
                for y in 0..height {
                    let start = (c * width * height) + (y * width);
                    let line_end = start + width;
                    let out_line = rle_compress(&planar_data[start..line_end]);

                    // write scanline length at reserved spot and append data
                    compressed
                        .write_u16_be_at(((c * height + y) * 2) as u64, out_line.len() as u16)?;
                    compressed.write_all(&out_line)?;
                }
            }
            planar_data = compressed.into_inner();
        }
        let compression = if self.compress { 1 } else { 0 };
        self.psd.image_data.image_data = planar_data;
        self.psd.image_data.compression = compression;
        self.psd.pack(&mut writer, true, self.encoding, &None)?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct PsdReader {
    psd: PsdFile,
    encoding: Encoding,
    channel_start_indices: Vec<usize>,
}

#[derive(Debug)]
pub struct NormalLayer<'a> {
    layer: &'a LayerRecord,
    layer_idx: usize,
    psd: &'a PsdReader,
}

impl<'a> NormalLayer<'a> {
    /// Returns the name of the layer.
    pub fn layer_name(&self) -> Result<String> {
        self.layer.layer_name(self.psd.encoding)
    }

    /// Returns the current layer's index in the PSD file.
    pub fn layer_index(&self) -> usize {
        self.layer_idx
    }

    /// Returns the top position of the layer.
    pub fn top(&self) -> i32 {
        self.layer.base.top
    }

    /// Returns the left position of the layer.
    pub fn left(&self) -> i32 {
        self.layer.base.left
    }

    /// Returns the bottom position of the layer.
    pub fn bottom(&self) -> i32 {
        self.layer.base.bottom
    }

    /// Returns the right position of the layer.
    pub fn right(&self) -> i32 {
        self.layer.base.right
    }

    /// Returns the width of the layer.
    pub fn width(&self) -> u32 {
        (self.layer.base.right - self.layer.base.left) as u32
    }

    /// Returns the height of the layer.
    pub fn height(&self) -> u32 {
        (self.layer.base.bottom - self.layer.base.top) as u32
    }

    /// Returns the number of channels in the layer.
    pub fn channels(&self) -> u16 {
        self.layer.base.channels
    }

    /// Reads and returns the raw channel id and data of the layer.
    pub fn read_raw_data(&self) -> Result<Vec<(i16, Vec<u8>)>> {
        let mut start_idx = self.psd.channel_start_indices[self.layer_idx];
        let mut channels_data = Vec::new();
        for cinfo in &self.layer.base.channel_infos {
            let mut data = self
                .psd
                .psd
                .layer_and_mask_info
                .layer_info
                .channel_image_data[start_idx]
                .clone();
            start_idx += 1;
            decompress_channel_image_data(&mut data, self)?;
            channels_data.push((cinfo.channel_id, data.image_data));
        }
        Ok(channels_data)
    }

    /// Reads and returns the image data of the layer.
    pub fn image(&self) -> Result<ImageData> {
        let color_mode = self.psd.color_mode();
        if !matches!(color_mode, 1 | 3) {
            anyhow::bail!(
                "Unsupported PSD color mode for image extraction: {}",
                self.psd.color_mode()
            );
        }
        let channels = self.layer.base.channels;
        let width = (self.layer.base.right - self.layer.base.left) as u32;
        let height = (self.layer.base.bottom - self.layer.base.top) as u32;
        if channels < 1 {
            anyhow::bail!("PSD layer has no channels");
        }
        let channels_map = BTreeMap::from_iter(self.read_raw_data()?);
        if color_mode == 1 {
            let grayscale = channels_map
                .get(&0)
                .ok_or_else(|| anyhow::anyhow!("PSD grayscale layer missing channel 0"))?;
            let alpha = channels_map.get(&-1);
            let depth = self.psd.bit_depth() as u8;
            if let Some(alpha) = alpha {
                let mut g = MsbBitStream::new(MemReaderRef::new(&grayscale));
                let mut a = MsbBitStream::new(MemReaderRef::new(&alpha));
                let mut data = MemWriter::new();
                let mut o = MsbBitWriter::new(&mut data);
                for _ in 0..height {
                    g.m_cached_bits = 0;
                    a.m_cached_bits = 0;
                    for _ in 0..width {
                        let gray = g.get_bits(depth as u32)?;
                        let alpha = a.get_bits(depth as u32)?;
                        o.put_bits(gray, depth)?;
                        o.put_bits(gray, depth)?;
                        o.put_bits(gray, depth)?;
                        o.put_bits(alpha, depth)?;
                    }
                    o.flush()?;
                }
                Ok(ImageData {
                    width,
                    height,
                    color_type: ImageColorType::Rgba,
                    depth,
                    data: data.into_inner(),
                })
            } else {
                Ok(ImageData {
                    width,
                    height,
                    color_type: ImageColorType::Grayscale,
                    depth,
                    data: grayscale.clone(),
                })
            }
        } else {
            let red = channels_map
                .get(&0)
                .ok_or_else(|| anyhow::anyhow!("PSD RGB layer missing channel 0"))?;
            let green = channels_map
                .get(&1)
                .ok_or_else(|| anyhow::anyhow!("PSD RGB layer missing channel 1"))?;
            let blue = channels_map
                .get(&2)
                .ok_or_else(|| anyhow::anyhow!("PSD RGB layer missing channel 2"))?;
            let mut a = channels_map
                .get(&-1)
                .map(|v| MsbBitStream::new(MemReaderRef::new(v)));
            let depth = self.psd.bit_depth() as u8;
            let mut r = MsbBitStream::new(MemReaderRef::new(&red));
            let mut g = MsbBitStream::new(MemReaderRef::new(&green));
            let mut b = MsbBitStream::new(MemReaderRef::new(&blue));
            let mut data = MemWriter::new();
            let mut o = MsbBitWriter::new(&mut data);
            for _ in 0..height {
                r.m_cached_bits = 0;
                g.m_cached_bits = 0;
                b.m_cached_bits = 0;
                if let Some(alpha) = &mut a {
                    alpha.m_cached_bits = 0;
                }
                for _ in 0..width {
                    let red = r.get_bits(depth as u32)?;
                    let green = g.get_bits(depth as u32)?;
                    let blue = b.get_bits(depth as u32)?;
                    o.put_bits(red, depth)?;
                    o.put_bits(green, depth)?;
                    o.put_bits(blue, depth)?;
                    if let Some(alpha) = &mut a {
                        let alpha = alpha.get_bits(depth as u32)?;
                        o.put_bits(alpha, depth)?;
                    }
                }
                o.flush()?;
            }
            Ok(ImageData {
                width,
                height,
                color_type: if a.is_some() {
                    ImageColorType::Rgba
                } else {
                    ImageColorType::Rgb
                },
                depth,
                data: data.into_inner(),
            })
        }
    }
}

#[derive(Debug)]
pub struct GroupLayer<'a> {
    layer: &'a LayerRecord,
    layer_idx: usize,
    psd: &'a PsdReader,
    pub childrens: Vec<Layer<'a>>,
    is_closed: bool,
}

impl<'a> GroupLayer<'a> {
    fn create<'b>(layer_idx: &'b mut usize, psd: &'a PsdReader) -> Result<Self> {
        let mut childrens = Vec::new();
        // skip the end marker
        *layer_idx += 1;
        let layer_count = psd.psd.layer_count();
        while *layer_idx < layer_count {
            let layer = &psd.psd.layer_and_mask_info.layer_info.layer_records[*layer_idx];
            let layer_type = if let Some(lsct) = layer.get_info(SECTION_DIVIDER_SETTING_KEY) {
                let type_info = SectionDividerSetting::unpack(
                    &mut MemReaderRef::new(lsct),
                    true,
                    psd.encoding,
                    &None,
                )?;
                type_info.typ
            } else {
                0
            };
            if layer_type == 1 || layer_type == 2 {
                let is_closed = layer_type == 2;
                return Ok(GroupLayer {
                    layer,
                    layer_idx: *layer_idx,
                    psd,
                    childrens,
                    is_closed,
                });
            } else if layer_type == 3 {
                childrens.push(Layer::Group(GroupLayer::create(layer_idx, psd)?));
            } else if layer_type == 0 {
                childrens.push(Layer::Normal(NormalLayer {
                    layer,
                    layer_idx: *layer_idx,
                    psd,
                }));
            } else {
                anyhow::bail!("Unknown layer section divider type: {}", layer_type);
            }
            *layer_idx += 1;
        }
        anyhow::bail!("Layer group does not have a start marker");
    }

    /// Returns whether the layer group is closed.
    pub fn is_closed(&self) -> bool {
        self.is_closed
    }

    /// Returns the name of the layer group.
    pub fn layer_name(&self) -> Result<String> {
        self.layer.layer_name(self.psd.encoding)
    }

    /// Returns the current layer's index in the PSD file.
    pub fn layer_index(&self) -> usize {
        self.layer_idx
    }
}

impl<'a> Deref for GroupLayer<'a> {
    type Target = Vec<Layer<'a>>;

    fn deref(&self) -> &Self::Target {
        &self.childrens
    }
}

#[derive(Debug)]
pub enum Layer<'a> {
    Normal(NormalLayer<'a>),
    Group(GroupLayer<'a>),
}

impl<'a> Layer<'a> {
    /// Returns the name of the layer.
    pub fn layer_name(&self) -> Result<String> {
        match self {
            Layer::Normal(n) => n.layer_name(),
            Layer::Group(g) => g.layer_name(),
        }
    }

    /// Returns the current layer's index in the PSD file.
    pub fn layer_index(&self) -> usize {
        match self {
            Layer::Normal(n) => n.layer_index(),
            Layer::Group(g) => g.layer_index(),
        }
    }
}

impl PsdReader {
    pub fn new<T: Read + Seek>(mut reader: T, encoding: Encoding) -> Result<Self> {
        let psd = PsdFile::unpack(&mut reader, true, encoding, &None)?;
        if psd.header.signature != *PSD_SIGNATURE {
            anyhow::bail!("Invalid PSD signature");
        }
        if psd.header.version != 1 {
            anyhow::bail!("Unsupported PSD version: {}", psd.header.version);
        }
        let mut channel_start_indices = Vec::new();
        let mut idx = 0;
        for layer in &psd.layer_and_mask_info.layer_info.layer_records {
            channel_start_indices.push(idx);
            idx += layer.base.channels as usize;
        }
        Ok(Self {
            psd,
            encoding,
            channel_start_indices,
        })
    }

    /// Returns the width of the PSD image.
    pub fn width(&self) -> u32 {
        self.psd.header.width
    }

    /// Returns the height of the PSD image.
    pub fn height(&self) -> u32 {
        self.psd.header.height
    }

    /// Returns the color mode of the PSD file.
    ///
    /// 1 = Grayscale, 3 = RGB, etc.
    pub fn color_mode(&self) -> u16 {
        self.psd.header.color_mode
    }

    /// Returns the number of channels in the PSD file.
    pub fn channels(&self) -> u16 {
        self.psd.header.channels
    }

    /// Returns the bit depth of the PSD file.
    pub fn bit_depth(&self) -> u16 {
        self.psd.header.depth
    }

    /// Reads and returns the layers in the PSD file.
    pub fn read_layers<'a>(&'a self) -> Result<Vec<Layer<'a>>> {
        let mut layers = Vec::new();
        let mut layer_idx = 0;
        let count = self.psd.layer_count();
        while layer_idx < count {
            let layer = &self.psd.layer_and_mask_info.layer_info.layer_records[layer_idx];
            let layer_type = if let Some(lsct) = layer.get_info(SECTION_DIVIDER_SETTING_KEY) {
                let type_info = SectionDividerSetting::unpack(
                    &mut MemReaderRef::new(lsct),
                    true,
                    self.encoding,
                    &None,
                )?;
                type_info.typ
            } else {
                0
            };
            if layer_type == 1 || layer_type == 2 {
                anyhow::bail!("Layer group does not have an end marker");
            }
            if layer_type == 0 {
                layers.push(Layer::Normal(NormalLayer {
                    layer,
                    layer_idx,
                    psd: self,
                }));
            } else if layer_type == 3 {
                layers.push(Layer::Group(GroupLayer::create(&mut layer_idx, self)?));
            } else {
                anyhow::bail!("Unknown layer section divider type: {}", layer_type);
            }
            layer_idx += 1;
        }
        Ok(layers)
    }

    /// Returns the normal layers in the PSD file. This function ignores layer groups.
    pub fn read_normal_layers<'a>(&'a self) -> Result<Vec<NormalLayer<'a>>> {
        let mut layers = Vec::new();
        let count = self.psd.layer_count();
        for layer_idx in 0..count {
            let layer = &self.psd.layer_and_mask_info.layer_info.layer_records[layer_idx];
            let layer_type = if let Some(lsct) = layer.get_info(SECTION_DIVIDER_SETTING_KEY) {
                let type_info = SectionDividerSetting::unpack(
                    &mut MemReaderRef::new(lsct),
                    true,
                    self.encoding,
                    &None,
                )?;
                type_info.typ
            } else {
                0
            };
            if layer_type == 0 {
                layers.push(NormalLayer {
                    layer,
                    layer_idx,
                    psd: self,
                });
            }
        }
        Ok(layers)
    }
}
