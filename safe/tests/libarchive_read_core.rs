#![allow(warnings, clippy::all)]

#[path = "libarchive/read_mainstream/mod.rs"]
mod read_mainstream_support;
#[path = "support/mod.rs"]
mod support;
#[path = "libarchive/write_disk/mod.rs"]
mod write_disk_support;

use std::ffi::{c_void, CString};
use std::ptr;

use archive::common::error::{ARCHIVE_EOF, ARCHIVE_OK};
use archive::entry::to_wide_null;
use archive::ffi::archive_entry_api as entry;
use archive::ffi::archive_read as read;

unsafe fn read_single_pathname(reader: *mut archive::ffi::archive) -> String {
    let mut entry_ptr = ptr::null_mut();
    assert_eq!(
        ARCHIVE_OK,
        read::archive_read_next_header(reader, &mut entry_ptr)
    );
    read_mainstream_support::entry_pathname(entry_ptr)
}

#[test]
fn reader_open_variants_read_the_same_archive() {
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
            read::archive_read_open_memory2(
                reader,
                archive_bytes.as_ptr().cast(),
                archive_bytes.len(),
                7,
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
        let fd = libc::open(archive_path_c.as_ptr(), libc::O_RDONLY | libc::O_CLOEXEC);
        assert!(fd >= 0);
        assert_eq!(ARCHIVE_OK, read::archive_read_open_fd(reader, fd, 10240));
        assert_eq!("payload.txt", read_single_pathname(reader));
        assert_eq!(ARCHIVE_OK, read::archive_read_free(reader));
        libc::close(fd);

        let reader = read::archive_read_new();
        assert!(!reader.is_null());
        assert_eq!(ARCHIVE_OK, read::archive_read_support_filter_all(reader));
        assert_eq!(ARCHIVE_OK, read::archive_read_support_format_all(reader));
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_open_file(reader, archive_path_c.as_ptr(), 10240)
        );
        assert_eq!("payload.txt", read_single_pathname(reader));
        assert_eq!(ARCHIVE_OK, read::archive_read_free(reader));

        let reader = read::archive_read_new();
        assert!(!reader.is_null());
        assert_eq!(ARCHIVE_OK, read::archive_read_support_filter_all(reader));
        assert_eq!(ARCHIVE_OK, read::archive_read_support_format_all(reader));
        let mut filenames = support::CStringArray::new(&[archive_path_str]);
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_open_filenames(reader, filenames.as_mut_ptr().cast(), 10240)
        );
        assert_eq!("payload.txt", read_single_pathname(reader));
        assert_eq!(ARCHIVE_OK, read::archive_read_free(reader));

        let reader = read::archive_read_new();
        assert!(!reader.is_null());
        assert_eq!(ARCHIVE_OK, read::archive_read_support_filter_all(reader));
        assert_eq!(ARCHIVE_OK, read::archive_read_support_format_all(reader));
        let wide_path = to_wide_null(archive_path_str);
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_open_filename_w(reader, wide_path.as_ptr(), 10240)
        );
        assert_eq!("payload.txt", read_single_pathname(reader));
        assert_eq!(ARCHIVE_OK, read::archive_read_free(reader));
    }
}

#[test]
fn reader_callbacks_option_wrappers_and_callback_data_plumbing_work() {
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
            read::archive_read_set_options(reader, c"joliet".as_ptr())
        );
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_add_passphrase(reader, c"secret".as_ptr())
        );

        let mut state = read_mainstream_support::CallbackReader::new(archive_bytes, 11);
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_set_passphrase_callback(
                reader,
                (&mut state as *mut read_mainstream_support::CallbackReader).cast(),
                Some(read_mainstream_support::passphrase_callback),
            )
        );
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_set_open_callback(
                reader,
                Some(read_mainstream_support::open_callback)
            )
        );
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_set_read_callback(
                reader,
                Some(read_mainstream_support::read_callback)
            )
        );
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_set_skip_callback(
                reader,
                Some(read_mainstream_support::skip_callback)
            )
        );
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_set_close_callback(
                reader,
                Some(read_mainstream_support::close_callback),
            )
        );
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_set_callback_data(
                reader,
                (&mut state as *mut read_mainstream_support::CallbackReader).cast(),
            )
        );
        assert_eq!(ARCHIVE_OK, read::archive_read_open1(reader));

        let mut entry_ptr = ptr::null_mut();
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_next_header(reader, &mut entry_ptr)
        );
        assert_eq!(
            "callbacks.txt",
            read_mainstream_support::entry_pathname(entry_ptr)
        );
        assert_eq!(ARCHIVE_OK, read::archive_read_data_skip(reader));
        assert_eq!(
            ARCHIVE_EOF,
            read::archive_read_next_header(reader, &mut entry_ptr)
        );
        assert_eq!(1, state.opens);
        assert!(state.read_calls > 0);
        assert_eq!(ARCHIVE_OK, read::archive_read_free(reader));
        assert_eq!(1, state.closes);

        let reader = read::archive_read_new();
        assert!(!reader.is_null());
        assert_eq!(ARCHIVE_OK, read::archive_read_support_filter_all(reader));
        assert_eq!(ARCHIVE_OK, read::archive_read_support_format_all(reader));
        let mut first = 1u8;
        let mut second = 2u8;
        let mut third = 3u8;
        let mut fourth = 4u8;
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_set_callback_data(reader, (&mut second as *mut u8).cast::<c_void>())
        );
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_append_callback_data(
                reader,
                (&mut third as *mut u8).cast::<c_void>()
            )
        );
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_prepend_callback_data(
                reader,
                (&mut first as *mut u8).cast::<c_void>()
            )
        );
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_set_callback_data2(
                reader,
                (&mut second as *mut u8).cast::<c_void>(),
                1,
            )
        );
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_add_callback_data(
                reader,
                (&mut fourth as *mut u8).cast::<c_void>(),
                2,
            )
        );
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_set_switch_callback(
                reader,
                Some(read_mainstream_support::switch_callback),
            )
        );
        assert_eq!(ARCHIVE_OK, read::archive_read_free(reader));
    }
}
