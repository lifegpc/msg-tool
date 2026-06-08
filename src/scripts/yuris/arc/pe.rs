use anyhow::Result;
use pelite::{PeFile, Wrap};

const YSER_MAGIC: &[u8; 4] = b"YSER";

/// Find the YPF archive base offset inside a PE (EXE) file.
///
/// Searches the PE overlay for the "YSER" header signature at 0x10-aligned
/// boundaries, then reads the 32-bit header size field at offset+4 and returns
/// `YSER_offset + header_size` as the start of the YPF data.
pub fn get_base_offset<D: AsRef<[u8]> + ?Sized>(data: &D) -> Result<u64> {
    let file = PeFile::from_bytes(data)?;
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
    if aligned_offset + 8 > data.len() {
        anyhow::bail!("No overlay for pe image.");
    }
    for i in (aligned_offset..(data.len() - 8)).step_by(0x10) {
        if &data[i..i + 4] == YSER_MAGIC {
            let header_size = u32::from_le_bytes([
                data[i + 4],
                data[i + 5],
                data[i + 6],
                data[i + 7],
            ]);
            return Ok(i as u64 + header_size as u64);
        }
    }
    anyhow::bail!("Failed to find YSER header in pe file.")
}
