#!/usr/bin/env python3
"""Generate the source-owned Verse documentation reference; --check refuses drift."""

import argparse
import json
from pathlib import Path
import re
import sys
import tomllib


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "docs/verse/runtime-contract.json"


def extract(source, pattern, numeric=False):
    matches = re.findall(pattern, (ROOT / source).read_text())
    if len(matches) != 1:
        raise ValueError(f"Expected one contract value in {source}; found {len(matches)}")
    return {"source": source, "value": int(matches[0]) if numeric else matches[0]}


def contract():
    constants = {
        "wire_version": extract("crates/verse-world/src/service/wire.rs", r"pub const VERSION: u16 = (\d+);", True),
        "save_write_version": extract("crates/verse-world/src/service/save.rs", r"let saved = Saved \{\s*version: (\d+),", True),
        "character_schema": extract("crates/verse-world/src/service/save.rs", r"pub const CHARACTER_SCHEMA: u16 = (\d+);", True),
        "rules_revision": extract("crates/verse-world/src/play.rs", r'pub const RULES_REVISION: &str = "([^"]+)";'),
        "social_profile": extract("crates/verse-world/src/play/social.rs", r"pub const PROFILE_REVISION: u16 = (\d+);", True),
        "input_replay_schema": extract("crates/verse-world/src/replay.rs", r'pub const SCHEMA: &str = "([^"]+)";'),
        "public_release_schema": extract("crates/verse-content/src/authoring/release.rs", r'const SCHEMA: &str = "([^"]+)";'),
        "public_content_profile": extract("crates/verse-content/src/authoring/release.rs", r'schema: "([^"]+)"\.into\(\),'),
        "public_pack_version": extract("crates/verse-content/src/authoring/release.rs", r"\bpack: (\d+),", True),
    }
    packages = {}
    for name in ("verse-world", "verse-engine", "verse-content", "verse", "verse-host", "everglade-web"):
        source = f"crates/{name}/Cargo.toml"
        manifest = tomllib.loads((ROOT / source).read_text())
        dependencies = {}
        tables = [("all", manifest.get("dependencies", {}))]
        tables.extend((target, table.get("dependencies", {})) for target, table in manifest.get("target", {}).items())
        for target, table in tables:
            for dependency, spec in table.items():
                if not isinstance(spec, dict) or "path" not in spec:
                    continue
                dependencies[f"{target}:{dependency}"] = {
                    "path": spec["path"],
                    "default_features": spec.get("default-features", True),
                    "features": spec.get("features", []),
                    "optional": spec.get("optional", False),
                }
        packages[name] = {"source": source, "features": manifest.get("features", {}), "local_dependencies": dependencies}
    toolchain = tomllib.loads((ROOT / "rust-toolchain.toml").read_text())["toolchain"]["channel"]
    return {
        "schema": "verse.documentation.runtime-contract.v1",
        "generator": "scripts/verse-runtime-contract.py",
        "scope": "Declared source constants and Cargo feature edges; not resolved build features, compatibility admission, or platform acceptance.",
        "toolchain": {"source": "rust-toolchain.toml", "value": toolchain},
        "contracts": constants,
        "packages": packages,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="Refuse a missing or stale generated reference without writing it.")
    args = parser.parse_args()
    try:
        rendered = json.dumps(contract(), indent=2, sort_keys=True) + "\n"
    except (OSError, ValueError) as error:
        print(f"Cannot generate Verse runtime reference: {error}", file=sys.stderr)
        return 1
    if args.check:
        if not OUTPUT.exists() or OUTPUT.read_text() != rendered:
            print("Verse runtime reference is stale. Run python3 scripts/verse-runtime-contract.py.", file=sys.stderr)
            return 1
        print("Verse runtime reference matches source.")
    else:
        OUTPUT.write_text(rendered)
        print("Generated docs/verse/runtime-contract.json.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
