#![allow(warnings, clippy::all)]

#[path = "support/mod.rs"]
mod support;
#[path = "libarchive/write_disk/mod.rs"]
mod write_disk_support;

use std::ffi::{CStr, CString};
use std::ptr;

use archive::common::error::{ARCHIVE_EOF, ARCHIVE_OK};
use archive::ffi::archive_entry_api as entry;
use archive::ffi::archive_read as read;

unsafe fn read_single_pathname(reader: *mut archive::ffi::archive) -> String {
    let mut entry_ptr = ptr::null_mut();
    assert_eq!(
        ARCHIVE_OK,
        read::archive_read_next_header(reader, &mut entry_ptr)
    );
    CStr::from_ptr(entry::archive_entry_pathname(entry_ptr))
        .to_string_lossy()
        .into_owned()
}

#[test]
fn reader_phase3_open_variants_read_the_same_archive() {
    let archive =
        unsafe { write_disk_support::write_single_file_archive("payload.txt", b"hello world") };
    let archive_bytes = &archive.buffer[..archive.used];
    let archive_path = support::write_temp_file("read-core.tar", archive_bytes);
    let archive_path_str = archive_path.to_str().expect("utf-8 temp path");
    let archive_path_c = CString::new(archive_path_str).unwrap();

    unsafe {
        let reader = read::archive_read_new();
        assert!(!reader.is_null());
        assert_eq!(ARCHIVE_OK, read::archive_read_support_filter_all(reader));
        assert_eq!(ARCHIVE_OK, read::archive_read_support_format_all(reader));
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_open_memory(
                reader,
                archive_bytes.as_ptr().cast(),
                archive_bytes.len()
            )
        );
        assert_eq!("payload.txt", read_single_pathname(reader));
        assert!(read::archive_read_header_position(reader) >= 0);
        assert_eq!(ARCHIVE_OK, read::archive_read_data_skip(reader));
        let mut eof_entry = ptr::null_mut();
        assert_eq!(
            ARCHIVE_EOF,
            read::archive_read_next_header(reader, &mut eof_entry)
        );
        assert_eq!(ARCHIVE_OK, read::archive_read_free(reader));

        let reader = read::archive_read_new();
        assert!(!reader.is_null());
        assert_eq!(ARCHIVE_OK, read::archive_read_support_filter_all(reader));
        assert_eq!(ARCHIVE_OK, read::archive_read_support_format_all(reader));
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_open_filename(reader, archive_path_c.as_ptr(), 10240)
        );
        assert_eq!("payload.txt", read_single_pathname(reader));
        assert_eq!(ARCHIVE_OK, read::archive_read_free(reader));
    }
}

#[test]
fn reader_phase3_next_header2_and_data_round_trip_entry_contents() {
    let archive = unsafe {
        write_disk_support::write_single_file_archive("callbacks.txt", b"callback payload")
    };
    let archive_bytes = &archive.buffer[..archive.used];

    unsafe {
        let reader = read::archive_read_new();
        assert!(!reader.is_null());
        assert_eq!(ARCHIVE_OK, read::archive_read_support_filter_all(reader));
        assert_eq!(ARCHIVE_OK, read::archive_read_support_format_all(reader));
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_open_memory(
                reader,
                archive_bytes.as_ptr().cast(),
                archive_bytes.len()
            )
        );

        let entry_ptr = entry::archive_entry_new();
        assert!(!entry_ptr.is_null());
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_next_header2(reader, entry_ptr)
        );
        assert_eq!(
            "callbacks.txt",
            CStr::from_ptr(entry::archive_entry_pathname(entry_ptr))
                .to_string_lossy()
                .as_ref()
        );
        let mut payload = [0u8; 32];
        let size = read::archive_read_data(reader, payload.as_mut_ptr().cast(), payload.len());
        assert_eq!(16, size);
        assert_eq!(b"callback payload", &payload[..size as usize]);
        assert_eq!(
            ARCHIVE_EOF,
            read::archive_read_next_header2(reader, entry_ptr)
        );

        entry::archive_entry_free(entry_ptr);
        assert_eq!(ARCHIVE_OK, read::archive_read_free(reader));
    }
}
