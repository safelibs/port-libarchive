#![allow(warnings, clippy::all)]

#[path = "support/mod.rs"]
mod support;
#[path = "libarchive/write_disk/mod.rs"]
mod write_disk_support;

use std::ffi::{CStr, CString};
use std::path::Path;
use std::ptr;

use archive::common::error::{ARCHIVE_EOF, ARCHIVE_OK};
use archive::ffi::archive_common as common;
use archive::ffi::archive_entry_api as entry;
use archive::ffi::archive_read as read;
use archive::ffi::archive_write as write;
use archive::ffi::archive_write_disk as write_disk;

#[test]
fn read_extract2_uses_secure_write_disk_root() {
    let archive =
        unsafe { write_disk_support::write_single_file_archive("inner/file.txt", b"payload") };
    let temp = support::TempDir::new("extract-root");
    let _cwd = support::pushd(temp.path());

    unsafe {
        let reader = read::archive_read_new();
        assert!(!reader.is_null());
        assert_eq!(ARCHIVE_OK, read::archive_read_support_filter_all(reader));
        assert_eq!(ARCHIVE_OK, read::archive_read_support_format_all(reader));
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_open_memory(reader, archive.buffer.as_ptr().cast(), archive.used)
        );

        let disk = write_disk::archive_write_disk_new();
        assert!(!disk.is_null());
        assert_eq!(
            ARCHIVE_OK,
            write_disk::archive_write_disk_set_options(
                disk,
                write_disk_support::ARCHIVE_EXTRACT_SECURE_SYMLINKS
                    | write_disk_support::ARCHIVE_EXTRACT_SECURE_NODOTDOT
                    | write_disk_support::ARCHIVE_EXTRACT_SECURE_NOABSOLUTEPATHS
                    | write_disk_support::ARCHIVE_EXTRACT_SAFE_WRITES,
            )
        );

        let mut raw_entry = ptr::null_mut();
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_next_header(reader, &mut raw_entry)
        );
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_extract2(reader, raw_entry, disk)
        );
        assert_eq!(
            ARCHIVE_EOF,
            read::archive_read_next_header(reader, &mut raw_entry)
        );

        assert_eq!(
            b"payload",
            support::read_file(Path::new("inner/file.txt")).as_slice()
        );

        assert_eq!(ARCHIVE_OK, common::archive_read_free(reader));
        assert_eq!(ARCHIVE_OK, common::archive_write_free(disk));
    }
}

#[test]
fn write_disk_header_data_writes_files_and_rejects_parent_escape() {
    let temp = support::TempDir::new("write-disk");
    let parent = temp.path().parent().unwrap().join("write-disk-escape.txt");
    let _cwd = support::pushd(temp.path());

    unsafe {
        let disk = write_disk::archive_write_disk_new();
        assert!(!disk.is_null());
        assert_eq!(
            ARCHIVE_OK,
            write_disk::archive_write_disk_set_options(
                disk,
                write_disk_support::ARCHIVE_EXTRACT_SECURE_NODOTDOT
                    | write_disk_support::ARCHIVE_EXTRACT_SECURE_NOABSOLUTEPATHS
                    | write_disk_support::ARCHIVE_EXTRACT_SAFE_WRITES,
            )
        );

        let raw_entry = entry::archive_entry_new();
        assert!(!raw_entry.is_null());
        entry::archive_entry_copy_pathname(raw_entry, c"plain.txt".as_ptr());
        entry::archive_entry_set_mode(raw_entry, entry::AE_IFREG | 0o644);
        entry::archive_entry_set_size(raw_entry, 5);
        assert_eq!(ARCHIVE_OK, write::archive_write_header(disk, raw_entry));
        assert_eq!(
            5,
            write::archive_write_data(disk, b"hello".as_ptr().cast(), 5)
        );
        assert_eq!(ARCHIVE_OK, write::archive_write_finish_entry(disk));
        assert_eq!(
            b"hello",
            support::read_file(Path::new("plain.txt")).as_slice()
        );

        entry::archive_entry_clear(raw_entry);
        let escape = CString::new("../write-disk-escape.txt").unwrap();
        entry::archive_entry_copy_pathname(raw_entry, escape.as_ptr());
        entry::archive_entry_set_mode(raw_entry, entry::AE_IFREG | 0o644);
        entry::archive_entry_set_size(raw_entry, 4);
        assert_ne!(ARCHIVE_OK, write::archive_write_header(disk, raw_entry));
        assert!(!parent.exists());

        entry::archive_entry_free(raw_entry);
        assert_eq!(ARCHIVE_OK, common::archive_write_free(disk));
    }
}
