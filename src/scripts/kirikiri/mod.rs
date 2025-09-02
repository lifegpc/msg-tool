//! Kirikiri Scripts
#[cfg(feature = "kirikiri-img")]
pub mod image;
pub mod ks;
pub mod mdf;
pub mod scn;
pub mod simple_crypt;
pub mod tjs2;
pub mod tjs_ns0;
use std::collections::HashMap;
use std::sync::Arc;

/// Read a Kirikiri Comu JSON file. (For CIRCUS games)
pub fn read_kirikiri_comu_json(path: &str) -> anyhow::Result<Arc<HashMap<String, String>>> {
    let mut reader = std::fs::File::open(path)?;
    let data = serde_json::from_reader(&mut reader)?;
    Ok(Arc::new(data))
}
