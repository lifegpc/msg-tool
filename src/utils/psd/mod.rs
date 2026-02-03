//! A simple PSD writer
mod types;

use crate::ext::io::*;
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
    compress: bool,
    zlib_compression_level: u32,
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
        Ok(Self {
            psd,
            color_type,
            compress: true,
            zlib_compression_level: 6,
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

    /// Add a visible layer to the PSD file.
    pub fn add_layer(&mut self, name: &str, x: u32, y: u32, mut data: ImageData) -> Result<()> {
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
        let mut layer_base = LayerRecordBase {
            top: y as i32,
            left: x as i32,
            bottom: (y + data.height) as i32,
            right: (x + data.width) as i32,
            channels: data.color_type.bpp(1) as u16,
            channel_infos: Vec::new(),
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
                    let mut idx = start;
                    let mut literal: Vec<u8> = Vec::new();
                    let mut out_line: Vec<u8> = Vec::new();

                    while idx < line_end {
                        // detect run length at current position
                        let mut run_len = 1;
                        while idx + run_len < line_end
                            && planar_data[idx + run_len] == planar_data[idx]
                            && run_len < 128
                        {
                            run_len += 1;
                        }

                        if run_len >= 3 {
                            // flush any pending literals
                            if !literal.is_empty() {
                                // header = literal_len - 1 (0..127)
                                let header = (literal.len() - 1) as i8;
                                out_line.push(header as u8);
                                out_line.extend_from_slice(&literal);
                                literal.clear();
                            }
                            // write run: header = -(run_len - 1), then single byte value
                            let header = -(((run_len as u8) - 1) as i8);
                            out_line.push(header as u8);
                            out_line.push(planar_data[idx]);
                            idx += run_len;
                        } else {
                            // collect literal bytes until a run of >=3 or 128 reached
                            literal.push(planar_data[idx]);
                            idx += 1;
                            // if literal is full, flush it
                            if literal.len() == 128 {
                                let header = (literal.len() - 1) as i8;
                                out_line.push(header as u8);
                                out_line.extend_from_slice(&literal);
                                literal.clear();
                            } else {
                                // peek ahead: if next starts a run >=3, flush literal now
                                if idx < line_end {
                                    let mut look_run = 1;
                                    while idx + look_run < line_end
                                        && planar_data[idx + look_run] == planar_data[idx]
                                        && look_run < 128
                                    {
                                        look_run += 1;
                                    }
                                    if look_run >= 3 {
                                        if !literal.is_empty() {
                                            let header = (literal.len() - 1) as i8;
                                            out_line.push(header as u8);
                                            out_line.extend_from_slice(&literal);
                                            literal.clear();
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // flush remaining literal
                    if !literal.is_empty() {
                        let header = (literal.len() - 1) as i8;
                        out_line.push(header as u8);
                        out_line.extend_from_slice(&literal);
                        literal.clear();
                    }

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
        self.psd.pack(&mut writer, true, encoding, &None)?;
        Ok(())
    }
}
