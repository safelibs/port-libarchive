#![allow(warnings, clippy::all)]

#[path = "libarchive/advanced/mod.rs"]
mod advanced_support;
#[path = "libarchive/security/mod.rs"]
mod security_support;
#[path = "support/mod.rs"]
mod support;

use std::collections::BTreeSet;
use std::ffi::CString;
use std::path::Path;
use std::ptr;

use archive::common::error::{ARCHIVE_EOF, ARCHIVE_FAILED, ARCHIVE_OK};
use archive::ffi::archive_common as common;
use archive::ffi::archive_entry_api as entry;
use archive::ffi::archive_read as read;
use archive::ffi::archive_read_disk as read_disk;
use archive::ffi::archive_write as write;
use archive::ffi::archive_write_disk as write_disk;

fn tar_header(name: &str, size: usize, typeflag: u8) -> [u8; 512] {
    fn write_octal(field: &mut [u8], value: u64) {
        let width = field.len().saturating_sub(1);
        let text = format!("{value:0width$o}", width = width);
        field[..width].copy_from_slice(text.as_bytes());
        field[width] = 0;
    }

    let mut header = [0u8; 512];
    header[..name.len()].copy_from_slice(name.as_bytes());
    write_octal(&mut header[100..108], 0o644);
    write_octal(&mut header[108..116], 0);
    write_octal(&mut header[116..124], 0);
    write_octal(&mut header[124..136], size as u64);
    write_octal(&mut header[136..148], 0);
    header[148..156].fill(b' ');
    header[156] = typeflag;
    header[257..263].copy_from_slice(b"ustar\0");
    header[263..265].copy_from_slice(b"00");

    let checksum = header.iter().map(|byte| u32::from(*byte)).sum::<u32>();
    let checksum_text = format!("{checksum:06o}\0 ");
    header[148..156].copy_from_slice(checksum_text.as_bytes());
    header
}

fn cpio_newc_entry(path: &str, mode: u32, declared_size: u32, data: &[u8]) -> Vec<u8> {
    let header = format!(
        concat!(
            "070701",
            "{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}",
            "{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}"
        ),
        1u32,
        mode,
        0u32,
        0u32,
        1u32,
        0u32,
        declared_size,
        0u32,
        0u32,
        0u32,
        0u32,
        path.len() as u32 + 1,
        0u32,
    );
    let mut bytes = header.into_bytes();
    bytes.extend_from_slice(path.as_bytes());
    bytes.push(0);
    while bytes.len() % 4 != 0 {
        bytes.push(0);
    }
    bytes.extend_from_slice(data);
    while bytes.len() % 4 != 0 {
        bytes.push(0);
    }
    bytes
}

#[test]
fn matrix_covers_relevant_cves_and_verification_targets_are_present() {
    let relevant = security_support::load_json("relevant_cves.json");
    let matrix = security_support::load_json("safe/generated/cve_matrix.json");
    let manifest = security_support::load_json("safe/generated/test_manifest.json");
    let safe_root = Path::new(env!("CARGO_MANIFEST_DIR"));

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
        let verification = row["verification"].as_str().expect("verification");
        assert!(!verification.is_empty());

        let tokens = verification.split_whitespace().collect::<Vec<_>>();
        assert!(!tokens.is_empty());
        match tokens[0] {
            "./scripts/check-i686-cve.sh" => {
                let script = safe_root.join(tokens[0].trim_start_matches("./"));
                assert!(script.exists(), "missing verification script {script:?}");
            }
            "./scripts/run-upstream-c-tests.sh" => {
                let script = safe_root.join(tokens[0].trim_start_matches("./"));
                assert!(script.exists(), "missing verification script {script:?}");
                assert_eq!(4, tokens.len(), "expected exact upstream test invocation");
                let suite = tokens[1];
                let phase_group = tokens[2];
                let define_test = tokens[3];
                assert!(manifest["rows"]
                    .as_array()
                    .expect("manifest rows")
                    .iter()
                    .any(|row| {
                        row["suite"].as_str() == Some(suite)
                            && row["phase_group"].as_str() == Some(phase_group)
                            && row["define_test"].as_str() == Some(define_test)
                    }));
            }
            "cargo" => {
                assert_eq!(Some(&"test"), tokens.get(1));
                assert_eq!(Some(&"--test"), tokens.get(2));
                assert_eq!(Some(&"--"), tokens.get(4));
                assert_eq!(Some(&"--exact"), tokens.get(5));
                let test_bin = tokens.get(3).expect("cargo test binary");
                let test_name = tokens.get(6).expect("cargo test name");
                let test_file = safe_root.join("tests").join(format!("{test_bin}.rs"));
                assert!(
                    test_file.exists(),
                    "missing verification test file {test_file:?}"
                );
                let contents = std::fs::read_to_string(&test_file).expect("read verification test");
                assert!(
                    contents.contains(test_name),
                    "missing verification test name {test_name} in {test_file:?}"
                );
            }
            other => panic!("unsupported verification target {other}"),
        }
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
        let raw_entry = security_support::regular_file_entry("inside.txt", 4);
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
fn gnu_longlink_truncation_is_rejected() {
    let mut archive = Vec::new();
    archive.extend_from_slice(&tar_header("././@LongLink", 20, b'L'));
    archive.extend_from_slice(b"truncated-name");

    unsafe {
        let reader = read::archive_read_new();
        assert!(!reader.is_null());
        assert_eq!(ARCHIVE_OK, read::archive_read_support_filter_all(reader));
        assert_eq!(ARCHIVE_OK, read::archive_read_support_format_tar(reader));
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_open_memory(reader, archive.as_ptr().cast(), archive.len())
        );

        let mut raw_entry = ptr::null_mut();
        assert_ne!(
            ARCHIVE_OK,
            read::archive_read_next_header(reader, &mut raw_entry)
        );
        assert_eq!(ARCHIVE_OK, common::archive_read_free(reader));
    }
}

#[test]
fn acl_metadata_write_rejects_symlink_escape_before_apply() {
    let temp = support::TempDir::new("cve-acl-root");
    let outside = support::TempDir::new("cve-acl-outside");
    let _cwd = support::pushd(temp.path());
    support::symlink(Path::new("pivot"), outside.path());

    unsafe {
        let disk = write_disk::archive_write_disk_new();
        assert!(!disk.is_null());
        assert_eq!(
            0,
            write_disk::archive_write_disk_set_options(
                disk,
                security_support::SECURE_WRITE_FLAGS | 0x0020,
            )
        );

        let raw_entry = security_support::regular_file_entry("pivot/acl-escape.txt", 0);
        assert_eq!(
            ARCHIVE_OK,
            entry::archive_entry_acl_add_entry(
                raw_entry,
                entry::ARCHIVE_ENTRY_ACL_TYPE_ACCESS,
                entry::ARCHIVE_ENTRY_ACL_READ | entry::ARCHIVE_ENTRY_ACL_WRITE,
                entry::ARCHIVE_ENTRY_ACL_USER_OBJ,
                -1,
                ptr::null(),
            )
        );

        assert_ne!(ARCHIVE_OK, write::archive_write_header(disk, raw_entry));
        entry::archive_entry_free(raw_entry);
        assert_eq!(ARCHIVE_OK, common::archive_write_free(disk));
    }

    assert!(!outside.path().join("acl-escape.txt").exists());
}

#[test]
fn cpio_symlink_size_mismatch_is_rejected() {
    let archive = cpio_newc_entry("link", libc::S_IFLNK | 0o777, 16, b"abc");

    unsafe {
        let reader = read::archive_read_new();
        assert!(!reader.is_null());
        assert_eq!(ARCHIVE_OK, read::archive_read_support_filter_all(reader));
        assert_eq!(ARCHIVE_OK, read::archive_read_support_format_cpio(reader));
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_open_memory(reader, archive.as_ptr().cast(), archive.len())
        );

        let mut raw_entry = ptr::null_mut();
        let header_status = read::archive_read_next_header(reader, &mut raw_entry);
        if header_status == ARCHIVE_OK {
            let mut buf = [0u8; 16];
            assert!(read::archive_read_data(reader, buf.as_mut_ptr().cast(), buf.len()) < 0);
        } else {
            assert_ne!(ARCHIVE_OK, header_status);
        }
        assert_eq!(ARCHIVE_OK, common::archive_read_free(reader));
    }
}

#[test]
fn callback_reader_extract2_streams_multiple_blocks_through_rust_guard() {
    let payload = vec![b'x'; 32 * 1024];
    let archive = unsafe {
        advanced_support::write_single_entry_archive("streamed.txt", &payload, |writer| {
            assert_eq!(ARCHIVE_OK, write::archive_write_set_format_pax(writer));
        })
    };
    let temp = support::TempDir::new("cve-callback-extract");
    let _cwd = support::pushd(temp.path());

    unsafe {
        let reader = read::archive_read_new();
        assert!(!reader.is_null());
        let mut state = security_support::CallbackReader::new(&archive, 257);
        assert_eq!(
            ARCHIVE_OK,
            security_support::open_reader_with_callbacks(reader, &mut state)
        );

        let mut raw_entry = ptr::null_mut();
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_next_header(reader, &mut raw_entry)
        );

        let disk = security_support::secure_disk_writer();
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_extract2(reader, raw_entry, disk)
        );
        assert_eq!(ARCHIVE_OK, common::archive_write_free(disk));
        assert_eq!(ARCHIVE_OK, common::archive_read_free(reader));

        assert_eq!(1, state.opens);
        assert_eq!(1, state.closes);
        assert!(state.read_calls > 1);
    }

    assert_eq!(payload, support::read_file(Path::new("streamed.txt")));
}

#[test]
fn disk_reader_skip_covers_sparse_monotonic_block_offsets() {
    unsafe fn seek_to_sparse_file(
        reader: *mut archive::ffi::archive,
        entry_ptr: *mut archive::ffi::archive_entry,
    ) {
        loop {
            let status = read::archive_read_next_header2(reader, entry_ptr);
            assert!(matches!(status, ARCHIVE_OK | ARCHIVE_EOF));
            assert_eq!(ARCHIVE_OK, status, "expected sparse.bin entry");
            let pathname = std::ffi::CStr::from_ptr(entry::archive_entry_pathname(entry_ptr))
                .to_string_lossy()
                .into_owned();
            if pathname.ends_with("sparse.bin") {
                return;
            }
            if read_disk::archive_read_disk_can_descend(reader) == 1 {
                assert_eq!(ARCHIVE_OK, read_disk::archive_read_disk_descend(reader));
            }
        }
    }

    let temp = support::TempDir::new("cve-disk-skip");
    let _cwd = support::pushd(temp.path());
    support::make_dir(Path::new("root"));
    security_support::write_sparse_file(
        Path::new("root/sparse.bin"),
        1 << 20,
        &[(0, b"A"), (1 << 19, b"B")],
    );

    unsafe {
        let reader = read_disk::archive_read_disk_new();
        assert!(!reader.is_null());
        assert_eq!(
            ARCHIVE_OK,
            read_disk::archive_read_disk_open(reader, c"root".as_ptr())
        );
        let entry_ptr = entry::archive_entry_new();
        assert!(!entry_ptr.is_null());
        seek_to_sparse_file(reader, entry_ptr);

        let mut blocks = Vec::new();
        loop {
            let mut block = ptr::null();
            let mut size = 0usize;
            let mut offset = 0i64;
            let status = read::archive_read_data_block(reader, &mut block, &mut size, &mut offset);
            if status == ARCHIVE_EOF {
                break;
            }
            assert_eq!(ARCHIVE_OK, status);
            blocks.push((offset, size));
        }
        assert!(blocks.len() >= 2, "expected multiple sparse data blocks");
        assert!(blocks.windows(2).all(|pair| pair[1].0 > pair[0].0));
        entry::archive_entry_free(entry_ptr);
        assert_eq!(ARCHIVE_OK, common::archive_read_free(reader));

        let reader = read_disk::archive_read_disk_new();
        assert!(!reader.is_null());
        assert_eq!(
            ARCHIVE_OK,
            read_disk::archive_read_disk_open(reader, c"root".as_ptr())
        );
        let entry_ptr = entry::archive_entry_new();
        assert!(!entry_ptr.is_null());
        seek_to_sparse_file(reader, entry_ptr);
        assert_eq!(ARCHIVE_OK, read::archive_read_data_skip(reader));
        entry::archive_entry_free(entry_ptr);
        assert_eq!(ARCHIVE_OK, common::archive_read_free(reader));
    }
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
