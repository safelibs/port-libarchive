#![allow(warnings, clippy::all)]

#[path = "libarchive/read_mainstream/mod.rs"]
mod read_mainstream_support;
#[path = "support/mod.rs"]
mod support;
#[path = "libarchive/write_disk/mod.rs"]
mod write_disk_support;

use std::ffi::CString;
use std::os::fd::AsRawFd;
use std::ptr;

use archive::common::error::{ARCHIVE_EOF, ARCHIVE_OK};
use archive::ffi::archive_read as read;

#[test]
fn reader_callback_open2_and_open1_cover_phase4_surface() {
    let archive =
        unsafe { write_disk_support::write_single_file_archive("phase4.txt", b"callback payload") };
    let archive_bytes = &archive.buffer[..archive.used];

    unsafe {
        let reader = read::archive_read_new();
        assert!(!reader.is_null());
        let mut state = read_mainstream_support::CallbackReader::new(archive_bytes, 5);
        assert_eq!(
            ARCHIVE_OK,
            read_mainstream_support::open_reader_with_callbacks(reader, &mut state)
        );
        let mut entry_ptr = ptr::null_mut();
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_next_header(reader, &mut entry_ptr)
        );
        assert_eq!(
            "phase4.txt",
            read_mainstream_support::entry_pathname(entry_ptr)
        );
        assert_eq!(ARCHIVE_OK, read::archive_read_free(reader));
        assert_eq!(1, state.opens);
        assert_eq!(1, state.closes);
        assert!(state.read_calls > 0);

        let reader = read::archive_read_new();
        assert!(!reader.is_null());
        assert_eq!(ARCHIVE_OK, read::archive_read_support_filter_all(reader));
        assert_eq!(ARCHIVE_OK, read::archive_read_support_format_all(reader));
        let mut state = read_mainstream_support::CallbackReader::new(archive_bytes, 7);
        read_mainstream_support::configure_reader_callbacks(reader, &mut state);
        assert_eq!(ARCHIVE_OK, read::archive_read_open1(reader));
        let mut entry_ptr = ptr::null_mut();
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_next_header(reader, &mut entry_ptr)
        );
        assert_eq!(
            "phase4.txt",
            read_mainstream_support::entry_pathname(entry_ptr)
        );
        assert_eq!(ARCHIVE_OK, read::archive_read_data_skip(reader));
        assert_eq!(ARCHIVE_OK, read::archive_read_free(reader));
        assert_eq!(1, state.opens);
        assert_eq!(1, state.closes);
        assert!(state.read_calls > 0);
    }
}

#[test]
fn reader_open_fd_and_open_memory2_read_the_same_entry() {
    let archive =
        unsafe { write_disk_support::write_single_file_archive("fd-memory.txt", b"phase4 data") };
    let archive_bytes = &archive.buffer[..archive.used];
    let archive_path = support::write_temp_file("read-mainstream.tar", archive_bytes);
    let archive_path_str = archive_path.to_str().expect("utf-8 temp path");
    let archive_path_c = CString::new(archive_path_str).unwrap();
    let archive_file = std::fs::File::open(&archive_path).unwrap();

    unsafe {
        let reader = read::archive_read_new();
        assert!(!reader.is_null());
        assert_eq!(ARCHIVE_OK, read::archive_read_support_filter_all(reader));
        assert_eq!(ARCHIVE_OK, read::archive_read_support_format_all(reader));
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_open_memory2(
                reader,
                archive_bytes.as_ptr().cast(),
                archive_bytes.len(),
                3
            )
        );
        let mut entry_ptr = ptr::null_mut();
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_next_header(reader, &mut entry_ptr)
        );
        assert_eq!(
            "fd-memory.txt",
            read_mainstream_support::entry_pathname(entry_ptr)
        );
        assert_eq!(ARCHIVE_OK, read::archive_read_data_skip(reader));
        assert_eq!(ARCHIVE_OK, read::archive_read_free(reader));

        let reader = read::archive_read_new();
        assert!(!reader.is_null());
        assert_eq!(ARCHIVE_OK, read::archive_read_support_filter_all(reader));
        assert_eq!(ARCHIVE_OK, read::archive_read_support_format_all(reader));
        let fd = libc::dup(archive_file.as_raw_fd());
        assert!(fd >= 0);
        assert_eq!(ARCHIVE_OK, read::archive_read_open_fd(reader, fd, 4096));
        let mut entry_ptr = ptr::null_mut();
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_next_header(reader, &mut entry_ptr)
        );
        assert_eq!(
            "fd-memory.txt",
            read_mainstream_support::entry_pathname(entry_ptr)
        );
        assert_eq!(ARCHIVE_OK, read::archive_read_free(reader));

        let reader = read::archive_read_new();
        assert!(!reader.is_null());
        assert_eq!(ARCHIVE_OK, read::archive_read_support_filter_all(reader));
        assert_eq!(ARCHIVE_OK, read::archive_read_support_format_all(reader));
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_open_file(reader, archive_path_c.as_ptr(), 10240)
        );
        let mut entry_ptr = ptr::null_mut();
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_next_header(reader, &mut entry_ptr)
        );
        assert_eq!(
            "fd-memory.txt",
            read_mainstream_support::entry_pathname(entry_ptr)
        );
        assert_eq!(ARCHIVE_OK, read::archive_read_free(reader));
    }
}
