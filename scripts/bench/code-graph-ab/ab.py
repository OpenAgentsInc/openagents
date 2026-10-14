#!/usr/bin/env python3
"""The Code Graph A/B (#11622): Coder with and without the graph tools.

    ab.py run --issues N,N,... --reps 2 [--workers 4] [--tag TAG]
    ab.py report --tag TAG

Each trial replays one closed issue from the briefed-agent bench's task set
(`scripts/bench/briefed-ab/tasks`) at its fix's parent commit, in a fresh
worktree of a shallow clone that holds no commit after the parent (the same
isolation as `briefed-ab/ab.py`'s `base_repo`), with a fresh `HOME`, the
Gemini API key, and no browser. Coder runs headless (`openagents coder
chat --mode coder --in-place --json`) as its own loop (`CODER_NO_HARNESS=1`)
on `google/gemini-4-argon` through Google's Gemini API (`AB_MODEL` changes
it; no account is signed in, so no gateway door is tried). The arms differ
only in `OPENAGENTS_CODE_GRAPH` (0 or 1), which turns the `graph_*` tools
and their one line of guidance off or on.

The task is localization: find the existing files a fix changes, without
editing or building. The graph for each base is built once before the
trials (the published graph's stand-in); its build time is recorded.

Per trial, from Coder's ATIF trajectory: total tokens, tokens until the
first Read of a file the fix changed, files read, tool calls by name, and
the answer's recall and precision against the fix's changed source files.
A trial is correct when the answer names at least half of them.

Environment: `OPENAGENTS_BIN` (the `openagents` binary built with the
graph tools), `CODE_GRAPH_BIN` (`code-graph`), `AB_REPO` (the full
repository to clone from), `AB_WORK` (where clones, worktrees and results
go), `AB_KEY_FILE` (an env file with `GEMINI_API_KEY`; never printed).
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import statistics
import subprocess
import sys
import threading
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
TASKS = HERE.parent / "briefed-ab" / "tasks"
REPO = Path(os.environ.get("AB_REPO", str(Path.home() / "work" / "openagents")))
WORK = Path(os.environ.get("AB_WORK", str(HERE / ".work")))
OPENAGENTS = os.environ.get("OPENAGENTS_BIN", "openagents")
CODE_GRAPH = os.environ.get("CODE_GRAPH_BIN", "code-graph")
TOKEN_FILE = Path(os.environ.get(
    "AB_KEY_FILE", str(Path.home() / "work" / ".secrets" / "openagents-gemini-provider.env")))
BASE_DEPTH = 50
MODEL = os.environ.get("AB_MODEL", "google/gemini-4-argon")

TIMEOUT = 900
_lock = threading.Lock()

PROMPT = """This is a GitHub issue for this repository. The repository is checked out at the commit before the issue was fixed.

# {title}

{body}

Do not edit any files and do not run cargo. Find where in the code this has to change. Reply with the existing source files a fix would change, most important first (at most 6), one per line as `path - the function or type to change`."""


def run(cmd, cwd=None, env=None, timeout=None, check=True):
    done = subprocess.run(cmd, cwd=cwd, env=env, timeout=timeout,
                          capture_output=True, text=True)
    if check and done.returncode != 0:
        raise RuntimeError(f"{cmd[:3]} failed: {done.stderr[-400:]}")
    return done


def token_env() -> dict:
    """The model's key from the env file; never printed."""
    env = {}
    for line in TOKEN_FILE.read_text().splitlines():
        name, _, value = line.partition("=")
        if name in ("GEMINI_API_KEY", "GOOGLE_API_KEY") and value.strip():
            env["GEMINI_API_KEY"] = value.strip().strip('"')
    if "GEMINI_API_KEY" not in env:
        sys.exit(f"{TOKEN_FILE} has no GEMINI_API_KEY")
    return env


IMPACT_PROMPT = """In this repository, `{symbol}` (defined in `{file}`) is about to gain a new required parameter. Without editing anything or running cargo, find every place that calls it directly and so must be updated. Reply with one line per call site as `path:line - the calling function`, and nothing else."""


def load_impact(path: Path) -> list[dict]:
    """Impact tasks: a symbol at a commit, and the files that call it
    (found with `git grep` and checked by hand, not with the graph)."""
    tasks = []
    for entry in json.loads(path.read_text()):
        tasks.append({
            "issue": entry["id"], "parent": entry["commit"], "truth": entry["callers"],
            "prompt": IMPACT_PROMPT.format(symbol=entry["symbol"], file=entry["file"]),
            "pass_recall": 1.0, "max_answer": 30, "base": "impact-" + entry["commit"][:10],
        })
    return tasks


def load_task(issue: int) -> dict:
    task = json.loads((TASKS / str(issue) / "task.json").read_text())
    out = run(["git", "-C", str(REPO), "diff", "--name-status", task["parent"], task["fix"]]).stdout
    changed = []
    for line in out.splitlines():
        status, *paths = line.split("\t")
        path = paths[-1]
        if status.startswith("M") and path.endswith(".rs") and not is_test(path):
            changed.append(path)
    task["truth"] = changed
    return task


def is_test(path: str) -> bool:
    name = path.rsplit("/", 1)[-1]
    return ("/tests/" in path or name in ("tests.rs",) or name.endswith("_tests.rs")
            or name.startswith("test_"))


def base_repo(task: dict) -> Path:
    """A shallow clone holding the parent and its recent history only."""
    path = WORK / "bases" / str(task.get("base", task["issue"]))
    with _lock:
        if (path / ".git").exists():
            return path
        shutil.rmtree(path, ignore_errors=True)
        path.mkdir(parents=True)
        run(["git", "init", "-q", str(path)])
        run(["git", "-C", str(path), "fetch", "-q", f"--depth={BASE_DEPTH}",
             f"file://{REPO}", task["parent"]], timeout=900)
        run(["git", "-C", str(path), "update-ref", "refs/heads/main", task["parent"]])
        run(["git", "-C", str(path), "symbolic-ref", "HEAD", "refs/heads/main"])
    return path


def prebuild(task: dict) -> dict:
    """Build the base's graph once, as the published graph would be."""
    base = base_repo(task)
    tree = WORK / "trees" / "prebuild" / str(task.get("base", task["issue"]))
    shutil.rmtree(tree, ignore_errors=True)
    run(["git", "-C", str(base), "worktree", "prune"])
    run(["git", "-C", str(base), "worktree", "add", "-q", "--detach", str(tree), task["parent"]])
    started = time.time()
    env = dict(os.environ, CODE_GRAPH_NO_FETCH="1")
    out = run([CODE_GRAPH, "query", "graph_symbol", '{"symbol":"main"}', "--root", str(tree)],
              env=env, timeout=600)
    run(["git", "-C", str(base), "worktree", "remove", "--force", str(tree)], check=False)
    line = out.stdout.splitlines()[0] if out.stdout else ""
    built = re.search(r"built in (\d+) ms", line)
    return {"issue": task["issue"], "wall_s": round(time.time() - started, 1),
            "build_ms": int(built.group(1)) if built else None}


def trial(task: dict, arm: str, rep: int, tag: str, secrets: dict) -> dict:
    name = f"{task['issue']}-{arm}-{rep}"
    out_dir = WORK / "results" / tag
    out_dir.mkdir(parents=True, exist_ok=True)
    result_file = out_dir / f"{name}.json"
    if result_file.exists():
        return json.loads(result_file.read_text())
    base = base_repo(task)
    tree = WORK / "trees" / tag / name
    home = WORK / "homes" / tag / name
    shutil.rmtree(tree, ignore_errors=True)
    shutil.rmtree(home, ignore_errors=True)
    home.mkdir(parents=True)
    with _lock:
        run(["git", "-C", str(base), "worktree", "add", "-q", "--detach", str(tree), task["parent"]])
    prompt = home / "prompt.md"
    prompt.write_text(task.get("prompt") or PROMPT.format(title=task["title"], body=task["body"][:4000]))
    env = {
        "PATH": os.environ["PATH"], "HOME": str(home), "BROWSER": "/usr/bin/true",
        "NO_OPEN_BROWSER": "1", "OPENAGENTS_TEST_RUN": "1", "CODE_GRAPH_NO_FETCH": "1",
        "OPENAGENTS_CODE_GRAPH": "1" if arm == "B" else "0", "TERM": "dumb",
        # Coder's own loop on one model, through Google's Gemini API with
        # the key below: no account sign-in, so no gateway door.
        "CODER_NO_HARNESS": "1", "CODER_NO_HARNESS_MODEL": MODEL,
        **secrets,
    }
    started = time.time()
    try:
        done = subprocess.run(
            [OPENAGENTS, "coder", "chat", "--prompt-file", str(prompt), "--mode", "coder",
             "--in-place", "--json"],
            cwd=tree, env=env, capture_output=True, text=True, timeout=TIMEOUT)
        stdout, stderr, code = done.stdout, done.stderr, done.returncode
    except subprocess.TimeoutExpired as expired:
        text = lambda b: b.decode(errors="replace") if isinstance(b, bytes) else (b or "")
        stdout, stderr, code = text(expired.stdout), text(expired.stderr), "timeout"
    wall = time.time() - started
    (out_dir / f"{name}.out").write_text(stdout)
    (out_dir / f"{name}.err").write_text(stderr)
    record = {"issue": task["issue"], "arm": arm, "rep": rep, "exit": code,
              "wall_s": round(wall, 1), "truth": task["truth"]}
    record["pass_recall"] = task.get("pass_recall", 0.5)
    record["max_answer"] = task.get("max_answer", 6)
    record.update(score(stdout, home, task["truth"], record["pass_recall"], record["max_answer"]))
    record["stalled"] = "Nothing came back from the model" in stdout
    result_file.write_text(json.dumps(record, indent=1))
    with _lock:
        run(["git", "-C", str(base), "worktree", "remove", "--force", str(tree)], check=False)
    return record


PATH_RE = re.compile(r"(?:crates|bins|src|apps)/[A-Za-z0-9_./-]+\.rs")


def score(stdout: str, home: Path, truth: list[str], pass_recall: float = 0.5,
          max_answer: int = 6) -> dict:
    trajectory = None
    reply = ""
    for line in stdout.splitlines():
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        if event.get("event") == "finished":
            reply = event.get("reply") or ""
            trajectory = event.get("trajectory")
    steps = []
    model = None
    if not trajectory:
        # A run that ended without its `finished` event still saved its
        # trajectory as it went.
        saved = sorted((home / ".openagents" / "coder" / "sessions").glob("*.atif.json"))
        trajectory = str(saved[-1]) if saved else None
    if trajectory and Path(trajectory).exists():
        document = json.loads(Path(trajectory).read_text())
        steps = document.get("steps", [])
        model = document.get("agent", {}).get("model_name")
    total = 0
    prompt_tokens = cached = completion = 0
    first_hit = None
    reads: list[str] = []
    tools: dict[str, int] = {}
    truth_set = set(truth)
    for step in steps:
        metrics = step.get("metrics") or {}
        total += (metrics.get("prompt_tokens") or 0) + (metrics.get("completion_tokens") or 0)
        for call in (step.get("extra") or {}).get("model_calls") or []:
            prompt_tokens += call.get("prompt_tokens") or 0
            cached += call.get("cached_tokens") or 0
            completion += call.get("completion_tokens") or 0
        for call in step.get("tool_calls") or []:
            name = call.get("function_name", "")
            tools[name] = tools.get(name, 0) + 1
            args = call.get("arguments") or {}
            touched = []
            if name == "Read":
                touched = [str(args.get("path", ""))]
            elif name == "Run":
                command = str(args.get("command", ""))
                if re.search(r"\b(cat|sed|head|tail|less|awk|nl)\b", command):
                    touched = PATH_RE.findall(command)
            for path in touched:
                rel = next((t for t in truth_set if path.endswith(t)), None)
                reads.append(path)
                if rel and first_hit is None:
                    first_hit = total
    answer = []
    for path in PATH_RE.findall(reply):
        if path not in answer:
            answer.append(path)
    answer = answer[:max_answer]
    hits = [p for p in answer if p in truth_set]
    recall = len(hits) / len(truth_set) if truth_set else 0.0
    precision = len(hits) / len(answer) if answer else 0.0
    unique_reads = sorted(set(reads))
    # Argon's list price on Google until 2026-11-30: $2 per million input
    # tokens, $0.10 cached, $10 output (crates/coder-new/src/provider.rs).
    cost = ((prompt_tokens - cached) * 2.0 + cached * 0.10 + completion * 10.0) / 1e6
    return {"model": model, "tokens": total, "cached_tokens": cached,
            "cost_usd": round(cost, 3), "tokens_to_first_fix_file": first_hit,
            "files_read": len(unique_reads), "reads": unique_reads, "tools": tools,
            "graph_calls": sum(v for k, v in tools.items() if k.startswith("graph_")),
            "answer": answer, "recall": round(recall, 3), "precision": round(precision, 3),
            "correct": bool(truth_set) and recall >= pass_recall, "steps": len(steps),
            "reply": reply[-1500:]}


def cmd_run(args):
    tasks = [load_task(int(i)) for i in args.issues.split(",") if i]
    if args.impact:
        tasks += load_impact(Path(args.impact))
    secrets = token_env()
    builds = []
    for task in tasks:
        if task["parent"] not in {b.get("parent") for b in builds}:
            builds.append({**prebuild(task), "parent": task["parent"]})
    out_dir = WORK / "results" / args.tag
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "builds.json").write_text(json.dumps(builds, indent=1))
    jobs = [(task, arm, rep) for rep in range(args.reps) for task in tasks for arm in ("A", "B")]
    with ThreadPoolExecutor(args.workers) as pool:
        futures = [pool.submit(trial, task, arm, rep, args.tag, secrets) for task, arm, rep in jobs]
        for future in futures:
            record = future.result()
            print(f"{record['issue']} {record['arm']}{record['rep']}: tokens {record['tokens']} "
                  f"recall {record['recall']} reads {record['files_read']} graph {record['graph_calls']} "
                  f"wall {record['wall_s']}s exit {record['exit']}", flush=True)


def cmd_rescore(args):
    """Score saved trials again from their output and trajectories."""
    out_dir = WORK / "results" / args.tag
    for path in sorted(out_dir.glob("*-*-*.json")):
        record = json.loads(path.read_text())
        name = path.stem
        stdout = (out_dir / f"{name}.out").read_text()
        home = WORK / "homes" / args.tag / name
        record.update(score(stdout, home, record["truth"], record.get("pass_recall", 0.5),
                            record.get("max_answer", 6)))
        record["stalled"] = "Nothing came back from the model" in stdout
        path.write_text(json.dumps(record, indent=1))


def median(values):
    values = [v for v in values if v is not None]
    return statistics.median(values) if values else None


def cmd_report(args):
    out_dir = WORK / "results" / args.tag
    records = [json.loads(p.read_text()) for p in sorted(out_dir.glob("*-*-*.json"))]
    by = {}
    for r in records:
        by.setdefault((r["issue"], r["arm"]), []).append(r)
    rows = []
    for issue in sorted({r["issue"] for r in records}, key=str):
        row = {"issue": issue}
        for arm in ("A", "B"):
            rs = by.get((issue, arm), [])
            row[arm] = {
                "n": len(rs),
                "tokens": median([r["tokens"] for r in rs]),
                "to_fix_file": median([r["tokens_to_first_fix_file"] for r in rs]),
                "reads": median([r["files_read"] for r in rs]),
                "recall": round(statistics.mean([r["recall"] for r in rs]), 2) if rs else None,
                "correct": sum(r["correct"] for r in rs),
                "graph_calls": median([r["graph_calls"] for r in rs]),
                "wall": median([r["wall_s"] for r in rs]),
                "cost": round(sum(r.get("cost_usd") or 0 for r in rs), 2),
            }
        rows.append(row)
    totals = {}
    for arm in ("A", "B"):
        rs = [r for r in records if r["arm"] == arm]
        totals[arm] = {
            "n": len(rs),
            "tokens_median": median([r["tokens"] for r in rs]),
            "tokens_sum": sum(r["tokens"] for r in rs),
            "to_fix_file_median": median([r["tokens_to_first_fix_file"] for r in rs]),
            "reads_median": median([r["files_read"] for r in rs]),
            "recall_mean": round(statistics.mean([r["recall"] for r in rs]), 3) if rs else None,
            "correct": sum(r["correct"] for r in rs),
            "wall_median": median([r["wall_s"] for r in rs]),
            "cost_usd": round(sum(r.get("cost_usd") or 0 for r in rs), 2),
            "graph_calls_total": sum(r["graph_calls"] for r in rs),
            "trials_using_graph": sum(1 for r in rs if r["graph_calls"] > 0),
            "models": sorted({r.get("model") or "?" for r in rs}),
        }
    print(json.dumps({"rows": rows, "totals": totals}, indent=1))


def main():
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="command", required=True)
    p = sub.add_parser("run")
    p.add_argument("--issues", default="")
    p.add_argument("--impact", help="a JSON file of impact tasks (impact.json)")
    p.add_argument("--reps", type=int, default=2)
    p.add_argument("--workers", type=int, default=4)
    p.add_argument("--tag", default="main")
    p.set_defaults(func=cmd_run)
    p = sub.add_parser("rescore")
    p.add_argument("--tag", default="main")
    p.set_defaults(func=cmd_rescore)
    p = sub.add_parser("report")
    p.add_argument("--tag", default="main")
    p.set_defaults(func=cmd_report)
    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
