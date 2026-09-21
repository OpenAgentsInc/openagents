#!/usr/bin/env python3
"""Resolve changed paths into the Cargo packages a scoped gate must cover.

Reads the paths that differ from a ref (committed) plus uncommitted edits,
maps `crates/<name>/` paths to package names, maps repository data
directories to the crate that loads them, and reports workspace-wide files
that invalidate crate scoping entirely.
"""
import argparse
import json
import subprocess
import sys
from pathlib import Path

# Repository files whose change can affect every crate's build or checks.
WORKSPACE_FILES = {
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    "rust-toolchain",
    "rustfmt.toml",
    "deny.toml",
    "build.rs",
    ".cargo/config.toml",
}
# Data directories a crate loads at runtime; changes there are that crate's
# coverage, not prose.
DATA_DIRS = {
    "programs/": "coder",
    "questions/": "coder",
    "capabilities/": "coder",
    "sources/": "coder",
}


def git(root: Path, *args: str) -> list[str]:
    result = subprocess.run(
        ["git", "-C", str(root), *args],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise SystemExit(f"git {' '.join(args)}: {result.stderr.strip()}")
    return [line for line in result.stdout.splitlines() if line]


def changed_paths(root: Path, ref: str, uncommitted: bool) -> list[str]:
    """Committed paths since `ref`, plus the working tree's own edits."""
    paths = set(git(root, "diff", "--name-only", f"{ref}...HEAD"))
    if uncommitted:
        for line in git(root, "status", "--porcelain", "--untracked-files=all"):
            # `XY path` or `XY orig -> renamed`; take the final path.
            entry = line[3:]
            if " -> " in entry:
                entry = entry.rsplit(" -> ", 1)[1]
            paths.add(entry)
    return sorted(paths)


def package_of(root: Path, name: str) -> str:
    """The `[package] name` a crates/<name> directory actually builds."""
    manifest = root / "crates" / name / "Cargo.toml"
    if manifest.exists():
        for line in manifest.read_text().splitlines():
            stripped = line.strip()
            if stripped.startswith("name"):
                return stripped.split('"')[1]
    return name


def resolve(root: Path, paths: list[str]) -> dict:
    crates: set[str] = set()
    workspace = False
    uncovered: list[str] = []
    for path in paths:
        if path.startswith("crates/"):
            parts = path.split("/")
            if len(parts) > 2:
                crates.add(package_of(root, parts[1]))
            continue
        matched_dir = next(
            (pkg for prefix, pkg in DATA_DIRS.items() if path.startswith(prefix)),
            None,
        )
        if matched_dir:
            crates.add(matched_dir)
        elif path in WORKSPACE_FILES or any(
            path.startswith(prefix) for prefix in (".cargo/",)
        ):
            workspace = True
        else:
            uncovered.append(path)
    return {
        "crates": sorted(crates),
        "workspace": workspace,
        "uncovered": uncovered,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ref", default="origin/main",
                        help="the base ref paths diff against")
    parser.add_argument("--committed-only", action="store_true",
                        help="ignore uncommitted edits")
    parser.add_argument("--root", default=".",
                        help="repository root")
    args = parser.parse_args()
    root = Path(args.root).resolve()
    paths = changed_paths(root, args.ref, not args.committed_only)
    plan = resolve(root, paths)
    plan["ref"] = args.ref
    plan["changed"] = paths
    print(json.dumps(plan, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
