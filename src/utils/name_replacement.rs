//! Name Replacement Utilities
use crate::types::*;
use anyhow::Result;
use std::collections::HashMap;

/// Read Name Replacement Table from CSV
pub fn read_csv(path: &str) -> Result<HashMap<String, String>> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(path)?;
    let mut map = HashMap::new();
    for result in reader.deserialize() {
        let record: NameTableCell = result?;
        if record.jp_name.is_empty() || record.cn_name.is_empty() {
            continue;
        }
        map.insert(record.jp_name, record.cn_name);
    }
    Ok(map)
}

/// Replace names in the message with the given name table.
pub fn replace_message(mes: &mut Vec<Message>, name_table: &HashMap<String, String>) {
    for message in mes.iter_mut() {
        if let Some(name) = &message.name {
            if let Some(replace_name) = name_table.get(name) {
                message.name = Some(replace_name.clone());
            }
        }
    }
}
