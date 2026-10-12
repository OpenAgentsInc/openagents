#!/usr/bin/env python3
"""Briefer arms (#11389): the Briefer's judgments asked of a decision door
(Jev, Clef) or of the constrained Haiku agent, over the same candidates.

The Briefer does two kinds of work: judgments (which candidate files the fix
needs, what kind of change it is) and the generative part (the analysis
text). This measures the judgments only, so the arms compare like with
like. Every arm sees the same candidate files for an issue: `filefind
query --no-cards` (deterministic stages, no decision model), top K, at the
fix's parent. Then:

- `clef`  : one Noul per candidate, "Does the fix for this issue need to
            change this file?", over the issue text and the file's first 80
            lines at the base, plus the issue-kind Choice: the exact
            questions `crates/coder-new/src/issue_run/decide.rs` asks, at
            Clef-Flash on this computer's Ollama (`POST /v1/systemone`).
- `hosted`: the same questions at the OpenAgents decision API
            (`https://openagents.com/api`, connected Pylons then Vertex).
- `jev`   : the same questions at TypeSafe (`TYPESAFE_API_KEY`).
- `subq`  : the same questions at SubQ Snap (`subq-snap-preview`,
            `https://api.subq.ai`, `SUBQ_API_KEY`), priced from its usage at
            $0.05 per million input tokens (output is free).
- `haiku` : `haiku_briefer.ask` with the same candidates: a candidate it
            names (any role but context) is "keep", one it leaves out is
            "drop". Files it names outside the candidates are recorded.

A door's file is kept at p >= 0.35, and at least 3 stay (decide.rs's
KEEP_AT and MIN_FILES). Labels are the fix commit's own files
(`task.json` source_files + test_files): a judgment is right when "keep"
matches "the fix changed it".

    briefer_arms.py run --plan plans/briefer-arms.json [--arms clef,haiku] [--issues 10074 ...]
    briefer_arms.py report --plan plans/briefer-arms.json [--md]

Rows land in WORK/briefer-arms/<tag>/<arm>/<issue>.json; a rerun skips rows
already written. Disagreements between arms on the same (issue, file) are
listed in the report, never resolved.

Downstream pass rate (does a briefing built from each arm's selection lead
to accepted trials) needs agent trials: the bench's `finder` lever takes
`clef-arm` (see briefing.py) so an `ab.py batch` plan can run that arm
beside B0 and BH.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request
from itertools import combinations
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

from common import LEVERS, REPO, WORK, dump_json, load_json  # noqa: E402

KEEP_AT = 0.35
MIN_FILES = 3
RELEVANCE_Q = "Does the fix for this issue need to change this file?"
KIND_Q = "What kind of change does this issue ask for?"
KINDS = [
    ("bug", "Something that should work does not: fix it"),
    ("feature", "New behavior or a new surface"),
    ("docs", "Documentation or text only"),
    ("refactor", "Restructure code without changing behavior"),
    ("config", "Build, release, settings or dependency change"),
]
DOORS = {
    "clef": {"url": "http://127.0.0.1:11434", "model": "clef-flash", "key": None},
    "hosted": {"url": os.environ.get("OPENAGENTS_DECISIONS_URL", "https://openagents.com/api"),
               "model": "jev-latest", "key": None},
    "jev": {"url": "https://api.typesafe.ai", "model": "jev-latest", "key": "TYPESAFE_API_KEY"},
    "subq": {"url": "https://api.subq.ai", "model": "subq-snap-preview", "key": "SUBQ_API_KEY",
             "usd_per_input_token": 0.05 / 1_000_000},
}
SECRETS = Path("/Users/christopherdavid/work/.secrets")
ARMS = ("clef", "hosted", "jev", "subq", "subq-perfile-full", "subq-full", "haiku")
# Arms that ask an existing door a different way: the door they call.
ARM_DOOR = {"subq-perfile-full": "subq", "subq-full": "subq"}
# subq-full: the whole candidate set in one state, the issue split into narrow
# questions (Choice over the files), a file kept when any question gives it
# p >= CURATE_KEEP_AT (SubQ's "curating context" demo uses 0.3).
CURATE_KEEP_AT = 0.3
# Room for the candidates' bodies in one state, under Snap's 384K-token window.
FULL_BUDGET_CHARS = 1_000_000
# Per-file full-body door: one file's cap (the largest ba1 candidate is 441k).
PERFILE_CAP_CHARS = 600_000
# The issue-independent roles a fix's files play; each is one Choice question.
ROLE_QUESTIONS = {
    "role_main_change": "Which file holds the code that must change to do what this issue asks?",
    "role_cause": "Which file contains the cause of the behavior the issue describes?",
    "role_test": "Which file holds the tests that should cover this change?",
    "role_types": "Which file defines the types, data or interfaces the change touches?",
    "role_caller": "Which file calls or wires up the code that changes, and must change with it?",
    "role_surface": "Which file holds the user-facing text, command or screen the issue is about?",
    "role_reviewer": "Which file would a reviewer open first to check the fix?",
}
MAX_PART_QUESTIONS = 10
NONE_OPTION = "none_of_these"
# The secrets file each door key is read from when the environment lacks it.
KEY_FILES = {"TYPESAFE_API_KEY": "typesafe.env", "SUBQ_API_KEY": "subq.env"}


# ------------------------------------------------------------------ inputs

def task_for(issue: int) -> dict:
    import briefing as briefing_mod
    task = load_json(briefing_mod.TASKS / str(issue) / "task.json")
    return task


def issue_state(task: dict) -> str:
    """decide.rs's `issue_state`: the title and the first 4000 characters."""
    body = (task.get("body") or "")[:4000]
    return f"Issue #{task['issue']}: {task['title']}\n\n{body}"


def file_body(rev: str, path: str) -> str:
    proc = subprocess.run(["git", "-C", str(REPO), "show", f"{rev}:{path}"], capture_output=True, text=True)
    return proc.stdout if proc.returncode == 0 else "(the file does not exist at this commit)"


def fill_caps(sizes: list[int], budget: int) -> int:
    """The largest per-file cap c with sum(min(size, c)) <= budget (water-filling):
    small files go whole, the largest ones are cut to the same length."""
    if sum(sizes) <= budget:
        return max(sizes, default=0)
    lo, hi = 0, max(sizes)
    while lo < hi:
        mid = (lo + hi + 1) // 2
        if sum(min(x, mid) for x in sizes) <= budget:
            lo = mid
        else:
            hi = mid - 1
    return lo


def issue_parts(task: dict, limit: int = MAX_PART_QUESTIONS) -> list[str]:
    """The issue body cut into its own paragraphs and list items (structure,
    not meaning: each becomes one narrow question), code fences dropped."""
    body = task.get("body") or ""
    parts, fence, buf = [], False, []
    def flush():
        text = " ".join(x.strip() for x in buf).strip()
        if len(text) >= 40:
            parts.append(text[:300])
        buf.clear()
    for line in body.splitlines():
        if line.strip().startswith("```"):
            fence = not fence
            flush()
            continue
        if fence:
            continue
        st = line.strip()
        if not st or st.startswith("#") or st[:2] in ("- ", "* ") or st[:3].rstrip(".").isdigit():
            flush()
            if st.startswith("#"):
                continue
        buf.append(st.lstrip("-* "))
    flush()
    return parts[:limit]


def curate_questions(task: dict, paths: list[str]) -> dict:
    """subq-full's questions: each a Choice over the candidate files (plus none)."""
    criteria = {p: None for p in paths}
    criteria[NONE_OPTION] = "No listed file"
    qs = {name: {"type": "choice", "instructions": text, "criteria": criteria}
          for name, text in ROLE_QUESTIONS.items()}
    for i, part in enumerate(issue_parts(task)):
        qs[f"part_{i + 1}"] = {"type": "choice", "criteria": criteria,
                               "instructions": f"Which file must change to satisfy this part of the issue: {part}"}
    return qs


def file_head(rev: str, path: str) -> str:
    proc = subprocess.run(["git", "-C", str(REPO), "show", f"{rev}:{path}"], capture_output=True, text=True)
    if proc.returncode != 0:
        return "(the file does not exist at this commit)"
    return "\n".join(proc.stdout.splitlines()[:80])


def candidates(task: dict, k: int, out: Path) -> list[dict]:
    """The shared candidate list, cached so every arm reads the same one."""
    path = out / "candidates" / f"{task['issue']}.json"
    if path.exists():
        return load_json(path)["files"]
    import briefing as briefing_mod
    ff, used = briefing_mod.finder_files(task, dict(LEVERS, briefing_files=k), no_cards=True)
    dump_json(path, {"issue": task["issue"], "finder": used, "files": ff})
    return ff


def labels(task: dict) -> set[str]:
    return set(task.get("source_files") or []) | set(task.get("test_files") or [])


# ------------------------------------------------------------------ doors

def door_key(name: str | None) -> str | None:
    if not name:
        return None
    if os.environ.get(name):
        return os.environ[name]
    f = SECRETS / KEY_FILES.get(name, "typesafe.env")
    if f.exists():
        for line in f.read_text().splitlines():
            line = line.strip().removeprefix("export ")
            if line.startswith(name + "="):
                return line.split("=", 1)[1].strip().strip("'\"")
    return None


def system_one(arm: str, state: str, questions: dict, timeout: float = 120) -> dict:
    """One `POST /v1/systemone`; {"ok", "answers", "model", "door", "ms", "cost_usd", "error"}."""
    door = DOORS[ARM_DOOR.get(arm, arm)]
    body = json.dumps({"model": door["model"], "state": state, "questions": questions}).encode()
    headers = {"content-type": "application/json"}
    key = door_key(door["key"])
    if door["key"] and not key:
        return {"ok": False, "error": f"{door['key']} is not set", "ms": 0}
    if key:
        headers["authorization"] = f"Bearer {key}"
    req = urllib.request.Request(door["url"].rstrip("/") + "/v1/systemone", data=body, headers=headers)
    t0 = time.time()
    for attempt in range(4):
        t0 = time.time()
        try:
            with urllib.request.urlopen(req, timeout=timeout) as resp:
                res = json.loads(resp.read())
            break
        except urllib.error.HTTPError as e:
            text = e.read().decode(errors="replace")[:300]
            if e.code in (429, 503) and attempt < 3:
                time.sleep(2 ** attempt)  # rate limit or a busy door: back off and ask again
                continue
            return {"ok": False, "error": f"HTTP {e.code}: {text}", "ms": round((time.time() - t0) * 1000)}
        except (urllib.error.URLError, TimeoutError, OSError) as e:
            return {"ok": False, "error": f"no answer: {e}", "ms": round((time.time() - t0) * 1000)}
    ms = round((time.time() - t0) * 1000)
    usage = res.get("usage") or {}
    cost = usage.get("cost_usd")
    gw = ((res.get("provider_metadata") or {}).get("gateway") or {}).get("cost")
    if cost is None and gw is not None:
        cost = float(gw)
    if cost is None and arm == "clef":
        cost = 0.0  # this computer's own Ollama: no bill
    if cost is None and door.get("usd_per_input_token") and usage.get("input_tokens") is not None:
        cost = usage["input_tokens"] * door["usd_per_input_token"]
    return {"ok": True, "answers": res.get("answers") or {}, "model": res.get("model"),
            "door": (res.get("service") or {}).get("door") or arm, "ms": ms, "cost_usd": cost,
            "tokens": usage.get("input_tokens")}


def keep_set(probs: list[tuple[str, float]]) -> set[str]:
    """decide.rs: keep p >= KEEP_AT, and at least MIN_FILES by probability."""
    keep = {p for p, v in probs if v >= KEEP_AT}
    for p, _ in sorted(probs, key=lambda x: -x[1]):
        if len(keep) >= MIN_FILES:
            break
        keep.add(p)
    return keep


def run_curate(arm: str, task: dict, cands: list[dict]) -> dict:
    """subq-full: one request, every candidate's full body, many narrow questions."""
    t0 = time.time()
    paths = [c["path"] for c in cands]
    bodies = [file_body(task["parent"], p) for p in paths]
    cap = fill_caps([len(b) for b in bodies], FULL_BUDGET_CHARS)
    cut = [p for p, b in zip(paths, bodies) if len(b) > cap]
    files = "\n\n".join(f"=== File: {p}\n{b[:cap]}" for p, b in zip(paths, bodies))
    state = f"{issue_state(task)}\n\nCandidate files at the issue's base commit:\n\n{files}"
    qs = curate_questions(task, paths)
    r = system_one(arm, state, qs, timeout=300)
    base = {"arm": arm, "kind": None, "extra": [], "questions": len(qs), "cut_files": cut, "cap_chars": cap}
    if not r["ok"]:
        return dict(base, ok=False, judgments=[], keep=[], secs=round(time.time() - t0, 2), cost_usd=0.0,
                    unpriced=0, errors=[r["error"]])
    best = {p: 0.0 for p in paths}
    hits: dict[str, list[str]] = {p: [] for p in paths}
    for name, a in r["answers"].items():
        for p, v in (a.get("probabilities") or {}).items():
            if p in best:
                best[p] = max(best[p], float(v))
                if float(v) >= CURATE_KEEP_AT:
                    hits[p].append(name)
    keep = {p for p in paths if best[p] >= CURATE_KEEP_AT}
    judgments = [{"path": p, "p": round(best[p], 4), "decision": "keep" if p in keep else "drop",
                  "questions": hits[p]} for p in paths]
    return dict(base, ok=True, judgments=judgments, keep=sorted(keep), secs=round(time.time() - t0, 2),
                ms=r["ms"], tokens=r.get("tokens"), cost_usd=round(r["cost_usd"] or 0.0, 6),
                unpriced=0 if r["cost_usd"] is not None else 1, errors=[])


LINE_KEEP_AT = 0.1
MAX_WINDOWS = 250


def fix_hunks(issue: int) -> list[tuple[str, int, int]]:
    """The fix's hunks as (path, first base line, last base line), from the
    task's source.patch and tests.patch."""
    import briefing as briefing_mod
    hunks, path = [], None
    for name in ("source.patch", "tests.patch"):
        f = briefing_mod.TASKS / str(issue) / name
        if not f.exists():
            continue
        for line in f.read_text(errors="replace").splitlines():
            if line.startswith("--- "):
                path = line[6:] if line.startswith("--- a/") else None
            elif line.startswith("@@ ") and path:
                old = line.split()[1].lstrip("-")
                start, _, n = old.partition(",")
                start, n = int(start), int(n or 1)
                hunks.append((path, start, start + max(n, 1) - 1))
    return hunks


def windows_for(rev: str, paths: list[str]) -> list[tuple[str, int, int, str]]:
    """Fixed-size line windows over the kept files, at most MAX_WINDOWS."""
    texts = {p: file_body(rev, p).splitlines() for p in paths}
    total = sum(len(t) for t in texts.values())
    size = max(40, -(-total // MAX_WINDOWS))
    out = []
    for p, lines in texts.items():
        for a in range(0, len(lines), size):
            chunk = lines[a:a + size]
            out.append((p, a + 1, a + len(chunk), "\n".join(f"{a + 1 + i}| {x}" for i, x in enumerate(chunk))))
    return out[:MAX_WINDOWS]


def run_lines(arm: str, task: dict, keep: list[str]) -> dict:
    """The second curation step: which line ranges inside the kept files the
    fix needs (the same narrow questions, a Choice over line windows). The
    packet is the chosen windows' text; scored by the fix hunks it covers."""
    wins = windows_for(task["parent"], keep)
    cap = fill_caps([len(w[3]) for w in wins], FULL_BUDGET_CHARS)
    label = {f"{p}:{a}-{b}": (p, a, b, t) for p, a, b, t in wins}
    state = issue_state(task) + "\n\nLine windows of the files kept for this issue:\n\n" + "\n\n".join(
        f"=== {k}\n{v[3][:cap]}" for k, v in label.items())
    criteria = {k: None for k in label}
    criteria[NONE_OPTION] = "No listed window"
    qs = {n: dict(q, criteria=criteria) for n, q in curate_questions(task, keep).items()}
    r = system_one(arm, state, qs, timeout=300)
    if not r["ok"]:
        return {"ok": False, "error": r["error"]}
    chosen = set()
    for a in r["answers"].values():
        for k, v in (a.get("probabilities") or {}).items():
            if k in label and float(v) >= LINE_KEEP_AT:
                chosen.add(k)
    hunks = fix_hunks(task["issue"])
    in_kept = [h for h in hunks if h[0] in keep]
    def covered(h):
        return any(label[k][0] == h[0] and label[k][1] <= h[2] and h[1] <= label[k][2] for k in chosen)
    return {"ok": True, "windows": len(label), "window_lines": (wins[0][2] - wins[0][1] + 1) if wins else 0,
            "chosen": sorted(chosen), "packet_bytes": sum(len(label[k][3].encode()) for k in chosen),
            "kept_bytes": sum(len(file_body(task["parent"], p).encode()) for p in keep),
            "fix_hunks": len(hunks), "fix_hunks_in_kept": len(in_kept),
            "covered": sum(covered(h) for h in in_kept), "ms": r["ms"], "tokens": r.get("tokens"),
            "cost_usd": round(r["cost_usd"] or 0.0, 6)}


def cmd_lines(a: argparse.Namespace) -> None:
    """Run the line-range step over each subq-full row's kept files."""
    plan = load_json(Path(a.plan))
    out = WORK / "briefer-arms" / plan["tag"]
    rows = []
    for issue in a.issues or plan["issues"]:
        src = out / "subq-full" / f"{issue}.json"
        dst = out / "subq-full-lines" / f"{issue}.json"
        if not src.exists():
            continue
        if dst.exists() and not a.force:
            rows.append(load_json(dst))
            continue
        row = load_json(src)
        res = dict(run_lines("subq-full", task_for(issue), row["keep"]) if row["keep"] else {"ok": False, "error": "nothing kept"},
                   issue=issue)
        dump_json(dst, res)
        rows.append(res)
        print(json.dumps({k: res.get(k) for k in ("issue", "ok", "windows", "packet_bytes", "fix_hunks_in_kept",
                                                   "covered", "cost_usd", "error")}), flush=True)
    ok = [r for r in rows if r.get("ok")]
    if ok:
        pk = sorted(r["packet_bytes"] for r in ok)
        print(json.dumps({"issues": len(ok), "packet_bytes_p50": pk[len(pk) // 2], "packet_bytes_max": pk[-1],
                          "kept_bytes_mean": round(sum(r["kept_bytes"] for r in ok) / len(ok)),
                          "fix_hunks": sum(r["fix_hunks"] for r in ok),
                          "fix_hunks_in_kept": sum(r["fix_hunks_in_kept"] for r in ok),
                          "covered": sum(r["covered"] for r in ok),
                          "usd_total": round(sum(r["cost_usd"] for r in ok), 4)}))


def run_door(arm: str, task: dict, cands: list[dict]) -> dict:
    t0 = time.time()
    state = issue_state(task)
    judgments, costs, unpriced, errors = [], 0.0, 0, []
    choice = {"type": "choice", "instructions": KIND_Q, "criteria": {n: a for n, a in KINDS}}
    r = system_one(arm, state, {"kind": choice})
    kind = None
    if r["ok"]:
        a = r["answers"].get("kind") or {}
        kind = {"choice": a.get("choice"), "probabilities": a.get("probabilities"), "ms": r["ms"],
                "model": r["model"], "door": r["door"]}
        if r["cost_usd"] is None:
            unpriced += 1
        else:
            costs += r["cost_usd"]
    else:
        errors.append(f"kind: {r['error']}")
    probs = []
    for c in cands:
        path = c["path"]
        text = (file_body(task["parent"], path)[:PERFILE_CAP_CHARS] if arm == "subq-perfile-full"
                else file_head(task["parent"], path))
        st = f"{state}\n\nFile: {path}\n\n{text}"
        r = system_one(arm, st, {"relevant": {"type": "noul", "instructions": RELEVANCE_Q}})
        if not r["ok"]:
            errors.append(f"{path}: {r['error']}")
            if "billing" in r["error"] or "credits" in r["error"] or "not set" in r["error"]:
                break  # the door's own refusal: every next question gets it too
            continue
        p = (r["answers"].get("relevant") or {}).get("noul")
        if p is None:
            errors.append(f"{path}: no noul")
            continue
        probs.append((path, float(p)))
        if r["cost_usd"] is None:
            unpriced += 1
        else:
            costs += r["cost_usd"]
        judgments.append({"path": path, "p": round(float(p), 4), "ms": r["ms"], "model": r["model"],
                          "door": r["door"], "tokens": r.get("tokens")})
    keep = keep_set(probs) if probs else set()
    for j in judgments:
        j["decision"] = "keep" if j["path"] in keep else "drop"
    return {"arm": arm, "ok": bool(judgments) and not any("billing" in e for e in errors),
            "kind": kind, "judgments": judgments, "keep": sorted(keep), "extra": [],
            "secs": round(time.time() - t0, 2), "cost_usd": round(costs, 6), "unpriced": unpriced,
            "errors": errors[:20]}


def run_haiku(task: dict, cands: list[dict], out: Path) -> dict:
    import ab
    import haiku_briefer as hb
    levers = dict(LEVERS)
    t = task
    extra = ab.interface_text(task) if levers.get("interface") else ""
    if extra:
        t = dict(task, body=(task["body"] or "") + extra)
    root = ab.make_worktree(task, WORK / "wt" / f"arms-{task['issue']}")
    t0 = time.time()
    try:
        r = hb.ask(t, root, levers, cands or None, out / "haiku-logs" / str(task["issue"]))
        named = []
        if r["ok"]:
            for f in r["answer"].get("files") or []:
                path = str(f.get("path") or "").strip().lstrip("./")
                if path and f.get("role") != "context" and hb.exists_at(root, path) and path not in named:
                    named.append(path)
    finally:
        ab.drop_worktree(task, root)
    cand_paths = [c["path"] for c in cands]
    keep = [p for p in named if p in cand_paths]
    judgments = [{"path": p, "decision": "keep" if p in keep else "drop",
                  "rank": named.index(p) if p in named else None} for p in cand_paths] if r["ok"] else []
    return {"arm": "haiku", "ok": r["ok"], "kind": None, "judgments": judgments, "keep": sorted(keep),
            "extra": [p for p in named if p not in cand_paths], "secs": round(time.time() - t0, 2),
            "agent_secs": r.get("secs"), "cost_usd": r.get("cost_usd"), "unpriced": 0 if r.get("cost_usd") is not None else 1,
            "turns": r.get("turns"), "model": r.get("model"), "errors": [r["error"]] if r.get("error") else [],
            "answer": r.get("answer")}


# ------------------------------------------------------------------ run / report

def cmd_run(a: argparse.Namespace) -> None:
    plan = load_json(Path(a.plan))
    out = WORK / "briefer-arms" / plan["tag"]
    arms = a.arms.split(",") if a.arms else plan["arms"]
    issues = a.issues or plan["issues"]
    k = int(plan.get("candidates", 12))
    for issue in issues:
        task = task_for(issue)
        cands = candidates(task, k, out)
        for arm in arms:
            row_path = out / arm / f"{issue}.json"
            if row_path.exists() and not a.force:
                old = load_json(row_path)
                if old["ok"] or not old.get("limited"):
                    continue  # a usage-limited row is asked again

            if arm == "haiku":
                row = run_haiku(task, cands, out)
            elif arm == "subq-full":
                row = run_curate(arm, task, cands)
            else:
                row = run_door(arm, task, cands)
            row.update(issue=issue, candidates=[c["path"] for c in cands], fix=sorted(labels(task)))
            dump_json(row_path, row)
            print(json.dumps({"issue": issue, "arm": arm, "ok": row["ok"], "keep": len(row["keep"]),
                              "secs": row["secs"], "cost_usd": row["cost_usd"],
                              "errors": row["errors"][:1]}), flush=True)


def summarize(plan: dict, common: bool = False) -> dict:
    """Per-arm metrics, pairwise agreement and divergences. `common`: only
    the issues every arm that answered at all answered (a like-for-like
    table when an arm lost issues to a usage limit)."""
    out = WORK / "briefer-arms" / plan["tag"]
    rows: dict[str, dict[int, dict]] = {}
    for arm in ARMS:
        for f in sorted((out / arm).glob("*.json")) if (out / arm).exists() else []:
            r = load_json(f)
            if r["issue"] in plan["issues"]:
                rows.setdefault(arm, {})[r["issue"]] = r
    if common:
        live = [by for by in rows.values() if any(r["ok"] for r in by.values())]
        keep = set(plan["issues"])
        for by in live:
            keep &= {i for i, r in by.items() if r["ok"]}
        rows = {arm: {i: r for i, r in by.items() if i in keep} for arm, by in rows.items()}
    per_arm = {}
    for arm, by in rows.items():
        ok = [r for r in by.values() if r["ok"]]
        right = total = tp = fp = fn = 0
        recall, cand_recall, top1 = [], [], []
        for r in ok:
            fix = set(r["fix"])
            cand_fix = fix & set(r["candidates"])
            keep = set(r["keep"])
            for j in r["judgments"]:
                lab = j["path"] in fix
                kept = j["decision"] == "keep"
                total += 1
                right += lab == kept
                tp += lab and kept
                fp += kept and not lab
                fn += lab and not kept
            if cand_fix:
                cand_recall.append(len(keep & cand_fix) / len(cand_fix))
            recall.append(len((keep | set(r.get("extra") or [])) & fix) / max(1, len(fix)))
            if arm == "haiku":
                first = (r.get("answer") or {}).get("files") or []
                top1.append(bool(first) and str(first[0].get("path", "")).lstrip("./") in fix)
            else:
                best = max(r["judgments"], key=lambda j: j["p"], default=None)
                top1.append(bool(best) and best["path"] in fix)
        costs = [r["cost_usd"] for r in ok if r["cost_usd"] is not None]
        secs = sorted(r["secs"] for r in ok)
        per_arm[arm] = {
            "issues": len(by), "answered": len(ok),
            "failed": {i: r["errors"][:1] for i, r in by.items() if not r["ok"]},
            "judgments": total, "accuracy": round(right / total, 3) if total else None,
            "precision": round(tp / (tp + fp), 3) if tp + fp else None,
            "recall_in_candidates": round(sum(cand_recall) / len(cand_recall), 3) if cand_recall else None,
            "fix_recall": round(sum(recall) / len(recall), 3) if recall else None,
            "top1_is_fix": round(sum(top1) / len(top1), 3) if top1 else None,
            "kept_mean": round(sum(len(r["keep"]) for r in ok) / len(ok), 2) if ok else None,
            "usd_total": round(sum(costs), 4), "usd_unpriced_issues": sum(1 for r in ok if r["unpriced"]),
            "usd_per_issue": round(sum(costs) / len(costs), 5) if costs else None,
            "secs_p50": secs[len(secs) // 2] if secs else None,
            "secs_p95": secs[min(len(secs) - 1, int(len(secs) * 0.95))] if secs else None,
        }
    agreement, divergences = {}, []
    for x, y in combinations(sorted(rows), 2):
        same = n = 0
        for issue in set(rows[x]) & set(rows[y]):
            rx, ry = rows[x][issue], rows[y][issue]
            if not (rx["ok"] and ry["ok"]):
                continue
            dx = {j["path"]: j for j in rx["judgments"]}
            dy = {j["path"]: j for j in ry["judgments"]}
            fix = set(rx["fix"])
            for path in set(dx) & set(dy):
                n += 1
                if dx[path]["decision"] == dy[path]["decision"]:
                    same += 1
                else:
                    divergences.append({"issue": issue, "path": path, x: dx[path]["decision"],
                                        y: dy[path]["decision"], f"{x}_p": dx[path].get("p"),
                                        f"{y}_p": dy[path].get("p"), "fix_changed_it": path in fix})
        agreement[f"{x}~{y}"] = {"pairs": n, "agree": round(same / n, 3) if n else None}
    return {"tag": plan["tag"], "per_arm": per_arm, "agreement": agreement, "divergences": divergences}


def cmd_report(a: argparse.Namespace) -> None:
    plan = load_json(Path(a.plan))
    s = summarize(plan, common=a.common)
    dump_json(WORK / "briefer-arms" / plan["tag"] / ("summary-common.json" if a.common else "summary.json"), s)
    if not a.md:
        print(json.dumps(s, indent=2))
        return
    print("| Arm | Issues answered | Judgments | Accuracy | Precision (keep) | Recall in candidates | Fix recall | Top 1 is a fix file | Kept (mean) | $ per issue | Seconds p50 / p95 |")
    print("|---|---|---|---|---|---|---|---|---|---|---|")
    for arm, m in s["per_arm"].items():
        usd = "n/a" if m["usd_per_issue"] is None else f"{m['usd_per_issue']:.5f}"
        if m["usd_unpriced_issues"]:
            usd += f" ({m['usd_unpriced_issues']} unpriced)"
        print(f"| {arm} | {m['answered']}/{m['issues']} | {m['judgments']} | {m['accuracy']} | {m['precision']} | "
              f"{m['recall_in_candidates']} | {m['fix_recall']} | {m['top1_is_fix']} | {m['kept_mean']} | {usd} | "
              f"{m['secs_p50']} / {m['secs_p95']} |")
    print()
    for pair, g in s["agreement"].items():
        print(f"- Agreement {pair}: {g['agree']} over {g['pairs']} (issue, file) judgments")
    print(f"- Divergences recorded: {len(s['divergences'])} (summary.json `divergences`)")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    r = sub.add_parser("run")
    r.add_argument("--plan", required=True)
    r.add_argument("--arms")
    r.add_argument("--issues", type=int, nargs="*")
    r.add_argument("--force", action="store_true")
    p = sub.add_parser("report")
    p.add_argument("--plan", required=True)
    p.add_argument("--md", action="store_true")
    p.add_argument("--common", action="store_true", help="only issues every answering arm answered")
    li = sub.add_parser("lines", help="line-range step over subq-full's kept files")
    li.add_argument("--plan", required=True)
    li.add_argument("--issues", type=int, nargs="*")
    li.add_argument("--force", action="store_true")
    a = ap.parse_args()
    {"run": cmd_run, "report": cmd_report, "lines": cmd_lines}[a.cmd](a)


if __name__ == "__main__":
    main()
