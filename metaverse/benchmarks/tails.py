#!/usr/bin/env python3
"""Check R0 frame captures (Documents/R0-frame.json, optionally gzipped)
against the written tail budget in frame-budget.json.

For each metric it recomputes median, p95, p99, max, coefficient of
variation and median absolute deviation from the raw samples, never from the
capture's own summary. Outcomes per metric: pass, over-budget, not-measured
(absent or too few samples). The overall verdict is device-pass only for a
physical device capture with every metric passing; a Simulator capture is at
best simulator-only. Missing execution is never a pass.
"""
import argparse, gzip, hashlib, json, statistics, sys
from pathlib import Path

BASE = Path(__file__).resolve().parent


def load(path):
    raw = Path(path).read_bytes()
    return json.loads(gzip.decompress(raw) if raw[:2] == b"\x1f\x8b" else raw), hashlib.sha256(raw).hexdigest()


def rank(samples, q):
    return samples[(len(samples) * q + 99) // 100 - 1]


def stats(samples):
    s = sorted(samples)
    n = len(s)
    median = s[n // 2] if n % 2 else (s[n // 2 - 1] + s[n // 2]) / 2
    mean = statistics.fmean(s)
    return {"count": n, "median_ns": median, "p95_ns": rank(s, 95), "p99_ns": rank(s, 99), "max_ns": s[-1],
            "cv": statistics.pstdev(s) / mean if mean else None,
            "mad_ns": statistics.median(abs(x - median) for x in s)}


def check(capture, budget):
    out = {}
    for name, limit in budget["metrics"].items():
        metric = capture.get("metrics", {}).get(name)
        samples = metric.get("samples_ns") if metric else None
        if not samples or len(samples) < budget["minimum_samples"]:
            out[name] = {"outcome": "not-measured", "count": len(samples or [])}
            continue
        row = stats(samples)
        over = [k for k in ("p99_ns", "max_ns") if row[k] > limit[k]]
        row["outcome"] = "over-budget" if over else "pass"
        if over:
            row["over"] = over
        out[name] = row
    environment = str(capture.get("environment", ""))
    device = bool(environment) and "simulator" not in environment.lower()
    healthy = capture.get("gpu_errors") == 0 and capture.get("completed_commands") == capture.get("requested_samples")
    outcomes = {r["outcome"] for r in out.values()}
    if not healthy:
        verdict = "infrastructure-error"
    elif "over-budget" in outcomes:
        verdict = "over-budget"
    elif "not-measured" in outcomes:
        verdict = "incomplete"
    else:
        verdict = "device-pass" if device else "simulator-only"
    return {"environment": environment, "physical_device": device, "healthy": healthy, "verdict": verdict, "metrics": out}


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("captures", nargs="+")
    p.add_argument("--budget", default=BASE / "frame-budget.json")
    p.add_argument("--output")
    p.add_argument("--enforce", action="store_true", help="exit 1 unless every capture is device-pass")
    a = p.parse_args(argv)
    budget = json.loads(Path(a.budget).read_text())
    runs = []
    for path in a.captures:
        capture, digest = load(path)
        r = check(capture, budget)
        r.update(capture=str(path), capture_sha256=digest, provenance=capture.get("capture_provenance"))
        runs.append(r)
    # Dispersion across repeated runs: the spread of each metric's p99.
    across = {}
    for name in budget["metrics"]:
        p99 = [r["metrics"][name]["p99_ns"] for r in runs if "p99_ns" in r["metrics"][name]]
        if len(p99) >= 2:
            across[name] = {"runs": len(p99), "p99_min_ns": min(p99), "p99_max_ns": max(p99), "p99_median_ns": statistics.median(p99)}
    report = {"schema": "coax.r0.frame-tails.v1", "budget": budget, "runs": runs, "across_runs": across}
    text = json.dumps(report, indent=2) + "\n"
    if a.output:
        Path(a.output).write_text(text)
    for r in runs:
        print(f"{r['capture']}: {r['verdict']} ({r['environment']})")
        for name, m in r["metrics"].items():
            detail = f"p99 {m['p99_ns']/1000:.1f} us, max {m['max_ns']/1000:.1f} us, cv {m['cv']:.3f}" if "p99_ns" in m else f"{m['count']} samples"
            print(f"  {name}: {m['outcome']}; {detail}")
    if a.enforce and any(r["verdict"] != "device-pass" for r in runs):
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
