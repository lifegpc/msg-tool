use std::hash::Hasher;

const M: u32 = 0x5bd1e995;
const R: u32 = 24;

pub struct StreamingMurmur2 {
    h: u32,
    buf: [u8; 4],
    buf_len: usize,
}

impl StreamingMurmur2 {
    /// Create a hasher with already known size.
    pub fn new(seed: u32, total_len: u32) -> Self {
        let h = seed ^ total_len;
        Self {
            h,
            buf: [0; 4],
            buf_len: 0,
        }
    }
}

#[inline]
fn mix_block(mut h: u32, mut k: u32) -> u32 {
    k = k.wrapping_mul(M);
    k ^= k >> R;
    k = k.wrapping_mul(M);
    h = h.wrapping_mul(M);
    h ^= k;
    h
}

impl Hasher for StreamingMurmur2 {
    fn write(&mut self, mut bytes: &[u8]) {
        // Try process buf first
        if self.buf_len > 0 {
            let needed = 4 - self.buf_len;
            if bytes.len() >= needed {
                self.buf[self.buf_len..4].copy_from_slice(&bytes[..needed]);
                bytes = &bytes[needed..];

                // Process block
                let k = u32::from_le_bytes(self.buf);
                self.h = mix_block(self.h, k);
                self.buf_len = 0;
            } else {
                // Write to buffer is buffer len not enough
                self.buf[self.buf_len..self.buf_len + bytes.len()].copy_from_slice(bytes);
                self.buf_len += bytes.len();
                return;
            }
        }

        // Process blocks
        while bytes.len() >= 4 {
            let k = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            self.h = mix_block(self.h, k);
            bytes = &bytes[4..];
        }

        // Write to buffer
        if !bytes.is_empty() {
            self.buf[..bytes.len()].copy_from_slice(bytes);
            self.buf_len = bytes.len();
        }
    }

    fn finish(&self) -> u64 {
        let mut h = self.h;

        // Tail
        if self.buf_len > 0 {
            if self.buf_len >= 3 {
                h ^= (self.buf[2] as u32) << 16;
            }
            if self.buf_len >= 2 {
                h ^= (self.buf[1] as u32) << 8;
            }
            if self.buf_len >= 1 {
                h ^= self.buf[0] as u32;
            }
            h = h.wrapping_mul(M);
        }

        // Finalization
        h ^= h >> 13;
        h = h.wrapping_mul(M);
        h ^= h >> 15;

        h as u64
    }
}

pub struct Murmur2 {
    seed: u32,
    buf: Vec<u8>,
}

impl Murmur2 {
    pub fn new(seed: u32) -> Self {
        Self {
            seed,
            buf: Vec::new(),
        }
    }
}

impl Hasher for Murmur2 {
    fn write(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    fn finish(&self) -> u64 {
        let mut hasher = StreamingMurmur2::new(self.seed, self.buf.len() as u32);
        hasher.write(&self.buf);
        hasher.finish()
    }
}

#[test]
fn test_streaming_murmur2() {
    let mut hasher = StreamingMurmur2::new(0, 4);
    hasher.write(b"TEST");
    assert_eq!(hasher.finish(), 2297143075);
    hasher = StreamingMurmur2::new(0x300, 11);
    hasher.write(b"HELLO");
    hasher.write(b" WORLD");
    assert_eq!(hasher.finish(), 3206656488);
}
