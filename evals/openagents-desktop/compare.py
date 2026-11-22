#!/usr/bin/env python3
# Firm reason: 5 (reads Tier 2 and Tier 4 summary.json files and writes Markdown: the baseline diff and the trend table)
"""Compare Coder Desktop eval runs (Tier 2 and Tier 4 summary.json files).

  compare.py diff BASE NEW [--out FILE] [--fail-on-regression]
      BASE and NEW are summary.json files or run folders holding one. Prints a Markdown report:
      pass rate, cost a task, p95 task wall time, the tasks that went from pass to fail
      (regressions; a task marked known_failure in NEW is not one), the fixes, the tasks only one
      side ran, and per-task cost or wall-time rises over 20% (and over $0.005 or 5 s, so noise on
      tiny tasks is not reported). Exit 1 with --fail-on-regression when there is a regression or
      the pass rate fell by more than 5 points.

  compare.py trend [--results DIR] [--tier N] [--out FILE]
      One row per run folder under DIR (default evals/openagents-desktop/results) with a summary.json,
      oldest first: time, commit, tier, pass rate, cost a task, p95 wall, and regressions against
      the previous run of the same tier.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

EVAL_DIR = Path(__file__).resolve().parent
SPIKE = 0.20
MIN_COST_RISE = 5000      # micros
MIN_WALL_RISE = 5000      # ms
MAX_RATE_DROP = 5.0       # percentage points


def load(path: str | Path) -> dict[str, Any]:
    p = Path(path)
    if p.is_dir():
        p = p / "summary.json"
    s = json.loads(p.read_text())
    s["_path"] = str(p)
    return s


def p95(s: dict[str, Any]) -> int:
    if "wall_p95_ms" in s:
        return int(s["wall_p95_ms"])
    walls = sorted(int(c.get("wall_ms") or 0) for c in s.get("cases", []))
    return walls[min(len(walls) - 1, int(round(0.95 * (len(walls) - 1))))] if walls else 0


def rate(s: dict[str, Any]) -> float:
    return 100.0 * s.get("passed", 0) / s["total"] if s.get("total") else 0.0


def per_task(s: dict[str, Any]) -> int:
    return s.get("cost_per_task_usd_micros") or (s.get("cost_usd_micros", 0) // s["total"] if s.get("total") else 0)


def usd(m: int | float) -> str:
    return f"${m / 1e6:.4f}"


def diff(base: dict[str, Any], new: dict[str, Any]) -> dict[str, Any]:
    b = {c["id"]: c for c in base.get("cases", []) if not c.get("not_applicable")}
    n = {c["id"]: c for c in new.get("cases", []) if not c.get("not_applicable")}
    both = [i for i in n if i in b]
    regressions = [i for i in both if b[i].get("passed") and not n[i].get("passed") and not n[i].get("known_failure")]
    fixes = [i for i in both if not b[i].get("passed") and n[i].get("passed")]
    spikes = []
    for i in both:
        bc, nc = int(b[i].get("cost_usd_micros") or 0), int(n[i].get("cost_usd_micros") or 0)
        if bc and nc > bc * (1 + SPIKE) and nc - bc >= MIN_COST_RISE:
            spikes.append((i, "cost", usd(bc), usd(nc)))
        bw, nw = int(b[i].get("wall_ms") or 0), int(n[i].get("wall_ms") or 0)
        if bw and nw > bw * (1 + SPIKE) and nw - bw >= MIN_WALL_RISE:
            spikes.append((i, "wall", f"{bw / 1000:.0f} s", f"{nw / 1000:.0f} s"))
    return {
        "regressions": regressions, "fixes": fixes, "spikes": spikes,
        "only_base": [i for i in b if i not in n], "only_new": [i for i in n if i not in b],
        "rate_drop": rate(base) - rate(new),
    }


def render(base: dict[str, Any], new: dict[str, Any], d: dict[str, Any]) -> str:
    def row(name: str, bv: str, nv: str) -> str:
        return f"| {name} | {bv} | {nv} |"
    b_na = base.get("not_applicable_count", 0)
    n_na = new.get("not_applicable_count", 0)
    b_pass = f"{base.get('passed', 0)}/{base.get('total', 0)} ({rate(base):.0f}%)" + (f" ({b_na} N/A)" if b_na else "")
    n_pass = f"{new.get('passed', 0)}/{new.get('total', 0)} ({rate(new):.0f}%)" + (f" ({n_na} N/A)" if n_na else "")
    out = [f"# OpenAgents Desktop Tier {new.get('tier', '?')} comparison", "",
           f"Base: `{base.get('git_sha', '?')}` ({base.get('created', '?')}, {base['_path']})  ",
           f"New: `{new.get('git_sha', '?')}` ({new.get('created', '?')}, {new['_path']})", "",
           "| | base | new |", "|---|---|---|",
           row("passed", b_pass, n_pass),
           row("engine", str(base.get("engine") or "-"), str(new.get("engine") or "-")),
           row("models", ", ".join(base.get("models", [])) or "-", ", ".join(new.get("models", [])) or "-"),
           row("p50 task wall", f"{int(base.get('wall_p50_ms', 0)) / 1000:.1f} s", f"{int(new.get('wall_p50_ms', 0)) / 1000:.1f} s"),
           row("p95 task wall", f"{p95(base) / 1000:.1f} s", f"{p95(new) / 1000:.1f} s"),
           row("mean TTFT / tok/s", f"{base.get('mean_ttft_ms', 0)} ms / {base.get('mean_decode_tps', 0.0):.1f}",
               f"{new.get('mean_ttft_ms', 0)} ms / {new.get('mean_decode_tps', 0.0):.1f}"),
           row("boot / peak RSS", f"{base.get('server_boot_ms', 0)} ms / {base.get('peak_rss_mb', 0)} MB",
               f"{new.get('server_boot_ms', 0)} ms / {new.get('peak_rss_mb', 0)} MB"),
           row("cost (total / task)", f"{usd(base.get('cost_usd_micros', 0))} / {usd(per_task(base))}",
               f"{usd(new.get('cost_usd_micros', 0))} / {usd(per_task(new))}"), ""]
    out.append(f"Regressions ({len(d['regressions'])}): " + (", ".join(d["regressions"]) or "none"))
    out.append("")
    out.append(f"Fixed ({len(d['fixes'])}): " + (", ".join(d["fixes"]) or "none"))
    if d["only_base"] or d["only_new"]:
        out += ["", f"Only in base: {', '.join(d['only_base']) or 'none'}. Only in new: {', '.join(d['only_new']) or 'none'}."]
    if d["spikes"]:
        out += ["", f"Rises over {int(SPIKE * 100)}%:", "", "| task | what | base | new |", "|---|---|---|---|"]
        out += [f"| {i} | {what} | {bv} | {nv} |" for i, what, bv, nv in d["spikes"]]
    return "\n".join(out) + "\n"


def cmd_diff(args: argparse.Namespace) -> int:
    base, new = load(args.base), load(args.new)
    d = diff(base, new)
    text = render(base, new, d)
    if args.out:
        Path(args.out).write_text(text)
    sys.stdout.write(text)
    bad = bool(d["regressions"]) or d["rate_drop"] > MAX_RATE_DROP
    return 1 if (bad and args.fail_on_regression) else 0


def cmd_trend(args: argparse.Namespace) -> int:
    runs = []
    for p in sorted(Path(args.results).glob("*/summary.json")):
        try:
            s = load(p)
        except (OSError, ValueError):
            continue
        if args.tier and int(s.get("tier", 0)) != args.tier:
            continue
        runs.append(s)
    runs.sort(key=lambda s: s.get("created", ""))
    rows = ["| time (UTC) | commit | tier | passed | cost a task | p95 wall | regressions |", "|---|---|---|---|---|---|---|"]
    last: dict[int, dict[str, Any]] = {}
    for s in runs:
        t = int(s.get("tier", 0))
        reg = "-"
        if t in last:
            r = diff(last[t], s)["regressions"]
            reg = f"{len(r)}" + (f" ({', '.join(r)})" if r else "")
        last[t] = s
        rows.append(f"| {s.get('created', '?')} | {s.get('git_sha', '?')} | {t} | {s.get('passed', 0)}/{s.get('total', 0)} ({rate(s):.0f}%) | "
                    f"{usd(per_task(s))} | {p95(s) / 1000:.0f} s | {reg} |")
    text = "# Coder Desktop eval trend\n\n" + "\n".join(rows) + "\n"
    if args.out:
        Path(args.out).write_text(text)
    sys.stdout.write(text)
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    d = sub.add_parser("diff", help="compare two runs")
    d.add_argument("base")
    d.add_argument("new")
    d.add_argument("--out", default="")
    d.add_argument("--fail-on-regression", action="store_true")
    t = sub.add_parser("trend", help="one row per run")
    t.add_argument("--results", default=str(EVAL_DIR / "results"))
    t.add_argument("--tier", type=int, default=0)
    t.add_argument("--out", default="")
    args = ap.parse_args()
    return cmd_diff(args) if args.cmd == "diff" else cmd_trend(args)


if __name__ == "__main__":
    sys.exit(main())
