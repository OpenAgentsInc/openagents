#!/usr/bin/env python3
"""Check a DeepSeek-V4.1 stage build bundle before anything runs it (#12407, #12410).

A bundle is a folder holding the worker `psionic-v41-stage`, its CUDA
libraries in `lib/`, and `BUILD.json` (schema v2, written by
scripts/pylon/v41-stage-build.sh), which lists the SHA-256 of the worker and
of every library. The bundle hash is the SHA-256 of `BUILD.json`, so a pin on
it covers every file the document lists.

  v41-build-check.py build DIR [MANIFEST] [--pin SHA256]
      Check DIR's worker and libraries against DIR/BUILD.json. With --pin,
      first check that BUILD.json hashes to SHA256, a pin supplied
      independently of the download, such as `pylon cloud up --worker-build`
      or the qualified profile's expected bundle. With MANIFEST, check that
      the build was made with that group manifest. Prints the build's GPU
      architecture.
  v41-build-check.py file PATH SHA256
      Check that one file hashes to SHA256.
  v41-build-check.py field JSON KEY
      Print a non-empty string field of a JSON document.

Exits 1 with the reason on standard error when a check fails.
v41-stage-startup.sh installs this file as /opt/openagents/oa-verify, and
stage-run and head-run call it before they start anything. Use it the same
way after any download of a bundle, for example:

  python3 scripts/pylon/v41-build-check.py build "$DIR" "$MANIFEST" --pin "$BUILD"
"""
import hashlib
import json
import os
import sys

SCHEMA = "openagents.psionic_v41_stage.build.v2"


def sha256(path):
    digest = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def refuse(message):
    print(message, file=sys.stderr)
    sys.exit(1)


def check_build(root, manifest=None, pin=None):
    record = os.path.join(root, "BUILD.json")
    if pin is not None:
        got = sha256(record)
        if got != pin.lower():
            refuse(f"BUILD.json hashes to {got}, but the pinned build is {pin}")
    build = json.load(open(record))
    if build.get("schema") != SCHEMA:
        refuse(f"BUILD.json has schema {build.get('schema')}; rebuild with scripts/pylon/v41-stage-build.sh")
    libraries = build.get("libraries") or {}
    if not libraries:
        refuse("BUILD.json lists no CUDA libraries")
    got = sha256(os.path.join(root, "psionic-v41-stage"))
    if got != build.get("sha256"):
        refuse(f"the worker hashes to {got}, but BUILD.json says {build.get('sha256')}")
    present = sorted(os.listdir(os.path.join(root, "lib")))
    if present != sorted(libraries):
        refuse(f"lib/ holds {present}, but BUILD.json lists {sorted(libraries)}")
    for name, want in sorted(libraries.items()):
        got = sha256(os.path.join(root, "lib", name))
        if got != want:
            refuse(f"lib/{name} hashes to {got}, but BUILD.json says {want}")
    if manifest and manifest not in (build.get("manifests") or {}):
        refuse(f"the build was not made with group manifest {manifest}")
    return build.get("arch", "")


def main(args):
    pin = None
    if "--pin" in args:
        at = args.index("--pin")
        if at + 1 >= len(args):
            refuse("--pin needs the SHA-256 of BUILD.json")
        pin = args[at + 1]
        del args[at:at + 2]
        if len(pin) != 64 or any(c not in "0123456789abcdefABCDEF" for c in pin):
            refuse(f"--pin takes a SHA-256 in hex, not {pin!r}")
    verb = args[0] if args else ""
    if verb == "file" and len(args) == 3:
        got = sha256(args[1])
        if got != args[2]:
            refuse(f"{args[1]} hashes to {got}, not {args[2]}")
    elif verb == "field" and len(args) == 3:
        value = json.load(open(args[1])).get(args[2])
        if not isinstance(value, str) or not value:
            refuse(f"{args[1]} has no {args[2]}")
        print(value)
    elif verb == "build" and len(args) in (2, 3):
        print(check_build(args[1], args[2] if len(args) == 3 else None, pin))
    else:
        refuse(__doc__.strip())


if __name__ == "__main__":
    main(sys.argv[1:])
