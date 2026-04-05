#[path = "support/mod.rs"]
mod support;

use std::ffi::{c_char, c_void, CStr, CString};
use std::ptr;

use archive::common::error::{ARCHIVE_EOF, ARCHIVE_OK};
use archive::ffi::archive_common as common;
use archive::ffi::archive_entry_api as entry;
use archive::ffi::archive_match_api as match_api;
use archive::ffi::archive_read as read;
use archive::ffi::archive_read_disk as read_disk;

unsafe extern "C" fn uname_lookup(_private: *mut c_void, _id: i64) -> *const c_char {
    c"TESTUSER".as_ptr()
}

unsafe extern "C" fn gname_lookup(_private: *mut c_void, _id: i64) -> *const c_char {
    c"TESTGROUP".as_ptr()
}

unsafe extern "C" fn metadata_filter(
    archive: *mut archive::ffi::archive,
    _private: *mut c_void,
    entry_ptr: *mut archive::ffi::archive_entry,
) -> i32 {
    let pathname = CStr::from_ptr(entry::archive_entry_pathname(entry_ptr))
        .to_string_lossy()
        .into_owned();
    if pathname.ends_with("skip.txt") {
        return 0;
    }
    if read_disk::archive_read_disk_can_descend(archive) == 1 {
        assert_eq!(ARCHIVE_OK, read_disk::archive_read_disk_descend(archive));
    }
    1
}

#[test]
fn read_disk_matching_and_entry_from_file_use_lookup_callbacks() {
    let temp = support::TempDir::new("read-disk");
    let _cwd = support::pushd(temp.path());
    support::make_dir(std::path::Path::new("root"));
    support::write_file(std::path::Path::new("root/keep.txt"), b"keep");
    support::write_file(std::path::Path::new("root/skip.txt"), b"skip");

    unsafe {
        let matcher = match_api::archive_match_new();
        assert!(!matcher.is_null());
        let pattern = CString::new("root/skip.txt").unwrap();
        assert_eq!(
            ARCHIVE_OK,
            match_api::archive_match_exclude_pattern(matcher, pattern.as_ptr())
        );
        assert_eq!(
            ARCHIVE_OK,
            match_api::archive_match_include_uname(matcher, c"TESTUSER".as_ptr())
        );
        assert_eq!(
            ARCHIVE_OK,
            match_api::archive_match_include_gname(matcher, c"TESTGROUP".as_ptr())
        );

        let scan_disk = read_disk::archive_read_disk_new();
        assert!(!scan_disk.is_null());
        assert_eq!(
            ARCHIVE_OK,
            read_disk::archive_read_disk_set_uname_lookup(
                scan_disk,
                ptr::null_mut(),
                Some(uname_lookup),
                None,
            )
        );
        assert_eq!(
            ARCHIVE_OK,
            read_disk::archive_read_disk_set_gname_lookup(
                scan_disk,
                ptr::null_mut(),
                Some(gname_lookup),
                None,
            )
        );
        assert_eq!(
            ARCHIVE_OK,
            read_disk::archive_read_disk_set_matching(scan_disk, matcher, None, ptr::null_mut())
        );
        assert_eq!(
            ARCHIVE_OK,
            read_disk::archive_read_disk_open(scan_disk, c"root".as_ptr())
        );

        let entry_ptr = entry::archive_entry_new();
        assert!(!entry_ptr.is_null());
        let mut seen = Vec::new();
        loop {
            let status = read::archive_read_next_header2(scan_disk, entry_ptr);
            if status == ARCHIVE_EOF {
                break;
            }
            assert_eq!(ARCHIVE_OK, status);
            let pathname = CStr::from_ptr(entry::archive_entry_pathname(entry_ptr))
                .to_string_lossy()
                .into_owned();
            seen.push(pathname.clone());
            if pathname.ends_with("keep.txt") {
                let mut buff = [0u8; 16];
                let size = read::archive_read_data(scan_disk, buff.as_mut_ptr().cast(), buff.len());
                assert_eq!(4, size);
                assert_eq!(b"keep", &buff[..4]);
            }
            if read_disk::archive_read_disk_can_descend(scan_disk) == 1 {
                assert_eq!(ARCHIVE_OK, read_disk::archive_read_disk_descend(scan_disk));
            }
        }
        assert_eq!(
            seen,
            vec![String::from("root"), String::from("root/keep.txt")]
        );
        assert_eq!(ARCHIVE_OK, common::archive_read_free(scan_disk));

        let entry_disk = read_disk::archive_read_disk_new();
        assert!(!entry_disk.is_null());
        assert_eq!(
            ARCHIVE_OK,
            read_disk::archive_read_disk_set_uname_lookup(
                entry_disk,
                ptr::null_mut(),
                Some(uname_lookup),
                None,
            )
        );
        assert_eq!(
            ARCHIVE_OK,
            read_disk::archive_read_disk_set_gname_lookup(
                entry_disk,
                ptr::null_mut(),
                Some(gname_lookup),
                None,
            )
        );

        entry::archive_entry_clear(entry_ptr);
        entry::archive_entry_copy_pathname(entry_ptr, c"root/keep.txt".as_ptr());
        assert_eq!(
            ARCHIVE_OK,
            read_disk::archive_read_disk_entry_from_file(entry_disk, entry_ptr, -1, ptr::null())
        );
        assert_eq!(4, entry::archive_entry_size(entry_ptr));
        assert_eq!(
            "TESTUSER",
            CStr::from_ptr(entry::archive_entry_uname(entry_ptr))
                .to_str()
                .unwrap()
        );
        assert_eq!(
            "TESTGROUP",
            CStr::from_ptr(entry::archive_entry_gname(entry_ptr))
                .to_str()
                .unwrap()
        );
        assert_eq!(
            0,
            match_api::archive_match_owner_excluded(matcher, entry_ptr)
        );
        assert_eq!(
            0,
            match_api::archive_match_path_excluded(matcher, entry_ptr)
        );

        entry::archive_entry_clear(entry_ptr);
        entry::archive_entry_copy_pathname(entry_ptr, c"root/skip.txt".as_ptr());
        assert_eq!(
            ARCHIVE_OK,
            read_disk::archive_read_disk_entry_from_file(entry_disk, entry_ptr, -1, ptr::null())
        );
        assert_eq!(
            1,
            match_api::archive_match_path_excluded(matcher, entry_ptr)
        );

        entry::archive_entry_free(entry_ptr);
        assert_eq!(ARCHIVE_OK, common::archive_read_free(entry_disk));
        assert_eq!(ARCHIVE_OK, common::archive_free(matcher));
    }
}

#[test]
fn read_disk_metadata_filter_can_descend_and_exclude_entries() {
    let temp = support::TempDir::new("read-disk-filter");
    let _cwd = support::pushd(temp.path());
    support::make_dir(std::path::Path::new("root"));
    support::write_file(std::path::Path::new("root/keep.txt"), b"payload");
    support::write_file(std::path::Path::new("root/skip.txt"), b"skip");

    unsafe {
        let disk = read_disk::archive_read_disk_new();
        assert!(!disk.is_null());
        assert_eq!(
            ARCHIVE_OK,
            read_disk::archive_read_disk_set_metadata_filter_callback(
                disk,
                Some(metadata_filter),
                ptr::null_mut(),
            )
        );
        assert_eq!(
            ARCHIVE_OK,
            read_disk::archive_read_disk_open(disk, c"root".as_ptr())
        );

        let entry_ptr = entry::archive_entry_new();
        assert!(!entry_ptr.is_null());
        let mut seen = Vec::new();
        loop {
            let status = read::archive_read_next_header2(disk, entry_ptr);
            if status == ARCHIVE_EOF {
                break;
            }
            assert_eq!(ARCHIVE_OK, status);
            seen.push(
                CStr::from_ptr(entry::archive_entry_pathname(entry_ptr))
                    .to_string_lossy()
                    .into_owned(),
            );
        }

        assert_eq!(
            seen,
            vec![String::from("root"), String::from("root/keep.txt")]
        );
        entry::archive_entry_free(entry_ptr);
        assert_eq!(ARCHIVE_OK, common::archive_read_free(disk));
    }
}
