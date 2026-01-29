use crate::ext::io::*;
use crate::types::*;
use crate::utils::struct_pack::*;
use anyhow::Result;
use msg_tool_macro::{StructPack, StructUnpack};
use std::io::{Read, Seek, Write};

pub const HASH_VER_1_2_SIGNATURE: &[u8; 16] = b"HashVer1.2\x00\x00\x00\x00\x00\x00";
pub const HASH_VER_1_3_SIGNATURE: &[u8; 16] = b"HashVer1.3\x00\x00\x00\x00\x00\x00";
pub const HASH_VER_1_4_SIGNATURE: &[u8; 16] = b"HashVer1.4\x00\x00\x00\x00\x00\x00";

/// HashVer 1.2
#[derive(StructPack, StructUnpack, Debug, Clone)]
pub struct QlieHash12 {
    pub signature: [u8; 16],
    /// Always 0x200
    pub const_: u32,
    pub file_count: u32,
    pub index_size: u32,
    #[pvec(u32)]
    pub hash_data: Vec<u8>,
}

/// HashVer 1.3
#[derive(StructPack, StructUnpack, Debug, Clone)]
pub struct QlieHash13 {
    pub signature: [u8; 16],
    /// Always 0x100
    pub const_: u32,
    pub file_count: u32,
    pub index_size: u32,
    #[pvec(u32)]
    pub hash_data: Vec<u8>,
}

/// HashVer 1.4
#[derive(StructPack, StructUnpack, Debug, Clone)]
pub struct QlieHash14 {
    pub signature: [u8; 16],
    /// Always 0x100
    pub const_: u32,
    pub file_count: u32,
    pub index_size: u32,
    pub hash_data_size: u32,
    pub is_compressed: u32,
    pub unk: [u8; 32],
    #[unpack_vec_len(hash_data_size)]
    #[pack_vec_len(self.hash_data_size)]
    pub hash_data: Vec<u8>,
}

#[derive(StructPack, StructUnpack, Debug, Clone)]
pub struct QlieHeader {
    pub signature: [u8; 16],
    pub file_count: u32,
    pub index_offset: u64,
}

impl QlieHeader {
    pub fn is_valid(&self) -> bool {
        self.signature.starts_with(b"FilePackVer")
            && self.signature[12] == b'.'
            && &self.signature[14..] == b"\x00\x00"
            && self.signature[11].is_ascii_digit()
            && self.signature[13].is_ascii_digit()
    }

    pub fn major_version(&self) -> u8 {
        self.signature[11] - b'0'
    }

    pub fn minor_version(&self) -> u8 {
        self.signature[13] - b'0'
    }
}

#[derive(StructPack, StructUnpack, Debug, Clone)]
pub struct QlieKey {
    pub signature: [u8; 32],
    pub hash_size: u32,
    pub key: [u8; 0x400],
}

#[derive(Debug, Clone)]
pub struct QlieEntry {
    pub name: String,
    pub offset: u64,
    pub size: u32,
    pub unpacked_size: u32,
    pub is_packed: u32,
    pub is_encrypted: u32,
    pub hash: u32,
    pub key: u32,
    pub common_key: Option<Vec<u8>>,
}
