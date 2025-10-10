use adler::Adler32;
use std::io::{PipeReader, Read};

pub struct Reader {
    inner: PipeReader,
    adler: Adler32,
}

impl Reader {
    pub fn new(inner: PipeReader) -> Self {
        Self {
            inner,
            adler: Adler32::new(),
        }
    }

    pub fn into_checksum(self) -> u32 {
        self.adler.checksum()
    }
}

impl Read for Reader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.adler.write_slice(&buf[..n]);
        Ok(n)
    }
}
