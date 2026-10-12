#!/usr/bin/env python3
"""Merge sharded run archives into one results directory (#11309).

    merge_shards.py TAG [--prefix gs://openagents-bench-artifacts/briefed-ab-11309]
                        [--from DIR_OR_TGZ ...] [--dry-run]

remote/run-shard2.sh archives the whole `.work/results/TAG` directory of
each shard machine to `PREFIX/TAG-nN-shardI/TAG-nN-shardI.tgz`; a machine
that was restored from an earlier partial archive carries those trials
too. This pulls every such archive (plus any `--from` archives or result
directories, such as the `partial-*` ones), and writes the union into
`.work/results/TAG`.

A trial directory counts only when it holds `result.json`. When two
sources hold the same trial, the copies must be the same run: the merge
keeps the one with the most judge files and reports a conflict when their
`result.json` differ (that would mean a trial ran twice, which the frozen
plans forbid, so it is shown, never silently chosen). Nothing already in
`.work/results/TAG` is deleted; existing judge files are kept.
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
import tarfile
import tempfile
from pathlib import Path

from common import WORK

PREFIX = "gs://openagents-bench-artifacts/briefed-ab-11309"
JUDGES = ("judge.json", "judge2.json")


def gcs_archives(prefix: str, tag: str) -> list[str]:
    proc = subprocess.run(["gcloud", "storage", "ls", f"{prefix}/{tag}-n*/{tag}-n*.tgz"],
                          capture_output=True, text=True)
    return sorted(u for u in proc.stdout.split() if u.endswith(".tgz") and not u.endswith("-validation.tgz"))


def unpack(src: str, into: Path) -> Path:
    """Return a directory holding TAG/... for one source."""
    if src.startswith("gs://"):
        local = into / Path(src).name
        subprocess.run(["gcloud", "storage", "cp", "-q", src, str(local)], check=True)
        src = str(local)
    p = Path(src)
    if p.is_dir():
        return p
    out = into / (p.name + ".d")
    out.mkdir()
    with tarfile.open(p) as t:
        try:
            t.extractall(out, filter="data")
        except TypeError:  # Python < 3.12
            t.extractall(out)
    return out


def trials(root: Path, tag: str) -> dict[str, Path]:
    base = root / tag if (root / tag).is_dir() else root
    return {d.name: d for d in base.iterdir() if d.is_dir() and (d / "result.json").exists()}


def score(d: Path) -> int:
    return sum((d / j).exists() for j in JUDGES)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("tag")
    ap.add_argument("--prefix", default=PREFIX)
    ap.add_argument("--from", dest="extra", action="append", default=[])
    ap.add_argument("--no-gcs", action="store_true")
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()

    dest = WORK / "results" / args.tag
    sources = ([] if args.no_gcs else gcs_archives(args.prefix, args.tag)) + args.extra
    if not sources:
        sys.exit(f"no archives for {args.tag}")
    best: dict[str, Path] = {}
    conflicts: list[str] = []
    with tempfile.TemporaryDirectory() as tmp:
        for src in sources:
            found = trials(unpack(src, Path(tmp)), args.tag)
            print(f"{src}: {len(found)} trials")
            for name, d in found.items():
                if name in best:
                    a, b = json.loads((best[name] / "result.json").read_text()), json.loads((d / "result.json").read_text())
                    if a != b:
                        conflicts.append(name)
                    if score(d) <= score(best[name]):
                        continue
                best[name] = d
        existing = trials(dest, args.tag) if dest.is_dir() else {}
        added = 0
        for name, d in sorted(best.items()):
            if name in existing:
                for j in JUDGES:  # bring judge files the local copy lacks
                    if (d / j).exists() and not (existing[name] / j).exists() and not args.dry_run:
                        shutil.copy2(d / j, existing[name] / j)
                continue
            added += 1
            if not args.dry_run:
                shutil.copytree(d, dest / name)
    print(f"{args.tag}: {len(best)} trials across {len(sources)} sources; {added} added to {dest}; "
          f"{len(existing)} were there")
    if conflicts:
        print(f"CONFLICT: {len(conflicts)} trials differ between sources: {', '.join(sorted(set(conflicts))[:20])}")


if __name__ == "__main__":
    main()
