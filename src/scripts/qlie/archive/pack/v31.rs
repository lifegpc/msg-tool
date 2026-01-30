use super::encryption::*;
use super::types::*;
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use anyhow::Result;
use rand::Rng;
use std::io::{Seek, Write};

struct MListEntry<T> {
    back: *mut MListEntry<T>,
    next: *mut MListEntry<T>,
    data: T,
}

struct MList<T> {
    head: *mut MListEntry<T>,
    depth: usize,
}

impl<T> MList<T> {
    pub fn new() -> Self {
        Self {
            head: std::ptr::null_mut(),
            depth: 0,
        }
    }

    pub fn push(&mut self, data: T, way: bool) -> usize {
        let entry = Box::new(MListEntry {
            back: std::ptr::null_mut(),
            next: std::ptr::null_mut(),
            data,
        });
        let entry_ptr = Box::into_raw(entry);
        if self.head.is_null() {
            self.head = entry_ptr;
            unsafe {
                (*self.head).back = self.head;
                (*self.head).next = self.head;
            }
        } else {
            if way {
                unsafe {
                    (*(*self.head).back).next = entry_ptr;
                    (*entry_ptr).back = (*self.head).back;
                    (*entry_ptr).next = self.head;
                    (*self.head).back = entry_ptr;
                }
            } else {
                unsafe {
                    (*(*self.head).back).next = entry_ptr;
                    (*entry_ptr).back = (*self.head).back;
                    (*entry_ptr).next = self.head;
                    (*self.head).back = entry_ptr;
                    self.head = entry_ptr;
                }
            }
        }
        self.depth += 1;
        self.depth
    }

    pub fn pop(&mut self, way: bool) -> Option<T> {
        if self.head.is_null() {
            return None;
        }
        if self.depth > 0 {
            self.depth -= 1;
            let ret;
            if way {
                unsafe {
                    ret = (*self.head).back;
                    (*(*ret).back).next = (*ret).next;
                    (*self.head).back = (*ret).back;
                }
            } else {
                unsafe {
                    ret = self.head;
                    (*(*ret).back).next = (*ret).next;
                    (*(*ret).next).back = (*ret).back;
                    self.head = (*ret).next;
                }
            }
            if self.depth == 0 {
                self.head = std::ptr::null_mut();
            }
            let boxed = unsafe { Box::from_raw(ret) };
            return Some(boxed.data);
        }
        None
    }
}

impl<T> Drop for MList<T> {
    fn drop(&mut self) {
        if self.head.is_null() {
            return;
        }
        let mut current = self.head;
        loop {
            unsafe {
                let next = (*current).next;
                let _ = Box::from_raw(current);
                if next == self.head {
                    break;
                }
                current = next;
            }
        }
    }
}

pub struct QliePackArchiveWriterV31<T: Write + Seek> {
    writer: T,
    encryption: Encryption31,
    qkey: QlieKey,
    header: QlieHeader,
    hash: QlieHash14,
    has_key_file: bool,
    entries: Vec<QlieEntry>,
    key: u32,
    common_key: Option<Vec<u8>>,
}

struct FilenameEntry {
    name: Vec<u16>,
    hash: u32,
    index: u32,
}

fn get_pos(hash: u32, count: u32) -> u32 {
    let v = (hash as u16 as u32)
        .wrapping_add(hash >> 8)
        .wrapping_add(hash >> 16);
    v % count
}

impl<T: Write + Seek> QliePackArchiveWriterV31<T> {
    pub fn new(writer: T, files: &[&str], config: &ExtraConfig) -> Result<Self> {
        let has_key_file = files.iter().any(|f| *f == QLIE_KEY_FILE);
        let mut file_count = files.len() as u32;
        if !has_key_file {
            if config.qlie_pack_keyfile.is_none() {
                anyhow::bail!(
                    "Qlie Pack Archive key file is required but not provided. Put a key file named '{}' in the directory or specify the path using '--qlie-pack-keyfile' option.",
                    QLIE_KEY_FILE
                );
            }
            // Add 1 for the key file
            file_count += 1;
        }
        let header = QlieHeader {
            signature: *b"FilePackVer3.1\x00\x00",
            file_count,
            index_offset: 0,
        };
        let encryption = Encryption31::new();
        let mut qkey = QlieKey {
            signature: *QLIE_KEY_SIGNATURE,
            hash_size: 0,
            key: [0; 0x400],
        };
        rand::rng().fill(&mut qkey.key[..0x100]);
        let key = encryption.compute_hash(&qkey.key[..0x100])? & 0xFFFFFFF;
        encrypt(&mut qkey.signature, key)?;
        let mut entries = Vec::new();
        let mut list = Vec::with_capacity(256);
        for _ in 0..256 {
            list.push(MList::<FilenameEntry>::new());
        }
        let key_entry = QlieEntry {
            name: QLIE_KEY_FILE.to_string(),
            ..Default::default()
        };
        entries.push(key_entry);
        let key_filename: Vec<_> = QLIE_KEY_FILE.encode_utf16().collect();
        let key_hash = encryption.compute_name_hash(&key_filename)?;
        let key_name_entry = FilenameEntry {
            name: key_filename,
            hash: key_hash,
            index: 0,
        };
        let pos = get_pos(key_hash, 256);
        list[pos as usize].push(key_name_entry, true);
        for name in files {
            if *name == QLIE_KEY_FILE {
                continue;
            }
            let filename: Vec<_> = name.encode_utf16().collect();
            let name_hash = encryption.compute_name_hash(&filename)?;
            let entry = QlieEntry {
                name: name.to_string(),
                ..Default::default()
            };
            entries.push(entry);
            let name_entry = FilenameEntry {
                name: filename,
                hash: name_hash,
                index: (entries.len() - 1) as u32,
            };
            let pos = get_pos(name_hash, 256);
            list[pos as usize].push(name_entry, true);
        }
        let mut hash_data = MemWriter::new();
        for mut list in list {
            hash_data.write_u32(list.depth as u32)?;
            while let Some(entry) = list.pop(false) {
                hash_data.write_u16(entry.name.len() as u16)?;
                hash_data.write_struct(&entry.name, false, Encoding::Utf16LE, &None)?;
                hash_data.write_u64(entry.index as u64 * 4)?;
                hash_data.write_u32(entry.hash)?;
            }
        }
        for i in 0..file_count {
            hash_data.write_u32(i)?;
        }
        let mut hash_data = hash_data.into_inner();
        encrypt(&mut hash_data, 0x0428)?;
        let hash = QlieHash14 {
            signature: *HASH_VER_1_4_SIGNATURE,
            table_size: 256,
            file_count: header.file_count,
            index_size: header.file_count * 4,
            hash_data_size: hash_data.len() as u32,
            is_compressed: 0,
            unk: [0; 32],
            hash_data,
        };
        qkey.hash_size = hash.hash_data_size + 68;
        let mut inner = Self {
            writer,
            encryption,
            qkey,
            header,
            hash,
            has_key_file,
            entries,
            key,
            common_key: None,
        };
        if !has_key_file {
            let key_path = config.qlie_pack_keyfile.as_ref().unwrap();
            let key_data = std::fs::read(key_path)?;
            inner.write_key(key_data)?;
        }
        Ok(inner)
    }

    fn write_key(&mut self, key_data: Vec<u8>) -> Result<()> {
        let entry = &mut self.entries[0];
        entry.size = key_data.len() as u32;
        entry.offset = self.writer.stream_position()?;
        entry.unpacked_size = entry.size;
        entry.is_packed = 0;
        entry.is_encrypted = 1;
        self.common_key = Some(get_common_key(&key_data)?);
        let hasher = Encryption31Hasher::new();
        let size = entry.size;
        let compute = EntryWriter {
            entry,
            inner: &mut self.writer,
            hasher,
        };
        let mut encryptor =
            Encryption31EncryptV1::new(compute, size, QLIE_KEY_FILE.to_string(), self.key)?;
        encryptor.write_all(&key_data)?;
        Ok(())
    }
}

struct Writer<'a> {
    inner: Box<dyn Write + 'a>,
    mem: MemWriter,
}

impl std::fmt::Debug for Writer<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Writer").field("mem", &self.mem).finish()
    }
}

impl<'a> Write for Writer<'a> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.mem.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.mem.flush()
    }
}

impl<'a> Seek for Writer<'a> {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        self.mem.seek(pos)
    }

    fn stream_position(&mut self) -> std::io::Result<u64> {
        self.mem.stream_position()
    }

    fn rewind(&mut self) -> std::io::Result<()> {
        self.mem.rewind()
    }
}

impl<'a> Drop for Writer<'a> {
    fn drop(&mut self) {
        let _ = self.inner.write_all(&self.mem.data);
        let _ = self.inner.flush();
    }
}

struct Writer2<'a, T: Write + Seek> {
    inner: &'a mut QliePackArchiveWriterV31<T>,
    entry_idx: usize,
    mem: MemWriter,
    is_v1: bool,
}

impl<'a, T: Write + Seek> Writer2<'a, T> {
    fn close(&mut self) -> Result<()> {
        let entry = &mut self.inner.entries[self.entry_idx];
        entry.size = self.mem.data.len() as u32;
        entry.offset = self.inner.writer.stream_position()?;
        entry.unpacked_size = entry.size;
        entry.is_packed = 0;
        let hasher = Encryption31Hasher::new();
        let size = entry.size;
        let compute = EntryWriter {
            entry,
            inner: &mut self.inner.writer,
            hasher,
        };
        if self.is_v1 {
            compute.entry.is_encrypted = 1;
            let name = compute.entry.name.clone();
            let mut encryptor = Encryption31EncryptV1::new(compute, size, name, self.inner.key)?;
            encryptor.write_all(&self.mem.data)?;
            self.inner.common_key = Some(get_common_key(&self.mem.data)?);
        } else {
            compute.entry.is_encrypted = 2;
            let name = compute.entry.name.clone();
            let common_key = self
                .inner
                .common_key
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Common key is not available"))?;
            let mut encryptor = Encryption31EncryptV2::new(
                compute,
                size,
                name,
                self.inner.key,
                common_key.to_vec(),
            )?;
            encryptor.write_all(&self.mem.data)?;
        }
        Ok(())
    }
}

impl<T: Write + Seek> std::fmt::Debug for Writer2<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Writer").field("mem", &self.mem).finish()
    }
}

impl<'a, T: Write + Seek> Write for Writer2<'a, T> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.mem.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.mem.flush()
    }
}

impl<'a, T: Write + Seek> Seek for Writer2<'a, T> {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        self.mem.seek(pos)
    }

    fn stream_position(&mut self) -> std::io::Result<u64> {
        self.mem.stream_position()
    }

    fn rewind(&mut self) -> std::io::Result<()> {
        self.mem.rewind()
    }
}

impl<'a, T: Write + Seek> Drop for Writer2<'a, T> {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

struct EntryWriter<'a, T: Write> {
    entry: &'a mut QlieEntry,
    inner: T,
    hasher: Encryption31Hasher,
}

impl<'a, T: Write> Write for EntryWriter<'a, T> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let writed = self.inner.write(buf)?;
        self.hasher
            .update(&buf[..writed])
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        Ok(writed)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

impl<'a, T: Write> Drop for EntryWriter<'a, T> {
    fn drop(&mut self) {
        if let Ok(hash) = self.hasher.finalize() {
            self.entry.hash = hash;
        }
    }
}

impl<T: Write + Seek> Archive for QliePackArchiveWriterV31<T> {
    fn prelist<'a>(&'a self) -> Result<Option<Box<dyn Iterator<Item = Result<String>> + 'a>>> {
        if !self.has_key_file {
            Ok(None)
        } else {
            let iter = std::iter::once(Ok(QLIE_KEY_FILE.to_string()));
            Ok(Some(Box::new(iter)))
        }
    }

    fn new_file<'a>(
        &'a mut self,
        name: &str,
        size: Option<u64>,
    ) -> Result<Box<dyn WriteSeek + 'a>> {
        let inner = self.new_file_non_seek(name, size)?;
        Ok(Box::new(Writer {
            inner,
            mem: MemWriter::new(),
        }))
    }

    fn new_file_non_seek<'a>(
        &'a mut self,
        name: &str,
        size: Option<u64>,
    ) -> Result<Box<dyn Write + 'a>> {
        if self.common_key.is_none() {
            if name != QLIE_KEY_FILE {
                anyhow::bail!("Common key is not available before writing key file");
            }
            let entry_idx = self
                .entries
                .iter()
                .position(|e| e.name == name)
                .ok_or_else(|| anyhow::anyhow!("File {} not found in entries", name))?;
            return Ok(Box::new(Writer2 {
                inner: self,
                entry_idx,
                mem: MemWriter::new(),
                is_v1: true,
            }));
        }
        if size.is_none() {
            let entry_idx = self
                .entries
                .iter()
                .position(|e| e.name == name)
                .ok_or_else(|| anyhow::anyhow!("File {} not found in entries", name))?;
            return Ok(Box::new(Writer2 {
                inner: self,
                entry_idx,
                mem: MemWriter::new(),
                is_v1: false,
            }));
        }
        let entry_idx = self
            .entries
            .iter()
            .position(|e| e.name == name)
            .ok_or_else(|| anyhow::anyhow!("File {} not found in entries", name))?;
        let entry = &mut self.entries[entry_idx];
        entry.size = size.unwrap() as u32;
        entry.offset = self.writer.stream_position()?;
        entry.unpacked_size = entry.size;
        entry.is_packed = 0;
        entry.is_encrypted = 2;
        let common_key = self
            .common_key
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Common key is not available"))?;
        let hasher = Encryption31Hasher::new();
        let size = entry.size;
        let compute = EntryWriter {
            entry,
            inner: &mut self.writer,
            hasher,
        };
        let encryptor = Encryption31EncryptV2::new(
            compute,
            size,
            name.to_string(),
            self.key,
            common_key.to_vec(),
        )?;
        Ok(Box::new(encryptor))
    }

    fn write_header(&mut self) -> Result<()> {
        self.header.index_offset = self.writer.stream_position()?;
        for entry in &self.entries {
            let name_length = entry.name.encode_utf16().count() as u16;
            self.writer.write_u16(name_length)?;
            let mut encoded = encode_string(Encoding::Utf16LE, &entry.name, true)?;
            self.encryption
                .encrypt_name(&mut encoded, self.key as i32)?;
            self.writer.write_all(&encoded)?;
            self.writer.write_u64(entry.offset)?;
            self.writer.write_u32(entry.size)?;
            self.writer.write_u32(entry.unpacked_size)?;
            self.writer.write_u32(entry.is_packed)?;
            self.writer.write_u32(entry.is_encrypted)?;
            self.writer.write_u32(entry.hash)?;
        }
        self.writer
            .write_struct(&self.hash, false, Encoding::Utf8, &None)?;
        self.writer
            .write_struct(&self.qkey, false, Encoding::Utf8, &None)?;
        self.writer
            .write_struct(&self.header, false, Encoding::Utf8, &None)?;
        Ok(())
    }
}

#[test]
fn test_drop_mlist() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicI32, Ordering};
    let t = Arc::new(AtomicI32::new(0));
    struct Test {
        value: i32,
        t: Arc<AtomicI32>,
    }

    impl Test {
        fn new(value: i32, t: Arc<AtomicI32>) -> Self {
            Self { value, t }
        }
    }

    impl Drop for Test {
        fn drop(&mut self) {
            self.t.fetch_add(self.value, Ordering::SeqCst);
        }
    }
    {
        let mut list: MList<Test> = MList::new();
        list.push(Test::new(1, t.clone()), true);
        list.push(Test::new(2, t.clone()), true);
        list.push(Test::new(3, t.clone()), true);
    }
    let v = t.load(Ordering::SeqCst);
    assert_eq!(v, 6);
}

#[test]
fn test_mlist() {
    let mut list = MList::new();
    list.push(1, true);
    list.push(2, true);
    list.push(3, true);
    assert_eq!(list.depth, 3);
    assert_eq!(list.pop(false), Some(1));
    assert_eq!(list.depth, 2);
    assert_eq!(list.pop(false), Some(2));
    assert_eq!(list.depth, 1);
    assert_eq!(list.pop(false), Some(3));
    assert_eq!(list.depth, 0);
    assert_eq!(list.pop(false), None);
}
