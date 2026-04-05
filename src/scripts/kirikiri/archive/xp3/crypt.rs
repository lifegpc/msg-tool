use super::archive::*;
use crate::ext::io::*;
use crate::types::*;
use crate::utils::encoding::*;
use anyhow::Result;
use std::io::Read;

pub trait Crypt: std::fmt::Debug {
    /// Initializes the cryptographic context for the archive.
    fn init(&self, _archive: &mut Xp3Archive) -> Result<()> {
        Ok(())
    }

    /// Read a entry name from archive index
    fn read_name<'a>(&self, reader: &mut Box<dyn Read + 'a>) -> Result<(String, u64)> {
        let name_length = reader.read_u16()?;
        let name = reader.read_exact_vec(name_length as usize * 2)?;
        Ok((
            decode_to_string(Encoding::Utf16LE, &name, true)?,
            name_length as u64 * 2 + 2,
        ))
    }
}

#[derive(Debug)]
pub struct NoCrypt {}

impl NoCrypt {
    pub fn new() -> Self {
        Self {}
    }
}

impl Crypt for NoCrypt {}
