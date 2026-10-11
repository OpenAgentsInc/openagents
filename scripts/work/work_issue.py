#!/usr/bin/env python3
"""Work one GitHub issue with the briefed agent (#11211, #11258).

    work_issue.py --repo OWNER/NAME --issue N [--land queue|pr|none]
                  [--engine briefed|bare] [--out DIR] [--checkout PATH]

The one engine entry point behind `POST /v1/work`, the web's "Work on this
issue", agent fleet runs, and `openagents chat work` (the default engine):

1. **Briefing**: the issue (from `gh`) at `origin/main`, the context finder
   (`scripts/filefind`, #11210) and the brief generator
   (`scripts/bench/briefed-ab/briefing.py`).
2. **Briefed agent**: `crates/briefed-agent` with `verify` (#11229): the five
   file tools held to a fresh worktree, a briefing-built system prompt, and
   `verify`, which compiles, runs the change's tests and formats.
3. **Judged by the diff**: the driver replays the checks itself on what the
   agent actually changed: `cargo fmt` and `cargo test` for each package the
   diff touches. A run passes only when there is a diff and every replayed
   check passes.
4. **Escalation**: low briefing confidence, repeated `verify` failures, a
   failed replay, or a missing capability (no Rust package to check) falls
   back to bare Claude Code (`claude -p`, "Complete this issue.") in a fresh
   worktree, judged the same way. The result records why.
5. **Landing**: on a pass, commit; `queue` hands the branch to the landing
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

SYSTEM = """You are a senior Rust engineer making one change in this repository. Your working directory is the repository checkout at the issue's base commit.

Below is a briefing for this issue: a change plan, the files to change with excerpts (line numbers are the file's own), similar past changes, the checks, and the repo rules.

How to work:
- Trust the briefing. Start from the listed files and excerpts. Open a file the briefing does not list only when the change, the compiler, or a test demands it.
- Make the smallest complete change that resolves the issue, in the surrounding style, and add or update a test that pins the new behavior.
- Check your work with `verify`, passing `tests` with your new tests' names. It compiles, runs those tests and formats, and returns only what is wrong; fix that and call it again. `fast: true` only compiles. Call it once when you think you are done, not after every edit.
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


def changed_files(root: Path) -> list[str]:
    rows = sh(["git", "-C", str(root), "status", "--porcelain", "--untracked-files=all"]).stdout.splitlines()
    return sorted({row[3:].split(" -> ")[-1].strip('"') for row in rows if len(row) > 3})


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


def replay_checks(run: Run, root: Path) -> list[dict]:
    """fmt, then the tests of every package the diff touches."""
    packages = []
    for path in changed_files(root):
        if path.endswith(".rs") or path.endswith("Cargo.toml"):
            pkg = package_of(root, path)
            if pkg and pkg not in packages:
                packages.append(pkg)
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
    return checks


# ------------------------------------------------------------------ engines

def briefing_for(issue: dict, base: str, checkout: Path) -> dict:
    """The #11211 briefing, built at `base` in `checkout`."""
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
    b = briefing_mod.build(task, levers)
    b["markdown"] = briefing_mod.render(b)
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
            elif status not in (None, "compiles"):
                fails += 1
    return fails, passed


def usage_of(result: dict | None) -> dict:
    result = result or {}
    u = result.get("usage") or {}
    return {"cost_usd": result.get("total_cost_usd"), "turns": result.get("num_turns"),
            "input_tokens": u.get("input_tokens"), "output_tokens": u.get("output_tokens"),
            "cache_read": u.get("cache_read_input_tokens"), "is_error": result.get("is_error")}


def run_briefed(run: Run, b: dict, root: Path, out: Path, model: str, timeout: int) -> dict:
    out.mkdir(parents=True, exist_ok=True)
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
        "custom": ["verify"], "inherit_api_key": keeps_api_key(),
        "verify": {
            "exec": str(LOCAL_EXEC), "crates": packages, "baseline_errors": [],
            "done_when": [p.removeprefix("Required: ") for p in b["plan"] if p.startswith("Required: ")]
            or [b["title"]],
            "cochange": {}, "log": str(out / "verify.jsonl"),
        },
    }
    (out / "agent.json").write_text(json.dumps(config, indent=2))
    agent = os.environ.get("OA_BRIEFED_AGENT") or shutil.which("briefed-agent") or "briefed-agent"
    run.say("agent", "the briefed agent is working", engine="briefed")
    t0 = time.time()
    proc = subprocess.Popen([agent, str(out / "agent.json")], cwd=root, env=clean_env(),
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
    prompt = f"Complete this issue.\n\n#{issue['number']} {issue['title']}\n\n{issue.get('body') or ''}"
    argv = [shutil.which("claude") or "claude", "-p", prompt, "--output-format", "stream-json", "--verbose",
            "--model", model, "--dangerously-skip-permissions", "--no-session-persistence",
            "--strict-mcp-config"]
    run.say("agent", "Claude Code is working", engine="bare")
    t0 = time.time()
    result, timed_out = None, False
    with open(out / "agent-events.jsonl", "w") as sink:
        proc = subprocess.Popen(argv, cwd=root, env=clean_env(), stdout=subprocess.PIPE,
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

def commit(root: Path, issue: dict, engine: str) -> str:
    sh(["git", "-C", str(root), "add", "-A"])
    message = (f"{issue['title']} (#{issue['number']})\n\n"
               f"Worked by the {'briefed agent' if engine == 'briefed' else 'bare Claude Code'} "
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
            entry = json.loads(proc.stdout)
        except json.JSONDecodeError:
            entry = None
        if proc.returncode != 0:
            raise RuntimeError(f"the landing queue refused it: {(proc.stderr or proc.stdout)[-400:]}")
        run.say("land", "handed to the landing queue", entry=(entry or {}).get("id"))
        return {"how": "queue", "entry": entry}
    branch = f"work/issue-{issue['number']}-{run_id[:8]}"
    sh(["git", "-C", str(root), "push", "-q", "origin", f"HEAD:refs/heads/{branch}"], timeout=600)
    lines = [f"Fixes #{issue['number']}.", "",
             f"Worked by the {'briefed agent' if engine == 'briefed' else 'bare Claude Code'} "
             f"(`openagents work`, run `{run_id}`).", "", "Checks replayed on this diff:", ""]
    lines += [f"- {'pass' if c['ok'] else 'FAIL'}: `{c['name']}`" for c in checks] or ["- none (no Rust package touched)"]
    lines += ["", "🤖 Generated with [Claude Code](https://claude.com/claude-code)"]
    proc = sh(["gh", "pr", "create", "--repo", repo, "--head", branch, "--base", "main",
               "--title", f"{issue['title']} (#{issue['number']})", "--body", "\n".join(lines)],
              cwd=root, timeout=300)
    url = proc.stdout.strip().splitlines()[-1]
    run.say("land", "opened a pull request", url=url)
    return {"how": "pr", "branch": branch, "url": url}


# --------------------------------------------------------------------- main

def attempt(run: Run, engine: str, issue: dict, checkout: Path, base: str, out: Path, args, b: dict | None) -> dict:
    wt = add_worktree(checkout, out / "worktree", base)
    if engine == "briefed":
        agent = run_briefed(run, b, wt, out, args.model, args.timeout)
    else:
        agent = run_bare(run, issue, wt, out, args.model, args.timeout)
    files = changed_files(wt)
    checks = replay_checks(run, wt) if files else []
    stat = diff_stat(wt, base) if files else {"files": [], "added": 0, "removed": 0}
    ok = bool(files) and all(c["ok"] for c in checks)
    return {"engine": engine, "worktree": str(wt), "agent": agent, "checks": checks, "diff": stat, "ok": ok}


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--repo", required=True)
    ap.add_argument("--issue", type=int, required=True)
    ap.add_argument("--land", choices=["queue", "pr", "none"], default="pr")
    ap.add_argument("--engine", choices=["briefed", "bare"], default="briefed")
    ap.add_argument("--checkout")
    ap.add_argument("--out")
    ap.add_argument("--run-id")
    ap.add_argument("--model", default=os.environ.get("OA_WORK_MODEL", "claude-opus-5-5"))
    ap.add_argument("--timeout", type=int, default=1500)
    ap.add_argument("--keep", action="store_true", help="keep the worktrees")
    args = ap.parse_args()

    run_id = args.run_id or uuid.uuid4().hex[:12]
    out = Path(args.out or Path.home() / ".openagents" / "work" / "runs" / run_id)
    run = Run(out)
    result = {"type": "result", "run_id": run_id, "repo": args.repo, "issue": args.issue,
              "requested_engine": args.engine, "engine": None, "escalated": None, "ok": False,
              "land": args.land, "landed": None, "checks": [], "diff": None, "cost_usd": None,
              "secs": None, "attempts": [], "error": None}
    try:
        if not claude_signed_in():
            raise RuntimeError("no Claude sign-in in this environment (CLAUDE_CODE_OAUTH_TOKEN or ~/.claude)")
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
        if engine == "briefed":
            run.say("briefing", "finding the files this issue needs")
            t0 = time.time()
            b = briefing_for(issue, base, checkout)
            conf = confidence_of(b)
            (out / "briefing.md").write_text(b["markdown"])
            result["briefing"] = {"secs": round(time.time() - t0, 1), "finder": b["finder"],
                                  "files": [f["path"] for f in b["files"]], "confidence": conf,
                                  "checks": [c["id"] for c in b["checks"]]}
            run.say("briefing", f"briefing ready: {len(b['files'])} files, finder {b['finder']}",
                    files=[f["path"] for f in b["files"]][:8], confidence=conf)
            agent = os.environ.get("OA_BRIEFED_AGENT") or shutil.which("briefed-agent")
            if not agent or not Path(agent).exists():
                engine, why = "bare", "missing capability: the briefed agent isn't installed on this computer"
            elif not b["files"]:
                engine, why = "bare", "low briefing confidence: the finder found no files"
            elif not b["checks"]:
                engine, why = "bare", "missing capability: no Rust package to check with verify"
            elif b["finder"].startswith("filefind") and conf is not None and conf < MIN_CONFIDENCE:
                engine, why = "bare", f"low briefing confidence ({conf:.3f} < {MIN_CONFIDENCE})"
        if why:
            run.say("escalate", why)
            result["escalated"] = why
        a = attempt(run, engine, issue, checkout, base, out / engine, args, b)
        result["attempts"].append(a)
        if engine == "briefed" and not a["ok"]:
            ag = a["agent"]
            if not a["diff"]["files"]:
                why = "the briefed agent made no change"
            elif ag["verify_fails"] >= MAX_VERIFY_FAILS and not ag["verify_passed"]:
                why = f"repeated verify failures ({ag['verify_fails']})"
            else:
                why = "the replayed checks failed on the briefed agent's change"
            run.say("escalate", why + "; falling back to bare Claude Code")
            result["escalated"] = why
            engine = "bare"
            a = attempt(run, engine, issue, checkout, base, out / engine, args, None)
            result["attempts"].append(a)
        result["engine"], result["ok"] = engine, a["ok"]
        result["checks"], result["diff"] = a["checks"], a["diff"]
        costs = [x["agent"]["usage"]["cost_usd"] for x in result["attempts"]]
        result["cost_usd"] = None if any(c is None for c in costs) else round(sum(costs), 4)
        if a["ok"]:
            wt = Path(a["worktree"])
            result["commit"] = commit(wt, issue, engine)
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
        (out / "result.json").write_text(json.dumps(result, indent=2))
        run.say("done", "done" if result["ok"] else f"stopped: {result['error']}", ok=result["ok"])
        print(json.dumps(result), flush=True)
    return 0 if result["ok"] else 1


if __name__ == "__main__":
    sys.exit(main())
