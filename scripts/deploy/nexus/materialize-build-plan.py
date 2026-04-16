#!/usr/bin/env python3

import json
import shutil
import subprocess
import sys
from pathlib import Path


def run_metadata(manifest_path: Path) -> dict:
    result = subprocess.run(
        [
            "cargo",
            "metadata",
            "--manifest-path",
            str(manifest_path),
            "--format-version",
            "1",
            "--no-deps",
        ],
        check=True,
        capture_output=True,
        text=True,
    )
    return json.loads(result.stdout)


def copy_path(src: Path, dest: Path) -> None:
    reset_path(dest)
    dest.parent.mkdir(parents=True, exist_ok=True)
    if src.is_dir():
        shutil.copytree(src, dest)
    else:
        shutil.copy2(src, dest)


def write_text(path: Path, body: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(body)


def reset_path(path: Path) -> None:
    if path.exists() or path.is_symlink():
        if path.is_dir() and not path.is_symlink():
            shutil.rmtree(path)
        else:
            path.unlink()


def rel_to(path: Path, root: Path) -> str:
    return path.relative_to(root).as_posix()


def placeholder_manifest(package: dict) -> str:
    lines = [
        "[package]",
        f'name = "{package["name"]}"',
        f'version = "{package["version"]}"',
        f'edition = "{package["edition"]}"',
        "publish = false",
        "",
        "[lints]",
        "workspace = true",
        "",
        "[lib]",
        'path = "src/lib.rs"',
        "",
    ]
    return "\n".join(lines)


def placeholder_body(target: dict) -> str:
    kinds = set(target.get("kind", []))
    if "proc-macro" in kinds:
        return (
            "extern crate proc_macro;\n"
            "use proc_macro::TokenStream;\n\n"
            "#[proc_macro]\n"
            "pub fn placeholder(_input: TokenStream) -> TokenStream {\n"
            "    TokenStream::new()\n"
            "}\n"
        )
    if "bin" in kinds:
        return "fn main() {}\n"
    return "pub fn placeholder() {}\n"


def materialize_real_crate(source_root: Path, package: dict, destination_root: Path) -> None:
    reset_path(destination_root)
    manifest_path = Path(package["manifest_path"])
    copy_path(manifest_path, destination_root / "Cargo.toml")

    for target in package.get("targets", []):
        source_path = Path(target["src_path"])
        relative_path = source_path.relative_to(source_root)
        if "custom-build" in set(target.get("kind", [])):
            copy_path(source_path, destination_root / relative_path)
            continue
        if not source_path.exists():
            continue
        write_text(destination_root / relative_path, placeholder_body(target))

    build_script_path = source_root / "build.rs"
    if build_script_path.exists():
        copy_path(build_script_path, destination_root / "build.rs")

    for supplemental_dir in ("proto",):
        supplemental_path = source_root / supplemental_dir
        if supplemental_path.exists():
            copy_path(supplemental_path, destination_root / supplemental_dir)


def main() -> None:
    if len(sys.argv) < 4:
        raise SystemExit(
            "usage: materialize-build-plan.py <root-dir> <context-dir> <real-workspace-path>..."
        )

    root_dir = Path(sys.argv[1]).resolve()
    context_dir = Path(sys.argv[2]).resolve()
    real_workspace_paths = set(sys.argv[3:])
    extra_real_paths = {"third_party/nostr-rs-relay"}
    all_real_paths = real_workspace_paths | extra_real_paths
    build_plan_dir = context_dir / ".nexus-build-plan"

    root_metadata = run_metadata(root_dir / "Cargo.toml")
    workspace_member_ids = set(root_metadata["workspace_members"])
    workspace_packages = [
        package for package in root_metadata["packages"] if package["id"] in workspace_member_ids
    ]
    workspace_packages_by_path = {
        rel_to(Path(package["manifest_path"]).parent, root_dir): package for package in workspace_packages
    }

    for member_path, package in workspace_packages_by_path.items():
        member_dir = context_dir / member_path
        if member_path in all_real_paths:
            continue
        reset_path(member_dir)
        member_dir.mkdir(parents=True, exist_ok=True)
        write_text(member_dir / "Cargo.toml", placeholder_manifest(package))
        write_text(member_dir / "src/lib.rs", "pub fn placeholder() {}\n")

    if build_plan_dir.exists():
        shutil.rmtree(build_plan_dir)
    build_plan_dir.mkdir(parents=True, exist_ok=True)

    for root_relative in ("Cargo.toml", "Cargo.lock", ".cargo", "proto", "scripts/dev/protocw"):
        source_path = context_dir / root_relative
        if source_path.exists():
            copy_path(source_path, build_plan_dir / root_relative)

    for member_path, package in workspace_packages_by_path.items():
        destination = build_plan_dir / member_path
        if member_path in all_real_paths:
            materialize_real_crate(root_dir / member_path, package, destination)
        else:
            copy_path(context_dir / member_path, destination)

    for extra_path in extra_real_paths:
        if extra_path in workspace_packages_by_path:
            continue
        extra_manifest = root_dir / extra_path / "Cargo.toml"
        if not extra_manifest.exists():
            continue
        extra_metadata = run_metadata(extra_manifest)
        extra_package = None
        for package in extra_metadata["packages"]:
            if Path(package["manifest_path"]).resolve() == extra_manifest.resolve():
                extra_package = package
                break
        if extra_package is None:
            continue
        materialize_real_crate(root_dir / extra_path, extra_package, build_plan_dir / extra_path)


if __name__ == "__main__":
    main()
