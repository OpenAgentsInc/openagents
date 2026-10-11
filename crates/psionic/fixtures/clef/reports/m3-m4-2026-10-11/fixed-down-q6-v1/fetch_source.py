#!/usr/bin/env python3
"""Fill missing pinned public objects in the task-owned scratch checkout."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tarfile
import urllib.request

HERE = Path(__file__).resolve().parent
SOURCE = HERE / "upstream-checkout"
REVISION = "62a6f74bab748b1755f4cd8dfc0871caf865c6c8"


def main():
    if os.environ.get("CLEF_Q6_REGEN_GUARDED") != "1":
        raise ValueError("requires owned-process guard")
    if SOURCE.resolve().parent != HERE or not (SOURCE / ".git").is_dir():
        raise ValueError("expected the task-owned incomplete checkout")
    pins = json.loads((HERE / "upstream.json").read_text())
    tree = subprocess.check_output(["git", "-C", str(SOURCE), "ls-tree", "-rz", REVISION], timeout=15)
    expected = {}
    for record in tree.split(b"\0"):
        if not record:
            continue
        header, name = record.split(b"\t", 1)
        mode, kind, oid = header.decode().split()
        if kind != "blob" or mode not in {"100644", "100755", "120000"}:
            raise ValueError("unsupported source tree entry")
        expected[os.fsdecode(name)] = (mode, oid)
    archive = Path(os.environ["CLEF_Q6_FETCH_RECEIPT"]).with_name("upstream.tar.gz")
    url = "https://codeload.github.com/ggml-org/llama.cpp/tar.gz/" + REVISION
    downloaded = 0
    archive_digest = hashlib.sha256()
    with urllib.request.urlopen(url, timeout=30) as remote, archive.open("xb") as output:
        while raw := remote.read(1024 * 1024):
            downloaded += len(raw)
            if downloaded > 256 * 1024**2:
                raise ValueError("source archive exceeds 256 MiB")
            archive_digest.update(raw)
            output.write(raw)
    seen, expanded = set(), 0
    prefix = "llama.cpp-" + REVISION + "/"
    with tarfile.open(archive, "r:gz") as bundle:
        for member in bundle:
            if not member.name.startswith(prefix) or member.isdir():
                if member.name.rstrip("/") != prefix.rstrip("/") and not member.name.startswith(prefix):
                    raise ValueError("unexpected archive root")
                continue
            name = member.name[len(prefix):]
            if name not in expected or name in seen:
                raise ValueError("archive entry not in pinned Git tree")
            target = SOURCE / name
            if not target.parent.resolve().is_relative_to(SOURCE.resolve()):
                raise ValueError("archive path escapes source checkout")
            mode, oid = expected[name]
            expanded += member.size
            if expanded > 1024**3 or member.size > 128 * 1024**2:
                raise ValueError("expanded source archive exceeds bound")
            target.parent.mkdir(parents=True, exist_ok=True)
            if mode == "120000":
                if not member.issym():
                    raise ValueError("expected symbolic link")
                data = os.fsencode(member.linkname)
                if not (target.parent / member.linkname).resolve().is_relative_to(SOURCE.resolve()):
                    raise ValueError("archive symbolic link escapes source checkout")
                digest = hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest()
                if digest != oid:
                    raise ValueError("source symbolic link differs from pinned tree")
                if target.is_symlink() or target.exists():
                    target.unlink()
                target.symlink_to(member.linkname)
            else:
                if not member.isfile() or target.is_symlink():
                    raise ValueError("expected regular source file")
                digest = hashlib.sha1(b"blob " + str(member.size).encode() + b"\0")
                stream = bundle.extractfile(member)
                if stream is None:
                    raise ValueError("missing archive payload")
                written = 0
                with target.open("wb") as output:
                    while raw := stream.read(1024 * 1024):
                        digest.update(raw)
                        written += len(raw)
                        output.write(raw)
                if written != member.size or digest.hexdigest() != oid:
                    raise ValueError("source bytes differ from pinned Git tree")
                target.chmod(0o755 if mode == "100755" else 0o644)
            seen.add(name)
    if seen != set(expected):
        raise ValueError("source archive did not contain the complete pinned tree")
    head = subprocess.check_output(["git", "-C", str(SOURCE), "rev-parse", "HEAD"], text=True, timeout=10).strip()
    status = subprocess.check_output(["git", "-C", str(SOURCE), "status", "--porcelain", "--untracked-files=all"], text=True, timeout=10)
    if head != REVISION or status:
        raise ValueError("source checkout is not the pinned clean tree")
    for name, item in pins["files"].items():
        if hashlib.sha256((SOURCE / name).read_bytes()).hexdigest() != item["sha256"]:
            raise ValueError("pinned source hash differs")
    output = Path(os.environ["CLEF_Q6_FETCH_RECEIPT"])
    output.write_text(json.dumps(dict(status="fetched", source=str(SOURCE), upstream_revision=REVISION,
                                    source_url=url, archive_sha256=archive_digest.hexdigest(), compressed_bytes=downloaded,
                                    expanded_bytes=expanded, verified_git_blobs=len(seen),
                                    source_files=pins["files"], original_checkout_changed=False), indent=2)+"\n")


if __name__ == "__main__":
    main()
