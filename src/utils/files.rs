use crate::scripts::{ALL_EXTS, ARCHIVE_EXTS};
use std::fs;
use std::io;
use std::io::{Read, Write};
use std::path::Path;

pub fn find_files(path: &str, recursive: bool, no_ext_filter: bool) -> io::Result<Vec<String>> {
    let mut result = Vec::new();
    let dir_path = Path::new(&path);

    if dir_path.is_dir() {
        for entry in fs::read_dir(dir_path)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file()
                && (no_ext_filter
                    || path.file_name().map_or(false, |file| {
                        path.extension().map_or(true, |_| {
                            let file = file.to_string_lossy().to_lowercase();
                            for ext in ALL_EXTS.iter() {
                                if file.ends_with(&format!(".{}", ext)) {
                                    return true;
                                }
                            }
                            false
                        })
                    }))
            {
                if let Some(path_str) = path.to_str() {
                    result.push(path_str.to_string());
                }
            } else if recursive && path.is_dir() {
                if let Some(path_str) = path.to_str() {
                    let mut sub_files =
                        find_files(&path_str.to_string(), recursive, no_ext_filter)?;
                    result.append(&mut sub_files);
                }
            }
        }
    }

    Ok(result)
}

pub fn find_arc_files(path: &str, recursive: bool) -> io::Result<Vec<String>> {
    let mut result = Vec::new();
    let dir_path = Path::new(&path);

    if dir_path.is_dir() {
        for entry in fs::read_dir(dir_path)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file()
                && path.file_name().map_or(false, |file| {
                    path.extension().map_or(true, |_| {
                        let file = file.to_string_lossy().to_lowercase();
                        for ext in ARCHIVE_EXTS.iter() {
                            if file.ends_with(&format!(".{}", ext)) {
                                return true;
                            }
                        }
                        false
                    })
                })
            {
                if let Some(path_str) = path.to_str() {
                    result.push(path_str.to_string());
                }
            } else if recursive && path.is_dir() {
                if let Some(path_str) = path.to_str() {
                    let mut sub_files = find_arc_files(&path_str.to_string(), recursive)?;
                    result.append(&mut sub_files);
                }
            }
        }
    }

    Ok(result)
}

pub fn collect_files(
    path: &str,
    recursive: bool,
    no_ext_filter: bool,
) -> io::Result<(Vec<String>, bool)> {
    let pa = Path::new(path);
    if pa.is_dir() {
        return Ok((find_files(path, recursive, no_ext_filter)?, true));
    }
    if pa.is_file() {
        return Ok((vec![path.to_string()], false));
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("Path {} is neither a file nor a directory", pa.display()),
    ))
}

pub fn collect_arc_files(path: &str, recursive: bool) -> io::Result<(Vec<String>, bool)> {
    let pa = Path::new(path);
    if pa.is_dir() {
        return Ok((find_arc_files(path, recursive)?, true));
    }
    if pa.is_file() {
        return Ok((vec![path.to_string()], false));
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

pub fn make_sure_dir_exists<F: AsRef<Path> + ?Sized>(f: &F) -> io::Result<()> {
    let path = f.as_ref();
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}
