"""Transport between the existing film service and the Rust timeline editor."""
import json
import os
from pathlib import Path
import subprocess


def call(repo, action, document, command=None, media=None):
    binary = os.environ.get("VIDEO_LAB_TIMELINE_BIN")
    target = Path(os.environ.get("CARGO_TARGET_DIR", str(Path.home() / "work/openagents-target-videolab")))
    built = target / "debug/video-lab-timeline"
    argv = [binary] if binary else ([str(built)] if built.exists() else
           ["cargo", "run", "--quiet", "-p", "verse-film", "--bin", "video-lab-timeline", "--"])
    env = {**os.environ, "CARGO_TARGET_DIR": str(target)}
    result = subprocess.run(argv, input=json.dumps({"action": action, "document": document,
                            "command": command, "media": media or {}}), capture_output=True,
                            text=True, cwd=repo, env=env, timeout=180)
    if result.returncode:
        raise RuntimeError("The timeline editor could not start. " + result.stderr[-500:])
    out = json.loads(result.stdout)
    if out.get("error"):
        raise ValueError(out["error"])
    return out
