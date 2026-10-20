#!/usr/bin/env bash
# Write every checked-in generated file again from its sources (#12208).
#
#   scripts/dev/regenerate.sh                     # every generator
#   scripts/dev/regenerate.sh --changed BASE      # those the change since BASE is due
#   scripts/dev/regenerate.sh --only cli-tree     # named generators (comma list)
#   scripts/dev/regenerate.sh --packages a,b      # also those whose check is in a or b
#   scripts/dev/regenerate.sh --list              # name, check, and command of each
#
# The generators are listed in .openagents/generated.json, each with the
# test that fails when its file is stale (`check`) beside the command that
# writes it again (`regenerate`). Run this before you push when such a test
# fails, then commit what it rewrote. The landing queue's integrator runs it
# with --changed on each rebased code entry before the checks and commits
# the result as "Regenerate generated files" (docs/cloud/land-queue.md).
#
# A change is due a generator when it touches one of the generator's
# `sources` (holding one of its `markers`, when given) or any file of the
# package whose test checks it. A generator whose `requires` is missing
# here (the Android bindings need ANDROID_NDK_HOME and cargo-ndk) is skipped
# and named.
#
# One line per generator on standard output:
#   regenerated NAME: FILE...   the command rewrote these files
#   current NAME                the command ran; nothing changed
#   skipped NAME: WHY           not run here
#   failed NAME: WHY            the command failed (exit status 1)
# Commands run at the top of the repository. CARGO_TARGET_DIR defaults to
# ~/work/openagents-target-1, the shared root-workspace target.
set -euo pipefail

top=$(git rev-parse --show-toplevel)
cd "$top"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/work/openagents-target-1}"
export CARGO_INCREMENTAL="${CARGO_INCREMENTAL:-0}"

exec python3 -I - "$top" "$@" <<'PY'
import json, os, shutil, subprocess, sys
from pathlib import Path

top = Path(sys.argv[1])
args = sys.argv[2:]
USAGE = "usage: scripts/dev/regenerate.sh [--changed BASE] [--only NAMES] [--packages NAMES] [--list]"


def take(flag):
    if flag not in args:
        return None
    i = args.index(flag)
    if i + 1 >= len(args):
        sys.exit(f"{flag} needs a value\n{USAGE}")
    value = args[i + 1]
    del args[i:i + 2]
    return value


changed_base = take("--changed")
only = take("--only")
packages = [p for p in (take("--packages") or "").split(",") if p]
listing = "--list" in args
if listing:
    args.remove("--list")
if args:
    sys.exit(f"unknown argument {args[0]}\n{USAGE}")

registry = json.loads((top / ".openagents/generated.json").read_text())
generators = registry.get("generated", [])
if only:
    names = only.split(",")
    unknown = [n for n in names if n not in {g["name"] for g in generators}]
    if unknown:
        sys.exit(f"no generator named {', '.join(unknown)}")
    generators = [g for g in generators if g["name"] in names]

if listing:
    for g in generators:
        print(f"{g['name']}\n  check:      {g.get('check', '')}\n  regenerate: {g['regenerate']}")
    sys.exit(0)


def git(*words):
    return subprocess.run(["git", *words], cwd=top, capture_output=True, text=True, check=True).stdout


def package_of(path):
    """The Cargo package whose folder holds `path`, by its nearest manifest."""
    folder = (top / path).parent
    while True:
        manifest = folder / "Cargo.toml"
        if manifest.is_file():
            in_package = False
            for line in manifest.read_text(errors="replace").splitlines():
                line = line.strip()
                if line.startswith("["):
                    in_package = line == "[package]"
                elif in_package and line.startswith("name") and "=" in line:
                    return line.split("=", 1)[1].strip().strip('"')
        if folder == top or folder == folder.parent:
            return None
        folder = folder.parent


def due(g, files, owners):
    if g.get("package") and g["package"] in packages:
        return True
    for f in files:
        if f in g.get("files", []):
            continue
        if g.get("package") and owners.get(f) == g["package"]:
            return True
        if any(f.startswith(s) for s in g.get("sources", [])):
            markers = g.get("markers") or []
            if not markers:
                return True
            try:
                text = (top / f).read_text(errors="replace")
            except OSError:
                continue
            if any(m in text for m in markers):
                return True
    return False


if changed_base is not None:
    files = [f for f in git("diff", "--name-only", changed_base, "HEAD").splitlines() if f]
    owners = {f: package_of(f) for f in files}
    generators = [g for g in generators if due(g, files, owners)]


def missing(g):
    needs = g.get("requires") or {}
    gone = [f"${v}" for v in needs.get("env", []) if not os.environ.get(v)]
    gone += [c for c in needs.get("commands", []) if shutil.which(c) is None]
    return gone


def snapshot(path):
    try:
        return (top / path).read_bytes()
    except OSError:
        return None


failed = False
for g in generators:
    name = g["name"]
    gone = missing(g)
    if gone:
        print(f"skipped {name}: needs {' and '.join(gone)}", flush=True)
        continue
    print(f"regenerate: {name}: {g['regenerate']}", file=sys.stderr, flush=True)
    before = {f: snapshot(f) for f in g["files"]}
    run = subprocess.run(["bash", "-c", g["regenerate"]], cwd=top, stdin=subprocess.DEVNULL,
                         stdout=sys.stderr, stderr=subprocess.PIPE, text=True)
    if run.returncode != 0:
        tail = " / ".join(line for line in run.stderr.strip().splitlines()[-6:])
        print(f"failed {name}: exit {run.returncode}: {tail}", flush=True)
        failed = True
        continue
    rewritten = [f for f in g["files"] if snapshot(f) != before[f]]
    if rewritten:
        print(f"regenerated {name}: {' '.join(rewritten)}", flush=True)
    else:
        print(f"current {name}", flush=True)
sys.exit(1 if failed else 0)
PY
