use std::collections::BTreeMap;
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
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("safe crate should live under repo root")
        .to_path_buf()
}
