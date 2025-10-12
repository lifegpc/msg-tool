use super::reader::Reader;
use anyhow::Result;
use fastcdc::v2020::StreamCDC;
use std::io::Read;

#[derive(Copy, Clone, Debug)]
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
    ) -> Box<dyn Iterator<Item = Result<Vec<u8>>> + 'a> {
        let cdc = StreamCDC::new(data, self.min_size, self.avg_size, self.max_size);
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

pub fn create_segmenter(config: SegmenterConfig) -> Option<Box<dyn Segmenter + Send + Sync>> {
    match config {
        SegmenterConfig::None => None,
        SegmenterConfig::FastCdc {
            min_size,
            avg_size,
            max_size,
        } => Some(Box::new(FastCdcSegmenter {
            min_size,
            avg_size,
            max_size,
        })),
        SegmenterConfig::Fixed(size) => Some(Box::new(FixedSizeSegmenter { size })),
    }
}
