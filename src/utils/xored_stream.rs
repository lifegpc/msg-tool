use std::io::{Read, Seek, Write};

pub struct XoredStream<T> {
    reader: T,
    key: u8,
}

impl<T> XoredStream<T> {
    pub fn new(reader: T, key: u8) -> Self {
        XoredStream { reader, key }
    }
}

impl<T: Read> Read for XoredStream<T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let read_bytes = self.reader.read(buf)?;
        for byte in &mut buf[..read_bytes] {
            *byte ^= self.key;
        }
        Ok(read_bytes)
    }
}

impl<T: Seek> Seek for XoredStream<T> {
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

impl<T: Write> Write for XoredStream<T> {
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

impl<T: std::fmt::Debug> std::fmt::Debug for XoredStream<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("XoredStream")
            .field("reader", &self.reader)
            .field("key", &self.key)
            .finish()
    }
}

/// A stream that XORs data with a repeating key based on the current position.
pub struct XoredKeyStream<T> {
    inner: T,
    key: Vec<u8>,
    base_position: u64,
}

impl<T> XoredKeyStream<T> {
    pub fn new(inner: T, key: Vec<u8>, base_position: u64) -> Self {
        XoredKeyStream {
            inner,
            key,
            base_position,
        }
    }
}

impl<T: Read + Seek> Read for XoredKeyStream<T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let key_len = self.key.len();
        let start_pos =
            ((self.inner.stream_position()? + self.base_position) % (key_len as u64)) as usize;
        let readed = self.inner.read(buf)?;
        for i in 0..readed {
            buf[i] ^= self.key[(start_pos + i) % key_len];
        }
        Ok(readed)
    }
}

impl<T: Seek> Seek for XoredKeyStream<T> {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(pos)
    }

    fn rewind(&mut self) -> std::io::Result<()> {
        self.inner.rewind()
    }

    fn stream_position(&mut self) -> std::io::Result<u64> {
        self.inner.stream_position()
    }
}

impl<T: Write + Seek> Write for XoredKeyStream<T> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let key_len = self.key.len();
        let start_pos =
            ((self.inner.stream_position()? + self.base_position) % (key_len as u64)) as usize;
        let mut encrypted_buf = buf.to_vec();
        for i in 0..buf.len() {
            encrypted_buf[i] ^= self.key[(start_pos + i) % key_len];
        }
        self.inner.write(&encrypted_buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for XoredKeyStream<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("XoredKeyStream")
            .field("inner", &self.inner)
            .field("base_position", &self.base_position)
            .finish()
    }
}
