#!/usr/bin/env python3
"""Check that every `*_FIXTURE_PATH` constant in Psionic resolves to a file.

`crates/psionic` was imported as the dependency closure of
`psionic-openai-server`, but many fixture files those crates read were not
imported. Tests that load a missing fixture fail by construction, which hides
real failures and blocks adding Psionic to any gate. This script makes that
state visible and ratchets it: the recorded allowlist may only shrink.

Each `const NAME_FIXTURE_PATH: &str = "path"` is resolved relative to
`crates/psionic/` (the nested workspace root, which is what
`env!("CARGO_MANIFEST_DIR")`-based loaders join against). A path that does not
exist is reported unless its constant name is in the allowlist.

Usage:
  scripts/dev/check-psionic-fixtures.py            # report and fail on new misses
  scripts/dev/check-psionic-fixtures.py --list     # print every missing path
  scripts/dev/check-psionic-fixtures.py --write-allowlist
"""
import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
PSIONIC = ROOT / "crates" / "psionic"
ALLOWLIST = ROOT / "scripts" / "dev" / "psionic-missing-fixtures.txt"

CONST = re.compile(
    r'const\s+([A-Z0-9_]*_FIXTURE_PATH[A-Z0-9_]*)\s*:\s*&str\s*=\s*"([^"]+)"')


def fixture_consts() -> list[tuple[Path, str, str]]:
    """Every fixture-path constant: (source file, name, path)."""
    found = []
    for source in sorted(PSIONIC.rglob("*.rs")):
        text = source.read_text(errors="replace")
        for match in CONST.finditer(text):
            found.append((source, match.group(1), match.group(2)))
    return found


def missing() -> list[tuple[str, str]]:
    """Sorted (constant name, path) pairs whose file does not exist."""
    out = []
    for _source, name, path in fixture_consts():
        if not (PSIONIC / path).exists():
            out.append((name, path))
    return sorted(out)


def read_allowlist() -> set[str]:
    if not ALLOWLIST.exists():
        return set()
    names = set()
    for line in ALLOWLIST.read_text().splitlines():
        line = line.split("#", 1)[0].strip()
        if line:
            names.add(line)
    return names


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--list", action="store_true",
                        help="print every missing path, then exit 0")
    parser.add_argument("--write-allowlist", action="store_true",
                        help="record the current missing set as the allowlist")
    args = parser.parse_args()

    gone = missing()

    if args.list:
        for name, path in gone:
            print(f"{name}\t{path}")
        return 0

    if args.write_allowlist:
        header = (
            "# Psionic fixture constants whose files were never imported.\n"
            "# Regenerate with scripts/dev/check-psionic-fixtures.py "
            "--write-allowlist.\n"
            "# This list may only shrink: import the fixture or delete the\n"
            "# constant and its test.\n")
        ALLOWLIST.write_text(header + "".join(f"{n}\n" for n, _ in gone))
        print(f"wrote {len(gone)} names to {ALLOWLIST.relative_to(ROOT)}")
        return 0

    allowed = read_allowlist()
    current = {name for name, _ in gone}
    new = sorted(current - allowed)
    fixed = sorted(allowed - current)

    total = len(fixture_consts())
    print(f"psionic fixture paths: {total} total, "
          f"{total - len(gone)} resolve, {len(gone)} missing")

    if fixed:
        print(f"\n{len(fixed)} allowlisted fixtures now resolve; remove them "
              f"from {ALLOWLIST.relative_to(ROOT)}:")
        for name in fixed:
            print(f"  {name}")

    if new:
        print(f"\n{len(new)} fixture paths do not resolve and are not "
              f"allowlisted:")
        for name, path in gone:
            if name in new:
                print(f"  {name} -> {path}")
        print("\nImport the fixture, or delete the constant and the test "
              "that reads it.")
        return 1

    print("no new missing fixtures")
    return 0


if __name__ == "__main__":
    sys.exit(main())
