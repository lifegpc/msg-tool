use crate::simple_pack::SimplePack;
use std::collections::HashSet;
use std::path::Path;

/// Generate crypt.json.zst from crypt.json with minimum format
pub fn gen_crypt<P: AsRef<Path> + ?Sized, D: AsRef<Path> + ?Sized>(
    json_path: &P,
    outdir: &D,
    level: i32,
) -> std::io::Result<()> {
    let p = json_path.as_ref();
    let json_data = std::fs::read_to_string(p)?;
    let json = json::parse(&json_data)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let out_data = json::stringify(json);
    let out_path = outdir.as_ref().join("crypt.json.zst");
    let mut out_file = std::io::BufWriter::new(std::fs::File::create(out_path)?);
    let level = if level >= 0 && level <= 22 { level } else { 22 };
    let mut encoder = zstd::stream::write::Encoder::new(&mut out_file, level)?;
    std::io::copy(&mut out_data.as_bytes(), &mut encoder)?;
    encoder.finish()?;
    Ok(())
}

/// Pack all binary files in cx_cb into a single archive.
pub fn gen_cx_cb<P: AsRef<Path> + ?Sized, D: AsRef<Path> + ?Sized>(
    json_path: &P,
    outdir: &D,
    level: i32,
) -> std::io::Result<()> {
    let p = json_path.as_ref();
    let pb = p.parent().unwrap_or_else(|| Path::new("")).join("cx_cb");
    let json_data = std::fs::read_to_string(p)?;
    let json = json::parse(&json_data)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let mut pack = SimplePack::new(&outdir.as_ref().join("cx_cb.pck"))?;
    let mut seen_files = HashSet::new();
    for (_, obj) in json.entries() {
        if let Some(name) = obj["ControlBlockName"].as_str() {
            if seen_files.contains(name) {
                continue;
            }
            seen_files.insert(name.to_string());
            let file_path = pb.join(name);
            if !file_path.exists() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("File not found: {}", file_path.display()),
                ));
            }
            let file = std::fs::File::open(file_path)?;
            let file_size = file.metadata()?.len();
            if file_size != 4096 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("File size must be 4096 bytes: {}", name),
                ));
            }
            let file = std::io::BufReader::new(file);
            pack.add_file(name, file)?;
        }
    }
    if level >= 0 && level <= 22 {
        pack.compress(level)?;
    }
    Ok(())
}
