use crate::ext::io::*;
use crate::types::*;
use crate::utils::encoding::*;
use crate::utils::struct_pack::*;
use anyhow::Result;
use msg_tool_macro::*;
use std::any::Any;
use std::io::{Read, Seek, Write};

pub const PSD_SIGNATURE: &[u8; 4] = b"8BPS";
pub const IMAGE_RESOURCE_SIGNATURE: &[u8; 4] = b"8BIM";
pub const LAYER_NAME_SOURCE_SETTING_KEY: &[u8; 4] = b"lnsr";
pub const LAYER_ID_KEY: &[u8; 4] = b"lyid";
pub const SECTION_DIVIDER_SETTING_KEY: &[u8; 4] = b"lsct";
pub const UNICODE_LAYER_KEY: &[u8; 4] = b"luni";

#[derive(Debug, Clone)]
pub struct UnicodeString(pub String);

impl StructPack for UnicodeString {
    fn pack<W: Write>(
        &self,
        writer: &mut W,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<()> {
        let encoded: Vec<_> = self.0.encode_utf16().collect();
        let len = encoded.len() as u32;
        len.pack(writer, big, encoding, info)?;
        for c in encoded {
            c.pack(writer, big, encoding, info)?;
        }
        Ok(())
    }
}

impl StructUnpack for UnicodeString {
    fn unpack<R: Read + Seek>(
        reader: &mut R,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<Self> {
        let len = u32::unpack(reader, big, encoding, info)?;
        if len == 0 {
            return Ok(UnicodeString(String::new()));
        }
        let mut encoded: Vec<u16> = Vec::with_capacity(len as usize);
        for _ in 0..len {
            let c = u16::unpack(reader, big, encoding, info)?;
            encoded.push(c);
        }
        let string = String::from_utf16(&encoded)
            .map_err(|e| anyhow::anyhow!("Failed to decode UTF-16 string: {}", e))?;
        Ok(UnicodeString(string))
    }
}

#[derive(Debug, Clone)]
pub struct PascalString(pub Vec<u8>);

impl StructPack for PascalString {
    fn pack<W: Write>(
        &self,
        writer: &mut W,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<()> {
        let len = self.0.len() as u8;
        len.pack(writer, big, encoding, info)?;
        writer.write_all(&self.0)?;
        if len % 2 == 0 {
            writer.write_u8(0)?; // padding byte
        }
        Ok(())
    }
}

impl StructUnpack for PascalString {
    fn unpack<R: Read + Seek>(
        reader: &mut R,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<Self> {
        let len = u8::unpack(reader, big, encoding, info)?;
        let encoded = reader.read_exact_vec(len as usize)?;
        if len % 2 == 0 {
            reader.read_u8()?; // padding byte
        }
        Ok(PascalString(encoded))
    }
}

#[derive(Debug, Clone)]
pub struct PascalString4(pub Vec<u8>);

impl StructPack for PascalString4 {
    fn pack<W: Write>(
        &self,
        writer: &mut W,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<()> {
        let len = self.0.len() as u8;
        len.pack(writer, big, encoding, info)?;
        let padding = 4 - (len as usize + 1) % 4;
        writer.write_all(&self.0)?;
        if padding != 4 {
            for _ in 0..padding {
                writer.write_u8(0)?; // padding byte
            }
        }
        Ok(())
    }
}

impl StructUnpack for PascalString4 {
    fn unpack<R: Read + Seek>(
        reader: &mut R,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<Self> {
        let len = u8::unpack(reader, big, encoding, info)?;
        let encoded = reader.read_exact_vec(len as usize)?;
        let padding = 4 - (len as usize + 1) % 4;
        if padding != 4 {
            for _ in 0..padding {
                let pad_byte = reader.read_u8()?;
                if pad_byte != 0 {
                    return Err(anyhow::anyhow!(
                        "Expected padding byte to be 0, got {}",
                        pad_byte
                    ));
                }
            }
        }
        Ok(PascalString4(encoded))
    }
}

#[derive(Debug, Clone, StructPack, StructUnpack)]
pub struct PsdHeader {
    pub signature: [u8; 4],
    pub version: u16,
    pub reserved: [u8; 6],
    pub channels: u16,
    pub height: u32,
    pub width: u32,
    pub depth: u16,
    pub color_mode: u16,
}

#[derive(Debug, Clone, StructPack, StructUnpack)]
pub struct ColorModeData {
    #[pvec(u32)]
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct ImageResourceSection {
    pub resources: Vec<ImageResourceBlock>,
}

impl StructUnpack for ImageResourceSection {
    fn unpack<R: Read + Seek>(
        reader: &mut R,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<Self> {
        let length = u32::unpack(reader, big, encoding, info)?;
        let mut stream_region = StreamRegion::with_size(reader, length as u64)?;
        let mut resources = Vec::new();
        while stream_region.cur_pos() < length as u64 {
            let resource = ImageResourceBlock::unpack(&mut stream_region, big, encoding, info)?;
            resources.push(resource);
            if let Ok(d) = stream_region.peek_u8() {
                if d == 0 {
                    stream_region.read_u8()?; // padding byte
                }
            }
        }
        Ok(ImageResourceSection { resources })
    }
}

impl StructPack for ImageResourceSection {
    fn pack<W: Write>(
        &self,
        writer: &mut W,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<()> {
        let mut mem = MemWriter::new();
        for resource in &self.resources {
            resource.pack(&mut mem, big, encoding, info)?;
            // #TODO: check if padding byte is needed
        }
        let data = mem.into_inner();
        let length = data.len() as u32;
        length.pack(writer, big, encoding, info)?;
        writer.write_all(&data)?;
        Ok(())
    }
}

#[derive(Debug, Clone, StructPack, StructUnpack)]
pub struct ImageResourceBlock {
    pub signature: [u8; 4],
    pub resource_id: u16,
    pub name: PascalString,
    #[pvec(u32)]
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct LayerAndMaskInfo {
    pub layer_info: LayerInfo,
    pub global_layer_mask_info: Option<GlobalLayerMaskInfo>,
    pub tagged_blocks: Vec<u8>,
}

impl StructUnpack for LayerAndMaskInfo {
    fn unpack<R: Read + Seek>(
        reader: &mut R,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<Self> {
        let length = u32::unpack(reader, big, encoding, info)?;
        let mut stream_region = StreamRegion::with_size(reader, length as u64)?;
        let layer_info = LayerInfo::unpack(&mut stream_region, big, encoding, info)?;
        let length = u32::unpack(&mut stream_region, big, encoding, info)?;
        let global_layer_mask_info = if length > 0 {
            stream_region.seek_relative(-4)?;
            Some(GlobalLayerMaskInfo::unpack(
                &mut stream_region,
                big,
                encoding,
                info,
            )?)
        } else {
            None
        };
        let mut tagged_blocks = Vec::new();
        stream_region.read_to_end(&mut tagged_blocks)?;
        Ok(LayerAndMaskInfo {
            layer_info,
            global_layer_mask_info,
            tagged_blocks,
        })
    }
}

impl StructPack for LayerAndMaskInfo {
    fn pack<W: Write>(
        &self,
        writer: &mut W,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<()> {
        let mut mem = MemWriter::new();
        self.layer_info.pack(&mut mem, big, encoding, info)?;
        if let Some(global_layer_mask_info) = &self.global_layer_mask_info {
            global_layer_mask_info.pack(&mut mem, big, encoding, info)?;
        } else {
            0u32.pack(&mut mem, big, encoding, info)?; // no global layer mask info
        }
        mem.write_all(&self.tagged_blocks)?;
        let data = mem.into_inner();
        let length = data.len() as u32;
        length.pack(writer, big, encoding, info)?;
        writer.write_all(&data)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct LayerInfo {
    pub layer_count: i16,
    pub layer_records: Vec<LayerRecord>,
    pub channel_image_data: Vec<ChannelImageData>,
}

impl StructUnpack for LayerInfo {
    fn unpack<R: Read + Seek>(
        reader: &mut R,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<Self> {
        let length = u32::unpack(reader, big, encoding, info)?;
        let mut stream_region = StreamRegion::with_size(reader, length as u64)?;
        let layer_count = i16::unpack(&mut stream_region, big, encoding, info)?;
        let mut layer_records = Vec::new();
        for _ in 0..layer_count.abs() {
            let layer_record = LayerRecord::unpack(&mut stream_region, big, encoding, info)?;
            layer_records.push(layer_record);
        }
        let mut channel_image_data = Vec::new();
        for i in 0..layer_count.abs() {
            let layer = &layer_records[i as usize];
            for j in 0..layer.base.channels {
                let info = Some(Box::new((layer.clone(), j as usize)) as Box<dyn Any>);
                let data = ChannelImageData::unpack(&mut stream_region, big, encoding, &info)?;
                channel_image_data.push(data);
            }
        }
        stream_region.seek_to_end()?;
        Ok(LayerInfo {
            layer_count,
            layer_records,
            channel_image_data,
        })
    }
}

impl StructPack for LayerInfo {
    fn pack<W: Write>(
        &self,
        writer: &mut W,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<()> {
        let mut mem = MemWriter::new();
        self.layer_count.pack(&mut mem, big, encoding, info)?;
        for layer_record in &self.layer_records {
            layer_record.pack(&mut mem, big, encoding, info)?;
        }
        let mut index = 0usize;
        for i in 0..self.layer_count {
            let layer = &self.layer_records[i as usize];
            let info = Some(Box::new(layer.clone()) as Box<dyn Any>);
            for _ in 0..layer.base.channels {
                let data = &self.channel_image_data[index];
                index += 1;
                data.pack(&mut mem, big, encoding, &info)?;
            }
        }
        let data = mem.into_inner();

        // Pad to 2 bytes
        let mut length = data.len() as u32;
        let need_pad = length % 2 != 0;
        if need_pad {
            length += 1;
        }
        length.pack(writer, big, encoding, info)?;
        writer.write_all(&data)?;
        if need_pad {
            writer.write_u8(0)?; // padding byte
        }
        Ok(())
    }
}

#[derive(Debug, Clone, StructPack, StructUnpack)]
pub struct LayerRecordBase {
    pub top: i32,
    pub left: i32,
    pub bottom: i32,
    pub right: i32,
    pub channels: u16,
    #[unpack_vec_len(channels)]
    #[pack_vec_len(self.channels)]
    pub channel_infos: Vec<ChannelInfo>,
    pub blend_mode_signature: [u8; 4],
    pub blend_mode_key: [u8; 4],
    pub opacity: u8,
    pub clipping: u8,
    pub flags: u8,
    pub filler: u8,
}

#[derive(Debug, Clone)]
pub struct LayerRecord {
    pub base: LayerRecordBase,
    pub layer_mask: Option<LayerMask>,
    pub layer_blending_ranges: LayerBlendingRanges,
    pub layer_name: PascalString4,
    pub infos: Vec<AdditionalLayerInfo>,
}

impl LayerRecord {
    pub fn get_info<'a>(&'a self, key: &[u8; 4]) -> Option<&'a [u8]> {
        for info in &self.infos {
            if &info.key == key {
                return Some(&info.data);
            }
        }
        None
    }

    pub fn layer_name(&self, encoding: Encoding) -> Result<String> {
        if let Some(uni) = self.get_info(UNICODE_LAYER_KEY) {
            let data = UnicodeLayer::unpack(&mut MemReaderRef::new(uni), true, encoding, &None)?;
            Ok(data.name.0)
        } else {
            let s = decode_to_string(encoding, &self.layer_name.0, true)?;
            Ok(s)
        }
    }
}

impl StructPack for LayerRecord {
    fn pack<W: Write>(
        &self,
        writer: &mut W,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<()> {
        self.base.pack(writer, big, encoding, info)?;
        let mut mem = MemWriter::new();
        if let Some(layer_mask) = &self.layer_mask {
            layer_mask.pack(&mut mem, big, encoding, info)?;
        } else {
            0u32.pack(&mut mem, big, encoding, info)?; // no layer mask
        }
        self.layer_blending_ranges
            .pack(&mut mem, big, encoding, info)?;
        self.layer_name.pack(&mut mem, big, encoding, info)?;
        for additional_info in &self.infos {
            additional_info.pack(&mut mem, big, encoding, info)?;
        }
        let data = mem.into_inner();
        let extra_data_length = data.len() as u32;
        extra_data_length.pack(writer, big, encoding, info)?;
        writer.write_all(&data)?;
        Ok(())
    }
}

impl StructUnpack for LayerRecord {
    fn unpack<R: Read + Seek>(
        reader: &mut R,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<Self> {
        let base = LayerRecordBase::unpack(reader, big, encoding, info)?;
        let extra_data_length = u32::unpack(reader, big, encoding, info)?;
        let mut stream_region = StreamRegion::with_size(reader, extra_data_length as u64)?;
        let layer_mask_len = u32::unpack(&mut stream_region, big, encoding, info)?;
        let layer_mask = if layer_mask_len > 0 {
            stream_region.seek_relative(-4)?;
            Some(LayerMask::unpack(&mut stream_region, big, encoding, info)?)
        } else {
            None
        };
        let layer_blending_ranges =
            LayerBlendingRanges::unpack(&mut stream_region, big, encoding, info)?;
        let layer_name = PascalString4::unpack(&mut stream_region, big, encoding, info)?;
        let mut infos = Vec::new();
        while stream_region.cur_pos() < extra_data_length as u64 {
            let additional_info =
                AdditionalLayerInfo::unpack(&mut stream_region, big, encoding, info)?;
            infos.push(additional_info);
        }
        Ok(LayerRecord {
            base,
            layer_mask,
            layer_blending_ranges,
            layer_name,
            infos,
        })
    }
}

#[derive(Debug, Clone)]
pub struct LayerMask {
    pub top: i32,
    pub left: i32,
    pub bottom: i32,
    pub right: i32,
    pub default_color: u8,
    pub flags: u8,
    pub mask_parameters: Option<u8>,
    pub mask_data: Option<Vec<u8>>,
    pub real_flags: Option<u8>,
    pub real_user_mask_background: Option<u8>,
    pub mask_top: Option<i32>,
    pub mask_left: Option<i32>,
    pub mask_bottom: Option<i32>,
    pub mask_right: Option<i32>,
}

impl StructPack for LayerMask {
    fn pack<W: Write>(
        &self,
        writer: &mut W,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<()> {
        let mut mem = MemWriter::new();
        self.top.pack(&mut mem, big, encoding, info)?;
        self.left.pack(&mut mem, big, encoding, info)?;
        self.bottom.pack(&mut mem, big, encoding, info)?;
        self.right.pack(&mut mem, big, encoding, info)?;
        self.default_color.pack(&mut mem, big, encoding, info)?;
        self.flags.pack(&mut mem, big, encoding, info)?;
        if self.flags == 4 {
            if let Some(mask_parameters) = self.mask_parameters {
                mask_parameters.pack(&mut mem, big, encoding, info)?;
            } else {
                return Err(anyhow::anyhow!(
                    "mask_parameters is required when flags == 4"
                ));
            }
        }
        if let Some(mask_data) = &self.mask_data {
            mem.write_all(mask_data)?;
        }
        if let Some(real_flags) = self.real_flags {
            real_flags.pack(&mut mem, big, encoding, info)?;
            let real_user_mask_background = self
                .real_user_mask_background
                .ok_or_else(|| anyhow::anyhow!("real_user_mask_background is required"))?;
            real_user_mask_background.pack(&mut mem, big, encoding, info)?;
            let mask_top = self
                .mask_top
                .ok_or_else(|| anyhow::anyhow!("mask_top is required"))?;
            mask_top.pack(&mut mem, big, encoding, info)?;
            let mask_left = self
                .mask_left
                .ok_or_else(|| anyhow::anyhow!("mask_left is required"))?;
            mask_left.pack(&mut mem, big, encoding, info)?;
            let mask_bottom = self
                .mask_bottom
                .ok_or_else(|| anyhow::anyhow!("mask_bottom is required"))?;
            mask_bottom.pack(&mut mem, big, encoding, info)?;
            let mask_right = self
                .mask_right
                .ok_or_else(|| anyhow::anyhow!("mask_right is required"))?;
            mask_right.pack(&mut mem, big, encoding, info)?;
        } else {
            if mem.data.len() == 18 {
                mem.write_u16(0)?; // padding to 20 bytes
            }
        }
        let data = mem.into_inner();
        let length = data.len() as u32;
        length.pack(writer, big, encoding, info)?;
        writer.write_all(&data)?;
        Ok(())
    }
}

impl StructUnpack for LayerMask {
    fn unpack<R: Read + Seek>(
        reader: &mut R,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<Self> {
        let length = u32::unpack(reader, big, encoding, info)?;
        let mut stream_region = StreamRegion::with_size(reader, length as u64)?;
        let top = i32::unpack(&mut stream_region, big, encoding, info)?;
        let left = i32::unpack(&mut stream_region, big, encoding, info)?;
        let bottom = i32::unpack(&mut stream_region, big, encoding, info)?;
        let right = i32::unpack(&mut stream_region, big, encoding, info)?;
        let default_color = u8::unpack(&mut stream_region, big, encoding, info)?;
        let flags = u8::unpack(&mut stream_region, big, encoding, info)?;
        let mask_parameters = if flags == 4 {
            Some(u8::unpack(&mut stream_region, big, encoding, info)?)
        } else {
            None
        };
        let mask_data = if flags == 0 || flags == 2 {
            Some(stream_region.read_exact_vec(1)?)
        } else if flags == 1 || flags == 3 {
            Some(stream_region.read_exact_vec(8)?)
        } else {
            None
        };
        if length == 20 {
            let _ = stream_region.read_u16()?; // padding
        }
        if stream_region.cur_pos() < length as u64 {
            let real_flags = u8::unpack(&mut stream_region, big, encoding, info)?;
            let real_user_mask_background = u8::unpack(&mut stream_region, big, encoding, info)?;
            let mask_top = i32::unpack(&mut stream_region, big, encoding, info)?;
            let mask_left = i32::unpack(&mut stream_region, big, encoding, info)?;
            let mask_bottom = i32::unpack(&mut stream_region, big, encoding, info)?;
            let mask_right = i32::unpack(&mut stream_region, big, encoding, info)?;
            stream_region.seek_to_end()?;
            Ok(LayerMask {
                top,
                left,
                bottom,
                right,
                default_color,
                flags,
                mask_parameters,
                mask_data,
                real_flags: Some(real_flags),
                real_user_mask_background: Some(real_user_mask_background),
                mask_top: Some(mask_top),
                mask_left: Some(mask_left),
                mask_bottom: Some(mask_bottom),
                mask_right: Some(mask_right),
            })
        } else {
            stream_region.seek_to_end()?;
            Ok(LayerMask {
                top,
                left,
                bottom,
                right,
                default_color,
                flags,
                mask_parameters,
                mask_data,
                real_flags: None,
                real_user_mask_background: None,
                mask_top: None,
                mask_left: None,
                mask_bottom: None,
                mask_right: None,
            })
        }
    }
}

#[derive(Debug, Clone, StructPack, StructUnpack)]
pub struct ChannelInfo {
    pub channel_id: i16,
    pub length: u32,
}

#[derive(Debug, Clone)]
pub struct LayerBlendingRanges {
    pub gray_blend_source: u32,
    pub gray_blend_dest: u32,
    pub channel_ranges: Vec<ChannelRange>,
}

impl StructUnpack for LayerBlendingRanges {
    fn unpack<R: Read + Seek>(
        reader: &mut R,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<Self> {
        let total_length = u32::unpack(reader, big, encoding, info)?;
        let mut stream_region = StreamRegion::with_size(reader, total_length as u64)?;
        let gray_blend_source = u32::unpack(&mut stream_region, big, encoding, info)?;
        let gray_blend_dest = u32::unpack(&mut stream_region, big, encoding, info)?;
        let mut channel_ranges = Vec::new();
        while stream_region.cur_pos() < total_length as u64 {
            let channel_range = ChannelRange::unpack(&mut stream_region, big, encoding, info)?;
            channel_ranges.push(channel_range);
        }
        Ok(LayerBlendingRanges {
            gray_blend_source,
            gray_blend_dest,
            channel_ranges,
        })
    }
}

impl StructPack for LayerBlendingRanges {
    fn pack<W: Write>(
        &self,
        writer: &mut W,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<()> {
        let mut mem = MemWriter::new();
        self.gray_blend_source.pack(&mut mem, big, encoding, info)?;
        self.gray_blend_dest.pack(&mut mem, big, encoding, info)?;
        for channel_range in &self.channel_ranges {
            channel_range.pack(&mut mem, big, encoding, info)?;
        }
        let data = mem.into_inner();
        let total_length = data.len() as u32;
        total_length.pack(writer, big, encoding, info)?;
        writer.write_all(&data)?;
        Ok(())
    }
}

#[derive(Debug, Clone, StructPack, StructUnpack)]
pub struct ChannelRange {
    pub source_range: u32,
    pub dest_range: u32,
}

#[derive(Debug, Clone)]
pub struct AdditionalLayerInfo {
    pub signature: [u8; 4],
    pub key: [u8; 4],
    pub data: Vec<u8>,
}

impl StructUnpack for AdditionalLayerInfo {
    fn unpack<R: Read + Seek>(
        reader: &mut R,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<Self> {
        let signature = <[u8; 4]>::unpack(reader, big, encoding, info)?;
        let key = <[u8; 4]>::unpack(reader, big, encoding, info)?;
        let length = u32::unpack(reader, big, encoding, info)?;
        let data = reader.read_exact_vec(length as usize)?;
        Ok(AdditionalLayerInfo {
            signature,
            key,
            data,
        })
    }
}

impl StructPack for AdditionalLayerInfo {
    fn pack<W: Write>(
        &self,
        writer: &mut W,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<()> {
        self.signature.pack(writer, big, encoding, info)?;
        self.key.pack(writer, big, encoding, info)?;
        let mut length = self.data.len() as u32;
        let need_pad = length % 2 != 0;
        if need_pad {
            length += 1;
        }
        length.pack(writer, big, encoding, info)?;
        writer.write_all(&self.data)?;
        if need_pad {
            writer.write_u8(0)?; // padding byte
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ChannelImageData {
    pub compression: u16,
    pub image_data: Vec<u8>,
}

fn get_layer_info(info: &Option<Box<dyn Any>>) -> Result<&LayerRecord> {
    if let Some(boxed) = info {
        if let Some(layer_record) = boxed.downcast_ref::<LayerRecord>() {
            return Ok(layer_record);
        }
    }
    Err(anyhow::anyhow!(
        "LayerRecord info is required for ChannelImageData unpacking"
    ))
}

fn get_layer_info_with_channel_index(info: &Option<Box<dyn Any>>) -> Result<&(LayerRecord, usize)> {
    if let Some(boxed) = info {
        if let Some(layer_info) = boxed.downcast_ref::<(LayerRecord, usize)>() {
            return Ok(layer_info);
        }
    }
    Err(anyhow::anyhow!(
        "LayerRecord and channel index info is required for ChannelImageData unpacking"
    ))
}

impl StructUnpack for ChannelImageData {
    fn unpack<R: Read + Seek>(
        reader: &mut R,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<Self> {
        let (layer, idx) = get_layer_info_with_channel_index(info)?;
        let layer_len = layer
            .base
            .channel_infos
            .get(*idx)
            .ok_or_else(|| anyhow::anyhow!("Channel index {} out of bounds for layer", idx))?
            .length;
        let mut stream_region = StreamRegion::with_size(reader, layer_len as u64)?;
        let compression = u16::unpack(&mut stream_region, big, encoding, info)?;
        let mut image_data = Vec::new();
        stream_region.read_to_end(&mut image_data)?;
        Ok(ChannelImageData {
            compression,
            image_data,
        })
    }
}

impl StructPack for ChannelImageData {
    fn pack<W: Write>(
        &self,
        writer: &mut W,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<()> {
        self.compression.pack(writer, big, encoding, info)?;
        if self.compression == 0 {
            let layer_info = get_layer_info(info)?;
            let expected_len = (layer_info.base.bottom - layer_info.base.top) as usize
                * (layer_info.base.right - layer_info.base.left) as usize;
            if self.image_data.len() != expected_len {
                return Err(anyhow::anyhow!(
                    "Channel image data length does not match expected size"
                ));
            }
        }
        writer.write_all(&self.image_data)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct GlobalLayerMaskInfo {
    pub overlays_color_space: u16,
    pub overlays_color_components: [u16; 4],
    pub opacity: u16,
    pub kind: u8,
    pub filler: Vec<u8>,
}

impl StructUnpack for GlobalLayerMaskInfo {
    fn unpack<R: Read + Seek>(
        reader: &mut R,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<Self> {
        let length = u32::unpack(reader, big, encoding, info)?;
        println!(
            "GlobalLayerMaskInfo length = {}, stream position = {}",
            length,
            reader.stream_position()?
        );
        let mut stream_region = StreamRegion::with_size(reader, length as u64)?;
        let overlays_color_space = u16::unpack(&mut stream_region, big, encoding, info)?;
        let mut overlays_color_components = [0u16; 4];
        for i in 0..4 {
            overlays_color_components[i] = u16::unpack(&mut stream_region, big, encoding, info)?;
        }
        let opacity = u16::unpack(&mut stream_region, big, encoding, info)?;
        let kind = u8::unpack(&mut stream_region, big, encoding, info)?;
        let filler_length = length as usize - 2 - (4 * 2) - 2 - 1;
        let mut filler = vec![0u8; filler_length];
        stream_region.read_exact(&mut filler)?;
        stream_region.seek_to_end()?;
        Ok(GlobalLayerMaskInfo {
            overlays_color_space,
            overlays_color_components,
            opacity,
            kind,
            filler,
        })
    }
}

impl StructPack for GlobalLayerMaskInfo {
    fn pack<W: Write>(
        &self,
        writer: &mut W,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<()> {
        let mut mem = MemWriter::new();
        self.overlays_color_space
            .pack(&mut mem, big, encoding, info)?;
        for component in &self.overlays_color_components {
            component.pack(&mut mem, big, encoding, info)?;
        }
        self.opacity.pack(&mut mem, big, encoding, info)?;
        self.kind.pack(&mut mem, big, encoding, info)?;
        mem.write_all(&self.filler)?;
        let data = mem.into_inner();
        let length = data.len() as u32;
        length.pack(writer, big, encoding, info)?;
        writer.write_all(&data)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ImageDataSection {
    pub compression: u16,
    pub image_data: Vec<u8>,
}

fn get_psd_header(info: &Option<Box<dyn Any>>) -> Result<&PsdHeader> {
    if let Some(boxed) = info {
        if let Some(psd_header) = boxed.downcast_ref::<PsdHeader>() {
            return Ok(psd_header);
        }
    }
    Err(anyhow::anyhow!("PsdHeader info is required"))
}

impl StructUnpack for ImageDataSection {
    fn unpack<R: Read + Seek>(
        reader: &mut R,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<Self> {
        let compression = u16::unpack(reader, big, encoding, info)?;
        let mut image_data = Vec::new();
        reader.read_to_end(&mut image_data)?;
        Ok(ImageDataSection {
            compression,
            image_data,
        })
    }
}

impl StructPack for ImageDataSection {
    fn pack<W: Write>(
        &self,
        writer: &mut W,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<()> {
        self.compression.pack(writer, big, encoding, info)?;
        if self.compression == 0 {
            let psd_header = get_psd_header(info)?;
            let expected_len = psd_header.channels as usize
                * psd_header.height as usize
                * psd_header.width as usize
                * psd_header.depth as usize
                / 8;
            if self.image_data.len() != expected_len {
                return Err(anyhow::anyhow!(
                    "Image data length does not match expected size"
                ));
            }
        }
        writer.write_all(&self.image_data)?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct PsdFile {
    pub header: PsdHeader,
    pub color_mode_data: ColorModeData,
    pub image_resource: ImageResourceSection,
    pub layer_and_mask_info: LayerAndMaskInfo,
    pub image_data: ImageDataSection,
}

impl StructPack for PsdFile {
    fn pack<W: Write>(
        &self,
        writer: &mut W,
        big: bool,
        encoding: Encoding,
        _info: &Option<Box<dyn Any>>,
    ) -> Result<()> {
        let psd_info = Some(Box::new(self.header.clone()) as Box<dyn Any>);
        self.header.pack(writer, big, encoding, &psd_info)?;
        self.color_mode_data
            .pack(writer, big, encoding, &psd_info)?;
        self.image_resource.pack(writer, big, encoding, &psd_info)?;
        self.layer_and_mask_info
            .pack(writer, big, encoding, &psd_info)?;
        self.image_data.pack(writer, big, encoding, &psd_info)?;
        Ok(())
    }
}

impl StructUnpack for PsdFile {
    fn unpack<R: Read + Seek>(
        reader: &mut R,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<Self> {
        let header = PsdHeader::unpack(reader, big, encoding, info)?;
        let psd_info = Some(Box::new(header.clone()) as Box<dyn Any>);
        let color_mode_data = ColorModeData::unpack(reader, big, encoding, &psd_info)?;
        let image_resource = ImageResourceSection::unpack(reader, big, encoding, &psd_info)?;
        let layer_and_mask_info = LayerAndMaskInfo::unpack(reader, big, encoding, &psd_info)?;
        let image_data = ImageDataSection::unpack(reader, big, encoding, &psd_info)?;
        Ok(PsdFile {
            header,
            color_mode_data,
            image_resource,
            layer_and_mask_info,
            image_data,
        })
    }
}

impl PsdFile {
    pub fn layer_count(&self) -> usize {
        self.layer_and_mask_info.layer_info.layer_count.abs() as usize
    }
}

#[derive(Debug, Clone, StructPack, StructUnpack)]
pub struct SectionDividerSetting {
    pub typ: u32,
    // TODO: implement the rest fields
}

#[derive(Debug, Clone, StructPack, StructUnpack)]
pub struct UnicodeLayer {
    pub name: UnicodeString,
}

#[derive(Debug, Clone, StructPack, StructUnpack)]
pub struct LayerID {
    /// ID for the layer
    pub id: u32,
}

#[derive(Debug, Clone, StructPack, StructUnpack)]
pub struct LayerNameSourceSetting {
    /// ID for the layer name
    pub id: i32,
}
