#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"

for path in \
  "$ROOT/generated/link_compat_manifest.json" \
  "$ROOT/generated/original_package_metadata.json" \
  "$ROOT/debian/libarchive13t64.symbols" \
  "$ROOT/abi/original_exported_symbols.txt" \
  "$ROOT/abi/original_version_info.txt"
do
  [[ -f "$path" ]] || {
    printf 'missing required link-compat input: %s\n' "$path" >&2
    exit 1
  }
done

cargo build --manifest-path "$ROOT/Cargo.toml" --release >/dev/null

python3 - "$ROOT" <<'PY'
import filecmp
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

root = Path(sys.argv[1])
repo_root = root.parent
manifest = json.loads((root / "generated/link_compat_manifest.json").read_text(encoding="utf-8"))
package_metadata = json.loads(
    (root / "generated/original_package_metadata.json").read_text(encoding="utf-8")
)
lib_dir = root / "target/release"
libarchive = lib_dir / "libarchive.so"
libarchive_static = lib_dir / "libarchive.a"
build_dir = root / "target/link-compat"
symbols_file = root / "debian/libarchive13t64.symbols"
original_exports_file = root / "abi/original_exported_symbols.txt"
original_version_info_file = root / "abi/original_version_info.txt"


def run(cmd, *, cwd=None, env=None, check=True):
    result = subprocess.run(
        cmd,
        cwd=cwd,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if check and result.returncode != 0:
        raise RuntimeError(
            f"command failed ({result.returncode}): {' '.join(cmd)}\n"
            f"stdout:\n{result.stdout}\n"
            f"stderr:\n{result.stderr}"
        )
    return result


def resolve_repo_path(path: str) -> Path:
    candidate = Path(path)
    if candidate.is_absolute():
        return candidate
    return repo_root / candidate


def ensure_build_tree_links() -> None:
    soname_name = Path(package_metadata["runtime_soname_install_path"]).name
    runtime_name = Path(package_metadata["runtime_shared_library_install_path"]).name
    for link_name in (soname_name, runtime_name):
        link_path = lib_dir / link_name
        if link_path.exists() or link_path.is_symlink():
            link_path.unlink()
        link_path.symlink_to("libarchive.so")


def extract_exports(elf_path: Path) -> list[str]:
    output = run(["readelf", "--dyn-syms", "--wide", str(elf_path)]).stdout
    symbols: set[str] = set()
    for line in output.splitlines():
        line = line.strip()
        if not line or ":" not in line:
            continue
        columns = line.split()
        if len(columns) < 8:
            continue
        if columns[4] != "GLOBAL" or columns[6] == "UND":
            continue
        name = columns[-1].split("@", 1)[0]
        if name:
            symbols.add(name)
    return sorted(symbols)


def load_live_symbols(path: Path) -> list[str]:
    symbols: set[str] = set()
    for raw_line in path.read_text(encoding="utf-8").splitlines():
        line = raw_line.rstrip()
        if not line or line.startswith("#") or line.startswith("*"):
            continue
        if not line.startswith(" "):
            continue
        symbols.add(line.split()[0].split("@", 1)[0])
    return sorted(symbols)


def load_plain_symbols(path: Path) -> list[str]:
    return sorted(
        {
            line.strip()
            for line in path.read_text(encoding="utf-8").splitlines()
            if line.strip()
        }
    )


def extract_defined_version_names(version_info: str) -> list[str]:
    names: set[str] = set()
    in_definition_section = False
    for line in version_info.splitlines():
        if line.startswith("Version definition section "):
            in_definition_section = True
            continue
        if line.startswith("Version needs section ") or line.startswith("Version symbols section "):
            in_definition_section = False
        if not in_definition_section:
            continue
        if "Name:" not in line:
            continue
        names.add(line.split("Name:", 1)[1].split()[0])
    return sorted(names)


def compare_sets(label: str, actual: list[str], expected: list[str]) -> None:
    actual_set = set(actual)
    expected_set = set(expected)
    if actual_set == expected_set:
        return
    missing = sorted(expected_set - actual_set)
    extra = sorted(actual_set - expected_set)
    raise RuntimeError(
        f"{label} drifted from the recorded contract\n"
        f"missing: {missing[:20]}\n"
        f"extra: {extra[:20]}"
    )


def apply_placeholders(value: str, fixture_roots: dict[str, Path]) -> str:
    rendered = value
    for key, path in fixture_roots.items():
        rendered = rendered.replace(f"{{{key}}}", str(path))
    return rendered


def ensure_locale(environment: dict[str, str]) -> Path | None:
    if environment.get("LANG") != "en_US.UTF-8" or environment.get("LC_ALL") != "en_US.UTF-8":
        return None

    available = {
        entry.strip().lower()
        for entry in run(["locale", "-a"]).stdout.splitlines()
        if entry.strip()
    }
    if "en_us.utf8" in available or "en_us.utf-8" in available:
        return None

    locale_root = Path(tempfile.mkdtemp(prefix="link-compat-locales-", dir=build_dir))
    locale_path = locale_root / "en_US.UTF-8"
    run(
        [
            "localedef",
            "-i",
            "/usr/share/i18n/locales/en_US",
            "-c",
            "-f",
            "UTF-8",
            "-A",
            "/usr/share/locale/locale.alias",
            str(locale_path),
        ]
    )
    environment["LOCPATH"] = str(locale_root)
    return locale_root


def apply_action(
    target_name: str,
    action: dict,
    *,
    workdir: Path,
    fixture_roots: dict[str, Path],
    environment: dict[str, str],
) -> None:
    action_type = action["type"]
    if action_type == "write_file":
        path = workdir / action["path"]
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(action["content"], encoding="utf-8")
    elif action_type == "copy_file":
        src = workdir / action["source"]
        dst = workdir / action["destination"]
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(src, dst)
    elif action_type == "remove_path":
        path = workdir / action["path"]
        if path.is_dir() and not path.is_symlink():
            shutil.rmtree(path)
        else:
            path.unlink(missing_ok=True)
    elif action_type == "host_run":
        argv = [apply_placeholders(arg, fixture_roots) for arg in action["argv"]]
        result = run(argv, cwd=workdir, env=environment, check=False)
        if result.returncode != action["expected_exit_status"]:
            raise RuntimeError(
                f"{target_name}: host_run exit status {result.returncode} != "
                f"{action['expected_exit_status']}\n{result.stderr}"
            )
    else:
        raise RuntimeError(f"unsupported contract action {action_type} for {target_name}")


def run_contract(target_name: str, executable: Path, contract: dict) -> None:
    if contract.get("working_directory_policy") != "fresh_tempdir":
        raise RuntimeError(f"unsupported working_directory_policy for {target_name}")

    workdir = Path(tempfile.mkdtemp(prefix=f"{target_name}-", dir=build_dir))
    fixture_roots = {
        key: resolve_repo_path(path)
        for key, path in contract.get("fixture_roots", {}).items()
    }
    environment = os.environ.copy()
    ld_library_path = str(lib_dir)
    if environment.get("LD_LIBRARY_PATH"):
        ld_library_path += os.pathsep + environment["LD_LIBRARY_PATH"]
    environment["LD_LIBRARY_PATH"] = ld_library_path
    environment.update(contract.get("environment_overrides", {}))
    locale_root = ensure_locale(environment)

    try:
        for action in contract.get("setup_actions", []):
            apply_action(
                target_name,
                action,
                workdir=workdir,
                fixture_roots=fixture_roots,
                environment=environment,
            )

        for step in contract.get("steps", []):
            step_type = step["type"]
            if step_type == "run":
                argv = [str(executable)] + [
                    apply_placeholders(arg, fixture_roots) for arg in step["argv"]
                ]
                result = run(argv, cwd=workdir, env=environment, check=False)
                if result.returncode != step["expected_exit_status"]:
                    raise RuntimeError(
                        f"{target_name}: run exit status {result.returncode} != "
                        f"{step['expected_exit_status']}\nstdout:\n{result.stdout}\n"
                        f"stderr:\n{result.stderr}"
                    )
            elif step_type == "assert_mime":
                target = workdir / step["path"]
                mime = run(["file", "-b", "--mime-type", str(target)]).stdout.strip()
                if mime != step["expected_mime"]:
                    raise RuntimeError(
                        f"{target_name}: mime mismatch for {target}: {mime} != {step['expected_mime']}"
                    )
            elif step_type == "assert_files_equal":
                left = workdir / step["left"]
                right = workdir / step["right"]
                if not filecmp.cmp(left, right, shallow=False):
                    raise RuntimeError(f"{target_name}: files differ: {left} != {right}")
            elif step_type in {"write_file", "copy_file", "remove_path", "host_run"}:
                apply_action(
                    target_name,
                    step,
                    workdir=workdir,
                    fixture_roots=fixture_roots,
                    environment=environment,
                )
            else:
                raise RuntimeError(f"unsupported step type {step_type} for {target_name}")
    finally:
        if locale_root is not None:
            shutil.rmtree(locale_root, ignore_errors=True)


if not libarchive.exists():
    raise RuntimeError(f"missing built safe library: {libarchive}")
if not libarchive_static.exists():
    raise RuntimeError(f"missing built safe static library: {libarchive_static}")

shutil.rmtree(build_dir, ignore_errors=True)
build_dir.mkdir(parents=True, exist_ok=True)
ensure_build_tree_links()

built_exports = extract_exports(libarchive)
compare_sets(
    "exported symbol set",
    built_exports,
    load_plain_symbols(original_exports_file),
)
compare_sets(
    "live Debian symbols set",
    built_exports,
    load_live_symbols(symbols_file),
)

built_version_info = run(["readelf", "--version-info", "--wide", str(libarchive)]).stdout
compare_sets(
    "defined version-info names",
    extract_defined_version_names(built_version_info),
    extract_defined_version_names(original_version_info_file.read_text(encoding="utf-8")),
)

cc = os.environ.get("CC", "cc")
ldflags = os.environ.get("LDFLAGS", "").split()

for target in manifest["targets"]:
    output = build_dir / target["final_link_target_name"]
    object_paths = [resolve_repo_path(obj["preserved_object_path"]) for obj in target["ordered_objects"]]
    for object_path in object_paths:
        if not object_path.is_file():
            raise RuntimeError(f"missing preserved object file: {object_path}")

    link_cmd = [cc, "-o", str(output)]
    link_cmd.extend(str(path) for path in object_paths)
    link_cmd.append(str(libarchive_static))
    link_cmd.extend(target.get("extra_libraries", []))
    link_cmd.extend(ldflags)
    run(link_cmd)

    if target.get("run_contract") is not None:
        run_contract(target["final_link_target_name"], output, target["run_contract"])
PY
