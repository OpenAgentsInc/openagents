#!/usr/bin/env python3
"""Stage the local files named by Coax's typed initial asset observations.

URL classification, pack expansion, delivery IDs and readiness are already
decided by asset-profile.coax. This tool only copies those declared files.
"""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import tempfile


def sha(data): return hashlib.sha256(data).hexdigest()


def stage(table_path: Path, package_path: Path, web_root: Path, output: Path):
    raw = table_path.read_bytes(); table = json.loads(raw); package = json.loads(package_path.read_bytes())
    if table.get("schema") != "aethyr.render.assets.v1": raise ValueError("unknown typed asset table")
    for key in ("profile_index", "slug", "app"):
        if table.get(key) != package.get(key): raise ValueError("asset table differs from native package identity")
    root = web_root.resolve(); products = {}
    for row in table["rows"]:
        if type(row["initial"]) is not bool or type(row["gpu_required"]) is not bool: raise ValueError("invalid asset flags")
        if not (row["initial"] or row["gpu_required"]): continue
        for url in row["urls"]:
            if not isinstance(url, str) or not url.startswith("/") or url.startswith("//") or "\\" in url or "?" in url or "#" in url:
                raise ValueError("asset URL has no canonical local file path")
            parts = url[1:].split("/")
            if any(part in ("", ".", "..") for part in parts): raise ValueError("asset URL escapes its local tree")
            relative = PurePosixPath(*parts); source = (root / relative).resolve()
            if not source.is_relative_to(root): raise ValueError("asset source escapes its local tree")
            if url not in products: products[url] = (relative, source.read_bytes())
    # Authenticate all source observations before publishing the first copy.
    if raw != table_path.read_bytes(): raise ValueError("asset table changed during staging")
    output.mkdir(parents=True, exist_ok=True)
    records = []
    for url, (relative, data) in products.items():
        target = output / relative; target.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.NamedTemporaryFile(dir=target.parent, prefix=".render-asset-", delete=False) as pending:
            pending.write(data); temporary = Path(pending.name)
        temporary.replace(target)
        records.append(dict(url=url, file=str(relative), bytes=len(data), sha256=sha(data)))
    (output / "assets.json.pending").write_bytes(raw); (output / "assets.json.pending").replace(output / "assets.json")
    receipt = dict(schema="aethyr.render.asset-staging.v1", status="staged", profile_index=table["profile_index"],
                   slug=table["slug"], table_sha256=sha(raw), files=records,
                   scope="Copies of Coax-declared initial or GPU-required local files. Pack rows retain their separate delivery IDs while the container is copied once. No asset parsing, runtime readiness, image decoding or GPU qualification.")
    (output / "staging.json").write_text(json.dumps(receipt, indent=2) + "\n")
    return receipt


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--table", type=Path, required=True)
    parser.add_argument("--package", type=Path, required=True)
    parser.add_argument("--web-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(); stage(args.table, args.package, args.web_root, args.output)


if __name__ == "__main__": main()
