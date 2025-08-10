//! PCM Utilities
use crate::ext::io::*;
use crate::types::*;
use crate::utils::struct_pack::*;
use anyhow::Result;
use msg_tool_macro::*;
use std::io::{Read, Seek, Write};

#[derive(Debug, StructPack, StructUnpack)]
/// PCM Audio Format
pub struct PcmFormat {
    /// The format tag
    pub format_tag: u16,
    /// The number of channels
    pub channels: u16,
    /// The sample rate
    pub sample_rate: u32,
    /// The average bytes per second
    pub average_bytes_per_second: u32,
    /// The block alignment
    pub block_align: u16,
    /// The bits per sample
    pub bits_per_sample: u16,
}

/// Writes PCM data to a file.
///
/// * `format` - The PCM format to write.
/// * `reader` - The reader to read PCM data from.
/// * `writer` - The writer to write PCM data to.
pub fn write_pcm<W: Write + Seek, R: Read>(
    format: &PcmFormat,
    mut reader: R,
    mut writer: W,
) -> Result<()> {
    writer.write_all(b"RIFF")?;
    let mut total_size = 0x24u32;
    writer.write_u32(0)?; // Placeholder for total size
    writer.write_all(b"WAVE")?;
    writer.write_all(b"fmt ")?;
    writer.write_u32(16)?; // Size of fmt chunk
    format.pack(&mut writer, false, Encoding::Utf8)?;
    writer.write_all(b"data")?;
    let mut data_size = 0u32;
    writer.write_u32(0)?; // Placeholder for data size
    let mut buffer = [0u8; 4096];
    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        writer.write_all(&buffer[..bytes_read])?;
        data_size += bytes_read as u32;
    }
    total_size += data_size;
    writer.seek(std::io::SeekFrom::Start(4))?;
    writer.write_u32(total_size)?;
    writer.seek(std::io::SeekFrom::Start(40))?;
    writer.write_u32(data_size)?;
    Ok(())
}
