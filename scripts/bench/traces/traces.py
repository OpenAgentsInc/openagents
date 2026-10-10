#!/usr/bin/env python3
"""Verify-replayed agent traces (#11218, roadmap X11).

Every `coder issue-run` and every briefed-agent A/B trial (#11211) leaves a
diff and the result of its checks. This tool turns those into labelled
training traces, and admits a trace only after an independent replay:

    capture  read run folders, store each diff and the check spec by
             digest, and write one trace record per run. Labels come from
             the diff alone (files changed, its digest, the tree it makes at
             the base commit) and from the checks the harness ran, never
             from the agent's own summary or reply.
             `--landed COMMIT --issue N` captures a commit the issue flow
             or the landing queue landed (#11243): base COMMIT~1, its diff,
             and the gate's checks as the landing reported them (pass).
    replay   for each trace: re-read the stored diff and compare its digest,
             apply it in a clean index at the base commit and compare the
             tree, re-derive the files it changes, then run the same verify
             checks again in a clean checkout at the base commit and compare
             their results. The receipt says verified or rejected, and names
             the first field that diverged.
    admit    write the verified traces, with corpus items in the
             `tenancy::training` CorpusItem shape, to admitted.jsonl.
             Rejected and unverifiable traces are never admitted. Each
             item's partition is its issue group's partition in the
             file-relevance-v1 corpus map (#11215, `--corpus-map`); an
             issue the map does not hold gets no items at all.
    manifest digests only (no diffs, no content), for committing.

The store defaults to ~/.openagents/traces (override with --store). Nothing
in it goes into git; `manifest` is what a commit carries.

Where the checks run (`--on`):
    local    this computer (`mac` is the older name; it works on Linux
             too): a fresh git worktree at the base commit and a
             dedicated CARGO_TARGET_DIR, both deleted after. A cargo check
             is refused (unverifiable) when the disk has less than
             TRACES_MIN_FREE_GB (default 30) free.
    HOST     a build host over ssh, through the A/B bench's own grader
             (`~/ab/bin/eval.sh`) in its second build checkout
             (AB_BUILD=v, its own target dir), so the bench's trial builds
             are untouched.
"""

from __future__ import annotations

import argparse
import datetime as dt
import glob
import hashlib
import io
import json
import os
import re
import shlex
import shutil
import subprocess
import sys
import tarfile
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
TRACE_SCHEMA = "openagents.coder-trace.v1"
RECEIPT_SCHEMA = "openagents.coder-trace-replay.v1"
EXACT_REPLAY = "exact_replay"
STORE = Path(os.environ.get("TRACES_STORE", os.path.expanduser("~/.openagents/traces")))
# The owner's Mac checkout when it exists, else the checkout holding this script
# (a Linux cloud environment).
_MAC_REPO = Path("/Users/christopherdavid/work/openagents")
REPO = Path(os.environ.get("TRACES_REPO") or (_MAC_REPO if _MAC_REPO.exists() else HERE.parents[2]))
# `--on` values that mean this computer.
LOCAL = ("local", "mac")
MIN_FREE_GB = float(os.environ.get("TRACES_MIN_FREE_GB", "30"))
OVERLAY = HERE.parent / "briefed-ab" / "remote" / "overlay.py"
# The #11215 corpus's issue-group map: one partition per issue, for every feed.
CORPUS_MAP = HERE.parents[2] / "crates" / "gym" / "suites" / "file-relevance-v1" / "issues.tsv"
ROLES = ("training", "calibration", "development", "locked")
# The fields of the A/B grade (remote/eval.sh) a replay must reproduce.
AB_FIELDS = ["applied", "compiles", "tests_applied", "tests_compiled", "tests_pass", "passed", "failed"]


# ---------------------------------------------------------------- digests


def sha256(data: bytes | str) -> str:
    if isinstance(data, str):
        data = data.encode()
    return "sha256:" + hashlib.sha256(data).hexdigest()


def canonical(value) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def put_blob(store: Path, data: bytes | str) -> str:
    """Store `data` under its digest; return the digest."""
    if isinstance(data, str):
        data = data.encode()
    digest = sha256(data)
    path = store / "blobs" / digest.split(":", 1)[1]
    if not path.exists():
        path.parent.mkdir(parents=True, exist_ok=True)
        tmp = path.with_suffix(f".{os.getpid()}.tmp")
        tmp.write_bytes(data)
        os.replace(tmp, path)
    return digest


def get_blob(store: Path, digest: str) -> bytes | None:
    path = store / "blobs" / digest.split(":", 1)[-1]
    return path.read_bytes() if path.exists() else None


# ---------------------------------------------------------------- git


def git(repo: Path, *args, env=None, input=None) -> subprocess.CompletedProcess:
    return subprocess.run(["git", "-C", str(repo), *args], capture_output=True, env=env, input=input)


def diff_files(patch: str) -> list[str]:
    """The paths a unified git diff touches, read from its headers."""
    files = []
    for line in patch.splitlines():
        m = re.match(r"^diff --git a/(.+?) b/(.+)$", line)
        if m:
            path = m.group(2)
            if path not in files:
                files.append(path)
    return sorted(files)


def tree_after(repo: Path, base: str, patch: bytes) -> tuple[str | None, str]:
    """Apply `patch` to `base` in a clean, throwaway index (no working
    files) and return the resulting tree id, or None and why."""
    with tempfile.TemporaryDirectory(prefix="traces-idx-") as tmp:
        env = dict(os.environ, GIT_INDEX_FILE=os.path.join(tmp, "index"))
        r = git(repo, "read-tree", base, env=env)
        if r.returncode:
            return None, "base commit unknown: " + r.stderr.decode(errors="replace").strip()[:200]
        if patch.strip():
            r = git(repo, "apply", "--cached", "--binary", "--whitespace=nowarn", "-", env=env, input=patch)
            if r.returncode:
                return None, "diff does not apply at base: " + r.stderr.decode(errors="replace").strip()[:300]
        r = git(repo, "write-tree", env=env)
        if r.returncode:
            return None, "write-tree failed"
        return r.stdout.decode().strip(), ""


# ---------------------------------------------------------------- capture


def _now() -> str:
    return dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def _load(path: Path):
    try:
        return json.loads(path.read_text())
    except (OSError, ValueError):
        return None


def base_record(store: Path, repo: Path, *, tid: str, kind: str, path: Path, issue: int, base: str | None,
                patch: bytes | None) -> dict:
    """The diff-derived half of a trace: digest, tree at base, files."""
    t = {
        "v": TRACE_SCHEMA,
        "id": tid,
        "source": {"kind": kind, "path": str(path)},
        "issue": int(issue),
        "base": base,
        "captured_at": _now(),
        "evidence_class": "recorded",
        "diff_digest": None,
        "result_tree": None,
        "files_changed": [],
        "capture_error": None,
    }
    if patch is None:
        t["capture_error"] = "no diff was recorded for this run"
        return t
    if not base:
        t["capture_error"] = "no base commit was recorded for this run"
        return t
    t["diff_digest"] = put_blob(store, patch)
    t["files_changed"] = diff_files(patch.decode(errors="replace"))
    tree, why = tree_after(repo, base, patch)
    t["result_tree"] = tree
    if tree is None:
        t["capture_error"] = why
    return t


def capture_ab(store: Path, repo: Path, trial: Path, tasks: Path, root: Path) -> dict | None:
    """One A/B trial folder (result.json, change.patch) as a trace."""
    r = _load(trial / "result.json")
    if not r or "grade" not in r:
        return None
    task = _load(tasks / str(r["issue"]) / "task.json")
    tid = "ab:" + os.path.relpath(trial, root)
    patch = (trial / "change.patch").read_bytes() if (trial / "change.patch").exists() else None
    base = task.get("parent") if task else None
    t = base_record(store, repo, tid=tid, kind="ab", path=trial, issue=r["issue"], base=base, patch=patch)
    t["arm"] = r.get("arm")
    t["rep"] = r.get("rep")
    briefing = trial / "briefing.md"
    t["briefing_digest"] = put_blob(store, briefing.read_bytes()) if briefing.exists() else None
    b = _load(trial / "briefing.json") or {}
    t["briefed"] = sorted({f["path"] if isinstance(f, dict) else f for f in b.get("files") or []})
    # Files the agent reached outside its briefing: the harness's own
    # tool-call accounting, not the agent's words. Arm A has no briefing.
    misses = r.get("misses")
    t["opened_outside_briefing"] = None if misses is None else sorted(
        {m.get("file") if isinstance(m, dict) else m for m in misses if m})
    g = r["grade"]
    t["checks"] = {k: g.get(k) for k in AB_FIELDS}
    if g.get("empty"):
        # No change: the bench grades it as not passing without a build.
        t["checks"] = {"tests_pass": False}
        t["check_spec"] = {"kind": "empty"}
    elif task is None:
        t["capture_error"] = t["capture_error"] or f"task {r['issue']} not found under {tasks}"
        t["check_spec"] = None
    else:
        tests = (tasks / str(r["issue"]) / "tests.patch").read_bytes()
        t["check_spec"] = {
            "kind": "ab-grade",
            "package": task["package"],
            "test_names": task.get("test_names") or [],
            "test_targets": task.get("test_targets") or [],
            "tests_patch_digest": put_blob(store, tests),
        }
    t["checks_digest"] = sha256(canonical(t["checks"]))
    # The harness's own reading of the diff, kept to catch disagreement.
    t["claimed"] = {"files": sorted(r.get("files_changed") or [])}
    t["teacher"] = {"judge": (_load(trial / "judge.json") or {}).get("score")}
    t["cost_usd"] = r.get("cost_usd")
    if t["cost_usd"] is None:
        t["cost_unknown_reason"] = "the trial recorded no cost"
    t["wall_secs"] = r.get("wall_secs")
    return t


def unknown_cost(component: str, reason: str) -> dict:
    return {"usd": None, "unknown_reason": f"{component}: {reason}"}


def issue_run_cost(s: dict) -> dict:
    """The run's cost with every component kept: an amount, or None and why
    (#11230, PRODUCT-04). A total exists only when every component is
    known; a missing amount is never read as 0."""
    if isinstance(s.get("cost"), dict) and isinstance(s["cost"].get("components"), dict):
        parts = {k: dict(v) for k, v in s["cost"]["components"].items()}
    else:
        # Summaries before #11230 carry only the two amounts.
        parts = {}
        for key, name in (("agent_usd", "agent"), ("decision_usd", "decisions")):
            v = s.get(key)
            parts[name] = {"usd": v, "unknown_reason": None} if isinstance(v, (int, float)) else \
                {"usd": None, "unknown_reason": f"the summary recorded no {name} cost"}
    known = [p["usd"] for p in parts.values() if isinstance(p.get("usd"), (int, float))]
    complete = len(known) == len(parts) and bool(parts)
    return {
        "denomination": "USD",
        "basis": "reported",
        "components": parts,
        "known_subtotal_usd": sum(known),
        "total_usd": sum(known) if complete else None,
        "complete": complete,
    }


def _pid_alive(pid) -> bool:
    try:
        os.kill(int(pid), 0)
    except (OSError, TypeError, ValueError):
        return False
    return True


def capture_issue_run(store: Path, repo: Path, folder: Path) -> dict | None:
    """One `coder issue-run` folder (summary.json and, from #11218 on,
    change.patch) as a trace. Every attempt is kept (#11230): a run that
    stopped in setup, in the decision steps, was cancelled, or was killed
    before its summary (run.json but no summary.json, and its process gone)
    is still a trace, unverifiable, with its outcome."""
    s = _load(folder / "summary.json")
    started = _load(folder / "run.json")
    if not s:
        if not started or _pid_alive(started.get("pid")):
            return None  # not a run folder, or a run still working
        s = {"issue": started.get("issue"), "run_id": started.get("run_id"), "attempt": started.get("attempt"),
             "outcome": {"status": "incomplete", "delivers": False,
                         "reason": "the run ended without a summary (killed or crashed)"}}
    if s.get("issue") is None:
        return None
    patch_path = folder / "change.patch"
    patch = patch_path.read_bytes() if patch_path.exists() else None
    t = base_record(store, repo, tid="issue-run:" + folder.name, kind="issue-run", path=folder,
                    issue=s["issue"], base=s.get("base"), patch=patch)
    t["run_id"] = s.get("run_id") or folder.name
    t["attempt"] = s.get("attempt")
    # Summaries before #11230 have no outcome: unknown, never "passed".
    t["outcome"] = s.get("outcome") or {"status": "unknown", "delivers": None,
                                        "reason": "the summary predates recorded outcomes"}
    briefing = folder / "briefing.md"
    t["briefing_digest"] = put_blob(store, briefing.read_bytes()) if briefing.exists() else None
    t["briefed"] = sorted(s.get("briefed") or [])
    t["opened_outside_briefing"] = sorted(s.get("opened_outside_briefing") or [])
    t["checks"] = {c["id"]: bool(c.get("ok")) for c in s.get("checks") or []}
    t["required_checks"] = s.get("required_checks")
    t["optional_checks"] = s.get("optional_checks")
    t["check_spec"] = {"kind": "commands", "checks": s.get("check_commands") or []}
    if t["checks"] and not t["check_spec"]["checks"]:
        t["capture_error"] = t["capture_error"] or "the run did not record its check commands"
    t["checks_digest"] = sha256(canonical(t["checks"]))
    # What the summary card said changed. Kept to show disagreement; a
    # label never comes from here.
    t["claimed"] = {"files": sorted(s.get("changed") or [])}
    if s.get("diff_sha256") and t["diff_digest"] and s["diff_sha256"] != t["diff_digest"]:
        t["capture_error"] = "change.patch is not the diff the summary was computed from"
    t["teacher"] = None
    t["cost"] = issue_run_cost(s)
    # None when any component is unknown: never a partial sum shown as total.
    t["cost_usd"] = t["cost"]["total_usd"]
    t["wall_secs"] = s["wall_ms"] / 1000 if isinstance(s.get("wall_ms"), (int, float)) else None
    return t


def _show(repo: Path, rev: str, path: str) -> str | None:
    r = git(repo, "show", f"{rev}:{path}")
    return r.stdout.decode(errors="replace") if r.returncode == 0 else None


def gate_packages(repo: Path, commit: str, patch: str) -> list[tuple[str, None]]:
    """The Cargo packages whose directories the diff touches, read from the
    commit's tree: the issue flow's gate (`changed_packages` in
    crates/coder-delegate/src/issue.rs). A package outside the root
    workspace can't be tested by name from the root, so it is skipped."""
    root = _show(repo, commit, "Cargo.toml") or ""
    m = re.search(r"\nexclude\s*=\s*\[(.*?)\]", root, re.S)
    excluded = m.group(1) if m else ""
    packages = []
    for line in patch.splitlines():
        if not line.startswith("+++ b/"):
            continue
        at = os.path.dirname(line[len("+++ b/"):])
        while True:
            text = _show(repo, commit, f"{at}/Cargo.toml" if at else "Cargo.toml")
            if text is not None and "[package]" in text:
                if any(l.strip() == "[workspace]" for l in text.splitlines()) or f'"{at}"' in excluded:
                    break
                body = text.split("[package]", 1)[1]
                name = re.search(r'^\s*name\s*=\s*"([^"]+)"', body, re.M)
                if name and name.group(1) not in packages:
                    packages.append(name.group(1))
                break
            if not at:
                break
            at = os.path.dirname(at)
    return packages


def capture_landed(store: Path, repo: Path, commit: str, issue: int, checks: list[str] | None = None) -> dict:
    """A commit the issue flow or the landing queue (#11227) landed, as a
    trace (#11243). Its base is COMMIT~1 and its diff `git diff COMMIT~1
    COMMIT`. The checks are the issue flow's gate (`cargo test -p PKG` per
    touched package; none for a docs-only diff) or the given commands, and
    their recorded results are the ones the landing reported: pass."""
    r = git(repo, "rev-parse", "--verify", f"{commit}^{{commit}}")
    full = r.stdout.decode().strip() if r.returncode == 0 else commit
    short = full[:12]
    parent = git(repo, "rev-parse", "--verify", f"{full}~1")
    base = parent.stdout.decode().strip() if parent.returncode == 0 else None
    patch = None
    if r.returncode == 0 and base:
        d = git(repo, "diff", "--binary", base, full)
        patch = d.stdout if d.returncode == 0 else None
    t = base_record(store, repo, tid=f"landed:{short}-issue-{int(issue)}", kind="landed",
                    path=Path(f"landed:{full}"), issue=issue, base=base, patch=patch)
    t["source"]["path"] = f"landed:{full}"
    t["commit"] = full
    if r.returncode:
        t["capture_error"] = f"commit {commit} is not in {repo}"
    elif not base:
        t["capture_error"] = f"commit {short} has no parent to diff against"
    t["outcome"] = {"status": "landed", "delivers": True, "reason": "the commit is on the landed branch"}
    t["attempt"] = None
    t["run_id"] = None
    t["briefing_digest"] = None
    t["briefed"] = []
    t["opened_outside_briefing"] = None
    if checks:
        argvs = [shlex.split(c) for c in checks]
    else:
        text = patch.decode(errors="replace") if patch else ""
        argvs = [["cargo", "test", "-p", p] for p in gate_packages(repo, full, text)] if base else []
    specs = [{"id": "check:" + shlex.join(a), "argv": a} for a in argvs]
    t["check_spec"] = {"kind": "commands", "checks": specs} if specs else {"kind": "none"}
    # The landing ran these and landed only because they passed.
    t["checks"] = {c["id"]: True for c in specs}
    t["checks_recorded_by"] = "issue_flow_gate"
    t["checks_digest"] = sha256(canonical(t["checks"]))
    t["claimed"] = None
    t["teacher"] = None
    why = "not recorded by the issue flow"
    t["cost"] = {
        "denomination": "USD",
        "basis": "reported",
        "components": {name: {"usd": None, "unknown_reason": why} for name in ("agent", "decisions")},
        "known_subtotal_usd": None,
        "total_usd": None,
        "complete": False,
    }
    t["cost_usd"] = None
    t["wall_secs"] = None
    return t


def mark_captured(folder: Path, t: dict) -> None:
    """Tell the issue-run that this folder's trace is stored, so a later run
    may clean its worktree (only when the digests match)."""
    body = {"trace": t["id"], "diff_digest": t.get("diff_digest"), "captured_at": t.get("captured_at"),
            "capture_error": t.get("capture_error")}
    tmp = folder / f"trace-captured.json.{os.getpid()}.tmp"
    tmp.write_text(json.dumps(body, indent=1, sort_keys=True) + "\n")
    os.replace(tmp, folder / "trace-captured.json")


def read_jsonl(path: Path) -> list[dict]:
    rows = []
    if path.exists():
        for line in path.read_text().splitlines():
            try:
                rows.append(json.loads(line))
            except ValueError:
                pass
    return rows


def write_jsonl(path: Path, rows: list[dict]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(f".{os.getpid()}.tmp")
    tmp.write_text("".join(json.dumps(r, sort_keys=True) + "\n" for r in rows))
    os.replace(tmp, path)


def save_traces(store: Path, new: list[dict]) -> list[dict]:
    """Merge `new` into traces.jsonl by id. A trace already stored keeps its
    first capture: re-capturing never rewrites recorded labels. Safe to call
    from concurrent trials (a lock file serializes writers)."""
    import fcntl

    path = store / "traces.jsonl"
    store.mkdir(parents=True, exist_ok=True)
    with open(store / "traces.lock", "w") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        rows = {r["id"]: r for r in read_jsonl(path)}
        for t in new:
            if t:
                rows.setdefault(t["id"], t)
        out = sorted(rows.values(), key=lambda r: r["id"])
        write_jsonl(path, out)
    return out


def cmd_capture(a) -> None:
    store, repo = Path(a.store), Path(a.repo)
    new = []
    for root in a.ab or []:
        root = Path(os.path.expanduser(root))
        for f in sorted(glob.glob(str(root / "**" / "result.json"), recursive=True)):
            t = capture_ab(store, repo, Path(f).parent, Path(a.tasks), root)
            if t:
                new.append(t)
    folders = []
    for root in a.issue_runs or []:
        root = os.path.expanduser(root)
        found = set(glob.glob(os.path.join(root, "*", "summary.json"))) | set(
            glob.glob(os.path.join(root, "*", "run.json")))
        folders += sorted({Path(f).parent for f in found})
    folders += [Path(os.path.expanduser(f)) for f in getattr(a, "issue_run_folders", None) or []]
    captured = []
    for folder in folders:
        t = capture_issue_run(store, repo, folder)
        if t:
            new.append(t)
            captured.append((folder, t["id"]))
    issue = getattr(a, "issue", None)
    for spec in getattr(a, "landed", None) or []:
        for commit in filter(None, (c.strip() for c in spec.split(","))):
            if issue is None:
                raise SystemExit("capture --landed needs --issue N")
            new.append(capture_landed(store, repo, commit, issue, getattr(a, "check", None)))
    rows = save_traces(store, new)
    stored = {r["id"]: r for r in rows}
    for folder, tid in captured:
        # The stored trace, which may be an earlier capture of this folder.
        mark_captured(folder, stored[tid])
    bad = sum(1 for t in new if t.get("capture_error"))
    print(f"capture: {len(new)} runs read ({bad} with no replayable diff), {len(rows)} traces in {store}")


# ---------------------------------------------------------------- replay


def free_gb(path: str = "/") -> float:
    st = os.statvfs(path)
    return st.f_bavail * st.f_frsize / 1e9


class Unverifiable(Exception):
    pass


def run_checks_mac(t: dict, patch: bytes, store: Path, repo: Path, limit: int) -> dict:
    """Run the trace's checks on this computer in a fresh worktree at the
    base commit, with a dedicated CARGO_TARGET_DIR; both are deleted."""
    spec = t["check_spec"]
    argvs = []
    if spec["kind"] == "commands":
        argvs = [c["argv"] for c in spec["checks"]]
    needs_cargo = spec["kind"] == "ab-grade" or any(a and a[0] == "cargo" for a in argvs)
    if needs_cargo and free_gb() < MIN_FREE_GB:
        raise Unverifiable(f"this computer has {free_gb():.0f} GB free, under the {MIN_FREE_GB:.0f} GB floor for a build")
    tmp = Path(tempfile.mkdtemp(prefix="traces-replay-"))
    wt, target = tmp / "wt", tmp / "target"
    try:
        r = git(repo, "worktree", "add", "-q", "--detach", str(wt), t["base"])
        if r.returncode:
            raise Unverifiable("cannot check out the base: " + r.stderr.decode(errors="replace")[:200])
        env = dict(os.environ, CARGO_TARGET_DIR=str(target), CARGO_TERM_COLOR="never")
        applied = True
        if patch.strip():
            applied = subprocess.run(["git", "apply", "--binary", "--whitespace=nowarn", "-"], cwd=wt,
                                     input=patch, capture_output=True).returncode == 0
        if spec["kind"] == "commands":
            out = {}
            for c in spec["checks"]:
                if not applied:
                    out[c["id"]] = False
                    continue
                try:
                    p = subprocess.run(c["argv"], cwd=wt, env=env, capture_output=True, timeout=limit)
                    out[c["id"]] = p.returncode == 0
                except subprocess.TimeoutExpired:
                    out[c["id"]] = False
            return out
        return _ab_grade_local(spec, store, wt, env, applied, limit)
    finally:
        git(repo, "worktree", "remove", "--force", str(wt))
        shutil.rmtree(tmp, ignore_errors=True)


def _ab_grade_local(spec: dict, store: Path, wt: Path, env: dict, applied: bool, limit: int) -> dict:
    """remote/eval.sh's steps, on this computer."""
    g = {k: None for k in AB_FIELDS}
    g.update(applied=applied, passed=0, failed=0)
    if not applied:
        return g
    pkg = spec["package"]
    g["compiles"] = subprocess.run(["cargo", "check", "-q", "-p", pkg, "--tests", "--keep-going"], cwd=wt, env=env,
                                   capture_output=True, timeout=limit).returncode == 0
    subprocess.run(["git", "add", "-A"], cwd=wt, capture_output=True)
    tests = get_blob(store, spec["tests_patch_digest"]) or b""
    if tests.strip():
        with tempfile.NamedTemporaryFile(suffix=".patch", delete=False) as f:
            f.write(tests)
        g["tests_applied"] = subprocess.run([sys.executable, str(OVERLAY), f.name], cwd=wt,
                                            capture_output=True).returncode == 0
        os.unlink(f.name)
    else:
        g["tests_applied"] = True
    names = spec["test_names"]
    if g["tests_applied"] and names:
        p = subprocess.run(["cargo", "test", "-p", pkg, *spec["test_targets"], "--no-fail-fast", "--", *names],
                           cwd=wt, env=env, capture_output=True, timeout=limit)
        text = p.stdout.decode(errors="replace") + p.stderr.decode(errors="replace")
        g["tests_compiled"] = "could not compile" not in text
        g["passed"] = sum(int(n) for n in re.findall(r"test result: [a-zA-Z]+\. (\d+) passed", text))
        g["failed"] = sum(int(n) for n in re.findall(r"(\d+) failed", text))
        g["tests_pass"] = p.returncode == 0 and g["passed"] >= len(names)
    return g


def run_checks_host(t: dict, patch: bytes, store: Path, host: str, limit: int) -> dict:
    """The A/B grader on the build host, in its second checkout."""
    spec = t["check_spec"]
    if spec["kind"] != "ab-grade":
        raise Unverifiable("only the A/B grade runs on the build host; run command checks with --on local")
    tests = get_blob(store, spec["tests_patch_digest"])
    if tests is None:
        raise Unverifiable("the stored test overlay is missing")
    buf = io.BytesIO()
    with tarfile.open(fileobj=buf, mode="w") as tar:
        names = spec["test_names"]
        targets = spec["test_targets"]
        for name, data in (
            ("change.patch", patch),
            ("tests.patch", tests),
            ("names.txt", ("\n".join(names) + ("\n" if names else "")).encode()),
            ("targets.txt", ("\n".join(targets) + ("\n" if targets else "")).encode()),
        ):
            info = tarfile.TarInfo(name)
            info.size = len(data)
            tar.addfile(info, io.BytesIO(data))
    ssh = ["ssh", "-o", "ConnectTimeout=20", host,
           f"AB_BUILD=v bash ~/ab/bin/eval.sh 7 {t['base']} {spec['package']} {limit}"]
    p = subprocess.run(ssh, input=buf.getvalue(), capture_output=True, timeout=limit * 2 + 600)
    out = p.stdout.decode(errors="replace").strip().splitlines()
    try:
        g = json.loads(out[-1])
    except (IndexError, ValueError):
        raise Unverifiable("the build host returned no grade: " + p.stderr.decode(errors="replace")[-300:])
    return {k: g.get(k) for k in AB_FIELDS}


def replay(t: dict, store: Path, repo: Path, on: str, limit: int = 1500, checks_runner=None) -> dict:
    """Replay one trace and return its receipt. The steps stop at the first
    divergence, which the receipt names."""
    started = time.time()
    rec = {
        "v": RECEIPT_SCHEMA,
        "trace": t["id"],
        "at": _now(),
        "host": on,
        "base": t.get("base"),
        "clean": True,
    }

    def done(verification: str, **extra) -> dict:
        rec.update(extra)
        rec["verification"] = verification
        rec["verdict"] = {"passed": "verified", "failed": "rejected"}.get(verification, "unverifiable")
        if verification == "passed":
            rec["class"] = EXACT_REPLAY
        rec["secs"] = round(time.time() - started, 1)
        rec["digest"] = sha256(canonical({k: v for k, v in rec.items() if k not in ("digest", "secs", "at")}))
        return rec

    def diverged(field: str, expected, actual) -> dict:
        return done("failed", divergent={"field": field, "expected": expected, "actual": actual})

    if t.get("capture_error"):
        return done("unverifiable", reason=t["capture_error"])
    patch = get_blob(store, t["diff_digest"])
    if patch is None:
        return done("unverifiable", reason="the stored diff is missing")
    # 1. The diff is the one the trace recorded.
    actual = sha256(patch)
    rec["diff_digest"] = actual
    if actual != t["diff_digest"]:
        return diverged("diff_digest", t["diff_digest"], actual)
    # 2. It makes the same tree at the base commit, in a clean index.
    tree, why = tree_after(repo, t["base"], patch)
    rec["result_tree"] = tree
    if tree != t["result_tree"]:
        return diverged("result_tree", t["result_tree"], tree or why)
    # 3. The labels the trace carries are the diff's own.
    files = diff_files(patch.decode(errors="replace"))
    if files != t["files_changed"]:
        return diverged("files_changed", t["files_changed"], files)
    # 4. The same verify checks, run again from a clean checkout.
    try:
        if t["check_spec"]["kind"] == "none":
            # A docs-only landing: the diff, tree and files replayed; no check applies.
            rec["checks"] = "none"
            return done("passed")
        if t["check_spec"]["kind"] == "empty":
            checks = {"tests_pass": False if not patch.strip() else None}
        elif checks_runner is not None:
            checks = checks_runner(t, patch)
        elif on in LOCAL:
            checks = run_checks_mac(t, patch, store, repo, limit)
        else:
            checks = run_checks_host(t, patch, store, on, limit)
    except Unverifiable as e:
        return done("unverifiable", reason=str(e))
    rec["checks"] = checks
    rec["checks_digest"] = sha256(canonical(checks))
    if rec["checks_digest"] != t["checks_digest"]:
        first = next((k for k in sorted(set(checks) | set(t["checks"])) if checks.get(k) != t["checks"].get(k)), "checks")
        return diverged("checks." + first, t["checks"].get(first), checks.get(first))
    claimed = (t.get("claimed") or {}).get("files")
    if claimed is not None and claimed != files:
        # The run's own account of its change disagrees with the diff. The
        # labels never used it; say so on the receipt.
        rec["claim_mismatch"] = {"claimed": claimed, "diff": files}
    return done("passed")


def cmd_replay(a) -> None:
    store, repo = Path(a.store), Path(a.repo)
    traces = read_jsonl(store / "traces.jsonl")
    done = {r["trace"]: r for r in read_jsonl(store / "replays.jsonl")}
    todo = [t for t in traces
            if (not a.ids or t["id"] in a.ids)
            and (not a.issues or t["issue"] in a.issues)
            and (a.again or t["id"] not in done)]
    todo.sort(key=lambda t: (t.get("base") or "", t["id"]))  # one base at a time keeps builds warm
    if a.limit:
        todo = todo[: a.limit]
    path = store / "replays.jsonl"
    for t in todo:
        rec = replay(t, store, repo, a.on, a.timeout)
        with open(path, "a") as f:
            f.write(json.dumps(rec, sort_keys=True) + "\n")
        what = rec.get("divergent", {}).get("field") or rec.get("reason") or ""
        print(f"{t['id']}: {rec['verdict']} {what} ({rec['secs']}s)", flush=True)


# ---------------------------------------------------------------- admit


def accepted(t: dict, checks: dict) -> bool:
    if t["check_spec"] and t["check_spec"]["kind"] == "ab-grade":
        return bool(checks.get("tests_pass"))
    return bool(checks) and all(checks.values())


def load_partition_map(path: Path = CORPUS_MAP) -> dict:
    """The corpus's issue -> partition map (issues.tsv: issue, partition, ...),
    with the file's digest so an admitted row names the map it used."""
    data = Path(path).read_bytes()
    issues = {}
    for i, line in enumerate(data.decode().splitlines()):
        cols = line.split("\t")
        if i == 0 or len(cols) < 2:
            continue
        if cols[1] not in ROLES:
            raise ValueError(f"{path}: issue {cols[0]} has unknown partition {cols[1]!r}")
        issues[int(cols[0])] = cols[1]
    return {"path": str(path), "digest": sha256(data), "issues": issues}


def partition_of(t: dict, pmap: dict) -> dict:
    """The trace's partition: its issue group's role in the corpus map. Never
    a default: an issue the map does not hold has no role, and no items."""
    role = pmap["issues"].get(int(t["issue"]))
    out = {"role": role, "group": f"issue-{t['issue']}", "map": os.path.basename(pmap["path"]),
           "map_digest": pmap["digest"]}
    if role is None:
        out["why"] = "the issue is not in the corpus map; no partition until the corpus assigns its group"
    return out


def corpus_items(t: dict, rec: dict, pmap: dict | None = None) -> list[dict]:
    """The trace's corpus items, in the `tenancy::training` CorpusItem
    shape. One outcome item (did the replayed checks pass), and one
    file item per path the diff changed when they did.

    The partition is the issue group's partition in the #11215 corpus map
    (LEARN-02): a calibration, development or locked group keeps its role,
    and an issue outside the map yields no items."""
    pmap = pmap if pmap is not None else load_partition_map()
    role = partition_of(t, pmap)["role"]
    if role is None:
        return []
    unchecked = (t.get("check_spec") or {}).get("kind") == "none"
    ok = not unchecked and accepted(t, rec["checks"])
    prov = {
        "source": f"openagents repository, {t['id']} (#11218)",
        "license": "Apache-2.0 (repository)",
        "permission": "owner-run agent trace on the public repository, admitted by replay receipt " + rec["digest"],
    }
    common = {"group": f"issue-{t['issue']}", "partition": role, "label_source": "measurement",
              "provenance": prov}
    items = [dict(common,
                  id=f"{t['id']}#outcome",
                  state={"issue": t["issue"], "base": t["base"], "briefing_digest": t.get("briefing_digest"),
                         "diff_digest": t["diff_digest"], "files_changed": t["files_changed"],
                         "opened_outside_briefing": t.get("opened_outside_briefing"),
                         "teacher": t.get("teacher")},
                  label="unchecked" if unchecked else "accepted" if ok else "rejected",
                  label_rule="the verify checks, replayed from a clean checkout at the base commit, passed")]
    if ok:
        for path in t["files_changed"]:
            items.append(dict(common, id=f"{t['id']}#{path}",
                              state={"issue": t["issue"], "base": t["base"], "path": path},
                              label="changed",
                              label_rule="the file is in a diff whose replayed checks passed"))
    assert all(i["partition"] == pmap["issues"][int(t["issue"])] for i in items)
    return items


def cmd_admit(a) -> None:
    store = Path(a.store)
    pmap = load_partition_map(Path(getattr(a, "corpus_map", None) or CORPUS_MAP))
    traces = {t["id"]: t for t in read_jsonl(store / "traces.jsonl")}
    latest = {}
    for r in read_jsonl(store / "replays.jsonl"):
        latest[r["trace"]] = r
    rows, counts, roles = [], {}, {}
    for tid, rec in sorted(latest.items()):
        counts[rec["verdict"]] = counts.get(rec["verdict"], 0) + 1
        t = traces.get(tid)
        if t is None or rec["verdict"] != "verified":
            continue
        admitted = dict(t, evidence_class=EXACT_REPLAY, replay=rec)
        part = partition_of(t, pmap)
        roles[part["role"] or "unmapped"] = roles.get(part["role"] or "unmapped", 0) + 1
        rows.append({"trace": admitted, "partition": part, "items": corpus_items(t, rec, pmap)})
    write_jsonl(store / "admitted.jsonl", rows)
    print(f"admit: {len(rows)} traces admitted to {store / 'admitted.jsonl'} (replays: {counts}; "
          f"partitions from {os.path.basename(pmap['path'])} {pmap['digest'][:19]}: {roles})")


def attempt_inventory(traces: dict, latest: dict) -> list[dict]:
    """One row per captured attempt, whether or not it was replayed, with its
    outcome, attempt position and cost (unknown stays None, with reasons)."""
    rows = []
    for tid, t in sorted(traces.items()):
        rec = latest.get(tid) or {}
        cost = t.get("cost")
        rows.append({
            "trace": tid, "kind": (t.get("source") or {}).get("kind"), "issue": t.get("issue"),
            "run_id": t.get("run_id"), "attempt": t.get("attempt"), "arm": t.get("arm"), "rep": t.get("rep"),
            "outcome": t.get("outcome"), "capture_error": t.get("capture_error"),
            "cost_usd": t.get("cost_usd"),
            "cost_complete": cost.get("complete") if cost else t.get("cost_usd") is not None,
            "cost_unknown": [p.get("unknown_reason") for p in (cost or {}).get("components", {}).values()
                             if p.get("usd") is None] if cost else
                            ([t.get("cost_unknown_reason")] if t.get("cost_usd") is None else []),
            "replay_verdict": rec.get("verdict"),
        })
    return rows


def cmd_manifest(a) -> None:
    store = Path(a.store)
    traces = {t["id"]: t for t in read_jsonl(store / "traces.jsonl")}
    latest = {}
    for r in read_jsonl(store / "replays.jsonl"):
        latest[r["trace"]] = r
    pmap = load_partition_map(Path(getattr(a, "corpus_map", None) or CORPUS_MAP))
    rows = []
    for tid, rec in sorted(latest.items()):
        t = traces.get(tid, {})
        rows.append({
            "partition": pmap["issues"].get(t.get("issue")) if t else None,
            "trace": tid, "issue": t.get("issue"), "base": t.get("base"), "arm": t.get("arm"),
            "diff_digest": t.get("diff_digest"), "result_tree": t.get("result_tree"),
            "files_changed": t.get("files_changed"), "checks": t.get("checks"),
            "verdict": rec["verdict"], "class": rec.get("class"), "host": rec.get("host"),
            "divergent": rec.get("divergent"), "reason": rec.get("reason"),
            "claim_mismatch": rec.get("claim_mismatch"), "receipt_digest": rec["digest"],
        })
    body = {"v": "openagents.coder-trace-manifest.v1", "generated": _now(),
            "partition_map": {"map": os.path.basename(pmap["path"]), "digest": pmap["digest"]},
            "admitted": sum(1 for r in rows if r["verdict"] == "verified"), "rows": rows}
    body["digest"] = sha256(canonical(rows))
    # Every captured attempt, replayed or not: failed setups, cancellations,
    # killed runs and retries stay in the export (#11230).
    attempts = attempt_inventory(traces, latest)
    body["attempts"] = attempts
    body["attempts_digest"] = sha256(canonical(attempts))
    Path(a.out).write_text(json.dumps(body, indent=1, sort_keys=True) + "\n")
    print(f"manifest: {len(rows)} receipts ({body['admitted']} admitted) -> {a.out}")


def main(argv=None) -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--store", default=str(STORE))
    ap.add_argument("--repo", default=str(REPO))
    sub = ap.add_subparsers(dest="cmd", required=True)
    c = sub.add_parser("capture")
    c.add_argument("--ab", nargs="*", default=[], help="A/B results directories (…/.work/results)")
    c.add_argument("--tasks", default=str(HERE.parent / "briefed-ab" / "tasks"))
    c.add_argument("--issue-runs", nargs="*", default=[], help="e.g. ~/.openagents/coder-new/issue-runs")
    c.add_argument("--issue-run-folders", nargs="*", default=[], help="single run folders (issue-run calls this)")
    c.add_argument("--landed", action="append", default=[],
                   help="a commit the issue flow or landing queue landed; repeatable, or A,B")
    c.add_argument("--issue", type=int, help="the issue the --landed commits resolve")
    c.add_argument("--check", action="append", default=[],
                   help="a check command for --landed (default: cargo test -p PKG per touched package)")
    r = sub.add_parser("replay")
    r.add_argument("--on", default="local",
                   help="local (this computer; mac is the same), or a build host name (A/B grades only)")
    r.add_argument("--ids", nargs="*")
    r.add_argument("--issues", nargs="*", type=int)
    r.add_argument("--limit", type=int, default=0)
    r.add_argument("--timeout", type=int, default=1500)
    r.add_argument("--again", action="store_true", help="replay traces that already have a receipt")
    ad = sub.add_parser("admit")
    ad.add_argument("--corpus-map", default=str(CORPUS_MAP), help="the corpus's issues.tsv (#11215)")
    m = sub.add_parser("manifest")
    m.add_argument("--corpus-map", default=str(CORPUS_MAP))
    m.add_argument("--out", required=True)
    a = ap.parse_args(argv)
    {"capture": cmd_capture, "replay": cmd_replay, "admit": cmd_admit, "manifest": cmd_manifest}[a.cmd](a)


if __name__ == "__main__":
    main()
