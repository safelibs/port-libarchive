use std::path::{Path, PathBuf};

use archive::common::error::{ARCHIVE_EOF, ARCHIVE_OK};
use archive::ffi::archive_common as common;
use archive::ffi::archive_entry_api as entry;
use archive::ffi::archive_read as read;
use archive::ffi::archive_write as write;
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

pub unsafe fn write_zisofs_iso(pathname: &str, contents: &[u8]) -> Vec<u8> {
    let writer = write::archive_write_new();
    assert!(!writer.is_null());
    assert_eq!(ARCHIVE_OK, write::archive_write_set_format_iso9660(writer));
    assert_eq!(
        ARCHIVE_OK,
        write::archive_write_set_option(
            writer,
            std::ptr::null(),
            c"zisofs".as_ptr(),
            c"1".as_ptr()
        )
    );
    assert_eq!(ARCHIVE_OK, write::archive_write_add_filter_none(writer));

    let mut buffer = vec![0u8; 1024 * 1024];
    let mut used = 0usize;
    assert_eq!(
        ARCHIVE_OK,
        write::archive_write_open_memory(
            writer,
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &mut used,
        )
    );

    let raw_entry = regular_file_entry(pathname, contents.len());
    assert_eq!(ARCHIVE_OK, write::archive_write_header(writer, raw_entry));
    assert_eq!(
        contents.len() as isize,
        write::archive_write_data(writer, contents.as_ptr().cast(), contents.len())
    );
    assert_eq!(ARCHIVE_OK, write::archive_write_close(writer));
    assert_eq!(ARCHIVE_OK, common::archive_write_free(writer));
    entry::archive_entry_free(raw_entry);

    buffer.truncate(used);
    buffer
}

pub unsafe fn first_entry_from_memory(bytes: &[u8]) -> (String, Vec<u8>) {
    let reader = read::archive_read_new();
    assert!(!reader.is_null());
    assert_eq!(ARCHIVE_OK, read::archive_read_support_filter_all(reader));
    assert_eq!(
        ARCHIVE_OK,
        read::archive_read_support_format_iso9660(reader)
    );
    assert_eq!(
        ARCHIVE_OK,
        read::archive_read_open_memory(reader, bytes.as_ptr().cast(), bytes.len())
    );

    let mut raw_entry = std::ptr::null_mut();
    loop {
        let status = read::archive_read_next_header(reader, &mut raw_entry);
        assert!(matches!(status, ARCHIVE_OK | ARCHIVE_EOF));
        assert_eq!(ARCHIVE_OK, status, "expected a regular file entry");
        if entry::archive_entry_filetype(raw_entry) == entry::AE_IFDIR {
            assert_eq!(ARCHIVE_OK, read::archive_read_data_skip(reader));
            continue;
        }

        let pathname = std::ffi::CStr::from_ptr(entry::archive_entry_pathname(raw_entry))
            .to_string_lossy()
            .into_owned();
        let mut data = vec![0u8; 4096];
        let read_size = read::archive_read_data(reader, data.as_mut_ptr().cast(), data.len());
        assert!(read_size >= 0);
        data.truncate(read_size as usize);

        assert_eq!(ARCHIVE_OK, common::archive_read_free(reader));
        return (pathname, data);
    }
}
