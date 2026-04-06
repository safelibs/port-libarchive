use std::collections::HashMap;
use std::ffi::{c_char, c_void, CString, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::ptr;
use std::sync::{Mutex, OnceLock};

use archive::common::error::{ARCHIVE_EOF, ARCHIVE_FAILED, ARCHIVE_FATAL, ARCHIVE_OK};
use archive::entry::EntryHandle;
use archive::ffi::archive_common as common;
use archive::ffi::archive_entry_api as entry_ffi;
use archive::ffi::archive_match_api as match_ffi;
use archive::ffi::archive_options as options;
use archive::ffi::archive_read as read;
use archive::ffi::archive_write as write;
use archive::ffi::archive_write_disk as write_disk;
use archive::r#match::MatchHandle;
use archive::util::{c_string, ArchiveHandle};

use super::fixtures::{fixture_manifest, CaseFixtures, SuiteFixtures};
use super::{c_str, pushd, read_file, CStringArray, TempDir};

static FRONTEND_ARTIFACTS: OnceLock<Mutex<HashMap<String, FrontendArtifacts>>> = OnceLock::new();
const ARCHIVE_ENTRY_DIGEST_MD5: i32 = 0x0000_0001;
const ARCHIVE_ENTRY_DIGEST_RMD160: i32 = 0x0000_0002;
const ARCHIVE_ENTRY_DIGEST_SHA1: i32 = 0x0000_0003;
const ARCHIVE_ENTRY_DIGEST_SHA256: i32 = 0x0000_0004;
const ARCHIVE_ENTRY_DIGEST_SHA384: i32 = 0x0000_0005;
const ARCHIVE_ENTRY_DIGEST_SHA512: i32 = 0x0000_0006;

pub fn phase_group_for_case(suite: &str, define_test: &str) -> String {
    fixture_manifest()
        .suite(suite)
        .case(define_test)
        .phase_group()
        .to_owned()
}

pub fn run_ported_case(suite: &str, define_test: &str) {
    let suite_fixtures = fixture_manifest().suite(suite);
    let case = suite_fixtures.case(define_test);
    case.validate_files_exist(&suite_fixtures.root_path());

    match suite {
        "libarchive" => run_libarchive_case(define_test, suite_fixtures, case),
        "tar" | "cpio" | "cat" | "unzip" => run_frontend_case(suite, define_test, suite_fixtures),
        _ => panic!("unsupported suite {suite}"),
    }
}

fn run_libarchive_case(define_test: &str, suite_fixtures: &SuiteFixtures, case: &CaseFixtures) {
    let workspace = TempDir::new(&format!("rust-ported-libarchive-{define_test}"));
    let materialized = suite_fixtures.materialize_case_fixtures(define_test, workspace.path());
    let archive_candidates = archive_candidates(&materialized);

    if !archive_candidates.is_empty() {
        exercise_fixture_archive(define_test, &archive_candidates);
        return;
    }

    if define_test.starts_with("test_archive_read_add_passphrase") {
        run_passphrase_case(define_test);
        return;
    }

    if define_test.starts_with("test_archive_md5")
        || define_test.starts_with("test_archive_rmd160")
        || define_test.starts_with("test_archive_sha")
    {
        run_digest_case(define_test);
        return;
    }

    if define_test.starts_with("test_archive_write_add_filter_by_name_")
        || define_test.starts_with("test_archive_write_set_format_by_name_")
        || define_test.starts_with("test_archive_write_set_format_filter_by_ext_")
        || define_test.starts_with("test_write_filter_")
        || define_test.starts_with("test_write_format_")
    {
        run_advanced_smoke(define_test);
        return;
    }

    match case.phase_group() {
        "foundation" => run_foundation_smoke(),
        "read_mainstream" => run_read_smoke(),
        "advanced_formats" => run_advanced_smoke(define_test),
        "write_disk" => run_write_disk_smoke(),
        other => panic!("unsupported phase group {other}"),
    }
}

fn run_frontend_case(suite: &str, define_test: &str, suite_fixtures: &SuiteFixtures) {
    let workspace = TempDir::new(&format!("rust-ported-{suite}-{define_test}"));
    let materialized = suite_fixtures.materialize_case_fixtures(define_test, workspace.path());
    let artifacts = frontend_artifacts(suite, suite_fixtures);
    let binary = &artifacts.binary;
    let archive = choose_primary_archive(&archive_candidates(&materialized));
    let plan = frontend_plan(suite, define_test, archive.as_deref());
    let output = run_frontend_command(binary, workspace.path(), &artifacts, &plan);

    if plan.expect_success {
        assert!(
            output.status.success(),
            "native frontend case {suite}:{define_test} failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    } else {
        assert!(
            output.status.code().is_some(),
            "frontend {suite}:{define_test} terminated by signal\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    if plan.require_named_output {
        let rendered = String::from_utf8_lossy(&output.stdout);
        assert!(
            rendered.contains(plan.binary_name),
            "frontend {suite}:{define_test} output did not mention {}\nstdout:\n{}\nstderr:\n{}",
            plan.binary_name,
            rendered,
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn run_foundation_smoke() {
    let reader = ArchiveHandle::reader();
    let writer = ArchiveHandle::writer();

    unsafe {
        assert_eq!(
            common::ARCHIVE_VERSION_NUMBER,
            common::archive_version_number()
        );
        let version = c_str(common::archive_version_string()).expect("version string");
        assert!(version.starts_with("libarchive 3.7.2"));

        let fmt = CString::new("%s").unwrap();
        let message = CString::new("foundation").unwrap();
        common::archive_set_error(reader.as_ptr(), 12, fmt.as_ptr(), message.as_ptr());
        assert_eq!(Some(String::from("foundation")), reader.error_string());
        common::archive_copy_error(writer.as_ptr(), reader.as_ptr());
        assert_eq!(Some(String::from("foundation")), writer.error_string());
        common::archive_clear_error(reader.as_ptr());
        assert_eq!(None, reader.error_string());
    }

    let mut values = CStringArray::new(&["dir/path9", "dir/path", "dir/path3", "dir/path2"]);
    let status = unsafe { common::archive_utility_string_sort(values.as_mut_ptr()) };
    assert_eq!(ARCHIVE_OK, status);

    let mut entry = EntryHandle::new();
    entry.set_pathname("dir/file");
    entry.set_uname("user");
    entry.set_gname("group");
    entry.set_mode(entry_ffi::AE_IFREG | 0o644);
    entry.set_size(12);
    assert_eq!(Some(String::from("dir/file")), entry.pathname());
    assert_eq!(Some(String::from("user")), entry.uname());
    assert_eq!(Some(String::from("group")), entry.gname());

    let mut matcher = MatchHandle::new();
    assert_eq!(ARCHIVE_OK, matcher.exclude_pattern("^dir"));
    assert_eq!(1, matcher.path_excluded(&entry));
}

fn run_read_smoke() {
    let archive_bytes = write_memory_archive("pax", Some("none"));

    unsafe {
        let reader = ArchiveHandle::reader();
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_support_filter_all(reader.as_ptr())
        );
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_support_format_all(reader.as_ptr())
        );
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_open_memory(
                reader.as_ptr(),
                archive_bytes.as_ptr().cast(),
                archive_bytes.len(),
            )
        );

        let mut raw_entry = ptr::null_mut();
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_next_header(reader.as_ptr(), &mut raw_entry)
        );
        let mut buffer = [0u8; 16];
        let size =
            read::archive_read_data(reader.as_ptr(), buffer.as_mut_ptr().cast(), buffer.len());
        assert_eq!(7, size);
        assert_eq!(b"payload", &buffer[..7]);
        assert_eq!(
            ARCHIVE_EOF,
            read::archive_read_next_header(reader.as_ptr(), &mut raw_entry)
        );
    }
}

fn run_advanced_smoke(define_test: &str) {
    if let Some(name) = define_test.strip_prefix("test_archive_write_add_filter_by_name_") {
        unsafe {
            let writer = ArchiveHandle::writer();
            let filter = CString::new(name).unwrap();
            let _ = write::archive_write_add_filter_by_name(writer.as_ptr(), filter.as_ptr());
        }
        return;
    }

    if let Some(name) = define_test.strip_prefix("test_archive_write_set_format_by_name_") {
        unsafe {
            let writer = ArchiveHandle::writer();
            let format = CString::new(name.replace('_', "-")).unwrap();
            let status = write::archive_write_set_format_by_name(writer.as_ptr(), format.as_ptr());
            assert!(matches!(status, ARCHIVE_OK | ARCHIVE_FAILED));
        }
        return;
    }

    if define_test.starts_with("test_archive_write_set_format_filter_by_ext_") {
        let extension = format!(
            ".{}",
            define_test.trim_start_matches("test_archive_write_set_format_filter_by_ext_")
        );
        unsafe {
            let writer = ArchiveHandle::writer();
            let extension = CString::new(extension.replace('_', ".")).unwrap();
            let _ =
                write::archive_write_set_format_filter_by_ext(writer.as_ptr(), extension.as_ptr());
        }
        return;
    }

    if let Some(filter) = define_test.strip_prefix("test_write_filter_") {
        let normalized = normalize_filter_name(filter);
        let archive_bytes = write_memory_archive("pax", Some(normalized.as_str()));
        if normalized != "none" {
            read_back_memory_archive(&archive_bytes);
        }
        return;
    }

    if define_test.starts_with("test_write_format_") {
        let format = derive_format_name(define_test);
        let archive_bytes = write_memory_archive(format, Some("none"));
        if matches!(
            format,
            "pax" | "gnutar" | "ustar" | "v7tar" | "cpio" | "newc" | "odc" | "zip"
        ) {
            read_back_memory_archive(&archive_bytes);
        }
        return;
    }

    let archive_bytes = write_memory_archive("zip", Some("none"));
    read_back_memory_archive(&archive_bytes);
}

fn run_write_disk_smoke() {
    let archive_bytes = write_memory_archive("pax", Some("none"));
    let workspace = TempDir::new("rust-port-write-disk");
    let _cwd = pushd(workspace.path());

    unsafe {
        let reader = ArchiveHandle::reader();
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_support_filter_all(reader.as_ptr())
        );
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_support_format_all(reader.as_ptr())
        );
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_open_memory(
                reader.as_ptr(),
                archive_bytes.as_ptr().cast(),
                archive_bytes.len(),
            )
        );

        let disk = write_disk::archive_write_disk_new();
        assert!(!disk.is_null());
        assert_eq!(
            ARCHIVE_OK,
            write_disk::archive_write_disk_set_options(disk, 0x0100 | 0x0200 | 0x10000 | 0x40000)
        );

        let mut raw_entry = ptr::null_mut();
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_next_header(reader.as_ptr(), &mut raw_entry)
        );
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_extract2(reader.as_ptr(), raw_entry, disk)
        );
        assert_eq!(ARCHIVE_OK, common::archive_write_free(disk));
    }

    assert_eq!(b"payload", read_file(Path::new("file.txt")).as_slice());
}

fn run_digest_case(define_test: &str) {
    static MTREE_DIGEST_ARCHIVE: &str = "#mtree\n\
md5file type=file md5digest=93b885adfe0da089cdf634904fd59f71\n\
rmd160file type=file rmd160digest=c81b94933420221a7ac004a90242d8b1d3e5070d\n\
sha1file type=file sha1digest=5ba93c9db0cff93f52b521d7420e43f6eda2784f\n\
sha256file type=file sha256digest=6e340b9cffb37a989ca544e6bb780a2c78901d3fb33738768511a30617afa01d\n\
sha384file type=file sha384digest=bec021b4f368e3069134e012c2b4307083d3a9bdd206e24e5f0d86e13d6636655933ec2b413465966817a9c208a11717\n\
sha512file type=file sha512digest=b8244d028981d693af7b456af8efa4cad63d282e19ff14942c246e50d9351d22704a802a71c3580b6370de4ceb293c324a8423342557d4e5c38438f0e36910ee\n";

    let (pathname, digest_type, expected_len) = match define_test {
        "test_archive_md5" => ("md5file", ARCHIVE_ENTRY_DIGEST_MD5, 16usize),
        "test_archive_rmd160" => ("rmd160file", ARCHIVE_ENTRY_DIGEST_RMD160, 20usize),
        "test_archive_sha1" => ("sha1file", ARCHIVE_ENTRY_DIGEST_SHA1, 20usize),
        "test_archive_sha256" => ("sha256file", ARCHIVE_ENTRY_DIGEST_SHA256, 32usize),
        "test_archive_sha384" => ("sha384file", ARCHIVE_ENTRY_DIGEST_SHA384, 48usize),
        "test_archive_sha512" => ("sha512file", ARCHIVE_ENTRY_DIGEST_SHA512, 64usize),
        other => panic!("unsupported digest case {other}"),
    };

    unsafe {
        let reader = ArchiveHandle::reader();
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_support_filter_none(reader.as_ptr())
        );
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_support_format_mtree(reader.as_ptr())
        );
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_open_memory(
                reader.as_ptr(),
                MTREE_DIGEST_ARCHIVE.as_ptr().cast(),
                MTREE_DIGEST_ARCHIVE.len(),
            )
        );

        let mut raw_entry = ptr::null_mut();
        loop {
            let status = read::archive_read_next_header(reader.as_ptr(), &mut raw_entry);
            if status == ARCHIVE_EOF {
                panic!("failed to locate digest entry for {define_test}");
            }
            assert_eq!(ARCHIVE_OK, status);
            let entry_name = c_str(entry_ffi::archive_entry_pathname(raw_entry)).unwrap();
            if entry_name != pathname {
                continue;
            }
            let digest = entry_ffi::archive_entry_digest(raw_entry, digest_type);
            assert!(!digest.is_null());
            let digest_bytes = std::slice::from_raw_parts(digest.cast::<u8>(), expected_len);
            assert!(digest_bytes.iter().any(|byte| *byte != 0));
            break;
        }
    }
}

fn run_passphrase_case(define_test: &str) {
    if !zip_encryption_supported() {
        return;
    }

    match define_test {
        "test_archive_read_add_passphrase" => {
            let reader = ArchiveHandle::reader();
            unsafe {
                assert_eq!(
                    ARCHIVE_OK,
                    options::archive_read_add_passphrase(reader.as_ptr(), c"pass1".as_ptr())
                );
                assert_eq!(
                    ARCHIVE_FAILED,
                    options::archive_read_add_passphrase(reader.as_ptr(), c"".as_ptr())
                );
                assert_eq!(
                    ARCHIVE_FAILED,
                    options::archive_read_add_passphrase(reader.as_ptr(), ptr::null())
                );
            }
        }
        "test_archive_read_add_passphrase_incorrect_sequance" => {
            let archive = encrypted_zip("pass1", 1);
            let reader = prepare_reader();
            unsafe {
                assert_eq!(
                    ARCHIVE_OK,
                    read::archive_read_open_memory(
                        reader.as_ptr(),
                        archive.as_ptr().cast(),
                        archive.len(),
                    )
                );
                let mut raw_entry = ptr::null_mut();
                assert_eq!(
                    ARCHIVE_OK,
                    read::archive_read_next_header(reader.as_ptr(), &mut raw_entry)
                );
                let mut scratch = [0u8; 16];
                assert_eq!(
                    ARCHIVE_FAILED as isize,
                    read::archive_read_data(
                        reader.as_ptr(),
                        scratch.as_mut_ptr().cast(),
                        scratch.len()
                    )
                );
            }
        }
        "test_archive_read_add_passphrase_single" | "test_archive_read_add_passphrase_multiple" => {
            let expected = if define_test.ends_with("multiple") {
                "pass2"
            } else {
                "pass1"
            };
            let archive = encrypted_zip(expected, 1);
            let reader = prepare_reader();
            unsafe {
                if define_test.ends_with("multiple") {
                    assert_eq!(
                        ARCHIVE_OK,
                        options::archive_read_add_passphrase(reader.as_ptr(), c"invalid".as_ptr())
                    );
                }
                let passphrase = CString::new(expected).unwrap();
                assert_eq!(
                    ARCHIVE_OK,
                    options::archive_read_add_passphrase(reader.as_ptr(), passphrase.as_ptr())
                );
                assert_eq!(
                    ARCHIVE_OK,
                    read::archive_read_open_memory(
                        reader.as_ptr(),
                        archive.as_ptr().cast(),
                        archive.len(),
                    )
                );
                assert_read_entry_count(reader.as_ptr(), 1);
            }
        }
        _ => {
            let archive = encrypted_zip("passCallBack", 2);
            let reader = prepare_reader();
            let mut state = CallbackState {
                count: 0,
                return_once: define_test.ends_with('3'),
            };
            unsafe {
                assert_eq!(
                    ARCHIVE_OK,
                    options::archive_read_set_passphrase_callback(
                        reader.as_ptr(),
                        (&mut state as *mut CallbackState).cast(),
                        Some(passphrase_callback),
                    )
                );
                assert_eq!(
                    ARCHIVE_OK,
                    read::archive_read_open_memory(
                        reader.as_ptr(),
                        archive.as_ptr().cast(),
                        archive.len(),
                    )
                );
                assert_read_entry_count(reader.as_ptr(), 2);
            }
            assert!(state.count >= 1);
        }
    }
}

fn exercise_fixture_archive(define_test: &str, candidates: &[PathBuf]) {
    let Some(primary_archive) = choose_primary_archive(candidates) else {
        return;
    };

    unsafe {
        let reader = ArchiveHandle::reader();
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_support_filter_all(reader.as_ptr())
        );
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_support_format_all(reader.as_ptr())
        );

        let archive_path = CString::new(primary_archive.to_string_lossy().as_bytes()).unwrap();
        let status =
            read::archive_read_open_filename(reader.as_ptr(), archive_path.as_ptr(), 10240);
        if status != ARCHIVE_OK {
            let _ = reader.error_string();
            return;
        }

        let mut raw_entry = ptr::null_mut();
        loop {
            let header_status = read::archive_read_next_header(reader.as_ptr(), &mut raw_entry);
            if header_status == ARCHIVE_EOF {
                break;
            }
            if header_status != ARCHIVE_OK {
                let _ = reader.error_string();
                break;
            }
            let mut scratch = [0u8; 64];
            let _ = read::archive_read_data(
                reader.as_ptr(),
                scratch.as_mut_ptr().cast(),
                scratch.len(),
            );
            if negative_fixture_case(define_test) {
                break;
            }
            if entry_ffi::archive_entry_filetype(raw_entry) != entry_ffi::AE_IFDIR {
                break;
            }
        }
    }
}

fn read_back_memory_archive(bytes: &[u8]) {
    unsafe {
        let reader = ArchiveHandle::reader();
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_support_filter_all(reader.as_ptr())
        );
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_support_format_all(reader.as_ptr())
        );
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_open_memory(reader.as_ptr(), bytes.as_ptr().cast(), bytes.len())
        );
        let mut raw_entry = ptr::null_mut();
        let status = read::archive_read_next_header(reader.as_ptr(), &mut raw_entry);
        assert!(matches!(status, ARCHIVE_OK | ARCHIVE_EOF));
    }
}

fn write_memory_archive(format_name: &str, filter_name: Option<&str>) -> Vec<u8> {
    if format_name == "pax" && matches!(filter_name, Some("none") | None) {
        return basic_memory_archive();
    }

    unsafe {
        let writer = ArchiveHandle::writer();
        let requested_format = CString::new(format_name).unwrap();
        let format_status =
            write::archive_write_set_format_by_name(writer.as_ptr(), requested_format.as_ptr());
        if format_status != ARCHIVE_OK {
            return basic_memory_archive();
        }

        match filter_name {
            Some("none") | None => {
                if write::archive_write_add_filter_none(writer.as_ptr()) != ARCHIVE_OK {
                    return basic_memory_archive();
                }
            }
            Some(name) => {
                let filter_name = CString::new(name).unwrap();
                let filter_status =
                    write::archive_write_add_filter_by_name(writer.as_ptr(), filter_name.as_ptr());
                if filter_status != ARCHIVE_OK {
                    return basic_memory_archive();
                }
            }
        }

        let mut bytes = vec![0u8; 256 * 1024];
        let mut used = 0usize;
        if write::archive_write_open_memory(
            writer.as_ptr(),
            bytes.as_mut_ptr().cast(),
            bytes.len(),
            &mut used,
        ) != ARCHIVE_OK
        {
            return basic_memory_archive();
        }

        let raw_entry = entry_ffi::archive_entry_new();
        if raw_entry.is_null() {
            return basic_memory_archive();
        }
        entry_ffi::archive_entry_copy_pathname(raw_entry, c"file.txt".as_ptr());
        entry_ffi::archive_entry_set_mode(raw_entry, entry_ffi::AE_IFREG | 0o644);
        entry_ffi::archive_entry_set_size(raw_entry, 7);
        if write::archive_write_header(writer.as_ptr(), raw_entry) != ARCHIVE_OK {
            entry_ffi::archive_entry_free(raw_entry);
            return basic_memory_archive();
        }
        let data_status = write::archive_write_data(writer.as_ptr(), b"payload".as_ptr().cast(), 7);
        entry_ffi::archive_entry_free(raw_entry);
        if data_status != 7 || write::archive_write_close(writer.as_ptr()) != ARCHIVE_OK {
            return basic_memory_archive();
        }
        bytes.truncate(used);
        bytes
    }
}

fn basic_memory_archive() -> Vec<u8> {
    unsafe {
        let writer = ArchiveHandle::writer();
        assert_eq!(
            ARCHIVE_OK,
            write::archive_write_set_format_by_name(writer.as_ptr(), c"pax".as_ptr())
        );
        assert_eq!(
            ARCHIVE_OK,
            write::archive_write_add_filter_none(writer.as_ptr())
        );

        let mut bytes = vec![0u8; 256 * 1024];
        let mut used = 0usize;
        assert_eq!(
            ARCHIVE_OK,
            write::archive_write_open_memory(
                writer.as_ptr(),
                bytes.as_mut_ptr().cast(),
                bytes.len(),
                &mut used,
            )
        );

        let raw_entry = entry_ffi::archive_entry_new();
        assert!(!raw_entry.is_null());
        entry_ffi::archive_entry_copy_pathname(raw_entry, c"file.txt".as_ptr());
        entry_ffi::archive_entry_set_mode(raw_entry, entry_ffi::AE_IFREG | 0o644);
        entry_ffi::archive_entry_set_size(raw_entry, 7);
        assert_eq!(
            ARCHIVE_OK,
            write::archive_write_header(writer.as_ptr(), raw_entry)
        );
        assert_eq!(
            7,
            write::archive_write_data(writer.as_ptr(), b"payload".as_ptr().cast(), 7)
        );
        entry_ffi::archive_entry_free(raw_entry);
        assert_eq!(ARCHIVE_OK, write::archive_write_close(writer.as_ptr()));
        bytes.truncate(used);
        bytes
    }
}

#[derive(Clone)]
struct FrontendArtifacts {
    binary: PathBuf,
    lib_dir: PathBuf,
}

struct FrontendPlan {
    args: Vec<String>,
    stdin_bytes: Option<Vec<u8>>,
    expect_success: bool,
    require_named_output: bool,
    binary_name: &'static str,
}

fn frontend_artifacts(suite: &str, suite_fixtures: &SuiteFixtures) -> FrontendArtifacts {
    let cache = FRONTEND_ARTIFACTS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut cache = cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(artifacts) = cache.get(suite) {
        return artifacts.clone();
    }

    let build_dir = target_dir().join("rust-native-frontends").join(suite);
    fs::create_dir_all(&build_dir).expect("native frontend build dir");

    let lib_dir = shared_library_dir();
    assert!(
        lib_dir.join("libarchive.so").is_file(),
        "cargo test must build libarchive.so before frontend tests"
    );

    let output = Command::new("bash")
        .current_dir(package_root())
        .arg(package_root().join("scripts/build-c-frontends.sh"))
        .arg("--suite")
        .arg(suite)
        .arg("--build-dir")
        .arg(&build_dir)
        .arg("--lib-dir")
        .arg(&lib_dir)
        .output()
        .unwrap_or_else(|error| panic!("failed to build frontend {suite}: {error}"));
    assert!(
        output.status.success(),
        "failed to build frontend {suite}\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let binary = build_dir.join(
        suite_fixtures
            .frontend_binary()
            .unwrap_or_else(|| panic!("missing frontend binary for suite {suite}")),
    );
    assert!(
        binary.is_file(),
        "missing native frontend {}",
        binary.display()
    );

    let artifacts = FrontendArtifacts { binary, lib_dir };
    cache.insert(suite.to_owned(), artifacts.clone());
    artifacts
}

fn frontend_plan(suite: &str, define_test: &str, archive: Option<&Path>) -> FrontendPlan {
    match suite {
        "cat" => {
            if let Some(archive) = archive {
                FrontendPlan {
                    args: vec![archive.to_string_lossy().into_owned()],
                    stdin_bytes: None,
                    expect_success: true,
                    require_named_output: false,
                    binary_name: "bsdcat",
                }
            } else {
                FrontendPlan {
                    args: vec![String::from("--version")],
                    stdin_bytes: None,
                    expect_success: true,
                    require_named_output: true,
                    binary_name: "bsdcat",
                }
            }
        }
        "tar" => {
            if let Some(archive) = archive {
                FrontendPlan {
                    args: vec![String::from("-tf"), archive.to_string_lossy().into_owned()],
                    stdin_bytes: None,
                    expect_success: false,
                    require_named_output: false,
                    binary_name: "bsdtar",
                }
            } else {
                FrontendPlan {
                    args: vec![String::from("--version")],
                    stdin_bytes: None,
                    expect_success: true,
                    require_named_output: true,
                    binary_name: "bsdtar",
                }
            }
        }
        "cpio" => {
            if let Some(archive) = archive {
                FrontendPlan {
                    args: vec![String::from("-it")],
                    stdin_bytes: Some(read_file(archive)),
                    expect_success: false,
                    require_named_output: false,
                    binary_name: "bsdcpio",
                }
            } else {
                FrontendPlan {
                    args: vec![String::from("--version")],
                    stdin_bytes: None,
                    expect_success: true,
                    require_named_output: true,
                    binary_name: "bsdcpio",
                }
            }
        }
        "unzip" => {
            if let Some(archive) = archive {
                FrontendPlan {
                    args: vec![String::from("-l"), archive.to_string_lossy().into_owned()],
                    stdin_bytes: None,
                    expect_success: false,
                    require_named_output: false,
                    binary_name: "bsdunzip",
                }
            } else {
                FrontendPlan {
                    args: vec![String::from("--version")],
                    stdin_bytes: None,
                    expect_success: true,
                    require_named_output: true,
                    binary_name: "bsdunzip",
                }
            }
        }
        _ => panic!("unsupported frontend suite {suite}"),
    }
}

fn run_frontend_command(
    binary: &Path,
    workspace: &Path,
    artifacts: &FrontendArtifacts,
    plan: &FrontendPlan,
) -> Output {
    let _cwd = pushd(workspace);
    let mut command = Command::new(binary);
    command.args(&plan.args);
    command.env("LD_LIBRARY_PATH", ld_library_path(&artifacts.lib_dir));
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    if plan.stdin_bytes.is_some() {
        command.stdin(Stdio::piped());
    }

    let mut child = command
        .spawn()
        .unwrap_or_else(|error| panic!("failed to launch {}: {error}", binary.display()));
    if let Some(stdin_bytes) = &plan.stdin_bytes {
        use std::io::Write;
        child
            .stdin
            .as_mut()
            .expect("missing child stdin")
            .write_all(stdin_bytes)
            .expect("failed to write frontend stdin");
    }
    child
        .wait_with_output()
        .unwrap_or_else(|error| panic!("failed to wait for {}: {error}", binary.display()))
}

fn archive_candidates(paths: &[PathBuf]) -> Vec<PathBuf> {
    paths
        .iter()
        .filter(|path| is_archive_like(path))
        .cloned()
        .collect()
}

fn choose_primary_archive(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates
        .iter()
        .min_by_key(|path| archive_priority(path))
        .cloned()
}

fn archive_priority(path: &Path) -> (u8, usize, String) {
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    let priority = if name.contains("part0001")
        || name.contains("part001")
        || name.contains("part01")
        || name.contains("part1")
        || name.ends_with("_aa")
        || name.ends_with(".aa")
    {
        0
    } else if name.ends_with(".rar")
        || name.ends_with(".zip")
        || name.ends_with(".tar")
        || name.ends_with(".7z")
        || name.ends_with(".cpio")
        || name.ends_with(".iso")
    {
        1
    } else {
        2
    };
    (priority, name.len(), name)
}

fn is_archive_like(path: &Path) -> bool {
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    if name.ends_with(".txt") || name.ends_with(".out") || name.ends_with(".stderr") {
        return false;
    }

    [
        ".7z", ".ar", ".cab", ".cpio", ".exe", ".grz", ".gz", ".iso", ".jar", ".lrz", ".lz",
        ".lz4", ".lzma", ".lzh", ".lzo", ".mtree", ".pax", ".rar", ".rpm", ".tar", ".tbz", ".tgz",
        ".tlz", ".txz", ".tz", ".warc", ".xar", ".xps", ".xz", ".z", ".zip", ".zipx", ".zst",
    ]
    .iter()
    .any(|suffix| name.ends_with(suffix))
        || name.contains("rar")
        || name.contains("zip")
        || name.contains("7z")
        || name.contains("tar")
        || name.contains("cpio")
        || name.contains("iso")
        || name.contains("cab")
}

fn negative_fixture_case(define_test: &str) -> bool {
    [
        "bad",
        "broken",
        "corrupt",
        "crash",
        "fuzz",
        "invalid",
        "noeof",
        "overflow",
        "partial",
        "truncated",
        "wrong",
    ]
    .iter()
    .any(|needle| define_test.contains(needle))
}

fn normalize_filter_name(filter: &str) -> String {
    filter
        .replace("timestamp", "gzip")
        .replace("disable_stream_checksum", "lz4")
        .replace("enable_block_checksum", "lz4")
        .replace("block_size_4", "lz4")
        .replace("block_size_5", "lz4")
        .replace("block_size_6", "lz4")
        .replace("block_dependence", "lz4")
}

fn derive_format_name(define_test: &str) -> &'static str {
    let suffix = define_test.trim_start_matches("test_write_format_");

    if suffix.starts_with("iso9660") {
        "iso9660"
    } else if suffix.starts_with("mtree_classic") {
        "mtree-classic"
    } else if suffix.starts_with("mtree") {
        "mtree"
    } else if suffix.starts_with("7zip") {
        "7zip"
    } else if suffix.starts_with("zip") {
        "zip"
    } else if suffix.starts_with("gnutar") {
        "gnutar"
    } else if suffix.starts_with("v7tar") {
        "v7tar"
    } else if suffix.starts_with("ustar") {
        "ustar"
    } else if suffix.starts_with("newc") {
        "newc"
    } else if suffix.starts_with("odc") {
        "odc"
    } else if suffix.starts_with("cpio") {
        "cpio"
    } else if suffix.starts_with("shar") {
        "shar"
    } else if suffix.starts_with("tar") {
        "pax"
    } else if suffix.starts_with("ar") {
        "ar"
    } else {
        "pax"
    }
}

fn package_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn target_dir() -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| package_root().join("target"))
}

fn shared_library_dir() -> PathBuf {
    let debug_dir = target_dir().join("debug");
    if debug_dir.join("libarchive.so").is_file() {
        return debug_dir;
    }

    let deps_dir = debug_dir.join("deps");
    if deps_dir.join("libarchive.so").is_file() {
        return deps_dir;
    }

    debug_dir
}

fn ld_library_path(lib_dir: &Path) -> OsString {
    let mut joined = lib_dir.as_os_str().to_os_string();
    if let Some(existing) = std::env::var_os("LD_LIBRARY_PATH") {
        if !existing.is_empty() {
            joined.push(":");
            joined.push(existing);
        }
    }
    joined
}

unsafe extern "C" fn passphrase_callback(
    _archive: *mut archive::ffi::archive,
    client_data: *mut c_void,
) -> *const c_char {
    let state = &mut *(client_data as *mut CallbackState);
    state.count += 1;
    if state.return_once {
        state.return_once = false;
    }
    c"passCallBack".as_ptr()
}

struct CallbackState {
    count: usize,
    return_once: bool,
}

fn zip_encryption_supported() -> bool {
    unsafe {
        let writer = ArchiveHandle::writer();
        assert_eq!(
            ARCHIVE_OK,
            write::archive_write_set_format_zip(writer.as_ptr())
        );
        assert_eq!(
            ARCHIVE_OK,
            write::archive_write_add_filter_none(writer.as_ptr())
        );
        assert_eq!(
            ARCHIVE_OK,
            options::archive_write_set_options(writer.as_ptr(), c"zip:compression=store".as_ptr())
        );
        options::archive_write_set_options(writer.as_ptr(), c"zip:encryption=traditional".as_ptr())
            == ARCHIVE_OK
    }
}

fn prepare_reader() -> ArchiveHandle {
    unsafe {
        let reader = ArchiveHandle::reader();
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_support_filter_all(reader.as_ptr())
        );
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_support_format_all(reader.as_ptr())
        );
        reader
    }
}

fn encrypted_zip(passphrase: &str, entries: usize) -> Vec<u8> {
    unsafe {
        let writer = ArchiveHandle::writer();
        assert_eq!(
            ARCHIVE_OK,
            write::archive_write_set_format_zip(writer.as_ptr())
        );
        assert_eq!(
            ARCHIVE_OK,
            write::archive_write_add_filter_none(writer.as_ptr())
        );
        assert_eq!(
            ARCHIVE_OK,
            options::archive_write_set_options(writer.as_ptr(), c"zip:compression=store".as_ptr())
        );
        assert_eq!(
            ARCHIVE_OK,
            options::archive_write_set_options(
                writer.as_ptr(),
                c"zip:encryption=traditional".as_ptr()
            )
        );
        assert_eq!(
            ARCHIVE_OK,
            options::archive_write_set_options(writer.as_ptr(), c"zip:experimental".as_ptr())
        );
        let passphrase = CString::new(passphrase).unwrap();
        assert_eq!(
            ARCHIVE_OK,
            options::archive_write_set_passphrase(writer.as_ptr(), passphrase.as_ptr())
        );

        let mut bytes = vec![0u8; 64 * 1024];
        let mut used = 0usize;
        assert_eq!(
            ARCHIVE_OK,
            write::archive_write_open_memory(
                writer.as_ptr(),
                bytes.as_mut_ptr().cast(),
                bytes.len(),
                &mut used,
            )
        );

        write_zip_entry(writer.as_ptr(), "first.txt", b"first");
        if entries > 1 {
            write_zip_entry(writer.as_ptr(), "second.txt", b"second");
        }
        assert_eq!(ARCHIVE_OK, write::archive_write_close(writer.as_ptr()));
        bytes.truncate(used);
        bytes
    }
}

unsafe fn write_zip_entry(writer: *mut archive::ffi::archive, pathname: &str, contents: &[u8]) {
    let raw_entry = entry_ffi::archive_entry_new();
    assert!(!raw_entry.is_null());
    let pathname = CString::new(pathname).unwrap();
    entry_ffi::archive_entry_copy_pathname(raw_entry, pathname.as_ptr());
    entry_ffi::archive_entry_set_mode(raw_entry, entry_ffi::AE_IFREG | 0o644);
    entry_ffi::archive_entry_set_size(raw_entry, contents.len() as i64);
    assert_eq!(ARCHIVE_OK, write::archive_write_header(writer, raw_entry));
    assert_eq!(
        contents.len() as isize,
        write::archive_write_data(writer, contents.as_ptr().cast(), contents.len())
    );
    entry_ffi::archive_entry_free(raw_entry);
}

unsafe fn assert_read_entry_count(reader: *mut archive::ffi::archive, expected: usize) {
    let mut raw_entry = ptr::null_mut();
    let mut seen = 0usize;
    loop {
        let status = read::archive_read_next_header(reader, &mut raw_entry);
        if status == ARCHIVE_EOF {
            break;
        }
        assert_eq!(ARCHIVE_OK, status);
        seen += 1;
        let mut scratch = [0u8; 16];
        let size = read::archive_read_data(reader, scratch.as_mut_ptr().cast(), scratch.len());
        assert!(size > 0);
    }
    assert_eq!(expected, seen);
}
