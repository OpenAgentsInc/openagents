#!/usr/bin/env python3
"""Keep every crate we wrote at the product's version.

The product's version is Coder's: `version` in crates/coder-new/Cargo.toml.
Every first-party crate follows it: the root workspace's
`[workspace.package] version` (most crates inherit it), crates that set
their own `version`, the nested Psionic workspace, and our separate
workspaces (mc-bridge, deploy/iroh-relay/live-test). Vendored code
(`vendor/`) and test fixtures are not ours to version and are skipped.

  scripts/release/versions.py check          exit 1 when a crate differs
  scripts/release/versions.py set VERSION    set every crate to VERSION

`set` also rewrites our own entries in the Cargo.lock files, so no build
is needed. scripts/release/coder.sh runs `check` before it builds.
"""

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
PRODUCT = ROOT / "crates/coder-new/Cargo.toml"
# Workspaces whose [workspace.package] version their crates inherit.
WORKSPACES = [ROOT / "Cargo.toml", ROOT / "crates/psionic/Cargo.toml"]
# Our crates outside crates/*: their own workspaces and lock files.
EXTRA = [ROOT / "mc-bridge/Cargo.toml", ROOT / "deploy/iroh-relay/live-test/Cargo.toml"]
LOCKS = [
    ROOT / "Cargo.lock",
    ROOT / "crates/psionic/Cargo.lock",
    ROOT / "mc-bridge/Cargo.lock",
    ROOT / "deploy/iroh-relay/live-test/Cargo.lock",
]
SEMVER = re.compile(r"^\d+\.\d+\.\d+(-[0-9A-Za-z.]+)?$")


def manifests():
    """Every Cargo.toml we version: workspaces, then crates, then extras."""
    found = list(WORKSPACES)
    for path in sorted((ROOT / "crates").glob("*/Cargo.toml")):
        if path not in found:
            found.append(path)
    found += sorted((ROOT / "crates/psionic/crates").glob("*/Cargo.toml"))
    found += EXTRA
    return [path for path in found if path.is_file()]


def table_version(text, table):
    """The `version = "..."` line inside [table], as (match, value)."""
    head = re.search(rf"^\[{re.escape(table)}\]\s*$", text, re.M)
    if not head:
        return None
    end = re.search(r"^\[", text[head.end():], re.M)
    body_end = head.end() + (end.start() if end else len(text) - head.end())
    line = re.compile(r'^version\s*=\s*"([^"]*)"', re.M)
    found = line.search(text, head.end(), body_end)
    return found


def product_version():
    found = table_version(PRODUCT.read_text(), "package")
    if not found:
        sys.exit(f"{PRODUCT} has no [package] version")
    return found.group(1)


def own_versions(path):
    """The versions this manifest sets itself (not inherited)."""
    text = path.read_text()
    out = []
    for table in ("workspace.package", "package"):
        found = table_version(text, table)
        if found:
            out.append((table, found.group(1)))
    return out


def our_names():
    """Package names of every crate we version, for the lock files."""
    names = set()
    for path in manifests():
        found = re.search(r'^\[package\][^\[]*?^name\s*=\s*"([^"]+)"', path.read_text(), re.M | re.S)
        if found:
            names.add(found.group(1))
    return names


def check():
    want = product_version()
    wrong = []
    for path in manifests():
        for table, have in own_versions(path):
            if have != want:
                wrong.append(f"  {path.relative_to(ROOT)} [{table}] version = \"{have}\"")
    if wrong:
        print(f"These crates aren't at the product's version, {want} (crates/coder-new/Cargo.toml):")
        print("\n".join(wrong))
        print(f"Set them all with: scripts/release/versions.py set {want}")
        return 1
    print(f"All {len(manifests())} manifests are at {want}.")
    return 0


def set_version(version):
    if not SEMVER.match(version):
        sys.exit(f"{version} is not a version like 1.0.0 or 1.0.0-rc.13")
    for path in manifests():
        text = path.read_text()
        for table in ("workspace.package", "package"):
            found = table_version(text, table)
            if found and found.group(1) != version:
                text = text[: found.start(1)] + version + text[found.end(1):]
        # Path dependencies that pin one of our crates' versions.
        text = re.sub(
            r'(\{[^}\n]*?version\s*=\s*")[^"]*("[^}\n]*?path\s*=\s*"\.\./[^"]+"[^}\n]*\})',
            rf"\g<1>{version}\g<2>",
            text,
        )
        path.write_text(text)
    names = our_names()
    for lock in LOCKS:
        if not lock.is_file():
            continue
        text = lock.read_text()
        def bump(found):
            if found.group(1) in names and "\nsource =" not in found.group(0):
                return re.sub(r'version = "[^"]*"', f'version = "{version}"', found.group(0), count=1)
            return found.group(0)
        text = re.sub(r'\[\[package\]\]\nname = "([^"]+)"\nversion = "[^"]*"(?:\nsource = [^\n]*)?', bump, text)
        lock.write_text(text)
    print(f"Set {len(manifests())} manifests to {version}.")
    return check()


def main(argv):
    if argv[1:2] == ["check"]:
        return check()
    if argv[1:2] == ["set"] and len(argv) == 3:
        return set_version(argv[2])
    print(__doc__.strip())
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv))
