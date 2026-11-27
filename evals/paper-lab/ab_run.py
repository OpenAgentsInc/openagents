#!/usr/bin/env python3
"""Paper-Lab: Rapid paired A/B evaluation runner for research paper hypotheses.

Runs BASE (control env) vs EXP (paper mechanism env, e.g. OA_EXP_MEMTRACE=1)
against the exact same compiled oa-worker binary through OpenAgents Desktop's
real tier2/run_tasks.py harness, across one or more failure-axis probe banks,
trials (k >= 1), and models (local Inkling 2 Mini and/or cloud models).

Produces:
  - Paired 2x2 matrix: Rescued / Preserved / Damaged / Still Failed
  - Pass@1 and Pass^k consistency
  - Hidden-tax deltas: prompt cache hit rate, tokens, turns, tool calls, wall time, USD cost
  - Automatic verdict tag: SHIP / LOCAL-ONLY / ITERATE / KILL / NEEDS-HARDER-PROBES
  - Permanent record under evals/papers/<paper_id>/{hypothesis.json,verdict.json,verdict.md}
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[2]
PAPER_LAB_DIR = REPO_ROOT / "evals/paper-lab"
PROBES_DIR = PAPER_LAB_DIR / "probes"
PAPERS_OUT_DIR = REPO_ROOT / "evals/papers"
RUN_TASKS = REPO_ROOT / "evals/openagents-desktop/tier2/run_tasks.py"
RESULTS_ROOT = REPO_ROOT / "evals/openagents-desktop/results/paper-lab"


def parse_kv(items: list[str]) -> dict[str, str]:
    out: dict[str, str] = {}
    for item in items:
        if "=" not in item:
            raise SystemExit(f"invalid env flag {item!r} (expected KEY=VAL)")
        k, v = item.split("=", 1)
        out[k.strip()] = v.strip()
    return out


def discover_probe_dirs(axes: list[str], explicit_tasks_dir: str) -> list[Path]:
    if explicit_tasks_dir:
        p = Path(explicit_tasks_dir)
        if not p.is_absolute():
            p = REPO_ROOT / p
        if not p.is_dir():
            raise SystemExit(f"--tasks-dir not found: {p}")
        return [p]
    dirs: list[Path] = []
    for axis in axes:
        td = PROBES_DIR / axis / "tasks"
        if not td.is_dir():
            avail = sorted(d.name for d in PROBES_DIR.iterdir() if d.is_dir()) if PROBES_DIR.is_dir() else []
            raise SystemExit(f"unknown probe axis {axis!r} (available: {', '.join(avail)})")
        dirs.append(td)
    return dirs


def run_arm(
    arm_name: str,
    model: str,
    tasks_dirs: list[Path],
    out_dir: Path,
    env_flags: dict[str, str],
    filter_str: str,
    workers: int,
    effort: str,
    timeout_floor: int,
    worker_bin: str,
    allow_dirty_host: bool,
) -> dict[str, Any]:
    """Run one arm across one or more probe task directories and merge summary.json."""
    out_dir.mkdir(parents=True, exist_ok=True)
    merged_cases: list[dict[str, Any]] = []
    merged_meta: dict[str, Any] = {}
    for idx, td in enumerate(tasks_dirs):
        sub_out = out_dir / (td.parent.name if len(tasks_dirs) > 1 else "run")
        cmd = [
            sys.executable,
            str(RUN_TASKS),
            "--tasks-dir",
            str(td),
            "--model",
            model,
            "--worker",
            worker_bin,
            "--out",
            str(sub_out),
            "-j",
            str(workers),
            "--no-subagents",
        ]
        if filter_str:
            cmd += ["--filter", filter_str]
        if effort:
            cmd += ["--effort", effort]
        if timeout_floor >= 0:
            cmd += ["--timeout-floor", str(timeout_floor)]
        if allow_dirty_host:
            cmd.append("--allow-dirty-host")
        for k, v in sorted(env_flags.items()):
            cmd += ["--env", f"{k}={v}"]

        print(f"\n=== [{arm_name}] model={model} tasks={td.parent.name} env={env_flags or '{}'} ===", flush=True)
        proc = subprocess.run(cmd, cwd=str(REPO_ROOT), check=False)
        summary_p = sub_out / "summary.json"
        if not summary_p.exists():
            raise RuntimeError(f"{arm_name} failed (exit {proc.returncode}) without writing {summary_p}")
        s = json.loads(summary_p.read_text())
        if not merged_meta:
            merged_meta = {
                "git_sha": s.get("git_sha", ""),
                "engine": s.get("engine", ""),
                "worker": s.get("worker", worker_bin),
                "models": s.get("models", [model]),
                "exp_env": env_flags,
            }
        for c in s.get("cases", []):
            if not c.get("not_applicable"):
                c["_axis"] = td.parent.name
                c["_case_dir"] = str(sub_out / "cases" / c["id"])
                merged_cases.append(c)

    app_n = len(merged_cases)
    passed_n = sum(1 for c in merged_cases if c.get("passed"))
    tot_in = sum(int((c.get("tokens") or {}).get("input") or 0) for c in merged_cases)
    tot_out = sum(int((c.get("tokens") or {}).get("output") or 0) for c in merged_cases)
    tot_cached = sum(int((c.get("tokens") or {}).get("cached") or 0) for c in merged_cases)
    tot_cost = sum(int(c.get("cost_usd_micros") or 0) for c in merged_cases)
    tot_wall = sum(int(c.get("wall_ms") or 0) for c in merged_cases)
    tot_turns = sum(int(c.get("turns") or 0) for c in merged_cases)
    tot_tools = sum(int(c.get("tool_calls") or 0) for c in merged_cases)
    summary = {
        **merged_meta,
        "arm": arm_name,
        "total": app_n,
        "passed": passed_n,
        "pass_rate_pct": round(100.0 * passed_n / app_n, 1) if app_n else 0.0,
        "total_input_tokens": tot_in,
        "total_output_tokens": tot_out,
        "total_cached_tokens": tot_cached,
        "cache_hit_pct": round(100.0 * tot_cached / tot_in, 1) if tot_in else 0.0,
        "cost_usd_micros": tot_cost,
        "wall_ms": tot_wall,
        "turns": tot_turns,
        "tool_calls": tot_tools,
        "cases": merged_cases,
    }
    (out_dir / "arm_summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    return summary


def aggregate_model_trials(
    model: str,
    base_trials: list[dict[str, Any]],
    exp_trials: list[dict[str, Any]],
) -> dict[str, Any]:
    """Compute paired 2x2 Rescued/Preserved/Damaged/StillFailed, Pass@1, Pass^k, and tax deltas."""
    k = len(base_trials)
    assert len(exp_trials) == k

    # Collect ordered task IDs
    task_ids: list[str] = []
    for c in base_trials[0]["cases"]:
        if c["id"] not in task_ids:
            task_ids.append(c["id"])

    rescued = 0
    preserved = 0
    damaged = 0
    still_failed = 0

    per_task: list[dict[str, Any]] = []
    for tid in task_ids:
        b_pass = []
        e_pass = []
        b_walls, e_walls = [], []
        b_toks, e_toks = [], []
        b_costs, e_costs = [], []
        axis = ""
        for t in range(k):
            bm = {c["id"]: c for c in base_trials[t]["cases"]}.get(tid)
            em = {c["id"]: c for c in exp_trials[t]["cases"]}.get(tid)
            if bm is None or em is None:
                continue
            axis = bm.get("_axis", "")
            bp = bool(bm.get("passed"))
            ep = bool(em.get("passed"))
            b_pass.append(bp)
            e_pass.append(ep)
            b_walls.append(int(bm.get("wall_ms") or 0))
            e_walls.append(int(em.get("wall_ms") or 0))
            b_toks.append(int((bm.get("tokens") or {}).get("input") or 0) + int((bm.get("tokens") or {}).get("output") or 0))
            e_toks.append(int((em.get("tokens") or {}).get("input") or 0) + int((em.get("tokens") or {}).get("output") or 0))
            b_costs.append(int(bm.get("cost_usd_micros") or 0))
            e_costs.append(int(em.get("cost_usd_micros") or 0))
            if not bp and ep:
                rescued += 1
            elif bp and ep:
                preserved += 1
            elif bp and not ep:
                damaged += 1
            else:
                still_failed += 1

        n_t = len(b_pass)
        per_task.append({
            "id": tid,
            "axis": axis,
            "trials": n_t,
            "base_passed": sum(b_pass),
            "exp_passed": sum(e_pass),
            "base_pass_all": all(b_pass) if b_pass else False,
            "exp_pass_all": all(e_pass) if e_pass else False,
            "delta_passes": sum(e_pass) - sum(b_pass),
            "base_wall_ms_avg": int(round(sum(b_walls) / n_t)) if n_t else 0,
            "exp_wall_ms_avg": int(round(sum(e_walls) / n_t)) if n_t else 0,
            "base_tokens_avg": int(round(sum(b_toks) / n_t)) if n_t else 0,
            "exp_tokens_avg": int(round(sum(e_toks) / n_t)) if n_t else 0,
            "base_cost_micros_avg": int(round(sum(b_costs) / n_t)) if n_t else 0,
            "exp_cost_micros_avg": int(round(sum(e_costs) / n_t)) if n_t else 0,
        })

    def sum_arm(trials: list[dict[str, Any]]) -> dict[str, Any]:
        tot_cases = sum(t["total"] for t in trials)
        tot_pass = sum(t["passed"] for t in trials)
        tot_in = sum(t["total_input_tokens"] for t in trials)
        tot_out = sum(t["total_output_tokens"] for t in trials)
        tot_cached = sum(t["total_cached_tokens"] for t in trials)
        tot_uncached = max(0, tot_in - tot_cached)
        tot_cost = sum(t["cost_usd_micros"] for t in trials)
        tot_wall = sum(t["wall_ms"] for t in trials)
        tot_turns = sum(t["turns"] for t in trials)
        tot_tools = sum(t["tool_calls"] for t in trials)
        # arXiv:2611.22238 Eq. 1 three-stream price vector: U + c*C + p*O (p=4)
        pv_c01 = round(tot_uncached + 0.1 * tot_cached + 4.0 * tot_out, 1)
        pv_c10 = round(tot_uncached + 1.0 * tot_cached + 4.0 * tot_out, 1)
        return {
            "total_trials_cases": tot_cases,
            "passed": tot_pass,
            "pass_at_1_pct": round(100.0 * tot_pass / tot_cases, 1) if tot_cases else 0.0,
            "input_tokens": tot_in,
            "uncached_input_tokens": tot_uncached,
            "output_tokens": tot_out,
            "total_tokens": tot_in + tot_out,
            "cached_tokens": tot_cached,
            "cache_hit_pct": round(100.0 * tot_cached / tot_in, 1) if tot_in else 0.0,
            "pv_cost_c01": pv_c01,
            "pv_cost_c10": pv_c10,
            "cost_usd_micros": tot_cost,
            "wall_ms": tot_wall,
            "turns": tot_turns,
            "tool_calls": tot_tools,
        }

    b_agg = sum_arm(base_trials)
    e_agg = sum_arm(exp_trials)
    b_agg["pass_pow_k_pct"] = round(100.0 * sum(1 for r in per_task if r["base_pass_all"]) / len(per_task), 1) if per_task else 0.0
    e_agg["pass_pow_k_pct"] = round(100.0 * sum(1 for r in per_task if r["exp_pass_all"]) / len(per_task), 1) if per_task else 0.0

    def pct_delta(b_val: float, e_val: float) -> float:
        if b_val <= 0:
            return 0.0
        return round(100.0 * (e_val - b_val) / b_val, 1)

    deltas = {
        "pass_at_1_pp": round(e_agg["pass_at_1_pct"] - b_agg["pass_at_1_pct"], 1),
        "pass_pow_k_pp": round(e_agg["pass_pow_k_pct"] - b_agg["pass_pow_k_pct"], 1),
        "cache_hit_pp": round(e_agg["cache_hit_pct"] - b_agg["cache_hit_pct"], 1),
        "tokens_pct": pct_delta(b_agg["total_tokens"], e_agg["total_tokens"]),
        "pv_c01_pct": pct_delta(b_agg["pv_cost_c01"], e_agg["pv_cost_c01"]),
        "pv_c10_pct": pct_delta(b_agg["pv_cost_c10"], e_agg["pv_cost_c10"]),
        "wall_pct": pct_delta(b_agg["wall_ms"], e_agg["wall_ms"]),
        "cost_pct": pct_delta(b_agg["cost_usd_micros"], e_agg["cost_usd_micros"]),
        "turns_delta": e_agg["turns"] - b_agg["turns"],
        "tool_calls_delta": e_agg["tool_calls"] - b_agg["tool_calls"],
    }

    return {
        "model": model,
        "trials": k,
        "paired": {
            "rescued": rescued,
            "preserved": preserved,
            "damaged": damaged,
            "still_failed": still_failed,
            "net_gain": rescued - damaged,
        },
        "base": b_agg,
        "exp": e_agg,
        "deltas": deltas,
        "tasks": per_task,
    }


def decide_verdict(model_reports: list[dict[str, Any]], max_wall_pct: float, max_cache_drop_pp: float) -> tuple[str, str]:
    """Deterministic decision rule across single or dual (local + cloud) model evaluations."""
    local_r = [r for r in model_reports if r["model"] in ("inkling-2-mini", "inkling", "hybrid-exec-v1")]
    cloud_r = [r for r in model_reports if r not in local_r]

    def model_wins(r: dict[str, Any]) -> bool:
        p = r["paired"]
        d = r["deltas"]
        return p["net_gain"] > 0 and p["damaged"] == 0 and d["wall_pct"] <= max_wall_pct and d["cache_hit_pp"] >= -max_cache_drop_pp

    def model_efficiency_wins(r: dict[str, Any]) -> bool:
        p = r["paired"]
        d = r["deltas"]
        return (
            p["damaged"] == 0
            and p["net_gain"] >= 0
            and r["exp"]["passed"] > 0
            and (d["tokens_pct"] <= -15.0 or d["cost_pct"] <= -15.0)
            and d["cost_pct"] <= 0.0
            and d["wall_pct"] <= max_wall_pct
        )

    def model_mixed(r: dict[str, Any]) -> bool:
        p = r["paired"]
        return p["net_gain"] > 0 and not model_wins(r)

    # Efficiency win check (arXiv:2610.04433 + arXiv:2508.21433): same/higher pass rate with >=15% token/cost reduction
    if all(model_efficiency_wins(r) for r in model_reports):
        r0 = model_reports[0]
        return "SHIP-EFFICIENCY", (
            f"Preserved pass rate ({r0['exp']['passed']}/{r0['exp']['total_trials_cases']}, 0 damaged) "
            f"while cutting tokens {r0['deltas']['tokens_pct']:+.1f}% and cost {r0['deltas']['cost_pct']:+.1f}%."
        )

    # Check if baseline saturated >= 85% across all models with no rescues AND no regressions (probes too easy)
    if (
        all(r["base"]["pass_at_1_pct"] >= 85.0 for r in model_reports)
        and all(r["paired"]["rescued"] == 0 and r["paired"]["damaged"] == 0 for r in model_reports)
    ):
        return "NEEDS-HARDER-PROBES", "Baseline already passed >=85% with 0 rescues; harden probes before concluding."

    if local_r and cloud_r:
        lw = all(model_wins(r) for r in local_r)
        lw_eff = all(model_wins(r) or model_efficiency_wins(r) for r in local_r)
        cw = all(model_wins(r) for r in cloud_r)
        if lw and cw:
            return "SHIP", "Positive net rescues with zero damaged cases and acceptable latency/cache tax on both local and cloud models."
        if lw_eff and not cw:
            return "LOCAL-ONLY", "Clean accuracy or efficiency win on local 32K Inkling without net benefit on cloud; gate behind local/small-context mode."
        if cw and not lw:
            return "ITERATE", "Clean win on cloud model but not yet on local 32K Inkling; inspect damaged/failed local traces."
        if any(model_mixed(r) for r in model_reports):
            return "ITERATE", "Net rescues observed, but damaged cases or latency/cache tax exceeded ceiling; refine mechanism."
        return "KILL", "No net improvement across local or cloud models."

    r = model_reports[0]
    if model_wins(r):
        return "SHIP", f"Clean win (+{r['paired']['rescued']} rescued, 0 damaged, wall {r['deltas']['wall_pct']:+.1f}%, cache {r['deltas']['cache_hit_pp']:+.1f} pp)."
    if model_mixed(r):
        return "ITERATE", f"Net gain (+{r['paired']['rescued']} rescued, {r['paired']['damaged']} damaged, wall {r['deltas']['wall_pct']:+.1f}%, cache {r['deltas']['cache_hit_pp']:+.1f} pp) — inspect wire diffs."
    return "KILL", f"No net gain (+{r['paired']['rescued']} rescued, {r['paired']['damaged']} damaged)."


def usd(micros: int) -> str:
    return f"${micros / 1e6:.4f}"


def render_verdict_md(record: dict[str, Any]) -> str:
    h = record["hypothesis"]
    lines = [
        f"# Paper-Lab Verdict: {h['name']} (`arXiv:{h['paper_id']}`)",
        "",
        f"- **Verdict:** **`{record['verdict']}`** — {record['verdict_reason']}",
        f"- **Claim:** {h['claim']}",
        f"- **Axes:** `{', '.join(h['axes'])}`",
        f"- **Base Env:** `{json.dumps(h['base_env'])}`",
        f"- **Exp Env:** `{json.dumps(h['exp_env'])}`",
        f"- **Git SHA:** `{record['git_sha']}` | **Date (UTC):** `{record['created']}`",
        "",
    ]
    for mr in record["models"]:
        p = mr["paired"]
        b = mr["base"]
        e = mr["exp"]
        d = mr["deltas"]
        lines += [
            f"## Model: `{mr['model']}` (`k={mr['trials']}` trial(s)/probe)",
            "",
            f"**Paired 2×2 Matrix:** **Rescued `{p['rescued']}`** | Preserved `{p['preserved']}` | **Damaged `{p['damaged']}`** | Still Failed `{p['still_failed']}` (Net `{p['net_gain']:+d}`)",
            "",
            "| Metric | BASE | EXP | Delta |",
            "|---|---|---|---|",
            f"| Pass@1 | {b['passed']}/{b['total_trials_cases']} ({b['pass_at_1_pct']:.1f}%) | {e['passed']}/{e['total_trials_cases']} ({e['pass_at_1_pct']:.1f}%) | **{d['pass_at_1_pp']:+.1f} pp** |",
            f"| Pass^{mr['trials']} (all trials) | {b['pass_pow_k_pct']:.1f}% | {e['pass_pow_k_pct']:.1f}% | {d['pass_pow_k_pp']:+.1f} pp |",
            f"| Prompt Cache Hit Rate | {b['cache_hit_pct']:.1f}% ({b['cached_tokens']}/{b['input_tokens']}) | {e['cache_hit_pct']:.1f}% ({e['cached_tokens']}/{e['input_tokens']}) | {d['cache_hit_pp']:+.1f} pp |",
            f"| Price-Vector (c=0.1 cloud) | {b['pv_cost_c01']:,.0f} u | {e['pv_cost_c01']:,.0f} u | {d['pv_c01_pct']:+.1f}% |",
            f"| Price-Vector (c=1.0 local) | {b['pv_cost_c10']:,.0f} u | {e['pv_cost_c10']:,.0f} u | {d['pv_c10_pct']:+.1f}% |",
            f"| Total Tokens (in+out) | {b['total_tokens']:,} ({b['input_tokens']:,}in/{b['output_tokens']:,}out) | {e['total_tokens']:,} ({e['input_tokens']:,}in/{e['output_tokens']:,}out) | {d['tokens_pct']:+.1f}% |",
            f"| Wall Time (sum) | {b['wall_ms'] / 1000:.1f} s | {e['wall_ms'] / 1000:.1f} s | {d['wall_pct']:+.1f}% |",
            f"| Cost (USD) | {usd(b['cost_usd_micros'])} | {usd(e['cost_usd_micros'])} | {d['cost_pct']:+.1f}% |",
            f"| Cost / Solved Task | {usd(b['cost_usd_micros'] // b['passed']) if b['passed'] else 'n/a'} | {usd(e['cost_usd_micros'] // e['passed']) if e['passed'] else 'n/a'} | — |",
            f"| Turns / Tool Calls | {b['turns']} / {b['tool_calls']} | {e['turns']} / {e['tool_calls']} | {d['turns_delta']:+d} / {d['tool_calls_delta']:+d} |",
            "",
            "### Per-Probe Breakdown",
            "",
            "| Probe | Axis | BASE | EXP | Delta | BASE wall | EXP wall |",
            "|---|---|---|---|---|---|---|",
        ]
        for t in mr["tasks"]:
            lines.append(
                f"| `{t['id']}` | `{t['axis']}` | {t['base_passed']}/{t['trials']} | {t['exp_passed']}/{t['trials']} | "
                f"{t['delta_passes']:+d} | {t['base_wall_ms_avg'] / 1000:.1f} s | {t['exp_wall_ms_avg'] / 1000:.1f} s |"
            )
        lines.append("")
    return "\n".join(lines)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--paper", required=True, help="arXiv ID or experiment slug (e.g. 2610.04838)")
    ap.add_argument("--name", default="", help="Human-readable mechanism name")
    ap.add_argument("--claim", default="", help="One-sentence falsifiable claim")
    ap.add_argument("--axis", default="compaction", help="Comma-separated probe axes under evals/paper-lab/probes/")
    ap.add_argument("--tasks-dir", default="", help="Explicit tasks directory (overrides --axis)")
    ap.add_argument("--filter", default="", help="Comma-separated task ID substring filter")
    ap.add_argument("--models", default="gemini-4-neon", help="Comma-separated model IDs (e.g. inkling-2-mini,gemini-4-neon)")
    ap.add_argument("--trials", type=int, default=1, help="Paired trials per probe (default 1; recommend 3 for ship decisions)")
    ap.add_argument("--base-env", action="append", default=[], help="Control arm env KEY=VAL (repeatable)")
    ap.add_argument("--exp-env", action="append", default=[], help="Experiment arm env KEY=VAL (repeatable)")
    ap.add_argument("--worker", default="oa-worker", choices=["oa-worker", "mcworker", "inky"])
    ap.add_argument("--effort", default="medium", help="Reasoning effort override (default medium for fast iteration)")
    ap.add_argument("--timeout-floor", type=int, default=-1)
    ap.add_argument("-j", "--workers", type=int, default=3, help="Parallel task workers for cloud runs (default 3)")
    ap.add_argument("--cache-isolated", action="store_true", help="Force serial task execution (-j 1) to prevent sibling prefix-cache cross-warming (arXiv:2611.22238 Sec. 2)")
    ap.add_argument("--max-wall-pct", type=float, default=35.0, help="Max allowed wall-time regression pct for SHIP")
    ap.add_argument("--max-cache-drop-pp", type=float, default=10.0, help="Max allowed cache-hit drop in percentage points for SHIP")
    ap.add_argument("--calibrate-base", action="store_true", help="Run ONLY BASE arm to verify probe difficulty band (20-70 pct)")
    ap.add_argument("--allow-dirty-host", action="store_true")
    ap.add_argument("--merge-verdict", action="store_true", help="Merge new model reports into existing evals/papers/<paper>/verdict.json")
    ap.add_argument("--no-save-verdict", action="store_true", help="Do not write evals/papers/<paper>/verdict.{json,md}")
    args = ap.parse_args()
    if args.cache_isolated:
        args.workers = 1

    axes = [a.strip() for a in args.axis.split(",") if a.strip()]
    models = [m.strip() for m in args.models.split(",") if m.strip()]
    tasks_dirs = discover_probe_dirs(axes, args.tasks_dir)
    base_env = parse_kv(args.base_env)
    exp_env = parse_kv(args.exp_env)

    ts = time.strftime("%Y%m%dT%H%M%SZ", time.gmtime())
    sha = subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=str(REPO_ROOT), capture_output=True, text=True).stdout.strip() or "nogit"
    run_root = RESULTS_ROOT / f"{args.paper}-{ts}-{sha}"
    run_root.mkdir(parents=True, exist_ok=True)

    hypothesis = {
        "paper_id": args.paper,
        "name": args.name or f"Experiment {args.paper}",
        "claim": args.claim or "(no claim specified)",
        "axes": axes if not args.tasks_dir else [args.tasks_dir],
        "base_env": base_env,
        "exp_env": exp_env,
        "models": models,
        "trials": args.trials,
        "effort": args.effort,
        "worker": args.worker,
    }

    if args.calibrate_base:
        for model in models:
            s = run_arm(
                "BASE-CALIBRATE", model, tasks_dirs, run_root / f"{model}-base-cal",
                base_env, args.filter, args.workers, args.effort, args.timeout_floor, args.worker, args.allow_dirty_host,
            )
            print(f"\n[CALIBRATE] model={model} BASE pass={s['passed']}/{s['total']} ({s['pass_rate_pct']:.1f}%) "
                  f"cache_hit={s['cache_hit_pct']:.1f}% cost={usd(s['cost_usd_micros'])}")
        return 0

    model_reports: list[dict[str, Any]] = []
    for model in models:
        base_trials: list[dict[str, Any]] = []
        exp_trials: list[dict[str, Any]] = []
        for t in range(1, args.trials + 1):
            b_sum = run_arm(
                f"BASE-t{t}", model, tasks_dirs, run_root / f"{model}-t{t}-base",
                base_env, args.filter, args.workers, args.effort, args.timeout_floor, args.worker, args.allow_dirty_host,
            )
            e_sum = run_arm(
                f"EXP-t{t}", model, tasks_dirs, run_root / f"{model}-t{t}-exp",
                exp_env, args.filter, args.workers, args.effort, args.timeout_floor, args.worker, args.allow_dirty_host,
            )
            base_trials.append(b_sum)
            exp_trials.append(e_sum)
        model_reports.append(aggregate_model_trials(model, base_trials, exp_trials))

    paper_dir = PAPERS_OUT_DIR / args.paper
    existing_verdict_p = paper_dir / "verdict.json"
    if args.merge_verdict and existing_verdict_p.exists():
        prev = json.loads(existing_verdict_p.read_text())
        new_models_set = {m["model"] for m in model_reports}
        kept = [m for m in prev.get("models", []) if m.get("model") not in new_models_set]
        model_reports = kept + model_reports
        hypothesis["models"] = [m["model"] for m in model_reports]

    verdict, reason = decide_verdict(model_reports, args.max_wall_pct, args.max_cache_drop_pp)
    record = {
        "created": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "git_sha": sha,
        "run_dir": str(run_root),
        "verdict": verdict,
        "verdict_reason": reason,
        "hypothesis": hypothesis,
        "models": model_reports,
    }
    md = render_verdict_md(record)
    (run_root / "verdict.json").write_text(json.dumps(record, indent=2) + "\n")
    (run_root / "verdict.md").write_text(md)

    if not args.no_save_verdict:
        paper_dir.mkdir(parents=True, exist_ok=True)
        (paper_dir / "hypothesis.json").write_text(json.dumps(hypothesis, indent=2) + "\n")
        (paper_dir / "verdict.json").write_text(json.dumps(record, indent=2) + "\n")
        (paper_dir / "verdict.md").write_text(md)

    print("\n" + md)
    return 0


if __name__ == "__main__":
    sys.exit(main())
