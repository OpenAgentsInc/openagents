"""Shared helpers for the briefed-agent A/B bench (#11211).

Paths, git access against the main checkout's object store, the build
host, and the lever defaults every run records.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = Path(os.environ.get("AB_REPO", str(Path.home() / "openagents")
                          if os.environ.get("AB_HOST") == "local" else "/Users/christopherdavid/work/openagents"))
WORK = Path(os.environ.get("AB_WORK", str(HERE / ".work")))
HOST = os.environ.get("AB_HOST", "coderos-4080")
GH_REPO = "OpenAgentsInc/openagents"
# AB_HOST=local: the trials and the build run on this machine (a cloud dev
# environment, docs/cloud/dogfood-dev-on-prod.md); the build host's scripts
# run through bash instead of ssh.
SSH = ["bash", "-c"] if HOST == "local" else [
    "ssh",
    "-o", "ControlMaster=auto",
    "-o", "ControlPath=/tmp/ab-ssh-%C",
    "-o", "ControlPersist=15m",
    "-o", "ConnectTimeout=20",
    HOST,
]

# Every lever a run can turn (#11211's lever table). A run records the
# full settings, so any two runs can be compared lever by lever.
#
# These are also the production defaults (scripts/work/work_issue.py reads
# them). #11309 tested each against B0 under the frozen keep rule and none
# moved: results/ab-tools.md (Bbash, Brelated, Boutline, Bfinish: no cost
# reduction CI above 0, so tools stay "verify"), results/ab-levers.md
# (Bsmall, Blarge, Bterse, Blow, Bsonnet: none passes, so briefing_files 8,
# excerpt_lines 140, history 2, template v3, effort default and Opus 5.5
# stay), results/ab-cold.md (Bcold: no cost change, so build_cache "warm").
LEVERS: dict[str, object] = {
    # 1 Briefing
    "finder": "finder",          # finder (#11210 filefind) | lite | oracle | haiku | haiku-solo | clef-arm | hosted-arm | jev-arm (#11389)
    "briefer_turns": 14,         # haiku: most agent turns (haiku_briefer.py)
    "briefer_secs": 120,         # haiku: hard time cap, seconds
    "briefer_usd": 0.4,          # haiku: list-price cap
    "briefer_candidates": 15,    # haiku: filefind candidates it starts from
    "briefing_files": 8,         # most files listed
    "excerpt": "windows",        # windows | whole (whole file when small)
    "excerpt_lines": 140,        # most excerpt lines per file
    "briefing_tokens": 14000,    # rough cap on the briefing text
    "history": 2,                # analogous past changes included
    "plan": "deterministic",     # deterministic | none
    "checks": "rust",            # rust (S2: cargo for 1-2 crates) | kinds (#11310: checks per package kind, multi-package)
    "acceptance": "bullets",     # bullets (the issue's bullet points) | typed (#11310: typed extraction, mapped to checks)
    "packages": 4,               # most packages a kinds briefing checks and budgets excerpts across
    "interface": True,           # tell every arm the new items the fix's tests call
    # 2 Instructions
    "template": "v3",            # system prompt template (v3: tool-aware)
    # 3 Tools
    "tools": "verify",           # verify (B0) | bash | checks | verify+related|outline|finish ...
    "check_output": "smart",     # smart | full
    # 4 Plugins / MCP / skills
    "mcp": "checks",             # only the bench's check server
    # 5 Model routing
    "engine": "claude",          # claude | codex | opencode | gemini (#11308)
    "model": "claude-opus-5-5",
    "effort": "default",         # default | low | medium | high
    # 6 Loop control
    "max_turns": 0,              # 0: no limit
    "timeout_secs": 1200,
    # 7 Token economics
    "briefing_in": "system",     # system | user
    # 8 Execution environment
    "build_cache": "warm",       # warm | cold
    "build_host": HOST,
    # 9 Post-processing
    "post_fmt": False,           # run cargo fmt after the agent, by script
    # 10 Decomposition
    "decomposition": "single",
    # 11 Triage
    "triage": "none",
    # 12 Learning loop
    "learning": "none",
    # 13 Recovery (#11311): a second briefed attempt from the first one's misses and check evidence
    "rebrief": 0,                # 0 | 1: at most one re-brief before any fallback
    "rebrief_worktree": "clean",  # clean (fresh worktree, first diff shown) | keep (continue on the first diff)
}


def run(cmd, cwd=None, check=True, input=None, timeout=None, env=None):
    proc = subprocess.run(
        cmd,
        cwd=cwd,
        input=input,
        capture_output=True,
        text=isinstance(input, str) or input is None,
        timeout=timeout,
        env=env,
    )
    if check and proc.returncode != 0:
        err = proc.stderr if isinstance(proc.stderr, str) else proc.stderr.decode(errors="replace")
        raise RuntimeError(f"{' '.join(map(str, cmd))[:200]} failed: {err[-2000:]}")
    return proc


def git(*args, check=True, cwd=None) -> str:
    return run(["git", *args], cwd=cwd or REPO, check=check).stdout


def show(commit: str, path: str) -> str | None:
    proc = run(["git", "show", f"{commit}:{path}"], cwd=REPO, check=False)
    return proc.stdout if proc.returncode == 0 else None


def package_of(commit: str, path: str) -> str | None:
    """The Cargo package that owns `path` at `commit`."""
    parts = path.split("/")
    for i in range(len(parts) - 1, 0, -1):
        manifest = "/".join(parts[:i]) + "/Cargo.toml"
        text = show(commit, manifest)
        if text and "[package]" in text:
            m = re.search(r'^\s*name\s*=\s*"([^"]+)"', text.split("[package]", 1)[1], re.M)
            if m:
                return m.group(1)
    return None


def is_test_file(path: str) -> bool:
    return (
        "/tests/" in path
        or path.endswith("/tests.rs")
        or path.endswith("_tests.rs")
        or path.endswith("_test.rs")
        or "/tests/" in path
    )


def load_json(path: Path):
    return json.loads(Path(path).read_text())


def dump_json(path: Path, value) -> None:
    Path(path).parent.mkdir(parents=True, exist_ok=True)
    Path(path).write_text(json.dumps(value, indent=2, sort_keys=False) + "\n")
