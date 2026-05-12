use super::consts::*;
use anyhow::Result;
use memchr::memmem::find;
use pelite::{PeFile, Wrap};

pub fn get_base_offset<D: AsRef<[u8]> + ?Sized>(data: &D) -> Result<u64> {
    let file = PeFile::from_bytes(data)?;
    if let Some(rsrc) = file.section_headers().by_name(".rsrc") {
        let bytes = file.get_section_bytes(rsrc)?;
        if let Some(pos) = find(bytes, XP3_MAGIC) {
            return Ok(rsrc.file_range().start as u64 + pos as u64);
        }
    }
    let last_section_end = file
        .section_headers()
        .iter()
        .map(|s| s.PointerToRawData + s.SizeOfRawData)
        .max()
        .unwrap_or_else(|| match file.optional_header() {
            Wrap::T32(h) => h.SizeOfHeaders,
            Wrap::T64(h) => h.SizeOfHeaders,
        });
    let aligned_offset = ((last_section_end + 0xF) & !0xF) as usize;
    let data = data.as_ref();
    if aligned_offset >= data.len() {
        anyhow::bail!("No overlay for pe image.");
    }
    for i in (aligned_offset..(data.len() - 11)).step_by(0x10) {
        if &data[i..i + 11] == XP3_MAGIC {
            return Ok(i as u64);
        }
    }
    anyhow::bail!("Failed to find xp3 file in pe file.")
}
