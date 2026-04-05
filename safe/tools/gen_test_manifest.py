#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
import sys
from collections import Counter
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
REPO_ROOT = ROOT.parent
ORIGINAL_ROOT = REPO_ROOT / "original" / "libarchive-3.7.2"
GENERATED_C_BUILD = ROOT / "generated" / "original_c_build"
PHASE_GROUPS_PATH = ROOT / "config" / "libarchive_test_phase_groups.json"
OUTPUT = ROOT / "generated" / "test_manifest.json"

SUITES = {
    "libarchive": {
        "list_h": GENERATED_C_BUILD / "libarchive" / "test" / "list.h",
        "source_dir": ORIGINAL_ROOT / "libarchive" / "test",
    },
    "tar": {
        "list_h": GENERATED_C_BUILD / "tar" / "test" / "list.h",
        "source_dir": ORIGINAL_ROOT / "tar" / "test",
    },
    "cpio": {
        "list_h": GENERATED_C_BUILD / "cpio" / "test" / "list.h",
        "source_dir": ORIGINAL_ROOT / "cpio" / "test",
    },
    "cat": {
        "list_h": GENERATED_C_BUILD / "cat" / "test" / "list.h",
        "source_dir": ORIGINAL_ROOT / "cat" / "test",
    },
    "unzip": {
        "list_h": GENERATED_C_BUILD / "unzip" / "test" / "list.h",
        "source_dir": ORIGINAL_ROOT / "unzip" / "test",
    },
}

EXPECTED_SUITE_COUNTS = {
    "libarchive": 604,
    "tar": 70,
    "cpio": 48,
    "cat": 18,
    "unzip": 22,
}

EXPECTED_LIBARCHIVE_PHASE_COUNTS = {
    "foundation": 16,
    "write_disk": 73,
    "read_mainstream": 147,
    "advanced_formats": 368,
}

DEFINE_TEST_RE = re.compile(r"(?m)^\s*DEFINE_TEST\(([^)]+)\)")
STRING_LITERAL_RE = re.compile(r'"((?:[^"\\]|\\.)*)"')
FIXTURE_SUFFIX_RE = re.compile(
    r"\.(?:"
    r"7z|Z|ar|bz2|cab|cpio|data|exe|grz|gz|iso|jar|json|lrz|lz|lz4|lzh|lzma|lzo|"
    r"mtree|out|pax|rar|rpm|tar|tbz|tgz|tlz|txz|txt|uu|warc|xps|xz|zip|zipx|zst"
    r")$"
)

FOUNDATION_FILES = {
    "test_acl_nfs4.c",
    "test_acl_posix1e.c",
    "test_acl_text.c",
    "test_archive_api_feature.c",
    "test_archive_clear_error.c",
    "test_archive_getdate.c",
    "test_archive_match_owner.c",
    "test_archive_match_path.c",
    "test_archive_pathmatch.c",
    "test_archive_set_error.c",
    "test_archive_string.c",
    "test_entry.c",
    "test_entry_strmode.c",
    "test_link_resolver.c",
}

ADVANCED_EXACT = {
    "test_archive_digest.c",
    "test_archive_string_conversion.c",
}
ADVANCED_REGEXES = [
    re.compile(r"^test_compat_.*\.c$"),
    re.compile(r"^test_fuzz\.c$"),
    re.compile(r"^test_archive_write_add_filter_by_name\.c$"),
    re.compile(r"^test_archive_write_set_filter_option\.c$"),
    re.compile(r"^test_archive_write_set_format_by_name\.c$"),
    re.compile(r"^test_archive_write_set_format_filter_by_ext\.c$"),
    re.compile(r"^test_archive_write_set_format_option\.c$"),
    re.compile(r"^test_archive_write_set_option\.c$"),
    re.compile(r"^test_archive_write_set_options\.c$"),
    re.compile(r"^test_archive_write_set_passphrase\.c$"),
    re.compile(r"^test_read_set_format\.c$"),
    re.compile(r"^test_read_format_(7zip|cab|iso|isojoliet|isorr|isozisofs|lha|mtree|rar|rar5|warc|xar|zip).*\.c$"),
    re.compile(r"^test_write_format_(7zip|iso9660|mtree|warc|xar|zip).*\.c$"),
    re.compile(r"^test_write_read_format_zip\.c$"),
    re.compile(r"^test_zip_filename_encoding\.c$"),
]

WRITE_DISK_EXACT = {
    "test_acl_pax.c",
    "test_acl_platform_nfs4.c",
    "test_acl_platform_posix1e.c",
    "test_archive_cmdline.c",
    "test_archive_match_time.c",
    "test_empty_write.c",
    "test_extattr_freebsd.c",
    "test_pax_xattr_header.c",
    "test_read_disk.c",
    "test_read_disk_directory_traversals.c",
    "test_read_disk_entry_from_file.c",
    "test_read_extract.c",
    "test_short_writes.c",
    "test_warn_missing_hardlink_target.c",
    "test_xattr_platform.c",
}
WRITE_DISK_REGEXES = [re.compile(r"^test_write_.*\.c$")]

READ_MAINSTREAM_EXACT = {
    "test_bad_fd.c",
    "test_filter_count.c",
}
READ_MAINSTREAM_REGEXES = [
    re.compile(
        r"^test_archive_read_(add_passphrase|close_twice|close_twice_open_fd|close_twice_open_filename|multiple_data_objects|next_header_empty|next_header_raw|open2|set_filter_option|set_format_option|set_option|set_options|support)\.c$"
    ),
    re.compile(r"^test_open_(failure|fd|file|filename)\.c$"),
    re.compile(r"^test_(gnutar|pax|ustar)_filename_encoding\.c$"),
    re.compile(
        r"^test_read_(data_large|file_nonexistent|large|pax_xattr_rht_security_selinux|pax_xattr_schily|pax_truncated|position|too_many_filters|truncated|truncated_filter)\.c$"
    ),
    re.compile(r"^test_read_filter_(compress|grzip|lrzip|lzop|lzop_multiple_parts|program|program_signature|uudecode|uudecode_raw)\.c$"),
    re.compile(r"^test_read_format_(ar|cpio_.*|empty|gtar_.*|pax_bz2|raw|tar.*|tbz|tgz|tlz|txz|tz|ustar_.*)\.c$"),
    re.compile(r"^test_(sparse_basic|tar_filenames|tar_large|ustar_filenames)\.c$"),
]


def load_list_h_entries(path: Path) -> list[str]:
    if not path.exists():
        raise SystemExit(f"missing generated list.h snapshot: {path}")
    return DEFINE_TEST_RE.findall(path.read_text(encoding="utf-8"))


def extract_fixture_refs(source_text: str) -> list[str]:
    refs = set()
    for raw in STRING_LITERAL_RE.findall(source_text):
        try:
            candidate = bytes(raw, "utf-8").decode("unicode_escape")
        except UnicodeDecodeError:
            candidate = raw
        if candidate.startswith("%") or "\n" in candidate or "\t" in candidate:
            continue
        if FIXTURE_SUFFIX_RE.search(candidate):
            refs.add(candidate)
    return sorted(refs)


def scan_suite_sources(source_dir: Path) -> tuple[dict[str, str], dict[str, list[str]]]:
    mapping: dict[str, str] = {}
    fixtures_by_symbol: dict[str, list[str]] = {}
    for source_path in sorted(source_dir.glob("*.c")):
        text = source_path.read_text(encoding="utf-8", errors="replace")
        define_tests = DEFINE_TEST_RE.findall(text)
        fixture_refs = extract_fixture_refs(text)
        for define_test in define_tests:
            if define_test in mapping:
                raise SystemExit(f"duplicate DEFINE_TEST({define_test}) in source scan")
            mapping[define_test] = source_path.relative_to(REPO_ROOT).as_posix()
            fixtures_by_symbol[define_test] = fixture_refs
    return mapping, fixtures_by_symbol


def classify_phase_group(source_file: str) -> str:
    basename = Path(source_file).name
    if basename in FOUNDATION_FILES:
        return "foundation"
    if basename in ADVANCED_EXACT or any(regex.match(basename) for regex in ADVANCED_REGEXES):
        return "advanced_formats"
    if basename in WRITE_DISK_EXACT or any(regex.match(basename) for regex in WRITE_DISK_REGEXES):
        return "write_disk"
    if basename in READ_MAINSTREAM_EXACT or any(regex.match(basename) for regex in READ_MAINSTREAM_REGEXES):
        return "read_mainstream"
    return "advanced_formats"


def build_phase_group_rows(list_entries: list[str], source_map: dict[str, str]) -> dict[str, object]:
    rows = []
    for define_test in list_entries:
        source_file = source_map.get(define_test)
        if source_file is None:
            raise SystemExit(f"no source file found for {define_test}")
        rows.append(
            {
                "define_test": define_test,
                "source_file": source_file,
                "phase_group": classify_phase_group(source_file),
            }
        )

    counts = Counter(row["phase_group"] for row in rows)
    if dict(counts) != EXPECTED_LIBARCHIVE_PHASE_COUNTS:
        raise SystemExit(
            f"libarchive phase-group counts mismatch: expected {EXPECTED_LIBARCHIVE_PHASE_COUNTS}, got {dict(counts)}"
        )

    by_source = {}
    for row in rows:
        previous = by_source.setdefault(row["source_file"], row["phase_group"])
        if previous != row["phase_group"]:
            raise SystemExit(f"source file split across phase groups: {row['source_file']}")

    return {
        "schema_version": 1,
        "counts": dict(counts),
        "rows": rows,
    }


def load_or_validate_phase_groups(expected: dict[str, object], check_mode: bool) -> dict[str, str]:
    if not PHASE_GROUPS_PATH.exists():
        raise SystemExit(f"missing phase-group table: {PHASE_GROUPS_PATH}")

    existing = json.loads(PHASE_GROUPS_PATH.read_text(encoding="utf-8"))
    if check_mode and existing != expected:
        raise SystemExit("safe/config/libarchive_test_phase_groups.json is out of date")

    rows = existing.get("rows", [])
    seen = set()
    mapping: dict[str, str] = {}
    for row in rows:
        define_test = row["define_test"]
        if define_test in seen:
            raise SystemExit(f"duplicate phase-group entry for {define_test}")
        seen.add(define_test)
        phase_group = row["phase_group"]
        if phase_group not in EXPECTED_LIBARCHIVE_PHASE_COUNTS:
            raise SystemExit(f"unknown phase group: {phase_group}")
        mapping[define_test] = phase_group

    expected_names = {row["define_test"] for row in expected["rows"]}
    actual_names = set(mapping)
    if actual_names != expected_names:
        missing = sorted(expected_names - actual_names)
        extra = sorted(actual_names - expected_names)
        raise SystemExit(
            f"phase-group table mismatch: missing={missing}, extra={extra}"
        )

    counts = Counter(mapping[name] for name in mapping)
    if dict(counts) != EXPECTED_LIBARCHIVE_PHASE_COUNTS:
        raise SystemExit(
            f"phase-group table counts mismatch: expected {EXPECTED_LIBARCHIVE_PHASE_COUNTS}, got {dict(counts)}"
        )

    return mapping


def build_manifest(phase_groups: dict[str, str]) -> dict[str, object]:
    rows = []
    suite_counts: dict[str, int] = {}

    for suite_name, suite_info in SUITES.items():
        list_entries = load_list_h_entries(suite_info["list_h"])
        if len(list_entries) != EXPECTED_SUITE_COUNTS[suite_name]:
            raise SystemExit(
                f"unexpected {suite_name} list.h count: expected {EXPECTED_SUITE_COUNTS[suite_name]}, got {len(list_entries)}"
            )

        source_map, fixtures_map = scan_suite_sources(suite_info["source_dir"])
        scanned_names = set(source_map)
        list_names = set(list_entries)
        extra_scanned = sorted(scanned_names - list_names)
        if extra_scanned:
            raise SystemExit(
                f"DEFINE_TEST entries found by source scanning are absent from generated {suite_name} list.h: {extra_scanned}"
            )

        for order, define_test in enumerate(list_entries, start=1):
            source_file = source_map.get(define_test)
            if source_file is None:
                raise SystemExit(f"missing source mapping for {suite_name}:{define_test}")

            phase_group = phase_groups[define_test] if suite_name == "libarchive" else "all"
            rows.append(
                {
                    "suite": suite_name,
                    "define_test": define_test,
                    "source_file": source_file,
                    "suite_order": order,
                    "fixture_refs": fixtures_map.get(define_test, []),
                    "phase_group": phase_group,
                }
            )

        suite_counts[suite_name] = len(list_entries)

    total_expected = sum(EXPECTED_SUITE_COUNTS.values())
    if len(rows) != total_expected:
        raise SystemExit(f"expected {total_expected} manifest rows, found {len(rows)}")

    return {
        "schema_version": 1,
        "counts": {
            "suite_rows": suite_counts,
            "total_rows": len(rows),
            "libarchive_phase_groups": EXPECTED_LIBARCHIVE_PHASE_COUNTS,
        },
        "rows": rows,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="validate checked-in outputs")
    parser.add_argument(
        "--write-phase-groups",
        action="store_true",
        help="write safe/config/libarchive_test_phase_groups.json before generating the manifest",
    )
    args = parser.parse_args()

    libarchive_entries = load_list_h_entries(SUITES["libarchive"]["list_h"])
    libarchive_source_map, _ = scan_suite_sources(SUITES["libarchive"]["source_dir"])
    expected_phase_groups = build_phase_group_rows(libarchive_entries, libarchive_source_map)
    expected_phase_groups_rendered = json.dumps(expected_phase_groups, indent=2, sort_keys=True) + "\n"

    if args.write_phase_groups:
        PHASE_GROUPS_PATH.parent.mkdir(parents=True, exist_ok=True)
        PHASE_GROUPS_PATH.write_text(expected_phase_groups_rendered, encoding="utf-8")

    phase_groups = load_or_validate_phase_groups(expected_phase_groups, args.check)
    manifest = build_manifest(phase_groups)
    rendered_manifest = json.dumps(manifest, indent=2, sort_keys=True) + "\n"

    if args.check:
        if not OUTPUT.exists():
            raise SystemExit(f"missing generated manifest: {OUTPUT}")
        if OUTPUT.read_text(encoding="utf-8") != rendered_manifest:
            raise SystemExit("safe/generated/test_manifest.json is out of date")
    else:
        OUTPUT.parent.mkdir(parents=True, exist_ok=True)
        OUTPUT.write_text(rendered_manifest, encoding="utf-8")

    return 0


if __name__ == "__main__":
    sys.exit(main())
