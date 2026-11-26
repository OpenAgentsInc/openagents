#!/usr/bin/env python3
# Firm reason: 5 (A/B benchmark orchestrator invoking tier2/run_tasks.py with/without subagents)
"""A/B Subagent Parallelization Gym Benchmark Runner.

Runs an identical task suite twice on the exact same model and effort:
  Arm A (single-agent baseline): --no-subagents (OA_AGENT_SUBAGENTS=0)
  Arm B (parallel subagents):    --subagents    (OA_AGENT_SUBAGENTS=1)

Outputs side-by-side wall-clock speedup, pass rate, subagent fan-out,
tokens, and API cost in Markdown + JSON.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[2]
RUN_TASKS = REPO_ROOT / "evals/openagents-desktop/tier2/run_tasks.py"
DEFAULT_TASKS_DIR = Path(__file__).resolve().parent / "tasks"
RESULTS_ROOT = Path(__file__).resolve().parent / "results"


def run_arm(
    label: str,
    tasks_dir: Path,
    out_dir: Path,
    model: str,
    effort: str,
    subagents: bool,
    subagent_effort: str,
    task_filter: str,
    workers: int,
) -> dict[str, Any]:
    cmd = [
        sys.executable,
        str(RUN_TASKS),
        "--tasks-dir", str(tasks_dir),
        "--out", str(out_dir),
        "--worker", "oa-worker",
        "--model", model,
        "--effort", effort,
        "-j", str(workers),
        "--subagents" if subagents else "--no-subagents",
    ]
    if subagent_effort and subagents:
        cmd += ["--subagent-effort", subagent_effort]
    if task_filter:
        cmd += ["--filter", task_filter]

    print(f"\n=== ARM [{label}] ({'subagents=ON' if subagents else 'subagents=OFF'}, model={model}, effort={effort}, subagent_effort={subagent_effort or 'auto'}) ===", flush=True)
    t0 = time.time()
    proc = subprocess.run(cmd, cwd=str(REPO_ROOT), check=False)
    elapsed_s = round(time.time() - t0, 2)
    summary_path = out_dir / "summary.json"
    if not summary_path.is_file():
        raise RuntimeError(f"Arm {label} did not produce {summary_path} (rc={proc.returncode})")
    data = json.loads(summary_path.read_text(encoding="utf-8"))
    data["_arm_wall_s"] = elapsed_s
    data["_rc"] = proc.returncode
    return data


def fmt_s(ms: int | float) -> str:
    return f"{ms / 1000.0:.1f}s"


def fmt_usd(micros: int | float) -> str:
    return f"${micros / 1_000_000.0:.4f}"


def build_report(
    arm_a: dict[str, Any],
    arm_b: dict[str, Any],
    model: str,
    effort: str,
    subagent_effort: str,
    tag: str,
) -> tuple[dict[str, Any], str]:
    by_a = {r["id"]: r for r in (arm_a.get("cases") or arm_a.get("results") or [])}
    by_b = {r["id"]: r for r in (arm_b.get("cases") or arm_b.get("results") or [])}
    ids = list(by_a.keys())
    for k in by_b:
        if k not in by_a:
            ids.append(k)

    rows: list[dict[str, Any]] = []
    common_pass_a_ms = 0
    common_pass_b_ms = 0
    common_pass_n = 0

    for tid in ids:
        ra = by_a.get(tid, {})
        rb = by_b.get(tid, {})
        wa = int(ra.get("wall_ms", 0) or 0)
        wb = int(rb.get("wall_ms", 0) or 0)
        pa = bool(ra.get("passed"))
        pb = bool(rb.get("passed"))
        speedup = (wa / wb) if (wa > 0 and wb > 0) else 0.0
        pct_faster = ((wa - wb) / wa * 100.0) if (wa > 0 and wb > 0) else 0.0
        if pa and pb:
            common_pass_a_ms += wa
            common_pass_b_ms += wb
            common_pass_n += 1
        rows.append({
            "id": tid,
            "a_passed": pa,
            "b_passed": pb,
            "a_wall_ms": wa,
            "b_wall_ms": wb,
            "speedup": round(speedup, 2),
            "pct_faster": round(pct_faster, 1),
            "a_cost_micros": int(ra.get("cost_usd_micros", 0) or 0),
            "b_cost_micros": int(rb.get("cost_usd_micros", 0) or 0),
            "a_turns": int(ra.get("turns", 0) or 0),
            "b_turns": int(rb.get("turns", 0) or 0),
            "b_agent_tasks": int(rb.get("agent_tasks", 0) or 0),
            "a_tool_calls": int(ra.get("tool_calls", 0) or 0),
            "b_tool_calls": int(rb.get("tool_calls", 0) or 0),
            "a_timed_out": bool(ra.get("timed_out")),
            "b_timed_out": bool(rb.get("timed_out")),
        })

    tot_a_ms = sum(r["a_wall_ms"] for r in rows)
    tot_b_ms = sum(r["b_wall_ms"] for r in rows)
    tot_speedup = round(tot_a_ms / tot_b_ms, 2) if tot_b_ms > 0 else 0.0
    tot_pct_faster = round((tot_a_ms - tot_b_ms) / tot_a_ms * 100.0, 1) if tot_a_ms > 0 else 0.0
    common_speedup = round(common_pass_a_ms / common_pass_b_ms, 2) if common_pass_b_ms > 0 else 0.0
    common_pct_faster = round((common_pass_a_ms - common_pass_b_ms) / common_pass_a_ms * 100.0, 1) if common_pass_a_ms > 0 else 0.0

    pass_a = sum(1 for r in rows if r["a_passed"])
    pass_b = sum(1 for r in rows if r["b_passed"])
    cost_a = sum(r["a_cost_micros"] for r in rows)
    cost_b = sum(r["b_cost_micros"] for r in rows)
    total_subagents = sum(r["b_agent_tasks"] for r in rows)

    ab = {
        "tag": tag,
        "model": model,
        "effort": effort,
        "subagent_effort": subagent_effort or "auto",
        "tasks_count": len(rows),
        "arm_a_single": {
            "passed": pass_a,
            "total": len(rows),
            "wall_ms": tot_a_ms,
            "cost_usd_micros": cost_a,
            "input_tokens": int(arm_a.get("total_input_tokens", 0) or 0),
            "output_tokens": int(arm_a.get("total_output_tokens", 0) or 0),
        },
        "arm_b_subagents": {
            "passed": pass_b,
            "total": len(rows),
            "wall_ms": tot_b_ms,
            "cost_usd_micros": cost_b,
            "input_tokens": int(arm_b.get("total_input_tokens", 0) or 0),
            "output_tokens": int(arm_b.get("total_output_tokens", 0) or 0),
            "agent_tasks": total_subagents,
        },
        "delta": {
            "total_speedup": tot_speedup,
            "total_pct_faster": tot_pct_faster,
            "common_pass_count": common_pass_n,
            "common_pass_speedup": common_speedup,
            "common_pass_pct_faster": common_pct_faster,
            "pass_delta": pass_b - pass_a,
        },
        "rows": rows,
    }

    lines = [
        f"# Subagent Parallelization A/B Benchmark (`{tag}`)",
        "",
        f"- **Model:** `{model}` (`effort={effort}`, `subagent_effort={subagent_effort or 'auto'}`)",
        f"- **Pass Rate:** Single-agent `{pass_a}/{len(rows)}` vs. Subagents `{pass_b}/{len(rows)}` (`{pass_b - pass_a:+d}`)",
        f"- **Total Wall Time:** `{fmt_s(tot_a_ms)}` → `{fmt_s(tot_b_ms)}` (**`{tot_speedup:.2f}×`**, `{tot_pct_faster:+.1f}%` faster)",
        f"- **Common-Pass Wall Time ({common_pass_n} tasks):** `{fmt_s(common_pass_a_ms)}` → `{fmt_s(common_pass_b_ms)}` (**`{common_speedup:.2f}×`**, `{common_pct_faster:+.1f}%` faster)",
        f"- **Total Subagents Spawned (Arm B):** `{total_subagents}`",
        f"- **API Cost:** `{fmt_usd(cost_a)}` (single) vs. `{fmt_usd(cost_b)}` (subagents)",
        "",
        "| Task | Single-Agent (`--no-subagents`) | Parallel Subagents (`--subagents`) | Speedup | Subagents | Cost (A → B) |",
        "|---|---:|---:|---:|---:|---:|",
    ]
    for r in rows:
        sa = ("PASS" if r["a_passed"] else ("TO" if r["a_timed_out"] else "FAIL")) + f" {fmt_s(r['a_wall_ms'])}"
        sb = ("PASS" if r["b_passed"] else ("TO" if r["b_timed_out"] else "FAIL")) + f" {fmt_s(r['b_wall_ms'])}"
        lines.append(
            f"| `{r['id']}` | {sa} | {sb} | **{r['speedup']:.2f}×** ({r['pct_faster']:+.1f}%) | {r['b_agent_tasks']} | {fmt_usd(r['a_cost_micros'])} → {fmt_usd(r['b_cost_micros'])} |"
        )
    lines.append("")
    return ab, "\n".join(lines)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--tasks-dir", default=str(DEFAULT_TASKS_DIR))
    ap.add_argument("--filter", default="")
    ap.add_argument("--model", default="gemini-4-neon")
    ap.add_argument("--effort", default="high")
    ap.add_argument("--subagent-effort", default="")
    ap.add_argument("-j", "--workers", type=int, default=1)
    ap.add_argument("--tag", default="ab")
    ap.add_argument("--only-b", action="store_true", help="skip Arm A and compare against --base-a summary.json")
    ap.add_argument("--base-a", default="", help="existing Arm A summary.json to compare against when --only-b is set")
    args = ap.parse_args()

    ts = time.strftime("%Y%m%dT%H%M%SZ", time.gmtime())
    out_root = RESULTS_ROOT / f"{args.tag}-{ts}"
    out_root.mkdir(parents=True, exist_ok=True)
    tasks_dir = Path(args.tasks_dir).resolve()

    if args.only_b and args.base_a:
        arm_a = json.loads(Path(args.base_a).read_text(encoding="utf-8"))
    else:
        arm_a = run_arm("A_single", tasks_dir, out_root / "arm_a_single", args.model, args.effort, False, "", args.filter, args.workers)

    arm_b = run_arm("B_subagents", tasks_dir, out_root / "arm_b_subagents", args.model, args.effort, True, args.subagent_effort, args.filter, args.workers)

    ab, md = build_report(arm_a, arm_b, args.model, args.effort, args.subagent_effort, args.tag)
    (out_root / "ab-summary.json").write_text(json.dumps(ab, indent=2) + "\n", encoding="utf-8")
    (out_root / "ab-report.md").write_text(md, encoding="utf-8")
    print("\n" + md, flush=True)
    print(f"Saved A/B results to: {out_root}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
