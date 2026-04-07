use crate::simple_pack::SimplePack;
use std::path::Path;

/// Pack all binary files in cx_cb into a single archive.
pub fn gen_cx_cb<P: AsRef<Path> + ?Sized, D: AsRef<Path> + ?Sized>(
    json_path: &P,
    outdir: &D,
    level: i32,
) -> std::io::Result<()> {
    let p = json_path.as_ref();
    let pb = p
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .join("crypt")
        .join("cx_cb");
    let json_data = std::fs::read_to_string(p)?;
    let json = json::parse(&json_data)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let mut pack = SimplePack::new(&outdir.as_ref().join("cx_cb.pck"))?;
    for (_, obj) in json.entries() {
        if let Some(name) = obj["ControlBlockName"].as_str() {
            let file_path = pb.join(name);
            if !file_path.exists() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("File not found: {}", file_path.display()),
                ));
            }
            let file = std::fs::File::open(file_path)?;
            let file = std::io::BufReader::new(file);
            pack.add_file(name, file)?;
        }
    }
    if level >= 0 && level <= 22 {
        pack.compress(level)?;
    }
    Ok(())
}
