use super::reader::Reader;
use anyhow::Result;
use fastcdc::v2020::StreamCDC;
use serde::Deserialize;
use std::collections::HashMap;
use std::io::Read;
use std::sync::Arc;

#[derive(Clone, Debug, Deserialize)]
pub struct Segments {
    segments: Arc<HashMap<String, Vec<u64>>>,
    #[serde(default)]
    default_config: Box<SegmenterConfig>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "@type")]
/// Configuration options for the segmenter.
pub enum SegmenterConfig {
    /// Do not segment the data.
    None,
    /// Use the FastCDC algorithm with specified minimum, average, and maximum chunk sizes.
    FastCdc {
        min_size: u32,
        avg_size: u32,
        max_size: u32,
    },
    /// Use fixed-size segments.
    Fixed(usize),
    Custom(Segments),
}

impl Default for SegmenterConfig {
    fn default() -> Self {
        SegmenterConfig::FastCdc {
            min_size: 32 * 1024,
            avg_size: 256 * 1024,
            max_size: 8 * 1024 * 1024,
        }
    }
}

impl SegmenterConfig {
    pub fn is_none(&self) -> bool {
        matches!(self, SegmenterConfig::None)
    }
}

/// A trait for strategies that split a byte slice into one or more segments.
pub trait Segmenter {
    fn segment<'a>(
        &'a self,
        data: &'a mut Reader,
        filename: &'a str,
    ) -> Box<dyn Iterator<Item = Result<Vec<u8>>> + 'a>;
}

pub struct FastCdcSegmenter {
    min_size: u32,
    avg_size: u32,
    max_size: u32,
}

impl Segmenter for FastCdcSegmenter {
    fn segment<'a>(
        &'a self,
        data: &'a mut Reader,
        _filename: &'a str,
    ) -> Box<dyn Iterator<Item = Result<Vec<u8>>> + 'a> {
        let cdc = StreamCDC::new(
            data,
            self.min_size as usize,
            self.avg_size as usize,
            self.max_size as usize,
        );
        Box::new(cdc.map(|chunk| Ok(chunk?.data)))
    }
}

pub struct FixedSizeSegmenter {
    size: usize,
}

impl Segmenter for FixedSizeSegmenter {
    fn segment<'a>(
        &'a self,
        data: &'a mut Reader,
        _filename: &'a str,
    ) -> Box<dyn Iterator<Item = Result<Vec<u8>>> + 'a> {
        let size = self.size;
        let mut buf = vec![0; size];
        Box::new(std::iter::from_fn(move || {
            let nbuf = &mut buf;
            let mut total_read = 0;
            while total_read < size {
                match data.read(&mut nbuf[total_read..]) {
                    Ok(0) => break, // EOF
                    Ok(n) => total_read += n,
                    Err(e) => return Some(Err(e.into())),
                }
            }
            if total_read == 0 {
                None // No more data to read
            } else {
                Some(Ok(buf[..total_read].to_vec()))
            }
        }))
    }
}

pub struct CustomSegmenter {
    segments: Arc<HashMap<String, Vec<u64>>>,
    inner: Box<dyn Segmenter + Send + Sync>,
}

impl Segmenter for CustomSegmenter {
    fn segment<'a>(
        &'a self,
        data: &'a mut Reader,
        filename: &'a str,
    ) -> Box<dyn Iterator<Item = Result<Vec<u8>>> + 'a> {
        if let Some(segment_offsets) = self.segments.get(filename) {
            let mut current_seg_idx = 0;
            let mut reached_eof = false;

            Box::new(std::iter::from_fn(move || {
                if reached_eof {
                    return None;
                }

                // 获取当前 Reader 的绝对位置
                let current_pos = data.total_readed();

                if current_seg_idx < segment_offsets.len() {
                    // 1. 处理预设的分割点
                    let target_pos = segment_offsets[current_seg_idx];
                    current_seg_idx += 1;

                    if target_pos <= current_pos {
                        // 如果分割点无效（小于当前位置），跳过或视作该段为空
                        return Some(Ok(Vec::new()));
                    }

                    let to_read = (target_pos - current_pos) as usize;
                    let mut buf = vec![0; to_read];
                    let mut total_read = 0;

                    while total_read < to_read {
                        match data.read(&mut buf[total_read..]) {
                            Ok(0) => {
                                reached_eof = true;
                                break;
                            }
                            Ok(n) => total_read += n,
                            Err(e) => return Some(Err(e.into())),
                        }
                    }

                    if total_read == 0 && reached_eof {
                        None
                    } else {
                        buf.truncate(total_read);
                        Some(Ok(buf))
                    }
                } else {
                    // 2. 处理“最后一个分割点之后”的剩余数据 (Tail)
                    // 标记为已到达末尾，保证下一次调用返回 None
                    reached_eof = true;

                    let mut final_buf = Vec::new();
                    let mut temp_buf = [0u8; 8192]; // 临时缓冲区用于读取剩余所有内容

                    loop {
                        match data.read(&mut temp_buf) {
                            Ok(0) => break,
                            Ok(n) => final_buf.extend_from_slice(&temp_buf[..n]),
                            Err(e) => return Some(Err(e.into())),
                        }
                    }

                    if final_buf.is_empty() {
                        None
                    } else {
                        Some(Ok(final_buf))
                    }
                }
            }))
        } else {
            self.inner.segment(data, filename)
        }
    }
}

pub fn create_segmenter(config: &SegmenterConfig) -> Option<Box<dyn Segmenter + Send + Sync>> {
    match config {
        SegmenterConfig::None => None,
        SegmenterConfig::FastCdc {
            min_size,
            avg_size,
            max_size,
        } => Some(Box::new(FastCdcSegmenter {
            min_size: *min_size,
            avg_size: *avg_size,
            max_size: *max_size,
        })),
        SegmenterConfig::Fixed(size) => Some(Box::new(FixedSizeSegmenter { size: *size })),
        SegmenterConfig::Custom(manifest) => Some(Box::new(CustomSegmenter {
            segments: manifest.segments.clone(),
            inner: match create_segmenter(&manifest.default_config) {
                Some(cfg) => cfg,
                None => {
                    return None;
                }
            },
        })),
    }
}
