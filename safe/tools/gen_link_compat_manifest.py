#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
BUILD_CONTRACT = ROOT / "generated" / "original_build_contract.json"
OUTPUT = ROOT / "generated" / "link_compat_manifest.json"


def libarchive_test_object_path(source_path: str) -> str:
    if source_path.startswith("original/libarchive-3.7.2/test_utils/"):
        stem = Path(source_path).stem
        return (
            Path("safe/generated/original_link_objects/libarchive_test/test_utils")
            / f"libarchive_test-{stem}.o"
        ).as_posix()
    if source_path.startswith("original/libarchive-3.7.2/libarchive/test/"):
        stem = Path(source_path).stem
        return (
            Path("safe/generated/original_link_objects/libarchive_test/libarchive/test")
            / f"test-{stem}.o"
        ).as_posix()
    raise SystemExit(f"unexpected libarchive_test consumer source: {source_path}")


def build_manifest() -> dict[str, object]:
    contract = json.loads(BUILD_CONTRACT.read_text(encoding="utf-8"))

    rows = []
    targets = []

    libarchive_sources = contract["link_targets"]["libarchive_test"]["consumer_sources"]
    libarchive_objects = []
    for order, source_path in enumerate(libarchive_sources):
        preserved_object_path = libarchive_test_object_path(source_path)
        if not (ROOT.parent / preserved_object_path).exists():
            raise SystemExit(f"missing preserved object: {preserved_object_path}")
        record = {
            "target_name": "libarchive_test",
            "source_path": source_path,
            "preserved_object_path": preserved_object_path,
            "link_order": order,
        }
        rows.append(record)
        libarchive_objects.append(record)

    example_records = [
        (
            "minitar",
            "original/libarchive-3.7.2/examples/minitar/minitar.c",
            "safe/generated/original_link_objects/examples/minitar.o",
        ),
        (
            "untar",
            "original/libarchive-3.7.2/examples/untar.c",
            "safe/generated/original_link_objects/examples/untar.o",
        ),
    ]
    example_target_members: dict[str, list[dict[str, object]]] = {}
    for order, (target_name, source_path, preserved_object_path) in enumerate(example_records):
        if not (ROOT.parent / preserved_object_path).exists():
            raise SystemExit(f"missing preserved object: {preserved_object_path}")
        record = {
            "target_name": target_name,
            "source_path": source_path,
            "preserved_object_path": preserved_object_path,
            "link_order": 0,
        }
        rows.append(record)
        example_target_members[target_name] = [record]

    targets.append(
        {
            "target_name": "libarchive_test",
            "final_link_target_name": "libarchive_test",
            "ordered_objects": libarchive_objects,
            "extra_libraries": contract["link_targets"]["libarchive_test"]["extra_libraries"],
            "must_run_as_part_of_link_compat_verification": True,
            "run_contract": {
                "working_directory_policy": "fresh_tempdir",
                "fixture_roots": {
                    "reference_dir": "original/libarchive-3.7.2/libarchive/test",
                },
                "environment_overrides": {
                    "LANG": "en_US.UTF-8",
                    "LC_ALL": "en_US.UTF-8",
                },
                "setup_actions": [],
                "steps": [
                    {
                        "type": "run",
                        "argv": ["-r", "{reference_dir}"],
                        "expected_exit_status": 0,
                    }
                ],
            },
        }
    )

    common_example_libs = contract["link_targets"]["examples"]["extra_libraries"]
    targets.append(
        {
            "target_name": "minitar",
            "final_link_target_name": "minitar",
            "ordered_objects": example_target_members["minitar"],
            "extra_libraries": common_example_libs,
            "must_run_as_part_of_link_compat_verification": True,
            "run_contract": {
                "working_directory_policy": "fresh_tempdir",
                "fixture_roots": {},
                "environment_overrides": {},
                "setup_actions": [
                    {
                        "type": "write_file",
                        "path": "foo",
                        "content": "Deadbeaf\n",
                    }
                ],
                "steps": [
                    {
                        "type": "run",
                        "argv": ["-cf", "foo.tar", "foo"],
                        "expected_exit_status": 0,
                    },
                    {
                        "type": "assert_mime",
                        "path": "foo.tar",
                        "expected_mime": "application/x-tar",
                    },
                    {
                        "type": "run",
                        "argv": ["-czf", "foo.tar.gz", "foo"],
                        "expected_exit_status": 0,
                    },
                    {
                        "type": "assert_mime",
                        "path": "foo.tar.gz",
                        "expected_mime": "application/gzip",
                    },
                    {
                        "type": "run",
                        "argv": ["-cyf", "foo.tar.bz2", "foo"],
                        "expected_exit_status": 0,
                    },
                    {
                        "type": "assert_mime",
                        "path": "foo.tar.bz2",
                        "expected_mime": "application/x-bzip2",
                    },
                    {
                        "type": "copy_file",
                        "source": "foo",
                        "destination": "foo.orig",
                    },
                    {
                        "type": "remove_path",
                        "path": "foo",
                    },
                    {
                        "type": "run",
                        "argv": ["-xf", "foo.tar"],
                        "expected_exit_status": 0,
                    },
                    {
                        "type": "assert_files_equal",
                        "left": "foo",
                        "right": "foo.orig",
                    },
                    {
                        "type": "remove_path",
                        "path": "foo",
                    },
                    {
                        "type": "run",
                        "argv": ["-xf", "foo.tar.gz"],
                        "expected_exit_status": 0,
                    },
                    {
                        "type": "assert_files_equal",
                        "left": "foo",
                        "right": "foo.orig",
                    },
                    {
                        "type": "remove_path",
                        "path": "foo",
                    },
                    {
                        "type": "run",
                        "argv": ["-xf", "foo.tar.bz2"],
                        "expected_exit_status": 0,
                    },
                    {
                        "type": "assert_files_equal",
                        "left": "foo",
                        "right": "foo.orig",
                    },
                ],
            },
        }
    )

    targets.append(
        {
            "target_name": "untar",
            "final_link_target_name": "untar",
            "ordered_objects": example_target_members["untar"],
            "extra_libraries": common_example_libs,
            "must_run_as_part_of_link_compat_verification": True,
            "run_contract": {
                "working_directory_policy": "fresh_tempdir",
                "fixture_roots": {},
                "environment_overrides": {},
                "setup_actions": [
                    {
                        "type": "write_file",
                        "path": "foo",
                        "content": "Deadbeaf\n",
                    },
                    {
                        "type": "copy_file",
                        "source": "foo",
                        "destination": "foo.orig",
                    },
                    {
                        "type": "host_run",
                        "argv": ["tar", "-cf", "foo.tar", "foo"],
                        "expected_exit_status": 0,
                    },
                    {
                        "type": "remove_path",
                        "path": "foo",
                    },
                ],
                "steps": [
                    {
                        "type": "run",
                        "argv": ["-xf", "foo.tar"],
                        "expected_exit_status": 0,
                    },
                    {
                        "type": "assert_files_equal",
                        "left": "foo",
                        "right": "foo.orig",
                    },
                ],
            },
        }
    )

    for target in targets:
        if target["target_name"] == "libarchive_test":
            for item in target["ordered_objects"]:
                source_path = item["source_path"]
                if not (
                    source_path.startswith("original/libarchive-3.7.2/test_utils/")
                    or source_path.startswith("original/libarchive-3.7.2/libarchive/test/")
                ):
                    raise SystemExit(
                        f"libarchive_test includes disallowed preserved source: {source_path}"
                    )

    return {
        "schema_version": 1,
        "objects": rows,
        "targets": targets,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="validate the checked-in manifest")
    args = parser.parse_args()

    if not BUILD_CONTRACT.exists():
        raise SystemExit(f"missing original build contract: {BUILD_CONTRACT}")

    manifest = build_manifest()
    rendered = json.dumps(manifest, indent=2, sort_keys=True) + "\n"

    if args.check:
        if not OUTPUT.exists():
            raise SystemExit(f"missing link-compat manifest: {OUTPUT}")
        if OUTPUT.read_text(encoding="utf-8") != rendered:
            raise SystemExit("safe/generated/link_compat_manifest.json is out of date")
    else:
        OUTPUT.parent.mkdir(parents=True, exist_ok=True)
        OUTPUT.write_text(rendered, encoding="utf-8")

    return 0


if __name__ == "__main__":
    sys.exit(main())
