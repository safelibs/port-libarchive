use std::collections::HashMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, OnceLock};

use crate::support::fixtures::fixture_manifest;

static SUITE_ARTIFACTS: OnceLock<Mutex<HashMap<(String, String), SuiteArtifacts>>> =
    OnceLock::new();
static TEST_PHASE_GROUPS: OnceLock<HashMap<(String, String), String>> = OnceLock::new();
static PORTED_CASE_RUNNER_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

pub fn run_ported_case(suite: &str, define_test: &str) {
    let _runner_lock = PORTED_CASE_RUNNER_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let suite_fixtures = fixture_manifest().suite(suite);
    let case = suite_fixtures.case(define_test);
    let reference_dir = suite_fixtures.root_path();
    case.validate_files_exist(&reference_dir);

    let phase_group = phase_group_for_case(suite, define_test);
    let artifacts = suite_artifacts(suite, &phase_group);
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

pub(crate) fn phase_group_for_case(suite: &str, define_test: &str) -> String {
    TEST_PHASE_GROUPS
        .get_or_init(load_test_phase_groups)
        .get(&(suite.to_owned(), define_test.to_owned()))
        .cloned()
        .unwrap_or_else(|| String::from("all"))
}

fn suite_artifacts(suite: &str, phase_group: &str) -> SuiteArtifacts {
    match suite {
        "libarchive" | "tar" | "cpio" | "cat" | "unzip" => {}
        _ => panic!("unsupported suite {suite}"),
    }

    let cache = SUITE_ARTIFACTS.get_or_init(|| Mutex::new(HashMap::new()));
    let key = (suite.to_owned(), phase_group.to_owned());
    let mut cache = cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(artifacts) = cache.get(&key) {
        return artifacts.clone();
    }

    let artifacts = build_suite_artifacts(suite, phase_group);
    cache.insert(key, artifacts.clone());
    artifacts
}

fn build_suite_artifacts(suite: &str, phase_group: &str) -> SuiteArtifacts {
    let suite_fixtures = fixture_manifest().suite(suite);
    let build_dir = target_dir()
        .join("rust-suite-runners")
        .join(suite)
        .join(phase_group);
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
        .arg(phase_group)
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

    let test_binary = build_dir.join(format!("{suite}-{phase_group}-tests"));
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

#[derive(Clone)]
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

fn load_test_phase_groups() -> HashMap<(String, String), String> {
    #[derive(serde::Deserialize)]
    struct TestManifest {
        rows: Vec<TestManifestRow>,
    }

    #[derive(serde::Deserialize)]
    struct TestManifestRow {
        suite: String,
        define_test: String,
        phase_group: String,
    }

    let manifest_path = package_root().join("generated/test_manifest.json");
    let manifest = std::fs::read_to_string(&manifest_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", manifest_path.display()));
    let manifest: TestManifest = serde_json::from_str(&manifest)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", manifest_path.display()));

    manifest
        .rows
        .into_iter()
        .map(|row| ((row.suite, row.define_test), row.phase_group))
        .collect()
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
