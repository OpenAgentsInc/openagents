#!/usr/bin/env python3
"""Hash a downloaded DeepSeek-V4.1-Flash checkpoint and check it against Hugging Face.

Retained acceptance tooling for the V0 oracle (#12371). It reads the
revision's file listing from the Hugging Face tree API, hashes every local
file with SHA-256 in parallel, and checks each one: LFS files against the
LFS SHA-256 the API reports, and other files against their git blob ID
(SHA-1 of `blob <size>\\0<bytes>`). It writes a manifest and exits nonzero on
any missing, extra, or mismatched file.

    weights_manifest.py --dir DIR --out MANIFEST.json [--tree TREE.json] [--jobs N]
    weights_manifest.py --dir DIR --out MANIFEST.json --no-hf   (hash only, e.g. converted files)
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import sys
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

REPO = "deepseek-ai/DeepSeek-V4.1-Flash-1017"
REVISION = "1bae0e1088993b9b82de08f59de5be5a34e42170"
CHUNK = 64 << 20


def hf_tree() -> list[dict]:
    url = f"https://huggingface.co/api/models/{REPO}/tree/{REVISION}?recursive=true&expand=false"
    with urllib.request.urlopen(url, timeout=120) as response:
        return json.load(response)


def hash_file(path: Path) -> tuple[str, str, int]:
    sha256 = hashlib.sha256()
    size = path.stat().st_size
    blob = hashlib.sha1(f"blob {size}\0".encode())
    with path.open("rb") as handle:
        while chunk := handle.read(CHUNK):
            sha256.update(chunk)
            blob.update(chunk)
    return sha256.hexdigest(), blob.hexdigest(), size


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--dir", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--tree", type=Path, help="a saved tree listing instead of the live API")
    parser.add_argument("--no-hf", action="store_true", help="hash local files only")
    parser.add_argument("--jobs", type=int, default=min(48, os.cpu_count() or 8))
    args = parser.parse_args()

    local = sorted(
        p.relative_to(args.dir).as_posix()
        for p in args.dir.rglob("*")
        if p.is_file() and not p.relative_to(args.dir).as_posix().startswith(".cache/")
    )
    with ThreadPoolExecutor(args.jobs) as pool:
        hashes = dict(zip(local, pool.map(lambda rel: hash_file(args.dir / rel), local)))

    files, problems = [], []
    if args.no_hf:
        for rel in local:
            sha, _, size = hashes[rel]
            files.append({"path": rel, "size": size, "sha256": sha})
    else:
        tree = json.loads(args.tree.read_text()) if args.tree else hf_tree()
        expected = {e["path"]: e for e in tree if e["type"] == "file"}
        for rel in sorted(set(expected) | set(local)):
            entry, got = expected.get(rel), hashes.get(rel)
            if entry is None:
                problems.append(f"{rel}: not in the revision")
                continue
            if got is None:
                problems.append(f"{rel}: missing locally")
                continue
            sha, blob, size = got
            record = {"path": rel, "size": size, "sha256": sha}
            if size != entry["size"]:
                problems.append(f"{rel}: size {size}, revision says {entry['size']}")
            lfs = entry.get("lfs")
            if lfs:
                record["hf_lfs_sha256"] = lfs["oid"]
                record["verified"] = sha == lfs["oid"]
            else:
                record["hf_git_blob"] = entry["oid"]
                record["verified"] = blob == entry["oid"]
            if not record["verified"]:
                problems.append(f"{rel}: hash does not match the revision")
            files.append(record)

    manifest = {
        "schema": "openagents.deepseek_v41.weights_manifest.v1",
        "repo": REPO,
        "revision": REVISION,
        "verified_against_hf": not args.no_hf,
        "total_bytes": sum(f["size"] for f in files),
        "files": files,
        "problems": problems,
    }
    args.out.write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"{len(files)} files, {manifest['total_bytes']:,} bytes, {len(problems)} problems")
    for problem in problems:
        print("  " + problem, file=sys.stderr)
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
