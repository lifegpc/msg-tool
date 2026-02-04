use super::NormalLayer;
use super::types::*;
use crate::ext::io::*;
use anyhow::Result;
use std::io::Read;

pub fn rle_compress(data: &[u8]) -> Vec<u8> {
    let start = 0;
    let line_end = data.len();
    let mut idx = start;
    let mut literal: Vec<u8> = Vec::new();
    let mut out_line: Vec<u8> = Vec::new();
    while idx < line_end {
        // detect run length at current position
        let mut run_len = 1;
        while idx + run_len < line_end && data[idx + run_len] == data[idx] && run_len < 128 {
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
            out_line.push(data[idx]);
            idx += run_len;
        } else {
            // collect literal bytes until a run of >=3 or 128 reached
            literal.push(data[idx]);
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
                        && data[idx + look_run] == data[idx]
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
    out_line
}

pub fn rle_decompress(data: &[u8]) -> Result<Vec<u8>> {
    let mut reader = MemReaderRef::new(data);
    let len = data.len();
    let mut out = Vec::new();
    while reader.pos < len {
        let c = reader.read_i8()?;
        if c >= 0 {
            let rlen = (c as usize) + 1;
            let old_len = out.len();
            out.resize(old_len + rlen, 0);
            reader.read_exact(&mut out[old_len..old_len + rlen])?;
        } else {
            let rlen = (-(c as isize) as usize) + 1;
            let val = reader.read_u8()?;
            let old_len = out.len();
            out.resize(old_len + rlen, val);
        }
    }
    Ok(out)
}

pub fn decompress_channel_image_data(
    data: &mut ChannelImageData,
    layer: &NormalLayer,
) -> Result<()> {
    match data.compression {
        0 => Ok(()), // no compression
        1 => {
            let mut reader = MemReaderRef::new(&data.image_data);
            let base = &layer.layer.base;
            let height = (base.bottom - base.top) as u32;
            let length_len = height as usize * 2;
            let mut start = length_len;
            let mut image = Vec::new();
            for _ in 0..height {
                let len = reader.read_u16_be()? as usize;
                let decompressed = rle_decompress(&data.image_data[start..start + len])?;
                start += len;
                image.extend(decompressed);
            }
            data.image_data = image;
            data.compression = 0;
            Ok(())
        }
        2 => {
            let mut decoder = flate2::read::ZlibDecoder::new(&data.image_data[..]);
            let mut decompressed = Vec::new();
            decoder.read_to_end(&mut decompressed)?;
            data.image_data = decompressed;
            data.compression = 0;
            Ok(())
        }
        3 => {
            let mut decoder = flate2::read::ZlibDecoder::new(&data.image_data[..]);
            let mut decompressed = Vec::new();
            decoder.read_to_end(&mut decompressed)?;
            let base = &layer.layer.base;
            let height = (base.bottom - base.top) as u32;
            let width = (base.right - base.left) as u32;
            let bit_depth = layer.psd.bit_depth();
            if bit_depth == 1 {
                anyhow::bail!("Decompression for 1-bit images is not implemented yet");
            }
            let mut writer = MemWriterRef::new(&mut decompressed);
            for _ in 0..height {
                if bit_depth == 8 {
                    let mut pre = writer.read_u8()?;
                    for _ in 1..width {
                        let cur = writer.peek_u8()?;
                        let val = cur.wrapping_add(pre);
                        writer.write_u8(val)?;
                        pre = val;
                    }
                } else if bit_depth == 16 {
                    let mut pre = writer.read_u16_be()?;
                    for _ in 1..width {
                        let cur = writer.peek_u16_be()?;
                        let val = cur.wrapping_add(pre);
                        writer.write_u16_be(val)?;
                        pre = val;
                    }
                } else if bit_depth == 32 {
                    let mut pre = writer.read_u32_be()?;
                    for _ in 1..width {
                        let cur = writer.peek_u32_be()?;
                        let val = cur.wrapping_add(pre);
                        writer.write_u32_be(val)?;
                        pre = val;
                    }
                } else {
                    anyhow::bail!("Unsupported bit depth for decompression: {}", bit_depth);
                }
            }
            data.image_data = decompressed;
            data.compression = 0;
            Ok(())
        }
        _ => anyhow::bail!("Unsupported compression type: {}", data.compression),
    }
}
