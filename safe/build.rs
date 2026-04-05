use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn trim_component(component: &str) -> u32 {
    component
        .parse::<u32>()
        .unwrap_or_else(|err| panic!("failed to parse version component {component}: {err}"))
}

fn collect_public_symbols(header: &Path, symbols: &mut BTreeSet<String>) {
    let contents = fs::read_to_string(header)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", header.display()));
    for line in contents.lines() {
        if !line.contains("__LA_DECL") {
            continue;
        }
        let mut rest = line;
        while let Some(index) = rest.find("archive_") {
            let candidate = &rest[index..];
            let name_len = candidate
                .chars()
                .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
                .count();
            let name = &candidate[..name_len];
            let suffix = candidate[name_len..].trim_start();
            if suffix.starts_with('(') {
                symbols.insert(name.to_owned());
            }
            rest = &candidate[name_len..];
        }
    }
}

fn write_backend_symbol_prefix(out_dir: &Path, headers: &[PathBuf]) -> PathBuf {
    let mut symbols = BTreeSet::new();
    for header in headers {
        collect_public_symbols(header, &mut symbols);
    }

    let header_path = out_dir.join("backend_symbol_prefix.h");
    let mut output = String::from(
        "#ifndef SAFE_LIBARCHIVE_BACKEND_SYMBOL_PREFIX_H\n#define SAFE_LIBARCHIVE_BACKEND_SYMBOL_PREFIX_H\n",
    );
    for symbol in symbols {
        output.push_str("#define ");
        output.push_str(&symbol);
        output.push_str(" backend_");
        output.push_str(&symbol);
        output.push('\n');
    }
    output.push_str("#endif\n");
    fs::write(&header_path, output)
        .unwrap_or_else(|err| panic!("failed to write {}: {err}", header_path.display()));
    header_path
}

fn extract_api_entries(source: &str) -> Vec<(String, String)> {
    let start = source
        .find("pub struct Api {")
        .unwrap_or_else(|| panic!("failed to locate Api struct in backend.rs"));
    let body_start = source[start..]
        .find('{')
        .map(|offset| start + offset + 1)
        .unwrap_or_else(|| panic!("failed to locate Api struct body"));

    let mut brace_depth = 1usize;
    let mut paren_depth = 0usize;
    let mut fields = Vec::new();
    let mut current = String::new();

    for ch in source[body_start..].chars() {
        match ch {
            '{' => brace_depth += 1,
            '}' => {
                brace_depth -= 1;
                if brace_depth == 0 {
                    break;
                }
            }
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            _ => {}
        }

        if brace_depth != 1 {
            continue;
        }

        current.push(ch);
        if ch == ',' && paren_depth == 0 {
            let field = current.trim();
            if field.contains("unsafe extern \"C\" fn") {
                let field = field.trim_end_matches(',');
                let (name, signature) = field
                    .split_once(':')
                    .unwrap_or_else(|| panic!("malformed Api field: {field}"));
                let name = name.trim().trim_start_matches("pub ").to_owned();
                let signature = signature
                    .trim()
                    .strip_prefix("unsafe extern \"C\" fn")
                    .unwrap_or_else(|| panic!("unexpected Api field signature: {field}"))
                    .trim()
                    .to_owned();
                fields.push((name, signature));
            }
            current.clear();
        }
    }

    fields
}

fn split_top_level_commas(input: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut paren_depth = 0usize;
    let mut angle_depth = 0usize;

    for ch in input.chars() {
        match ch {
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '<' => angle_depth += 1,
            '>' => angle_depth = angle_depth.saturating_sub(1),
            ',' if paren_depth == 0 && angle_depth == 0 => {
                let trimmed = current.trim();
                if !trimmed.is_empty() {
                    parts.push(trimmed.to_owned());
                }
                current.clear();
                continue;
            }
            _ => {}
        }
        current.push(ch);
    }

    let trimmed = current.trim();
    if !trimmed.is_empty() {
        parts.push(trimmed.to_owned());
    }

    parts
}

fn render_extern_signature(signature: &str) -> String {
    let signature = signature.trim();
    let close = signature
        .find(')')
        .unwrap_or_else(|| panic!("malformed function signature: {signature}"));
    let params = signature[1..close].trim();
    let ret = signature[close + 1..].trim();

    let rendered_params = if params.is_empty() {
        String::new()
    } else {
        split_top_level_commas(params)
            .into_iter()
            .enumerate()
            .map(|(index, param)| format!("arg{index}: {param}"))
            .collect::<Vec<_>>()
            .join(", ")
    };

    format!("({rendered_params}) {ret}")
}

fn write_backend_linked_rs(out_dir: &Path, backend_rs: &Path) -> PathBuf {
    let source = fs::read_to_string(backend_rs)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", backend_rs.display()));
    let fields = extract_api_entries(&source);

    let linked_path = out_dir.join("backend_linked.rs");
    let mut output = String::from("unsafe extern \"C\" {\n");
    for (name, signature) in &fields {
        output.push_str("    fn backend_");
        output.push_str(name);
        output.push_str(&render_extern_signature(signature));
        output.push_str(";\n");
    }
    output.push_str("}\n\n");
    output.push_str("static LINKED_API: Api = Api {\n");
    output.push_str("    _library: std::ptr::null_mut(),\n");
    for (name, _) in &fields {
        output.push_str("    ");
        output.push_str(name);
        output.push_str(": backend_");
        output.push_str(name);
        output.push_str(",\n");
    }
    output.push_str("};\n\n");
    output.push_str("pub(crate) fn linked_api() -> &'static Api {\n    &LINKED_API\n}\n");

    fs::write(&linked_path, output)
        .unwrap_or_else(|err| panic!("failed to write {}: {err}", linked_path.display()));
    linked_path
}

fn libarchive_backend_sources(original_dir: &Path) -> Vec<PathBuf> {
    let libarchive_dir = original_dir.join("libarchive");
    let names = [
        "archive_acl.c",
        "archive_blake2s_ref.c",
        "archive_blake2sp_ref.c",
        "archive_check_magic.c",
        "archive_cmdline.c",
        "archive_cryptor.c",
        "archive_digest.c",
        "archive_disk_acl_linux.c",
        "archive_entry.c",
        "archive_entry_copy_stat.c",
        "archive_entry_link_resolver.c",
        "archive_entry_sparse.c",
        "archive_entry_stat.c",
        "archive_entry_strmode.c",
        "archive_entry_xattr.c",
        "archive_getdate.c",
        "archive_hmac.c",
        "archive_match.c",
        "archive_options.c",
        "archive_pack_dev.c",
        "archive_pathmatch.c",
        "archive_ppmd7.c",
        "archive_ppmd8.c",
        "archive_random.c",
        "archive_rb.c",
        "archive_read.c",
        "archive_read_add_passphrase.c",
        "archive_read_append_filter.c",
        "archive_read_data_into_fd.c",
        "archive_read_open_filename.c",
        "archive_read_open_memory.c",
        "archive_read_set_format.c",
        "archive_read_set_options.c",
        "archive_read_support_filter_bzip2.c",
        "archive_read_support_filter_compress.c",
        "archive_read_support_filter_grzip.c",
        "archive_read_support_filter_gzip.c",
        "archive_read_support_filter_lrzip.c",
        "archive_read_support_filter_lz4.c",
        "archive_read_support_filter_lzop.c",
        "archive_read_support_filter_none.c",
        "archive_read_support_filter_program.c",
        "archive_read_support_filter_uu.c",
        "archive_read_support_filter_xz.c",
        "archive_read_support_filter_zstd.c",
        "archive_read_support_format_ar.c",
        "archive_read_support_format_cpio.c",
        "archive_read_support_format_empty.c",
        "archive_read_support_format_raw.c",
        "archive_read_support_format_tar.c",
        "archive_string.c",
        "archive_string_sprintf.c",
        "archive_util.c",
        "archive_version_details.c",
        "archive_virtual.c",
        "archive_write.c",
        "archive_write_add_filter.c",
        "archive_write_add_filter_b64encode.c",
        "archive_write_add_filter_by_name.c",
        "archive_write_add_filter_bzip2.c",
        "archive_write_add_filter_compress.c",
        "archive_write_add_filter_grzip.c",
        "archive_write_add_filter_gzip.c",
        "archive_write_add_filter_lrzip.c",
        "archive_write_add_filter_lz4.c",
        "archive_write_add_filter_lzop.c",
        "archive_write_add_filter_none.c",
        "archive_write_add_filter_program.c",
        "archive_write_add_filter_uuencode.c",
        "archive_write_add_filter_xz.c",
        "archive_write_add_filter_zstd.c",
        "archive_write_open_fd.c",
        "archive_write_open_filename.c",
        "archive_write_open_memory.c",
        "archive_write_set_format.c",
        "archive_write_set_format_ar.c",
        "archive_write_set_format_cpio.c",
        "archive_write_set_format_cpio_binary.c",
        "archive_write_set_format_cpio_newc.c",
        "archive_write_set_format_cpio_odc.c",
        "archive_write_set_format_gnutar.c",
        "archive_write_set_format_pax.c",
        "archive_write_set_format_raw.c",
        "archive_write_set_format_shar.c",
        "archive_write_set_format_ustar.c",
        "archive_write_set_format_v7tar.c",
        "archive_write_set_options.c",
        "archive_write_set_passphrase.c",
        "filter_fork_posix.c",
        "xxhash.c",
    ];

    names
        .into_iter()
        .map(|name| libarchive_dir.join(name))
        .collect()
}

fn build_vendored_backend(
    manifest_dir: &Path,
    original_dir: &Path,
    generated_config_dir: &Path,
    out_dir: &Path,
) {
    let backend_rs = manifest_dir.join("src/common/backend.rs");
    let public_headers = [
        manifest_dir.join("include/archive.h"),
        manifest_dir.join("include/archive_entry.h"),
    ];
    let prefix_header = write_backend_symbol_prefix(out_dir, &public_headers);
    let linked_rs = write_backend_linked_rs(out_dir, &backend_rs);

    println!("cargo:rerun-if-changed={}", backend_rs.display());
    for header in &public_headers {
        println!("cargo:rerun-if-changed={}", header.display());
    }
    println!("cargo:rerun-if-changed={}", prefix_header.display());
    println!("cargo:rerun-if-changed={}", linked_rs.display());

    let mut backend_build = cc::Build::new();
    backend_build
        .warnings(false)
        .pic(true)
        .define("HAVE_CONFIG_H", None)
        .define("LIBARCHIVE_STATIC", None)
        .include(generated_config_dir)
        .include("/usr/include/libxml2")
        .include(original_dir.join("libarchive"))
        .flag_if_supported("-std=gnu99")
        .flag("-include")
        .flag(
            prefix_header
                .to_str()
                .unwrap_or_else(|| panic!("non-utf8 path: {}", prefix_header.display())),
        );

    for source in libarchive_backend_sources(original_dir) {
        println!("cargo:rerun-if-changed={}", source.display());
        backend_build.file(source);
    }

    backend_build.compile("archive_backend");

    for library in ["acl", "bz2", "z", "lzma", "zstd", "lz4", "nettle", "xml2"] {
        println!("cargo:rustc-link-lib={library}");
    }
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let manifest_dir = PathBuf::from(
        env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set for build.rs"),
    );
    let original_dir = manifest_dir.join("../original/libarchive-3.7.2");
    let version_path = original_dir.join("build/version");
    let cmake_path = original_dir.join("CMakeLists.txt");
    let configure_ac_path = original_dir.join("configure.ac");
    let map_path = manifest_dir.join("abi/libarchive.map");
    let build_contract_path = manifest_dir.join("generated/original_build_contract.json");
    let generated_config_dir = manifest_dir.join("generated/original_c_build");
    let generated_config_path = generated_config_dir.join("config.h");
    let variadic_shim = manifest_dir.join("c_shims/archive_set_error.c");

    println!("cargo:rerun-if-changed={}", version_path.display());
    println!("cargo:rerun-if-changed={}", cmake_path.display());
    println!("cargo:rerun-if-changed={}", configure_ac_path.display());
    println!("cargo:rerun-if-changed={}", map_path.display());
    println!("cargo:rerun-if-changed={}", build_contract_path.display());
    println!("cargo:rerun-if-changed={}", generated_config_path.display());
    println!("cargo:rerun-if-changed={}", variadic_shim.display());

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
        configure_ac.contains(
            "ARCHIVE_LIBTOOL_VERSION=$ARCHIVE_INTERFACE:$ARCHIVE_REVISION:$ARCHIVE_MINOR"
        ),
        "unexpected configure.ac libtool version logic"
    );

    let build_contract = fs::read_to_string(&build_contract_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", build_contract_path.display()));
    assert!(
        build_contract.contains("\"config_header\": \"safe/generated/original_c_build/config.h\""),
        "unexpected original build contract config header"
    );
    assert!(
        build_contract.contains("\"-lnettle\"")
            && build_contract.contains("\"-lacl\"")
            && build_contract.contains("\"-llzma\"")
            && build_contract.contains("\"-lzstd\"")
            && build_contract.contains("\"-llz4\"")
            && build_contract.contains("\"-lbz2\"")
            && build_contract.contains("\"-lz\""),
        "unexpected original build contract link libraries"
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

    cc::Build::new()
        .file(&variadic_shim)
        .compile("archive_variadic_shim");
    build_vendored_backend(&manifest_dir, &original_dir, &generated_config_dir, &out_dir);

    let target = env::var("TARGET").unwrap_or_default();
    if target.contains("linux") {
        println!("cargo:rustc-cdylib-link-arg=-Wl,--export-dynamic-symbol=archive_set_error");
        println!(
            "cargo:rustc-cdylib-link-arg=-Wl,--version-script={}",
            map_path.display()
        );
        println!("cargo:rustc-cdylib-link-arg=-Wl,-soname,{}", soname);
    }
}
