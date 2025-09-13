//! JPEG XL image support
use crate::types::*;
use anyhow::Result;
use jpegxl_sys::common::types::*;
use jpegxl_sys::decode::*;
use jpegxl_sys::encoder::encode::*;
use jpegxl_sys::metadata::codestream_header::*;
use std::io::Read;

struct JxlDecoderHandle {
    handle: *mut JxlDecoder,
}

impl Drop for JxlDecoderHandle {
    fn drop(&mut self) {
        unsafe {
            JxlDecoderDestroy(self.handle);
        }
    }
}

struct JxlEncoderHandle {
    handle: *mut JxlEncoder,
}

impl Drop for JxlEncoderHandle {
    fn drop(&mut self) {
        unsafe {
            JxlEncoderDestroy(self.handle);
        }
    }
}

fn check_decoder_status(status: JxlDecoderStatus) -> Result<()> {
    match status {
        JxlDecoderStatus::Success => Ok(()),
        _ => Err(anyhow::anyhow!("JXL decoder error: {:?}", status)),
    }
}

fn check_encoder_status(status: JxlEncoderStatus) -> Result<()> {
    match status {
        JxlEncoderStatus::Success => Ok(()),
        _ => Err(anyhow::anyhow!("JXL encoder error: {:?}", status)),
    }
}

fn default_basic_info() -> JxlBasicInfo {
    let basic_info = std::mem::MaybeUninit::<JxlBasicInfo>::zeroed();
    unsafe { basic_info.assume_init_read() }
}

/// Decode JXL image from reader
pub fn decode_jxl<R: Read>(mut r: R) -> Result<ImageData> {
    let decoder = unsafe { JxlDecoderCreate(std::ptr::null()) };
    if decoder.is_null() {
        return Err(anyhow::anyhow!("Failed to create JXL decoder"));
    }
    let dh = JxlDecoderHandle { handle: decoder };
    let events = JxlDecoderStatus::BasicInfo as i32
        | JxlDecoderStatus::FullImage as i32
        | JxlDecoderStatus::ColorEncoding as i32;
    check_decoder_status(unsafe { JxlDecoderSubscribeEvents(dh.handle, events) })?;
    let mut data = Vec::new();
    r.read_to_end(&mut data)?;
    check_decoder_status(unsafe { JxlDecoderSetInput(dh.handle, data.as_ptr(), data.len()) })?;
    unsafe {
        JxlDecoderCloseInput(dh.handle);
    };
    let mut basic_info = default_basic_info();
    let mut color_type = ImageColorType::Rgb;
    let mut buffer = Vec::new();
    loop {
        let status = unsafe { JxlDecoderProcessInput(dh.handle) };
        match status {
            JxlDecoderStatus::BasicInfo => {
                check_decoder_status(unsafe {
                    JxlDecoderGetBasicInfo(dh.handle, &mut basic_info)
                })?;
                match basic_info.num_color_channels {
                    1 => color_type = ImageColorType::Grayscale,
                    3 => {
                        if basic_info.alpha_bits > 0 {
                            color_type = ImageColorType::Rgba;
                        } else {
                            color_type = ImageColorType::Rgb;
                        }
                    }
                    _ => {
                        return Err(anyhow::anyhow!(
                            "Unsupported number of color channels: {}",
                            basic_info.num_color_channels
                        ));
                    }
                }
                if !matches!(basic_info.bits_per_sample, 8 | 16) {
                    return Err(anyhow::anyhow!(
                        "Unsupported bits per sample: {}",
                        basic_info.bits_per_sample
                    ));
                }
            }
            JxlDecoderStatus::NeedImageOutBuffer => {
                let format = JxlPixelFormat {
                    num_channels: color_type.bpp(1) as u32,
                    data_type: if basic_info.bits_per_sample <= 8 {
                        JxlDataType::Uint8
                    } else {
                        JxlDataType::Uint16
                    },
                    endianness: JxlEndianness::Little,
                    align: 0,
                };
                let mut buffer_size: usize = 0;
                check_decoder_status(unsafe {
                    JxlDecoderImageOutBufferSize(dh.handle, &format, &mut buffer_size)
                })?;
                buffer.resize(buffer_size, 0);
                check_decoder_status(unsafe {
                    JxlDecoderSetImageOutBuffer(
                        dh.handle,
                        &format,
                        buffer.as_mut_ptr() as *mut _,
                        buffer_size,
                    )
                })?;
            }
            JxlDecoderStatus::Success => {
                break;
            }
            JxlDecoderStatus::Error => {
                return Err(anyhow::anyhow!("JXL decoding error"));
            }
            _ => {}
        }
    }
    Ok(ImageData {
        width: basic_info.xsize,
        height: basic_info.ysize,
        color_type,
        depth: basic_info.bits_per_sample as u8,
        data: buffer,
    })
}

/// Encode image data to JXL format
pub fn encode_jxl(img: ImageData, _config: &ExtraConfig) -> Result<Vec<u8>> {
    let encoder = unsafe { JxlEncoderCreate(std::ptr::null()) };
    if encoder.is_null() {
        return Err(anyhow::anyhow!("Failed to create JXL encoder"));
    }
    let eh = JxlEncoderHandle { handle: encoder };
    let mut basic_info = default_basic_info();
    basic_info.xsize = img.width;
    basic_info.ysize = img.height;
    basic_info.bits_per_sample = match img.depth {
        8 => 8,
        16 => 16,
        _ => {
            return Err(anyhow::anyhow!(
                "Unsupported bits per sample: {}",
                img.depth
            ));
        }
    };
    basic_info.alpha_bits = match img.color_type {
        ImageColorType::Rgba | ImageColorType::Bgra => img.depth as u32,
        _ => 0,
    };
    basic_info.num_color_channels = match img.color_type {
        ImageColorType::Bgr | ImageColorType::Rgb | ImageColorType::Bgra | ImageColorType::Rgba => {
            3
        }
        ImageColorType::Grayscale => 1,
    };
    basic_info.num_extra_channels = if basic_info.alpha_bits > 0 { 1 } else { 0 };
    basic_info.orientation = JxlOrientation::Identity;
    basic_info.uses_original_profile = JxlBool::True;
    check_encoder_status(unsafe { JxlEncoderSetBasicInfo(eh.handle, &basic_info) })?;
    let options = unsafe { JxlEncoderFrameSettingsCreate(eh.handle, std::ptr::null()) };
    if options.is_null() {
        return Err(anyhow::anyhow!(
            "Failed to create JXL encoder frame settings"
        ));
    }
    check_encoder_status(unsafe { JxlEncoderSetFrameLossless(options, JxlBool::True) })?;
    let format = JxlPixelFormat {
        num_channels: img.color_type.bpp(1) as u32,
        data_type: if img.depth <= 8 {
            JxlDataType::Uint8
        } else {
            JxlDataType::Uint16
        },
        endianness: JxlEndianness::Little,
        align: 0,
    };
    check_encoder_status(unsafe {
        JxlEncoderAddImageFrame(
            options,
            &format,
            img.data.as_ptr() as *const _,
            img.data.len(),
        )
    })?;
    unsafe { JxlEncoderCloseInput(eh.handle) };
    let mut compressed_data = Vec::new();
    let mut buffer = [0u8; 4096];
    loop {
        let mut avail_out = buffer.len();
        let mut next_out = buffer.as_mut_ptr();
        let status = unsafe { JxlEncoderProcessOutput(eh.handle, &mut next_out, &mut avail_out) };
        let used = buffer.len() - avail_out;
        compressed_data.extend_from_slice(&buffer[..used]);
        match status {
            JxlEncoderStatus::Success => break,
            JxlEncoderStatus::NeedMoreOutput => {}
            _ => {
                return Err(anyhow::anyhow!("JXL encoding error: {:?}", status));
            }
        }
    }
    Ok(compressed_data)
}
