//! A simple PSD writer
mod types;

use crate::types::*;
use crate::utils::img::*;
use crate::utils::struct_pack::*;
use anyhow::Result;
use std::io::Write;
use types::*;

/// A simple PSD writer.
pub struct PsdWriter {
    psd: PsdFile,
    color_type: ImageColorType,
}

impl PsdWriter {
    /// Creates a new PSD writer with the specified dimensions, color type, and bit depth.
    pub fn new(width: u32, height: u32, color_type: ImageColorType, depth: u8) -> Result<Self> {
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
                global_layer_mask_info: GlobalLayerMaskInfo {
                    overlays_color_space: 0,
                    overlays_color_components: [0; 4],
                    opacity: 0,
                    kind: 128,
                    filler: vec![0],
                },
                tagged_blocks: vec![],
            },
            image_data: ImageDataSection {
                compression: 0,
                image_data: vec![],
            },
        };
        Ok(Self { psd, color_type })
    }

    /// Add a visible layer to the PSD file.
    pub fn add_layer(&mut self, name: &str, x: u32, y: u32, mut data: ImageData) -> Result<()> {
        if data.color_type == ImageColorType::Bgr {
            convert_bgr_to_rgb(&mut data)?;
        }
        if data.color_type == ImageColorType::Bgra {
            convert_bgra_to_rgba(&mut data)?;
        }
        let length = data.width as u32 * data.height as u32;
        let mut channel_infos = Vec::new();
        if data.color_type == ImageColorType::Grayscale {
            channel_infos.push(ChannelInfo {
                channel_id: 0,
                length: length + 2,
            });
        } else {
            channel_infos.push(ChannelInfo {
                channel_id: 0,
                length: length + 2,
            });
            channel_infos.push(ChannelInfo {
                channel_id: 1,
                length: length + 2,
            });
            channel_infos.push(ChannelInfo {
                channel_id: 2,
                length: length + 2,
            });
            if data.color_type == ImageColorType::Rgba {
                channel_infos.push(ChannelInfo {
                    channel_id: -1,
                    length: length + 2,
                });
            }
        }
        let layer_base = LayerRecordBase {
            top: y as i32,
            left: x as i32,
            bottom: (y + data.height) as i32,
            right: (x + data.width) as i32,
            channels: data.color_type.bpp(1) as u16,
            channel_infos: channel_infos,
            blend_mode_signature: *IMAGE_RESOURCE_SIGNATURE,
            blend_mode_key: *b"norm",
            opacity: 255,
            clipping: 0,
            flags: 1,
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
            image_data.push(ChannelImageData {
                compression: 0,
                image_data: d,
            });
        }
        let layer = LayerRecord {
            base: layer_base,
            layer_mask: None,
            layer_blending_ranges,
            layer_name: PascalString4(name.to_string()),
            infos: vec![],
        };
        self.psd
            .layer_and_mask_info
            .layer_info
            .layer_records
            .push(layer);

        // Update layer count
        self.psd.layer_and_mask_info.layer_info.layer_count =
            self.psd.layer_and_mask_info.layer_info.layer_records.len() as u16;

        self.psd
            .layer_and_mask_info
            .layer_info
            .channel_image_data
            .extend(image_data);
        Ok(())
    }

    /// Saves the PSD file to the specified writer with the given encoding.
    ///
    /// * `data` - The final composite image data to be saved in the PSD file.
    pub fn save<T: Write>(
        &mut self,
        data: ImageData,
        mut writer: T,
        encoding: Encoding,
    ) -> Result<()> {
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

        self.psd.image_data.image_data = planar_data;
        self.psd.image_data.compression = 0; // Raw
        self.psd.pack(&mut writer, true, encoding, &None)?;
        Ok(())
    }
}
