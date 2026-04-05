#![allow(warnings, clippy::all)]

#[path = "libarchive/security/mod.rs"]
mod security_support;
#[path = "support/mod.rs"]
mod support;

use std::collections::BTreeSet;
use std::ffi::CString;
use std::path::Path;

use archive::common::error::{ARCHIVE_FAILED, ARCHIVE_OK};
use archive::ffi::archive_common as common;
use archive::ffi::archive_entry_api as entry;
use archive::ffi::archive_read as read;
use archive::ffi::archive_write as write;

#[test]
fn matrix_covers_relevant_cves_and_verification_targets_are_present() {
    let relevant = security_support::load_json("relevant_cves.json");
    let matrix = security_support::load_json("safe/generated/cve_matrix.json");

    let relevant_ids = security_support::cve_ids(&relevant, "records")
        .into_iter()
        .collect::<BTreeSet<_>>();
    let matrix_ids = security_support::cve_ids(&matrix, "rows")
        .into_iter()
        .collect::<BTreeSet<_>>();

    assert_eq!(relevant_ids, matrix_ids);

    for row in matrix["rows"].as_array().expect("matrix rows") {
        assert!(row["targeted_area"]
            .as_str()
            .is_some_and(|value| !value.is_empty()));
        assert!(row["required_controls"]
            .as_array()
            .is_some_and(|controls| !controls.is_empty()));
        assert!(row["verification"]
            .as_str()
            .is_some_and(|value| !value.is_empty()));
    }
}

#[test]
fn filesystem_extraction_guards_block_absolute_parent_symlink_and_hardlink_escape() {
    let temp = support::TempDir::new("cve-disk-root");
    let outside = support::TempDir::new("cve-disk-outside");
    let _cwd = support::pushd(temp.path());

    let original_umask = unsafe {
        let current = libc::umask(0o077);
        libc::umask(current);
        current
    };

    unsafe {
        let disk = security_support::secure_disk_writer();

        let absolute_path = outside.path().join("absolute.txt");
        let raw_entry =
            security_support::regular_file_entry(absolute_path.to_string_lossy().as_ref(), 4);
        assert_ne!(ARCHIVE_OK, write::archive_write_header(disk, raw_entry));
        entry::archive_entry_free(raw_entry);
        assert!(!absolute_path.exists());

        let raw_entry = security_support::regular_file_entry("../parent-escape.txt", 4);
        assert_ne!(ARCHIVE_OK, write::archive_write_header(disk, raw_entry));
        entry::archive_entry_free(raw_entry);
        assert!(!temp
            .path()
            .parent()
            .unwrap()
            .join("parent-escape.txt")
            .exists());

        support::symlink(Path::new("pivot"), outside.path());
        let raw_entry = security_support::regular_file_entry("pivot/symlink-escape.txt", 4);
        assert_ne!(ARCHIVE_OK, write::archive_write_header(disk, raw_entry));
        entry::archive_entry_free(raw_entry);
        assert!(!outside.path().join("symlink-escape.txt").exists());

        let outside_target = outside.path().join("hardlink-target.txt");
        support::write_file(&outside_target, b"outside");
        let raw_entry = security_support::regular_file_entry("inside.txt", 0);
        let hardlink =
            std::ffi::CString::new(outside_target.to_string_lossy().to_string()).unwrap();
        entry::archive_entry_set_hardlink(raw_entry, hardlink.as_ptr());
        assert_ne!(ARCHIVE_OK, write::archive_write_header(disk, raw_entry));
        entry::archive_entry_free(raw_entry);

        assert_eq!(ARCHIVE_OK, common::archive_write_free(disk));
    }

    let current_umask = unsafe {
        let current = libc::umask(0o077);
        libc::umask(current);
        current
    };
    assert_eq!(original_umask, current_umask);
}

#[test]
fn checked_arithmetic_helpers_cover_legacy_and_advanced_formats() {
    let usize32 = u32::MAX as u64;

    assert!(archive::read::format::checked_zisofs_layout(15, 32 * 1024, usize32).is_some());
    assert!(archive::read::format::checked_zisofs_layout(31, 32 * 1024, usize32).is_none());
    assert!(
        archive::read::format::checked_zisofs_layout(7, u64::from(u32::MAX) << 7, usize32)
            .is_none()
    );

    assert_eq!(None, archive::read::format::checked_warc_skip(i64::MAX - 3));
    assert_eq!(Some(1028), archive::read::format::checked_warc_skip(1024));

    assert!(archive::read::format::substream_count_ok(2, 4, usize32));
    assert!(!archive::read::format::substream_count_ok(
        2,
        u64::MAX,
        usize32
    ));
    assert!(archive::read::format::skip_target_ok(0, 1024, 512, 2048));
    assert!(!archive::read::format::skip_target_ok(
        u64::MAX - 255,
        1024,
        512,
        u64::MAX
    ));
    assert!(archive::read::format::zip_extra_span_ok(1, 4, 8, 16));
    assert!(!archive::read::format::zip_extra_span_ok(0, 4, 8, 16));
    assert!(!archive::read::format::zip_extra_span_ok(1, 12, 8, 16));

    assert!(archive::write::format::checked_iso9660_name_len(32, 8, 255));
    assert!(!archive::write::format::checked_iso9660_name_len(
        250, 8, 255
    ));
    assert_eq!(
        Some(123),
        archive::write::format::checked_zip_entry_size(123)
    );
    assert_eq!(None, archive::write::format::checked_zip_entry_size(-1));
}

#[test]
fn forward_progress_and_bounds_guards_cover_decoder_edge_cases() {
    assert!(archive::read::format::forward_progress(0, 1, 0, 0));
    assert!(!archive::read::format::forward_progress(4, 4, 8, 8));
    assert!(archive::read::format::within_work_budget(32, 65, 2, 1));
    assert!(!archive::read::format::within_work_budget(32, 66, 2, 1));

    assert!(archive::read::format::continuation_budget_ok(2, 1, 8));
    assert!(!archive::read::format::continuation_budget_ok(8, 1, 8));
    assert!(archive::read::format::line_and_read_ahead_fit(64, 8, 80));
    assert!(!archive::read::format::line_and_read_ahead_fit(80, 8, 80));

    assert!(archive::read::format::window_and_filter_ok(4096, 2048));
    assert!(!archive::read::format::window_and_filter_ok(1024, 2048));
    assert!(archive::read::format::cursor_order_ok(7, 7));
    assert!(!archive::read::format::cursor_order_ok(8, 7));
    assert!(archive::read::format::monotonic_seek_ok(32, 64, 128));
    assert!(!archive::read::format::monotonic_seek_ok(64, 32, 128));

    assert!(archive::read::format::longlink_complete(b"name\0"));
    assert!(!archive::read::format::longlink_complete(b"name"));
    assert!(archive::read::format::cpio_symlink_size_ok(4, 4));
    assert!(!archive::read::format::cpio_symlink_size_ok(5, 4));
}

#[test]
fn i686_zisofs_pointer_table_overflow_is_rejected() {
    unsafe {
        let reader = read::archive_read_new();
        assert!(!reader.is_null());
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_support_format_iso9660(reader)
        );

        let overflow_size = if usize::BITS <= 32 {
            u64::from(u32::MAX) << 7
        } else {
            u64::MAX
        };
        let layout = CString::new(format!("7:{overflow_size}")).unwrap();
        assert_eq!(
            ARCHIVE_FAILED,
            read::archive_read_set_format_option(
                reader,
                c"iso9660".as_ptr(),
                c"zisofs-layout".as_ptr(),
                layout.as_ptr(),
            )
        );

        assert_eq!(ARCHIVE_OK, common::archive_read_free(reader));
    }
}

#[test]
fn i686_zisofs_block_shift_is_validated() {
    unsafe {
        let reader = read::archive_read_new();
        assert!(!reader.is_null());
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_support_format_iso9660(reader)
        );

        let valid = CString::new("7:4096").unwrap();
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_set_option(
                reader,
                c"iso9660".as_ptr(),
                c"zisofs-layout".as_ptr(),
                valid.as_ptr(),
            )
        );

        for invalid in ["6:4096", "31:4096"] {
            let invalid = CString::new(invalid).unwrap();
            assert_eq!(
                ARCHIVE_FAILED,
                read::archive_read_set_option(
                    reader,
                    c"iso9660".as_ptr(),
                    c"zisofs-layout".as_ptr(),
                    invalid.as_ptr(),
                )
            );
        }

        assert_eq!(ARCHIVE_OK, common::archive_read_free(reader));

        let iso = security_support::write_zisofs_iso("zisofs.txt", b"zisofs payload");
        let (pathname, data) = security_support::first_entry_from_memory(&iso);
        assert_eq!("zisofs.txt", pathname);
        assert_eq!(b"zisofs payload", data.as_slice());
    }
}

#[test]
fn i686_zstd_long_window_matches_ubuntu_patch_context() {
    unsafe {
        let writer = write::archive_write_new();
        assert!(!writer.is_null());
        assert_eq!(ARCHIVE_OK, write::archive_write_add_filter_zstd(writer));

        let accepted = CString::new(if usize::BITS <= 32 { "26" } else { "27" }).unwrap();
        assert_eq!(
            ARCHIVE_OK,
            write::archive_write_set_filter_option(
                writer,
                std::ptr::null(),
                c"long".as_ptr(),
                accepted.as_ptr(),
            )
        );

        let rejected = CString::new(if usize::BITS <= 32 { "27" } else { "28" }).unwrap();
        assert_eq!(
            ARCHIVE_FAILED,
            write::archive_write_set_filter_option(
                writer,
                std::ptr::null(),
                c"long".as_ptr(),
                rejected.as_ptr(),
            )
        );
        assert_eq!(
            ARCHIVE_FAILED,
            write::archive_write_set_filter_option(
                writer,
                std::ptr::null(),
                c"long".as_ptr(),
                c"-1".as_ptr(),
            )
        );

        assert_eq!(ARCHIVE_OK, common::archive_write_free(writer));
    }
}
