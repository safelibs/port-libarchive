#![allow(warnings, clippy::all)]

#[path = "libarchive/advanced/mod.rs"]
mod advanced_support;

use std::ffi::{c_char, c_void, CString};

use archive::common::error::{ARCHIVE_FAILED, ARCHIVE_OK};
use archive::ffi::archive_common as common;
use archive::ffi::archive_options as options;
use archive::ffi::archive_read as read;
use archive::ffi::archive_write as write;

unsafe extern "C" fn zip_passphrase_callback(
    _archive: *mut archive::ffi::archive,
    _client_data: *mut c_void,
) -> *const c_char {
    c"zip secret".as_ptr()
}

#[test]
fn advanced_writer_aliases_and_direct_exports_cover_remaining_formats() {
    unsafe {
        assert_eq!(9, archive::write::format::ADVANCED_WRITE_FORMAT_NAMES.len());
        assert_eq!(4, archive::write::format::ADVANCED_WRITE_EXTENSIONS.len());

        for name in archive::write::format::ADVANCED_WRITE_FORMAT_NAMES {
            let writer = write::archive_write_new();
            assert!(!writer.is_null());
            let name = CString::new(*name).unwrap();
            assert_eq!(
                ARCHIVE_OK,
                write::archive_write_set_format_by_name(writer, name.as_ptr())
            );
            assert_eq!(ARCHIVE_OK, common::archive_write_free(writer));
        }

        for filename in ["archive.7z", "archive.iso", "archive.zip", "archive.jar"] {
            let writer = write::archive_write_new();
            assert!(!writer.is_null());
            let filename = CString::new(filename).unwrap();
            assert_eq!(
                ARCHIVE_OK,
                write::archive_write_set_format_filter_by_ext(writer, filename.as_ptr())
            );
            assert_eq!(ARCHIVE_OK, common::archive_write_free(writer));
        }

        for setter in [
            write::archive_write_set_format_7zip as unsafe extern "C" fn(_) -> _,
            write::archive_write_set_format_iso9660,
            write::archive_write_set_format_mtree,
            write::archive_write_set_format_mtree_classic,
            write::archive_write_set_format_warc,
            write::archive_write_set_format_xar,
        ] {
            let writer = write::archive_write_new();
            assert!(!writer.is_null());
            assert_eq!(ARCHIVE_OK, setter(writer));
            assert_eq!(ARCHIVE_OK, common::archive_write_free(writer));
        }
    }
}

#[test]
fn advanced_zip_reader_convenience_exports_are_linked() {
    unsafe {
        for setter in [
            read::archive_read_support_format_zip as unsafe extern "C" fn(_) -> _,
            read::archive_read_support_format_zip_seekable,
            read::archive_read_support_format_zip_streamable,
        ] {
            let reader = read::archive_read_new();
            assert!(!reader.is_null());
            assert_eq!(ARCHIVE_OK, setter(reader));
            assert_eq!(ARCHIVE_OK, common::archive_read_free(reader));
        }
    }
}

#[test]
fn advanced_zip_option_wrappers_cover_compression_and_passphrase_callback() {
    unsafe {
        let writer = write::archive_write_new();
        assert!(!writer.is_null());

        assert_eq!(ARCHIVE_OK, write::archive_write_set_format_zip(writer));
        assert_eq!(
            ARCHIVE_OK,
            options::archive_write_set_options(writer, std::ptr::null())
        );
        assert_eq!(
            ARCHIVE_FAILED,
            options::archive_write_set_passphrase(writer, std::ptr::null())
        );
        assert_eq!(
            ARCHIVE_OK,
            options::archive_write_set_passphrase_callback(
                writer,
                std::ptr::null_mut(),
                Some(zip_passphrase_callback),
            )
        );
        assert_eq!(
            ARCHIVE_OK,
            write::archive_write_zip_set_compression_deflate(writer)
        );
        assert_eq!(
            ARCHIVE_OK,
            write::archive_write_zip_set_compression_store(writer)
        );
        assert_eq!(ARCHIVE_OK, common::archive_write_free(writer));
    }
}

#[test]
fn advanced_option_module_round_trips_real_iso9660_path() {
    let payload = b"wrapped payload";
    let iso = unsafe {
        advanced_support::write_single_entry_archive("wrapped.txt", payload, |writer| {
            let format = CString::new("iso9660").unwrap();
            assert_eq!(
                ARCHIVE_OK,
                write::archive_write_set_format_by_name(writer, format.as_ptr())
            );
            assert_eq!(
                ARCHIVE_OK,
                options::archive_write_set_option(
                    writer,
                    std::ptr::null(),
                    c"zisofs".as_ptr(),
                    c"1".as_ptr(),
                )
            );
        })
    };

    let (pathname, data) = unsafe {
        advanced_support::first_entry_from_memory_with_reader(&iso, |reader| {
            let layout = CString::new("7:4096").unwrap();
            assert_eq!(
                ARCHIVE_OK,
                options::archive_read_set_format_option(
                    reader,
                    c"iso9660".as_ptr(),
                    c"zisofs-layout".as_ptr(),
                    layout.as_ptr(),
                )
            );
        })
    };

    assert_eq!("wrapped.txt", pathname);
    assert_eq!(payload, data.as_slice());
}
