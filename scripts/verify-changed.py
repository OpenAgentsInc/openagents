#!/usr/bin/env python3
"""Resolve changed paths into the Cargo packages a scoped gate must cover.

Reads the paths that differ from a ref (committed) plus uncommitted edits,
maps `crates/<name>/` paths to package names, maps repository data
directories to the crate that loads them, and reports workspace-wide files
that invalidate crate scoping entirely.

The plan also names:

- `nested`: changes inside workspaces the root `Cargo.toml` excludes
  (`crates/psionic`, `crates/openagents-mobile`). Their packages are not
  root members, so they are checked with `--manifest-path`, never `-p`.
- `dependents`: root members that depend directly on a changed crate, so a
  change to a shared crate also checks its consumers.
- `lock_packages`: packages whose `Cargo.lock` entry changed. The root
  members that use them (the nearest members in the resolve graph) join
  `crates`, so a dependency bump never yields an empty plan.

Root `Cargo.toml` changes that only touch `[workspace.dependencies]` map to
the members using those entries; any other root manifest change still sets
`workspace`. Changes under a path-patched vendored package (`[patch]` in the
root manifest) map to the members that use it.

The output is JSON; `scripts/verify-rust.sh --print --changed` shows the
commands the plan becomes without running them.
"""
import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

# Repository files whose change can affect every crate's build or checks.
# `Cargo.toml` and `Cargo.lock` are refined below when a base is readable.
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
# Files at a nested workspace's root that affect every package in it.
NESTED_WORKSPACE_FILES = {
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    "rust-toolchain",
    "rustfmt.toml",
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


def base_reader(root: Path, ref: str):
    """A function returning a path's text at the merge base, or None."""
    merge_base = subprocess.run(
        ["git", "-C", str(root), "merge-base", ref, "HEAD"],
        capture_output=True, text=True)
    base = merge_base.stdout.strip() if merge_base.returncode == 0 else ref

    def read(path: str):
        shown = subprocess.run(
            ["git", "-C", str(root), "show", f"{base}:{path}"],
            capture_output=True, text=True)
        return shown.stdout if shown.returncode == 0 else None

    return read


# --- Minimal TOML reading (the gate runs on the macOS python3, which has
# no tomllib). Only the shapes Cargo manifests use here are handled.

def sections(text: str) -> dict:
    """Header -> list of lines, for a TOML document. `""` is the preamble."""
    out: dict = {"": []}
    current = ""
    for line in text.splitlines():
        header = re.match(r"^\s*\[\[?([^\]]+)\]\]?\s*(#.*)?$", line)
        if header:
            current = header.group(1).strip()
            out.setdefault(current, [])
            continue
        out[current].append(line)
    return out


def package_name(manifest: Path):
    """The `[package] name` a manifest declares, or None when it has none."""
    if not manifest.exists():
        return None
    body = sections(manifest.read_text()).get("package")
    if body is None:
        return None
    for line in body:
        found = re.match(r'^\s*name\s*=\s*"([^"]+)"', line)
        if found:
            return found.group(1)
    return None


def excluded_dirs(root: Path) -> list[str]:
    """`[workspace].exclude` entries of the root manifest."""
    manifest = root / "Cargo.toml"
    if not manifest.exists():
        return []
    text = "\n".join(sections(manifest.read_text()).get("workspace", []))
    found = re.search(r"exclude\s*=\s*\[(.*?)\]", text, re.S)
    if not found:
        return []
    body = re.sub(r"#[^\n]*", "", found.group(1))
    return [entry.strip("/") for entry in re.findall(r'"([^"]+)"', body)]


def workspace_dependency_entries(text: str) -> dict:
    """`[workspace.dependencies]` key -> its normalized definition."""
    entries = {}
    for line in sections(text).get("workspace.dependencies", []):
        stripped = re.sub(r"\s+#.*$", "", line).strip()
        if not stripped or stripped.startswith("#"):
            continue
        key = re.match(r'^"?([A-Za-z0-9_.-]+)"?\s*=', stripped)
        if key:
            entries[key.group(1)] = stripped
    return entries


def manifest_outside_dependencies(text: str) -> str:
    """The manifest with `[workspace.dependencies]` and comments removed."""
    kept = []
    for header, body in sections(text).items():
        if header == "workspace.dependencies":
            continue
        kept.append(header)
        kept.extend(
            stripped for stripped in
            (re.sub(r"\s+#.*$", "", line).strip() for line in body)
            if stripped and not stripped.startswith("#"))
    return "\n".join(kept)


def lock_entries(text: str) -> dict:
    """`Cargo.lock` package name -> set of its normalized entries."""
    entries: dict = {}
    for block in re.split(r"^\[\[package\]\]\s*$", text, flags=re.M)[1:]:
        name = re.search(r'^name\s*=\s*"([^"]+)"', block, re.M)
        if name:
            entries.setdefault(name.group(1), set()).add(block.strip())
    return entries


# --- Cargo metadata -------------------------------------------------------

class Metadata:
    """Lazily loaded `cargo metadata` for the root workspace.

    Tests inject `direct` and `full` dictionaries in the cargo output shape.
    A failure to run cargo degrades to no dependency information.
    """

    def __init__(self, root: Path, direct=None, full=None):
        self.root = root
        self._direct = direct
        self._full = full

    def _load(self, *flags: str):
        try:
            result = subprocess.run(
                ["cargo", "metadata", "--format-version", "1", "--offline",
                 *flags],
                cwd=self.root, capture_output=True, text=True)
        except OSError:
            return {}
        if result.returncode != 0:
            return {}
        return json.loads(result.stdout)

    def direct(self) -> dict:
        if self._direct is None:
            self._direct = self._load("--no-deps")
        return self._direct

    def full(self) -> dict:
        if self._full is None:
            self._full = self._load()
        return self._full

    def members(self) -> set:
        data = self.direct()
        ids = set(data.get("workspace_members", []))
        return {p["name"] for p in data.get("packages", []) if p["id"] in ids}

    def direct_dependents(self, names: set) -> set:
        """Members that list one of `names` as a dependency."""
        data = self.direct()
        ids = set(data.get("workspace_members", []))
        out = set()
        for package in data.get("packages", []):
            if package["id"] not in ids or package["name"] in names:
                continue
            if any(dep["name"] in names for dep in package.get("dependencies", [])):
                out.add(package["name"])
        return out

    def members_using_keys(self, keys: set) -> set:
        """Members whose manifest names one of `keys` (rename-aware)."""
        data = self.direct()
        ids = set(data.get("workspace_members", []))
        out = set()
        for package in data.get("packages", []):
            if package["id"] not in ids:
                continue
            for dep in package.get("dependencies", []):
                if (dep.get("rename") or dep["name"]) in keys or dep["name"] in keys:
                    out.add(package["name"])
        return out

    def nearest_members(self, names: set) -> set:
        """Members reached first walking reverse dependency edges from the
        packages named `names`: the members whose own dependency set moved.
        """
        data = self.full()
        resolve = data.get("resolve") or {}
        nodes = resolve.get("nodes", [])
        if not nodes:
            return set()
        member_ids = set(data.get("workspace_members", []))
        name_of = {p["id"]: p["name"] for p in data.get("packages", [])}
        reverse: dict = {}
        for node in nodes:
            for dep in node.get("deps", []):
                reverse.setdefault(dep["pkg"], set()).add(node["id"])
        frontier = [pid for pid, name in name_of.items() if name in names]
        seen = set(frontier)
        found = set()
        while frontier:
            pid = frontier.pop()
            for parent in reverse.get(pid, ()):
                if parent in seen:
                    continue
                seen.add(parent)
                if parent in member_ids:
                    found.add(name_of[parent])
                else:
                    frontier.append(parent)
        for pid in seen:
            if pid in member_ids and name_of[pid] in names:
                found.add(name_of[pid])
        return found

    def path_package_of(self, path: str):
        """The non-member path package (a `[patch]`ed vendored crate) whose
        directory holds `path`, or None.
        """
        data = self.full()
        member_ids = set(data.get("workspace_members", []))
        best = None
        for package in data.get("packages", []):
            if package["id"] in member_ids or package.get("source"):
                continue
            try:
                directory = Path(package["manifest_path"]).parent.resolve()
                relative = directory.relative_to(self.root.resolve()).as_posix()
            except ValueError:
                continue
            if path.startswith(relative + "/") and (
                    best is None or len(relative) > len(best[0])):
                best = (relative, package["name"])
        return best[1] if best else None


# --- Resolution -----------------------------------------------------------

def package_of(root: Path, name: str):
    """The `[package] name` a crates/<name> directory builds, or None."""
    return package_name(root / "crates" / name / "Cargo.toml")


def nested_package(root: Path, nested_root: str, path: str):
    """The package in a nested workspace that owns `path`, or None."""
    directory = (root / path).parent
    stop = root / nested_root
    while True:
        name = package_name(directory / "Cargo.toml")
        if name:
            return name
        if directory == stop or stop not in directory.parents:
            return None
        directory = directory.parent


def resolve(root: Path, paths: list[str], read_base=None, metadata=None) -> dict:
    """The scoped plan for `paths`.

    `read_base(path)` returns a file's text at the base (None when absent);
    without it, root `Cargo.toml`/`Cargo.lock` changes set `workspace`.
    `metadata` is a `Metadata`; by default it runs cargo on demand.
    """
    metadata = metadata or Metadata(root)
    excluded = excluded_dirs(root)
    crates: set = set()
    workspace = False
    uncovered: list = []
    nested: dict = {}
    lock_packages: set = set()
    for path in paths:
        nested_root = next(
            (d for d in excluded if path == d or path.startswith(d + "/")), None)
        if nested_root is not None:
            manifest = f"{nested_root}/Cargo.toml"
            if (root / manifest).exists():
                entry = nested.setdefault(
                    manifest, {"manifest": manifest, "packages": set(),
                               "workspace": False})
                rest = path[len(nested_root) + 1:]
                if rest in NESTED_WORKSPACE_FILES:
                    entry["workspace"] = True
                    continue
                package = nested_package(root, nested_root, path)
                if package:
                    entry["packages"].add(package)
                else:
                    uncovered.append(path)
                continue
            # A vendored directory the root patches in by path.
            patched = metadata.path_package_of(path)
            if patched:
                lock_packages.add(patched)
            else:
                uncovered.append(path)
            continue
        if path.startswith("crates/"):
            parts = path.split("/")
            if len(parts) > 2:
                package = package_of(root, parts[1])
                if package:
                    crates.add(package)
                else:
                    uncovered.append(path)
            continue
        matched_dir = next(
            (pkg for prefix, pkg in DATA_DIRS.items() if path.startswith(prefix)),
            None,
        )
        if matched_dir:
            crates.add(matched_dir)
        elif path == "Cargo.lock" and read_base is not None:
            before = read_base(path)
            current = root / path
            if before is None or not current.exists():
                workspace = True
                continue
            old, new = lock_entries(before), lock_entries(current.read_text())
            moved = {name for name in set(old) | set(new)
                     if old.get(name) != new.get(name)}
            lock_packages |= moved
        elif path == "Cargo.toml" and read_base is not None:
            before = read_base(path)
            current = root / path
            if before is None or not current.exists():
                workspace = True
                continue
            text = current.read_text()
            if manifest_outside_dependencies(before) != manifest_outside_dependencies(text):
                workspace = True
            old = workspace_dependency_entries(before)
            new = workspace_dependency_entries(text)
            keys = {k for k in set(old) | set(new) if old.get(k) != new.get(k)}
            crates |= metadata.members_using_keys(keys)
        elif path in WORKSPACE_FILES or any(
            path.startswith(prefix) for prefix in (".cargo/",)
        ):
            workspace = True
        else:
            uncovered.append(path)
    if lock_packages:
        users = metadata.nearest_members(lock_packages)
        if users:
            crates |= users
        else:
            # Never an empty plan for a dependency change cargo could not map.
            workspace = True
    dependents = metadata.direct_dependents(crates) - crates if crates else set()
    return {
        "crates": sorted(crates),
        "workspace": workspace,
        "uncovered": uncovered,
        "nested": [
            {"manifest": e["manifest"], "packages": sorted(e["packages"]),
             "workspace": e["workspace"]}
            for _, e in sorted(nested.items())
        ],
        "dependents": sorted(dependents),
        "lock_packages": sorted(lock_packages),
    }


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--ref", default="origin/main",
                        help="the base ref paths diff against")
    parser.add_argument("--committed-only", action="store_true",
                        help="ignore uncommitted edits")
    parser.add_argument("--root", default=".",
                        help="repository root")
    parser.add_argument("--paths", nargs="+", metavar="PATH",
                        help="resolve these paths instead of the diff "
                             "(a dry run for checking the mapping)")
    args = parser.parse_args()
    root = Path(args.root).resolve()
    if args.paths:
        paths = sorted(set(args.paths))
    else:
        paths = changed_paths(root, args.ref, not args.committed_only)
    plan = resolve(root, paths, read_base=base_reader(root, args.ref))
    plan["ref"] = args.ref
    plan["changed"] = paths
    print(json.dumps(plan, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
