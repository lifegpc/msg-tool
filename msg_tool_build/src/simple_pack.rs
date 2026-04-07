//! A simple implementation of a pack file
use std::fs::File;
use std::io::{BufWriter, Read, Result, Seek, Write};
use std::path::{Path, PathBuf};

pub struct SimplePack {
    file: File,
    path: PathBuf,
    tmp_path: PathBuf,
}

impl SimplePack {
    pub fn new<P: AsRef<Path> + ?Sized>(path: &P) -> Result<Self> {
        let mut file = File::create(path.as_ref())?;
        file.write_all(b"SPCK")?;
        file.write_all(&[0])?; // No compression
        Ok(Self {
            file,
            path: path.as_ref().to_path_buf(),
            tmp_path: path.as_ref().with_added_extension(".tmp"),
        })
    }
    pub fn add_file<R: Read>(&mut self, name: &str, mut data: R) -> Result<()> {
        let mut writer = BufWriter::new(&mut self.file);
        writer.write_all(name.as_bytes())?;
        writer.write_all(&[0])?; // Null terminator for the name
        let file_size_loc = writer.stream_position()?;
        writer.write_all(&0u64.to_le_bytes())?; // Placeholder for file size
        let size = std::io::copy(&mut data, &mut writer)?;
        let current_pos = writer.stream_position()?;
        writer.seek(std::io::SeekFrom::Start(file_size_loc))?;
        writer.write_all(&size.to_le_bytes())?; // Write the actual file size
        writer.seek(std::io::SeekFrom::Start(current_pos))?; // Move back to the end of the file
        writer.flush()?;
        Ok(())
    }
    pub fn compress(mut self, level: i32) -> Result<()> {
        self.file.flush()?;
        std::mem::drop(self.file); // Close the file before renaming
        // Move the file to a temporary location
        std::fs::rename(&self.path, &self.tmp_path)?;
        {
            let tmp_file = File::open(&self.tmp_path)?;
            let mut reader = std::io::BufReader::new(tmp_file);
            reader.seek_relative(5)?; // Skip header
            let original_size = reader.get_ref().metadata()?.len() - 5;
            let outfile = File::create(&self.path)?;
            let mut writer = std::io::BufWriter::new(outfile);
            writer.write_all(b"SPCK")?;
            writer.write_all(&[1])?; // Compression flag
            let compress_size_loc = writer.stream_position()?;
            writer.write_all(&0u64.to_le_bytes())?; // Placeholder for compressed size
            writer.write_all(&original_size.to_le_bytes())?;
            let cur_loc = writer.stream_position()?;
            let mut encoder = zstd::stream::write::Encoder::new(&mut writer, level)?;
            std::io::copy(&mut reader, &mut encoder)?;
            encoder.finish()?;
            writer.flush()?;
            let compressed_size = writer.stream_position()? - cur_loc;
            writer.seek(std::io::SeekFrom::Start(compress_size_loc))?;
            writer.write_all(&compressed_size.to_le_bytes())?; // Write the actual compressed size
        }
        std::fs::remove_file(&self.tmp_path)?; // Clean up the temporary file
        Ok(())
    }
}
