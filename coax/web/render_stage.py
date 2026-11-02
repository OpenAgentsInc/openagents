"""Copy a Coax-built render package without rewriting its hash-bound bytes.

This is release tooling. Coax owns package/catalog/pipeline admission; this
adapter checks copy integrity and gives the complete directory one immutable URL.
"""
import hashlib
import json
from pathlib import Path
import re


def package_products(source: Path) -> tuple[dict, dict[str, bytes]]:
    """Read and authenticate a complete package before publishing any copy."""
    raw = source.read_bytes()
    package = json.loads(raw)
    if package.get("schema") != "aethyr.render.package.v1":
        raise ValueError("unknown render package schema")
    modules, entries = package["modules"], package["entries"]
    if not 1 <= len(modules) <= 3 or not 1 <= len(entries) <= 14:
        raise ValueError("render package table capacity")
    products = {"render.json": raw}
    for rows, file_key, hash_key, extension in ((modules, "wgsl_file", "wgsl_sha256", ".wgsl"),
                                               (entries, "spirv_file", "spirv_sha256", ".spv"),
                                               (entries, "tree_file", "tree_sha256", ".tree")):
        for row in rows:
            name = row[file_key]
            if not re.fullmatch(r"[A-Za-z0-9_-][A-Za-z0-9_.-]{0,127}", name) or not name.endswith(extension):
                raise ValueError("unsafe shader sibling filename")
            data = (source.parent / name).read_bytes()
            if hashlib.sha256(data).hexdigest() != row[hash_key]:
                raise ValueError("render package product hash differs")
            products[name] = data
    return package, products


def stage_package(app: Path, url: str, stage: Path) -> str:
    if not re.fullmatch(r"/build/render/[a-z0-9-]+/render\.json", url):
        raise ValueError("unsafe render package URL")
    _, products = package_products(app / url.lstrip("/"))
    raw = products["render.json"]
    # The manifest and all its relative sibling URLs live in this same immutable
    # directory. Copy to both trees for normal deploy and prefixed pinned demos.
    rel = Path("build/render") / hashlib.sha256(raw).hexdigest()
    for tree in ("assets", "site"):
        directory = stage / tree / rel
        directory.mkdir(parents=True, exist_ok=True)
        for name, data in products.items():
            (directory / name).write_bytes(data)
    return "/" + str(rel / "render.json")
