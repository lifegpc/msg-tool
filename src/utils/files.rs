use std::fs;
use std::io;
use std::path::Path;
use std::io::{Read, Write};

use crate::scripts::ALL_EXTS;

pub fn find_files(path: &String, recursive: bool) -> io::Result<Vec<String>> {
    let mut result = Vec::new();
    let dir_path = Path::new(&path);

    if dir_path.is_dir() {
        for entry in fs::read_dir(dir_path)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file()
                && path.extension().map_or(true, |ext| {
                    ALL_EXTS.contains(&ext.to_string_lossy().to_lowercase())
                })
            {
                if let Some(path_str) = path.to_str() {
                    result.push(path_str.to_string());
                }
            } else if recursive && path.is_dir() {
                if let Some(path_str) = path.to_str() {
                    let mut sub_files = find_files(&path_str.to_string(), recursive)?;
                    result.append(&mut sub_files);
                }
            }
        }
    }

    Ok(result)
}

pub fn collect_files(path: &String, recursive: bool) -> io::Result<(Vec<String>, bool)> {
    let pa = Path::new(path);
    if pa.is_dir() {
        return Ok((find_files(path, recursive)?, true));
    }
    if pa.is_file() {
        return Ok((vec![path.clone()], false));
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("Path {} is neither a file nor a directory", pa.display()),
    ))
}

pub fn read_file<F: AsRef<Path> + ?Sized>(f: &F) -> io::Result<Vec<u8>> {
    let mut content = Vec::new();
    if f.as_ref() == Path::new("-") {
        io::stdin().read_to_end(&mut content)?;
    } else {
        content = fs::read(f)?;
    }
    Ok(content)
}

pub fn write_file<F: AsRef<Path> + ?Sized>(f: &F) -> io::Result<Box<dyn Write>> {
    Ok(if f.as_ref() == Path::new("-") {
        Box::new(io::stdout())
    } else {
        Box::new(fs::File::create(f)?)
    })
}
