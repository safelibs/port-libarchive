#![allow(warnings, clippy::all)]

#[path = "support/mod.rs"]
mod support;

use std::ffi::CString;

use archive::common::error::ARCHIVE_OK;
use archive::ffi::archive_common as ffi;
use archive::util::ArchiveHandle;

#[test]
fn version_strings_and_error_lifecycle_match_expectations() {
    let reader = ArchiveHandle::reader();
    let writer = ArchiveHandle::writer();

    unsafe {
        assert_eq!(ffi::ARCHIVE_VERSION_NUMBER, ffi::archive_version_number());

        let version = support::c_str(ffi::archive_version_string()).expect("version string");
        assert!(version.starts_with("libarchive 3.7.2"));

        let details = support::c_str(ffi::archive_version_details()).expect("version details");
        assert!(details.starts_with(&version));

        let fmt = CString::new("%s").unwrap();
        let message = CString::new("abcdefgh").unwrap();
        ffi::archive_set_error(reader.as_ptr(), 12, fmt.as_ptr(), message.as_ptr());
        assert_eq!(12, reader.errno());
        assert_eq!(Some(String::from("abcdefgh")), reader.error_string());

        ffi::archive_copy_error(writer.as_ptr(), reader.as_ptr());
        assert_eq!(12, writer.errno());
        assert_eq!(Some(String::from("abcdefgh")), writer.error_string());

        ffi::archive_clear_error(reader.as_ptr());
        assert_eq!(0, reader.errno());
        assert_eq!(None, reader.error_string());
        assert_eq!(12, writer.errno());
    }
}

#[test]
fn utility_string_sort_orders_null_terminated_arrays() {
    let mut values =
        support::CStringArray::new(&["dir/path9", "dir/path", "dir/path3", "dir/path2"]);
    let status = unsafe { ffi::archive_utility_string_sort(values.as_mut_ptr()) };
    assert_eq!(ARCHIVE_OK, status);
    assert_eq!(
        values.strings(),
        vec![
            String::from("dir/path"),
            String::from("dir/path2"),
            String::from("dir/path3"),
            String::from("dir/path9"),
        ]
    );
}

#[test]
fn base_archive_handles_expose_filter_and_version_helpers() {
    let read_disk = ArchiveHandle::read_disk();
    let write_disk = ArchiveHandle::write_disk();

    unsafe {
        assert_eq!(0, ffi::archive_filter_count(read_disk.as_ptr()));
        assert_eq!(0, ffi::archive_filter_code(read_disk.as_ptr(), 0));
        assert!(ffi::archive_filter_name(read_disk.as_ptr(), 0).is_null());
        assert_eq!(0, ffi::archive_position_compressed(write_disk.as_ptr()));
        assert_eq!(0, ffi::archive_position_uncompressed(write_disk.as_ptr()));
    }

    for version_fn in [
        unsafe { ffi::archive_bzlib_version() },
        unsafe { ffi::archive_liblz4_version() },
        unsafe { ffi::archive_liblzma_version() },
        unsafe { ffi::archive_libzstd_version() },
        unsafe { ffi::archive_zlib_version() },
    ] {
        if !version_fn.is_null() {
            assert!(!support::c_str(version_fn).unwrap().is_empty());
        }
    }
}

#[test]
fn ported_runner_resolves_case_phase_groups_from_manifest() {
    assert_eq!(
        "foundation",
        support::ported::phase_group_for_case("libarchive", "test_entry")
    );
    assert_eq!(
        "write_disk",
        support::ported::phase_group_for_case("libarchive", "test_archive_match_time")
    );
}
