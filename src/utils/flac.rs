//! FLAC audio utilities.
use super::pcm::*;
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use anyhow::Result;
use libflac_sys::*;
use std::ffi::CStr;
use std::io::{Read, Seek, Write};

extern "C" fn write_callback(
    _encoder: *const FLAC__StreamEncoder,
    buffer: *const u8,
    bytes: usize,
    _samples: u32,
    _current_frame: u32,
    client_data: *mut std::ffi::c_void,
) -> FLAC__StreamEncoderWriteStatus {
    let writer = unsafe { &mut *(client_data as *mut &mut dyn WriteSeek) };
    let slice = unsafe { std::slice::from_raw_parts(buffer, bytes) };
    match writer.write_all(slice) {
        Ok(_) => FLAC__STREAM_ENCODER_WRITE_STATUS_OK,
        Err(_) => FLAC__STREAM_ENCODER_WRITE_STATUS_FATAL_ERROR,
    }
}

extern "C" fn tell_callback(
    _encoder: *const FLAC__StreamEncoder,
    absolute_byte_offset: *mut u64,
    client_data: *mut std::ffi::c_void,
) -> FLAC__StreamEncoderTellStatus {
    if absolute_byte_offset.is_null() {
        return FLAC__STREAM_ENCODER_TELL_STATUS_ERROR;
    }
    let writer = unsafe { &mut *(client_data as *mut &mut dyn WriteSeek) };
    match writer.stream_position() {
        Ok(pos) => {
            unsafe {
                *absolute_byte_offset = pos;
            }
            FLAC__STREAM_ENCODER_TELL_STATUS_OK
        }
        Err(_) => FLAC__STREAM_ENCODER_TELL_STATUS_ERROR,
    }
}

extern "C" fn seek_callback(
    _encoder: *const FLAC__StreamEncoder,
    absolute_byte_offset: u64,
    client_data: *mut std::ffi::c_void,
) -> FLAC__StreamEncoderSeekStatus {
    let writer = unsafe { &mut *(client_data as *mut &mut dyn WriteSeek) };
    match writer.seek(std::io::SeekFrom::Start(absolute_byte_offset)) {
        Ok(_) => FLAC__STREAM_ENCODER_SEEK_STATUS_OK,
        Err(_) => FLAC__STREAM_ENCODER_SEEK_STATUS_ERROR,
    }
}

fn handle_init_error(status: u32) -> Result<()> {
    if status == 0 {
        return Ok(());
    }
    let index = status as usize;
    let s = unsafe { CStr::from_ptr(FLAC__StreamEncoderInitStatusString[index]) };
    Err(anyhow::anyhow!(
        "FLAC encoder error: {}",
        s.to_string_lossy()
    ))
}

struct EncoderHandle {
    encoder: *mut FLAC__StreamEncoder,
}

impl Drop for EncoderHandle {
    fn drop(&mut self) {
        unsafe {
            FLAC__stream_encoder_delete(self.encoder);
        }
    }
}

/// Writes lossless audio data to a flac file.
///
/// * `header` - The PCM format header.
/// * `reader` - The reader to read audio data from.
/// * `writer` - The writer to write audio data to.
/// * `config` - Extra configuration options.
pub fn write_flac<W: Write + Seek, R: Read>(
    header: &PcmFormat,
    mut reader: R,
    mut writer: W,
    config: &ExtraConfig,
) -> Result<()> {
    if header.bits_per_sample > 32 {
        return Err(anyhow::anyhow!(
            "FLAC supports up to 32 bits per sample, got {}",
            header.bits_per_sample
        ));
    }
    let encoder = unsafe { FLAC__stream_encoder_new() };
    if encoder.is_null() {
        return Err(anyhow::anyhow!("Failed to create FLAC encoder"));
    }
    let encoder = EncoderHandle { encoder };
    unsafe {
        FLAC__stream_encoder_set_channels(encoder.encoder, header.channels as u32);
        FLAC__stream_encoder_set_compression_level(encoder.encoder, config.flac_compression_level);
        FLAC__stream_encoder_set_bits_per_sample(encoder.encoder, header.bits_per_sample as u32);
        FLAC__stream_encoder_set_sample_rate(encoder.encoder, header.sample_rate);
        FLAC__stream_encoder_set_verify(encoder.encoder, 1);
    }
    let mut raw_writer: &mut dyn WriteSeek = &mut writer;
    let raw_writer = &mut raw_writer as *mut _;
    handle_init_error(unsafe {
        FLAC__stream_encoder_init_stream(
            encoder.encoder,
            Some(write_callback),
            Some(seek_callback),
            Some(tell_callback),
            None,
            raw_writer as *mut std::ffi::c_void,
        )
    })?;
    let mut buf = Vec::<i32>::with_capacity(1024 * header.channels as usize);
    buf.resize(buf.capacity(), 0);
    let mut read_buf = Vec::<u8>::with_capacity(
        (header.bits_per_sample / 8) as usize * 1024 * header.channels as usize,
    );
    read_buf.resize(read_buf.capacity(), 0);
    loop {
        let readed = reader.read(&mut read_buf)?;
        if readed == 0 {
            break;
        }
        let mut r = MemReaderRef::new(&read_buf[..readed]);
        let samples =
            readed as usize / (header.bits_per_sample as usize / 8) / header.channels as usize;
        let mut i = 0;
        for _ in 0..samples {
            for _ in 0..header.channels {
                let sample = match header.bits_per_sample {
                    8 => r.read_i8()? as i32,
                    16 => r.read_i16()? as i32,
                    24 => {
                        let b1 = r.read_u8()? as i32;
                        let b2 = r.read_u8()? as i32;
                        let b3 = r.read_u8()? as i32;
                        let mut val = (b3 << 16) | (b2 << 8) | b1;
                        // Sign extend from 24 bits to 32
                        if val & 0x800000 != 0 {
                            val |= !0xffffff;
                        }
                        val
                    }
                    32 => r.read_i32()?,
                    _ => {
                        return Err(anyhow::anyhow!(
                            "Unsupported bits per sample: {}",
                            header.bits_per_sample
                        ));
                    }
                };
                buf[i] = sample;
                i += 1;
            }
        }
        if samples == 0 {
            break;
        }
        if unsafe {
            FLAC__stream_encoder_process_interleaved(encoder.encoder, buf.as_ptr(), samples as u32)
        } == 0
        {
            let state = unsafe { FLAC__stream_encoder_get_state(encoder.encoder) };
            let s = unsafe { CStr::from_ptr(FLAC__StreamEncoderStateString[state as usize]) };
            return Err(anyhow::anyhow!(
                "FLAC encoding error: {}",
                s.to_string_lossy()
            ));
        }
    }
    if unsafe { FLAC__stream_encoder_finish(encoder.encoder) } == 0 {
        let state = unsafe { FLAC__stream_encoder_get_state(encoder.encoder) };
        let s = unsafe { CStr::from_ptr(FLAC__StreamEncoderStateString[state as usize]) };
        return Err(anyhow::anyhow!(
            "FLAC encoding error: {}",
            s.to_string_lossy()
        ));
    }
    Ok(())
}
