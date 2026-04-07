use super::consts::*;
use super::crypt::Crypt;
use crate::scripts::base::ReadSeek;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex};

/// Represents a single data segment for a file.
/// A file can be split into multiple segments, which can be compressed independently.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Segment {
    pub is_compressed: bool,
    /// The offset of the segment's data within the archive file.
    pub start: u64,
    /// The offset of this segment within the original, uncompressed file.
    pub offset_in_file: u64,
    /// The size of the segment after decompression.
    pub original_size: u64,
    /// The size of the segment in the archive (potentially compressed).
    pub archived_size: u64,
}

/// Represents a single file entry within the XP3 archive.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ArchiveItem {
    pub name: String,
    pub file_hash: u32,
    pub original_size: u64,
    pub archived_size: u64,
    pub segments: Vec<Segment>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Xp3Entry {
    pub name: String,
    pub flags: u32,
    pub file_hash: u32,
    pub original_size: u64,
    pub archived_size: u64,
    pub timestamp: Option<u64>,
    pub segments: Vec<Segment>,
    pub extras: Vec<ExtraProp>,
}

impl Xp3Entry {
    pub fn is_encrypted(&self) -> bool {
        self.flags != 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExtraProp {
    pub tag: PropTag,
    pub data: Vec<u8>,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PropTag {
    tag: [u8; 4],
}

impl std::fmt::Debug for PropTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", bytes::Bytes::copy_from_slice(&self.tag))
    }
}

impl Deref for PropTag {
    type Target = [u8; 4];

    fn deref(&self) -> &Self::Target {
        &self.tag
    }
}

impl DerefMut for PropTag {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.tag
    }
}

impl From<[u8; 4]> for PropTag {
    fn from(value: [u8; 4]) -> Self {
        PropTag { tag: value }
    }
}

impl PartialEq<&[u8; 4]> for PropTag {
    fn eq(&self, other: &&[u8; 4]) -> bool {
        &self.tag == *other
    }
}

impl ExtraProp {
    pub fn is_filename_hash(&self) -> bool {
        self.tag == CHUNK_HNFN
    }
}

/// Represents the entire XP3 archive
#[derive(Debug)]
#[allow(dead_code)]
pub struct Xp3Archive {
    pub inner: Arc<Mutex<Box<dyn ReadSeek>>>,
    pub crypt: Arc<Box<dyn Crypt>>,
    /// The offset which the archive file start. If the archive is embedded in another file (such as exe), this is the offset of the archive data within the larger file.
    pub base_offset: u64,
    /// The offset which index start. Releatived to whole file not just xp3 archive.
    pub index_offset: u64,
    /// Minor version
    pub minor_version: u32,
    pub entries: Vec<Xp3Entry>,
    pub extras: Vec<ExtraProp>,
}
