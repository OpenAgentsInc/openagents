#!/usr/bin/env python3
"""Work one GitHub issue with the briefed agent (#11211, #11258).

    work_issue.py --repo OWNER/NAME --issue N [--land queue|pr|none]
                  [--engine briefed|briefed:codex|briefed:opencode|briefed:gemini|bare]
                  [--out DIR] [--checkout PATH]

The one engine entry point behind `POST /v1/work`, the web's "Work on this
issue", agent fleet runs, and `openagents chat work` (the default engine):

1. **Briefing**: the issue (from `gh`) at `origin/main`, the context finder
   (`scripts/filefind`, #11210) and the brief generator
   (`scripts/bench/briefed-ab/briefing.py`).
2. **Briefed agent**: `crates/briefed-agent` with `verify` (#11229): the five
   file tools held to a fresh worktree, a briefing-built system prompt, and
   `verify`, which compiles, runs the change's tests and formats. `briefed`
   runs it on Claude; `briefed:codex` runs the same briefing and checks on
   Codex CLI (#11308), on Codex's own login, with `verify` served to it over
   MCP by the same binary. `briefed:opencode` runs it on OpenCode (an open
   model Vertex serves, `OA_WORK_OPENCODE_MODEL` to pick another) and
   `briefed:gemini` on Vertex AI's Gemini through OpenCode
   (`OA_WORK_GEMINI_MODEL`), on Google's application default credentials.
3. **Judged by the diff**: the driver replays the checks itself on what the
   agent actually changed: `cargo fmt` and `cargo test` for each package the
   diff touches. A run passes only when there is a diff and every replayed
   check passes.
   Every other changed file is checked by its kind (#11310,
   `scripts/bench/briefed-ab/kinds.py`): a script's tests, a document's
   links, a nested workspace's crate; a kind with no checker here (Kotlin)
   is reported, never passed.
4. **Re-brief** (#11311): when the briefed attempt makes no change, keeps
   failing `verify`, or fails the replayed checks, a second briefing is
   built from that evidence (the files the agent had to open that the
   briefing missed, the files the checks blamed, and what went wrong) and
   the briefed agent tries once more in a fresh worktree (`--rebrief` or
   `OA_WORK_REBRIEF=1`), within the run's
   time and dollar budget (`--budget-usd`, `--max-secs`).
5. **Escalation**: low briefing confidence, a briefed attempt that failed
   (after the re-brief, when it ran), or a missing capability falls back to
   bare Claude Code (`claude -p`, "Complete this issue.") in a fresh
   worktree, judged the same way. The result records why, and each
   attempt's engine, briefing digest, cost and outcome.
6. **Landing**: on a pass, commit; `queue` hands the branch to the landing
   queue (`openagents land submit`, docs/cloud/land-queue.md), `pr` pushes a
   branch and opens a pull request, `none` keeps the commit local.

Claude runs on the credential in the environment
(`CLAUDE_CODE_OAUTH_TOKEN`, a cloud provider's variables, or
`~/.claude/.credentials.json`): the launcher (`worker.py`) puts the
requesting person's own credential there, in a home of the run's own.
`ANTHROPIC_AUTH_TOKEN` is never passed to either engine, and
`ANTHROPIC_API_KEY` only when it is the person's own credential
(`OA_WORK_KEEP_API_KEY=1`).

Progress is one JSON line per step on stdout and in `OUT/events.jsonl`; the
result is `OUT/result.json` and the last stdout line (`{"type": "result"}`).
Cost is what Claude Code reports (list price); when it reports none, the
cost is `null` (unknown), never zero.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import time
import uuid
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
BENCH = ROOT / "scripts" / "bench" / "briefed-ab"
LOCAL_EXEC = HERE / "local-exec"

TRAILER = "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
DROP_ENV = ("ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN", "CLAUDECODE", "CLAUDE_CODE_ENTRYPOINT")
# A finder whose best file scores under this is a guess, not a briefing.
MIN_CONFIDENCE = float(os.environ.get("OA_WORK_MIN_CONFIDENCE", "0.05"))
# This many failing `verify` calls with no pass is "repeated verify failures".
MAX_VERIFY_FAILS = int(os.environ.get("OA_WORK_MAX_VERIFY_FAILS", "4"))

SYSTEM = """You are a senior engineer making one change in this repository. Your working directory is the repository checkout at the issue's base commit.

Below is a briefing for this issue: a change plan, the acceptance items, the files to change with excerpts (line numbers are the file's own), similar past changes, the checks, and the repo rules.

How to work:
- Trust the briefing. Start from the listed files and excerpts. Open a file the briefing does not list only when the change, the compiler, a test, or a check demands it.
- Make the smallest complete change that resolves the issue, in the surrounding style. For code and scripts, add or update a test that pins the new behavior; for a document, keep its links and paths valid.
- Check your work with `verify`, passing `tests` with your new Rust tests' names. It runs the checks every file you changed needs (a crate's tests and fmt, a script's tests, a document's links) and returns only what is wrong, with each acceptance item's result; fix that and call it again. `fast: true` only compiles. `unchecked` means nothing here can check what you changed: read it back and stop when it is right. Call it once when you think you are done, not after every edit.
- When the checks pass, stop. Reply in two or three lines with what changed.
- Do not commit.
"""


class Run:
    """One run's output directory, progress stream and clock."""

    def __init__(self, out: Path):
        self.out = out
        self.out.mkdir(parents=True, exist_ok=True)
        self.events = open(out / "events.jsonl", "a", buffering=1)
        self.started = time.time()

    def say(self, phase: str, text: str, **extra) -> None:
        row = {"type": "progress", "t": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
               "secs": round(time.time() - self.started, 1), "phase": phase, "text": text, **extra}
        line = json.dumps(row)
        self.events.write(line + "\n")
        print(line, flush=True)


def sh(cmd, cwd=None, check=True, timeout=None, env=None, input=None) -> subprocess.CompletedProcess:
    proc = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, timeout=timeout, env=env, input=input)
    if check and proc.returncode != 0:
        raise RuntimeError(f"{' '.join(map(str, cmd))[:160]}: {(proc.stderr or proc.stdout)[-1200:]}")
    return proc


def keeps_api_key() -> bool:
    """The run's own credential is an API key (the launcher sets
    OA_WORK_KEEP_API_KEY=1 when the requester saved one)."""
    return os.environ.get("OA_WORK_KEEP_API_KEY") == "1" and bool(os.environ.get("ANTHROPIC_API_KEY"))


def clean_env(extra: dict | None = None) -> dict:
    env = dict(os.environ)
    for key in DROP_ENV:
        if not (key == "ANTHROPIC_API_KEY" and keeps_api_key()):
            env.pop(key, None)
    env.setdefault("DISABLE_AUTOUPDATER", "1")
    env.update(extra or {})
    return env


# What an engine may not do: the run lands the change itself, after its
# own checks. Shims that refuse, and git with no way to push.
NO_LANDING = "Do not commit, push, open a pull request, or comment on or close the issue: this run checks your change and lands it itself."
GITHUB_ENV = ("GH_TOKEN", "GITHUB_TOKEN", "GH_ENTERPRISE_TOKEN", "GITHUB_ENTERPRISE_TOKEN")


def agent_env(out: Path) -> dict:
    """The engine's environment: no GitHub credential, `gh` and
    `openagents` refuse, and `git push` has nowhere to go."""
    shims = out / "shims"
    shims.mkdir(parents=True, exist_ok=True)
    for name in ("gh", "openagents"):
        shim = shims / name
        shim.write_text(f"#!/bin/sh\necho '{name}: not available in this run; it lands the change itself.' >&2\nexit 1\n")
        shim.chmod(0o755)
    env = clean_env()
    for key in GITHUB_ENV:
        env.pop(key, None)
    env.update({
        "PATH": f"{shims}:{env.get('PATH', '')}",
        "GIT_TERMINAL_PROMPT": "0",
        "GIT_CONFIG_COUNT": "2",
        "GIT_CONFIG_KEY_0": "remote.origin.pushurl", "GIT_CONFIG_VALUE_0": "no-push://this-run-lands-it",
        "GIT_CONFIG_KEY_1": "credential.helper", "GIT_CONFIG_VALUE_1": "",
    })
    return env


def codex_signed_in() -> bool:
    home = Path(os.environ.get("CODEX_HOME") or Path.home() / ".codex")
    return (home / "auth.json").exists()


def signed_in(engine: str) -> str | None:
    """None when `engine`'s login is here; otherwise what is missing."""
    if backend_of(engine) == "codex" and engine.startswith("briefed"):
        return None if codex_signed_in() else "no Codex sign-in in this environment (~/.codex/auth.json)"
    if backend_of(engine) in ("opencode", "gemini"):
        return None if shutil.which("opencode") else "OpenCode is not installed in this environment"
    return None if claude_signed_in() else "no Claude sign-in in this environment (CLAUDE_CODE_OAUTH_TOKEN or ~/.claude)"


def claude_signed_in() -> bool:
    return (bool(os.environ.get("CLAUDE_CODE_OAUTH_TOKEN")) or keeps_api_key()
            or any(os.environ.get(k) for k in ("CLAUDE_CODE_USE_BEDROCK", "CLAUDE_CODE_USE_VERTEX",
                                                 "CLAUDE_CODE_USE_FOUNDRY"))
            or (Path.home() / ".claude" / ".credentials.json").exists())


# ------------------------------------------------------------------ checkout

def checkout_for(repo: str, given: str | None) -> Path:
    """A clone of `repo` with an up-to-date `origin/main`."""
    if given:
        path = Path(given)
    elif repo.lower() == "openagentsinc/openagents" and (Path.home() / "openagents" / ".git").exists():
        path = Path.home() / "openagents"
    else:
        path = Path.home() / ".openagents" / "work" / "repos" / repo.replace("/", "__")
        if not (path / ".git").exists():
            path.parent.mkdir(parents=True, exist_ok=True)
            sh(["gh", "repo", "clone", repo, str(path), "--", "-q", "--filter=blob:none"], timeout=1800)
    sh(["git", "-C", str(path), "fetch", "-q", "origin", "main"], timeout=900)
    return path


def default_branch_sha(checkout: Path) -> str:
    return sh(["git", "-C", str(checkout), "rev-parse", "origin/main"]).stdout.strip()


def add_worktree(checkout: Path, dest: Path, base: str) -> Path:
    shutil.rmtree(dest, ignore_errors=True)
    sh(["git", "-C", str(checkout), "worktree", "prune"], check=False)
    sh(["git", "-C", str(checkout), "worktree", "add", "-f", "-q", "--detach", str(dest), base], timeout=900)
    return dest


def drop_worktree(checkout: Path, dest: Path) -> None:
    sh(["git", "-C", str(checkout), "worktree", "remove", "--force", str(dest)], check=False)
    shutil.rmtree(dest, ignore_errors=True)


def changed_files(root: Path, base: str) -> list[str]:
    """Every file the engine changed since `base`, committed or not."""
    rows = sh(["git", "-C", str(root), "status", "--porcelain", "--untracked-files=all"]).stdout.splitlines()
    files = {row[3:].split(" -> ")[-1].strip('"') for row in rows if len(row) > 3}
    files |= set(sh(["git", "-C", str(root), "diff", "--name-only", base]).stdout.split())
    return sorted(files)


def diff_stat(root: Path, base: str) -> dict:
    sh(["git", "-C", str(root), "add", "-A"])
    numstat = sh(["git", "-C", str(root), "diff", "--cached", "--numstat", base]).stdout
    added = removed = 0
    files = []
    for row in numstat.splitlines():
        parts = row.split("\t")
        if len(parts) >= 3:
            files.append(parts[2])
            added += int(parts[0]) if parts[0].isdigit() else 0
            removed += int(parts[1]) if parts[1].isdigit() else 0
    sh(["git", "-C", str(root), "reset", "-q"])
    return {"files": files, "added": added, "removed": removed}


def package_of(root: Path, path: str) -> str | None:
    """The Cargo package whose manifest is nearest above `path`."""
    parts = Path(path).parts
    for i in range(len(parts) - 1, 0, -1):
        manifest = root.joinpath(*parts[:i], "Cargo.toml")
        if manifest.exists():
            text = manifest.read_text(errors="replace")
            if "[package]" in text:
                m = re.search(r'^\s*name\s*=\s*"([^"]+)"', text.split("[package]", 1)[1], re.M)
                if m:
                    return m.group(1)
    return None


# ------------------------------------------------------------------- checks

def cargo(root: Path, args: list[str], timeout: int) -> subprocess.CompletedProcess:
    """One cargo command under the build lock (one shared warm target)."""
    return sh([str(LOCAL_EXEC), "cargo", *args], cwd=root, check=False,
              env=clean_env({"OA_WORK_CMD_TIMEOUT": str(timeout)}))


def kinds_mod():
    sys.path.insert(0, str(BENCH))
    import kinds  # noqa: E402
    return kinds


def replay_checks(run: Run, root: Path, base: str) -> list[dict]:
    """fmt, then the tests of every root-workspace Rust package the diff
    touches, then every other changed file's checks by kind (#11310)."""
    kinds = kinds_mod()
    files = [f for f in changed_files(root, base) if (root / f).exists()]
    planned = kinds.plan(files, kinds.Tree(root), base)
    nested = {c["package"] for c in planned if c["kind"] == "rust-nested"}
    packages = []
    for path in files:
        if path.endswith(".rs") or path.endswith("Cargo.toml"):
            pkg = package_of(root, path)
            if pkg and pkg not in packages and pkg not in nested:
                packages.append(pkg)
    for c in planned:
        if c["kind"] == "rust" and c["package"] not in packages:
            packages.append(c["package"])
    checks = []
    for pkg in packages:
        run.say("checks", f"formatting {pkg}")
        fmt = cargo(root, ["fmt", "-p", pkg], 300)
        checks.append({"name": f"cargo fmt -p {pkg}", "ok": fmt.returncode == 0,
                       "tail": (fmt.stdout + fmt.stderr)[-600:] if fmt.returncode else ""})
    for pkg in packages:
        run.say("checks", f"running {pkg}'s tests")
        t0 = time.time()
        test = cargo(root, ["test", "-p", pkg, "--message-format", "short"], 2400)
        text = test.stdout + test.stderr
        ran = sum(int(n) for n in re.findall(r"test result: \w+\. (\d+) passed", text))
        failed = sum(int(n) for n in re.findall(r"; (\d+) failed", text))
        checks.append({"name": f"cargo test -p {pkg}", "ok": test.returncode == 0, "passed": ran,
                       "failed": failed, "secs": round(time.time() - t0, 1),
                       "tail": text[-1500:] if test.returncode else ""})
    for c in planned:
        if c["kind"] == "rust" or c["id"].startswith("check:"):
            continue
        run.say("checks", f"{c['what']}" if c.get("available", True) else f"no checker here: {c['what']}")
        r = kinds.run_check(c, root, str(LOCAL_EXEC))
        checks.append({"name": " ".join(c["argv"])[:200], "id": c["id"], "kind": c["kind"], "ok": r["ok"],
                       "available": r["available"], "secs": r["secs"], "tail": r["tail"]})
    return checks


def checks_pass(checks: list[dict]) -> bool:
    """Every check that ran passed; a kind with no checker here (ok None)
    is reported, never counted as a pass or a failure."""
    return all(c["ok"] is not False for c in checks)


# ------------------------------------------------------------------ engines

def briefing_for(issue: dict, base: str, checkout: Path, seeds: list[dict] | None = None,
                 wrong: str | None = None, briefer: str = "filefind") -> dict:
    """The #11211 briefing, built at `base` in `checkout`, with checks per
    package kind and typed acceptance (#11310); `seeds` and `wrong` make the
    re-briefing (#11311). `briefer=haiku` swaps the finder's decision step
    for the constrained Haiku agent (docs/inference/haiku-briefer.md), which
    reads a worktree of its own at `base`, read-only."""
    os.environ["AB_REPO"] = str(checkout)
    os.environ.setdefault("AB_WORK", str(Path.home() / ".openagents" / "work" / "briefing"))
    sys.path.insert(0, str(BENCH))
    import briefing as briefing_mod  # noqa: E402
    from common import LEVERS  # noqa: E402

    briefing_mod.REPO = checkout
    import common
    common.REPO = checkout
    task = {"issue": issue["number"], "title": issue["title"], "body": issue.get("body") or "", "parent": base}
    levers = dict(LEVERS)
    levers["checks"] = os.environ.get("OA_WORK_CHECKS", "kinds")
    levers["acceptance"] = os.environ.get("OA_WORK_ACCEPTANCE", "typed")
    if briefer != "haiku":
        b = briefing_mod.build(task, levers, seeds=seeds, wrong=wrong)
    else:
        # Haiku reads files, so it gets a worktree at `base` of its own (the
        # checkout itself may sit on another commit or hold local edits).
        levers["finder"] = "haiku"
        root = add_worktree(checkout, Path.home() / ".openagents" / "work" / "briefer" / uuid.uuid4().hex[:12], base)
        try:
            b = briefing_mod.build(task, levers, seeds=seeds, wrong=wrong, root=root)
        finally:
            drop_worktree(checkout, root)
    b["markdown"] = briefing_mod.render(b)
    b["digest"] = "sha256:" + __import__("hashlib").sha256(b["markdown"].encode()).hexdigest()[:16]
    return b


def confidence_of(b: dict) -> float | None:
    """The finder's best file score (filefind's confidence, 0..1; the
    `lite` finder's scores are raw term weights)."""
    scores = [f.get("score") for f in b["files"] if isinstance(f.get("score"), (int, float))]
    return max(scores) if scores else None


def verify_fails(log: Path) -> tuple[int, bool]:
    """(failing verify calls, whether any passed)."""
    fails, passed = 0, False
    if log.exists():
        for line in log.read_text().splitlines():
            try:
                row = json.loads(line)
            except json.JSONDecodeError:
                continue
            status = (row.get("verdict") or row).get("status")
            if status == "pass":
                passed = True
            elif status not in (None, "compiles", "unchecked"):
                fails += 1
    return fails, passed


def usage_of(result: dict | None) -> dict:
    result = result or {}
    u = result.get("usage") or {}
    if "cached_input_tokens" in u:  # Codex names its usage differently; it reports no cost.
        return {"cost_usd": None, "turns": result.get("num_turns"),
                "input_tokens": u.get("input_tokens"), "output_tokens": u.get("output_tokens"),
                "cache_read": u.get("cached_input_tokens"), "is_error": False}
    return {"cost_usd": result.get("total_cost_usd"), "turns": result.get("num_turns"),
            "input_tokens": u.get("input_tokens"), "output_tokens": u.get("output_tokens"),
            "cache_read": u.get("cache_read_input_tokens"), "is_error": result.get("is_error")}


def backend_of(engine: str) -> str:
    """`claude`, `codex`, `opencode` or `gemini`: the engine the briefed agent runs on."""
    return engine.split(":", 1)[1] if ":" in engine else "claude"


def run_briefed(run: Run, b: dict, root: Path, out: Path, model: str, timeout: int,
                backend: str = "claude") -> dict:
    out.mkdir(parents=True, exist_ok=True)
    if "packages" in b:
        packages = [p["id"] for p in b["packages"] if p["kind"] == "rust"]
    else:
        packages = []
        for c in b["checks"]:
            pkg = c["id"].split(":", 1)[1]
            if pkg not in packages:
                packages.append(pkg)
    system = SYSTEM + "\n\n" + b["markdown"]
    (out / "system.md").write_text(system)
    (out / "prompt.md").write_text(f"Complete issue #{b['issue']} as the briefing describes.")
    config = {
        "worktree": str(root), "system_prompt_path": str(out / "system.md"),
        "prompt_path": str(out / "prompt.md"), "events_path": str(out / "agent-events.jsonl"),
        "summary_path": str(out / "summary.json"), "briefed_files": [f["path"] for f in b["files"]],
        "model": model, "timeout_secs": timeout, "tools": ["Read", "Edit", "Write", "Grep", "Glob"],
        "custom": ["verify"], "inherit_api_key": keeps_api_key(), "engine": backend,
        "verify": {
            "exec": str(LOCAL_EXEC), "crates": packages, "baseline_errors": [],
            "done_when": [p.removeprefix("Required: ") for p in b["plan"] if p.startswith("Required: ")]
            or [b["title"]],
            "acceptance": (b.get("acceptance") or {}).get("items", []),
            "planner": [sys.executable, str(BENCH / "kinds.py"), "plan", "--root", str(root), "--base", b["base"]],
            "cochange": {}, "log": str(out / "verify.jsonl"),
        },
    }
    if backend == "codex":
        config["model"] = os.environ.get("OA_WORK_CODEX_MODEL", "")  # empty: Codex's own default
        config["codex_bin"] = shutil.which("codex") or "codex"
    if backend in ("opencode", "gemini"):  # empty: the engine's default model (#11308)
        config["model"] = os.environ.get(f"OA_WORK_{backend.upper()}_MODEL", "")
        config["opencode_bin"] = shutil.which("opencode") or "opencode"
    (out / "agent.json").write_text(json.dumps(config, indent=2))
    agent = os.environ.get("OA_BRIEFED_AGENT") or shutil.which("briefed-agent") or "briefed-agent"
    run.say("agent", f"the briefed agent is working on {BACKEND_NAMES.get(backend, 'Claude')}",
            engine=f"briefed:{backend}" if backend != "claude" else "briefed")
    t0 = time.time()
    proc = subprocess.Popen([agent, str(out / "agent.json")], cwd=root, env=agent_env(out),
                            stdout=subprocess.DEVNULL, stderr=open(out / "agent-stderr.txt", "w"))
    seen = 0
    while proc.poll() is None:
        time.sleep(5)
        seen = relay_verify(run, out / "verify.jsonl", seen)
        if time.time() - t0 > timeout + 300:
            proc.kill()
    relay_verify(run, out / "verify.jsonl", seen)
    summary = json.loads((out / "summary.json").read_text()) if (out / "summary.json").exists() else {}
    fails, passed = verify_fails(out / "verify.jsonl")
    return {"secs": round(time.time() - t0, 1), "summary": summary, "usage": usage_of(summary.get("result")),
            "verify_fails": fails, "verify_passed": passed, "timed_out": summary.get("timed_out", False),
            "error": summary.get("error") or ((out / "agent-stderr.txt").read_text()[-600:] if proc.returncode else None)}


def relay_verify(run: Run, log: Path, seen: int) -> int:
    """Each new `verify` verdict as a progress line."""
    if not log.exists():
        return seen
    rows = log.read_text().splitlines()
    for line in rows[seen:]:
        try:
            row = json.loads(line)
        except json.JSONDecodeError:
            continue
        status = (row.get("verdict") or row).get("status", "?")
        run.say("verify", f"verify: {status.replace('_', ' ')}", status=status)
    return len(rows)


def run_bare(run: Run, issue: dict, root: Path, out: Path, model: str, timeout: int) -> dict:
    """Bare Claude Code, told "Complete this issue." (#11211's arm A)."""
    out.mkdir(parents=True, exist_ok=True)
    prompt = (f"Complete this issue.\n\n#{issue['number']} {issue['title']}\n\n{issue.get('body') or ''}"
              f"\n\n{NO_LANDING}")
    argv = [shutil.which("claude") or "claude", "-p", prompt, "--output-format", "stream-json", "--verbose",
            "--dangerously-skip-permissions", "--no-session-persistence", "--strict-mcp-config"]
    if model:
        argv += ["--model", model]
    run.say("agent", "Claude Code is working", engine="bare")
    t0 = time.time()
    result, timed_out = None, False
    with open(out / "agent-events.jsonl", "w") as sink:
        proc = subprocess.Popen(argv, cwd=root, env=agent_env(out), stdout=subprocess.PIPE,
                                stdin=subprocess.DEVNULL, stderr=open(out / "agent-stderr.txt", "w"), text=True)
        try:
            for line in proc.stdout:
                sink.write(line)
                try:
                    ev = json.loads(line)
                except json.JSONDecodeError:
                    continue
                if ev.get("type") == "result":
                    result = ev
                if time.time() - t0 > timeout:
                    timed_out = True
                    proc.kill()
                    break
        finally:
            proc.wait()
    return {"secs": round(time.time() - t0, 1), "usage": usage_of(result), "timed_out": timed_out,
            "error": None if result else (out / "agent-stderr.txt").read_text()[-600:]}


# ------------------------------------------------------------------ landing

BACKEND_NAMES = {"claude": "Claude", "codex": "Codex", "opencode": "OpenCode", "gemini": "Gemini"}


def engine_words(engine: str) -> str:
    if engine == "briefed":
        return "briefed agent"
    if engine.startswith("briefed:"):
        return f"briefed agent on {BACKEND_NAMES.get(backend_of(engine), backend_of(engine))}"
    return "bare Claude Code"


def commit(root: Path, issue: dict, engine: str, base: str) -> str:
    # One commit on `base`, whatever the engine committed on its own.
    sh(["git", "-C", str(root), "reset", "-q", "--soft", base])
    sh(["git", "-C", str(root), "add", "-A"])
    message = (f"{issue['title']} (#{issue['number']})\n\n"
               f"Worked by the {engine_words(engine)} "
               f"(openagents work, engine {engine}); fixes #{issue['number']}.\n\n{TRAILER}\n")
    sh(["git", "-C", str(root), "commit", "-q", "-m", message])
    return sh(["git", "-C", str(root), "rev-parse", "HEAD"]).stdout.strip()


def land(run: Run, root: Path, repo: str, issue: dict, how: str, run_id: str, engine: str,
         checks: list[dict]) -> dict:
    if how == "none":
        return {"how": "none"}
    if how == "queue":
        oa = os.environ.get("OA_CLI") or shutil.which("openagents") or "openagents"
        proc = sh([oa, "--json", "land", "submit", "--issue", str(issue["number"]),
                   "--summary", f"{engine} engine, run {run_id}"], cwd=root, check=False, timeout=600)
        try:
            entry = json.loads(proc.stdout).get("entry")
        except (json.JSONDecodeError, AttributeError):
            entry = None
        if proc.returncode != 0:
            raise RuntimeError(f"the landing queue refused it: {(proc.stderr or proc.stdout)[-400:]}")
        run.say("land", "handed to the landing queue", entry=(entry or {}).get("id"))
        return {"how": "queue", "entry": entry}
    branch = f"work/issue-{issue['number']}-{run_id[:8]}"
    sh(["git", "-C", str(root), "push", "-q", "origin", f"HEAD:refs/heads/{branch}"], timeout=600)
    lines = [f"Fixes #{issue['number']}.", "",
             f"Worked by the {engine_words(engine)} "
             f"(`openagents work`, run `{run_id}`).", "", "Checks replayed on this diff:", ""]
    lines += [f"- {'pass' if c['ok'] else ('no checker here' if c['ok'] is None else 'FAIL')}: `{c['name']}`"
              for c in checks] or ["- none (nothing to check)"]
    lines += ["", "🤖 Generated with [Claude Code](https://claude.com/claude-code)"]
    proc = sh(["gh", "pr", "create", "--repo", repo, "--head", branch, "--base", "main",
               "--title", f"{issue['title']} (#{issue['number']})", "--body", "\n".join(lines)],
              cwd=root, timeout=300)
    url = proc.stdout.strip().splitlines()[-1]
    run.say("land", "opened a pull request", url=url)
    return {"how": "pr", "branch": branch, "url": url}


def default_model() -> str | None:
    """The bench's model on Anthropic's own sign-ins; on a cloud provider's
    credential, the provider's default (its model names differ)."""
    cloud = ("CLAUDE_CODE_USE_BEDROCK", "CLAUDE_CODE_USE_VERTEX", "CLAUDE_CODE_USE_FOUNDRY")
    return None if any(os.environ.get(k) for k in cloud) else "claude-opus-5-5"


# --------------------------------------------------------------------- main

def cost_so_far(attempts: list[dict]) -> tuple[float | None, int | None]:
    """Claude Code's reported cost of every attempt so far (None when any is
    unknown, never 0 for unknown) and their tokens."""
    usages = [a["agent"]["usage"] for a in attempts]
    costs = [u.get("cost_usd") for u in usages]
    cost = None if not costs or any(c is None for c in costs) else round(sum(costs), 4)
    tokens = sum((u.get("input_tokens") or 0) + (u.get("output_tokens") or 0) for u in usages)
    return cost, (tokens or None)


def say_cost(run: Run, attempts: list[dict]) -> None:
    """A `cost` line after each attempt, for the fleet rows (#11250)."""
    cost, tokens = cost_so_far(attempts)
    text = f"${cost:.2f} so far" if cost is not None else "cost not known yet"
    run.say("cost", text, cost_usd=cost, tokens=tokens)


def attempt(run: Run, engine: str, issue: dict, checkout: Path, base: str, out: Path, args, b: dict | None,
            start_from: str | None = None) -> dict:
    """One engine's attempt in a fresh worktree; `start_from` is a diff
    applied first (a re-brief that keeps the first attempt's change)."""
    wt = add_worktree(checkout, out / "worktree", base)
    if start_from:
        sh(["git", "-C", str(wt), "apply", "--binary", "--whitespace=nowarn", "-"], input=start_from, check=False)
    if engine.startswith("briefed"):
        agent = run_briefed(run, b, wt, out, args.model, args.timeout, backend_of(engine))
    else:
        agent = run_bare(run, issue, wt, out, args.model, args.timeout)
    files = changed_files(wt, base)
    checks = replay_checks(run, wt, base) if files else []
    stat = diff_stat(wt, base) if files else {"files": [], "added": 0, "removed": 0}
    ok = bool(files) and checks_pass(checks)
    outcome = "passed" if ok else ("no change" if not files else "checks failed")
    return {"engine": engine, "worktree": str(wt), "out": str(out), "agent": agent, "checks": checks, "diff": stat,
            "ok": ok, "outcome": outcome, "briefing_digest": (b or {}).get("digest"),
            "cost_usd": agent["usage"].get("cost_usd"), "secs": agent.get("secs")}


def working_patch(root: Path, base: str) -> str:
    """The attempt's whole change against `base`, untracked files included."""
    sh(["git", "-C", str(root), "add", "-A"])
    patch = sh(["git", "-C", str(root), "diff", "--cached", "--binary", base]).stdout
    sh(["git", "-C", str(root), "reset", "-q"])
    return patch


def failure_reason(a: dict) -> str:
    ag = a["agent"]
    if not a["diff"]["files"]:
        return "the briefed agent made no change"
    if ag["verify_fails"] >= MAX_VERIFY_FAILS and not ag["verify_passed"]:
        return f"repeated verify failures ({ag['verify_fails']})"
    return "the replayed checks failed on the briefed agent's change"


def rebrief_allowed(args, attempts: list[dict], started: float) -> tuple[bool, str]:
    """At most one re-brief, and only when another attempt like the first
    still fits the run's dollar and time budget."""
    if not args.rebrief:
        return False, "re-brief is off"
    if any(a.get("rebrief") for a in attempts):
        return False, "already re-briefed once"
    first = attempts[-1]
    spent, _ = cost_so_far(attempts)
    if spent is not None and first.get("cost_usd") is not None and spent + first["cost_usd"] > args.budget_usd:
        return False, f"another attempt would pass the ${args.budget_usd:.2f} budget"
    if time.time() - started + (first.get("secs") or 0) > args.max_secs:
        return False, f"another attempt would pass the {args.max_secs}s limit"
    return True, ""


def rebrief_attempt(run: Run, engine: str, issue: dict, checkout: Path, base: str, out: Path, args,
                    first: dict) -> dict:
    """The second briefed attempt (#11311), from the first one's misses,
    its `verify` evidence and its replayed checks."""
    sys.path.insert(0, str(BENCH))
    import rebrief  # noqa: E402
    wt = Path(first["worktree"])
    patch = working_patch(wt, base)
    policy = os.environ.get("OA_WORK_REBRIEF_WORKTREE", "clean")

    def exists(path: str) -> bool:
        return sh(["git", "-C", str(checkout), "cat-file", "-e", f"{base}:{path}"], check=False).returncode == 0

    ev = rebrief.evidence(Path(first["out"]), first["checks"], patch, exists, policy)
    why = failure_reason(first)
    run.say("rebrief", f"{why}; briefing again from what the first attempt found",
            seeds=[s_["path"] for s_ in ev["seeds"]], sections=ev["sections"])
    b2 = briefing_for(issue, base, checkout, seeds=ev["seeds"], wrong=ev["wrong"])
    (out.parent / "briefing-rebrief.md").write_text(b2["markdown"])
    a = attempt(run, engine, issue, checkout, base, out, args, b2,
                start_from=patch if policy == "keep" and patch.strip() else None)
    a["rebrief"] = {"why": why, "seeds": [s_["path"] for s_ in ev["seeds"]], "sections": ev["sections"],
                    "misses": ev["misses"], "last_verify": ev["last_verify"], "worktree": policy,
                    "briefing_digest_before": first.get("briefing_digest"), "briefing_digest": b2["digest"]}
    return a


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--repo", required=True)
    ap.add_argument("--issue", type=int, required=True)
    ap.add_argument("--land", choices=["queue", "pr", "none"], default="pr")
    ap.add_argument("--engine", choices=["briefed", "briefed:codex", "briefed:opencode", "briefed:gemini", "bare"],
                    default="briefed")
    ap.add_argument("--briefer", choices=["filefind", "haiku"], default=os.environ.get("OA_WORK_BRIEFER", "filefind"),
                    help="the briefing's file step: filefind (default) or the Haiku briefer (experimental)")
    ap.add_argument("--checkout")
    ap.add_argument("--out")
    ap.add_argument("--run-id")
    ap.add_argument("--model", default=os.environ.get("OA_WORK_MODEL") or default_model())
    ap.add_argument("--timeout", type=int, default=1500)
    ap.add_argument("--keep", action="store_true", help="keep the worktrees")
    ap.add_argument("--rebrief", action=argparse.BooleanOptionalAction,
                    default=os.environ.get("OA_WORK_REBRIEF", "0") == "1",
                    help="one re-briefed attempt before the bare fallback (#11311)")
    ap.add_argument("--budget-usd", type=float, default=float(os.environ.get("OA_WORK_BUDGET_USD", "8")))
    ap.add_argument("--max-secs", type=int, default=int(os.environ.get("OA_WORK_MAX_SECS", "4500")))
    args = ap.parse_args()

    run_id = args.run_id or uuid.uuid4().hex[:12]
    out = Path(args.out or Path.home() / ".openagents" / "work" / "runs" / run_id)
    run = Run(out)
    result = {"type": "result", "run_id": run_id, "repo": args.repo, "issue": args.issue,
              "requested_engine": args.engine, "engine": None, "escalated": None, "ok": False,
              "land": args.land, "landed": None, "checks": [], "diff": None, "cost_usd": None,
              "secs": None, "attempts": [], "error": None}
    try:
        if missing := signed_in(args.engine):
            raise RuntimeError(missing)
        run.say("start", f"working {args.repo}#{args.issue}", run_id=run_id)
        issue = json.loads(sh(["gh", "issue", "view", str(args.issue), "--repo", args.repo,
                               "--json", "number,title,body,state,url"], timeout=120).stdout)
        result["title"], result["url"] = issue["title"], issue["url"]
        run.say("issue", f"#{issue['number']} {issue['title']}", title=issue["title"])
        if issue["state"] != "OPEN":
            raise RuntimeError(f"issue #{args.issue} is {issue['state'].lower()}")
        checkout = checkout_for(args.repo, args.checkout)
        base = default_branch_sha(checkout)
        result["base"] = base
        engine, why = args.engine, None
        b = None
        if engine.startswith("briefed"):
            run.say("briefing", "finding the files this issue needs")
            t0 = time.time()
            b = briefing_for(issue, base, checkout, briefer=args.briefer)
            conf = confidence_of(b)
            (out / "briefing.md").write_text(b["markdown"])
            result["briefing"] = {"secs": round(time.time() - t0, 1), "finder": b["finder"],
                                  "files": [f["path"] for f in b["files"]], "confidence": conf,
                                  "checks": [c["id"] for c in b["checks"]]}
            if b.get("briefer"):  # the Haiku stage's own time, spend and any fallback
                result["briefing"]["briefer"] = {k: b["briefer"].get(k) for k in
                                                 ("model", "secs", "cost_usd", "turns", "fallback", "error")}
            run.say("briefing", f"briefing ready: {len(b['files'])} files, finder {b['finder']}",
                    files=[f["path"] for f in b["files"]][:8], confidence=conf)
            agent = os.environ.get("OA_BRIEFED_AGENT") or shutil.which("briefed-agent")
            if not agent or not Path(agent).exists():
                engine, why = "bare", "missing capability: the briefed agent isn't installed on this computer"
            elif not b["files"]:
                engine, why = "bare", "low briefing confidence: the finder found no files"
            elif b["finder"].startswith("filefind") and conf is not None and conf < MIN_CONFIDENCE:
                engine, why = "bare", f"low briefing confidence ({conf:.3f} < {MIN_CONFIDENCE})"
        if why:
            run.say("escalate", why)
            result["escalated"] = why
        a = attempt(run, engine, issue, checkout, base, out / engine, args, b)
        result["attempts"].append(a)
        say_cost(run, result["attempts"])
        if engine.startswith("briefed") and not a["ok"]:
            allowed, skip = rebrief_allowed(args, result["attempts"], run.started)
            if allowed:
                a = rebrief_attempt(run, engine, issue, checkout, base, out / f"{engine}-rebrief", args, a)
                result["attempts"].append(a)
                result["rebriefed"] = a["rebrief"]["why"]
                say_cost(run, result["attempts"])
            else:
                result["rebrief_skipped"] = skip
        if engine.startswith("briefed") and not a["ok"]:
            why = failure_reason(a) + (" after a re-brief" if a.get("rebrief") else "")
            if missing := signed_in("bare"):
                run.say("escalate", f"{why}; no fallback: {missing}")
                result["escalated"] = f"{why}; {missing}"
            else:
                run.say("escalate", why + "; falling back to bare Claude Code")
                result["escalated"] = why
                engine = "bare"
                a = attempt(run, engine, issue, checkout, base, out / engine, args, None)
                result["attempts"].append(a)
                say_cost(run, result["attempts"])
        result["engine"], result["ok"] = engine, a["ok"]
        result["checks"], result["diff"] = a["checks"], a["diff"]
        result["cost_usd"] = cost_so_far(result["attempts"])[0]
        if a["ok"]:
            wt = Path(a["worktree"])
            result["commit"] = commit(wt, issue, engine, base)
            run.say("commit", f"committed {result['commit'][:10]}")
            result["landed"] = land(run, wt, args.repo, issue, args.land, run_id, engine, a["checks"])
        else:
            result["error"] = "no change passed its checks"
    except Exception as error:  # noqa: BLE001 - the run reports every failure as its result
        result["error"] = str(error)[-1500:]
    finally:
        result["secs"] = round(time.time() - run.started, 1)
        if not args.keep:
            for a in result["attempts"]:
                try:
                    drop_worktree(Path(a["worktree"]).parent.parent.parent, Path(a["worktree"]))
                except Exception:  # noqa: BLE001
                    pass
        for a in result["attempts"]:
            a.pop("worktree", None)
            a.pop("out", None)
        (out / "result.json").write_text(json.dumps(result, indent=2))
        run.say("done", "done" if result["ok"] else f"stopped: {result['error']}", ok=result["ok"])
        print(json.dumps(result), flush=True)
    return 0 if result["ok"] else 1


if __name__ == "__main__":
    sys.exit(main())
