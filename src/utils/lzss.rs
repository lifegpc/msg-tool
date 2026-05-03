//! LZSS Stream
use crate::ext::io::*;
use std::io::{Read, Result};

/// A Lzss Reader
pub struct LzssReader<T: Read> {
    reader: T,
    buf: Vec<u8>,
    buf_pos: usize,
    frame: Vec<u8>,
    frame_mask: usize,
    frame_pos: usize,
    tmp: u8,
    tmp_used: bool,
}

impl<T: Read> LzssReader<T> {
    pub fn new(reader: T) -> Self {
        Self::new2(reader, 0x1000, 0, 0xfee)
    }
    pub fn new2(reader: T, frame_size: usize, frame_fill: u8, frame_init_pos: usize) -> Self {
        Self {
            reader,
            buf: Vec::new(),
            buf_pos: 0,
            frame: vec![frame_fill; frame_size],
            frame_mask: frame_size - 1,
            frame_pos: frame_init_pos,
            tmp: 0,
            tmp_used: false,
        }
    }
    fn push(&mut self, buf: &mut MemWriterRef, data: u8) -> Result<()> {
        if buf.is_eof() {
            self.buf.push(data);
        } else {
            buf.write_u8(data)?;
        }
        Ok(())
    }
    fn unpack(&mut self, buf: &mut MemWriterRef) -> Result<()> {
        let mut bu = [0; 1];
        let mut readed = if self.tmp_used {
            bu[0] = self.tmp;
            self.tmp_used = false;
            1
        } else {
            self.reader.read(&mut bu)?
        };
        if readed == 0 {
            // eof
            return Ok(());
        }
        let ctl = bu[0];
        let mut bit = 1;
        readed = self.reader.read(&mut bu)?;
        while bit != 0x100 && readed > 0 {
            if ((ctl as u32) & bit) != 0 {
                let b = bu[0];
                self.frame[self.frame_pos] = b;
                self.frame_pos += 1;
                self.frame_pos &= self.frame_mask;
                self.push(buf, b)?;
            } else {
                let lo = bu[0];
                readed = self.reader.read(&mut bu)?;
                if readed == 0 {
                    return Ok(());
                }
                let hi = bu[0];
                let mut offset = (((hi as usize) & 0xF0) << 4) | (lo as usize);
                let mut count = 3 + (hi & 0xF);
                while count != 0 {
                    let v = self.frame[offset];
                    offset += 1;
                    offset &= self.frame_mask;
                    self.frame[self.frame_pos] = v;
                    self.frame_pos += 1;
                    self.frame_pos &= self.frame_mask;
                    self.push(buf, v)?;
                    count -= 1;
                }
            }
            bit <<= 1;
            readed = self.reader.read(&mut bu)?;
        }
        if readed > 0 {
            self.tmp = bu[0];
            self.tmp_used = true;
        }
        Ok(())
    }
}

impl<R: Read> Read for LzssReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        if !self.buf.is_empty() && self.buf_pos < self.buf.len() {
            let readed = buf.len().min(self.buf.len() - self.buf_pos);
            buf[..readed].copy_from_slice(&self.buf[self.buf_pos..self.buf_pos + readed]);
            self.buf_pos += readed;
            if self.buf_pos == self.buf.len() {
                self.buf.clear();
                self.buf_pos = 0;
            }
            return Ok(readed);
        }
        let mut writer = MemWriterRef::new(buf);
        self.unpack(&mut writer)?;
        Ok(writer.pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_literal_only() {
        let data = [0xFF, 0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80];
        let mut reader = LzssReader::new(&data[..]);
        let mut result = Vec::new();
        reader.read_to_end(&mut result).unwrap();
        assert_eq!(result, vec![0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80]);
    }

    #[test]
    fn test_back_ref() {
        // "ABCABCABCABCABCABC": 3 literals + 5 back-refs (offset=0, count=3)
        let data = [0x07, b'A', b'B', b'C', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let mut reader = LzssReader::new2(&data[..], 256, 0, 0);
        let mut result = Vec::new();
        reader.read_to_end(&mut result).unwrap();
        assert_eq!(result, b"ABCABCABCABCABCABC");
    }

    #[test]
    fn test_chunked_read() {
        let data = [0x07, b'A', b'B', b'C', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let mut reader = LzssReader::new2(&data[..], 256, 0, 0);
        let mut buf = [0u8; 5];
        let mut result = Vec::new();
        loop {
            let n = reader.read(&mut buf).unwrap();
            if n == 0 {
                break;
            }
            result.extend_from_slice(&buf[..n]);
        }
        assert_eq!(result, b"ABCABCABCABCABCABC");
    }

    #[test]
    fn test_run_length() {
        // "AAAAA" via literal 'A' + back-ref (offset=0, count=4) for expanding run
        let data = [0x01, b'A', 0, 0x01];
        let mut reader = LzssReader::new2(&data[..], 256, 0, 0);
        let mut result = Vec::new();
        reader.read_to_end(&mut result).unwrap();
        assert_eq!(result, b"AAAAA");
    }

    #[test]
    fn test_back_ref_offset() {
        // "XXXXXABCDEABCDE": literals + back-ref at non-zero offset
        let data = [
            0xFF, b'X', b'X', b'X', b'X', b'X', b'A', b'B', b'C', 0x03, b'D', b'E', 0x05, 0x02,
        ];
        let mut reader = LzssReader::new2(&data[..], 256, 0, 0);
        let mut result = Vec::new();
        reader.read_to_end(&mut result).unwrap();
        assert_eq!(result, b"XXXXXABCDEABCDE");
    }

    #[test]
    fn test_multi_control_byte() {
        // 16 literals across 2 control bytes
        let data = [
            0xFF, b'A', b'B', b'C', b'D', b'E', b'F', b'G', b'H', 0xFF, b'I', b'J', b'K', b'L',
            b'M', b'N', b'O', b'P',
        ];
        let mut reader = LzssReader::new(&data[..]);
        let mut result = Vec::new();
        reader.read_to_end(&mut result).unwrap();
        assert_eq!(result, b"ABCDEFGHIJKLMNOP");
    }
}
