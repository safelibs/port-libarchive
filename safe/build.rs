use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn trim_component(component: &str) -> u32 {
    component
        .parse::<u32>()
        .unwrap_or_else(|err| panic!("failed to parse version component {component}: {err}"))
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let manifest_dir = PathBuf::from(
        env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set for build.rs"),
    );
    let original_dir = manifest_dir.join("../original/libarchive-3.7.2");
    let original_libarchive_dir = original_dir.join("libarchive");
    let version_path = original_dir.join("build/version");
    let cmake_path = original_dir.join("CMakeLists.txt");
    let configure_ac_path = original_dir.join("configure.ac");
    let map_path = manifest_dir.join("abi/libarchive.map");
    let generated_config_dir = manifest_dir.join("generated/original_c_build");
    let generated_config_path = generated_config_dir.join("config.h");

    println!("cargo:rerun-if-changed={}", version_path.display());
    println!("cargo:rerun-if-changed={}", cmake_path.display());
    println!("cargo:rerun-if-changed={}", configure_ac_path.display());
    println!("cargo:rerun-if-changed={}", map_path.display());
    println!("cargo:rerun-if-changed={}", generated_config_path.display());

    let version_digits = fs::read_to_string(&version_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", version_path.display()))
        .trim()
        .to_owned();
    assert!(
        version_digits.len() >= 7 && version_digits.chars().all(|ch| ch.is_ascii_digit()),
        "unexpected build/version contents: {version_digits}"
    );

    let major = &version_digits[0..1];
    let minor_raw = &version_digits[1..4];
    let revision_raw = &version_digits[4..7];
    let minor = trim_component(minor_raw);
    let revision = trim_component(revision_raw);
    let package_version = format!("{major}.{minor}.{revision}");

    let cmake = fs::read_to_string(&cmake_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", cmake_path.display()));
    assert!(
        cmake.contains("math(EXPR INTERFACE_VERSION  \"13 + ${_minor}\")")
            && cmake.contains("SET(SOVERSION \"${INTERFACE_VERSION}\")"),
        "unexpected CMake SONAME logic"
    );

    let configure_ac = fs::read_to_string(&configure_ac_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", configure_ac_path.display()));
    assert!(
        configure_ac.contains("ARCHIVE_LIBTOOL_VERSION=$ARCHIVE_INTERFACE:$ARCHIVE_REVISION:$ARCHIVE_MINOR"),
        "unexpected configure.ac libtool version logic"
    );

    let cmake_interface_version = 13 + minor;
    let libtool_current = cmake_interface_version;
    let libtool_age = minor;
    let soname_major = libtool_current
        .checked_sub(libtool_age)
        .expect("libtool current must be >= age");
    let soname = format!("libarchive.so.{soname_major}");

    assert_eq!(version_digits, "3007002");
    assert_eq!(package_version, "3.7.2");
    assert_eq!(soname, "libarchive.so.13");

    let version_string = format!("libarchive {package_version}");
    let version_string_bytes = format!("{version_string}\\0");

    let version_rs = format!(
        "\
pub const LIBARCHIVE_VERSION_NUMBER: i32 = {version_digits};
pub const LIBARCHIVE_PACKAGE_VERSION: &str = \"{package_version}\";
pub const LIBARCHIVE_VERSION_STRING: &str = \"{version_string}\";
pub const LIBARCHIVE_VERSION_STRING_BYTES: &[u8] = b\"{version_string_bytes}\";
pub const LIBARCHIVE_VERSION_DETAILS_BYTES: &[u8] = b\"{version_string_bytes}\";
pub const LIBARCHIVE_SONAME: &str = \"{soname}\";
pub const LIBARCHIVE_CMAKE_INTERFACE_VERSION: u32 = {cmake_interface_version};
pub const LIBARCHIVE_LIBTOOL_CURRENT: u32 = {libtool_current};
pub const LIBARCHIVE_LIBTOOL_AGE: u32 = {libtool_age};
"
    );

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR must be set for build.rs"));
    fs::write(out_dir.join("version.rs"), version_rs)
        .unwrap_or_else(|err| panic!("failed to write generated version.rs: {err}"));

    compile_foundation_c_sources(&original_libarchive_dir, &generated_config_dir);

    let target = env::var("TARGET").unwrap_or_default();
    if target.contains("linux") {
        println!(
            "cargo:rustc-cdylib-link-arg=-Wl,--version-script={}",
            map_path.display()
        );
        println!("cargo:rustc-cdylib-link-arg=-Wl,-soname,{}", soname);
    }
}

fn compile_foundation_c_sources(original_libarchive_dir: &Path, generated_config_dir: &Path) {
    let sources = [
        "archive_acl.c",
        "archive_check_magic.c",
        "archive_entry.c",
        "archive_entry_copy_stat.c",
        "archive_entry_link_resolver.c",
        "archive_entry_sparse.c",
        "archive_entry_stat.c",
        "archive_entry_strmode.c",
        "archive_entry_xattr.c",
        "archive_getdate.c",
        "archive_match.c",
        "archive_pathmatch.c",
        "archive_random.c",
        "archive_rb.c",
        "archive_string.c",
        "archive_string_sprintf.c",
        "archive_util.c",
        "archive_version_details.c",
        "archive_virtual.c",
    ];

    let mut build = cc::Build::new();
    build
        .cargo_metadata(false)
        .warnings(false)
        .include(generated_config_dir)
        .include(original_libarchive_dir)
        .define("HAVE_CONFIG_H", "1");

    for source in sources {
        let path = original_libarchive_dir.join(source);
        println!("cargo:rerun-if-changed={}", path.display());
        build.file(path);
    }

    build.compile("archive_foundation");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR must be set for build.rs"));
    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static:+whole-archive=archive_foundation");

    for library in ["bz2", "lz4", "lzma", "z", "zstd"] {
        println!("cargo:rustc-link-lib={library}");
    }
}
