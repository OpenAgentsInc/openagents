#!/usr/bin/env python3
"""Verify the recorded source snapshot without network access."""
import argparse
import hashlib
import json
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--source-root", type=Path)
args = parser.parse_args()
root = Path(__file__).resolve().parent
manifest = json.loads((root / "provenance.json").read_text())
for row in manifest["files"]:
    path = root / row["destination"]
    assert path.resolve().is_relative_to(root), path
    assert hashlib.sha256(path.read_bytes()).hexdigest() == row["vendored_sha256"], path
    if row["source_sha256"] != row["vendored_sha256"]:
        assert row.get("adaptation"), path
    if args.source_root:
        source = args.source_root / row["source"]
        assert hashlib.sha256(source.read_bytes()).hexdigest() == row["source_sha256"], source
recorded = {row["destination"] for row in manifest["files"]}
actual = {str(p.relative_to(root)) for p in (root / "vendor").rglob("*") if p.is_file()}
assert recorded == actual, (sorted(actual - recorded), sorted(recorded - actual))
print(f"Verified {len(recorded)} retained files at {manifest['source_commit']}")
