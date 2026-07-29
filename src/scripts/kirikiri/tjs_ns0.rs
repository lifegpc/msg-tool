//! Kirikiri TJS NS0 binary encoded script
//!
//! Pipeline:
//!   header → [4s0 only: crypt + framed LZ4] → TypeByte Variant SM (+ trailing u32)
//!
//! `TJS/4s0` only adds filter layers; after unwrap it rejoins the same Variant parse
//! as ns0. PackinOne TypeByte (cedb0) is used for 4s0 because upstream `ByteChecker`
//! does not match encrypted-pack seeds (verified on 由月a).
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::{decode_to_string, encode_string};
use crate::utils::struct_pack::*;
use anyhow::Result;
use msg_tool_macro::*;
use overf::wrapping;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{Read, Seek, Write};

/// Shared TypeByte hook: ns0 keeps [`ByteChecker`]; 4s0 uses PackinOne engine checker.
trait TypeCheck {
    fn get_seed(&mut self, type_code: u8) -> u8;
}

#[derive(Debug)]
/// Kirikiri TJS NS0 Script Builder
pub struct TjsNs0Builder {}

impl TjsNs0Builder {
    /// Creates a new instance of `TjsNs0Builder`
    pub fn new() -> Self {
        Self {}
    }
}

impl ScriptBuilder for TjsNs0Builder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Utf16LE
    }

    fn build_script(
        &self,
        buf: Vec<u8>,
        filename: &str,
        encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script + Send + Sync>> {
        Ok(Box::new(TjsNs0::new(buf, filename, encoding, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["pbd", "tjs"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::KirikiriTjsNs0
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 8 && (buf.starts_with(b"TJS/ns0\0") || buf.starts_with(b"TJS/4s0\0")) {
            return Some(100);
        }
        None
    }

    fn can_create_file(&self) -> bool {
        true
    }

    fn create_file<'a>(
        &'a self,
        filename: &'a str,
        mut writer: Box<dyn WriteSeek + 'a>,
        encoding: Encoding,
        file_encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<()> {
        let s = crate::utils::files::read_file(filename)?;
        let s = decode_to_string(file_encoding, &s, true)?;
        let data: TjsValue = if config.custom_yaml {
            serde_yaml_ng::from_str(&s)?
        } else {
            serde_json::from_str(&s)?
        };
        let header = Header {
            magic: *b"TJS/",
            check: *b"ns0\0",
            seed: u32::from_le_bytes(*b"TJS\0"),
            crypt: 0,
            iv_len: 0,
        };
        let mut checker = ByteChecker::new(header.seed);
        header.pack(&mut writer, false, encoding, &None)?;
        data.pack(&mut checker, &mut writer, false, encoding)?;
        let checksum = checker.final_check();
        writer.write_u32(checksum)?;
        writer.flush()?;
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum TjsValue {
    Void(()),
    Int(i64),
    Double(f64),
    Str(String),
    Array(Vec<TjsValue>),
    Dict(BTreeMap<String, TjsValue>),
}

fn unpack_string<R: Read + Seek>(reader: &mut R, big: bool, encoding: Encoding) -> Result<String> {
    let len = u32::unpack(reader, big, encoding, &None)? as usize;
    let tlen = if encoding.is_utf16le() { len * 2 } else { len };
    let mut buf = vec![0u8; tlen];
    reader.read_exact(&mut buf)?;
    let s = decode_to_string(encoding, &buf, true)?;
    Ok(s)
}

fn pack_string<W: Write>(s: &str, writer: &mut W, big: bool, encoding: Encoding) -> Result<()> {
    let encoded = encode_string(encoding, s, false)?;
    let len = if encoding.is_utf16le() {
        (encoded.len() / 2) as u32
    } else {
        encoded.len() as u32
    };
    len.pack(writer, big, encoding, &None)?;
    writer.write_all(&encoded)?;
    Ok(())
}

impl TjsValue {
    fn pack<W: Write, C: TypeCheck>(
        &self,
        checker: &mut C,
        writer: &mut W,
        big: bool,
        encoding: Encoding,
    ) -> Result<()> {
        match self {
            Self::Void(()) => {
                let typ_byte = 0;
                let check_byte = checker.get_seed(typ_byte);
                let typ = ((check_byte as u16) << 8) | (typ_byte as u16);
                typ.pack(writer, big, encoding, &None)?;
            }
            Self::Str(s) => {
                let typ_byte = 2;
                let check_byte = checker.get_seed(typ_byte);
                let typ = ((check_byte as u16) << 8) | (typ_byte as u16);
                typ.pack(writer, big, encoding, &None)?;
                pack_string(s, writer, big, encoding)?;
            }
            Self::Int(i) => {
                let typ_byte = 4;
                let check_byte = checker.get_seed(typ_byte);
                let typ = ((check_byte as u16) << 8) | (typ_byte as u16);
                typ.pack(writer, big, encoding, &None)?;
                i.pack(writer, big, encoding, &None)?;
            }
            Self::Double(f) => {
                let typ_byte = 5;
                let check_byte = checker.get_seed(typ_byte);
                let typ = ((check_byte as u16) << 8) | (typ_byte as u16);
                typ.pack(writer, big, encoding, &None)?;
                f.pack(writer, big, encoding, &None)?;
            }
            Self::Array(arr) => {
                let typ_byte = 0x81;
                let check_byte = checker.get_seed(typ_byte);
                let typ = ((check_byte as u16) << 8) | (typ_byte as u16);
                typ.pack(writer, big, encoding, &None)?;
                let arr_len = arr.len() as u32;
                arr_len.pack(writer, big, encoding, &None)?;
                for item in arr {
                    item.pack(checker, writer, big, encoding)?;
                }
            }
            Self::Dict(dict) => {
                let typ_byte = 0xC1;
                let check_byte = checker.get_seed(typ_byte);
                let typ = ((check_byte as u16) << 8) | (typ_byte as u16);
                typ.pack(writer, big, encoding, &None)?;
                let dict_len = dict.len() as u32;
                dict_len.pack(writer, big, encoding, &None)?;
                for (key, value) in dict {
                    pack_string(key, writer, big, encoding)?;
                    value.pack(checker, writer, big, encoding)?;
                }
            }
        }
        Ok(())
    }

    fn unpack<R: Read + Seek, C: TypeCheck>(
        checker: &mut C,
        reader: &mut R,
        big: bool,
        encoding: Encoding,
    ) -> Result<Self> {
        let typ = u16::unpack(reader, big, encoding, &None)?;
        let typ_byte = (typ & 0xff) as u8;
        let check_byte = (typ >> 8) as u8;
        let expected_check = checker.get_seed(typ_byte);
        if check_byte != expected_check {
            return Err(anyhow::anyhow!(
                "TJS/ns0 byte check failed: expected {}, got {} at pos {}",
                expected_check,
                check_byte,
                reader.stream_position()? - 1
            ));
        }
        Ok(match typ_byte {
            0 => TjsValue::Void(()),
            2 => TjsValue::Str(unpack_string(reader, big, encoding)?),
            4 => TjsValue::Int(i64::unpack(reader, big, encoding, &None)?),
            5 => TjsValue::Double(f64::unpack(reader, big, encoding, &None)?),
            0x81 => {
                let arr_len = u32::unpack(reader, big, encoding, &None)? as usize;
                let mut arr = Vec::with_capacity(arr_len);
                for _ in 0..arr_len {
                    arr.push(TjsValue::unpack(checker, reader, big, encoding)?);
                }
                TjsValue::Array(arr)
            }
            0xC1 => {
                let kv_len = u32::unpack(reader, big, encoding, &None)? as usize;
                let mut dict = BTreeMap::new();
                for _ in 0..kv_len {
                    let key = unpack_string(reader, big, encoding)?;
                    let value = TjsValue::unpack(checker, reader, big, encoding)?;
                    dict.insert(key, value);
                }
                TjsValue::Dict(dict)
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Unsupported TJS/ns0 value type: {} at pos {}",
                    typ_byte,
                    reader.stream_position()? - 2
                ));
            }
        })
    }
}

#[derive(Debug)]
/// Kirikiri TJS NS0 Script
pub struct TjsNs0 {
    data: TjsValue,
    custom_yaml: bool,
    header: Header,
}

struct ByteChecker {
    seed: u32,
}

impl ByteChecker {
    pub fn new(seed: u32) -> Self {
        Self { seed }
    }

    fn calculate_round(seed: &mut [u8; 4]) {
        let a = seed[0] ^ wrapping!(seed[0] * 2);
        let mut b = a;
        wrapping! {
        b >>= 2;
        b ^= seed[2];
        b >>= 3;
        b ^= seed[2];
        b ^= a;
        }

        seed[0] = seed[1];
        seed[1] = seed[2];
        seed[2] = b;
    }

    pub fn get_seed(&mut self, type_code: u8) -> u8 {
        let mut s = self.seed.to_le_bytes();
        if type_code == 0 {
            return s[2];
        }
        Self::calculate_round(&mut s);
        self.seed = u32::from_le_bytes(s);
        return s[2];
    }

    pub fn final_check(&mut self) -> u32 {
        let mut s = self.seed.to_le_bytes();
        Self::calculate_round(&mut s);
        Self::calculate_round(&mut s);
        Self::calculate_round(&mut s);
        let tmp = s[0];
        s[0] = s[2];
        s[2] = tmp;
        u32::from_le_bytes(s)
    }
}

impl TypeCheck for ByteChecker {
    fn get_seed(&mut self, type_code: u8) -> u8 {
        ByteChecker::get_seed(self, type_code)
    }
}

#[derive(Clone, Debug, StructPack, StructUnpack)]
struct Header {
    magic: [u8; 4],
    check: [u8; 4],
    seed: u32,
    crypt: u16,
    iv_len: u16,
}

impl TjsNs0 {
    /// Creates a new `TjsNs0` script from the given buffer and filename
    ///
    /// * `buf` - The buffer containing the TJS/ns0 data
    /// * `filename` - The name of the file
    /// * `encoding` - The encoding to use for strings
    /// * `config` - Extra configuration options
    pub fn new(
        buf: Vec<u8>,
        _filename: &str,
        encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<Self> {
        let mut reader = MemReader::new(buf);
        let header = Header::unpack(&mut reader, false, encoding, &None)?;
        if &header.magic != b"TJS/" {
            return Err(anyhow::anyhow!("Not a valid TJS/ns0 file"));
        }
        if header.check[1] != b's' || header.check[2] != b'0' || header.check[3] != 0 {
            return Err(anyhow::anyhow!("Not a valid TJS/ns0 file"));
        }

        // Filter stage only:
        //   '4' → optional crypt + framed LZ4  (side path, then rejoin)
        //   'n' → upstream ns0 constraints, raw body
        // Variant SM + trailing check below is shared.
        let (mut reader, is_4s0) = match header.check[0] {
            b'4' => (Self::unwrap_4s0_filters(reader, &header)?, true),
            b'n' => {
                if header.crypt != 0 {
                    return Err(anyhow::anyhow!("Encrypted TJS/ns0 files are not supported"));
                }
                if header.iv_len != 0 {
                    return Err(anyhow::anyhow!("TJS/ns0 files with IV are not supported"));
                }
                (reader, false)
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Unsupported compression method in TJS/ns0 file"
                ));
            }
        };

        // ---- shared Variant parse (same as upstream ns0 after filters) ----
        // PackinOne 4s0 TypeByte uses engine cedb0/d6ec0, not upstream ByteChecker.
        // ns0 samples match both; 4s0 plaintext fails upstream ByteChecker on tag bytes.
        let data = if is_4s0 {
            let mut checker = four_s0::PackinOneTypeByteChecker::from_seed(header.seed);
            let data = TjsValue::unpack(&mut checker, &mut reader, false, encoding)?;
            // Trailing u32 present; engine trailer != upstream final_check algorithm.
            let _ = reader.read_u32();
            data
        } else {
            let mut checker = ByteChecker::new(header.seed);
            let data = TjsValue::unpack(&mut checker, &mut reader, false, encoding)?;
            let expected_checksum = checker.final_check();
            let actual_checksum = reader.read_u32()?;
            if expected_checksum != actual_checksum {
                return Err(anyhow::anyhow!(
                    "TJS/ns0 checksum mismatch: expected {:08X}, got {:08X}",
                    expected_checksum,
                    actual_checksum
                ));
            }
            data
        };

        Ok(Self {
            data,
            custom_yaml: config.custom_yaml,
            header,
        })
    }

    /// 4s0 filter unwrap only: read optional IV, decrypt if `crypt != 0`, framed LZ4.
    /// Returns a reader positioned at the plaintext Variant stream (same stage as ns0 body).
    fn unwrap_4s0_filters(mut reader: MemReader, header: &Header) -> Result<MemReader> {
        let iv = if header.iv_len != 0 {
            let n = header.iv_len as usize;
            if reader.pos + n > reader.data.len() {
                return Err(anyhow::anyhow!("TJS/4s0 IV truncated"));
            }
            let v = reader.data[reader.pos..reader.pos + n].to_vec();
            reader.pos += n;
            v
        } else {
            Vec::new()
        };

        let mut body = reader.data[reader.pos..].to_vec();
        if header.crypt != 0 {
            body = four_s0::stream_crypt_decrypt(&body, header.seed, &iv, header.crypt)?;
        }
        let plain = four_s0::lz4_stream_decompress(&body)?;
        Ok(MemReader::new(plain))
    }
}

impl Script for TjsNs0 {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Custom
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn is_output_supported(&self, output: OutputScriptType) -> bool {
        matches!(output, OutputScriptType::Custom)
    }

    fn custom_output_extension<'a>(&'a self) -> &'a str {
        if self.custom_yaml { "yaml" } else { "json" }
    }

    fn custom_export(&self, filename: &std::path::Path, encoding: Encoding) -> Result<()> {
        let s = if self.custom_yaml {
            serde_yaml_ng::to_string(&self.data)?
        } else {
            serde_json::to_string_pretty(&self.data)?
        };
        let s = encode_string(encoding, &s, false)?;
        let mut writer = crate::utils::files::write_file(filename)?;
        writer.write_all(&s)?;
        Ok(())
    }

    fn custom_import<'a>(
        &'a self,
        custom_filename: &'a str,
        mut file: Box<dyn WriteSeek + 'a>,
        encoding: Encoding,
        output_encoding: Encoding,
    ) -> Result<()> {
        let s = crate::utils::files::read_file(custom_filename)?;
        let s = decode_to_string(output_encoding, &s, true)?;
        let data: TjsValue = if self.custom_yaml {
            serde_yaml_ng::from_str(&s)?
        } else {
            serde_json::from_str(&s)?
        };
        let mut header = self.header.clone();
        header.check = *b"ns0\0";
        let mut checker = ByteChecker::new(header.seed);
        header.pack(&mut file, false, encoding, &None)?;
        data.pack(&mut checker, &mut file, false, encoding)?;
        let checksum = checker.final_check();
        file.write_u32(checksum)?;
        file.flush()?;
        Ok(())
    }
}


// =============================================================================
// 4s0 filter helpers only (crypt + framed LZ4). Variant parse rejoins `new()` above.
// =============================================================================

/// Isolated 4s0 crypt / LZ4 helpers (does not replace ns0 ByteChecker).
mod four_s0 {
    use super::TypeCheck;
    use anyhow::Result;
    use blake2::digest::{KeyInit, Mac};
    use blake2::Blake2sMac256;
    use std::os::raw::{c_char, c_int};

    /// PackinOne `FUN_101cedb0` / `FUN_101d6ec0` TypeByte checker.
    pub struct PackinOneTypeByteChecker {
        b0: u8,
        b1: u8,
        b2: u8,
    }

    impl PackinOneTypeByteChecker {
        pub fn from_seed(seed: u32) -> Self {
            Self {
                b0: ((seed >> 24) as u8) ^ (seed as u8),
                b1: (seed >> 8) as u8,
                b2: (seed >> 16) as u8,
            }
        }

        fn step(&mut self) -> u8 {
            let t = self.b0 ^ self.b0.wrapping_shl(1);
            self.b0 = self.b1;
            self.b1 = self.b2;
            self.b2 = self.b2 ^ (self.b2 >> 3) ^ t ^ (t >> 5);
            self.b2
        }

        fn expect_for_tag(&mut self, tag: u8) -> u8 {
            if tag == 0 {
                self.b2
            } else {
                self.step()
            }
        }
    }

    impl TypeCheck for PackinOneTypeByteChecker {
        fn get_seed(&mut self, type_code: u8) -> u8 {
            self.expect_for_tag(type_code)
        }
    }

    /// `b1330` switch(field-1) -> (ChaCha rounds p3, expand p4).
    fn crypt_params(field: u16) -> Result<(u32, u32)> {
        Ok(match field {
            1 => (8, 0x10),
            2 => (0xC, 8),
            3 => (0x14, 4),
            4 => (8, 1),
            5 => (0xC, 1),
            6 => (0x14, 1),
            _ => {
                return Err(anyhow::anyhow!(
                    "Unsupported TJS datapack crypt field: {field}"
                ))
            }
        })
    }

    pub fn derive_key(seed: u32, iv: &[u8]) -> Result<[u8; 32]> {
        let mut mac = <Blake2sMac256 as KeyInit>::new_from_slice(&seed.to_le_bytes())
            .map_err(|e| anyhow::anyhow!("blake2s key init: {e}"))?;
        Mac::update(&mut mac, iv);
        let out = Mac::finalize(mac).into_bytes();
        let mut key = [0u8; 32];
        key.copy_from_slice(&out);
        Ok(key)
    }

    fn quarter_round(s: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
        s[a] = s[a].wrapping_add(s[b]);
        s[d] = (s[d] ^ s[a]).rotate_left(16);
        s[c] = s[c].wrapping_add(s[d]);
        s[b] = (s[b] ^ s[c]).rotate_left(12);
        s[a] = s[a].wrapping_add(s[b]);
        s[d] = (s[d] ^ s[a]).rotate_left(8);
        s[c] = s[c].wrapping_add(s[d]);
        s[b] = (s[b] ^ s[c]).rotate_left(7);
    }

    fn chacha_block(
        key: &[u8; 32],
        nonce_lo: u32,
        nonce_hi: u32,
        block_counter: u64,
        rounds: u32,
    ) -> [u8; 64] {
        let mut st = [0u32; 16];
        st[0] = 0x6170_7865;
        st[1] = 0x3320_646e;
        st[2] = 0x7962_2d32;
        st[3] = 0x6b20_6574;
        for i in 0..8 {
            st[4 + i] = u32::from_le_bytes(key[i * 4..i * 4 + 4].try_into().unwrap());
        }
        st[12] = block_counter as u32;
        st[13] = (block_counter >> 32) as u32;
        st[14] = nonce_lo;
        st[15] = nonce_hi;
        let mut x = st;
        let mut r = rounds;
        while r > 0 {
            quarter_round(&mut x, 0, 4, 8, 12);
            quarter_round(&mut x, 1, 5, 9, 13);
            quarter_round(&mut x, 2, 6, 10, 14);
            quarter_round(&mut x, 3, 7, 11, 15);
            quarter_round(&mut x, 0, 5, 10, 15);
            quarter_round(&mut x, 1, 6, 11, 12);
            quarter_round(&mut x, 2, 7, 8, 13);
            quarter_round(&mut x, 3, 4, 9, 14);
            r -= 2;
        }
        let mut out = [0u8; 64];
        for i in 0..16 {
            out[i * 4..i * 4 + 4].copy_from_slice(&x[i].wrapping_add(st[i]).to_le_bytes());
        }
        out
    }

    fn xorshift32(mut x: u32, fallback: u32) -> u32 {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        if x == 0 {
            fallback
        } else {
            x
        }
    }

    pub fn gen_ks_buffer(
        key: &[u8; 32],
        nonce_lo: u32,
        nonce_hi: u32,
        block_counter: u64,
        p4: u32,
        rounds: u32,
        fallback: u32,
    ) -> Vec<u8> {
        let block = chacha_block(key, nonce_lo, nonce_hi, block_counter, rounds);
        if p4 <= 1 {
            return block.to_vec();
        }
        let mut words = Vec::with_capacity((p4 as usize) * 16);
        for i in 0..16 {
            words.push(u32::from_le_bytes(
                block[i * 4..i * 4 + 4].try_into().unwrap(),
            ));
        }
        let mut src_i = 0usize;
        let extra_groups = (p4 as usize) * 4 - 4;
        for _ in 0..extra_groups {
            for _k in 0..4 {
                let w = words[src_i];
                src_i += 1;
                words.push(xorshift32(w, fallback));
            }
        }
        let mut out = Vec::with_capacity(words.len() * 4);
        for w in words {
            out.extend_from_slice(&w.to_le_bytes());
        }
        out
    }

    pub fn stream_crypt_decrypt(
        body: &[u8],
        seed: u32,
        iv: &[u8],
        field: u16,
    ) -> Result<Vec<u8>> {
        let (rounds, p4) = crypt_params(field)?;
        let key = derive_key(seed, iv)?;
        let nonce_lo = xxhash_rust::xxh32::xxh32(iv, seed);
        let nonce_hi = seed;
        let mut fallback = nonce_hi ^ nonce_lo;
        if fallback == 0 {
            fallback = if seed != 0 { seed } else { 0xffff_ffff };
        }
        let mut out = Vec::with_capacity(body.len());
        let mut produced = 0usize;
        let mut bc = 0u64;
        while produced < body.len() {
            let ks = gen_ks_buffer(&key, nonce_lo, nonce_hi, bc, p4, rounds, fallback);
            let n = ks.len().min(body.len() - produced);
            for i in 0..n {
                out.push(body[produced + i] ^ ks[i]);
            }
            produced += n;
            bc += 1;
        }
        Ok(out)
    }

    unsafe extern "C" {
        fn LZ4_decompress_safe_usingDict(
            source: *const c_char,
            dest: *mut c_char,
            compressed_size: c_int,
            max_decompressed_size: c_int,
            dict_start: *const c_char,
            dict_size: c_int,
        ) -> c_int;
    }

    fn lz4_decompress_block(chunk: &[u8], dict: &[u8], prefer_us: i32) -> Option<Vec<u8>> {
        let mut ordered = vec![
            prefer_us,
            2048,
            1024,
            512,
            256,
            (chunk.len() as i32).saturating_mul(4),
            (chunk.len() as i32).saturating_mul(8),
            1 << 16,
        ];
        let mut seen = std::collections::BTreeSet::new();
        ordered.retain(|s| *s > 0 && seen.insert(*s));

        for us in ordered {
            let mut buf = vec![0u8; us as usize];
            let ret = if dict.is_empty() {
                unsafe {
                    lz4::liblz4::LZ4_decompress_safe(
                        chunk.as_ptr() as *const c_char,
                        buf.as_mut_ptr() as *mut c_char,
                        chunk.len() as c_int,
                        us,
                    )
                }
            } else {
                unsafe {
                    LZ4_decompress_safe_usingDict(
                        chunk.as_ptr() as *const c_char,
                        buf.as_mut_ptr() as *mut c_char,
                        chunk.len() as c_int,
                        us,
                        dict.as_ptr() as *const c_char,
                        dict.len() as c_int,
                    )
                }
            };
            if ret > 0 {
                buf.truncate(ret as usize);
                return Some(buf);
            }
        }
        None
    }

    /// `[u16 csize][LZ4 block]*` with dict = prior output (max 64KiB).
    pub fn lz4_stream_decompress(data: &[u8]) -> Result<Vec<u8>> {
        if data.is_empty() {
            return Ok(Vec::new());
        }
        let mut pos = 0usize;
        let mut out = Vec::new();
        while pos + 2 <= data.len() {
            let n = u16::from_le_bytes(data[pos..pos + 2].try_into().unwrap()) as usize;
            if n == 0 {
                break;
            }
            if pos + 2 + n > data.len() {
                break;
            }
            let chunk = &data[pos + 2..pos + 2 + n];
            let dict: &[u8] = if out.len() > 65536 {
                &out[out.len() - 65536..]
            } else if out.is_empty() {
                &[]
            } else {
                &out[..]
            };
            let got = lz4_decompress_block(chunk, dict, 4096).or_else(|| {
                if !dict.is_empty() {
                    lz4_decompress_block(chunk, &[], 4096)
                } else {
                    None
                }
            });
            let Some(block) = got else {
                break;
            };
            out.extend_from_slice(&block);
            pos += 2 + n;
        }
        if out.is_empty() {
            if let Ok(d) = lz4::block::decompress(data, Some(1 << 20)) {
                return Ok(d);
            }
            if data.len() > 2 {
                if let Ok(d) = lz4::block::decompress(&data[2..], Some(1 << 20)) {
                    return Ok(d);
                }
            }
            return Err(anyhow::anyhow!(
                "Failed to decompress TJS/4s0 LZ4 stream (consumed={pos}, len={})",
                data.len()
            ));
        }
        Ok(out)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn to_hex(data: &[u8]) -> String {
            data.iter().map(|b| format!("{b:02x}")).collect()
        }

        fn from_hex(s: &str) -> Vec<u8> {
            (0..s.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
                .collect()
        }

        #[test]
        fn yuzuki_empty_iv_key_schedule() {
            let seed = 0x1435_3CC6u32;
            let key = derive_key(seed, b"").unwrap();
            assert_eq!(
                to_hex(&key),
                "2fca145d50b8cb030395f0868c49ed85aa0280c6d0b844c09673351e64408aed"
            );
            let xx = xxhash_rust::xxh32::xxh32(b"", seed);
            assert_eq!(xx, 0x9848_23D3);
            let ks = gen_ks_buffer(&key, xx, seed, 0, 0x10, 8, seed ^ xx);
            assert_eq!(&ks[..16], &from_hex("2e9d7adaabc57ee65d0d4f2b440895d5")[..]);
        }
    }
}
