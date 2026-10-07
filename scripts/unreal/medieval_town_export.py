#!/usr/bin/env python3
"""Exports the Modular Medieval Town pack headlessly with Unreal Editor.

The pack is a licensed Fab asset. Everything this command writes is private
and stays outside the repository; see docs/verse/private-assets.md and
docs/verse/everglade-medieval-refactor.md.

The command never opens the owner's project. It builds a scratch project
under the output directory, clones the pack's content into it (an APFS
clone, so it costs no disk until a file changes), and runs
`scripts/unreal/medieval_town_ue.py` through Unreal's Python commandlet with
`-unattended -nullrhi`, so no window opens and nothing renders. Unreal
starts with a clean environment. The command records the source's file
sizes and modification times before and after and fails if any changed.

Usage:

    scripts/unreal/medieval_town_export.py
    scripts/unreal/medieval_town_export.py --steps meshes --limit 5
    scripts/unreal/medieval_town_export.py --catalog-only

Afterward it normalizes the output so two runs give the same bytes: Unreal's
glTF writer leaves the alignment padding between buffer views uninitialized,
and a map names each Blueprint's dynamic material instance by a per-run
object ID. It zeroes the padding and drops the IDs, then runs
`scripts/unreal/medieval_town_catalog.py` over the output.
`scripts/unreal/medieval_town_archive.py compare` checks two runs.
"""

import argparse
import json
import os
import re
import struct
import shutil
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent
DEFAULT_SOURCE = Path(
    "/Users/Shared/UnrealEngine/Launcher/VaultCache/ModularMcb8bfaae7940V2/data/Content"
)
DEFAULT_ENGINE = Path("/Users/Shared/Epic Games/UE_5.7")
DEFAULT_OUT = Path.home() / ".openagents/verse/private/medieval-town"
MIN_FREE_GB = 25
PROJECT = "MedievalExport"

UPROJECT = {
    "FileVersion": 3,
    "EngineAssociation": "5.7",
    "Category": "",
    "Description": "Private headless export project.",
    "Plugins": [
        {"Name": "PythonScriptPlugin", "Enabled": True},
        {"Name": "EditorScriptingUtilities", "Enabled": True},
        {"Name": "GLTFExporter", "Enabled": True},
    ],
}


def editor_binary(engine):
    mac = engine / "Engine/Binaries/Mac/UnrealEditor.app/Contents/MacOS/UnrealEditor"
    for candidate in (
        engine / "Engine/Binaries/Mac/UnrealEditor-Cmd",
        mac,
        engine / "Engine/Binaries/Linux/UnrealEditor-Cmd",
        engine / "Engine/Binaries/Win64/UnrealEditor-Cmd.exe",
    ):
        if candidate.exists():
            return candidate
    sys.exit(f"no UnrealEditor under {engine}")


def refuse_inside_repo(path):
    resolved = path.resolve()
    if resolved == REPO or REPO in resolved.parents:
        sys.exit(f"refusing to write licensed content inside the repository: {resolved}")


def free_gb(path):
    probe = path
    while not probe.exists():
        probe = probe.parent
    return shutil.disk_usage(probe).free / 1e9


def snapshot(root):
    """Size and modification time of every file under the source."""
    state = {}
    for dirpath, _, files in os.walk(root):
        for name in files:
            full = os.path.join(dirpath, name)
            info = os.stat(full)
            state[os.path.relpath(full, root)] = (info.st_size, info.st_mtime_ns)
    return state


def prepare_project(source, project_dir):
    content = project_dir / "Content"
    if not content.exists():
        content.mkdir(parents=True)
        print(f"cloning {source} into the scratch project")
        # `cp -c` makes APFS clones; fall back to a plain copy elsewhere.
        clone = subprocess.run(["cp", "-cR", f"{source}/.", str(content)])
        if clone.returncode != 0:
            shutil.rmtree(content)
            shutil.copytree(source, content)
    (project_dir / f"{PROJECT}.uproject").write_text(json.dumps(UPROJECT, indent=2) + "\n")
    os.chmod(project_dir.parent, 0o700)


def run_editor(editor, project_dir, out, args):
    env = {
        "HOME": os.environ["HOME"],
        "USER": os.environ.get("USER", ""),
        "PATH": "/usr/bin:/bin:/usr/sbin:/sbin",
        "TMPDIR": os.environ.get("TMPDIR", "/tmp"),
        "LANG": "en_US.UTF-8",
        "MEDIEVAL_EXPORT_OUT": str(out),
        "MEDIEVAL_EXPORT_ROOT": args.root,
        "MEDIEVAL_EXPORT_STEPS": args.steps,
        "MEDIEVAL_EXPORT_LIMIT": str(args.limit),
        "MEDIEVAL_EXPORT_MATCH": args.match,
    }
    command = [
        str(editor),
        str(project_dir / f"{PROJECT}.uproject"),
        "-run=pythonscript",
        f"-script={HERE / 'medieval_town_ue.py'}",
        "-unattended",
        "-nullrhi",
        "-nosplash",
        "-nosound",
        "-nopause",
        "-stdout",
        "-FullStdOutLogOutput",
    ]
    log_path = out / "editor.log"
    started = time.time()
    with open(log_path, "w") as log:
        code = subprocess.run(command, env=env, stdout=log, stderr=subprocess.STDOUT).returncode
    print(f"editor exited {code} after {time.time() - started:.0f} s; log: {log_path}")
    return code


# A Blueprint's generated component path carries a per-run ID, such as
# `..._C_CAT_UAID_FCB214DDF97AD60803_1791285944.`; the rest names the same
# object in every run.
RUN_ID = re.compile(r"_UAID_[0-9A-F]+_[0-9]+")


def normalize_glb(path):
    """Zeroes the bytes of a .glb's binary chunk that no buffer view covers,
    which Unreal leaves uninitialized. Returns whether the file changed."""
    data = bytearray(path.read_bytes())
    if data[:4] != b"glTF" or len(data) < 28:
        return False
    json_length = struct.unpack_from("<I", data, 12)[0]
    document = json.loads(data[20:20 + json_length])
    start = 20 + json_length
    if start + 8 > len(data):
        return False
    bin_length = struct.unpack_from("<I", data, start)[0]
    body = start + 8
    covered = bytearray(bin_length)
    for view in document.get("bufferViews", []):
        if view.get("buffer", 0) != 0:
            continue
        offset = view.get("byteOffset", 0)
        covered[offset:offset + view["byteLength"]] = b"\x01" * view["byteLength"]
    changed = False
    for i, used in enumerate(covered):
        if not used and data[body + i] != 0:
            data[body + i] = 0
            changed = True
    if changed:
        path.write_bytes(bytes(data))
    return changed


def normalize(export_dir):
    """Makes an export's bytes depend only on the source."""
    glbs = sum(normalize_glb(p) for p in sorted((export_dir / "meshes").rglob("*.glb")))
    maps = 0
    for path in sorted((export_dir / "maps").glob("*.json")):
        text = path.read_text()
        cleaned = RUN_ID.sub("", text)
        if cleaned != text:
            path.write_text(cleaned)
            maps += 1
    print(f"normalized {glbs} glTF files and {maps} maps")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--source", type=Path, default=DEFAULT_SOURCE, help="the pack's Content directory (vault copy)")
    parser.add_argument("--engine", type=Path, default=DEFAULT_ENGINE)
    parser.add_argument("--out", type=Path, default=DEFAULT_OUT, help="private output root")
    parser.add_argument("--root", default="/Game/Medieval_Town")
    parser.add_argument("--steps", default="meshes,textures,materials,maps")
    parser.add_argument("--limit", type=int, default=0)
    parser.add_argument("--match", default="")
    parser.add_argument("--catalog-only", action="store_true")
    parser.add_argument("--no-catalog", action="store_true")
    parser.add_argument("--normalize-only", action="store_true", help="normalize an existing export and stop")
    args = parser.parse_args()

    out_root = args.out.expanduser()
    refuse_inside_repo(out_root)
    export_dir = out_root / "export"
    project_dir = out_root / "ue" / PROJECT

    if args.normalize_only:
        normalize(export_dir)
        return

    if not args.catalog_only:
        if free_gb(out_root) < MIN_FREE_GB:
            sys.exit(f"under {MIN_FREE_GB} GB free; stopping")
        if not (args.source / "Medieval_Town").is_dir():
            sys.exit(f"{args.source} has no Medieval_Town folder")
        editor = editor_binary(args.engine)
        out_root.mkdir(parents=True, exist_ok=True)
        os.chmod(out_root, 0o700)
        export_dir.mkdir(exist_ok=True)
        before = snapshot(args.source)
        prepare_project(args.source, project_dir)
        code = run_editor(editor, project_dir, export_dir, args)
        after = snapshot(args.source)
        if before != after:
            changed = sorted(k for k in set(before) | set(after) if before.get(k) != after.get(k))
            sys.exit(f"the source changed during export: {changed[:10]}")
        print("source unchanged")
        if code != 0:
            sys.exit(code)
        normalize(export_dir)

    if not args.no_catalog:
        subprocess.run(
            [sys.executable, str(HERE / "medieval_town_catalog.py"), str(export_dir)], check=True
        )


if __name__ == "__main__":
    main()
