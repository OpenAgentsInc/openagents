#!/usr/bin/env python3
"""Refuse iOS bundles whose native libraries depend on the build machine."""

from __future__ import annotations

import argparse
import json
import plistlib
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path, PurePosixPath


class BundleError(ValueError):
    """The bundle cannot load independently of the build machine."""


@dataclass(frozen=True)
class Image:
    dependencies: tuple[str, ...]
    rpaths: tuple[str, ...]


LOAD_COMMANDS = {
    "LC_LOAD_DYLIB", "LC_LOAD_WEAK_DYLIB", "LC_REEXPORT_DYLIB",
    "LC_LOAD_UPWARD_DYLIB", "LC_LAZY_LOAD_DYLIB",
}
MACHO_MAGIC = {
    b"\xfe\xed\xfa\xce", b"\xce\xfa\xed\xfe",
    b"\xfe\xed\xfa\xcf", b"\xcf\xfa\xed\xfe",
    b"\xca\xfe\xba\xbe", b"\xbe\xba\xfe\xca",
    b"\xca\xfe\xba\xbf", b"\xbf\xba\xfe\xca",
}


def parse_load_commands(output: str) -> Image:
    """Read dependencies, excluding a dylib's own LC_ID_DYLIB identity."""
    dependencies: list[str] = []
    rpaths: list[str] = []
    command = ""
    for line in output.splitlines():
        text = line.strip()
        if text.startswith("cmd "):
            command = text[4:]
        elif command in LOAD_COMMANDS and text.startswith("name "):
            dependencies.append(re.sub(r" \(offset \d+\)$", "", text[5:]))
        elif command == "LC_RPATH" and text.startswith("path "):
            rpaths.append(re.sub(r" \(offset \d+\)$", "", text[5:]))
    return Image(tuple(dict.fromkeys(dependencies)), tuple(dict.fromkeys(rpaths)))


def command_output(arguments: list[str]) -> str:
    result = subprocess.run(arguments, capture_output=True, text=True, check=False)
    if result.returncode:
        raise BundleError(f"{' '.join(arguments[:2])} failed: {result.stderr.strip()}")
    return result.stdout


def inspect_macho(path: Path) -> Image:
    return parse_load_commands(command_output(["/usr/bin/otool", "-l", str(path)]))


def verify_signature(path: Path) -> None:
    command_output(["/usr/bin/codesign", "--verify", "--deep", "--strict", str(path)])


def is_system_path(value: str) -> bool:
    # Normalize traversal before matching the only device-provided locations.
    path = PurePosixPath(value)
    return ".." not in path.parts and (
        value.startswith("/System/Library/") or value.startswith("/usr/lib/")
    )


def inside(path: Path, bundle: Path) -> bool:
    return path.resolve().is_relative_to(bundle)


def expand_path(value: str, loader: Path, executable: Path, bundle: Path) -> Path:
    if value.startswith("@loader_path/"):
        path = loader.parent / value.removeprefix("@loader_path/")
    elif value.startswith("@executable_path/"):
        path = executable.parent / value.removeprefix("@executable_path/")
    elif is_system_path(value):
        return Path(value)
    else:
        raise BundleError(f"Nonportable library search path: {value}")
    if not inside(path, bundle):
        raise BundleError(f"Library search path escapes the app bundle: {value}")
    return path.resolve()


def resolve_dependency(
    dependency: str, loader: Path, executable: Path, bundle: Path,
    runpaths: tuple[Path, ...], images: dict[Path, Image],
) -> Path | None:
    if PurePosixPath(dependency).name == "libcoder_mobile.dylib":
        raise BundleError("Coder must link libcoder_mobile.a; its dynamic library is not a runtime dependency.")
    if is_system_path(dependency):
        return None
    if dependency.startswith("@rpath/"):
        relative = dependency.removeprefix("@rpath/")
        candidates = [directory / relative for directory in runpaths]
    elif dependency.startswith(("@loader_path/", "@executable_path/")):
        candidates = [expand_path(dependency, loader, executable, bundle)]
    else:
        raise BundleError(f"Nonportable library dependency in {loader.name}: {dependency}")
    for candidate in candidates:
        # Resolve symlinks and traversal before accepting an embedded image.
        resolved = candidate.resolve()
        if inside(resolved, bundle) and resolved in images:
            return resolved
    raise BundleError(f"Missing bundled library in {loader.name}: {dependency}")


def verify_bundle(bundle: Path, inspect=inspect_macho, signature=verify_signature) -> dict:
    bundle = bundle.resolve()
    if not bundle.is_dir() or bundle.suffix != ".app":
        raise BundleError("Provide a built .app bundle.")
    with (bundle / "Info.plist").open("rb") as source:
        info = plistlib.load(source)
    name = info.get("CFBundleExecutable", "")
    if not name or Path(name).name != name:
        raise BundleError("The bundle has no valid executable name.")
    executable = (bundle / name).resolve()
    images: dict[Path, Image] = {}
    for path in bundle.rglob("*"):
        if path.is_symlink() and not inside(path, bundle):
            raise BundleError(f"Bundle symlink escapes the app: {path.relative_to(bundle)}")
        if path.is_file():
            with path.open("rb") as source:
                magic = source.read(4)
            if magic in MACHO_MAGIC:
                images[path.resolve()] = inspect(path)
    if executable not in images:
        raise BundleError("The app executable is missing or is not Mach-O.")
    signature(bundle)
    for path in images:
        signature(path)

    visited: set[tuple[Path, tuple[Path, ...]]] = set()
    checked: set[Path] = set()

    def visit(path: Path, inherited: tuple[Path, ...]) -> None:
        metadata = images[path]
        local = tuple(expand_path(value, path, executable, bundle) for value in metadata.rpaths)
        runpaths = tuple(dict.fromkeys((*local, *inherited)))
        key = (path, runpaths)
        if key in visited:
            return
        visited.add(key)
        checked.add(path)
        for dependency in metadata.dependencies:
            target = resolve_dependency(dependency, path, executable, bundle, runpaths, images)
            if target is not None:
                visit(target, runpaths)

    visit(executable, ())
    # Check embedded plugins and unused libraries too, before they can be loaded.
    executable_paths = tuple(
        expand_path(value, executable, executable, bundle) for value in images[executable].rpaths
    )
    for path in images:
        if path not in checked:
            visit(path, executable_paths)
    return {
        "schema": "coder.ios.bundle-verification.v1",
        "bundle_id": info.get("CFBundleIdentifier"),
        "version": info.get("CFBundleShortVersionString"),
        "build": info.get("CFBundleVersion"),
        "status": "passed",
        "images": [
            {"path": str(path.relative_to(bundle)), "dependencies": list(images[path].dependencies)}
            for path in sorted(images)
        ],
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("bundle", type=Path)
    arguments = parser.parse_args()
    try:
        print(json.dumps(verify_bundle(arguments.bundle), indent=2))
    except (BundleError, OSError, plistlib.InvalidFileException) as error:
        print(f"iOS bundle verification failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
