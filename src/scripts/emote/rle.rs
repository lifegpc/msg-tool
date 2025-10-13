//! RL Encode used in mtn files
use crate::ext::io::*;
use anyhow::Result;
use std::io::{Read, Seek, SeekFrom};

const LZSS_LOOKAHED: usize = 1 << 7;

/// Decompress RL data
/// * `align` - alignment. usually 4
/// * `actual_size` - if known, set it to preallocate memory
pub fn rl_decompress<T: Read + Seek>(
    mut input: T,
    align: usize,
    actual_size: Option<usize>,
) -> Result<Vec<u8>> {
    let mut output = if let Some(size) = actual_size {
        Vec::with_capacity(size)
    } else {
        Vec::new()
    };
    let mut readed = input.stream_position()?;
    let len = input.stream_length()?;
    while readed < len {
        let current = input.read_u8()? as usize;
        readed += 1;
        let count;
        if (current & LZSS_LOOKAHED) != 0 {
            count = (current ^ LZSS_LOOKAHED) + 3;
            let buf = input.read_exact_vec(align)?;
            readed += align as u64;
            for _ in 0..count {
                output.extend_from_slice(&buf);
            }
        } else {
            count = (current + 1) * align;
            let buf = input.read_exact_vec(count)?;
            readed += count as u64;
            output.extend_from_slice(&buf);
        }
    }
    Ok(output)
}

fn compress_bound<T: Read + Seek>(input: &mut T, align: usize) -> Result<(usize, u8, Vec<u8>)> {
    let pos = input.stream_position()?;
    let mut curpos = pos;
    let len = input.stream_length()?;
    let mut buffer = vec![0u8; align];
    let mut tmp = vec![0u8; align];
    input.read_exact(&mut buffer)?;
    curpos += align as u64;
    let mut count = 1usize;
    for _ in 1..LZSS_LOOKAHED + 2 {
        if curpos >= len {
            break;
        }
        input.read_exact(&mut tmp)?;
        curpos += align as u64;
        if buffer == tmp {
            count += 1;
        } else {
            break;
        }
    }
    input.seek(SeekFrom::Start(pos))?;
    if count >= 3 {
        return Ok((count, (count - 3) as u8 | LZSS_LOOKAHED as u8, buffer));
    }
    Ok((0, 0, buffer))
}

fn compress_bound_np<T: Read + Seek>(input: &mut T, align: usize) -> Result<(usize, u8)> {
    let pos = input.stream_position()?;
    let mut curpos = pos;
    let len = input.stream_length()?;
    input.seek_relative(align as i64)?;
    curpos += align as u64;
    let mut count = 1;
    for _ in 1..LZSS_LOOKAHED {
        if curpos >= len {
            break;
        }
        let (ncount, _cmd, _buf) = compress_bound(input, align)?;
        if ncount == 0 {
            input.seek_relative(align as i64)?;
            count += 1;
            curpos += align as u64;
        } else {
            break;
        }
    }
    input.seek(SeekFrom::Start(pos))?;
    Ok((count, (count - 1) as u8))
}

/// Compress data using RL
/// * `align` - alignment. usually 4
pub fn rl_compress<T: Read + Seek>(mut input: T, align: usize) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    let len = input.stream_length()?;
    let mut readed = input.stream_position()?;
    while readed < len {
        let (count, cmd, buf) = compress_bound(&mut input, align)?;
        if count > 0 {
            output.push(cmd);
            output.extend_from_slice(&buf);
            readed += (count * align) as u64;
            input.seek_relative((count * align) as i64)?;
        } else {
            let (ncount, ncmd) = compress_bound_np(&mut input, align)?;
            output.push(ncmd);
            let buf = input.read_exact_vec(ncount * align)?;
            output.extend_from_slice(&buf);
            readed += (ncount * align) as u64;
        }
    }
    Ok(output)
}
