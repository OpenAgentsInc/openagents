#!/usr/bin/env python3
# Firm reason: 6 (Harbor job JSON report summarizer for Terminal-Bench 4.1 comparison tables)
"""Summarize and compare Harbor Terminal-Bench 4.1 job directories."""
import json
import sys
from pathlib import Path


def parse_job(job_dir: Path) -> dict:
    trials = {}
    for td in sorted(job_dir.iterdir()):
        if not td.is_dir():
            continue
        res_p = td / "result.json"
        if not res_p.is_file():
            continue
        try:
            r = json.loads(res_p.read_text(encoding="utf-8", errors="replace"))
        except Exception:
            continue
        task = r.get("task_name") or td.name.split("__")[0]
        vr = r.get("verifier_result") or {}
        rewards = vr.get("rewards") or {}
        rew = rewards.get("reward", 0.0)
        if rew is None:
            rew = 0.0
        ae = r.get("agent_execution") or {}
        t0 = ae.get("started_at")
        t1 = ae.get("finished_at")
        dur_s = None
        if t0 and t1:
            from datetime import datetime
            try:
                dur_s = (datetime.fromisoformat(t1.replace("Z", "+00:00")) - datetime.fromisoformat(t0.replace("Z", "+00:00"))).total_seconds()
            except Exception:
                pass
        ar = r.get("agent_result") or {}
        exc = r.get("exception_info")
        exc_type = exc.get("exception_type") if isinstance(exc, dict) else None
        trials[task] = {
            "reward": float(rew),
            "dur_s": dur_s,
            "in_tok": ar.get("n_input_tokens"),
            "out_tok": ar.get("n_output_tokens"),
            "cache_tok": ar.get("n_cache_tokens"),
            "cost": ar.get("cost_usd"),
            "meta": ar.get("metadata") or {},
            "exc": exc_type,
            "dir": str(td),
        }
    return trials


def main() -> None:
    if len(sys.argv) < 2:
        print("usage: summarize_tb41.py <job_dir> [job_dir2 ...]")
        sys.exit(2)
    jobs = [(Path(p).name, parse_job(Path(p))) for p in sys.argv[1:]]
    all_tasks = sorted({t for _, j in jobs for t in j})
    hdr = f"{'TASK':30s}" + "".join(f" | {name[:24]:24s}" for name, _ in jobs)
    print(hdr)
    print("-" * len(hdr))
    totals = [[0.0, 0, 0.0] for _ in jobs]
    for t in all_tasks:
        row = f"{t:30s}"
        for idx, (_, j) in enumerate(jobs):
            info = j.get(t)
            if not info:
                row += f" | {'-':24s}"
                continue
            rew = info["reward"]
            totals[idx][0] += rew
            totals[idx][1] += 1
            if info["dur_s"] is not None:
                totals[idx][2] += info["dur_s"]
            status = "PASS" if rew >= 0.99 else (f"P({rew:.2f})" if rew > 0 else "FAIL")
            if info["exc"]:
                status += f"({info['exc'][:6]})"
            ds = f"{info['dur_s']:.0f}s" if info["dur_s"] is not None else "?s"
            in_k = f"{(info['in_tok'] or 0)/1000:.0f}k" if info["in_tok"] else "-"
            cell = f"{status} {ds} {in_k}"
            row += f" | {cell:24s}"
        print(row)
    print("-" * len(hdr))
    tot_row = f"{'TOTAL':30s}"
    for idx, (name, _) in enumerate(jobs):
        s, n, d = totals[idx]
        pct = (s / n * 100.0) if n else 0.0
        cell = f"{s:.1f}/{n} ({pct:.1f}%) {d:.0f}s"
        tot_row += f" | {cell:24s}"
    print(tot_row)


if __name__ == "__main__":
    main()
