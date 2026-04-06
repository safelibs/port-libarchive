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
fn advanced_reader_wrappers_accept_remaining_formats_and_options() {
    unsafe {
        let reader = read::archive_read_new();
        assert!(!reader.is_null());
        assert_eq!(
            7,
            archive::read::format::ADVANCED_READ_SUPPORT_EXPORTS.len()
        );

        assert_eq!(ARCHIVE_OK, read::archive_read_support_format_7zip(reader));
        assert_eq!(ARCHIVE_OK, read::archive_read_support_format_cab(reader));
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_support_format_iso9660(reader)
        );
        assert_eq!(ARCHIVE_OK, read::archive_read_support_format_lha(reader));
        assert_eq!(ARCHIVE_OK, read::archive_read_support_format_mtree(reader));
        assert_eq!(ARCHIVE_OK, read::archive_read_support_format_warc(reader));
        assert_eq!(ARCHIVE_OK, read::archive_read_support_format_xar(reader));

        assert_eq!(
            ARCHIVE_OK,
            options::archive_read_set_options(reader, c"joliet".as_ptr())
        );
        assert_eq!(ARCHIVE_OK, common::archive_read_free(reader));
    }
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
fn advanced_zip_option_wrappers_cover_compression_and_passphrase_callback() {
    unsafe {
        let writer = write::archive_write_new();
        assert!(!writer.is_null());

        assert_eq!(ARCHIVE_OK, write::archive_write_set_format_zip(writer));
        assert_eq!(
            ARCHIVE_OK,
            write::archive_write_set_options(writer, std::ptr::null())
        );
        assert_eq!(
            ARCHIVE_FAILED,
            write::archive_write_set_passphrase(writer, std::ptr::null())
        );
        assert_eq!(
            ARCHIVE_OK,
            write::archive_write_set_passphrase_callback(
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
fn advanced_warc_and_xar_roundtrip_single_entries() {
    unsafe {
        let warc =
            advanced_support::write_single_entry_archive("warc.txt", b"warc payload", |writer| {
                assert_eq!(ARCHIVE_OK, write::archive_write_set_format_warc(writer));
            });
        let (pathname, data) = advanced_support::first_entry_from_memory(&warc);
        assert_eq!("warc.txt", pathname);
        assert_eq!(b"warc payload", data.as_slice());

        let xar =
            advanced_support::write_single_entry_archive("xar.txt", b"xar payload", |writer| {
                assert_eq!(ARCHIVE_OK, write::archive_write_set_format_xar(writer));
            });
        let (pathname, data) = advanced_support::first_entry_from_memory(&xar);
        assert_eq!("xar.txt", pathname);
        assert_eq!(b"xar payload", data.as_slice());
    }
}

#[test]
fn advanced_mtree_emits_a_readable_manifest() {
    unsafe {
        let mtree =
            advanced_support::write_single_entry_archive("mtree.txt", b"mtree payload", |writer| {
                assert_eq!(ARCHIVE_OK, write::archive_write_set_format_mtree(writer));
            });
        let (pathname, _data) = advanced_support::first_entry_from_memory(&mtree);
        assert!(pathname.ends_with("mtree.txt"));
    }
}
