#![cfg_attr(any(docsrs, feature = "unstable"), feature(doc_cfg))]
use std::io::Read;

/// Control Block data for CxEncryption packed with SimplePack.
pub const CX_CB_DATA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/cx_cb.pck"));
/// Name list data packed with SimplePack.
pub const NAME_LIST_DATA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/name_list.pck"));
const CRYPT_DATA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/crypt.json.zst"));

/// Get the crypt.json data as a string.
pub fn get_crypt_data() -> String {
    let mut decoder = zstd::stream::read::Decoder::new(CRYPT_DATA).unwrap();
    let mut out = String::new();
    decoder.read_to_string(&mut out).unwrap();
    out
}

/// AlteredPink KeyTable
pub const ALTERED_PINK_KEY_TABLE: &[u8] = include_bytes!("bin/altered_pink.bin");
