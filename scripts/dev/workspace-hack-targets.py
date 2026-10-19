#!/usr/bin/env python3
"""Restrict Hakari dependencies to each workspace's targets (Python 3.11+)."""

import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tomllib


HACK = "openagents-workspace-hack"
NATIVE_TARGET = 'cfg(not(target_arch = "wasm32"))'
DESKTOP_TARGET = 'cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))'
GENERATED_DEPENDENCY = {"version": "0.1"}
DEPENDENCY_LINE = f'{HACK} = {{ version = "0.1" }}\n'
TARGET_HEADER = f"[target.'{NATIVE_TARGET}'.dependencies]\n"


def section_headers(lines):
    """Return TOML table headers without reformatting their contents."""
    headers = []
    for index, line in enumerate(lines):
        if not re.match(r"^\s*\[.*\]\s*(?:#.*)?$", line):
            continue
        try:
            table = tomllib.loads(line)
        except tomllib.TOMLDecodeError:
            continue
        headers.append((index, table))
    return headers


def same_target(left, right):
    return re.sub(r"\s+", "", left) == re.sub(r"\s+", "", right)


def dependency_section(table):
    if table == {"dependencies": {}}:
        return ""
    for target, tables in table.get("target", {}).items():
        if tables == {"dependencies": {}}:
            return target
    return None


def remove_empty_old_target_tables(lines, wanted_target):
    """Remove obsolete generated tables, but preserve comments and dependencies."""
    headers = section_headers(lines)
    for offset in reversed(range(len(headers))):
        start, table = headers[offset]
        target = dependency_section(table)
        if not target or same_target(target, wanted_target):
            continue
        if not any(same_target(target, allowed) for allowed in (NATIVE_TARGET, DESKTOP_TARGET)):
            continue
        end = headers[offset + 1][0] if offset + 1 < len(headers) else len(lines)
        if "#" not in lines[start] and all(not line.strip() for line in lines[start + 1:end]):
            del lines[start:end]


def rewrite_manifest(text, wanted_target=NATIVE_TARGET):
    """Move only Hakari's generated line to the workspace's target table."""
    manifest = tomllib.loads(text)
    normal = manifest.get("dependencies", {}).get(HACK)
    edges = {}
    for target, tables in manifest.get("target", {}).items():
        dependency = tables.get("dependencies", {}).get(HACK)
        if dependency is None:
            continue
        if not any(same_target(target, allowed) for allowed in (NATIVE_TARGET, DESKTOP_TARGET)):
            raise ValueError(f"{HACK} must use {wanted_target}")
        edges[target] = dependency
    for kind in ("build-dependencies", "dev-dependencies"):
        if HACK in manifest.get(kind, {}) or any(
            HACK in tables.get(kind, {})
            for tables in manifest.get("target", {}).values()
        ):
            raise ValueError(f"unexpected {HACK} in {kind}")
    if normal is None and not edges:
        raise ValueError("missing workspace-hack dependency; run cargo hakari manage-deps")
    for dependency in (normal, *edges.values()):
        if dependency is not None and dependency != GENERATED_DEPENDENCY:
            raise ValueError(f"unexpected {HACK} dependency; run cargo hakari manage-deps")
    sources = {target for target in edges if not same_target(target, wanted_target)}
    if normal is not None:
        sources.add("")
    lines = text.splitlines(keepends=True)
    headers = section_headers(lines)
    remove = []
    for offset, (start, table) in enumerate(headers):
        if dependency_section(table) not in sources:
            continue
        end = headers[offset + 1][0] if offset + 1 < len(headers) else len(lines)
        for index in range(start + 1, end):
            if re.match(rf"^\s*{HACK}\s*=", lines[index]):
                if tomllib.loads(lines[index]) != {HACK: GENERATED_DEPENDENCY}:
                    raise ValueError("workspace-hack dependency must use Hakari's generated line")
                remove.append(index)
                break
        else:
            raise ValueError("cannot find Hakari's generated dependency line")
    if len(remove) != len(sources):
        raise ValueError("cannot find the workspace-hack dependency tables")
    for index in reversed(remove):
        del lines[index]
    remove_empty_old_target_tables(lines, wanted_target)

    if any(same_target(target, wanted_target) for target in edges):
        return "".join(lines)

    # Reuse an existing target table, including its comments and dependencies.
    headers = section_headers(lines)
    for start, table in headers:
        target = dependency_section(table)
        if target and same_target(target, wanted_target):
            lines.insert(start + 1, DEPENDENCY_LINE)
            return "".join(lines)

    header = f"[target.'{wanted_target}'.dependencies]\n"
    prefix = "".join(lines).rstrip("\n")
    return (prefix + "\n\n" if prefix else "") + header + DEPENDENCY_LINE


def workspace_manifests(workspace):
    """Include explicit members and Cargo's automatic path members."""
    env = dict(os.environ)
    for key in ("RUSTC", "CARGO_BUILD_RUSTC", "RUSTUP_TOOLCHAIN", "CARGO"):
        env.pop(key, None)
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"],
        cwd=workspace, env=env, text=True,
    ))
    members = set(metadata["workspace_members"])
    config = tomllib.loads((workspace / ".config/hakari.toml").read_text())
    excluded_names = {config["hakari-package"]}
    for section in ("traversal-excludes", "final-excludes"):
        excluded_names.update(config.get(section, {}).get("workspace-members", []))
    for package in sorted(metadata["packages"], key=lambda item: item["manifest_path"]):
        if package["id"] in members and package["name"] not in excluded_names:
            yield Path(package["manifest_path"]).resolve()


def main(argv=None, root=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="Report changes without writing files")
    args = parser.parse_args(argv)
    root = Path(root or Path(__file__).resolve().parents[2]).resolve()
    failures = []
    changed = 0
    for workspace, target in ((root, DESKTOP_TARGET), (root / "apps/openagents", NATIVE_TARGET)):
        for path in workspace_manifests(workspace):
            text = path.read_text()
            try:
                updated = rewrite_manifest(text, target)
            except ValueError as error:
                failures.append(f"{path.relative_to(root)}: {error}")
                continue
            if updated == text:
                continue
            if args.check:
                failures.append(f"{path.relative_to(root)}: workspace-hack needs {target}")
            else:
                path.write_text(updated)
                changed += 1
    for failure in failures:
        print(failure, file=sys.stderr)
    if failures:
        print("Run python3 scripts/dev/workspace-hack-targets.py after cargo hakari manage-deps.", file=sys.stderr)
        return 1
    if not args.check:
        print(f"Updated workspace-hack target dependencies in {changed} manifests.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
