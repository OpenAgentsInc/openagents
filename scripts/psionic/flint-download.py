#!/usr/bin/env python3
"""Download and verify the pinned Flint BF16 release on a qualification host."""
import argparse
import concurrent.futures
import hashlib
import json
from pathlib import Path
from huggingface_hub import hf_hub_download

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("directory", type=Path)
args = parser.parse_args()
repo = Path(__file__).resolve().parents[2]
artifact = json.loads((repo / "crates/psionic/crates/psionic-flint/testdata/artifact.json").read_text())
args.directory.mkdir(parents=True, exist_ok=True)


def get(file):
    path = Path(hf_hub_download(artifact["model"], file["path"], revision=artifact["revision"], local_dir=args.directory))
    assert path.stat().st_size == file["size"], path
    digest = hashlib.file_digest(path.open("rb"), "sha256").hexdigest()
    assert digest == file["sha256"], f"checksum mismatch: {path}"
    print(f"verified {file['path']} {digest}", flush=True)
    return dict(file, verified=True)


with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
    files = list(pool.map(get, artifact["files"]))
for name in ["LICENSE", "README.md"]:
    hf_hub_download(artifact["model"], name, revision=artifact["revision"], local_dir=args.directory)
(args.directory / "VERIFIED.json").write_text(json.dumps(dict(artifact, files=files), indent=2) + "\n")
