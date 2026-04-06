use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Deserialize;

static FIXTURE_MANIFEST: OnceLock<FixtureManifest> = OnceLock::new();

#[derive(Deserialize)]
struct FixtureManifestFile {
    schema_version: u32,
    #[serde(rename = "suite")]
    suites: Vec<SuiteRecord>,
}

#[derive(Deserialize)]
struct SuiteRecord {
    name: String,
    root: String,
    frontend_binary: Option<String>,
    #[serde(rename = "case")]
    cases: Vec<CaseRecord>,
}

#[derive(Deserialize)]
struct CaseRecord {
    define_test: String,
    source_file: String,
    phase_group: String,
    #[serde(default)]
    fixture_refs: Vec<String>,
}

pub struct FixtureManifest {
    suites: BTreeMap<String, SuiteFixtures>,
}

pub struct SuiteFixtures {
    root_rel: PathBuf,
    frontend_binary: Option<String>,
    cases: BTreeMap<String, CaseFixtures>,
}

pub struct CaseFixtures {
    source_file: PathBuf,
    phase_group: String,
    fixture_refs: Vec<PathBuf>,
}

enum FixtureSource {
    Direct(PathBuf),
    UuEncoded(PathBuf),
}

pub fn fixture_manifest() -> &'static FixtureManifest {
    FIXTURE_MANIFEST.get_or_init(|| {
        let parsed: FixtureManifestFile = toml::from_str(include_str!("../fixtures-manifest.toml"))
            .expect("fixture manifest must parse");
        assert_eq!(1, parsed.schema_version, "fixture manifest schema version");

        let suites = parsed
            .suites
            .into_iter()
            .map(|suite| {
                let cases = suite
                    .cases
                    .into_iter()
                    .map(|case| {
                        (
                            case.define_test,
                            CaseFixtures {
                                source_file: PathBuf::from(case.source_file),
                                phase_group: case.phase_group,
                                fixture_refs: case
                                    .fixture_refs
                                    .into_iter()
                                    .map(PathBuf::from)
                                    .collect(),
                            },
                        )
                    })
                    .collect();
                (
                    suite.name,
                    SuiteFixtures {
                        root_rel: PathBuf::from(suite.root),
                        frontend_binary: suite.frontend_binary,
                        cases,
                    },
                )
            })
            .collect();
        FixtureManifest { suites }
    })
}

impl FixtureManifest {
    pub fn suite(&self, suite: &str) -> &SuiteFixtures {
        self.suites
            .get(suite)
            .unwrap_or_else(|| panic!("missing suite fixture manifest entry for {suite}"))
    }
}

impl SuiteFixtures {
    pub fn root_path(&self) -> PathBuf {
        repo_root().join(&self.root_rel)
    }

    pub fn frontend_binary(&self) -> Option<&str> {
        self.frontend_binary.as_deref()
    }

    pub fn case(&self, define_test: &str) -> &CaseFixtures {
        self.cases
            .get(define_test)
            .unwrap_or_else(|| panic!("missing fixture manifest entry for test {define_test}"))
    }

    pub fn materialize_case_fixtures(&self, define_test: &str, dest_root: &Path) -> Vec<PathBuf> {
        self.case(define_test)
            .materialize_available_fixture_refs(&self.root_path(), dest_root)
    }
}

impl CaseFixtures {
    pub fn source_path(&self) -> PathBuf {
        repo_root().join(&self.source_file)
    }

    pub fn validate_files_exist(&self, suite_root: &Path) {
        assert!(
            self.source_path().is_file(),
            "missing preserved upstream source {} (phase group {})",
            self.source_path().display(),
            self.phase_group
        );
        let _ = suite_root;
    }

    pub fn phase_group(&self) -> &str {
        &self.phase_group
    }

    pub fn fixture_refs(&self) -> &[PathBuf] {
        &self.fixture_refs
    }

    pub fn materialize_available_fixture_refs(
        &self,
        suite_root: &Path,
        dest_root: &Path,
    ) -> Vec<PathBuf> {
        self.fixture_refs
            .iter()
            .filter_map(|fixture_ref| {
                let source = resolve_fixture_source(suite_root, fixture_ref)?;
                let destination = dest_root.join(fixture_ref);
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent).unwrap_or_else(|error| {
                        panic!("failed to create {}: {error}", parent.display())
                    });
                }

                match source {
                    FixtureSource::Direct(path) => {
                        if !destination.exists() {
                            #[cfg(unix)]
                            std::os::unix::fs::symlink(&path, &destination).unwrap_or_else(
                                |error| {
                                    panic!(
                                        "failed to materialize fixture {} -> {}: {error}",
                                        path.display(),
                                        destination.display()
                                    )
                                },
                            );
                        }
                    }
                    FixtureSource::UuEncoded(path) => {
                        let decoded = decode_uu_file(&path);
                        fs::write(&destination, decoded).unwrap_or_else(|error| {
                            panic!(
                                "failed to write decoded fixture {}: {error}",
                                destination.display()
                            )
                        });
                    }
                }

                Some(destination)
            })
            .collect()
    }
}

pub(crate) fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("safe crate should live under repo root")
        .to_path_buf()
}

fn resolve_fixture_source(suite_root: &Path, fixture_ref: &Path) -> Option<FixtureSource> {
    let direct = suite_root.join(fixture_ref);
    if direct.is_file() {
        return Some(FixtureSource::Direct(direct));
    }

    let uuencoded = PathBuf::from(format!("{}.uu", direct.display()));
    if uuencoded.is_file() {
        return Some(FixtureSource::UuEncoded(uuencoded));
    }

    None
}

fn decode_uu_file(path: &Path) -> Vec<u8> {
    let contents = fs::read_to_string(path).unwrap_or_else(|error| {
        panic!(
            "failed to read uuencoded fixture {}: {error}",
            path.display()
        )
    });
    let mut started = false;
    let mut decoded = Vec::new();

    for line in contents.lines() {
        if !started {
            if line.starts_with("begin ") {
                started = true;
            }
            continue;
        }

        if line == "end" {
            break;
        }

        let bytes = line.as_bytes();
        if bytes.is_empty() {
            continue;
        }
        let count = uu_value(bytes[0]) as usize;
        if count == 0 {
            continue;
        }

        let mut offset = 1usize;
        let mut produced = 0usize;
        while produced < count && offset + 3 < bytes.len() {
            let a = uu_value(bytes[offset]);
            let b = uu_value(bytes[offset + 1]);
            let c = uu_value(bytes[offset + 2]);
            let d = uu_value(bytes[offset + 3]);

            let chunk = [
                (a << 2) | (b >> 4),
                ((b & 0x0f) << 4) | (c >> 2),
                ((c & 0x03) << 6) | d,
            ];
            let take = (count - produced).min(3);
            decoded.extend_from_slice(&chunk[..take]);
            produced += take;
            offset += 4;
        }
    }

    decoded
}

fn uu_value(byte: u8) -> u8 {
    match byte {
        b'`' | b' ' => 0,
        _ => byte.wrapping_sub(32) & 0x3f,
    }
}
