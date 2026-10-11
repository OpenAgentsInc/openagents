#!/usr/bin/env python3
"""The Haiku briefer (docs/inference/haiku-briefer.md): the opening stage of
an issue run, as a constrained Claude Haiku agent on the Claude Agent SDK's
CLI, instead of the decision-model step of the file finder.

    haiku_briefer.py ISSUE [--solo] [--root DIR] [--out DIR]   # one bench task
    briefing.py ISSUE --finder haiku                           # inside a briefing
    work_issue.py ... --briefer haiku                          # inside an issue run

`filefind` (#11210) still does the deterministic retrieval: git-mined
indexes and rules rank the candidates, with its card decision (`--no-cards`)
left out. Haiku then reads the checkout at the issue's base commit with
read-only tools (Read, Grep, Glob and `git log`), for a bounded number of
turns and seconds, and answers one structured object: the files the fix
will change ranked with a reason each, the likely change, the checks to run
and the acceptance points the issue sets. `--solo` gives it no candidates
(an ablation: Haiku alone against the finder).

The answer only shapes the briefing; the engine that makes the change is
unchanged. Paths Haiku names that do not exist at the base are dropped. If
Haiku fails, runs past its time, or names no file, the caller falls back to
the finder's own list and records why.

Haiku runs on the Claude Code login in the environment (subscription or
`CLAUDE_CODE_OAUTH_TOKEN`); `ANTHROPIC_API_KEY` is never passed. Cost is
the list price the CLI reports.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import time
from pathlib import Path

MODEL = os.environ.get("OA_BRIEFER_MODEL", "claude-haiku-5-5")
CLAUDE = shutil.which("claude") or "claude"
DROP_ENV = ("ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN", "CLAUDECODE", "CLAUDE_CODE_ENTRYPOINT")

# Defaults for the briefer's levers (common.LEVERS carries them in the bench).
DEFAULTS = {"briefer_turns": 14, "briefer_secs": 120, "briefer_usd": 0.40, "briefer_candidates": 15}

SYSTEM = """You prepare the briefing for an engineer who will fix one GitHub issue in this repository (a Rust monorepo). Your working directory is the repository at the issue's base commit. You cannot change anything; you only read.

Your job: find the files the fix will change, and say what the change is.
- Start from the candidate files (ranked by a retrieval index; often right, sometimes not) and the names the issue mentions. Confirm by reading: Grep for the identifiers, messages and paths the issue names; Read the matching lines; `git log --oneline -n 5 -- PATH` shows recent changes to a file.
- Be fast: a few searches and reads, then answer. Do not read whole large files; read around the matches.
- Rank the files the fix will most likely edit first (the code that has the behavior), then the file where its test belongs (often the same file's `#[cfg(test)] mod tests`, or the crate's tests/ directory). List each path once, at most {k} files, repository-relative, and only files that exist. Give the line numbers that matter.
- `checks`: the exact commands that prove the change, normally `cargo test -p PACKAGE` (with a test-name filter when you know it) and `cargo fmt -p PACKAGE` for each package you would edit. The package name is the `name` in the crate's Cargo.toml, not always the directory name.
- `acceptance`: the points from the issue that must hold when it is done, each as one short testable sentence. Use the issue's own bullet points when it has them.
- `likely_change`: two or three sentences on what to change and where.
- `confidence`: your probability, 0 to 1, that the first file is one the fix edits.
Answer only through the structured output."""

SCHEMA = {
    "type": "object",
    "properties": {
        "files": {
            "type": "array",
            "items": {
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "role": {"type": "string", "enum": ["change", "test", "context"]},
                    "reason": {"type": "string"},
                    "lines": {"type": "array", "items": {"type": "integer"}},
                },
                "required": ["path", "role", "reason"],
            },
        },
        "likely_change": {"type": "string"},
        "checks": {"type": "array", "items": {"type": "string"}},
        "acceptance": {"type": "array", "items": {"type": "string"}},
        "confidence": {"type": "number"},
    },
    "required": ["files", "likely_change", "checks", "acceptance", "confidence"],
}

TOOLS = "Read,Grep,Glob,Bash"
ALLOWED = ["Read", "Grep", "Glob", "Bash(git log:*)"]


def prompt_for(task: dict, candidates: list[dict] | None) -> str:
    out = [f"# Issue #{task['issue']}: {task['title']}", "", (task.get("body") or "").strip(), ""]
    if candidates:
        out += ["## Candidate files (retrieval index, best first)", ""]
        for c in candidates:
            why = "; ".join(str(r) for r in (c.get("why") or [])[:3])
            out.append(f"- `{c['path']}`" + (f" ({why})" if why else ""))
        out.append("")
    out.append("Find the files this fix changes and answer with the briefing.")
    return "\n".join(out)


def exists_at(root: Path, path: str) -> bool:
    p = (root / path).resolve()
    try:
        p.relative_to(root.resolve())
    except ValueError:
        return False
    return p.is_file()


def ask(task: dict, root: Path, levers: dict, candidates: list[dict] | None, log_dir: Path | None = None) -> dict:
    """Run the constrained Haiku agent once. Returns
    {"ok", "answer", "secs", "cost_usd", "turns", "usage", "error"}."""
    lv = {k: levers.get(k, v) for k, v in DEFAULTS.items()}
    k = int(levers.get("briefing_files", 8))
    env = {key: v for key, v in os.environ.items() if key not in DROP_ENV}
    env["DISABLE_AUTOUPDATER"] = "1"
    argv = [
        CLAUDE, "-p", prompt_for(task, candidates),
        "--model", MODEL,
        "--system-prompt", SYSTEM.replace("{k}", str(k)),
        "--tools", TOOLS, "--allowedTools", *ALLOWED,
        "--permission-mode", "default",
        "--setting-sources", "", "--strict-mcp-config", "--disable-slash-commands",
        "--no-session-persistence",
        "--max-turns", str(int(lv["briefer_turns"])),
        "--max-budget-usd", str(float(lv["briefer_usd"])),
        "--json-schema", json.dumps(SCHEMA),
        "--output-format", "json",
    ]
    t0 = time.time()
    out = {"ok": False, "answer": None, "secs": None, "cost_usd": None, "turns": None, "usage": None,
           "model": MODEL, "error": None, "denied": []}
    try:
        proc = subprocess.run(argv, cwd=str(root), env=env, capture_output=True, text=True,
                              timeout=float(lv["briefer_secs"]), stdin=subprocess.DEVNULL)
    except subprocess.TimeoutExpired:
        out.update(secs=round(time.time() - t0, 2), error=f"ran past {lv['briefer_secs']}s")
        return out
    out["secs"] = round(time.time() - t0, 2)
    try:
        res = json.loads(proc.stdout)
    except json.JSONDecodeError:
        out["error"] = f"no result (exit {proc.returncode}): {proc.stderr[-300:]}"
        return out
    if log_dir:
        log_dir.mkdir(parents=True, exist_ok=True)
        (log_dir / "haiku-result.json").write_text(json.dumps(res, indent=2))
    out.update(cost_usd=res.get("total_cost_usd"), turns=res.get("num_turns"), usage=res.get("usage"),
               denied=[d.get("tool_name") for d in res.get("permission_denials") or []])
    text = json.dumps(res)[:4000]
    if res.get("api_error_status") == 429 or "usage limit" in text or "hit your" in text:
        out["error"] = "usage limit"
        out["limited"] = True
        return out
    answer = res.get("structured_output")
    if not isinstance(answer, dict) or not answer.get("files"):
        out["error"] = f"no structured answer ({res.get('subtype')})"
        return out
    out.update(ok=True, answer=answer)
    return out


def to_files(answer: dict, root: Path, k: int) -> list[dict]:
    """Haiku's ranked files as briefing rows; paths missing at the base are dropped."""
    rows, seen = [], set()
    conf = answer.get("confidence")
    for rank, f in enumerate(answer.get("files") or []):
        path = str(f.get("path") or "").strip().lstrip("./")
        if not path or path in seen or not exists_at(root, path):
            continue
        seen.add(path)
        lines = sorted({int(x) for x in (f.get("lines") or []) if isinstance(x, int) and x > 0})[:12]
        score = round(float(conf) * (0.85 ** rank), 3) if isinstance(conf, (int, float)) else None
        rows.append({"path": path, "score": score, "lines": lines,
                     "why": [f"Haiku briefer ({f.get('role')}): {str(f.get('reason') or '')[:300]}"]})
        if len(rows) >= k:
            break
    return rows


def plan_lines(answer: dict) -> list[str]:
    out = []
    if answer.get("likely_change"):
        out.append("Likely change (Haiku briefer): " + str(answer["likely_change"])[:600])
    for a in (answer.get("acceptance") or [])[:8]:
        out.append("Acceptance: " + str(a)[:240])
    for c in (answer.get("checks") or [])[:6]:
        out.append("Check: `" + str(c)[:160] + "`")
    return out


def main() -> None:
    import argparse
    import sys

    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import ab  # noqa: E402 - the bench's worktrees (no fix visible)
    import briefing as briefing_mod  # noqa: E402
    from common import LEVERS, WORK, dump_json, load_json  # noqa: E402

    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("issues", type=int, nargs="+")
    ap.add_argument("--solo", action="store_true", help="no finder candidates")
    ap.add_argument("--out", default=str(WORK / "haiku-briefer"))
    a = ap.parse_args()
    levers = dict(LEVERS)
    for issue in a.issues:
        task = load_json(briefing_mod.TASKS / str(issue) / "task.json")
        extra = ab.interface_text(task) if levers.get("interface") else ""
        if extra:
            task = dict(task, body=(task["body"] or "") + extra)
        out = Path(a.out) / f"{issue}{'-solo' if a.solo else ''}"
        root = ab.make_worktree(task, WORK / "wt" / f"brief-{issue}")
        try:
            t0 = time.time()
            ff, used = ([], "none") if a.solo else briefing_mod.finder_files(
                task, dict(levers, briefing_files=DEFAULTS["briefer_candidates"]), no_cards=True)
            ff_secs = time.time() - t0
            r = ask(task, root, levers, ff or None, out)
            files = to_files(r["answer"], root, int(levers["briefing_files"])) if r["ok"] else []
        finally:
            ab.drop_worktree(task, root)
        fix = set(task["source_files"] + task["test_files"])
        src = set(p for p in task["source_files"] if not p.endswith(".md"))
        got = [f["path"] for f in files]
        row = {"issue": issue, "solo": a.solo, "finder_secs": round(ff_secs, 2), **{k: r[k] for k in r if k != "answer"},
               "files": got, "recall": round(len(fix & set(got)) / max(1, len(fix)), 2),
               "src_recall": round(len(src & set(got)) / max(1, len(src)), 2),
               "candidates": [f["path"] for f in ff], "answer": r["answer"]}
        dump_json(out / "row.json", row)
        print(json.dumps({k: row[k] for k in ("issue", "ok", "secs", "cost_usd", "turns", "recall", "src_recall", "error")}),
              flush=True)


if __name__ == "__main__":
    main()
