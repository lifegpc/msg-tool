use std::io::{Read, Seek, Write};

pub struct Crypto<T> {
    reader: T,
    key: u8,
}

impl<T> Crypto<T> {
    pub fn new(reader: T, key: u8) -> Self {
        Crypto { reader, key }
    }
}

impl<T: Read> Read for Crypto<T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let read_bytes = self.reader.read(buf)?;
        for byte in &mut buf[..read_bytes] {
            *byte ^= self.key;
        }
        Ok(read_bytes)
    }
}

impl<T: Seek> Seek for Crypto<T> {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        self.reader.seek(pos)
    }

    fn rewind(&mut self) -> std::io::Result<()> {
        self.reader.rewind()
    }

    fn stream_position(&mut self) -> std::io::Result<u64> {
        self.reader.stream_position()
    }
}

impl<T: Write> Write for Crypto<T> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut encrypted_buf = buf.to_vec();
        for byte in &mut encrypted_buf {
            *byte ^= self.key;
        }
        self.reader.write(&encrypted_buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.reader.flush()
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for Crypto<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Crypto")
            .field("reader", &self.reader)
            .field("key", &self.key)
            .finish()
    }
}
