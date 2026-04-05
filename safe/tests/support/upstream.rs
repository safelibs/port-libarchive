use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

use crate::support::fixtures::fixture_manifest;

static LIBARCHIVE_ARTIFACTS: OnceLock<SuiteArtifacts> = OnceLock::new();
static TAR_ARTIFACTS: OnceLock<SuiteArtifacts> = OnceLock::new();
static CPIO_ARTIFACTS: OnceLock<SuiteArtifacts> = OnceLock::new();
static CAT_ARTIFACTS: OnceLock<SuiteArtifacts> = OnceLock::new();
static UNZIP_ARTIFACTS: OnceLock<SuiteArtifacts> = OnceLock::new();

pub fn run_ported_case(suite: &str, define_test: &str) {
    let suite_fixtures = fixture_manifest().suite(suite);
    let case = suite_fixtures.case(define_test);
    let reference_dir = suite_fixtures.root_path();
    case.validate_files_exist(&reference_dir);

    let artifacts = suite_artifacts(suite);
    let mut command = Command::new(&artifacts.test_binary);
    command.arg("-q");
    if let Some(frontend_binary) = &artifacts.frontend_binary {
        command.arg("-p").arg(frontend_binary);
    }
    command.arg("-r").arg(&reference_dir).arg(define_test);
    command.env("LD_LIBRARY_PATH", artifacts.ld_library_path());

    let output = command
        .output()
        .unwrap_or_else(|error| panic!("failed to execute {suite}:{define_test}: {error}"));
    assert!(
        output.status.success(),
        "ported upstream test {suite}:{define_test} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn suite_artifacts(suite: &str) -> &'static SuiteArtifacts {
    match suite {
        "libarchive" => LIBARCHIVE_ARTIFACTS.get_or_init(|| build_suite_artifacts("libarchive")),
        "tar" => TAR_ARTIFACTS.get_or_init(|| build_suite_artifacts("tar")),
        "cpio" => CPIO_ARTIFACTS.get_or_init(|| build_suite_artifacts("cpio")),
        "cat" => CAT_ARTIFACTS.get_or_init(|| build_suite_artifacts("cat")),
        "unzip" => UNZIP_ARTIFACTS.get_or_init(|| build_suite_artifacts("unzip")),
        _ => panic!("unsupported suite {suite}"),
    }
}

fn build_suite_artifacts(suite: &str) -> SuiteArtifacts {
    let suite_fixtures = fixture_manifest().suite(suite);
    let build_dir = target_dir().join("rust-suite-runners").join(suite);
    std::fs::create_dir_all(&build_dir).expect("suite runner build dir");

    let lib_dir = shared_library_dir();
    assert!(
        lib_dir.join("libarchive.so").is_file(),
        "cargo test must build the debug shared library before suite runners: expected {}",
        lib_dir.join("libarchive.so").display()
    );

    let output = Command::new("bash")
        .current_dir(package_root())
        .arg(package_root().join("scripts/run-upstream-c-tests.sh"))
        .arg("--suite")
        .arg(suite)
        .arg("--phase-group")
        .arg("all")
        .arg("--build-dir")
        .arg(&build_dir)
        .arg("--lib-dir")
        .arg(&lib_dir)
        .arg("--build-only")
        .output()
        .unwrap_or_else(|error| panic!("failed to build suite runner for {suite}: {error}"));
    assert!(
        output.status.success(),
        "failed to build suite runner for {suite}\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let frontend_binary = suite_fixtures
        .frontend_binary()
        .map(|binary| build_dir.join("frontends").join(binary));
    if let Some(frontend_binary) = &frontend_binary {
        assert!(
            frontend_binary.is_file(),
            "missing frontend binary {}",
            frontend_binary.display()
        );
    }

    let test_binary = build_dir.join(format!("{suite}-all-tests"));
    assert!(
        test_binary.is_file(),
        "missing suite test binary {}",
        test_binary.display()
    );

    SuiteArtifacts {
        test_binary,
        frontend_binary,
        lib_dir,
    }
}

struct SuiteArtifacts {
    test_binary: PathBuf,
    frontend_binary: Option<PathBuf>,
    lib_dir: PathBuf,
}

impl SuiteArtifacts {
    fn ld_library_path(&self) -> OsString {
        let mut joined = self.lib_dir.as_os_str().to_os_string();
        if let Some(existing) = std::env::var_os("LD_LIBRARY_PATH") {
            if !existing.is_empty() {
                joined.push(":");
                joined.push(existing);
            }
        }
        joined
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
