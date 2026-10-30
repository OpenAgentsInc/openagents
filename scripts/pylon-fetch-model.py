#!/usr/bin/env python3
"""Installer file/HTTP effects for a manifest emitted by the Coax registry.

Checksums here protect the download/cache; the native Coax engine independently
authenticates every checkpoint and metadata file before it may advertise.
Partial downloads survive interruption and are never treated as model files.
"""
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import sys
import tempfile
import time
import urllib.parse


def regular(path):
    if not path.exists() and not path.is_symlink():
        return False
    info = path.lstat()
    if not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid():
        raise ValueError("cache files must be regular files owned by this user: " + str(path))
    return True


def digest(path, deadline):
    value = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(8 * 1024 * 1024), b""):
            if time.monotonic() >= deadline:
                raise ValueError("model download/checksum reached its time bound; repeat the same join command to resume")
            value.update(block)
    return value.hexdigest()


def matches(path, item, deadline):
    if not regular(path):
        return False
    if item["bytes"] and path.stat().st_size != item["bytes"]:
        return False
    return digest(path, deadline) == item["sha256"]


def read_manifest(path):
    manifest = json.loads(Path(path).read_text())
    names = set()
    if not isinstance(manifest, dict) or not isinstance(manifest.get("files"), list) or not manifest["files"]:
        raise ValueError("Coax model manifest has no files")
    for item in manifest["files"]:
        if not isinstance(item, dict) or any(not isinstance(item.get(k), str) for k in ("name", "url", "sha256")):
            raise ValueError("invalid Coax model manifest file")
        name = item["name"]
        url = urllib.parse.urlsplit(item["url"])
        if (not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]{0,127}", name)
                or name in names or name == "source.json" or url.scheme != "https" or not url.hostname
                or url.username or url.password or url.fragment
                or not re.fullmatch(r"[0-9a-f]{64}", item["sha256"])
                or type(item["bytes"]) is not int or item["bytes"] < 0):
            raise ValueError("invalid Coax model manifest file")
        names.add(name)
    if any(name + ".part" in names for name in names):
        raise ValueError("model file collides with another file's partial download")
    return manifest


def fetch(manifest_path, directory, timeout=800):
    deadline = time.monotonic() + timeout
    manifest = read_manifest(manifest_path)
    cache = Path(directory)
    if cache.is_symlink():
        raise ValueError("model cache must not be a symlink")
    cache.mkdir(parents=True, exist_ok=True, mode=0o700)
    if not cache.is_dir() or cache.stat().st_uid != os.getuid():
        raise ValueError("model cache must be a directory owned by this user")
    regular(cache / "source.json")
    lock_path = cache / ".fetch.lock"
    flags = os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK
    with os.fdopen(os.open(lock_path, flags, 0o600), "a+") as lock:
        if not stat.S_ISREG(os.fstat(lock.fileno()).st_mode) or os.fstat(lock.fileno()).st_uid != os.getuid():
            raise ValueError("model cache lock is not a regular file")
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise ValueError("another model download owns this cache") from None
        for item in manifest["files"]:
            target, partial = cache / item["name"], cache / (item["name"] + ".part")
            regular(partial)
            if matches(target, item, deadline):
                print("pylon: cached and SHA-256 checked " + item["name"], flush=True)
                continue
            if regular(target):
                # This dedicated cache entry is corrupt, not an alternate
                # model. Its public pinned replacement is the authorized input.
                target.unlink()
            if matches(partial, item, deadline):
                os.replace(partial, target)
                print("pylon: completed retained " + item["name"], flush=True)
                continue
            if regular(partial) and item["bytes"] and partial.stat().st_size >= item["bytes"]:
                partial.unlink()
            if not partial.exists():
                partial.touch(mode=0o600)
            retained = partial.stat().st_size
            print("pylon: downloading %s%s" % (item["name"], " (resume at %d bytes)" % retained if retained else ""), flush=True)
            command = ["curl", "--fail", "--location", "--show-error", "--progress-bar",
                       "--proto", "=https", "--proto-redir", "=https", "--connect-timeout", "20",
                       "--max-time", "780", "--continue-at", "-",
                       "--output", str(partial), item["url"]]
            def transfer():
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise ValueError("model download/checksum reached its time bound; repeat the same join command to resume")
                try:
                    return subprocess.run(command, timeout=remaining)
                except subprocess.TimeoutExpired:
                    raise ValueError("model download interrupted at its time bound; partial progress retained, repeat the same join command to resume") from None
            result = transfer()
            if result.returncode == 33 and retained:
                # Some HTTPS mirrors do not support byte ranges. Keep the
                # ordinary path resumable; explicitly restart only this file.
                with partial.open("wb"):
                    pass
                result = transfer()
            if result.returncode:
                raise ValueError("download interrupted; rerun the same join command to resume " + item["name"])
            if not matches(partial, item, deadline):
                partial.unlink()
                raise ValueError("download SHA-256/length mismatch for " + item["name"] + "; no engine or beacon started")
            os.replace(partial, target)
            print("pylon: downloaded and SHA-256 checked " + item["name"], flush=True)
        with tempfile.NamedTemporaryFile(mode="w", prefix=".source-", dir=cache, delete=False) as receipt:
            receipt.write(json.dumps(manifest, indent=2) + "\n")
            receipt.flush()
            os.fsync(receipt.fileno())
        os.replace(receipt.name, cache / "source.json")
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest")
    parser.add_argument("cache")
    parser.add_argument("--timeout-seconds", type=float, default=800,
                        help="aggregate download/checksum bound, 1..800 seconds; repeat to resume")
    args = parser.parse_args()
    if not 1 <= args.timeout_seconds <= 800:
        parser.error("--timeout-seconds must be in 1..800")
    try:
        fetch(args.manifest, args.cache, args.timeout_seconds)
    except (ValueError, OSError, KeyError, json.JSONDecodeError) as error:
        print("pylon: " + str(error), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
