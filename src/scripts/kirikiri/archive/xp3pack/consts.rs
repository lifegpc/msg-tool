/// XP3 file header signature: `XP3\r\n \n\x1a\x8b\x67\x01`
pub const XP3_MAGIC: &[u8; 11] = b"XP3\r\n \n\x1a\x8b\x67\x01";

// Chunk names
pub const CHUNK_FILE: &[u8; 4] = b"File";
pub const CHUNK_INFO: &[u8; 4] = b"info";
pub const CHUNK_SEGM: &[u8; 4] = b"segm";
pub const CHUNK_ADLR: &[u8; 4] = b"adlr";

// Index entry flags
pub const TVP_XP3_INDEX_ENCODE_METHOD_MASK: u8 = 0x07;
pub const TVP_XP3_INDEX_ENCODE_RAW: u8 = 0;
pub const TVP_XP3_INDEX_ENCODE_ZLIB: u8 = 1;
pub const TVP_XP3_INDEX_CONTINUE: u8 = 0x80;

// File entry flags
pub const TVP_XP3_FILE_PROTECTED: u32 = 1 << 31;

// Segment entry flags
pub const TVP_XP3_SEGM_ENCODE_METHOD_MASK: u32 = 0x07;
pub const TVP_XP3_SEGM_ENCODE_RAW: u32 = 0;
pub const TVP_XP3_SEGM_ENCODE_ZLIB: u32 = 1;
