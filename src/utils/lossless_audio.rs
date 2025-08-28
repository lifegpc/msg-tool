//! Lossless audio utilities.
#[cfg(feature = "audio-flac")]
use super::flac::*;
use super::pcm::*;
use crate::types::*;
use anyhow::Result;
use std::io::{Read, Seek, Write};

pub fn write_audio<W: Write + Seek, R: Read>(
    header: &PcmFormat,
    reader: R,
    writer: W,
    config: &ExtraConfig,
) -> Result<()> {
    match config.lossless_audio_fmt {
        LosslessAudioFormat::Wav => write_pcm(header, reader, writer)?,
        #[cfg(feature = "audio-flac")]
        LosslessAudioFormat::Flac => write_flac(header, reader, writer, config)?,
    }
    Ok(())
}
