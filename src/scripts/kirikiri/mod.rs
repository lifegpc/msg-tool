#[cfg(feature = "kirikiri-img")]
pub mod image;
pub mod ks;
pub mod scn;
pub mod simple_crypt;
use std::collections::HashMap;
use std::sync::Arc;

pub fn read_kirikiri_comu_json(path: &str) -> anyhow::Result<Arc<HashMap<String, String>>> {
    let mut reader = std::fs::File::open(path)?;
    let data = serde_json::from_reader(&mut reader)?;
    Ok(Arc::new(data))
}
