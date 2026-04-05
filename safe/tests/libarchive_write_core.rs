#![allow(warnings, clippy::all)]

#[path = "support/mod.rs"]
mod support;
#[path = "libarchive/write_disk/mod.rs"]
mod write_disk_support;

use std::ffi::CString;
use std::ptr;

use archive::common::error::{ARCHIVE_FATAL, ARCHIVE_OK};
use archive::ffi::archive_common as common;
use archive::ffi::archive_entry_api as entry;
use archive::ffi::archive_read as read;
use archive::ffi::archive_write as write;

#[test]
fn write_open_memory_round_trips_through_reader_subset() {
    let archive =
        unsafe { write_disk_support::write_single_file_archive("payload.txt", b"hello world") };
    let (pathname, data) =
        unsafe { write_disk_support::read_first_file_from_memory(&archive.buffer[..archive.used]) };
    assert_eq!("payload.txt", pathname);
    assert_eq!(b"hello world", data.as_slice());
}

#[test]
fn write_open_callbacks_handle_short_writes() {
    unsafe {
        let writer = write::archive_write_new();
        assert!(!writer.is_null());
        assert_eq!(
            ARCHIVE_OK,
            write::archive_write_set_bytes_per_block(writer, 0)
        );
        assert_eq!(
            ARCHIVE_OK,
            write::archive_write_set_bytes_in_last_block(writer, 1)
        );
        assert_eq!(ARCHIVE_OK, write::archive_write_set_format_pax(writer));
        assert_eq!(ARCHIVE_OK, write::archive_write_add_filter_none(writer));

        let mut callback_state = write_disk_support::CallbackBuffer { bytes: Vec::new() };
        assert_eq!(
            ARCHIVE_OK,
            write::archive_write_open(
                writer,
                (&mut callback_state as *mut write_disk_support::CallbackBuffer).cast(),
                None,
                Some(write_disk_support::short_write_callback),
                None,
            )
        );

        let raw_entry = entry::archive_entry_new();
        assert!(!raw_entry.is_null());
        let pathname = CString::new("callbacks.txt").unwrap();
        entry::archive_entry_copy_pathname(raw_entry, pathname.as_ptr());
        entry::archive_entry_set_mode(raw_entry, entry::AE_IFREG | 0o644);
        entry::archive_entry_set_size(raw_entry, 128);
        assert_eq!(ARCHIVE_OK, write::archive_write_header(writer, raw_entry));

        let payload: Vec<u8> = (0..128u8).collect();
        assert_eq!(
            payload.len() as isize,
            write::archive_write_data(writer, payload.as_ptr().cast(), payload.len())
        );
        assert_eq!(ARCHIVE_OK, write::archive_write_close(writer));
        assert_eq!(ARCHIVE_OK, write::archive_write_free(writer));
        entry::archive_entry_free(raw_entry);

        let reader = read::archive_read_new();
        assert!(!reader.is_null());
        assert_eq!(ARCHIVE_OK, read::archive_read_support_filter_all(reader));
        assert_eq!(ARCHIVE_OK, read::archive_read_support_format_all(reader));
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_open_memory(
                reader,
                callback_state.bytes.as_ptr().cast(),
                callback_state.bytes.len(),
            )
        );

        let mut read_entry = ptr::null_mut();
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_next_header(reader, &mut read_entry)
        );
        let pathname = std::ffi::CStr::from_ptr(entry::archive_entry_pathname(read_entry))
            .to_string_lossy()
            .into_owned();
        assert_eq!("callbacks.txt", pathname);
        let mut roundtrip = vec![0; payload.len()];
        assert_eq!(
            payload.len() as isize,
            read::archive_read_data(reader, roundtrip.as_mut_ptr().cast(), roundtrip.len())
        );
        assert_eq!(payload, roundtrip);
        assert_eq!(ARCHIVE_OK, common::archive_read_free(reader));
    }
}

#[test]
fn write_format_name_and_extension_accept_supported_formats_and_reject_unknown_ones() {
    unsafe {
        let writer = write::archive_write_new();
        assert!(!writer.is_null());

        let pax = CString::new("pax").unwrap();
        assert_eq!(
            ARCHIVE_OK,
            write::archive_write_set_format_by_name(writer, pax.as_ptr())
        );

        let zip = CString::new("zip").unwrap();
        assert_eq!(
            ARCHIVE_OK,
            write::archive_write_set_format_by_name(writer, zip.as_ptr())
        );
        let _ = common::archive_write_free(writer);

        let writer = write::archive_write_new();
        assert!(!writer.is_null());
        let unknown = CString::new("definitely-not-a-format").unwrap();
        assert_eq!(
            ARCHIVE_FATAL,
            write::archive_write_set_format_by_name(writer, unknown.as_ptr())
        );
        let message = std::ffi::CStr::from_ptr(common::archive_error_string(writer))
            .to_string_lossy()
            .into_owned();
        assert!(message.contains("No such format 'definitely-not-a-format'"));
        let _ = common::archive_write_free(writer);

        let writer = write::archive_write_new();
        assert!(!writer.is_null());
        let tar_gz = CString::new("archive.tar.gz").unwrap();
        assert_eq!(
            ARCHIVE_OK,
            write::archive_write_set_format_filter_by_ext(writer, tar_gz.as_ptr())
        );

        let zip_ext = CString::new("archive.zip").unwrap();
        assert_eq!(
            ARCHIVE_OK,
            write::archive_write_set_format_filter_by_ext(writer, zip_ext.as_ptr())
        );
        let _ = common::archive_write_free(writer);

        let writer = write::archive_write_new();
        assert!(!writer.is_null());
        let unknown_ext = CString::new("archive.definitely-not-a-format").unwrap();
        assert_eq!(
            ARCHIVE_FATAL,
            write::archive_write_set_format_filter_by_ext(writer, unknown_ext.as_ptr())
        );
        let message = std::ffi::CStr::from_ptr(common::archive_error_string(writer))
            .to_string_lossy()
            .into_owned();
        assert!(message.contains("No such format 'archive.definitely-not-a-format'"));
        let _ = common::archive_write_free(writer);
    }
}
