use super::archive::*;
use super::consts::*;
use super::crypt::*;
use crate::ext::io::*;
use crate::types::*;
use anyhow::Result;
use std::io::{Read, Seek, SeekFrom};
use std::sync::{Arc, Mutex};

impl<'a> Xp3Archive<'a> {
    pub fn new<T: Read + Seek + std::fmt::Debug + 'a>(
        stream: T,
        config: &ExtraConfig,
        filename: &str,
    ) -> Result<Self> {
        let crypt: Box<dyn Crypt> = if let Some(game_title) = &config.xp3_game_title {
            query_crypt_schema(game_title)
                .ok_or_else(|| {
                    anyhow::anyhow!("Unsupported game title for XP3 archive: {}", game_title)
                })?
                .create_crypt(filename, config)?
        } else {
            Box::new(NoCrypt::new())
        };
        let mut stream = Box::new(stream);
        let base_offset = 0;
        if base_offset != 0 {
            stream.seek(SeekFrom::Start(base_offset))?;
        }
        stream
            .read_and_equal(XP3_MAGIC)
            .map_err(|e| anyhow::anyhow!("Invalid xp3 signature: {}", e))?;
        let mut index_offset = stream.read_u64()?;
        let mut minor_version = 0;
        if index_offset == TVP_XP3_CURRENT_HEADER_VERSION {
            minor_version = stream.read_u32()?;
            let sig = stream.read_u8()?;
            if sig != TVP_XP3_INDEX_CONTINUE {
                anyhow::bail!("Unsupported XP3 index format: {} is not continue flag", sig);
            }
            let index_offset_offset = stream.read_i64()?;
            if index_offset_offset != 0 {
                stream.seek_relative(index_offset_offset)?;
            }
            index_offset = stream.read_u64()?;
        }
        index_offset += base_offset;
        stream.seek(SeekFrom::Start(index_offset))?;
        let mut entries = Vec::new();
        let mut extras = Vec::new();
        {
            let mut index_stream = Self::get_index_stream(&mut stream)?;
            let mut sig = [0u8; 4];
            loop {
                let readed = index_stream.read_most(&mut sig)?;
                if readed == 0 {
                    break;
                }
                if readed < 4 {
                    anyhow::bail!("Invalid chunk signature in index");
                }
                let mut size = index_stream.read_u64()?;
                if &sig == CHUNK_FILE {
                    let mut name = None;
                    let mut flags = None;
                    let mut file_hash = None;
                    let mut original_size = None;
                    let mut archived_size = None;
                    let mut timestamp = None;
                    let mut segments = Vec::new();
                    let mut seg_offset = 0;
                    let mut entry_extras = Vec::new();
                    while size > 0 {
                        if size < 12 {
                            anyhow::bail!("Invalid chunk size in index");
                        }
                        let mut chunk_sig = [0u8; 4];
                        index_stream.read_exact(&mut chunk_sig)?;
                        let mut chunk_size = index_stream.read_u64()?;
                        size -= 12;
                        if size < chunk_size {
                            anyhow::bail!("Invalid chunk size in index");
                        }
                        size -= chunk_size;
                        if &chunk_sig == CHUNK_INFO {
                            if chunk_size < 20 {
                                anyhow::bail!("Invalid info chunk size in index");
                            }
                            flags = Some(index_stream.read_u32()?);
                            original_size = Some(index_stream.read_u64()?);
                            archived_size = Some(index_stream.read_u64()?);
                            chunk_size -= 20;
                            let (n, s) = crypt.read_name(&mut index_stream)?;
                            name = Some(n);
                            chunk_size -= s;
                        } else if &chunk_sig == CHUNK_ADLR {
                            if chunk_size == 4 {
                                file_hash = Some(index_stream.read_u32()?);
                                chunk_size -= 4;
                            }
                        } else if &chunk_sig == CHUNK_SEGM {
                            while chunk_size > 0 {
                                if chunk_size < 0x1C {
                                    anyhow::bail!("Invalid segm chunk size in index");
                                }
                                let seg_flags = index_stream.read_u32()?;
                                let start = index_stream.read_u64()?;
                                let original_size = index_stream.read_u64()?;
                                let archived_size = index_stream.read_u64()?;
                                chunk_size -= 0x1C;
                                segments.push(Segment {
                                    is_compressed: seg_flags != 0,
                                    start,
                                    offset_in_file: seg_offset,
                                    original_size,
                                    archived_size,
                                });
                                seg_offset += original_size;
                            }
                        } else if &chunk_sig == CHUNK_TIME {
                            if chunk_size == 8 {
                                timestamp = Some(index_stream.read_u64()?);
                                chunk_size -= 8;
                            }
                        } else {
                            let data = index_stream.read_exact_vec(chunk_size as usize)?;
                            chunk_size = 0;
                            entry_extras.push(ExtraProp {
                                tag: chunk_sig.into(),
                                data,
                            });
                        }
                        if chunk_size > 0 {
                            index_stream.skip(chunk_size)?;
                        }
                    }
                    let mut entry = Xp3Entry {
                        name: name
                            .ok_or_else(|| anyhow::anyhow!("Missing name chunk in file entry"))?,
                        flags: flags
                            .ok_or_else(|| anyhow::anyhow!("Missing flags chunk in file entry"))?,
                        file_hash: file_hash.unwrap_or(0),
                        original_size: original_size.ok_or_else(|| {
                            anyhow::anyhow!("Missing original size chunk in file entry")
                        })?,
                        archived_size: archived_size.ok_or_else(|| {
                            anyhow::anyhow!("Missing archived size chunk in file entry")
                        })?,
                        timestamp,
                        segments,
                        extras: entry_extras,
                    };
                    if entry.name == "startup.tjs"
                        && entry.flags != 0
                        && crypt.startup_tjs_not_encrypted()
                    {
                        entry.flags = 0;
                    }
                    entries.push(entry);
                } else {
                    let data = index_stream.read_exact_vec(size as usize)?;
                    extras.push(ExtraProp {
                        tag: sig.into(),
                        data,
                    });
                }
            }
        }
        let crypt = Arc::new(crypt);
        let mut archive = Self {
            inner: Arc::new(Mutex::new(stream)),
            crypt: crypt.clone(),
            base_offset,
            index_offset,
            minor_version,
            entries,
            extras,
        };
        crypt.init(&mut archive)?;
        Ok(archive)
    }

    fn get_index_stream<'c, 'b, T: Read + Seek + std::fmt::Debug + 'b>(
        stream: &'c mut Box<T>,
    ) -> Result<Box<dyn Read + 'c>> {
        let index_type = stream.read_u8()?;
        Ok(match index_type {
            TVP_XP3_INDEX_ENCODE_RAW => {
                let index_size = stream.read_u64()?;
                Box::new(StreamRegion::with_size(stream, index_size)?)
            }
            TVP_XP3_INDEX_ENCODE_ZLIB => {
                let packed_size = stream.read_u64()?;
                let _original_size = stream.read_u64()?;
                let mut compressed_data = StreamRegion::with_size(stream, packed_size)?;
                if compressed_data.peek_and_equal(ZSTD_SIGNATURE).is_ok() {
                    Box::new(zstd::stream::read::Decoder::new(compressed_data)?)
                } else {
                    Box::new(flate2::read::ZlibDecoder::new(compressed_data))
                }
            }
            _ => {
                anyhow::bail!("Unsupported index type: {}", index_type);
            }
        })
    }
}
