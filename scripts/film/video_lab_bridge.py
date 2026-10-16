"""Video Lab's rules live in Rust (crates/video-lab); this calls them.

One JSON request in, one JSON answer out, through the `video-lab` program:
VIDEO_LAB_BIN, else the build in CARGO_TARGET_DIR (default
~/work/openagents-target-videolab), else `cargo run` from this checkout.
"""
import json
import os
import subprocess
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]


class Refused(Exception):
    """The rules refused, with the reason in plain words."""


def _argv():
    binary = os.environ.get("VIDEO_LAB_BIN")
    if binary:
        return [binary]
    target = Path(os.environ.get("CARGO_TARGET_DIR", str(Path.home() / "work/openagents-target-videolab")))
    built = target / "debug/video-lab"
    if built.exists() and built.stat().st_mtime >= _newest_source():
        return [str(built)]
    return ["cargo", "run", "--quiet", "-p", "video-lab", "--bin", "video-lab", "--"]


def _newest_source():
    roots = [REPO / "crates/video-lab", REPO / "crates/lab-kit", REPO / "docs/video-lab/schemas"]
    return max((p.stat().st_mtime for r in roots for p in r.rglob("*") if p.is_file()), default=0)


def call(action, **request):
    target = os.environ.get("CARGO_TARGET_DIR", str(Path.home() / "work/openagents-target-videolab"))
    result = subprocess.run(_argv(), input=json.dumps({"action": action, **request}), capture_output=True,
                            text=True, cwd=REPO, env={**os.environ, "CARGO_TARGET_DIR": target}, timeout=600)
    if result.returncode:
        raise RuntimeError("Video Lab's rules could not run. " + result.stderr[-800:])
    out = json.loads(result.stdout)
    if out.get("error"):
        raise RuntimeError(out["error"])
    if "refused" in out:
        raise Refused(out["refused"])
    return out
