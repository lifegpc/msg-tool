//! Utilities for File Operations
use crate::scripts::{ALL_EXTS, ARCHIVE_EXTS};
use std::fs;
use std::io;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Returns the relative path from `root` to `target`.
pub fn relative_path<P: AsRef<Path>, T: AsRef<Path>>(root: P, target: T) -> PathBuf {
    let root = root
        .as_ref()
        .canonicalize()
        .unwrap_or_else(|_| root.as_ref().to_path_buf());
    let target = target
        .as_ref()
        .canonicalize()
        .unwrap_or_else(|_| target.as_ref().to_path_buf());

    let mut root_components: Vec<_> = root.components().collect();
    let mut target_components: Vec<_> = target.components().collect();

    // Remove common prefix
    while !root_components.is_empty()
        && !target_components.is_empty()
        && root_components[0] == target_components[0]
    {
        root_components.remove(0);
        target_components.remove(0);
    }

    // Add ".." for each remaining root component
    let mut result = PathBuf::new();
    for _ in root_components {
        result.push("..");
    }

    // Add remaining target components
    for component in target_components {
        result.push(component);
    }

    result
}

/// Finds all files in the specified directory and its subdirectories.
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

/// Finds all archive files in the specified directory and its subdirectories.
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

/// Collects files from the specified path, either as a directory or a single file.
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

/// Finds all files with specific extensions in the specified directory and its subdirectories.
pub fn find_ext_files(path: &str, recursive: bool, exts: &[&str]) -> io::Result<Vec<String>> {
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
                        for ext in exts {
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

/// Collects files with specific extensions from the specified path, either as a directory or a single file.
pub fn collect_ext_files(
    path: &str,
    recursive: bool,
    exts: &[&str],
) -> io::Result<(Vec<String>, bool)> {
    let pa = Path::new(path);
    if pa.is_dir() {
        return Ok((find_ext_files(path, recursive, exts)?, true));
    }
    if pa.is_file() {
        return Ok((vec![path.to_string()], false));
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("Path {} is neither a file nor a directory", pa.display()),
    ))
}

/// Collects archive files from the specified path, either as a directory or a single file.
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

/// Reads the content of a file or standard input if the path is "-".
pub fn read_file<F: AsRef<Path> + ?Sized>(f: &F) -> io::Result<Vec<u8>> {
    let mut content = Vec::new();
    if f.as_ref() == Path::new("-") {
        io::stdin().read_to_end(&mut content)?;
    } else {
        content = fs::read(f)?;
    }
    Ok(content)
}

/// Writes content to a file or standard output if the path is "-".
pub fn write_file<F: AsRef<Path> + ?Sized>(f: &F) -> io::Result<Box<dyn Write>> {
    Ok(if f.as_ref() == Path::new("-") {
        Box::new(io::stdout())
    } else {
        Box::new(fs::File::create(f)?)
    })
}

/// Ensures that the parent directory for the specified path exists, creating it if necessary.
pub fn make_sure_dir_exists<F: AsRef<Path> + ?Sized>(f: &F) -> io::Result<()> {
    let path = f.as_ref();
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

/// Replace symbols not allowed in Windows path with underscores.
pub fn sanitize_path(path: &str) -> String {
    // Split path into components, preserving separators
    if path.is_empty() {
        return String::new();
    }

    let invalid_chars: &[char] = &['<', '>', '"', '|', '?', '*'];
    let mut result = String::with_capacity(path.len());

    let reserved_names: Vec<String> = {
        let mut v = vec!["CON", "PRN", "AUX", "NUL"]
            .into_iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        for i in 1..=9 {
            v.push(format!("COM{}", i));
            v.push(format!("LPT{}", i));
        }
        v
    };

    let bytes = path.as_bytes();
    let len = bytes.len();
    let mut start = 0usize;

    while start < len {
        // find next separator index
        let mut end = start;
        while end < len && bytes[end] != b'\\' && bytes[end] != b'/' {
            end += 1;
        }
        // segment is path[start..end]
        let seg = &path[start..end];

        // sanitize segment
        let mut s = String::with_capacity(seg.len());
        for (i, ch) in seg.chars().enumerate() {
            // allow drive letter colon like "C:" (i == 1, first char is ASCII letter)
            if ch == ':' {
                if i == 1 {
                    // check first char is ASCII letter
                    if seg
                        .chars()
                        .next()
                        .map(|c| c.is_ascii_alphabetic())
                        .unwrap_or(false)
                    {
                        s.push(':');
                        continue;
                    }
                }
                // otherwise treat as invalid
                s.push('_');
                continue;
            }
            // keep separators out of segment (shouldn't appear here)
            // replace control chars and other invalids
            if (ch as u32) < 32 || invalid_chars.contains(&ch) {
                s.push('_');
            } else {
                s.push(ch);
            }
        }

        // trim trailing spaces and dots (Windows disallows filenames ending with space or dot)
        while s.ends_with(' ') || s.ends_with('.') {
            s.pop();
        }

        if s.is_empty() {
            s.push('_');
        } else {
            // check reserved names (base name before first '.')
            let base = s.split('.').next().unwrap_or("").to_ascii_uppercase();
            if reserved_names.iter().any(|r| r == &base) {
                s = format!("_{}", s);
            }
        }

        result.push_str(&s);

        // append separator if present
        if end < len {
            // keep original separator (preserve '\' or '/')
            result.push(path.as_bytes()[end] as char);
            start = end + 1;
        } else {
            start = end;
        }
    }

    result
}
