#!/usr/bin/env python3
"""The briefed agent's learning loop (#11312).

Every briefed run records what would improve the next one: the briefing it
got, the files it read and changed, the files it had to open outside the
briefing (misses), whether its change passed, and what it cost. This reads
those records back:

    learn.py collect  [--runs DIR ...] [--ab DIR ...] [--ledger-in FILE ...] --out LEDGER
    learn.py examples --ledger LEDGER --dataset DATASET --repo REPO --out DATASET2
    learn.py levers   --ledger LEDGER [--json OUT] [--md OUT]
    learn.py cycle    --repo REPO --work WORK [--promote] [--spend-cap USD] ...
    learn.py record   --report WORK/report.json --doc docs/inference/filefind-promotions.md

- `collect` turns run directories into one ledger line per run
  (`openagents.work.learn.run.v1`): production runs of `work_issue.py`
  (`~/.openagents/work/runs/*`, a work host's `host-runs/*/out`), the A/B
  bench's trials (`result.json` under `--ab`), and ledgers other machines
  uploaded (`--ledger-in`, the work hosts' `upload`). A run's own record is
  read, never its agent's report: changed files come from the run's diff.
- `examples` writes filefind's training dataset with the runs folded in: an
  accepted run's changed files are positives and its misses are half-weight
  positives (`extra`) on the issue's case; an issue with no case from main's
  history becomes a `production` case at the run's base commit. Issues in
  the held-out evaluation window are never touched, and the bench's split
  keeps a production case out of training until it is older than that
  window, so the gate stays held-out.
- `levers` reports briefing size against misses, escalations and cost per
  accepted change, by engine. It recommends; it never edits `common.LEVERS`
  (#11309 sets the production levers).
- `cycle` is the scheduled job (`scripts/work/learn-job/run.sh`): collect,
  examples, `scripts/filefind/retrain.sh` on the merged dataset under a
  spend cap, the gate's comparison, a no-regression check on the goldens
  (top-8 recall on the briefed-agent bench's tasks), and with `--promote`
  the gate's `promote` when both passed. It writes a report (JSON and Markdown);
  `record` adds a promotion to docs/inference/filefind-promotions.md.
"""

from __future__ import annotations

import argparse
import glob
import json
import os
import subprocess
import sys
import time
from collections import defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
RUN_SCHEMA = "openagents.work.learn.run.v1"
REPORT_SCHEMA = "openagents.work.learn.report.v1"
# A file read outside the briefing is a weaker label than a file the change touched.
MISS_WEIGHT = 0.5
EVAL_N = 100
MAX_HAND = 25


# ------------------------------------------------------------------ collect

def _load(path: Path):
    try:
        return json.loads(path.read_text())
    except (OSError, ValueError):
        return None


def _paths(items) -> list[str]:
    out = []
    for m in items or []:
        p = m.get("file") if isinstance(m, dict) else m
        if isinstance(p, str) and p and not p.startswith("/") and ".." not in p.split("/"):
            out.append(p)
    return sorted(set(out))


def production_record(run_dir: Path, host: str) -> dict | None:
    """One `work_issue.py` run (its result.json) as a ledger line."""
    r = _load(run_dir / "result.json")
    if not isinstance(r, dict) or not r.get("issue"):
        return None
    attempts = r.get("attempts") or []
    briefed = next((a for a in attempts if str(a.get("engine", "")).startswith("briefed")), None)
    summary = (briefed or {}).get("agent", {}).get("summary") or {}
    if not summary and briefed:
        summary = _load(run_dir / briefed["engine"] / "summary.json") or {}
    brief = r.get("briefing") or {}
    changed = _paths((r.get("diff") or {}).get("files"))
    briefing_files = _paths(brief.get("files"))
    misses = _paths(summary.get("misses"))
    if not summary and briefing_files:
        # No agent ledger (a cloud view, or an older run): what the change
        # touched outside the briefing is the miss we can see.
        misses = sorted(set(changed) - set(briefing_files))
    cost_parts = [((a.get("agent") or {}).get("usage") or {}).get("cost_usd") for a in attempts]
    return {
        "v": RUN_SCHEMA, "source": f"run:{host}:{r.get('run_id') or run_dir.name}", "kind": "production",
        "host": host, "run_id": r.get("run_id") or run_dir.name, "repo": r.get("repo"),
        "issue": int(r["issue"]), "title": r.get("title"), "base": r.get("base"),
        "at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime((run_dir / "result.json").stat().st_mtime)),
        "requested_engine": r.get("requested_engine"), "engine": r.get("engine"),
        "escalated": r.get("escalated"), "accepted": bool(r.get("ok")),
        "landed": (r.get("landed") or {}).get("how") if isinstance(r.get("landed"), dict) else r.get("landed"),
        "cost_usd": r.get("cost_usd"),
        "briefed_cost_usd": (((briefed or {}).get("agent") or {}).get("usage") or {}).get("cost_usd"),
        "attempt_costs": cost_parts, "secs": r.get("secs"),
        "briefing": {"files": briefing_files, "confidence": brief.get("confidence"),
                     "finder": brief.get("finder")} if brief else None,
        "changed": changed, "read": _paths(summary.get("files_read")), "misses": misses,
        "verify_fails": ((briefed or {}).get("agent") or {}).get("verify_fails"),
        "error": (r.get("error") or None) and str(r["error"])[:300],
    }


def bench_record(path: Path, root: Path) -> dict | None:
    """One A/B trial (`ab.py`'s result.json). Arm C (the oracle briefing) is
    left out: its briefing is the fix, so its misses say nothing about the finder."""
    r = _load(path)
    if not isinstance(r, dict) or not r.get("issue") or r.get("arm") in ("C", None):
        return None
    if r.get("tests_pass") is None and r.get("accepted") is None:
        return None  # not graded yet
    accepted = r.get("accepted") if r.get("accepted") is not None else bool(r.get("tests_pass"))
    files = _paths(r.get("briefing_files"))
    return {
        "v": RUN_SCHEMA, "source": "ab:" + str(path.parent.relative_to(root)), "kind": "bench",
        "arm": r.get("arm"), "issue": int(r["issue"]), "base": None,
        "at": r.get("finished_at"), "engine": "bare" if r.get("arm") == "A" else "briefed",
        "escalated": None, "accepted": bool(accepted), "landed": None,
        "cost_usd": r.get("cost_usd"), "briefed_cost_usd": None if r.get("arm") == "A" else r.get("cost_usd"),
        "secs": r.get("wall_secs"),
        "briefing": {"files": files, "chars": r.get("briefing_chars"), "finder": r.get("finder_used")} if files else None,
        "changed": _paths(r.get("files_changed")), "read": _paths(r.get("files_read")),
        "misses": _paths(r.get("misses")), "verify_fails": None,
    }


def collect(args) -> list[dict]:
    rows: dict[str, dict] = {}
    host = os.environ.get("OA_LEARN_HOST") or os.uname().nodename
    for d in args.runs or []:
        base = Path(os.path.expanduser(d))
        for res in sorted(glob.glob(str(base / "*" / "result.json")) + glob.glob(str(base / "*" / "out" / "result.json"))):
            rec = production_record(Path(res).parent, host)
            if rec:
                rows[rec["source"]] = rec
    for d in args.ab or []:
        base = Path(os.path.expanduser(d))
        for res in sorted(base.glob("**/result.json")):
            rec = bench_record(res, base)
            if rec:
                rows[rec["source"]] = rec
    for f in args.ledger_in or []:
        for line in Path(os.path.expanduser(f)).read_text().splitlines():
            try:
                rec = json.loads(line)
            except ValueError:
                continue
            if rec.get("v") == RUN_SCHEMA and rec.get("source"):
                rows[rec["source"]] = rec
    return [rows[k] for k in sorted(rows)]


def cmd_collect(args) -> int:
    rows = collect(args)
    text = "".join(json.dumps(r, sort_keys=True) + "\n" for r in rows)
    if args.out == "-":
        sys.stdout.write(text)
    else:
        Path(args.out).write_text(text)
        kinds = defaultdict(int)
        for r in rows:
            kinds[r["kind"]] += 1
        print(f"collect: {len(rows)} runs {dict(kinds)} -> {args.out}", file=sys.stderr)
    return 0


def read_ledger(path) -> list[dict]:
    out = []
    for line in Path(path).read_text().splitlines():
        try:
            r = json.loads(line)
        except ValueError:
            continue
        if r.get("v") == RUN_SCHEMA:
            out.append(r)
    return out


# ------------------------------------------------------------------ examples

def eval_issues(cases: list[dict], n_eval: int = EVAL_N, max_hand: int = MAX_HAND) -> set[int]:
    """The held-out window exactly as file-finding-bench.py's `split` takes it:
    the newest n_eval single-commit cases with at most max_hand files."""
    singles = [c for c in cases if c.get("kind", "single") == "single" and len(c["hand"]) <= max_hand]
    return {c["issue"] for c in singles[:n_eval]}


def _exists(repo: str, rev: str, path: str) -> bool:
    return subprocess.run(["git", "-C", repo, "cat-file", "-e", f"{rev}:{path}"],
                          capture_output=True).returncode == 0


def _issue_text(repo_name: str, n: int, cache: dict) -> dict | None:
    if n in cache:
        return cache[n]
    p = subprocess.run(["gh", "issue", "view", str(n), "--repo", repo_name, "--json", "number,title,body,createdAt"],
                       capture_output=True, text=True)
    cache[n] = json.loads(p.stdout) if p.returncode == 0 else None
    return cache[n]


def examples(ledger: list[dict], dataset: dict, repo: str, n_eval: int = EVAL_N,
             repo_name: str = "OpenAgentsInc/openagents", issue_text=None) -> tuple[dict, dict]:
    """(the merged dataset, what was folded in). Only accepted runs teach."""
    cases = dataset["cases"]
    held = eval_issues(cases, n_eval)
    by_issue = {c["issue"]: c for c in cases}
    pos: dict[int, dict[str, float]] = defaultdict(dict)
    bases: dict[int, str] = {}
    titles: dict[int, str] = {}
    stats = defaultdict(int)
    for r in ledger:
        if r.get("repo") not in (None, repo_name):
            stats["other repository"] += 1
            continue
        if not r.get("accepted"):
            stats["not accepted"] += 1
            continue
        n = int(r["issue"])
        for p in r.get("changed") or []:
            pos[n][p] = 1.0
        for p in r.get("misses") or []:
            pos[n][p] = max(pos[n].get(p, 0.0), MISS_WEIGHT)
        if r.get("base") and n not in bases:
            bases[n] = r["base"]
        if r.get("title"):
            titles[n] = r["title"]
    texts = {}
    added, extended, waiting = [], [], []
    out_cases = [dict(c) for c in cases]
    index = {c["issue"]: i for i, c in enumerate(out_cases)}
    for n, files in sorted(pos.items()):
        if n in held:
            stats["held-out issue (never trained on)"] += 1
            continue
        if n in by_issue:
            c = out_cases[index[n]]
            hand = {h["path"] for h in c["hand"]}
            extra = {p: w for p, w in files.items() if p not in hand and _exists(repo, c["parent"], p)}
            if extra:
                c["extra"] = dict(sorted({**(c.get("extra") or {}), **extra}.items()))
                extended.append(n)
            continue
        base = bases.get(n)
        if not base or subprocess.run(["git", "-C", repo, "cat-file", "-e", f"{base}^{{commit}}"],
                                      capture_output=True).returncode:
            stats["no base commit here"] += 1
            continue
        hand = [{"path": p, "status": "M", "existing": True} for p, w in sorted(files.items())
                if w >= 1.0 and _exists(repo, base, p)]
        extra = {p: w for p, w in sorted(files.items()) if w < 1.0 and _exists(repo, base, p)}
        if not hand:
            stats["no existing changed file"] += 1
            continue
        it = (issue_text or (lambda k: _issue_text(repo_name, k, texts)))(n)
        if not it:
            stats["issue text unavailable"] += 1
            continue
        out_cases.append({"issue": n, "commit": base, "parent": base, "commits": [], "last_commit": base,
                          "kind": "production", "subject": titles.get(n) or it["title"], "title": it["title"],
                          "body": it.get("body") or "", "created": it.get("createdAt"), "closed": None,
                          "hand": hand, "derived": [], "extra": extra})
        added.append(n)
        waiting.append(n)
    merged = dict(dataset, cases=out_cases)
    info = {"runs": len(ledger), "accepted_issues": len(pos), "extended_cases": extended,
            "production_cases": added, "eval_window": [min(held), max(held)] if held else None,
            "skipped": dict(stats),
            "extra_labels": sum(len(c.get("extra") or {}) for c in out_cases)}
    return merged, info


def cmd_examples(args) -> int:
    ledger = read_ledger(args.ledger)
    dataset = json.loads(Path(args.dataset).read_text())
    merged, info = examples(ledger, dataset, args.repo, args.eval)
    tmp = Path(str(args.out) + f".{os.getpid()}.tmp")
    tmp.write_text(json.dumps(merged, indent=1))
    os.replace(tmp, args.out)
    print(json.dumps(info, sort_keys=True))
    return 0


# ------------------------------------------------------------------ levers

def _median(xs):
    xs = sorted(x for x in xs if x is not None)
    if not xs:
        return None
    m = len(xs) // 2
    return xs[m] if len(xs) % 2 else (xs[m - 1] + xs[m]) / 2


def _per_accepted(rows):
    costs = [r.get("cost_usd") for r in rows]
    if not rows or any(c is None for c in costs):
        return None
    acc = sum(1 for r in rows if r.get("accepted"))
    return round(sum(costs) / acc, 3) if acc else None


def levers(ledger: list[dict]) -> dict:
    """Briefing size against misses and cost; escalations; cost per accepted by engine."""
    out = {"by_kind": {}}
    for kind in ("production", "bench"):
        rows = [r for r in ledger if r.get("kind") == kind]
        if not rows:
            continue
        briefed = [r for r in rows if r.get("briefing")]
        recall = []
        for r in briefed:
            ch = set(r.get("changed") or [])
            if ch and r.get("accepted"):
                recall.append(len(ch & set(r["briefing"]["files"])) / len(ch))
        size = defaultdict(list)
        for r in briefed:
            n = len(r["briefing"]["files"])
            size["<=4" if n <= 4 else "5-8" if n <= 8 else ">8"].append(r)
        by_engine = defaultdict(list)
        for r in rows:
            by_engine[(r.get("engine") or "?") + (f" ({r['arm']})" if r.get("arm") else "")].append(r)
        esc = defaultdict(int)
        for r in rows:
            if r.get("escalated"):
                esc[str(r["escalated"]).split(":")[0].split("(")[0].strip()[:60]] += 1
        missed = defaultdict(int)
        for r in briefed:
            for p in r.get("misses") or []:
                missed[p] += 1
        out["by_kind"][kind] = {
            "runs": len(rows), "accepted": sum(1 for r in rows if r.get("accepted")),
            "briefed_runs": len(briefed),
            "briefing_recall_of_accepted_changes": round(sum(recall) / len(recall), 3) if recall else None,
            "misses_per_briefed_run": round(sum(len(r.get("misses") or []) for r in briefed) / len(briefed), 2)
            if briefed else None,
            "by_briefing_size": {k: {"runs": len(v), "accepted": sum(1 for r in v if r.get("accepted")),
                                     "median_misses": _median([len(r.get("misses") or []) for r in v]),
                                     "median_cost": _median([r.get("cost_usd") for r in v]),
                                     "cost_per_accepted": _per_accepted(v)} for k, v in sorted(size.items())},
            "by_engine": {k: {"runs": len(v), "accepted": sum(1 for r in v if r.get("accepted")),
                              "median_cost": _median([r.get("cost_usd") for r in v]),
                              "median_secs": _median([r.get("secs") for r in v]),
                              "cost_per_accepted": _per_accepted(v)} for k, v in sorted(by_engine.items())},
            "escalations": dict(esc),
            "most_missed": sorted(missed.items(), key=lambda kv: (-kv[1], kv[0]))[:10],
        }
    return out


def levers_md(rep: dict) -> str:
    lines = []
    for kind, k in rep["by_kind"].items():
        lines.append(f"**{kind}**: {k['runs']} runs, {k['accepted']} accepted; briefing recall of accepted "
                     f"changes {k['briefing_recall_of_accepted_changes']}; misses per briefed run "
                     f"{k['misses_per_briefed_run']}.")
        lines.append("")
        lines.append("| Briefing files | Runs | Accepted | Median misses | Median $ | $ per accepted |")
        lines.append("|---|---|---|---|---|---|")
        for s, v in k["by_briefing_size"].items():
            lines.append(f"| {s} | {v['runs']} | {v['accepted']} | {v['median_misses']} | "
                         f"{_money(v['median_cost'])} | {_money(v['cost_per_accepted'])} |")
        lines.append("")
        lines.append("| Engine | Runs | Accepted | Median $ | Median s | $ per accepted |")
        lines.append("|---|---|---|---|---|---|")
        for e, v in k["by_engine"].items():
            lines.append(f"| {e} | {v['runs']} | {v['accepted']} | {_money(v['median_cost'])} | "
                         f"{v['median_secs'] if v['median_secs'] is None else round(v['median_secs'])} | "
                         f"{_money(v['cost_per_accepted'])} |")
        if k["escalations"]:
            lines.append("")
            lines.append("Escalations: " + ", ".join(f"{w} ({n})" for w, n in sorted(k["escalations"].items())))
        if k["most_missed"]:
            lines.append("")
            lines.append("Most missed: " + ", ".join(f"`{p}` ({n})" for p, n in k["most_missed"]))
        lines.append("")
    return "\n".join(lines)


def _money(x):
    return "unknown" if x is None else f"{x:.2f}"


def cmd_levers(args) -> int:
    rep = levers(read_ledger(args.ledger))
    if args.json:
        Path(args.json).write_text(json.dumps(rep, indent=1, sort_keys=True))
    md = levers_md(rep)
    if args.md:
        Path(args.md).write_text(md)
    else:
        print(md)
    return 0


# ------------------------------------------------------------------ cycle

class SpendCap(RuntimeError):
    pass


def cycle(args) -> dict:
    """One scheduled pass. Returns the report; never raises past the report."""
    work = Path(args.work)
    work.mkdir(parents=True, exist_ok=True)
    report = {"v": REPORT_SCHEMA, "at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
              "repo_rev": subprocess.run(["git", "-C", args.repo, "rev-parse", "HEAD"], capture_output=True,
                                         text=True).stdout.strip(),
              "spend_cap_usd": args.spend_cap, "promoted": False}
    ledger_path = work / "ledger.jsonl"
    rows = collect(args)
    ledger_path.write_text("".join(json.dumps(r, sort_keys=True) + "\n" for r in rows))
    report["ledger"] = {"runs": len(rows), "production": sum(r["kind"] == "production" for r in rows),
                        "bench": sum(r["kind"] == "bench" for r in rows)}
    report["levers"] = levers(rows)
    env = dict(os.environ)
    # The finder's embedding calls are the loop's only metered spend; filefind
    # stops when the estimate reaches the cap (FILEFIND_SPEND_CAP_USD).
    env["FILEFIND_SPEND_CAP_USD"] = str(args.spend_cap)
    env["FILEFIND_SPEND_LEDGER"] = str(work / "spend.jsonl")
    env["FILEFIND_EXAMPLES"] = str(ledger_path)
    t0 = time.time()
    log_path = work / "retrain.log"
    with open(log_path, "w") as log:  # written as it runs, so a long retrain can be watched
        retrain = os.environ.get("OA_LEARN_RETRAIN") or str(ROOT / "scripts" / "filefind" / "retrain.sh")
        code = subprocess.run(["bash", retrain, str(work / "train")],
                              cwd=args.repo, env=env, stdout=log, stderr=subprocess.STDOUT).returncode
    out = log_path.read_text(errors="replace")
    report["retrain"] = {"exit": code, "secs": round(time.time() - t0)}
    report["spend_usd"] = spend_so_far(work / "spend.jsonl")
    info = work / "train" / "examples.json"
    if info.exists():
        report["examples"] = json.loads(info.read_text())
    cand = rec = None
    for line in out.splitlines():
        if line.startswith("candidate: "):
            cand = line.split(": ", 1)[1].strip()
        if line.startswith("receipt:   "):
            rec = line.split(": ", 1)[1].strip()
    if not rec or not Path(rec).exists():
        report["verdict"] = "no comparison: " + out.strip().splitlines()[-1][-300:] if out.strip() else "no comparison"
        return report
    receipt = json.loads(Path(rec).read_text())
    report["candidate"], report["receipt_path"] = cand, rec
    report["gate"] = {"pass": receipt["pass"], "why": receipt.get("why"), "cases": receipt.get("cases"),
                      "metrics": receipt.get("metrics"), "baseline": receipt.get("baseline"),
                      "candidate": receipt.get("candidate"), "eval_issues": [min(receipt["eval_issues"]),
                                                                             max(receipt["eval_issues"])]}
    active_path = str(Path(args.repo) / "scripts" / "filefind" / "model.json")
    if receipt["pass"]:
        try:
            report["goldens"] = (getattr(args, "goldens", None) or measure_goldens)(work / "train", active_path, cand)
        except Exception as e:  # noqa: BLE001 - an unmeasured golden set blocks the promotion
            report["goldens"] = {"pass": False, "why": f"goldens not measured: {e}"[:300]}
    goldens_ok = bool((report.get("goldens") or {}).get("pass"))
    if receipt["pass"] and goldens_ok and args.promote:
        p = subprocess.run([sys.executable, str(ROOT / "scripts" / "filefind" / "ranker_gate.py"), "promote",
                            "--candidate", cand, "--receipt", rec, "--model",
                            str(Path(args.repo) / "scripts" / "filefind" / "model.json")],
                           capture_output=True, text=True)
        report["promote"] = (p.stdout + p.stderr).strip()
        report["promoted"] = p.returncode == 0
    if not receipt["pass"]:
        report["verdict"] = "kept the active model: " + "; ".join(receipt.get("why") or [])
    elif not goldens_ok:
        report["verdict"] = "kept the active model: the gate passed, the goldens did not: " + \
            str(report["goldens"].get("why"))
    else:
        report["verdict"] = "promoted" if report["promoted"] else "passed (not promoted)"
    return report


GOLDEN_K = 8  # the briefing's file count (common.LEVERS["briefing_files"])
GOLDEN_MAX_LOSS_SE = 2.0


def golden_issues(tasks_dir: Path) -> list[int]:
    """The briefed-agent bench's tasks (scripts/bench/briefed-ab/tasks): the goldens."""
    out = []
    for t in sorted(tasks_dir.glob("*/task.json")):
        try:
            out.append(int(json.loads(t.read_text())["issue"]))
        except (ValueError, KeyError, TypeError):
            continue
    return out


def golden_rows(feats2: dict, model: dict, issues: list[int], k: int = GOLDEN_K, rank=None) -> dict:
    """issue -> share of the fix's files in the model's top k (what a briefing lists)."""
    out = {}
    for n in issues:
        r = feats2.get(n)
        if not r or not r.get("hand"):
            continue
        order = [p for p, _ in rank(model["stage2"], r["feats"])][:k]
        out[n] = len(set(r["hand"]) & set(order)) / len(r["hand"])
    return out


def goldens_check(active_rows: dict, cand_rows: dict) -> dict:
    """No regression on the goldens: the candidate's paired top-8 recall may not
    fall by more than 2 SE. An extra guard the loop applies before promote, beside
    the gate's frozen plan."""
    pairs = [(active_rows[n], cand_rows[n]) for n in sorted(active_rows) if n in cand_rows]
    if len(pairs) < 2:
        return {"pass": False, "why": "fewer than two goldens measured", "cases": len(pairs)}
    d = [c - a for a, c in pairs]
    mean = sum(d) / len(d)
    var = sum((x - mean) ** 2 for x in d) / (len(d) - 1)
    se = (var / len(d)) ** 0.5
    ok = mean >= -GOLDEN_MAX_LOSS_SE * se
    return {"pass": ok, "cases": len(pairs), "k": GOLDEN_K,
            "active": round(sum(a for a, _ in pairs) / len(pairs), 4),
            "candidate": round(sum(c for _, c in pairs) / len(pairs), 4), "diff": round(mean, 4), "se": round(se, 4),
            "why": None if ok else f"top-{GOLDEN_K} recall on the goldens fell {mean:+.4f} (> {GOLDEN_MAX_LOSS_SE} SE)"}


def measure_goldens(train_dir: Path, active_path: str, cand_path: str) -> dict:
    import pickle
    sys.path.insert(0, str(ROOT / "scripts" / "filefind"))
    import filefind as ff  # noqa: E402
    with open(train_dir / "features2.pkl", "rb") as f:
        feats2 = pickle.load(f)
    issues = golden_issues(ROOT / "scripts" / "bench" / "briefed-ab" / "tasks")
    active, cand = json.loads(Path(active_path).read_text()), json.loads(Path(cand_path).read_text())
    return goldens_check(golden_rows(feats2, active, issues, rank=ff.rank),
                         golden_rows(feats2, cand, issues, rank=ff.rank))


def spend_so_far(path: Path) -> float:
    if not path.exists():
        return 0.0
    total = 0.0
    for line in path.read_text().splitlines():
        try:
            total += float(json.loads(line).get("usd") or 0)
        except ValueError:
            continue
    return round(total, 4)


def report_md(rep: dict) -> str:
    g = rep.get("gate") or {}
    lines = [f"### Learning loop, {rep['at'][:10]} (#11312)", ""]
    led = rep.get("ledger") or {}
    lines.append(f"Runs read: {led.get('runs')} ({led.get('production')} production, {led.get('bench')} bench). "
                 f"Spend: ${rep.get('spend_usd', 0):.2f} of the ${rep.get('spend_cap_usd')} cap "
                 "(embeddings, estimated).")
    ex = rep.get("examples")
    if ex:
        lines.append(f"Training rows: {ex['extra_labels']} labels from runs on {len(ex['extended_cases'])} "
                     f"issues already in the dataset, {len(ex['production_cases'])} new production cases "
                     f"(train once older than the held-out window #{ex['eval_window'][0]}..#{ex['eval_window'][1]}"
                     f"); skipped {ex['skipped']}.")
    lines.append("")
    if g:
        lines.append(f"Gate ({g.get('cases')} held-out cases #{g['eval_issues'][0]}..#{g['eval_issues'][1]}): "
                     f"**{rep['verdict']}**.")
        lines.append("")
        lines.append("| Metric | Active | Candidate | Diff | SE |")
        lines.append("|---|---|---|---|---|")
        for k, m in sorted((g.get("metrics") or {}).items()):
            lines.append(f"| {k} | {m['baseline']:.4f} | {m['candidate']:.4f} | {m['diff']:+.4f} | {m['se']:.4f} |")
        lines.append("")
        gd = rep.get("goldens")
        if gd and "diff" in gd:
            lines.append(f"Goldens (top-{gd['k']} recall on {gd['cases']} briefed-agent bench tasks): active "
                         f"{gd['active']:.4f}, candidate {gd['candidate']:.4f}, diff {gd['diff']:+.4f} "
                         f"(SE {gd['se']:.4f}): {'held' if gd['pass'] else 'fell'}.")
            lines.append("")
        lines.append(f"Active `{(g.get('baseline') or {}).get('digest', '')[:19]}`, candidate "
                     f"`{(g.get('candidate') or {}).get('digest', '')[:19]}`.")
    else:
        lines.append(f"No comparison: {rep.get('verdict')}")
    lines.append("")
    lines.append("Levers (recommendations only; production levers are set by #11309):")
    lines.append("")
    lines.append(levers_md(rep.get("levers") or {"by_kind": {}}))
    return "\n".join(lines)


def cmd_cycle(args) -> int:
    rep = cycle(args)
    work = Path(args.work)
    (work / "report.json").write_text(json.dumps(rep, indent=1, sort_keys=True))
    (work / "report.md").write_text(report_md(rep))
    print(report_md(rep))
    return 0


DOC_HEADER = """# filefind ranker promotions (#11312)

Every change to the active ranker (`scripts/filefind/model.json`) that the
learning loop made, newest first. A promotion happens only through the gate
(`scripts/filefind/ranker_gate.py`): a candidate retrained with the accepted
briefed runs' rows (`scripts/work/learn.py`) is measured against the active
model on the held-out window, paired per issue, and is activated only when
recall@50 rises by at least 2 SE and recall@20, recall@100 and calibration do
not fall by more than 2 SE, and only when top-8 recall (what a briefing lists)
on the briefed-agent bench's tasks, the goldens, does not fall by more than
2 SE either. The weekly run is `scripts/work/learn-job/` on `oa-learn-1`
(docs/inference/briefed-agent-ab.md, "Learning loop"). The receipt of the latest promotion is
`scripts/filefind/model.receipt.json`; every week's report, passing or not,
is in `gs://openagentsgemini-filefind-learn/reports/` and on #11313.

<!-- promotions: newest first -->
"""


def promotion_entry(rep: dict) -> str:
    g = rep["gate"]
    ex = rep.get("examples") or {}
    lines = [f"## {rep['at'][:10]}: `{g['candidate']['digest'][:19]}` replaces `{g['baseline']['digest'][:19]}`", ""]
    lines.append(f"Held-out window: {g['cases']} issues, #{g['eval_issues'][0]}..#{g['eval_issues'][1]} "
                 f"(the active model had trained on {g['baseline'].get('overlap_with_eval', 0)} of them; "
                 "the candidate on none).")
    if ex:
        lines.append(f"From runs: {ex.get('extra_labels', 0)} labels on {len(ex.get('extended_cases') or [])} "
                     f"issues, {len(ex.get('production_cases') or [])} production cases; "
                     f"ledger {rep['ledger']['production']} production and {rep['ledger']['bench']} bench runs.")
    lines += ["", "| Metric | Before | After | Diff | SE |", "|---|---|---|---|---|"]
    for k, m in sorted(g["metrics"].items()):
        lines.append(f"| {k} | {m['baseline']:.4f} | {m['candidate']:.4f} | {m['diff']:+.4f} | {m['se']:.4f} |")
    gd = rep.get("goldens") or {}
    if "diff" in gd:
        lines += ["", f"Goldens: top-{gd['k']} recall on {gd['cases']} briefed-agent bench tasks {gd['active']:.4f} "
                  f"-> {gd['candidate']:.4f} ({gd['diff']:+.4f}, SE {gd['se']:.4f})."]
    lines += ["", f"Embedding spend ${rep.get('spend_usd', 0):.2f} (cap ${rep.get('spend_cap_usd')}); "
              f"retrain {rep['retrain']['secs'] // 60} min; main at `{rep.get('repo_rev', '')[:10]}`.", ""]
    return "\n".join(lines)


def cmd_record(args) -> int:
    rep = json.loads(Path(args.report).read_text())
    if not rep.get("promoted"):
        print("not a promotion: nothing recorded", file=sys.stderr)
        return 1
    doc = Path(args.doc)
    text = doc.read_text() if doc.exists() else DOC_HEADER
    marker = "<!-- promotions: newest first -->\n"
    if marker not in text:
        text = text.rstrip("\n") + "\n\n" + marker
    head, tail = text.split(marker, 1)
    doc.write_text(head + marker + "\n" + promotion_entry(rep) + tail)
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)

    def sources(p):
        p.add_argument("--runs", nargs="*", default=[], help="work_issue.py run directories' parents")
        p.add_argument("--ab", nargs="*", default=[], help="A/B results directories")
        p.add_argument("--ledger-in", nargs="*", default=[], help="ledgers collected elsewhere")
    c = sub.add_parser("collect")
    sources(c)
    c.add_argument("--out", required=True)
    e = sub.add_parser("examples")
    e.add_argument("--ledger", required=True)
    e.add_argument("--dataset", required=True)
    e.add_argument("--repo", default=".")
    e.add_argument("--eval", type=int, default=EVAL_N)
    e.add_argument("--out", required=True)
    lv = sub.add_parser("levers")
    lv.add_argument("--ledger", required=True)
    lv.add_argument("--json")
    lv.add_argument("--md")
    cy = sub.add_parser("cycle")
    sources(cy)
    cy.add_argument("--repo", default=".")
    cy.add_argument("--work", required=True)
    cy.add_argument("--promote", action="store_true")
    cy.add_argument("--spend-cap", type=float, default=float(os.environ.get("OA_LEARN_SPEND_CAP_USD", "10")))
    rc = sub.add_parser("record")
    rc.add_argument("--report", required=True)
    rc.add_argument("--doc", required=True)
    a = ap.parse_args()
    return {"collect": cmd_collect, "examples": cmd_examples, "levers": cmd_levers, "cycle": cmd_cycle,
            "record": cmd_record}[a.cmd](a)


if __name__ == "__main__":
    sys.exit(main())
