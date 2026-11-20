#!/usr/bin/env python3
# Firm reason: 5 (live-agent eval runner: temp repos, process control and JSON grading for Tier 2)
"""Tier 2 headless live-agent runner for Coder Desktop.

Each task in evals/coder-desktop/tier2/tasks/*.json runs the real page (page.coax
through host.c, linked into host_driver) against the real Microcoder worker
(coder/tui/mcworker.coax) with the flags the desktop sends (gated, edits, agents,
unlimited, the skills prefix, the composer's default model) inside an isolated temp
git repo, with a temp HOME, a chat budget and a wall-clock limit.

It calls real models and costs money. run.sh only runs it with --tier 2.

Per task it writes, under evals/coder-desktop/results/tier2-<ts>-<sha>/cases/<id>/:
  wire.jsonl       every line each way between page and workers (host_driver WIRE_LOG)
  journal.jsonl    the workers' durable journals (.ajr frames), decoded, payloads cut
  attempts.jsonl   the model attempts (MWW_ATTEMPT lines) with cost and tokens
  chat.saved       the chat as the app keeps it (cd_save)
  workspace.diff   what the run changed in the temp repo
  result.json      pass/fail, every grader's verdict, cost, latency, turns, tool calls
"""

from __future__ import annotations

import argparse
import fnmatch
import glob
import hashlib
import json
import os
import shutil
import struct
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[3]
EVAL_DIR = REPO_ROOT / "evals/coder-desktop"
HOST_DRIVER_DIR = EVAL_DIR / "host_driver"
HOST_DRIVER_BIN = HOST_DRIVER_DIR / "build/host_driver"
TASKS_DIR = EVAL_DIR / "tier2/tasks"
FIXTURES_DIR = EVAL_DIR / "tier2/fixtures"
# Tier 4 (golden trajectories and the benchmark slice) runs through this same runner with
# --manifest tier4/slice.json: its tasks live in tier4/tasks, its fixtures in tier4/fixtures,
# and a manifest may name Tier 2 tasks too. A fixture is looked up beside its task first.
TIER4_TASKS_DIR = EVAL_DIR / "tier4/tasks"
TIER4_FIXTURES_DIR = EVAL_DIR / "tier4/fixtures"
RESOURCES_DIR = REPO_ROOT / "components/zeron"
REAL_HOME = Path(os.path.expanduser("~"))

# Gated commands naming any of these are answered no by host_driver (GUARD), whatever the
# page says. The temp HOME already keeps ~ inside the sandbox; this is the second wall.
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


# ---------------------------------------------------------------- sandbox

class Sandbox:
    def __init__(self, task: dict[str, Any], gcloud_cache: Path):
        self.root = Path(tempfile.mkdtemp(prefix=f"coder-t2-{task['id']}-"))
        self.repo = self.root / "repo"
        self.outside = self.root / "outside"
        self.home = self.root / "home"
        self.fixture = fixture_dir(task)
        shutil.copytree(self.fixture, self.repo)
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
        for args in (["init", "-q", "-b", "main"], ["add", "-A"], ["commit", "-q", "-m", "fixture"]):
            r = sh(["git", *args], cwd=self.repo, env=env)
            if r.returncode != 0:
                raise RuntimeError(f"git {' '.join(args)} failed: {r.stderr}")
        self.head = sh(["git", "rev-parse", "HEAD"], cwd=self.repo, env=env).stdout.strip()

    def git_env(self) -> dict[str, str]:
        return dict(os.environ, HOME=str(self.home), GIT_CONFIG_NOSYSTEM="1", GIT_AUTHOR_NAME="eval", GIT_AUTHOR_EMAIL="eval@example.invalid",
                    GIT_COMMITTER_NAME="eval", GIT_COMMITTER_EMAIL="eval@example.invalid")

    def worker_env(self) -> dict[str, str]:
        env = {k: v for k, v in os.environ.items() if not k.startswith(("CODER_", "GW_", "CLAUDE"))}
        env.update(HOME=str(self.home), BROWSER="/usr/bin/true", TMPDIR=str(self.root / "tmp"))
        env.update(GIT_CONFIG_NOSYSTEM="1", GIT_AUTHOR_NAME="eval", GIT_AUTHOR_EMAIL="eval@example.invalid",
                   GIT_COMMITTER_NAME="eval", GIT_COMMITTER_EMAIL="eval@example.invalid")
        (self.root / "tmp").mkdir(exist_ok=True)
        return env

    def diff(self) -> str:
        env = self.git_env()
        sh(["git", "add", "-A"], cwd=self.repo, env=env)
        return sh(["git", "diff", "--cached", self.head], cwd=self.repo, env=env).stdout

    def remove(self) -> None:
        shutil.rmtree(self.root, ignore_errors=True)


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
    def __init__(self, sb: Sandbox, worker: Path, claude_worker: Path | None, log_path: Path):
        args = [str(HOST_DRIVER_BIN), "--worker", str(worker)]
        if claude_worker:
            args += ["--claude-worker", str(claude_worker)]
        self.log = open(log_path, "w")
        self.p = subprocess.Popen(args, cwd=str(sb.root), env=sb.worker_env(), stdin=subprocess.PIPE, stdout=subprocess.PIPE,
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


def open_slot(d: Driver, sb: Sandbox, task: dict[str, Any], session: str, claude: bool) -> None:
    must(d.cmd(f"OPEN 0 0 {RESOURCES_DIR}"), "OPEN")
    must(d.cmd(f"SET_CWD 0 {sb.repo}"), "SET_CWD")
    must(d.cmd(f"SET_SESSION 0 {session}"), "SET_SESSION")
    if sb.skills_note:
        must(d.cmd(f"SKILLS 0 {esc(sb.skills_note)}"), "SKILLS")
    if claude:
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


def run_task(task: dict[str, Any], case_dir: Path, worker: Path, claude_worker: Path | None, gcloud_cache: Path, keep: bool) -> dict[str, Any]:
    case_dir.mkdir(parents=True, exist_ok=True)
    sb = Sandbox(task, gcloud_cache)
    wire = case_dir / "wire.jsonl"
    wire.write_text("")
    chat_saved = case_dir / "chat.saved"
    started = time.time()
    deadline = started + float(task.get("timeout_s", 300))
    budget = int(task.get("budget_usd_micros", 100000))
    session = f"t2-{task['id']}"
    states: list[dict[str, Any]] = []
    timed_out = False
    error = None
    reloads = 0
    d = Driver(sb, worker, claude_worker if task.get("claude") else None, case_dir / "driver.log")
    try:
        must(d.cmd(f"WIRE_LOG {wire}"), "WIRE_LOG")
        open_slot(d, sb, task, session, bool(task.get("claude")))
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
                open_slot(d, sb, task, session, bool(task.get("claude")))
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
    result = {
        "id": task["id"],
        "category": task["category"],
        "group": task.get("group"),
        "bug": task.get("bug"),
        "description": task.get("description", ""),
        "passed": passed,
        "known_failure": known if (known and not passed) else None,
        "timed_out": timed_out,
        "wall_ms": wall_ms,
        "cost_usd_micros": ctx.cost_micros(),
        "page_cost_usd_micros": final.get("cost_usd_micros"),
        "budget_usd_micros": budget,
        "attempts": len(attempts),
        "tokens": ctx.tokens(),
        "models": sorted({a.get("model", "") for a in attempts}),
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
        return sum(int(a.get("usd_micros") or 0) for a in self.attempts)

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
    """Runs a check in a copy of the workspace, with listed files restored from the fixture first (the hidden test)."""
    with tempfile.TemporaryDirectory(prefix="coder-t2-grade-") as td:
        work = Path(td) / "w"
        shutil.copytree(c.sb.repo, work, ignore=shutil.ignore_patterns(".git"))
        for rel in g.get("restore", []):
            src = c.sb.fixture / rel
            (work / rel).parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(src, work / rel)
        for rel, text in g.get("add", {}).items():
            (work / rel).parent.mkdir(parents=True, exist_ok=True)
            (work / rel).write_text(text)
        env = dict(os.environ, HOME=td, PYTHONDONTWRITEBYTECODE="1")
        r = subprocess.run(g["run"], shell=True, cwd=str(work), env=env, capture_output=True, text=True, timeout=g.get("timeout_s", 60))
        want = g.get("expect_exit", 0)
        out = (r.stdout + r.stderr)[-600:]
        if r.returncode != want:
            return False, f"exit {r.returncode} (want {want}): {out}"
        for s in g.get("stdout_contains", []):
            if s not in r.stdout:
                return False, f"stdout lacks {s!r}: {out}"
        return True, f"exit {r.returncode}"


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
    bad = [f"{k}={t.get(k)!r}" for k, v in g.get("flags", {}).items() if t.get(k) != v]
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
    "command": g_command, "file_contains": g_file_contains, "file_absent": g_file_absent, "files_unchanged": g_files_unchanged,
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
    ap.add_argument("--out", default="", help="run directory (default evals/coder-desktop/results/tier2-<ts>-<sha>)")
    ap.add_argument("--keep", action="store_true", help="keep each task's temp sandbox")
    ap.add_argument("--claude", action="store_true", help="build claudeworker and let tasks marked claude use it")
    ap.add_argument("--list", action="store_true", help="list the tasks and exit")
    ap.add_argument("--manifest", default="", help="run the tasks a manifest names (Tier 4: evals/coder-desktop/tier4/slice.json)")
    args = ap.parse_args()

    tier = 2
    if args.manifest:
        manifest, tasks = load_manifest(Path(args.manifest))
        tier = int(manifest.get("tier", 4))
    else:
        tasks = load_tasks(TASKS_DIR, FIXTURES_DIR)
    if args.filter:
        keys = [k for k in args.filter.split(",") if k]
        tasks = [t for t in tasks if any(k in t["id"] for k in keys)]
    if args.category:
        tasks = [t for t in tasks if t["category"] == args.category]
    if args.list:
        for t in tasks:
            print(f"{t['id']:34} {t.get('group') or t['category']:12} {t.get('description', '')}")
        return 0
    if not tasks:
        print("no tasks matched", file=sys.stderr)
        return 2

    os.environ["BROWSER"] = "/usr/bin/true"
    build_driver()
    worker = build_worker("mcworker")
    claude_worker = build_worker("claudeworker") if args.claude else None
    if args.claude:
        build_worker("gitguard")   # the git guard hook claudeworker finds beside it (#13771)

    sha = sh(["git", "rev-parse", "--short", "HEAD"], cwd=REPO_ROOT).stdout.strip() or "nogit"
    ts = time.strftime("%Y%m%dT%H%M%SZ", time.gmtime())
    run_dir = Path(args.out) if args.out else EVAL_DIR / f"results/tier{tier}-{ts}-{sha}"
    run_dir.mkdir(parents=True, exist_ok=True)
    gcloud_cache = Path(tempfile.mkdtemp(prefix="coder-t2-gcloud-"))

    results = []
    try:
        for t in tasks:
            t0 = time.time()
            r = run_task(t, run_dir / "cases" / t["id"], worker, claude_worker, gcloud_cache, args.keep)
            results.append(r)
            tag = "ok  " if r["passed"] else ("KNOWN" if r["known_failure"] else "FAIL")
            fails = "; ".join(f"{g['type']}: {g['detail']}" for g in r["graders"] if not g["pass"])
            print(f"{tag} task {r['id']}: {r['description']} ({r['wall_ms']} ms, ${r['cost_usd_micros'] / 1e6:.4f}, "
                  f"{r['attempts']} attempts, {r['turns']} turns, {r['tool_calls']} commands)" + (f" -- {fails}" if fails else ""), flush=True)
    finally:
        shutil.rmtree(gcloud_cache, ignore_errors=True)

    write_summary(run_dir, results, sha, tier)
    passed = sum(1 for r in results if r["passed"])
    known = sum(1 for r in results if r["known_failure"])
    failed = len(results) - passed - known
    total = sum(r["cost_usd_micros"] for r in results)
    print(f"Run directory: {run_dir}")
    print(f"Total cost: ${total / 1e6:.4f}")
    print(f"{len(results)} checks, {passed} passed, {failed + known} failed ({known} known)")
    return 0 if failed == 0 else 1


def percentile(values: list[int], q: float) -> int:
    if not values:
        return 0
    v = sorted(values)
    return v[min(len(v) - 1, int(round(q * (len(v) - 1))))]


def write_summary(run_dir: Path, results: list[dict[str, Any]], sha: str, tier: int = 2) -> None:
    cats: dict[str, dict[str, int]] = {}
    for r in results:
        c = cats.setdefault(r["category"], {"total": 0, "passed": 0, "cost_usd_micros": 0})
        c["total"] += 1
        c["passed"] += int(r["passed"])
        c["cost_usd_micros"] += r["cost_usd_micros"]
    summary = {
        "tier": tier, "git_sha": sha, "os": sys.platform, "created": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "models": sorted({m for r in results for m in r["models"] if m}),
        "total": len(results), "passed": sum(int(r["passed"]) for r in results),
        "cost_usd_micros": sum(r["cost_usd_micros"] for r in results),
        "wall_ms": sum(r["wall_ms"] for r in results),
        "cost_per_task_usd_micros": (sum(r["cost_usd_micros"] for r in results) // len(results)) if results else 0,
        "wall_p50_ms": percentile([r["wall_ms"] for r in results], 0.5),
        "wall_p95_ms": percentile([r["wall_ms"] for r in results], 0.95),
        "categories": cats,
        "cases": [{k: r.get(k) for k in ("id", "category", "group", "passed", "known_failure", "cost_usd_micros", "wall_ms", "attempts", "turns", "tool_calls", "guard_trips", "goal_status")} for r in results],
    }
    (run_dir / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    rows = ["| task | category | result | cost | wall | attempts | turns | commands |", "|---|---|---|---|---|---|---|---|"]
    for r in results:
        res = "pass" if r["passed"] else ("known failure" if r["known_failure"] else "FAIL")
        rows.append(f"| {r['id']} | {r['category']} | {res} | ${r['cost_usd_micros'] / 1e6:.4f} | {r['wall_ms'] / 1000:.0f} s | {r['attempts']} | {r['turns']} | {r['tool_calls']} |")
    cat_rows = [f"- {k}: {v['passed']}/{v['total']} (${v['cost_usd_micros'] / 1e6:.4f})" for k, v in sorted(cats.items())]
    (run_dir / "report.md").write_text(
        f"# Coder Desktop Tier {tier} run ({sha})\n\nModels: {', '.join(summary['models']) or 'none'}\n\n"
        f"Passed {summary['passed']}/{summary['total']}, cost ${summary['cost_usd_micros'] / 1e6:.4f}, wall {summary['wall_ms'] / 1000:.0f} s, "
        f"p95 task wall {summary['wall_p95_ms'] / 1000:.0f} s, ${summary['cost_per_task_usd_micros'] / 1e6:.4f} a task.\n\n"
        + "\n".join(cat_rows) + "\n\n" + "\n".join(rows) + "\n")


if __name__ == "__main__":
    sys.exit(main())
