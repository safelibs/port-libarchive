use std::path::{Path, PathBuf};

use archive::ffi::archive_entry_api as entry;
use archive::ffi::archive_write_disk as write_disk;
use serde_json::Value;

pub const SECURE_WRITE_FLAGS: i32 = 0x0100 | 0x0200 | 0x10000 | 0x40000;

pub fn load_json(path: &str) -> Value {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let path = root.join(path);
    serde_json::from_slice(&std::fs::read(path).expect("read json")).expect("parse json")
}

pub fn cve_ids(value: &Value, key: &str) -> Vec<String> {
    value[key]
        .as_array()
        .expect("json array")
        .iter()
        .map(|row| row["cve_id"].as_str().expect("cve id").to_string())
        .collect()
}

pub unsafe fn secure_disk_writer() -> *mut archive::ffi::archive {
    let disk = write_disk::archive_write_disk_new();
    assert!(!disk.is_null());
    assert_eq!(
        0,
        write_disk::archive_write_disk_set_options(disk, SECURE_WRITE_FLAGS)
    );
    disk
}

pub unsafe fn regular_file_entry(path: &str, size: usize) -> *mut archive::ffi::archive_entry {
    let raw_entry = entry::archive_entry_new();
    assert!(!raw_entry.is_null());
    let path = std::ffi::CString::new(path).unwrap();
    entry::archive_entry_copy_pathname(raw_entry, path.as_ptr());
    entry::archive_entry_set_mode(raw_entry, entry::AE_IFREG | 0o644);
    entry::archive_entry_set_size(raw_entry, size as i64);
    raw_entry
}

pub unsafe fn symlink_entry(path: &str, target: &Path) -> *mut archive::ffi::archive_entry {
    let raw_entry = entry::archive_entry_new();
    assert!(!raw_entry.is_null());
    let path = std::ffi::CString::new(path).unwrap();
    let target = std::ffi::CString::new(target.to_string_lossy().to_string()).unwrap();
    entry::archive_entry_copy_pathname(raw_entry, path.as_ptr());
    entry::archive_entry_set_mode(raw_entry, entry::AE_IFLNK | 0o777);
    entry::archive_entry_set_size(raw_entry, 0);
    entry::archive_entry_set_symlink(raw_entry, target.as_ptr());
    raw_entry
}
