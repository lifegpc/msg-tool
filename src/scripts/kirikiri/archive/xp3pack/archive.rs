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
