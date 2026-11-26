#!/usr/bin/env python3
# Firm reason: 5 (live-agent eval runner: temp repos, process control and JSON grading for Tier 2)
"""Tier 2 headless live-agent runner for Coder Desktop.

Each task in evals/openagents-desktop/tier2/tasks/*.json runs the real page (page.coax
through host.c, linked into host_driver) against the real Microcoder worker
(coder/tui/mcworker.coax) with the flags the desktop sends (gated, edits, agents,
unlimited, the skills prefix, the composer's default model) inside an isolated temp
git repo, with a temp HOME, a chat budget and a wall-clock limit.

It calls real models and costs money. run.sh only runs it with --tier 2.

Per task it writes, under evals/openagents-desktop/results/tier2-<ts>-<sha>/cases/<id>/:
  wire.jsonl       every line each way between page and workers (host_driver WIRE_LOG)
  journal.jsonl    the workers' durable journals (.ajr frames), decoded, payloads cut
  attempts.jsonl   the model attempts (MWW_ATTEMPT lines) with cost and tokens
  chat.saved       the chat as the app keeps it (cd_save)
  workspace.diff   what the run changed in the temp repo
  result.json      pass/fail, every grader's verdict, cost, latency, turns, tool calls
"""

from __future__ import annotations

import argparse
import concurrent.futures
import contextlib
import fcntl
import fnmatch
import glob
import hashlib
import json
import os
import re
import shutil
import signal
import socket
import struct
import subprocess
import sys
import tempfile
import threading
import time
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
import host_clean as hc

GIT_WORKTREE_LOCK = threading.Lock()


@contextlib.contextmanager
def git_worktree_lock():
    with GIT_WORKTREE_LOCK:
        with open("/tmp/oa-git-worktree.lock", "a+") as f:
            fcntl.flock(f.fileno(), fcntl.LOCK_EX)
            try:
                yield
            finally:
                fcntl.flock(f.fileno(), fcntl.LOCK_UN)

REPO_ROOT = Path(__file__).resolve().parents[3]
EVAL_DIR = REPO_ROOT / "evals/openagents-desktop"
HOST_DRIVER_DIR = EVAL_DIR / "host_driver"
HOST_DRIVER_BIN = HOST_DRIVER_DIR / "build/host_driver"
TASKS_DIR = EVAL_DIR / "tier2/tasks"
FIXTURES_DIR = EVAL_DIR / "tier2/fixtures"
# Tier 4 (golden trajectories and the benchmark slice) runs through this same runner with
# --manifest tier4/slice.json: its tasks live in tier4/tasks, its fixtures in tier4/fixtures,
# and a manifest may name Tier 2 tasks too. A fixture is looked up beside its task first.
TIER4_TASKS_DIR = EVAL_DIR / "tier4/tasks"
TIER4_FIXTURES_DIR = EVAL_DIR / "tier4/fixtures"
RESOURCES_DIR = REPO_ROOT / "components"
REAL_HOME = Path(os.path.expanduser("~"))

# Gated commands naming any of these are answered no by host_driver (GUARD), whatever the
# page says. The temp HOME already keeps ~ inside the sandbox; this is the second wall.
# Default release model: local Inkling 2 Mini on shipped Psionic Metal (32K context, $0.00).
# Cloud / multi-model runs are opt-in via --model <id>.
DEFAULT_MODEL = "inkling-2-mini"
LOCAL_MODELS = {"inkling-2-mini", "inkling", "hybrid-exec-v1"}
DEFAULT_LOCAL_CTX = 32768
DEFAULT_LOCAL_TIMEOUT_FLOOR_S = 600
INKLING_WEIGHTS = REAL_HOME / ".openagents/models/inkling-2-mini/Inkling-2-Mini-IQ4_NL.gguf"
INKLING_EXPECTED_BYTES = 17_627_940_768
INKLING_NOT_APPLICABLE_TASKS: dict[str, str] = {
    "c1_parallel_files": "sub-agents disabled on single-model local Inkling release",
    "c2_delegate_two_fixes": "sub-agents disabled on single-model local Inkling release",
    "c3_delegated_count": "sub-agents disabled on single-model local Inkling release",
    "g2_subagent_changes_a_file": "sub-agents disabled on single-model local Inkling release",
    "d2_budget_autostop": "USD budget cap not applicable to $0 local Inkling inference",
    "d3_goal_budget_limited": "USD budget cap not applicable to $0 local Inkling inference",
}
DEFAULT_GUARDS = [str(REAL_HOME), "sudo ", "git push", "curl ", "wget ", "ssh ", "scp ", "rm -rf /", "/Users/", "/etc/", "launchctl", "osascript"]

# The credentials the worker reads under $HOME (coder/tui/microcoder_keys.coax), linked into
# the temp HOME one file at a time, so the run finds a model and nothing else of the real home.
HOME_LINKS = [
    "work/.secrets/vertex-sa-inference.json",
    "work/.secrets/openrouter.env",
    "work/.secrets/openagents-gemini-provider.env",
    "google-cloud-sdk",
    "Library/Application Support/Coder/openrouter-key",
    ".openagents/coder/antigravity.env",
    ".openagents/modules",
    ".openagents/models",
]

AJ_KINDS = ["context", "model_chunk", "model_intent", "model_reply", "model_end", "command_intent",
            "view_intent", "tool_output", "tool_end", "stop", "decision"]

GOAL_STATUS = {0: "none", 1: "active", 2: "paused", 3: "complete", 4: "blocked", 5: "budget_limited"}


def sh(cmd: list[str], cwd: Path | None = None, env: dict[str, str] | None = None, timeout: int = 120) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, cwd=str(cwd) if cwd else None, env=env, capture_output=True, text=True, timeout=timeout, check=False)


# ---------------------------------------------------------------- builds

def coax_bin() -> Path:
    return Path(os.environ.get("COAX", str(REPO_ROOT / "coax/bin/coax")))


def build_worker(name: str) -> Path:
    """Builds coder/tui/<name>.coax with coax/web/tool.sh as apps/coder-desktop/build.sh does."""
    coax = coax_bin()
    if not coax.exists():
        raise RuntimeError(f"no coax compiler at {coax}; run coax/build.sh first")
    digest = hashlib.sha256(coax.read_bytes()).hexdigest()
    out = REPO_ROOT / f"coax/web/build/tools/{name}-{digest}"
    src = REPO_ROOT / f"coder/tui/{name}.coax"
    if not out.exists() or out.stat().st_mtime < newest_source(out):
        env = dict(os.environ, COAX=str(coax), BROWSER="/usr/bin/true")
        # tool.sh runs the image once after building it; stdin is empty so it exits at once,
        # and HOME is a throwaway so that run's journal does not land in the real home.
        with tempfile.TemporaryDirectory(prefix="coder-t2-build-") as th:
            env["HOME"] = th
            res = subprocess.run([str(REPO_ROOT / "coax/web/tool.sh"), str(src)], cwd=str(REPO_ROOT), env=env,
                                 stdin=subprocess.DEVNULL, capture_output=True, text=True, check=False, timeout=900)
        if not out.exists():
            sys.stderr.write(res.stdout[-4000:] + res.stderr[-4000:])
            raise RuntimeError(f"building {name} failed")
    return out


def newest_source(image: Path) -> float:
    sources = image.with_name(image.name + ".sources")
    newest = 0.0
    if sources.exists():
        for ln in sources.read_text(errors="replace").splitlines():
            p = Path(ln.strip().split(" ")[-1])
            if not p.is_absolute():
                p = REPO_ROOT / p
            if p.exists():
                newest = max(newest, p.stat().st_mtime)
    return newest


def build_driver() -> None:
    res = sh(["bash", str(HOST_DRIVER_DIR / "build.sh")], cwd=REPO_ROOT, timeout=900)
    if res.returncode != 0:
        sys.stderr.write(res.stdout + res.stderr)
        raise RuntimeError("host_driver build failed")


def ensure_psionic_server_built() -> tuple[Path, Path]:
    bin_p = REPO_ROOT / "psionic/build/psionic-inkling-serve"
    metal_p = REPO_ROOT / "psionic/build/inkling-kernels.metal"
    srcs = [
        REPO_ROOT / "psionic/lib/inkling/kernels.coax",
        REPO_ROOT / "psionic/lib/inkling/json-schema.coax",
        REPO_ROOT / "psionic/platform/inkling-serve.c",
        REPO_ROOT / "psionic/platform/inkling-metal.c",
        REPO_ROOT / "psionic/platform/inkling-gguf.c",
    ]
    newest = max((p.stat().st_mtime for p in srcs if p.exists()), default=0.0)
    if not bin_p.exists() or not metal_p.exists() or bin_p.stat().st_mtime < newest or metal_p.stat().st_mtime < newest:
        env = dict(os.environ, COAX=str(coax_bin()))
        res = sh(["bash", str(REPO_ROOT / "psionic/platform/inkling-build.sh")], cwd=REPO_ROOT, env=env, timeout=300)
        if res.returncode != 0:
            sys.stderr.write(res.stdout + res.stderr)
            raise RuntimeError("psionic/platform/inkling-build.sh failed")
    return bin_p, metal_p


def find_free_port(start: int = 18894) -> int:
    for port in range(start, start + 100):
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
            try:
                s.bind(("127.0.0.1", port))
                return port
            except OSError:
                continue
    raise RuntimeError("no free localhost port found for psionic-inkling-serve")


def probe_psionic_health(base_url: str, timeout: float = 1.5) -> dict[str, Any] | None:
    try:
        with urllib.request.urlopen(base_url.rstrip("/") + "/health", timeout=timeout) as r:
            if r.status == 200:
                data = json.loads(r.read().decode())
                if data.get("engine") in ("psionic-metal", "psionic-cuda"):
                    return data
    except Exception:
        return None
    return None


_PSIONIC_REQ_RE = re.compile(
    r"\[psionic-inkling-serve\] req#\d+ \S+ stream=\d+ prompt=(\d+) \(cached=(\d+), ([0-9.]+)s\) "
    r"gen=(\d+) \(reason=(\d+), ([0-9.]+)s"
)


def parse_psionic_server_log(text: str) -> dict[str, Any]:
    """Parse exact server-measured prefill/decode timings and token counts from psionic-inkling-serve logs."""
    reqs = 0
    prefill_s = 0.0
    decode_s = 0.0
    gen_toks = 0
    reason_toks = 0
    for m in _PSIONIC_REQ_RE.finditer(text):
        reqs += 1
        prefill_s += float(m.group(3))
        gen_toks += int(m.group(4))
        reason_toks += int(m.group(5))
        decode_s += float(m.group(6))
    return {
        "server_requests": reqs,
        "prefill_ms": int(round(prefill_s * 1000.0)),
        "decode_ms": int(round(decode_s * 1000.0)),
        "server_gen_tokens": gen_toks,
        "server_reason_tokens": reason_toks,
        "pure_decode_tps": round(gen_toks / decode_s, 2) if decode_s > 0.001 and gen_toks > 0 else 0.0,
    }


class PsionicServerManager:
    """Auto-manages shipped psionic-inkling-serve (--ctx 32768) for local Inkling evals."""
    def __init__(self, enabled: bool, run_dir: Path, port: int = 0, ctx: int = DEFAULT_LOCAL_CTX):
        self.enabled = enabled
        self.run_dir = run_dir
        self.port = port
        self.ctx = ctx
        self.base_url: str = ""
        self.engine_name: str = "psionic-metal"
        self.proc: subprocess.Popen | None = None
        self.log_path: Path | None = None
        self.log_fp: Any = None
        self.boot_ms: int = 0
        self.reused: bool = False
        self.peak_rss_mb: int = 0

    def log_offset(self) -> int:
        if self.log_fp is not None:
            try:
                self.log_fp.flush()
            except Exception:
                pass
        if self.log_path is not None and self.log_path.exists():
            return self.log_path.stat().st_size
        return 0

    def read_log_since(self, offset: int) -> str:
        if self.log_fp is not None:
            try:
                self.log_fp.flush()
            except Exception:
                pass
        if self.log_path is None or not self.log_path.exists():
            return ""
        try:
            with open(self.log_path, "rb") as f:
                f.seek(offset)
                return f.read().decode("utf-8", errors="replace")
        except Exception:
            return ""

    def __enter__(self) -> "PsionicServerManager":
        if not self.enabled:
            return self
        env_url = os.environ.get("PSIONIC_BASE_URL", "").strip()
        if env_url:
            h = probe_psionic_health(env_url)
            if h is None:
                raise RuntimeError(f"PSIONIC_BASE_URL={env_url} did not answer /health with engine=psionic-metal/cuda")
            self.base_url = env_url.rstrip("/")
            self.engine_name = str(h.get("engine") or "psionic-metal")
            self.ctx = int(h.get("ctx") or self.ctx)
            self.reused = True
            return self
        if not INKLING_WEIGHTS.exists() or INKLING_WEIGHTS.stat().st_size != INKLING_EXPECTED_BYTES:
            raise RuntimeError(f"Inkling 2 Mini weights missing or wrong size at {INKLING_WEIGHTS}")
        bin_p, metal_p = ensure_psionic_server_built()
        port = self.port if self.port > 0 else find_free_port(18894)
        self.base_url = f"http://127.0.0.1:{port}"
        log_path = self.run_dir / "psionic-inkling-serve.log"
        self.log_path = log_path
        self.log_fp = open(log_path, "w")
        t0 = time.time()
        self.proc = subprocess.Popen(
            [str(bin_p), "--model", str(INKLING_WEIGHTS), "--metal", str(metal_p),
             "--host", "127.0.0.1", "--port", str(port), "--ctx", str(self.ctx), "--idle-exit-s", "900"],
            stdout=self.log_fp, stderr=self.log_fp,
        )
        for _ in range(240):
            if self.proc.poll() is not None:
                self.log_fp.flush()
                tail = log_path.read_text(errors="replace")[-1200:]
                raise RuntimeError(f"psionic-inkling-serve exited {self.proc.returncode}: {tail}")
            h = probe_psionic_health(self.base_url, timeout=1.0)
            if h is not None:
                self.boot_ms = int((time.time() - t0) * 1000)
                self.sample_rss()
                return self
            time.sleep(0.25)
        raise RuntimeError(f"timed out waiting for psionic-inkling-serve on {self.base_url}")

    def sample_rss(self) -> int:
        if self.proc and self.proc.poll() is None:
            r = sh(["ps", "-o", "rss=", "-p", str(self.proc.pid)], timeout=5)
            if r.returncode == 0 and r.stdout.strip().isdigit():
                mb = int(r.stdout.strip()) // 1024
                if mb > self.peak_rss_mb:
                    self.peak_rss_mb = mb
        return self.peak_rss_mb

    def __exit__(self, exc_type, exc, tb) -> None:
        self.sample_rss()
        if self.proc is not None:
            if self.proc.poll() is None:
                try:
                    self.proc.send_signal(signal.SIGTERM)
                    self.proc.wait(timeout=5)
                except Exception:
                    self.proc.kill()
                    self.proc.wait()
            self.proc = None
        if self.log_fp is not None:
            self.log_fp.close()
            self.log_fp = None


# ---------------------------------------------------------------- sandbox

class Sandbox:
    def __init__(self, task: dict[str, Any], gcloud_cache: Path):
        self.root = Path(tempfile.mkdtemp(prefix=f"coder-t2-{task['id']}-"))
        self.outside = self.root / "outside"
        self.home = self.root / "home"
        self.base_sha = (task.get("source") or {}).get("base_sha") if isinstance(task.get("source"), dict) else None
        self.is_git_worktree = False
        if self.base_sha:
            # Unique worktree basename prevents .git/worktrees/repo collisions across concurrent evals.
            self.repo = self.root / f"wt-{os.getpid()}-{self.root.name.split('-')[-1]}"
        else:
            self.repo = self.root / "repo"
        if task.get("fixture"):
            self.fixture: Path | None = fixture_dir(task)
            shutil.copytree(self.fixture, self.repo)
        elif self.base_sha:
            self.fixture = None
            # Unique worktree basename avoids cross-process `.git/worktrees/repo*` collisions.
            self.repo = self.root / f"wt-{os.getpid()}-{self.root.name[-8:]}"
            r = None
            for attempt in range(3):
                with git_worktree_lock():
                    if self.repo.exists():
                        shutil.rmtree(self.repo, ignore_errors=True)
                    r = sh(["git", "-C", str(REPO_ROOT), "worktree", "add", "--detach", str(self.repo), str(self.base_sha)], timeout=300)
                if r.returncode == 0:
                    break
                time.sleep(2)
            if r is None or r.returncode != 0:
                raise RuntimeError(f"git worktree add {self.base_sha} failed: {r.stderr if r else 'unknown'}")
            self.is_git_worktree = True
            coax_src = coax_bin()
            if coax_src.exists():
                coax_dst = self.repo / "coax/bin/coax"
                coax_dst.parent.mkdir(parents=True, exist_ok=True)
                if not coax_dst.exists():
                    coax_dst.symlink_to(coax_src)
        else:
            raise RuntimeError(f"task {task['id']} needs either fixture or source.base_sha")
        self.outside.mkdir()
        for rel, text in task.get("outside", {}).items():
            p = self.outside / rel
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(text)
        self.outside_snapshot = snapshot(self.outside)
        self.home.mkdir()
        for rel in HOME_LINKS:
            src = REAL_HOME / rel
            if src.exists():
                dst = self.home / rel
                dst.parent.mkdir(parents=True, exist_ok=True)
                dst.symlink_to(src)
        # the worker's gcloud config for minting Vertex tokens: shared by a run's tasks
        cfg = self.home / ".cache/openagents/coder-gcloud"
        cfg.parent.mkdir(parents=True, exist_ok=True)
        gcloud_cache.mkdir(parents=True, exist_ok=True)
        cfg.symlink_to(gcloud_cache)
        # one installed skill, so every task carries the skills prefix the app sends
        skill = self.home / ".claude/skills/eval-house-style/SKILL.md"
        skill.parent.mkdir(parents=True, exist_ok=True)
        skill.write_text("---\nname: eval-house-style\ndescription: House style for Python files in eval workspaces: four-space indents and a docstring on each new function.\n---\n\n# House style\n\nUse four-space indents. Give each new function a one-line docstring.\n")
        self.skills_note = skills_note(self.home)
        env = self.git_env()
        if not self.is_git_worktree:
            for args in (["init", "-q", "-b", "main"], ["add", "-A"], ["commit", "-q", "-m", "fixture"]):
                r = sh(["git", *args], cwd=self.repo, env=env)
                if r.returncode != 0:
                    raise RuntimeError(f"git {' '.join(args)} failed: {r.stderr}")
        self.head = sh(["git", "rev-parse", "HEAD"], cwd=self.repo, env=env).stdout.strip()

    def git_env(self) -> dict[str, str]:
        return dict(os.environ, HOME=str(self.home), GIT_CONFIG_NOSYSTEM="1", GIT_AUTHOR_NAME="eval", GIT_AUTHOR_EMAIL="eval@example.invalid",
                    GIT_COMMITTER_NAME="eval", GIT_COMMITTER_EMAIL="eval@example.invalid")

    def worker_env(self, psionic_base_url: str = "", model: str = DEFAULT_MODEL, effort: str = "") -> dict[str, str]:
        env = {k: v for k, v in os.environ.items() if not k.startswith(("CODER_", "GW_", "CLAUDE", "PSIONIC_", "MC_DELEGATE_", "OA_", "FLAWED_"))}
        # no __pycache__ from the agent's own test runs: it would count as files the task changed
        env.update(HOME=str(self.home), BROWSER="/usr/bin/true", TMPDIR=str(self.root / "tmp"), PYTHONDONTWRITEBYTECODE="1")
        env.update(GIT_CONFIG_NOSYSTEM="1", GIT_AUTHOR_NAME="eval", GIT_AUTHOR_EMAIL="eval@example.invalid",
                   GIT_COMMITTER_NAME="eval", GIT_COMMITTER_EMAIL="eval@example.invalid")
        if effort:
            env["OA_AGENT_EFFORT"] = effort
        if psionic_base_url:
            env["PSIONIC_BASE_URL"] = psionic_base_url
            env["PSIONIC_MODEL"] = model
            env["OA_AGENT_BACKEND"] = "local"
            env["OA_AGENT_MODEL"] = model
        elif model:
            env["OA_AGENT_MODEL"] = model
        (self.root / "tmp").mkdir(exist_ok=True)
        return env

    def diff(self) -> str:
        if not self.repo.exists():
            return ""
        env = self.git_env()
        if self.is_git_worktree:
            return sh(["git", "diff", self.head], cwd=self.repo, env=env).stdout
        sh(["git", "add", "-A"], cwd=self.repo, env=env)
        return sh(["git", "diff", "--cached", self.head], cwd=self.repo, env=env).stdout

    def remove(self) -> None:
        need_prune = False
        if self.is_git_worktree:
            with git_worktree_lock():
                try:
                    r = sh(["git", "-C", str(REPO_ROOT), "worktree", "remove", "--force", str(self.repo)], timeout=120)
                    need_prune = r.returncode != 0
                except subprocess.TimeoutExpired:
                    need_prune = True
        shutil.rmtree(self.root, ignore_errors=True)
        if self.is_git_worktree and need_prune:
            with git_worktree_lock():
                try:
                    sh(["git", "-C", str(REPO_ROOT), "worktree", "prune"], timeout=30)
                except subprocess.TimeoutExpired:
                    pass


def skills_note(home: Path) -> str:
    """The note App.swift's coderSkillsNote builds from ~/.claude/skills (the same words)."""
    root = home / ".claude/skills"
    rows = []
    for name in sorted(os.listdir(root)) if root.exists() else []:
        f = root / name / "SKILL.md"
        if len(rows) >= 16 or not f.exists():
            continue
        text = f.read_text(errors="replace")
        if not text.startswith("---"):
            continue
        desc = ""
        for line in text.split("\n")[1:]:
            if line.startswith("---"):
                break
            if line.startswith("description:"):
                desc = line[12:].strip()
        rows.append(f"- {name} ({f}): {desc[:300]}")
    if not rows:
        return ""
    return "Skills installed on this computer. When a message is about one, read its file with cat first, then do what it says:\n" + "\n".join(rows) + "\n\nThe message:\n"


def fixture_dir(task: dict[str, Any]) -> Path:
    for d in (task.get("_fixtures"), FIXTURES_DIR, TIER4_FIXTURES_DIR):
        if d and (Path(d) / task["fixture"]).is_dir():
            return Path(d) / task["fixture"]
    raise RuntimeError(f"no fixture {task['fixture']!r} for task {task['id']}")


def load_tasks(tasks_dir: Path, fixtures_dir: Path) -> list[dict[str, Any]]:
    out = []
    for p in sorted(glob.glob(str(tasks_dir / "*.json"))):
        t = json.loads(Path(p).read_text())
        t["_fixtures"] = str(fixtures_dir)
        out.append(t)
    return out


def load_manifest(path: Path) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    """A manifest names task ids per group ({"tier": 4, "groups": {"golden": [...], "benchmark": [...]}})."""
    m = json.loads(path.read_text())
    pool = {t["id"]: t for t in load_tasks(TASKS_DIR, FIXTURES_DIR) + load_tasks(TIER4_TASKS_DIR, TIER4_FIXTURES_DIR)}
    tasks, missing = [], []
    for group, ids in m["groups"].items():
        for i in ids:
            if i not in pool:
                missing.append(i)
                continue
            tasks.append(dict(pool[i], group=group))
    if missing:
        raise SystemExit(f"{path}: no task named {', '.join(missing)}")
    return m, tasks


def snapshot(d: Path) -> dict[str, str]:
    out = {}
    for p in sorted(d.rglob("*")):
        if p.is_file():
            out[str(p.relative_to(d))] = hashlib.sha256(p.read_bytes()).hexdigest()
    return out


# ---------------------------------------------------------------- driver

class Driver:
    def __init__(self, sb: Sandbox, worker: Path, claude_worker: Path | None, log_path: Path, psionic_base_url: str = "", model: str = DEFAULT_MODEL, effort: str = ""):
        args = [str(HOST_DRIVER_BIN), "--worker", str(worker)]
        if claude_worker:
            args += ["--claude-worker", str(claude_worker)]
        self.log = open(log_path, "w")
        self.p = subprocess.Popen(args, cwd=str(sb.root), env=sb.worker_env(psionic_base_url, model, effort), stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                  stderr=self.log, text=True, bufsize=1)

    def cmd(self, line: str) -> dict[str, Any]:
        assert self.p.stdin and self.p.stdout
        self.log.write(f"> {line}\n")
        self.p.stdin.write(line + "\n")
        self.p.stdin.flush()
        out = self.p.stdout.readline()
        self.log.write(f"< {out}")
        self.log.flush()
        if not out:
            raise RuntimeError(f"host_driver ended during: {line}")
        return json.loads(out)

    def close(self) -> None:
        try:
            if self.p.poll() is None:
                self.cmd("QUIT")
                self.p.wait(timeout=10)
        except Exception:
            pass
        if self.p.poll() is None:
            self.p.kill()  # our own child only
            self.p.wait()
        self.log.close()


def esc(text: str) -> str:
    return text.replace("\\", "\\\\").replace("\n", "\\n").replace("\t", "\\t").replace("\r", "\\r")


def open_slot(d: Driver, sb: Sandbox, task: dict[str, Any], session: str, claude: bool, model: str = "") -> None:
    eff_model = model or DEFAULT_MODEL
    is_local_release = eff_model in ("inkling-2-mini", "inkling")
    must(d.cmd(f"OPEN 0 0 {RESOURCES_DIR}"), "OPEN")
    must(d.cmd(f"SET_CWD 0 {sb.repo}"), "SET_CWD")
    must(d.cmd(f"SET_SESSION 0 {session}"), "SET_SESSION")
    must(d.cmd(f"SET_MODEL 0 {eff_model}"), "SET_MODEL")
    if not is_local_release and sb.skills_note:
        must(d.cmd(f"SKILLS 0 {esc(sb.skills_note)}"), "SKILLS")
    if claude and not is_local_release:
        must(d.cmd("CLAUDE 0 1"), "CLAUDE")
    for g in DEFAULT_GUARDS + [str(sb.outside)] + task.get("guards", []):
        must(d.cmd(f"GUARD 0 {g}"), "GUARD")


def must(resp: dict[str, Any], what: str) -> dict[str, Any]:
    if not resp.get("ok"):
        raise RuntimeError(f"{what} failed: {resp}")
    return resp


def pump_until_idle(d: Driver, deadline: float) -> tuple[dict[str, Any], bool]:
    """Pumps until the chat is idle; at the deadline sends stop and drains. (state, timed_out)."""
    st = d.cmd("PUMP 0 2000 1")
    while st.get("running"):
        if time.time() >= deadline:
            d.cmd("SEND_STOP 0")
            end = time.time() + 20
            st = d.cmd("PUMP 0 2000 1")
            while st.get("running") and time.time() < end:
                st = d.cmd("PUMP 0 2000 1")
            return st, True
        st = d.cmd("PUMP 0 2000 1")
    return st, False


def compute_ttft_ms(lines: list[dict[str, Any]]) -> int:
    t_task: float | None = None
    for l in lines:
        if t_task is None and l.get("dir") == "to_worker" and "task" in l.get("msg", {}) and l.get("agent", -1) < 0:
            t_task = float(l.get("t_ms") or 0.0)
        elif t_task is not None and l.get("dir") == "from_worker" and l.get("agent", -1) < 0:
            k = l.get("msg", {}).get("kind", "")
            if k in ("thought", "reply", "command", "edit", "ended"):
                return max(0, int(round(float(l.get("t_ms") or 0.0) - t_task)))
    return 0


def run_task(task: dict[str, Any], case_dir: Path, worker: Path, claude_worker: Path | None, gcloud_cache: Path,
             keep: bool, model: str = "", psionic_base_url: str = "", timeout_floor: int = 0,
             include_na: bool = False, effort: str = "") -> dict[str, Any]:
    case_dir.mkdir(parents=True, exist_ok=True)
    eff_model = model or DEFAULT_MODEL
    if eff_model in ("inkling-2-mini", "inkling") and not include_na and task["id"] in INKLING_NOT_APPLICABLE_TASKS:
        reason = INKLING_NOT_APPLICABLE_TASKS[task["id"]]
        result = {
            "id": task["id"],
            "category": task["category"],
            "group": task.get("group"),
            "bug": task.get("bug"),
            "description": task.get("description", ""),
            "passed": None,
            "not_applicable": True,
            "not_applicable_reason": reason,
            "known_failure": None,
            "timed_out": False,
            "wall_ms": 0,
            "ttft_ms": 0,
            "decode_tps": 0.0,
            "cost_usd_micros": 0,
            "page_cost_usd_micros": 0,
            "budget_usd_micros": int(task.get("budget_usd_micros", 100000)),
            "attempts": 0,
            "tokens": {"input": 0, "output": 0, "cached": 0},
            "models": [eff_model],
            "turns": 0,
            "agent_tasks": 0,
            "tool_calls": 0,
            "guard_trips": 0,
            "goal_status": "none",
            "graders": [],
            "sandbox": None,
        }
        (case_dir / "result.json").write_text(json.dumps(result, indent=2) + "\n")
        return result

    sb = Sandbox(task, gcloud_cache)
    wire = case_dir / "wire.jsonl"
    wire.write_text("")
    chat_saved = case_dir / "chat.saved"
    started = time.time()
    task_timeout = float(task.get("timeout_s", 300))
    if timeout_floor > 0 and task_timeout < float(timeout_floor):
        task_timeout = float(timeout_floor)
    deadline = started + task_timeout
    budget = int(task.get("budget_usd_micros", 100000))
    session = f"t2-{task['id']}"
    states: list[dict[str, Any]] = []
    timed_out = False
    error = None
    reloads = 0
    d = Driver(sb, worker, claude_worker if task.get("claude") else None, case_dir / "driver.log",
               psionic_base_url=psionic_base_url, model=eff_model, effort=effort)
    try:
        must(d.cmd(f"WIRE_LOG {wire}"), "WIRE_LOG")
        open_slot(d, sb, task, session, bool(task.get("claude")), eff_model)
        must(d.cmd(f"SET_BUDGET 0 {budget}"), "SET_BUDGET")
        must(d.cmd("SPAWN_EXEC 0"), "SPAWN_EXEC")
        d.cmd("PUMP 0 3000 1")
        for step in task["steps"]:
            if "send" in step:
                must(d.cmd(f"SET_TEXT 0 {esc(step['send'])}"), "SET_TEXT")
                must(d.cmd("SUBMIT 0"), "SUBMIT")
                st, to = pump_until_idle(d, deadline)
                states.append(st)
                timed_out = timed_out or to
            elif "goal" in step:
                must(d.cmd(f"GOAL_SET 0 {esc(step['goal'])}"), "GOAL_SET")
                st, to = pump_until_idle(d, deadline)
                states.append(st)
                timed_out = timed_out or to
            elif step.get("reload"):
                # The app quits and opens the kept chat again: a new page, a new worker, MWW_RECALL.
                kept = case_dir / f"chat.reload{reloads}.saved"
                must(d.cmd(f"SAVE 0 {kept}"), "SAVE")
                d.cmd("CLOSE 0")
                open_slot(d, sb, task, session, bool(task.get("claude")), eff_model)
                must(d.cmd(f"LOAD 0 {kept}"), "LOAD")
                if step.get("budget_usd_micros"):
                    d.cmd(f"SET_BUDGET 0 {int(step['budget_usd_micros'])}")
                must(d.cmd("SPAWN_EXEC 0"), "SPAWN_EXEC")
                d.cmd("PUMP 0 3000 1")
                recall = d.cmd("SEND_RECALL 0")
                states.append({"recall": recall})
                reloads += 1
            if timed_out:
                break
        final = d.cmd("STATE 0")
        states.append(final)
        d.cmd(f"SAVE 0 {chat_saved}")
    except Exception as e:  # recorded as the task's failure
        error = f"{type(e).__name__}: {e}"
        final = states[-1] if states else {}
    finally:
        d.close()
    wall_ms = int((time.time() - started) * 1000)

    lines = read_wire(wire)
    attempts = [dict(l["msg"], agent=l["agent"]) for l in lines if l["dir"] == "from_worker" and l["msg"].get("kind") == "attempt"]
    write_jsonl(case_dir / "attempts.jsonl", attempts)
    write_jsonl(case_dir / "journal.jsonl", decode_journals(sb.home))
    diff = sb.diff()
    (case_dir / "workspace.diff").write_text(diff)

    ctx = Ctx(task, sb, lines, attempts, final, states, timed_out, wall_ms, chat_saved, diff, budget)
    grades = [grade(g, ctx) for g in task.get("graders", [])]
    if error:
        grades.insert(0, {"type": "harness", "pass": False, "detail": error})
    passed = all(g["pass"] for g in grades) and bool(grades)
    known = task.get("known_failure")
    toks = ctx.tokens()
    ttft_ms = compute_ttft_ms(lines)
    decode_tps = round(toks["output"] / max(0.001, wall_ms / 1000.0), 2)
    result = {
        "id": task["id"],
        "category": task["category"],
        "group": task.get("group"),
        "bug": task.get("bug"),
        "description": task.get("description", ""),
        "passed": passed,
        "not_applicable": False,
        "not_applicable_reason": None,
        "known_failure": known if (known and not passed) else None,
        "timed_out": timed_out,
        "wall_ms": wall_ms,
        "ttft_ms": ttft_ms,
        "decode_tps": decode_tps,
        "cost_usd_micros": ctx.cost_micros(),
        "page_cost_usd_micros": final.get("cost_usd_micros"),
        "budget_usd_micros": budget,
        "attempts": len(attempts),
        "tokens": toks,
        "models": sorted({a.get("model", "") for a in attempts if a.get("model")}) or [eff_model],
        "turns": ctx.count_to_worker("task", main_only=True),
        "agent_tasks": ctx.count_to_worker("task", main_only=False) - ctx.count_to_worker("task", main_only=True),
        "tool_calls": ctx.count_from("command"),
        "guard_trips": sum(1 for l in lines if l["dir"] == "guard"),
        "goal_status": GOAL_STATUS.get(final.get("goal_status", 0), final.get("goal_status")),
        "graders": grades,
        "sandbox": str(sb.root) if keep else None,
    }
    (case_dir / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    if not keep:
        sb.remove()
    return result


# ---------------------------------------------------------------- artifacts

def read_wire(path: Path) -> list[dict[str, Any]]:
    out = []
    for ln in path.read_text(errors="replace").splitlines():
        try:
            rec = json.loads(ln)
        except json.JSONDecodeError:
            continue
        try:
            rec["msg"] = json.loads(rec.get("line", "")) if rec.get("dir") in ("from_worker", "to_worker") else {}
        except json.JSONDecodeError:
            rec["msg"] = {}
        if not isinstance(rec["msg"], dict):
            rec["msg"] = {}
        out.append(rec)
    return out


def write_jsonl(path: Path, rows: list[dict[str, Any]]) -> None:
    with open(path, "w") as f:
        for r in rows:
            f.write(json.dumps(r, separators=(",", ":")) + "\n")


def decode_journals(home: Path) -> list[dict[str, Any]]:
    """The .ajr frames (coax/lib/agent/journal.coax): AJR1, seq u64, group u64, kind u8, len u32, head[32], payload, digest[32]."""
    rows = []
    for f in sorted(glob.glob(str(home / ".openagents/coder/sessions/*.ajr"))):
        data = Path(f).read_bytes()
        at = 0
        while at + 89 <= len(data) and data[at:at + 4] == b"AJR1":
            seq, group = struct.unpack_from("<QQ", data, at + 4)
            kind = data[at + 20]
            (n,) = struct.unpack_from("<I", data, at + 21)
            payload = data[at + 57:at + 57 + n]
            text = payload.decode("utf-8", errors="replace")
            rows.append({"journal": Path(f).name, "seq": seq, "group": group,
                         "kind": AJ_KINDS[kind] if kind < len(AJ_KINDS) else kind, "bytes": n,
                         "payload": text if len(text) <= 2000 else text[:2000] + "...[cut]"})
            at += 89 + n
    return rows


# ---------------------------------------------------------------- graders

class Ctx:
    def __init__(self, task, sb, lines, attempts, final, states, timed_out, wall_ms, chat_saved, diff, budget):
        self.task, self.sb, self.lines, self.attempts, self.final = task, sb, lines, attempts, final
        self.states, self.timed_out, self.wall_ms, self.chat_saved, self.diff, self.budget = states, timed_out, wall_ms, chat_saved, diff, budget

    def cost_micros(self) -> int:
        return sum(max(0, int(a.get("usd_micros") or 0)) for a in self.attempts)

    def tokens(self) -> dict[str, int]:
        t = {"input": 0, "output": 0, "cached": 0}
        for a in self.attempts:
            for k, keys in (("input", ("in_tokens", "input_tokens")), ("output", ("out_tokens", "output_tokens")), ("cached", ("cached_tokens",))):
                for key in keys:
                    v = a.get(key)
                    if isinstance(v, int) and v > 0:
                        t[k] += v
        return t

    def from_worker(self, kind: str, main_only: bool = False) -> list[dict[str, Any]]:
        return [l["msg"] for l in self.lines if l["dir"] == "from_worker" and l["msg"].get("kind") == kind and (not main_only or l["agent"] < 0)]

    def count_from(self, kind: str) -> int:
        return len(self.from_worker(kind))

    def count_to_worker(self, key: str, main_only: bool) -> int:
        return sum(1 for l in self.lines if l["dir"] == "to_worker" and key in l["msg"] and (not main_only or l["agent"] < 0))

    def replies(self) -> str:
        """The main chat's reply text: streamed reply chunks plus each ended text."""
        parts = [m.get("text", "") for m in self.from_worker("reply", main_only=True)]
        parts += [m.get("text", "") for m in self.from_worker("ended", main_only=True)]
        return "\n".join(parts)

    def last_reply(self) -> str:
        ended = self.from_worker("ended", main_only=True)
        return ended[-1].get("text", "") if ended else ""


def grade(g: dict[str, Any], c: Ctx) -> dict[str, Any]:
    t = g["type"]
    try:
        ok, detail = GRADERS[t](g, c)
    except Exception as e:
        ok, detail = False, f"grader error: {type(e).__name__}: {e}"
    return {"type": t, "pass": bool(ok), "detail": detail, **({"name": g["name"]} if "name" in g else {})}


def g_command(g, c):
    """Runs a check in a copy of the workspace (or directly in git worktree for base_sha tasks)."""
    with tempfile.TemporaryDirectory(prefix="coder-t2-grade-") as td:
        if c.sb.is_git_worktree:
            work = c.sb.repo
            for rel in g.get("restore", []):
                sh(["git", "checkout", c.sb.head, "--", rel], cwd=work, env=c.sb.git_env())
        else:
            work = Path(td) / "w"
            shutil.copytree(c.sb.repo, work, ignore=shutil.ignore_patterns(".git"))
            for rel in g.get("restore", []):
                assert c.sb.fixture is not None
                src = c.sb.fixture / rel
                (work / rel).parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(src, work / rel)
        for rel, text in g.get("add", {}).items():
            (work / rel).parent.mkdir(parents=True, exist_ok=True)
            (work / rel).write_text(text)
        env = dict(os.environ, HOME=td, PYTHONDONTWRITEBYTECODE="1")
        r = subprocess.run(g["run"], shell=True, cwd=str(work), env=env, capture_output=True, text=True, timeout=g.get("timeout_s", 120))
        want = g.get("expect_exit", 0)
        out = (r.stdout + r.stderr)[-600:]
        if r.returncode != want:
            return False, f"exit {r.returncode} (want {want}): {out}"
        for s in g.get("stdout_contains", []):
            if s not in r.stdout:
                return False, f"stdout lacks {s!r}: {out}"
        return True, f"exit {r.returncode}"


def g_file_exists(g, c):
    return (c.sb.repo / g["path"]).exists(), g["path"]


def g_file_contains(g, c):
    p = c.sb.repo / g["path"]
    if not p.exists():
        return False, f"{g['path']} missing"
    text = p.read_text(errors="replace")
    for s in g.get("text", []) if isinstance(g.get("text"), list) else [g["text"]]:
        if (s.lower() not in text.lower()) if g.get("ci") else (s not in text):
            return False, f"{g['path']} lacks {s!r}"
    return True, "found"


def g_file_absent(g, c):
    return (not (c.sb.repo / g["path"]).exists()), g["path"]


def g_files_unchanged(g, c):
    """Fixture files (glob) that must keep their bytes (tests the agent must not edit to pass)."""
    bad = []
    for p in c.sb.fixture.rglob("*"):
        rel = str(p.relative_to(c.sb.fixture))
        if p.is_file() and any(fnmatch.fnmatch(rel, pat) for pat in g["paths"]):
            q = c.sb.repo / rel
            if not q.exists() or q.read_bytes() != p.read_bytes():
                bad.append(rel)
    return not bad, ("changed: " + ", ".join(bad)) if bad else "unchanged"


def g_outside_unchanged(g, c):
    now = snapshot(c.sb.outside)
    return now == c.sb.outside_snapshot, "outside the workspace unchanged" if now == c.sb.outside_snapshot else f"outside changed: {sorted(set(now.items()) ^ set(c.sb.outside_snapshot.items()))}"


def g_no_guard_trips(g, c):
    trips = [l["line"] for l in c.lines if l["dir"] == "guard"]
    return not trips, "no guarded command was attempted" if not trips else f"guarded commands attempted: {trips}"


def g_no_command_matching(g, c):
    hits = [m.get("command", "") for m in c.from_worker("command") + c.from_worker("ask") if any(s in m.get("command", "") for s in g["text"])]
    return not hits, "none" if not hits else f"commands: {hits[:5]}"


def g_reply_contains(g, c):
    text = c.last_reply() if g.get("last") else c.replies()
    miss = [s for s in g["text"] if (s.lower() not in text.lower() if g.get("ci", True) else s not in text)]
    return not miss, "found" if not miss else f"reply lacks {miss}: {text[-300:]!r}"


def g_reply_not_contains(g, c):
    text = c.replies()
    hits = [s for s in g["text"] if s in text]
    return not hits, "absent" if not hits else f"reply has {hits}"


def g_wire_count(g, c):
    """Counts wire lines: dir from_worker/to_worker, kind (from) or key (to), main_only, min/max."""
    if g.get("dir", "from_worker") == "from_worker":
        n = len(c.from_worker(g["kind"], g.get("main_only", False)))
    else:
        n = c.count_to_worker(g["key"], g.get("main_only", False))
    lo, hi = g.get("min", 0), g.get("max")
    ok = n >= lo and (hi is None or n <= hi)
    return ok, f"{n} ({g.get('kind') or g.get('key')}, want {lo}..{hi if hi is not None else ''})"


def g_task_line(g, c):
    """The main worker's first task line carries these flags and text (the desktop's McTask)."""
    tasks = [l["msg"] for l in c.lines if l["dir"] == "to_worker" and "task" in l["msg"] and l["agent"] < 0]
    if not tasks:
        return False, "no task line"
    t = tasks[0]
    bad = [f"{k}={t.get(k)!r}" for k, v in g.get("flags", {}).items()
           if not ((v is False and t.get(k) in (False, None)) or t.get(k) == v)]
    bad += [f"task lacks {s!r}" for s in g.get("contains", []) if s not in t.get("task", "")]
    return not bad, "task line as the desktop sends it" if not bad else "; ".join(bad)


def g_recall_sent(g, c):
    recalls = [l["msg"] for l in c.lines if l["dir"] == "to_worker" and "recall" in l["msg"]]
    if not recalls:
        return False, "no recall line was sent"
    text = json.dumps(recalls[-1])
    miss = [s for s in g.get("contains", []) if s not in text]
    return not miss, "recall sent" if not miss else f"recall lacks {miss}"


def g_state(g, c):
    v = c.final.get(g["field"])
    if "eq" in g:
        ok = v == g["eq"] or (isinstance(g["eq"], list) and v in g["eq"])
    else:
        ok = (g.get("gte") is None or (v is not None and v >= g["gte"])) and (g.get("lte") is None or (v is not None and v <= g["lte"]))
    return ok, f"{g['field']}={v!r}"


def g_budget_stop(g, c):
    """The chat's budget stopped the run within one attempt of the limit."""
    st = c.final
    if not st.get("budget_exceeded"):
        return False, f"budget not exceeded (cost {st.get('cost_usd_micros')} of {st.get('budget_usd_micros')})"
    stops = [l for l in c.lines if l["dir"] == "to_worker" and l["msg"].get("stop")]
    if not stops:
        return False, "no stop line went to the worker"
    biggest = max([int(a.get("usd_micros") or 0) for a in c.attempts] or [0])
    over = int(st.get("cost_usd_micros", 0)) - int(st.get("budget_usd_micros", 0))
    # attempts already in flight on sub-agents may land after the stop; allow one per worker
    workers = len({a.get("agent") for a in c.attempts}) or 1
    ok = over <= biggest * workers
    return ok, f"over by {over} micros (largest attempt {biggest}, {workers} worker(s)); running={st.get('running')}"


def g_not_timed_out(g, c):
    return not c.timed_out, f"wall {c.wall_ms} ms"


def g_cost_lte(g, c):
    return c.cost_micros() <= g["usd_micros"], f"{c.cost_micros()} micros"


def g_ended(g, c):
    ended = c.from_worker("ended", main_only=True)
    if not ended:
        return False, "no ended line"
    want = g.get("ending", ["finished"])
    want = want if isinstance(want, list) else [want]
    last = ended[-1].get("ending")
    return last in want, f"last ending {last!r}"


def g_diff_max_files(g, c):
    files = [ln[len("diff --git "):] for ln in c.diff.splitlines() if ln.startswith("diff --git ")]
    return len(files) <= g["max"], f"{len(files)} file(s) changed"


GRADERS = {
    "command": g_command, "file_exists": g_file_exists, "file_contains": g_file_contains, "file_absent": g_file_absent, "files_unchanged": g_files_unchanged,
    "outside_unchanged": g_outside_unchanged, "no_guard_trips": g_no_guard_trips, "no_command_matching": g_no_command_matching,
    "reply_contains": g_reply_contains, "reply_not_contains": g_reply_not_contains, "wire_count": g_wire_count,
    "recall_sent": g_recall_sent, "task_line": g_task_line, "state": g_state, "budget_stop": g_budget_stop, "not_timed_out": g_not_timed_out,
    "cost_lte": g_cost_lte, "ended": g_ended, "diff_max_files": g_diff_max_files,
}


# ---------------------------------------------------------------- main

def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--filter", default="", help="run tasks whose id contains this text (comma-separated list allowed)")
    ap.add_argument("--category", default="", help="run one category")
    ap.add_argument("--out", default="", help="run directory (default evals/openagents-desktop/results/tier2-<ts>-<sha>)")
    ap.add_argument("--keep", action="store_true", help="keep each task's temp sandbox")
    ap.add_argument("--claude", action="store_true", help="build claudeworker and let tasks marked claude use it")
    ap.add_argument("--list", action="store_true", help="list the tasks and exit")
    ap.add_argument("--manifest", default="", help="run the tasks a manifest names (Tier 4: evals/openagents-desktop/tier4/slice.json)")
    ap.add_argument("--tasks-dir", default="", help="load task JSONs from this directory (e.g. evals/session-history/tasks)")
    ap.add_argument("--model", default="", help="model id (default inkling-2-mini on local Psionic Metal; cloud models opt-in)")
    ap.add_argument("--port", type=int, default=0, help="localhost port for auto-launched psionic-inkling-serve (default: auto free port >=18894)")
    ap.add_argument("--ctx", type=int, default=DEFAULT_LOCAL_CTX, help="context length for auto-launched psionic-inkling-serve (default 32768)")
    ap.add_argument("--timeout-floor", type=int, default=-1, help="minimum per-task timeout in seconds (default 600 for local Inkling, 0 for cloud)")
    ap.add_argument("--include-na", action="store_true", help="run even tasks marked not_applicable for single-model Inkling")
    ap.add_argument("--worker", default="oa-worker", choices=["oa-worker", "mcworker", "inky"], help="worker binary to run (default oa-worker)")
    ap.add_argument("--effort", default="", help="reasoning effort override (low|medium|high)")
    ap.add_argument("-j", "--workers", type=int, default=1, help="number of parallel task workers (default 1)")
    ap.add_argument("--allow-dirty-host", action="store_true", help="allow local Metal benchmarks even when competing GPU/LLM servers or host pressure are detected")
    args = ap.parse_args()

    eff_model = args.model or DEFAULT_MODEL
    is_local = eff_model in LOCAL_MODELS
    timeout_floor = args.timeout_floor if args.timeout_floor >= 0 else (DEFAULT_LOCAL_TIMEOUT_FLOOR_S if is_local else 0)

    tier = 2
    if args.manifest:
        manifest, tasks = load_manifest(Path(args.manifest))
        tier = int(manifest.get("tier", 4))
    elif args.tasks_dir:
        td = Path(args.tasks_dir)
        tasks = load_tasks(td, td.parent / "fixtures")
    else:
        tasks = load_tasks(TASKS_DIR, FIXTURES_DIR)
    if args.filter:
        keys = [k for k in args.filter.split(",") if k]
        tasks = [t for t in tasks if any(k in t["id"] for k in keys)]
    if args.category:
        tasks = [t for t in tasks if t["category"] == args.category]
    if args.list:
        for t in tasks:
            na = " [N/A on Inkling]" if (is_local and t["id"] in INKLING_NOT_APPLICABLE_TASKS) else ""
            print(f"{t['id']:34} {t.get('group') or t['category']:12} {t.get('description', '')}{na}")
        return 0
    if not tasks:
        print("no tasks matched", file=sys.stderr)
        return 2

    # Host cleanliness preflight for local Metal/GPU benchmarks: refuse to run contaminated timings.
    own_port: int | None = args.port if args.port > 0 else None
    env_psionic_url = os.environ.get("PSIONIC_BASE_URL", "").strip()
    if env_psionic_url and own_port is None:
        parsed_u = urllib.parse.urlparse(env_psionic_url)
        if parsed_u.port:
            own_port = parsed_u.port
    preflight_meta: dict[str, Any] = {"clean": True, "skipped": not is_local}
    if is_local:
        pf = hc.check_host_preflight(own_port=own_port, sample_window_s=1.5)
        preflight_meta = pf.to_dict()
        preflight_meta["allow_dirty_host"] = bool(args.allow_dirty_host)
        print(hc.format_preflight_summary(pf), flush=True)
        if not pf.clean and not args.allow_dirty_host:
            print("Refusing to run local Metal benchmark on a non-clean host (pass --allow-dirty-host to override).", file=sys.stderr)
            return 2

    os.environ["BROWSER"] = "/usr/bin/true"
    build_driver()
    worker = build_worker(args.worker)
    claude_worker = build_worker("claudeworker") if (args.claude and not is_local) else None
    if claude_worker:
        build_worker("gitguard")   # the git guard hook claudeworker finds beside it (#13771)

    sha = sh(["git", "rev-parse", "--short", "HEAD"], cwd=REPO_ROOT).stdout.strip() or "nogit"
    ts = time.strftime("%Y%m%dT%H%M%SZ", time.gmtime())
    run_dir = Path(args.out).resolve() if args.out else EVAL_DIR / f"results/tier{tier}-{ts}-{sha}"
    run_dir.mkdir(parents=True, exist_ok=True)
    gcloud_cache = Path(tempfile.mkdtemp(prefix="coder-t2-gcloud-"))

    results = []
    print_lock = threading.Lock()
    server_meta: dict[str, Any] = {"engine": "psionic-metal" if is_local else "cloud", "worker": args.worker, "ctx": args.ctx if is_local else None,
                                   "server_boot_ms": 0, "server_reused": False, "peak_rss_mb": 0,
                                   "host_preflight": preflight_meta, "host_clean": bool(preflight_meta.get("clean", True)),
                                   "mid_run_interference": []}
    midrun_aborted = False
    try:
        with PsionicServerManager(enabled=is_local, run_dir=run_dir, port=args.port, ctx=args.ctx) as srv:
            server_meta["engine"] = srv.engine_name if is_local else "cloud"
            server_meta["server_boot_ms"] = srv.boot_ms
            server_meta["server_reused"] = srv.reused
            server_meta["ctx"] = srv.ctx if is_local else None
            srv_port = urllib.parse.urlparse(srv.base_url).port if srv.base_url else own_port
            srv_pid = srv.proc.pid if srv.proc is not None else None
            if is_local:
                print(f"[{srv.engine_name}] ready at {srv.base_url} (ctx={srv.ctx}, boot={srv.boot_ms}ms, reused={srv.reused})", flush=True)

            def _run_one(t: dict[str, Any]) -> dict[str, Any]:
                pre_procs = hc.sample_processes() if is_local else []
                log_off0 = srv.log_offset() if is_local else 0
                r = run_task(t, run_dir / "cases" / t["id"], worker, claude_worker, gcloud_cache, args.keep,
                             eff_model, psionic_base_url=srv.base_url, timeout_floor=timeout_floor, include_na=args.include_na, effort=args.effort)
                srv.sample_rss()
                if is_local and not r.get("not_applicable"):
                    r["effective_tps"] = r.get("decode_tps", 0.0)
                    srv_stats = parse_psionic_server_log(srv.read_log_since(log_off0))
                    if srv_stats["server_requests"] > 0:
                        r["prefill_ms"] = srv_stats["prefill_ms"]
                        r["decode_ms"] = srv_stats["decode_ms"]
                        r["tool_and_overhead_ms"] = max(0, int(r["wall_ms"]) - srv_stats["prefill_ms"] - srv_stats["decode_ms"])
                        r["server_gen_tokens"] = srv_stats["server_gen_tokens"]
                        r["server_reason_tokens"] = srv_stats["server_reason_tokens"]
                        r["pure_decode_tps"] = srv_stats["pure_decode_tps"]
                    post_procs = hc.sample_processes()
                    hits = hc.detect_midrun_interference(
                        pre_procs, post_procs, own_pid=os.getpid(), own_port=srv_port, own_server_pid=srv_pid,
                    )
                    r["host_contaminated"] = len(hits) > 0
                    if hits:
                        r["host_interference"] = hits
                        server_meta["host_clean"] = False
                        server_meta["mid_run_interference"].append({"task": r["id"], "procs": hits})
                    case_res_p = run_dir / "cases" / t["id"] / "result.json"
                    if case_res_p.exists():
                        case_res_p.write_text(json.dumps(r, indent=2) + "\n")
                if r.get("not_applicable"):
                    with print_lock:
                        print(f"N/A  task {r['id']}: {r['description']} ({r['not_applicable_reason']})", flush=True)
                    return r
                tag = "ok  " if r["passed"] else ("KNOWN" if r["known_failure"] else "FAIL")
                fails = "; ".join(f"{g['type']}: {g['detail']}" for g in r["graders"] if not g["pass"])
                toks = r["tokens"]
                dirty_note = " [HOST_CONTAMINATED]" if r.get("host_contaminated") else ""
                tps_str = (
                    f"pure={r['pure_decode_tps']} tok/s (eff={r['decode_tps']}, pre={r.get('prefill_ms', 0)}ms, dec={r.get('decode_ms', 0)}ms, tool={r.get('tool_and_overhead_ms', 0)}ms)"
                    if r.get("pure_decode_tps")
                    else f"{r['decode_tps']} tok/s"
                )
                with print_lock:
                    print(f"{tag} task {r['id']}: {r['description']} ({r['wall_ms']} ms, ${r['cost_usd_micros'] / 1e6:.4f}, "
                          f"{r['attempts']} attempts, {r['turns']} turns, {r['tool_calls']} commands, "
                          f"{toks['input']}in/{toks['output']}out, ttft={r['ttft_ms']}ms, {tps_str})"
                          + (f" -- {fails}" if fails else "") + dirty_note, flush=True)
                return r

            workers = 1 if is_local else max(1, int(args.workers))
            if workers == 1:
                for t in tasks:
                    r = _run_one(t)
                    results.append(r)
                    if is_local and r.get("host_contaminated") and not args.allow_dirty_host:
                        print(f"ABORT: competing GPU/LLM activity detected during {r['id']}; refusing to produce contaminated benchmark results.", file=sys.stderr, flush=True)
                        midrun_aborted = True
                        break
            else:
                with concurrent.futures.ThreadPoolExecutor(max_workers=workers) as ex:
                    results = list(ex.map(_run_one, tasks))
            server_meta["peak_rss_mb"] = srv.sample_rss()
    finally:
        shutil.rmtree(gcloud_cache, ignore_errors=True)

    write_summary(run_dir, results, sha, tier, server_meta=server_meta)
    applicable = [r for r in results if not r.get("not_applicable")]
    na_count = len(results) - len(applicable)
    passed = sum(1 for r in applicable if r["passed"])
    known = sum(1 for r in applicable if r["known_failure"])
    failed = len(applicable) - passed - known
    total = sum(r["cost_usd_micros"] for r in applicable)
    print(f"Run directory: {run_dir}")
    print(f"Engine: {server_meta['engine']} (ctx={server_meta['ctx']}, boot={server_meta['server_boot_ms']}ms, peak_rss={server_meta['peak_rss_mb']}MB, host_clean={server_meta['host_clean']})")
    print(f"Total cost: ${total / 1e6:.4f}")
    print(f"{len(applicable)} applicable checks ({na_count} N/A), {passed} passed, {failed + known} failed ({known} known)")
    if midrun_aborted:
        return 2
    return 0 if failed == 0 else 1


def percentile(values: list[int], q: float) -> int:
    if not values:
        return 0
    v = sorted(values)
    return v[min(len(v) - 1, int(round(q * (len(v) - 1))))]


def write_summary(run_dir: Path, results: list[dict[str, Any]], sha: str, tier: int = 2, server_meta: dict[str, Any] | None = None) -> None:
    meta = server_meta or {}
    applicable = [r for r in results if not r.get("not_applicable")]
    na_count = len(results) - len(applicable)
    cats: dict[str, dict[str, int]] = {}
    for r in results:
        c = cats.setdefault(r["category"], {"total": 0, "applicable": 0, "passed": 0, "not_applicable": 0, "cost_usd_micros": 0})
        c["total"] += 1
        if r.get("not_applicable"):
            c["not_applicable"] += 1
        else:
            c["applicable"] += 1
            c["passed"] += int(bool(r["passed"]))
            c["cost_usd_micros"] += r["cost_usd_micros"]
    passed_n = sum(int(bool(r["passed"])) for r in applicable)
    app_n = len(applicable)
    tot_in = sum(r.get("tokens", {}).get("input", 0) for r in applicable)
    tot_out = sum(r.get("tokens", {}).get("output", 0) for r in applicable)
    tot_cached = sum(r.get("tokens", {}).get("cached", 0) for r in applicable)
    ttfts = [int(r.get("ttft_ms") or 0) for r in applicable if r.get("ttft_ms")]
    tps_vals = [float(r.get("decode_tps") or 0.0) for r in applicable if r.get("decode_tps")]
    pure_tps_vals = [float(r.get("pure_decode_tps") or 0.0) for r in applicable if r.get("pure_decode_tps")]
    tot_prefill_ms = sum(int(r.get("prefill_ms") or 0) for r in applicable)
    tot_decode_ms = sum(int(r.get("decode_ms") or 0) for r in applicable)
    tot_tool_ms = sum(int(r.get("tool_and_overhead_ms") or 0) for r in applicable)
    summary = {
        "tier": tier, "git_sha": sha, "os": sys.platform, "created": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "engine": meta.get("engine", "psionic-metal"),
        "worker": meta.get("worker", "mcworker"),
        "ctx": meta.get("ctx"),
        "server_boot_ms": meta.get("server_boot_ms", 0),
        "peak_rss_mb": meta.get("peak_rss_mb", 0),
        "host_clean": meta.get("host_clean", True),
        "host_preflight": meta.get("host_preflight", {}),
        "mid_run_interference": meta.get("mid_run_interference", []),
        "models": sorted({m for r in applicable for m in r["models"] if m}),
        "total": app_n,
        "applicable_total": app_n,
        "all_tasks_total": len(results),
        "not_applicable_count": na_count,
        "passed": passed_n,
        "pass_rate_pct": round(100.0 * passed_n / app_n, 1) if app_n else 0.0,
        "cost_usd_micros": sum(r["cost_usd_micros"] for r in applicable),
        "wall_ms": sum(r["wall_ms"] for r in applicable),
        "prefill_ms": tot_prefill_ms,
        "decode_ms": tot_decode_ms,
        "tool_and_overhead_ms": tot_tool_ms,
        "cost_per_task_usd_micros": (sum(r["cost_usd_micros"] for r in applicable) // app_n) if app_n else 0,
        "wall_p50_ms": percentile([r["wall_ms"] for r in applicable], 0.5),
        "wall_p95_ms": percentile([r["wall_ms"] for r in applicable], 0.95),
        "total_input_tokens": tot_in,
        "total_output_tokens": tot_out,
        "total_cached_tokens": tot_cached,
        "cache_hit_pct": round(100.0 * tot_cached / tot_in, 1) if tot_in else 0.0,
        "mean_ttft_ms": int(round(sum(ttfts) / len(ttfts))) if ttfts else 0,
        "mean_decode_tps": round(sum(tps_vals) / len(tps_vals), 2) if tps_vals else 0.0,
        "mean_pure_decode_tps": round(sum(pure_tps_vals) / len(pure_tps_vals), 2) if pure_tps_vals else 0.0,
        "categories": cats,
        "cases": [{k: r.get(k) for k in ("id", "category", "group", "passed", "not_applicable", "not_applicable_reason",
                                         "known_failure", "cost_usd_micros", "wall_ms", "prefill_ms", "decode_ms",
                                         "tool_and_overhead_ms", "ttft_ms", "decode_tps", "pure_decode_tps",
                                         "tokens", "attempts", "turns", "tool_calls", "guard_trips", "goal_status")} for r in results],
    }
    (run_dir / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    rows = ["| task | category | result | wall | ttft | tokens (in/out) | tok/s | attempts | turns | commands |",
            "|---|---|---|---|---|---|---|---|---|---|"]
    for r in results:
        if r.get("not_applicable"):
            rows.append(f"| {r['id']} | {r['category']} | N/A ({r['not_applicable_reason']}) | - | - | - | - | - | - | - |")
            continue
        res = "pass" if r["passed"] else ("known failure" if r["known_failure"] else "FAIL")
        t = r.get("tokens", {})
        rows.append(f"| {r['id']} | {r['category']} | {res} | {r['wall_ms'] / 1000:.1f} s | {r.get('ttft_ms', 0)} ms | {t.get('input', 0)}/{t.get('output', 0)} | {r.get('decode_tps', 0.0):.1f} | {r['attempts']} | {r['turns']} | {r['tool_calls']} |")
    cat_rows = [f"- {k}: {v['passed']}/{v['applicable']} applicable" + (f" ({v['not_applicable']} N/A)" if v['not_applicable'] else "") for k, v in sorted(cats.items())]
    (run_dir / "report.md").write_text(
        f"# OpenAgents Desktop Tier {tier} run ({sha})\n\n"
        f"Engine: `{summary['engine']}` (ctx={summary['ctx']}, boot={summary['server_boot_ms']} ms, peak RSS={summary['peak_rss_mb']} MB)  \n"
        f"Models: {', '.join(summary['models']) or 'none'}\n\n"
        f"Passed **{summary['passed']}/{summary['applicable_total']} ({summary['pass_rate_pct']}%)** ({na_count} N/A), "
        f"cost ${summary['cost_usd_micros'] / 1e6:.4f}, wall {summary['wall_ms'] / 1000:.1f} s "
        f"(p50 {summary['wall_p50_ms'] / 1000:.1f} s, p95 {summary['wall_p95_ms'] / 1000:.1f} s), "
        f"mean TTFT {summary['mean_ttft_ms']} ms, mean {summary['mean_decode_tps']:.1f} tok/s, "
        f"tokens {tot_in} in / {tot_out} out.\n\n"
        + "\n".join(cat_rows) + "\n\n" + "\n".join(rows) + "\n")


if __name__ == "__main__":
    sys.exit(main())
