#!/usr/bin/env python3
"""Bind one native build to the identity of its Coax-built renderer package.

This build adapter copies literal admitted identity. Runtime package and
pipeline admission stay in the shared Coax facade.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "coax/web"))
from render_stage import package_products


def identity(package: Path, app: str, profile: str) -> bytes:
    declaration, products = package_products(package)
    if declaration.get("app") != app or declaration.get("slug") != profile:
        raise ValueError("renderer package differs from the requested native app/profile")
    for value in (app, profile):
        if not re.fullmatch(r"[a-z][a-z0-9-]*", value):
            raise ValueError("unsafe native build identity")
    index = declaration.get("profile_index")
    if type(index) is not int or index < 0:
        raise ValueError("invalid compiled renderer profile index")
    manifest_hash = hashlib.sha256(products["render.json"]).hexdigest()
    return ("// Generated from the Coax-built render.json; do not edit.\n"
            "#ifndef AETHYR_RENDER_IDENTITY_H\n#define AETHYR_RENDER_IDENTITY_H\n#include <stdint.h>\n"
            "#define AETHYR_RENDER_TYPED 1\n"
            f"#define AETHYR_RENDER_PROFILE_ID INT64_C({index})\n"
            "#define AETHYR_RENDER_PROFILE AETHYR_RENDER_PROFILE_ID\n"
            f"#define AETHYR_RENDER_APP {json.dumps(app)}\n"
            f"#define AETHYR_RENDER_SLUG {json.dumps(profile)}\n"
            '#define AETHYR_RENDER_PACKAGE_FILE "render/render.json"\n'
            '#define AETHYR_RENDER_PACKAGE AETHYR_RENDER_PACKAGE_FILE\n'
            f"#define AETHYR_RENDER_PACKAGE_SHA256 {json.dumps(manifest_hash)}\n"
            "#endif\n").encode()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--package", type=Path, required=True)
    parser.add_argument("--app", required=True)
    parser.add_argument("--profile", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    data = identity(args.package, args.app, args.profile)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    temporary = args.output.with_suffix(args.output.suffix + ".pending")
    temporary.write_bytes(data)
    temporary.replace(args.output)


if __name__ == "__main__":
    main()
