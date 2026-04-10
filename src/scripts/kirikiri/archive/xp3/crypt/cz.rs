use super::*;
use aes::Aes128Dec;
use aes::cipher::{BlockDecryptMut, KeyIvInit};
use cbc::Decryptor;

type Aes128CbcDec = Decryptor<Aes128Dec>;

const CZ_MAGIC: &[u8; 4] = b"\xFD\xD7\x90\xA5";
const CZ_IV_SEED: u32 = 0xBFBFBFBF;
const CZ_HEADER_KEY: &[u8; 4] = b"\x9D\x1D\x9A\xF2";
const CZ_DEFAULT_KEY: &[u8] = b"\x91\x10\xfcuE\x8f\xb5\xe6\xfe\xac\xbaDvX\xc2\x1a";

fn cz_decrypt_int(data: &[u8], offset: usize, key: u8) -> u32 {
    let mut v: u32 = (data[offset] ^ key ^ CZ_HEADER_KEY[0]) as u32;
    v |= ((data[offset + 1] ^ key ^ CZ_HEADER_KEY[1]) as u32) << 8;
    v |= ((data[offset + 2] ^ key ^ CZ_HEADER_KEY[2]) as u32) << 16;
    v |= ((data[offset + 3] ^ key ^ CZ_HEADER_KEY[3]) as u32) << 24;
    v
}

fn cz_create_iv(seed: u32) -> [u8; 16] {
    let mut state = [0u32; 4];
    state[0] = 123456789;
    state[1] = 972436830;
    state[2] = 524018621;
    state[3] = seed;
    let mut iv = [0u8; 16];
    for i in 0..16 {
        let a = state[3];
        let b = state[0] ^ (state[0] << 11);
        state[0] = state[1];
        state[1] = state[2];
        state[2] = a;
        state[3] = b ^ a ^ ((b ^ (a >> 11)) >> 8);
        iv[i] = state[3] as u8;
    }
    iv
}

#[derive(Debug)]
struct AesDecryptor {
    aes: Aes128CbcDec,
    entry: StreamRegion<Entry>,
    pos: u64,
    original_size: u64,
}

impl AesDecryptor {
    fn new(
        aes: Aes128CbcDec,
        entry: StreamRegion<Entry>,
        original_size: u64,
    ) -> AlignedReader<16, Self> {
        AlignedReader::new(Self {
            aes,
            entry,
            pos: 0,
            original_size,
        })
    }
}

impl Read for AesDecryptor {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.entry.read_most(buf)?;
        if readed % 16 != 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "Not enough data to decrypt",
            ));
        }
        // NoPadding
        for i in (0..readed).step_by(16) {
            let block = &mut buf[i..i + 16];
            self.aes.decrypt_block_mut(block.into());
        }
        let remaining = self.original_size - self.pos;
        let readed = readed.min(remaining as usize);
        self.pos += readed as u64;
        Ok(readed)
    }
}

#[derive(Debug)]
pub struct KissCrypt {
    base: BaseSchema,
}

impl KissCrypt {
    pub fn new(base: BaseSchema) -> Self {
        Self { base }
    }
}

impl Crypt for KissCrypt {
    fn hash_after_crypt(&self) -> bool {
        self.base.hash_after_crypt
    }
    fn startup_tjs_not_encrypted(&self) -> bool {
        self.base.startup_tjs_not_encrypted
    }
    fn obfuscated_index(&self) -> bool {
        self.base.obfuscated_index
    }
    fn need_filter(&self, _filename: &str, buf: &[u8], buf_len: usize) -> bool {
        buf_len >= 4 && buf.starts_with(CZ_MAGIC)
    }
    fn filter(&self, mut entry: Entry) -> Result<Box<dyn ReadDebug>> {
        let mut header = [0u8; 15];
        entry.read_exact(&mut header)?;
        let typ = [header[4] ^ 0x11, header[5] ^ 0x7F, header[6] ^ 0x9A];
        let key = typ[0];
        let _unpacked_size = cz_decrypt_int(&header, 7, key);
        let packed_size = cz_decrypt_int(&header, 11, key);
        if (packed_size as u64) < entry.index.original_size && (packed_size - 5) & 0xF == 0 {
            let padded_size = packed_size - 5;
            let original_size = padded_size
                - (entry.peek_u8_at(15 + padded_size as u64 + 1)?
                    ^ entry.peek_u8_at(15 + padded_size as u64)?) as u32;
            let iv_seed = entry.peek_u32_at(15 + padded_size as u64 + 1)? ^ CZ_IV_SEED;
            let aes = Aes128CbcDec::new(CZ_DEFAULT_KEY.into(), &cz_create_iv(iv_seed).into());
            let entry = StreamRegion::with_size(entry, padded_size as u64)?;
            let stream = AesDecryptor::new(aes, entry, original_size as u64);
            if typ[0] == b'C' {
                let stream = flate2::read::ZlibDecoder::new(stream);
                return Ok(Box::new(stream));
            }
            Ok(Box::new(stream))
        } else {
            Ok(Box::new(entry))
        }
    }
    fn decrypt_supported(&self) -> bool {
        true
    }
    fn decrypt_seek_supported(&self) -> bool {
        true
    }
    fn decrypt<'a>(
        &self,
        entry: &Xp3Entry,
        cur_seg: &Segment,
        stream: Box<dyn Read + 'a>,
    ) -> Result<Box<dyn ReadDebug + 'a>> {
        let key = entry.file_hash ^ (entry.file_hash >> 19) ^ 0x4A9EEFF0;
        Ok(Box::new(KissCryptReader::new(stream, cur_seg, key)))
    }
    fn decrypt_with_seek<'a>(
        &self,
        entry: &Xp3Entry,
        cur_seg: &Segment,
        stream: Box<dyn ReadSeek + 'a>,
    ) -> Result<Box<dyn ReadSeek + 'a>> {
        let key = entry.file_hash ^ (entry.file_hash >> 19) ^ 0x4A9EEFF0;
        Ok(Box::new(KissCryptReader::new(stream, cur_seg, key)))
    }
}

impl<R: Read> Read for KissCryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let readed = self.inner.read(buf)?;
        let offset = self.seg_start + self.pos;
        let mut i = 0usize;
        while (i as u64 + offset) & 0xF != 0 {
            i += 1;
        }
        while i < readed {
            buf[i] ^= (self.key ^ (offset as u32 + i as u32)) as u8;
            i += 0x10;
        }
        self.pos += readed as u64;
        Ok(readed)
    }
}
