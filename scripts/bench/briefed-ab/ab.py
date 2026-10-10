#!/usr/bin/env python3
"""The briefed-agent A/B harness (#11211).

    ab.py batch --tag TAG --issues N,N,... --arms A,B[,C] --reps 3 [--workers 2] [lever=value ...]
    ab.py judge --tag TAG
    ab.py report --tag TAG [--tag TAG ...]

Each trial replays one closed issue at its fix's parent commit, in a fresh
worktree of a shallow clone that holds no commit after the parent:

- **Arm A**: bare Claude Code (`claude -p`), told "Complete this issue."
  plus the issue's title and body. Default tools and settings, except that
  it skips permission prompts, WebFetch and WebSearch are off (the fix is
  public), no MCP server loads, and `gh` refuses.
- **Arm B**: the briefed agent (`crates/briefed-agent`) with the `lite`
  briefing: a custom system prompt, Read/Edit/Write/Grep/Glob held to the
  worktree, and `run_check`, which runs only the briefing's checks.
- **Arm C**: arm B with the oracle briefing (the fix's real files): an
  upper bound, never a result.

Every cargo command, from either arm, runs on the build host
(coderos-4080) against the trial's working copy, in one persistent target
dir (`remote/run.sh`); the Mac keeps only the small sparse worktrees.

A trial records wall time, tokens and dollars as the CLI reports them, tool
calls, files read, whether the change compiles, whether the fix's own tests
pass on it (`remote/eval.sh`), overlap with the fix's files, and the full
lever settings. `judge` adds a blind quality score; `report` aggregates
cost per accepted change (passes the fix's tests and the judge).
"""

from __future__ import annotations

import argparse
import json
import os
import random
import re
import shutil
import signal
import statistics
import subprocess
import sys
import threading
import time
from pathlib import Path

import briefing as briefing_mod
from common import HERE, LEVERS, REPO, SSH, WORK, dump_json, is_test_file, load_json, run
from prepare import TASKS, grade, interface_text

SHIM = HERE / "shim"
AGENT_BIN = Path(os.environ.get(
    "AB_AGENT_BIN", str(WORK / "bin" / "briefed-agent-3")))
CLAUDE = shutil.which("claude") or "claude"
SPARSE = ["/*", "!/bench/terminal-bench/", "!/assets/"]
BASE_DEPTH = 50
_base_lock = threading.RLock()


# ---------------------------------------------------------------- worktrees

def base_repo(task: dict) -> Path:
    """A shallow clone holding the parent and its recent history only."""
    path = WORK / "bases" / str(task["issue"])
    with _base_lock:
        if (path / ".git").exists():
            return path
        shutil.rmtree(path, ignore_errors=True)
        path.mkdir(parents=True)
        run(["git", "init", "-q", str(path)])
        run(["git", "-C", str(path), "fetch", "-q", f"--depth={BASE_DEPTH}", f"file://{REPO}", task["parent"]],
            timeout=900)
        run(["git", "-C", str(path), "update-ref", "refs/heads/main", task["parent"]])
        run(["git", "-C", str(path), "symbolic-ref", "HEAD", "refs/heads/main"])
    return path


def make_worktree(task: dict, dest: Path) -> Path:
    base = base_repo(task)
    with _base_lock:  # git worktree and sparse-checkout write the base's config
        shutil.rmtree(dest, ignore_errors=True)
        run(["git", "-C", str(base), "worktree", "prune"], check=False)
        run(["git", "-C", str(base), "worktree", "add", "-f", "-q", "--detach", "--no-checkout", str(dest),
             task["parent"]])
        run(["git", "-C", str(dest), "sparse-checkout", "set", "--no-cone", *SPARSE])
    run(["git", "-C", str(dest), "checkout", "-q", "--detach", task["parent"]], timeout=600)
    return dest


def drop_worktree(task: dict, dest: Path) -> None:
    base = WORK / "bases" / str(task["issue"])
    with _base_lock:
        run(["git", "-C", str(base), "worktree", "remove", "--force", str(dest)], check=False)
        shutil.rmtree(dest, ignore_errors=True)


def working_diff(root: Path, base: str) -> str:
    """Everything the agent changed since `base`, committed or not."""
    idx = run(["git", "-C", str(root), "rev-parse", "--path-format=absolute", "--git-path", "index"]).stdout.strip()
    tmp = Path(str(root) + ".index")
    shutil.copy(idx, tmp)
    env = dict(os.environ, GIT_INDEX_FILE=str(tmp))
    subprocess.run(["git", "-C", str(root), "add", "-A"], env=env, capture_output=True)
    diff = subprocess.run(["git", "-C", str(root), "diff", "--cached", "--binary", base],
                          env=env, capture_output=True, text=True).stdout
    tmp.unlink(missing_ok=True)
    return diff


def diff_files(diff: str) -> list[str]:
    return sorted({line[6:] for line in diff.splitlines() if line.startswith("+++ b/")} |
                  {line[6:] for line in diff.splitlines() if line.startswith("--- a/")})


def trial_env(root: Path, task: dict, slot: int) -> dict:
    env = dict(os.environ)
    for key in ("ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN", "CLAUDECODE", "CLAUDE_CODE_ENTRYPOINT"):
        env.pop(key, None)
    env.update({
        "PATH": f"{SHIM}:{env.get('PATH', '')}",
        "AB_ROOT": str(root), "AB_BASE": task["parent"], "AB_SLOT": str(slot),
    })
    return env


# --------------------------------------------------------------------- arms

_TREES: dict[str, set[str]] = {}


def tree_of(rev: str) -> set[str]:
    if rev not in _TREES:
        _TREES[rev] = set(run(["git", "ls-tree", "-r", "--name-only", rev], cwd=REPO).stdout.splitlines())
    return _TREES[rev]


def bash_reads(command: str, root: Path, tree: set[str]) -> set[str]:
    """Repo files a shell command names (cat, sed, grep ... FILE)."""
    out = set()
    for tok in re.findall(r"[\w./@+-]+\.[A-Za-z0-9]+", command):
        tok = tok.strip("'\"")
        if tok.startswith(str(root)):
            tok = tok[len(str(root)):].lstrip("/")
        tok = tok.lstrip("./") if tok.startswith("./") else tok
        if tok in tree:
            out.add(tok)
    return out


def stream_metrics(events: list[dict], root: Path, rev: str | None = None) -> dict:
    tools: dict[str, int] = {}
    read: set[str] = set()
    tree = tree_of(rev) if rev else set()
    result = None
    for ev in events:
        if ev.get("type") == "assistant":
            for block in (ev.get("message") or {}).get("content") or []:
                if block.get("type") == "tool_use":
                    name = block.get("name", "?")
                    tools[name] = tools.get(name, 0) + 1
                    if name == "Bash" and tree:
                        read |= bash_reads((block.get("input") or {}).get("command", ""), root, tree)
                    path = (block.get("input") or {}).get("file_path")
                    if name == "Read" and path:
                        try:
                            read.add(str(Path(path).resolve().relative_to(root.resolve())))
                        except ValueError:
                            read.add(path)
        if ev.get("type") == "result":
            result = ev["result"] if isinstance(ev.get("result"), dict) else ev
    return {"tool_calls": tools, "files_read": sorted(read), "result": result}


def run_arm_a(task: dict, root: Path, out: Path, levers: dict, slot: int) -> dict:
    prompt = f"Complete this issue.\n\n#{task['issue']} {task['title']}\n\n{task['body'] or ''}"
    argv = [CLAUDE, "-p", prompt, "--output-format", "stream-json", "--verbose",
            "--model", levers["model"], "--dangerously-skip-permissions",
            "--no-session-persistence", "--strict-mcp-config",
            "--disallowedTools", "WebFetch", "WebSearch"]
    if levers["effort"] != "default":
        argv += ["--effort", levers["effort"]]
    if int(levers["max_turns"]):
        argv += ["--max-turns", str(levers["max_turns"])]
    started = time.time()
    timed_out = False
    with open(out / "events.jsonl", "w") as sink, open(out / "stderr.txt", "w") as errf:
        proc = subprocess.Popen(argv, cwd=root, env=trial_env(root, task, slot), stdout=subprocess.PIPE,
                                stderr=errf, text=True, start_new_session=True)

        def pump() -> None:
            # Stamp each event with the time it arrived (t_ms), as arm B's log does.
            for line in proc.stdout:
                try:
                    ev = json.loads(line)
                    ev["t_ms"] = int((time.time() - started) * 1000)
                    sink.write(json.dumps(ev) + "\n")
                except json.JSONDecodeError:
                    sink.write(line)
                sink.flush()

        reader = threading.Thread(target=pump)
        reader.start()
        try:
            proc.wait(timeout=int(levers["timeout_secs"]))
        except subprocess.TimeoutExpired:
            timed_out = True
            os.killpg(proc.pid, signal.SIGKILL)
            proc.wait()
        reader.join(timeout=30)
    err = (out / "stderr.txt").read_text()
    wall = time.time() - started
    events = []
    for line in (out / "events.jsonl").read_text().splitlines():
        try:
            events.append(json.loads(line))
        except json.JSONDecodeError:
            pass
    m = stream_metrics(events, root, task["parent"])
    return {"wall_secs": round(wall, 1), "timed_out": timed_out, "stderr": (err or "")[-800:], **m,
            "misses": None, "check_runs": None}


SYSTEM_V1 = """You are a senior Rust engineer making one change in the OpenAgents monorepo. Your working directory is the repository checkout at the issue's base commit.

Below is a briefing prepared for this issue: a change plan, the files to change with excerpts (line numbers are the file's own), similar past changes, the exact checks, and the repo rules.

How to work:
1. Trust the briefing. Start from the listed files and excerpts; read more of a file only where you need it. Open a file the briefing does not list only when the change, the compiler, or a test demands it.
2. Make the smallest complete change that resolves the issue, in the surrounding style. Add or update a test that pins the new behavior.
3. Run the checks with `run_check`: the `check` first, then the `test` with a filter naming your tests, then `fmt`. Fix what fails and run again.
4. When the checks pass, stop. Reply in two or three lines with what changed. Do not commit.
"""

SYSTEM_V2 = """You fix one issue in the OpenAgents Rust monorepo (cwd = checkout at the base commit). The briefing below is your map: plan, files with line-numbered excerpts, similar past changes, checks, rules.

Rules of work:
- Edit straight from the excerpts. Read a file region only when an excerpt is not enough; never re-read what you already have. Open unlisted files only when an error points there.
- Smallest complete change, house style, plus one test that pins the behavior.
- Verify with `run_check` only: `check:*` after editing, then `test:*` with a filter of your test's name, then `fmt:*`. On failure fix and rerun.
- Stop as soon as checks pass. Final reply: at most two lines. Do not commit.
"""

TEMPLATES = {"v1": SYSTEM_V1, "v2": SYSTEM_V2}


BUILTIN = ["Read", "Edit", "Write", "Grep", "Glob"]


def tool_set(spec: str) -> tuple[list[str], list[str], bool]:
    """A `tools` lever value -> (built-ins, custom tools, run_check server).

    `verify` (B0): the five file tools plus `verify`. `bash` (B-bash): the
    five plus Bash, no verify. `checks`: the pilot's run_check server.
    Extras join with `+`: `verify+related`, `verify+outline` (outline and
    read_symbol), `verify+finish`. A comma list names built-ins directly
    (the pilot's form).
    """
    if "," in spec:
        return spec.split(","), [], True
    parts = spec.split("+")
    builtins, custom, checks = list(BUILTIN), [], False
    base = parts[0]
    if base == "verify":
        custom.append("verify")
    elif base == "bash":
        builtins.append("Bash")
    elif base == "checks":
        checks = True
    for extra in parts[1:]:
        if extra == "outline":
            custom += ["outline", "read_symbol"]
        elif extra == "bash":
            builtins.append("Bash")
        else:
            custom.append(extra)
    return builtins, custom, checks


def how_to_work(builtins: list[str], custom: list[str], checks: bool, pkgs: list[str]) -> str:
    p = pkgs[0] if pkgs else "CRATE"
    lines = []
    if "outline" in custom:
        lines.append("- Read only what you need: `outline` a file to see its items, `read_symbol` to read one item. "
                     "Read whole files only when that is not enough.")
    if "related" in custom:
        lines.append("- `related` tells you what else changes with a file or symbol (history, uses, tests).")
    if "verify" in custom:
        lines.append("- Check your work with `verify`, passing `tests` with your new tests' names. It compiles, runs "
                     "those tests and formats, and returns only what is wrong; fix that and call it again. "
                     "`fast: true` only compiles. Call it once when you think you are done, not after every edit.")
    elif "Bash" in builtins:
        lines.append(f"- Check your work with Bash: `cargo check -p {p} --tests`, then `cargo test -p {p} <your test "
                     f"names>`, then `cargo fmt -p {p}`. Fix what fails and run again.")
    elif checks:
        lines.append("- Check your work with `run_check`: the `check` first, then the `test` with a filter naming "
                     "your tests, then `fmt`. Fix what fails and run again.")
    if "finish" in custom:
        lines.append("- When the checks pass, call `finish` with a short summary; that ends the run.")
    else:
        lines.append("- When the checks pass, stop. Reply in two or three lines with what changed.")
    return "\n".join(lines)


SYSTEM_V3 = """You are a senior Rust engineer making one change in the OpenAgents monorepo. Your working directory is the repository checkout at the issue's base commit.

Below is a briefing for this issue: a change plan, the files to change with excerpts (line numbers are the file's own), similar past changes, the checks, and the repo rules.

How to work:
- Trust the briefing. Start from the listed files and excerpts. Open a file the briefing does not list only when the change, the compiler, or a test demands it.
- Make the smallest complete change that resolves the issue, in the surrounding style, and add or update a test that pins the new behavior.
{tools}
- Do not commit.
"""
TEMPLATES["v3"] = SYSTEM_V3


def cochange(rev: str, paths: list[str]) -> dict:
    """path -> partners that changed with it before `rev` (for verify)."""
    out = {}
    for path in paths[:6]:
        log = run(["git", "log", rev, "-n", "300", "--no-merges", "--format=@@", "--name-only", "--", path],
                  cwd=REPO, check=False).stdout
        counts: dict[str, int] = {}
        for chunk in log.split("@@"):
            files = [f for f in chunk.splitlines() if f.strip()]
            if not files or len(files) > 40:
                continue
            for f in files:
                if f != path and not f.endswith("Cargo.lock"):
                    counts[f] = counts.get(f, 0) + 1
        out[path] = [{"path": f, "count": c} for f, c in sorted(counts.items(), key=lambda x: -x[1])[:6] if c >= 3]
    return out


def run_arm_b(task: dict, root: Path, out: Path, levers: dict, slot: int) -> dict:
    t0 = time.time()
    b = briefing_mod.build(task, levers)
    builtins, custom, checks = tool_set(levers["tools"])
    pkgs = []
    for c in b["checks"]:
        pkg = c["id"].split(":", 1)[1]
        if pkg not in pkgs:
            pkgs.append(pkg)
    co = cochange(task["parent"], [f["path"] for f in b["files"]]) if "verify" in custom else {}
    brief_secs = time.time() - t0
    md = briefing_mod.render(b)
    dump_json(out / "briefing.json", b)
    (out / "briefing.md").write_text(md)
    template = TEMPLATES[levers["template"]]
    if "{tools}" in template:
        template = template.replace("{tools}", how_to_work(builtins, custom, checks, pkgs))
    if levers["briefing_in"] == "system":
        system, prompt = template + "\n\n" + md, f"Complete issue #{task['issue']} as the briefing describes."
    else:
        system, prompt = template, md + f"\n\nComplete issue #{task['issue']} as the briefing above describes."
    (out / "system.md").write_text(system)
    (out / "prompt.md").write_text(prompt)
    env = trial_env(root, task, slot)
    config = {
        "worktree": str(root), "system_prompt_path": str(out / "system.md"), "prompt_path": str(out / "prompt.md"),
        "events_path": str(out / "events.jsonl"), "summary_path": str(out / "summary.json"),
        "briefed_files": [f["path"] for f in b["files"]],
        "model": levers["model"], "effort": None if levers["effort"] == "default" else levers["effort"],
        "max_turns": int(levers["max_turns"]) or None, "timeout_secs": int(levers["timeout_secs"]),
        "tools": builtins, "custom": custom,
        "finish_path": str(out / "finish.json"),
        "related": {"repo": str(REPO), "rev": task["parent"]},
    }
    if checks:
        mcp_env = {k: env[k] for k in ("PATH", "AB_ROOT", "AB_BASE", "AB_SLOT", "HOME") if k in env}
        mcp_env.update({"AB_CHECKS": json.dumps(b["checks"]), "AB_CHECK_OUTPUT": levers["check_output"],
                        "AB_CHECK_LOG": str(out / "checks.jsonl")})
        config["mcp"] = {"checks": {"command": sys.executable, "args": [str(HERE / "checks_mcp.py")], "env": mcp_env}}
    if "verify" in custom or "finish" in custom:
        v = TASKS / str(task["issue"]) / "validation.json"
        baseline = load_json(v)["on_parent"].get("errors", []) if v.exists() else []
        config["verify"] = {
            "exec": str(SHIM / "remote-exec"), "crates": pkgs,
            "baseline_errors": [e for e in baseline if not e.startswith("error: could not compile")],
            "done_when": [p.removeprefix("Required: ") for p in b["plan"] if p.startswith("Required: ")]
            or [task["title"]],
            "cochange": co, "log": str(out / "verify.jsonl"),
        }
    dump_json(out / "agent.json", config)
    started = time.time()
    proc = subprocess.run([str(AGENT_BIN), str(out / "agent.json")], cwd=root, env=env,
                          capture_output=True, text=True, timeout=int(levers["timeout_secs"]) + 300)
    wall = time.time() - started + brief_secs
    summary = load_json(out / "summary.json") if (out / "summary.json").exists() else {}
    return {
        "wall_secs": round(wall, 1), "briefing_secs": round(brief_secs, 2), "briefing_chars": len(md),
        "briefing_files": [f["path"] for f in b["files"]], "finder_used": b["finder"],
        "timed_out": summary.get("timed_out", False), "stderr": proc.stderr[-800:],
        "tool_calls": summary.get("tool_calls", {}), "files_read": summary.get("files_read", []),
        "misses": summary.get("misses", []), "check_runs": summary.get("check_runs"),
        "denied": summary.get("denied", []), "result": summary.get("result"),
        "finished": summary.get("finished"),
    }


# -------------------------------------------------------------------- trial

def usage_of(result: dict | None) -> dict:
    if not result:
        return {"cost_usd": None, "turns": None, "input_tokens": None, "output_tokens": None,
                "cache_read": None, "cache_write": None, "is_error": True, "subtype": None}
    u = result.get("usage") or {}
    return {
        "cost_usd": result.get("total_cost_usd"), "turns": result.get("num_turns"),
        "input_tokens": u.get("input_tokens"), "output_tokens": u.get("output_tokens"),
        "cache_read": u.get("cache_read_input_tokens"), "cache_write": u.get("cache_creation_input_tokens"),
        "is_error": result.get("is_error"), "subtype": result.get("subtype"),
        "api_error_status": result.get("api_error_status"),
    }


def _errs(g: dict) -> set[str]:
    return {e for e in g.get("errors") or [] if not e.startswith("error: could not compile")}


def new_errors(task: dict, g: dict) -> list[str]:
    """Compile errors the change added over the parent's own (some parents
    already fail to build an unrelated test target)."""
    v = TASKS / str(task["issue"]) / "validation.json"
    base = _errs(load_json(v)["on_parent"]) if v.exists() else set()
    return sorted(_errs(g) - base)


def compiles_vs_parent(task: dict, g: dict) -> bool | None:
    if g.get("empty"):
        return None
    if not g.get("applied", True):
        return False
    if g.get("compiles"):
        return True
    return not new_errors(task, g)


PATH_RE = re.compile(r"(?:crates|apps|docs|scripts|bins|os)/[\w./@+-]+\.[A-Za-z0-9]+")


def tool_log(events: list[dict], root: Path) -> list[dict]:
    """One row per tool call: tokens in and out (chars / 4), seconds, and
    whether the agent acted on the result (its next edit touched a file
    the result named)."""
    uses, results, order = {}, {}, []
    prefix = str(root.resolve()) + "/"
    for ev in events:
        t = ev.get("t_ms")
        msg = ev.get("message") or {}
        for block in msg.get("content") or [] if isinstance(msg.get("content"), list) else []:
            if not isinstance(block, dict):
                continue
            if block.get("type") == "tool_use":
                uses[block["id"]] = (block.get("name"), block.get("input") or {}, t)
                order.append(block["id"])
            elif block.get("type") == "tool_result":
                content = block.get("content")
                text = content if isinstance(content, str) else json.dumps(content)
                results[block.get("tool_use_id")] = (text, t)
    edits = []
    for i, uid in enumerate(order):
        name, inp, _ = uses[uid]
        if name in ("Edit", "Write", "MultiEdit"):
            edits.append((i, (inp.get("file_path") or "").replace(prefix, "")))
    rows = []
    for i, uid in enumerate(order):
        name, inp, t0 = uses[uid]
        text, t1 = results.get(uid, ("", None))
        named = {p for p in PATH_RE.findall(text.replace(prefix, ""))}
        nxt = next((f for j, f in edits if j > i), None)
        rows.append({
            "tool": name, "tokens_in": len(json.dumps(inp)) // 4, "tokens_out": len(text) // 4,
            "secs": round((t1 - t0) / 1000, 1) if t0 is not None and t1 is not None else None,
            "named_files": len(named), "acted_on": bool(nxt and nxt in named) if named else None,
        })
    return rows


# Arms (docs/inference/briefed-agent-tools.md): every arm but A is the
# briefed agent with these levers over the batch's.
ARMS: dict[str, dict] = {
    "B": {},                                   # the batch's own levers
    "B0": {"tools": "verify"},                 # briefed, minimal tools + verify
    "Bbash": {"tools": "bash"},                # briefed, Bash instead of verify
    "Bchecks": {"tools": "checks"},            # round 0's run_check server
    "C": {"finder": "oracle"},                 # oracle briefing, B0's tools
    "Brelated": {"tools": "verify+related"},
    "Boutline": {"tools": "verify+outline"},
    "Bfinish": {"tools": "verify+finish"},
}


class RateLimited(RuntimeError):
    pass


def limited(m: dict) -> bool:
    res = m.get("result") or {}
    text = json.dumps(res)[:4000] if res else ""
    return res.get("api_error_status") == 429 or "hit your" in text or "usage limit" in text


def wait_for_login() -> None:
    """Block until the Claude Code login answers again (after a limit)."""
    while True:
        proc = subprocess.run([CLAUDE, "-p", "Reply ok", "--output-format", "json", "--model", "haiku"],
                              capture_output=True, text=True, cwd=str(WORK))
        try:
            if not json.loads(proc.stdout).get("is_error"):
                return
        except json.JSONDecodeError:
            pass
        print("login is at its usage limit; waiting 10 minutes", flush=True)
        time.sleep(600)


def run_trial(task: dict, arm: str, rep: int, levers: dict, slot: int, out: Path) -> dict:
    out.mkdir(parents=True, exist_ok=True)
    lv = dict(levers)
    lv.update(ARMS.get(arm, {}))
    if lv.get("interface"):
        extra = interface_text(task)
        if extra:
            task = dict(task, body=(task["body"] or "") + extra)
    (out / "issue.md").write_text(f"#{task['issue']} {task['title']}\n\n{task['body'] or ''}")
    root = make_worktree(task, WORK / "wt" / f"slot{slot}")
    try:
        if arm == "A":
            m = run_arm_a(task, root, out, lv, slot)
        else:
            m = run_arm_b(task, root, out, lv, slot)
        if limited(m):
            raise RateLimited("the login hit its usage limit during the trial")
        diff = working_diff(root, task["parent"])
        if lv.get("post_fmt") and diff.strip() and task.get("package"):
            subprocess.run([str(SHIM / "remote-exec"), "cargo", "fmt", "-p", task["package"]],
                           cwd=root, env=trial_env(root, task, slot), capture_output=True, timeout=600)
            diff = working_diff(root, task["parent"])
    finally:
        drop_worktree(task, root)
    (out / "change.patch").write_text(diff)
    events = []
    if (out / "events.jsonl").exists():
        for line in (out / "events.jsonl").read_text().splitlines():
            try:
                events.append(json.loads(line))
            except json.JSONDecodeError:
                pass
    tlog = tool_log(events, root)
    changed = diff_files(diff)
    fix_src = [p for p in task["source_files"] if not p.endswith(".md")]
    src_changed = [p for p in changed if not is_test_file(p) and not p.endswith(".md") and not p.endswith(".lock")]
    hit = set(src_changed) & set(fix_src)
    g = grade(task, diff, slot) if diff.strip() else {"applied": True, "compiles": None, "tests_pass": False, "empty": True}
    record = {
        "issue": task["issue"], "arm": arm, "rep": rep, "slot": slot, "levers": lv,
        **{k: v for k, v in m.items() if k not in ("result",)},
        **usage_of(m.get("result")),
        "files_changed": changed,
        "tool_log": tlog,
        "overlap_recall": round(len(hit) / max(1, len(fix_src)), 2),
        "overlap_precision": round(len(hit) / max(1, len(src_changed)), 2) if src_changed else 0.0,
        "diff_lines": sum(1 for l in diff.splitlines() if l[:1] in "+-" and l[:3] not in ("+++", "---")),
        "grade": g,
        "compiles": compiles_vs_parent(task, g), "compiles_raw": g.get("compiles"),
        "new_errors": new_errors(task, g)[:10],
        "tests_pass": bool(g.get("tests_pass")),
        "finished_at": time.strftime("%Y-%m-%dT%H:%M:%S"),
    }
    dump_json(out / "result.json", record)
    return record


MIN_FREE_GB = 60


def disk_guard() -> None:
    """Keep the build host above its free-space floor (50 GB, with margin):
    below MIN_FREE_GB the bench's own target dir is cleared (a cold build
    beats a full disk). Never touches ~/openagents/target."""
    free = run(SSH + ["df -BG --output=avail / | tail -1"], check=False).stdout.strip().rstrip("G")
    if free.isdigit() and int(free) < MIN_FREE_GB:
        print(f"build host has {free} GB free; clearing ~/ab/target", flush=True)
        run(SSH + ["flock ~/ab/build.lock sh -c 'rm -rf ~/ab/target && mkdir -p ~/ab/target'"],
            check=False, timeout=1800)


def prewarm(task: dict, slot: int) -> float:
    """Build the parent's package and tests in the slot (the warm cache)."""
    t0 = time.time()
    pkg = task["package"]
    run(SSH + [f"bash ~/ab/bin/run.sh {slot} {task['parent']} . 2400 -- cargo test -p {pkg} --no-run -q"],
        input="", check=False, timeout=3000)
    return time.time() - t0


def batch(args) -> None:
    levers = dict(LEVERS)
    for kv in args.levers:
        k, v = kv.split("=", 1)
        levers[k] = (v.lower() in ("1", "true", "yes")) if isinstance(LEVERS[k], bool) else type(LEVERS[k])(v)
    tag_dir = WORK / "results" / args.tag
    tag_dir.mkdir(parents=True, exist_ok=True)
    dump_json(tag_dir / "levers.json", levers)
    issues = [int(x) for x in args.issues.split(",")]
    arms = args.arms.split(",")
    # Issues run one at a time and both workers share the issue's base
    # commit: the build host has one build checkout, so trials of one issue
    # only rebuild what their patches touch.
    for issue in issues:
        task = load_json(TASKS / str(issue) / "task.json")
        order = []
        for rep in range(args.reps):
            rot = arms[rep % len(arms):] + arms[: rep % len(arms)]
            order += [(arm, rep) for arm in rot]
        pending = [(a, r) for a, r in order if not (tag_dir / f"{issue}-{a}-{r}" / "result.json").exists()]
        if not pending:
            continue
        disk_guard()
        if levers["build_cache"] == "warm":
            secs = prewarm(task, args.first_slot)
            print(f"{issue} prewarmed in {secs:.0f}s", flush=True)
        lock = threading.Lock()

        def worker(slot: int) -> None:
            while True:
                with lock:
                    if not pending:
                        return
                    arm, rep = pending.pop(0)
                out = tag_dir / f"{issue}-{arm}-{rep}"
                if levers["build_cache"] == "cold":
                    with _base_lock:
                        run(SSH + ["flock ~/ab/build.lock sh -c 'rm -rf ~/ab/target && mkdir -p ~/ab/target'"], check=False)
                try:
                    try:
                        r = run_trial(task, arm, rep, levers, slot, out)
                    except RateLimited:
                        shutil.rmtree(out, ignore_errors=True)
                        wait_for_login()
                        r = run_trial(task, arm, rep, levers, slot, out)
                    print(f"[slot{slot}] {issue} {arm}{rep}: ${r['cost_usd']} {r['wall_secs']}s "
                          f"compiles={r['compiles']} tests={r['tests_pass']} overlap={r['overlap_recall']}", flush=True)
                except Exception as error:  # noqa: BLE001 - record and go on
                    print(f"[slot{slot}] {issue} {arm}{rep}: harness error {error}", flush=True)
                    out.mkdir(parents=True, exist_ok=True)
                    (out / "error.txt").write_text(str(error))

        threads = [threading.Thread(target=worker, args=(args.first_slot + s,)) for s in range(args.workers)]
        for t in threads:
            t.start()
        for t in threads:
            t.join()
        if not args.keep_bases:
            shutil.rmtree(WORK / "bases" / str(issue), ignore_errors=True)


def regrade(args) -> None:
    """Grade saved changes again (after a grader fix)."""
    for tag in args.tag:
        for d in sorted((WORK / "results" / tag).iterdir()):
            if not (d / "result.json").exists() or (args.issue and int(d.name.split("-")[0]) not in args.issue):
                continue
            r = load_json(d / "result.json")
            task = load_json(TASKS / str(r["issue"]) / "task.json")
            diff = (d / "change.patch").read_text()
            g = grade(task, diff, r["slot"]) if diff.strip() else r["grade"]
            r.update({"grade": g, "compiles": compiles_vs_parent(task, g), "compiles_raw": g.get("compiles"),
                      "new_errors": new_errors(task, g)[:10], "tests_pass": bool(g.get("tests_pass"))})
            dump_json(d / "result.json", r)
            print(d.name, r["compiles"], r["tests_pass"], flush=True)


# -------------------------------------------------------------------- judge

JUDGE = """You are reviewing a proposed code change for a GitHub issue in a Rust monorepo. You also get the change the maintainers actually merged, as a reference. Different code can be equally correct: judge whether the proposed change resolves the issue as well as the reference would, is correct, and is something a careful maintainer would merge. Ignore formatting-only differences and differences in test naming.

Answer with only a JSON object: {{"score": 1-5, "accept": true|false, "reason": "one sentence"}}
Scores: 5 as good as or better than the reference; 4 resolves the issue with minor problems; 3 partially resolves it or has a real problem; 2 mostly wrong; 1 no meaningful change or harmful. Accept means score >= 4.

## Issue #{issue}: {title}

{body}

## Reference change (merged)

```diff
{reference}
```

## Proposed change

```diff
{candidate}
```
"""


def judge_one(task: dict, result_dir: Path, model: str) -> dict:
    candidate = (result_dir / "change.patch").read_text()
    if not candidate.strip():
        return {"score": 1, "accept": False, "reason": "no change", "cost_usd": 0}
    tdir = TASKS / str(task["issue"])
    reference = (tdir / "source.patch").read_text() + (tdir / "tests.patch").read_text()
    prompt = JUDGE.format(issue=task["issue"], title=task["title"], body=(task["body"] or "")[:6000],
                          reference=reference[:30000], candidate=candidate[:30000])
    env = dict(os.environ)
    env.pop("ANTHROPIC_API_KEY", None)
    proc = subprocess.run([CLAUDE, "-p", prompt, "--output-format", "json", "--model", model, "--tools", "",
                           "--system-prompt", "You review code changes and answer only with the JSON asked for.",
                           "--no-session-persistence", "--strict-mcp-config"],
                          cwd=str(WORK), capture_output=True, text=True, timeout=600, env=env)
    try:
        outer = json.loads(proc.stdout)
        text = outer.get("result", "")
        start, end = text.find("{"), text.rfind("}")
        verdict = json.loads(text[start : end + 1])
        verdict["cost_usd"] = outer.get("total_cost_usd")
        verdict["model"] = model
        return verdict
    except (json.JSONDecodeError, ValueError):
        return {"error": (proc.stdout + proc.stderr)[-500:]}


def judge(args) -> None:
    dirs = []
    for tag in args.tag:
        dirs += sorted(p for p in (WORK / "results" / tag).iterdir() if (p / "result.json").exists())
    random.shuffle(dirs)

    def one(d: Path) -> None:
        if (d / "judge.json").exists() and "error" not in load_json(d / "judge.json"):
            return
        r = load_json(d / "result.json")
        task = load_json(TASKS / str(r["issue"]) / "task.json")
        v = judge_one(task, d, args.model)
        dump_json(d / "judge.json", v)
        print(d.name, v.get("score"), v.get("accept"), flush=True)

    pending = list(dirs)
    lock = threading.Lock()

    def worker():
        while True:
            with lock:
                if not pending:
                    return
                d = pending.pop()
            one(d)

    threads = [threading.Thread(target=worker) for _ in range(args.workers)]
    for t in threads:
        t.start()
    for t in threads:
        t.join()


# ------------------------------------------------------------------- report

def rows_of(tags: list[str]) -> list[dict]:
    rows = []
    for tag in tags:
        for d in sorted((WORK / "results" / tag).iterdir()):
            if not (d / "result.json").exists():
                continue
            r = load_json(d / "result.json")
            j = load_json(d / "judge.json") if (d / "judge.json").exists() else {}
            r["tag"] = tag
            r["judge_score"] = j.get("score")
            r["judge_accept"] = j.get("accept")
            r["accepted"] = bool(r["tests_pass"] and j.get("accept"))
            r["accepted_judge"] = bool(r["compiles"] and j.get("accept"))
            rows.append(r)
    return rows


def summarize(rows: list[dict]) -> dict:
    def med(xs):
        xs = [x for x in xs if x is not None]
        return round(statistics.median(xs), 3) if xs else None

    def sd(xs):
        xs = [x for x in xs if x is not None]
        return round(statistics.pstdev(xs), 3) if len(xs) > 1 else None

    n = len(rows)
    cost = sum(r["cost_usd"] or 0 for r in rows)
    acc = sum(r["accepted"] for r in rows)
    acc_j = sum(r["accepted_judge"] for r in rows)
    by_issue: dict[int, list[bool]] = {}
    for r in rows:
        by_issue.setdefault(r["issue"], []).append(r["accepted"])
    consistent = sum(1 for v in by_issue.values() if len(set(v)) == 1)
    return {
        "trials": n,
        "accepted": acc,
        "accept_rate": round(acc / n, 3) if n else None,
        "tests_pass_rate": round(sum(r["tests_pass"] for r in rows) / n, 3) if n else None,
        "compile_rate": round(sum(bool(r["compiles"]) for r in rows) / n, 3) if n else None,
        "judge_mean": round(statistics.mean([r["judge_score"] for r in rows if r["judge_score"]]), 2)
        if any(r["judge_score"] for r in rows) else None,
        "total_cost": round(cost, 2),
        "cost_per_accepted": round(cost / acc, 2) if acc else None,
        "accepted_judge": acc_j,
        "accept_judge_rate": round(acc_j / n, 3) if n else None,
        "cost_per_accepted_judge": round(cost / acc_j, 2) if acc_j else None,
        "median_cost": med([r["cost_usd"] for r in rows]),
        "sd_cost": sd([r["cost_usd"] for r in rows]),
        "median_secs": med([r["wall_secs"] for r in rows]),
        "sd_secs": sd([r["wall_secs"] for r in rows]),
        "median_turns": med([r["turns"] for r in rows]),
        "median_tool_calls": med([sum((r["tool_calls"] or {}).values()) for r in rows]),
        "median_files_read": med([len(r["files_read"] or []) for r in rows]),
        "median_output_tokens": med([r["output_tokens"] for r in rows]),
        "overlap_recall": round(statistics.mean([r["overlap_recall"] for r in rows]), 2) if rows else None,
        "issues_all_reps_same": f"{consistent}/{len(by_issue)}",
        "timeouts": sum(1 for r in rows if r.get("timed_out")),
    }


def tool_stats(rows: list[dict]) -> dict:
    """Per tool: calls per run, share of runs that used it, tokens in and
    out per call, seconds per call, and how often the next edit acted on a
    file the result named."""
    stats: dict[str, dict] = {}
    for r in rows:
        used = set()
        for t in r.get("tool_log") or []:
            s_ = stats.setdefault(t["tool"], {"calls": 0, "tin": 0, "tout": 0, "secs": [], "acted": [], "runs": 0})
            s_["calls"] += 1
            s_["tin"] += t["tokens_in"]
            s_["tout"] += t["tokens_out"]
            if t.get("secs") is not None:
                s_["secs"].append(t["secs"])
            if t.get("acted_on") is not None:
                s_["acted"].append(t["acted_on"])
            used.add(t["tool"])
        for name in used:
            stats[name]["runs"] += 1
    n = max(1, len(rows))
    return {
        name: {
            "calls_per_run": round(s_["calls"] / n, 2),
            "share_of_runs": round(s_["runs"] / n, 2),
            "tokens_in_per_call": round(s_["tin"] / s_["calls"]),
            "tokens_out_per_call": round(s_["tout"] / s_["calls"]),
            "median_secs": round(statistics.median(s_["secs"]), 1) if s_["secs"] else None,
            "acted_on": round(sum(s_["acted"]) / len(s_["acted"]), 2) if s_["acted"] else None,
        }
        for name, s_ in sorted(stats.items(), key=lambda kv: -kv[1]["calls"])
    }


def report(args) -> None:
    rows = rows_of(args.tag)
    arms = sorted({(r["tag"], r["arm"]) for r in rows})
    out = {f"{t}/{a}": summarize([r for r in rows if r["tag"] == t and r["arm"] == a]) for t, a in arms}
    if args.tools:
        out = {f"{t}/{a}": tool_stats([r for r in rows if r["tag"] == t and r["arm"] == a]) for t, a in arms}
    if args.issues:
        behavioral = {int(i) for i in args.issues.split(",")}
        rows = [r for r in rows if r["issue"] in behavioral]
        out = {f"{t}/{a}": summarize([r for r in rows if r["tag"] == t and r["arm"] == a]) for t, a in arms}
    print(json.dumps(out, indent=2))
    if args.csv:
        import csv
        keys = ["tag", "issue", "arm", "rep", "accepted", "accepted_judge", "tests_pass", "compiles", "judge_score", "cost_usd",
                "wall_secs", "turns", "output_tokens", "cache_read", "cache_write", "overlap_recall",
                "diff_lines", "timed_out"]
        with open(args.csv, "w", newline="") as f:
            w = csv.DictWriter(f, fieldnames=keys + ["tool_calls", "files_read", "misses"])
            w.writeheader()
            for r in rows:
                row = {k: r.get(k) for k in keys}
                row["tool_calls"] = sum((r.get("tool_calls") or {}).values())
                row["files_read"] = len(r.get("files_read") or [])
                row["misses"] = len(r.get("misses") or []) if r.get("misses") is not None else ""
                w.writerow(row)


def main() -> None:
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    b = sub.add_parser("batch")
    b.add_argument("--tag", required=True)
    b.add_argument("--issues", required=True)
    b.add_argument("--arms", default="A,B")
    b.add_argument("--reps", type=int, default=3)
    b.add_argument("--workers", type=int, default=2)
    b.add_argument("--keep-bases", action="store_true")
    b.add_argument("--first-slot", type=int, default=0)
    b.add_argument("levers", nargs="*")
    j = sub.add_parser("judge")
    j.add_argument("--tag", action="append", required=True)
    j.add_argument("--model", default="claude-opus-5-5")
    j.add_argument("--workers", type=int, default=2)
    r = sub.add_parser("report")
    r.add_argument("--tag", action="append", required=True)
    r.add_argument("--csv")
    r.add_argument("--tools", action="store_true", help="per-tool statistics")
    r.add_argument("--issues", help="only these issues (e.g. the behavioral subset)")
    g = sub.add_parser("regrade")
    g.add_argument("--tag", action="append", required=True)
    g.add_argument("--issue", type=int, action="append")
    args = ap.parse_args()
    {"batch": batch, "judge": judge, "report": report, "regrade": regrade}[args.cmd](args)


if __name__ == "__main__":
    main()
