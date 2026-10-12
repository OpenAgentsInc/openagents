#!/usr/bin/env python3
"""The #11313 acceptance: the briefed flow against bare Claude Code on real
open issues, three in a row.

For each issue it runs `work_issue.py` twice from the same `origin/main`
commit: the briefed flow (`--engine briefed`, or `briefed:codex`) and bare
Claude Code (`--engine bare --land none`). An arm is accepted when its change
passed the replayed checks (`ok` in the run's result). The briefed flow wins
an issue when it is accepted and its cost per accepted change is lower than
bare Claude Code's (bare not accepted counts as infinite cost). The run is
done when the briefed flow wins every issue, in order.

    # on a cloud dev env with briefed-agent installed and a Claude sign-in
    python3 scripts/work/acceptance.py --issues 11416 11417 11418 \\
        --out ~/.openagents/work/acceptance/$(date +%Y%m%d-%H%M)

    # re-read results already on disk (no runs)
    python3 scripts/work/acceptance.py --report ~/.openagents/work/acceptance/<dir>

It writes `<out>/acceptance.json` and prints a Markdown table for
`docs/inference/briefed-agent-ab.md`. Exit 0 only on all wins.
"""
from __future__ import annotations

import argparse
import json
import math
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
WORK = HERE / "work_issue.py"


def run_arm(repo: str, issue: int, engine: str, land: str, out: Path, checkout: str | None,
            extra: list[str]) -> dict:
    """One `work_issue.py` run; its result is the JSON it writes."""
    out.mkdir(parents=True, exist_ok=True)
    cmd = [sys.executable, str(WORK), "--repo", repo, "--issue", str(issue), "--engine", engine,
           "--land", land, "--out", str(out), *extra]
    if checkout:
        cmd += ["--checkout", checkout]
    with open(out / "stdout.log", "w") as log:
        subprocess.run(cmd, stdout=log, stderr=subprocess.STDOUT, check=False)
    return read_result(out)


def read_result(out: Path) -> dict:
    path = out / "result.json"
    if not path.exists():
        return {"ok": False, "error": f"no result.json in {out}", "cost_usd": None}
    return json.loads(path.read_text())


def per_accepted(r: dict) -> float:
    """Cost of an accepted change; a rejected arm costs infinity."""
    if not r.get("ok"):
        return math.inf
    return float(r["cost_usd"]) if r.get("cost_usd") is not None else math.nan


def verdict(briefed: dict, bare: dict) -> tuple[bool, str]:
    """(win, why) for one issue."""
    b, a = per_accepted(briefed), per_accepted(bare)
    if math.isinf(b):
        return False, "briefed flow not accepted: " + str(briefed.get("error") or "checks failed")
    if math.isnan(b):
        return False, "briefed flow accepted but its cost is unknown"
    if briefed.get("engine") == "bare":
        return False, "briefed flow fell back to bare Claude Code"
    if math.isinf(a):
        return True, "only the briefed flow was accepted"
    if math.isnan(a):
        return False, "bare cost unknown; cannot compare"
    if b < a:
        return True, f"${b:.2f} < ${a:.2f} per accepted change"
    return False, f"${b:.2f} >= ${a:.2f} per accepted change"


def money(r: dict) -> str:
    return "-" if r.get("cost_usd") is None else f"${r['cost_usd']:.2f}"


def secs(r: dict) -> str:
    return "-" if r.get("secs") is None else f"{r['secs']:.0f}"


def table(rows: list[dict]) -> str:
    lines = ["| Issue | Briefed engine | Briefed accepted | Briefed $ | Briefed s | Bare accepted | Bare $ | Bare s | Win | Why |",
             "|---|---|---|---|---|---|---|---|---|---|"]
    for row in rows:
        b, a = row["briefed"], row["bare"]
        lines.append(f"| #{row['issue']} | {b.get('engine') or row['engine']} | {'yes' if b.get('ok') else 'no'} | "
                     f"{money(b)} | {secs(b)} | {'yes' if a.get('ok') else 'no'} | {money(a)} | {secs(a)} | "
                     f"{'yes' if row['win'] else 'no'} | {row['why']} |")
    return "\n".join(lines)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--repo", default="OpenAgentsInc/openagents")
    ap.add_argument("--issues", type=int, nargs="+", default=[11416, 11417, 11418])
    ap.add_argument("--engine", default="briefed", help="the briefed arm's engine (briefed or briefed:codex)")
    ap.add_argument("--land", default="pr", choices=["queue", "pr", "none"], help="how the briefed arm lands")
    ap.add_argument("--out", type=Path)
    ap.add_argument("--checkout")
    ap.add_argument("--report", type=Path, help="read results already under this directory; run nothing")
    ap.add_argument("--keep-going", action="store_true", help="run every issue even after a loss")
    args, extra = ap.parse_known_args()

    root = args.report or args.out
    if root is None:
        ap.error("--out or --report is required")
    root = root.expanduser()
    rows = []
    for issue in args.issues:
        bdir, adir = root / str(issue) / "briefed", root / str(issue) / "bare"
        if args.report:
            briefed, bare = read_result(bdir), read_result(adir)
        else:
            # Bare first and without landing, so both arms see the same origin/main.
            bare = run_arm(args.repo, issue, "bare", "none", adir, args.checkout, extra)
            briefed = run_arm(args.repo, issue, args.engine, args.land, bdir, args.checkout, extra)
        win, why = verdict(briefed, bare)
        if briefed.get("base") and bare.get("base") and briefed["base"] != bare["base"]:
            why += f" (main moved between arms: {bare['base'][:10]} -> {briefed['base'][:10]})"
        rows.append({"issue": issue, "engine": args.engine, "briefed": briefed, "bare": bare, "win": win, "why": why})
        print(f"#{issue}: {'win' if win else 'loss'} ({why})", file=sys.stderr, flush=True)
        if not win and not args.keep_going and not args.report:
            break
    wins = len(rows) == len(args.issues) and all(r["win"] for r in rows)
    root.mkdir(parents=True, exist_ok=True)
    summary = {"issues": args.issues, "engine": args.engine, "all_won": wins,
               "rows": [{k: r[k] for k in ("issue", "win", "why")} |
                        {arm: {k: r[arm].get(k) for k in ("ok", "engine", "cost_usd", "secs", "base", "landed",
                                                           "escalated", "error")}
                         for arm in ("briefed", "bare")} for r in rows]}
    (root / "acceptance.json").write_text(json.dumps(summary, indent=2))
    print(table(rows))
    print(f"\n{'Three wins in a row' if wins else 'Not accepted'}: "
          f"{sum(r['win'] for r in rows)} of {len(args.issues)} issues won.")
    return 0 if wins else 1


if __name__ == "__main__":
    sys.exit(main())
