#!/usr/bin/env python3
"""Digest, compare, and archive the private Modular Medieval Town export.

    medieval_town_archive.py digest EXPORT_DIR
    medieval_town_archive.py compare EXPORT_DIR OTHER_EXPORT_DIR
    medieval_town_archive.py archive EXPORT_DIR [--source CONTENT_DIR] [--dry-run]

`digest` writes `digests.json` beside the export: the SHA-256 and length of
every exported mesh, texture, material table, and map, keyed by its path
inside the export. `compare` digests two exports and lists every file that
differs, which is how two runs of `medieval_town_export.py` are shown to
be deterministic. `archive` uploads the vendor's `Content` directory and the
export's `digests.json` to the private bucket under
`vendor/medieval-town/`, with the owner's own `gcloud` login, as
`verse-private add` archives a character's vendor files
(docs/verse/private-assets.md).

Nothing this script reads or writes belongs in the repository: it refuses
an export or source inside the checkout. The digests are private too; they
stay beside the export and in the bucket.
"""

import argparse
import hashlib
import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent
BUCKET = "openagentsgemini-verse-private-assets"
PREFIX = "vendor/medieval-town"
DEFAULT_SOURCE = Path(
    "/Users/Shared/UnrealEngine/Launcher/VaultCache/ModularMcb8bfaae7940V2/data/Content"
)
# What an export holds that a second run must reproduce; the editor log,
# timings, and catalog are reports about the run, not its output.
DIGESTED = ("meshes", "textures", "maps", "materials.json", "meshes.json")
# The merged backdrop building that exports from its render data with an
# index out of range (the plan's "Pieces that exported badly"): its buffer
# holds whatever memory Unreal read past the end, so its bytes vary by run.
# Nothing uses it.
KNOWN_UNSTABLE = {
    "meshes/Meshes/Architecture/MergedMeshes/SM_MERGED_StaticMeshActor_UAID_00E04CB184546F3D01_2057930594.glb",
}


def refuse_inside_repo(path):
    resolved = path.expanduser().resolve()
    if resolved == REPO or REPO in resolved.parents:
        sys.exit(f"{resolved} is inside the repository; the kit never enters it")


def sha256(path):
    digest = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def digests(export):
    out = {}
    for name in DIGESTED:
        root = export / name
        files = [root] if root.is_file() else sorted(p for p in root.rglob("*") if p.is_file())
        for path in files:
            rel = path.relative_to(export).as_posix()
            out[rel] = {"sha256": sha256(path), "bytes": path.stat().st_size}
    return out


def write_digests(export):
    table = digests(export)
    document = {
        "schema": "openagents.verse.medieval-town-export.v1",
        "files": len(table),
        "bytes": sum(v["bytes"] for v in table.values()),
        "digests": table,
    }
    path = export / "digests.json"
    path.write_text(json.dumps(document, indent=1, sort_keys=True) + "\n")
    print(f"{path}: {document['files']} files, {document['bytes']} bytes")
    return path


def compare(a, b):
    left, right = digests(a), digests(b)
    differ = sorted(
        k for k in set(left) | set(right) if left.get(k, {}).get("sha256") != right.get(k, {}).get("sha256")
    )
    for k in sorted(set(differ) & KNOWN_UNSTABLE):
        print(f"  known unstable, not compared: {k}")
    differ = [k for k in differ if k not in KNOWN_UNSTABLE]
    kinds = {}
    for k in differ:
        kinds.setdefault(k.split("/", 1)[0], []).append(k)
    print(f"{len(left)} and {len(right)} files; {len(differ)} differ")
    for kind, files in sorted(kinds.items()):
        print(f"  {kind}: {len(files)} differ, such as {files[:3]}")
    return 0 if not differ else 1


def gcloud(args, dry_run):
    command = ["gcloud", "storage", *args]
    print(("would run: " if dry_run else "running: ") + " ".join(command))
    if not dry_run:
        subprocess.run(command, check=True)


def archive(export, source, dry_run):
    if not (source / "Medieval_Town").is_dir():
        sys.exit(f"{source} has no Medieval_Town folder")
    path = write_digests(export)
    gcloud(["rsync", "--recursive", "--checksums-only", str(source), f"gs://{BUCKET}/{PREFIX}/content"], dry_run)
    gcloud(["cp", str(path), f"gs://{BUCKET}/{PREFIX}/export-digests.json"], dry_run)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("command", choices=["digest", "compare", "archive"])
    parser.add_argument("export", type=Path)
    parser.add_argument("other", type=Path, nargs="?")
    parser.add_argument("--source", type=Path, default=DEFAULT_SOURCE)
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    export = args.export.expanduser()
    refuse_inside_repo(export)
    if args.command == "digest":
        write_digests(export)
    elif args.command == "compare":
        if args.other is None:
            sys.exit("compare needs two exports")
        refuse_inside_repo(args.other)
        sys.exit(compare(export, args.other.expanduser()))
    else:
        refuse_inside_repo(args.source)
        archive(export, args.source, args.dry_run)


if __name__ == "__main__":
    main()
