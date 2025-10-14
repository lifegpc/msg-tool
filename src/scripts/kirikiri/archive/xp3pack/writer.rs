use super::archive::*;
use super::consts::*;
use super::reader::*;
use super::segmenter::*;
use crate::ext::io::*;
use crate::ext::mutex::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use crate::utils::threadpool::ThreadPool;
use anyhow::Result;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{Seek, Write};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct WrittenSegment {
    is_compressed: bool,
    start: u64,
    original_size: u64,
    archived_size: u64,
}

#[derive(Default)]
struct Stats {
    total_original_size: AtomicU64,
    final_archive_size: AtomicU64,
    total_segments: AtomicUsize,
    unique_segments: AtomicUsize,
    deduplication_savings: AtomicU64,
}

impl std::fmt::Display for Stats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let total_original_size = self
            .total_original_size
            .load(std::sync::atomic::Ordering::Relaxed);
        let final_archive_size = self
            .final_archive_size
            .load(std::sync::atomic::Ordering::Relaxed);
        let total_segments = self
            .total_segments
            .load(std::sync::atomic::Ordering::Relaxed);
        let unique_segments = self
            .unique_segments
            .load(std::sync::atomic::Ordering::Relaxed);
        let deduplication_savings = self
            .deduplication_savings
            .load(std::sync::atomic::Ordering::Relaxed);
        write!(
            f,
            "Total Original Size: {} bytes\nFinal Archive Size: {} bytes\nTotal Segments: {}\nUnique Segments: {}\nDeduplication Savings: {} bytes",
            total_original_size,
            final_archive_size,
            total_segments,
            unique_segments,
            deduplication_savings
        )
    }
}

pub struct Xp3ArchiveWriter<T: Write + Seek> {
    file: Arc<Mutex<T>>,
    segments: Arc<Mutex<HashMap<[u8; 32], WrittenSegment>>>,
    items: Arc<Mutex<BTreeMap<String, ArchiveItem>>>,
    runner: ThreadPool<Result<()>>,
    compress_files: bool,
    compress_index: bool,
    zlib_compression_level: u32,
    segmenter: Option<Arc<Box<dyn Segmenter + Send + Sync>>>,
    stats: Arc<Stats>,
    compress_workers: usize,
    processing_segments: Arc<Mutex<HashSet<[u8; 32]>>>,
    use_zstd: bool,
    zstd_compression_level: i32,
}

impl Xp3ArchiveWriter<std::io::BufWriter<std::fs::File>> {
    pub fn new(filename: &str, files: &[&str], config: &ExtraConfig) -> Result<Self> {
        let file = std::fs::File::create(filename)?;
        let mut file = std::io::BufWriter::new(file);
        let mut items = BTreeMap::new();
        for file in files {
            let item = ArchiveItem {
                name: file.to_string(),
                file_hash: 0,
                original_size: 0,
                archived_size: 0,
                segments: Vec::new(),
            };
            items.insert(file.to_string(), item);
        }
        let segmenter = create_segmenter(config.xp3_segmenter).map(|s| Arc::new(s));
        file.write_all(XP3_MAGIC)?;
        file.write_u64(0)?; // Placeholder for index offset
        Ok(Self {
            file: Arc::new(Mutex::new(file)),
            segments: Arc::new(Mutex::new(HashMap::new())),
            items: Arc::new(Mutex::new(items)),
            runner: ThreadPool::new(
                if config.xp3_segmenter.is_none() {
                    1
                } else {
                    config.xp3_pack_workers.max(1)
                },
                Some("xp3-writer"),
                false,
            )?,
            compress_files: config.xp3_compress_files,
            compress_index: config.xp3_compress_index,
            zlib_compression_level: config.zlib_compression_level,
            segmenter,
            stats: Arc::new(Stats::default()),
            compress_workers: config.xp3_compress_workers.max(1),
            processing_segments: Arc::new(Mutex::new(HashSet::new())),
            use_zstd: config.xp3_zstd,
            zstd_compression_level: config.zstd_compression_level,
        })
    }
}

struct Writer<'a> {
    inner: Box<dyn Write + 'a>,
    mem: MemWriter,
}

impl std::fmt::Debug for Writer<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Writer").field("mem", &self.mem).finish()
    }
}

impl<'a> Write for Writer<'a> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.mem.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.mem.flush()
    }
}

impl<'a> Seek for Writer<'a> {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        self.mem.seek(pos)
    }

    fn stream_position(&mut self) -> std::io::Result<u64> {
        self.mem.stream_position()
    }

    fn rewind(&mut self) -> std::io::Result<()> {
        self.mem.rewind()
    }
}

impl<'a> Drop for Writer<'a> {
    fn drop(&mut self) {
        let _ = self.inner.write_all(&self.mem.data);
        let _ = self.inner.flush();
    }
}

impl<T: Write + Seek + Sync + Send + 'static> Archive for Xp3ArchiveWriter<T> {
    fn new_file<'a>(&'a mut self, name: &str) -> Result<Box<dyn WriteSeek + 'a>> {
        let inner = self.new_file_non_seek(name)?;
        Ok(Box::new(Writer {
            inner,
            mem: MemWriter::new(),
        }))
    }

    fn new_file_non_seek<'a>(&'a mut self, name: &str) -> Result<Box<dyn Write + 'a>> {
        if self.segmenter.is_none() {
            self.runner.join();
        }
        for err in self.runner.take_results() {
            err?;
        }
        let item = {
            let items = self.items.lock_blocking();
            Arc::new(Mutex::new(
                items
                    .get(name)
                    .ok_or_else(|| anyhow::anyhow!("File not found in archive: {}", name))?
                    .clone(),
            ))
        };
        let (reader, writer) = std::io::pipe()?;
        let reader = Reader::new(reader);
        {
            let file = self.file.clone();
            let segments = self.segments.clone();
            let items = self.items.clone();
            let segmenter = self.segmenter.clone();
            let stats = self.stats.clone();
            let is_compressed = self.compress_files;
            let zlib_compression_level = self.zlib_compression_level;
            let workers = if self.segmenter.is_some() && is_compressed {
                Some(Arc::new(ThreadPool::<Result<()>>::new(
                    self.compress_workers,
                    Some("xp3-compress"),
                    false,
                )?))
            } else {
                None
            };
            let processiong_segments = self.processing_segments.clone();
            let use_zstd = self.use_zstd;
            let zstd_compression_level = self.zstd_compression_level;
            self.runner.execute(
                move |_| {
                    let mut reader = reader;
                    let mut offset_in_file = 0u64;
                    if let Some(segmenter) = segmenter {
                        for seg in segmenter.segment(&mut reader) {
                            let seg = seg?;
                            let hash: [u8; 32] = Sha256::digest(&seg).into();
                            let seg_offset_in_file = offset_in_file;
                            offset_in_file += seg.len() as u64;
                            let fseg = match {
                                let mut segments = segments.lock_blocking();
                                if let Some(old_seg) = segments.get(&hash) {
                                    Err(old_seg.clone())
                                } else {
                                    let seg_data = WrittenSegment {
                                        is_compressed,
                                        start: 0,
                                        original_size: seg.len() as u64,
                                        archived_size: seg.len() as u64,
                                    };
                                    segments.insert(hash, seg_data.clone());
                                    Ok(seg_data)
                                }
                            } {
                                Ok(mut info) => {
                                    if let Some(workers) = workers.as_ref() {
                                        {
                                            let mut processing =
                                                processiong_segments.lock_blocking();
                                            processing.insert(hash);
                                        }
                                        let file = file.clone();
                                        let segments = segments.clone();
                                        let stats = stats.clone();
                                        let item = item.clone();
                                        let processiong_segments = processiong_segments.clone();
                                        workers.execute(
                                            move |_| {
                                                let data = {
                                                    if use_zstd {
                                                        let mut e = zstd::stream::Encoder::new(
                                                            Vec::new(),
                                                            zstd_compression_level,
                                                        )?;
                                                        e.write_all(&seg)?;
                                                        e.finish()?
                                                    } else {
                                                        let mut e = flate2::write::ZlibEncoder::new(
                                                            Vec::new(),
                                                            flate2::Compression::new(
                                                                zlib_compression_level,
                                                            ),
                                                        );
                                                        e.write_all(&seg)?;
                                                        e.finish()?
                                                    }
                                                };
                                                let mut file = file.lock_blocking();
                                                let start = file.seek(std::io::SeekFrom::End(0))?;
                                                file.write_all(&data)?;
                                                info.start = start;
                                                info.archived_size = data.len() as u64;
                                                let stats = stats.clone();
                                                stats.total_original_size.fetch_add(
                                                    info.original_size,
                                                    Ordering::Relaxed,
                                                );
                                                stats.final_archive_size.fetch_add(
                                                    info.archived_size,
                                                    Ordering::Relaxed,
                                                );
                                                stats
                                                    .total_segments
                                                    .fetch_add(1, Ordering::Relaxed);
                                                stats
                                                    .unique_segments
                                                    .fetch_add(1, Ordering::Relaxed);
                                                let mut segments = segments.lock_blocking();
                                                segments.insert(hash, info.clone());
                                                let ninfo = Segment {
                                                    is_compressed: info.is_compressed,
                                                    start: info.start,
                                                    offset_in_file: seg_offset_in_file,
                                                    original_size: info.original_size,
                                                    archived_size: info.archived_size,
                                                };
                                                let mut item = item.lock_blocking();
                                                item.original_size += ninfo.original_size;
                                                item.archived_size += ninfo.archived_size;
                                                item.segments.push(ninfo);
                                                let mut processing =
                                                    processiong_segments.lock_blocking();
                                                processing.remove(&hash);
                                                Ok(())
                                            },
                                            true,
                                        )?;
                                        None
                                    } else {
                                        {
                                            let mut processing =
                                                processiong_segments.lock_blocking();
                                            processing.insert(hash);
                                        }
                                        let data = seg;
                                        let mut file = file.lock_blocking();
                                        let start = file.seek(std::io::SeekFrom::End(0))?;
                                        file.write_all(&data)?;
                                        info.start = start;
                                        info.archived_size = data.len() as u64;
                                        let stats = stats.clone();
                                        stats
                                            .total_original_size
                                            .fetch_add(info.original_size, Ordering::Relaxed);
                                        stats
                                            .final_archive_size
                                            .fetch_add(info.archived_size, Ordering::Relaxed);
                                        stats.total_segments.fetch_add(1, Ordering::Relaxed);
                                        stats.unique_segments.fetch_add(1, Ordering::Relaxed);
                                        let mut segments = segments.lock_blocking();
                                        segments.insert(hash, info.clone());
                                        let ninfo = Segment {
                                            is_compressed: info.is_compressed,
                                            start: info.start,
                                            offset_in_file: seg_offset_in_file,
                                            original_size: info.original_size,
                                            archived_size: info.archived_size,
                                        };
                                        {
                                            let mut processing =
                                                processiong_segments.lock_blocking();
                                            processing.remove(&hash);
                                        }
                                        Some(ninfo)
                                    }
                                }
                                Err(mut seg_info) => {
                                    let mut need_update = false;
                                    loop {
                                        if {
                                            let processing = processiong_segments.lock_blocking();
                                            !processing.contains(&hash)
                                        } {
                                            break;
                                        }
                                        need_update = true;
                                        std::thread::sleep(std::time::Duration::from_millis(10));
                                    }
                                    if need_update {
                                        seg_info = {
                                            let segments = segments.lock_blocking();
                                            segments
                                                .get(&hash)
                                                .ok_or(anyhow::anyhow!(
                                                    "Failed to get latest segment info."
                                                ))?
                                                .clone()
                                        };
                                    }
                                    let stats = stats.clone();
                                    stats
                                        .total_original_size
                                        .fetch_add(seg_info.original_size, Ordering::Relaxed);
                                    stats
                                        .deduplication_savings
                                        .fetch_add(seg_info.archived_size, Ordering::Relaxed);
                                    stats.total_segments.fetch_add(1, Ordering::Relaxed);
                                    let ninfo = Segment {
                                        is_compressed: seg_info.is_compressed,
                                        start: seg_info.start,
                                        offset_in_file: seg_offset_in_file,
                                        original_size: seg_info.original_size,
                                        archived_size: seg_info.archived_size,
                                    };
                                    Some(ninfo)
                                }
                            };
                            if let Some(fseg) = fseg {
                                let mut item = item.lock_blocking();
                                item.original_size += fseg.original_size;
                                item.archived_size += fseg.archived_size;
                                item.segments.push(fseg);
                            }
                        }
                    } else {
                        let mut file = file.lock_blocking();
                        let start = file.seek(std::io::SeekFrom::End(0))?;
                        let size = {
                            let mut writer = if is_compressed {
                                if use_zstd {
                                    let e = zstd::stream::Encoder::new(
                                        &mut *file,
                                        zstd_compression_level,
                                    )?;
                                    Box::new(e) as Box<dyn Write>
                                } else {
                                    let e = flate2::write::ZlibEncoder::new(
                                        &mut *file,
                                        flate2::Compression::new(zlib_compression_level),
                                    );
                                    Box::new(e) as Box<dyn Write>
                                }
                            } else {
                                Box::new(&mut *file) as Box<dyn Write>
                            };
                            std::io::copy(&mut reader, &mut writer)?
                        };
                        let ninfo = Segment {
                            is_compressed,
                            start,
                            offset_in_file: 0,
                            original_size: size,
                            archived_size: if is_compressed {
                                file.stream_position()? - start
                            } else {
                                size
                            },
                        };
                        let mut item = item.lock_blocking();
                        item.original_size += ninfo.original_size;
                        item.archived_size += ninfo.archived_size;
                        let stats = stats.clone();
                        stats
                            .total_original_size
                            .fetch_add(ninfo.original_size, Ordering::Relaxed);
                        stats
                            .final_archive_size
                            .fetch_add(ninfo.archived_size, Ordering::Relaxed);
                        stats.total_segments.fetch_add(1, Ordering::Relaxed);
                        stats.unique_segments.fetch_add(1, Ordering::Relaxed);
                        item.segments.push(ninfo);
                    }
                    if let Some(workers) = workers {
                        workers.join();
                        for err in workers.take_results() {
                            err?;
                        }
                    }
                    let mut item = item.lock_blocking().to_owned();
                    item.file_hash = reader.into_checksum();
                    item.segments.sort_by_key(|s| s.offset_in_file);
                    let mut items = items.lock_blocking();
                    items.insert(item.name.clone(), item);
                    Ok(())
                },
                true,
            )?;
        }
        Ok(Box::new(writer))
    }

    fn write_header(&mut self) -> Result<()> {
        self.runner.join();
        for err in self.runner.take_results() {
            err?;
        }
        let mut file = self.file.lock_blocking();
        let index_offset = file.seek(std::io::SeekFrom::End(0))?;
        let mut index_data = MemWriter::new();
        let items = self.items.lock_blocking();
        for (_, item) in items.iter() {
            let mut file_chunk = MemWriter::new();
            let name = encode_string(Encoding::Utf16LE, &item.name, false)?;
            let info_data_size = name.len() as u64 + 22;
            file_chunk.write_all(CHUNK_INFO)?;
            file_chunk.write_u64(info_data_size)?;
            file_chunk.write_u32(0)?; // flags
            file_chunk.write_u64(item.original_size)?;
            file_chunk.write_u64(item.archived_size)?;
            file_chunk.write_u16(name.len() as u16 / 2)?;
            file_chunk.write_all(&name)?;
            let segm_data_size = item.segments.len() as u64 * 28;
            file_chunk.write_all(CHUNK_SEGM)?;
            file_chunk.write_u64(segm_data_size)?;
            for seg in &item.segments {
                let flag = if seg.is_compressed {
                    TVP_XP3_SEGM_ENCODE_ZLIB
                } else {
                    TVP_XP3_SEGM_ENCODE_RAW
                };
                file_chunk.write_u32(flag)?;
                file_chunk.write_u64(seg.start)?;
                file_chunk.write_u64(seg.original_size)?;
                file_chunk.write_u64(seg.archived_size)?;
            }
            let adlr_data_size = 4;
            file_chunk.write_all(CHUNK_ADLR)?;
            file_chunk.write_u64(adlr_data_size)?;
            file_chunk.write_u32(item.file_hash)?;
            index_data.write_all(CHUNK_FILE)?;
            let file_chunk = file_chunk.into_inner();
            index_data.write_u64(file_chunk.len() as u64)?;
            index_data.write_all(&file_chunk)?;
        }
        let index_data = index_data.into_inner();
        if self.compress_index {
            let compressed_index = if self.use_zstd {
                let mut e = zstd::stream::Encoder::new(Vec::new(), self.zstd_compression_level)?;
                e.write_all(&index_data)?;
                e.finish()?
            } else {
                let mut e = flate2::write::ZlibEncoder::new(
                    Vec::new(),
                    flate2::Compression::new(self.zlib_compression_level),
                );
                e.write_all(&index_data)?;
                e.finish()?
            };
            file.write_u8(TVP_XP3_INDEX_ENCODE_ZLIB)?;
            file.write_u64(compressed_index.len() as u64)?;
            file.write_u64(index_data.len() as u64)?;
            file.write_all(&compressed_index)?;
        } else {
            file.write_u8(TVP_XP3_INDEX_ENCODE_RAW)?;
            file.write_u64(index_data.len() as u64)?;
            file.write_all(&index_data)?;
        }
        file.write_u64_at(11, index_offset)?; // Write index offset to header
        file.flush()?;
        eprintln!("XP3 Archive Statistics:\n{}", self.stats);
        Ok(())
    }
}
