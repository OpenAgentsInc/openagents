#!/usr/bin/env python3
"""Aggregate the ollama coding bench JSONL into a scorecard.

Reads bench-results/ollama-coding-<date>.jsonl (one row per model/prompt/run,
written by scripts/bench-ollama-coding.sh) and prints, per model:

  - median TTFT, prompt t/s (prefill), gen t/s, per prompt class
  - an agent_score = 0.45*norm(pp) + 0.35*norm(gen) + 0.20*norm(1/ttft)
    computed across the models that produced numbers (§1.4 of the plan doc)
  - gate results: codegen compiled, JSON parseable

Gates are checked from each model's output_preview: the codegen answer must
contain `fn parse_kv` and a `Result` return, the json answer must parse as a
JSON object with the three expected keys. A gate failure does not change the
speed score; it disqualifies the model from being the recommendation.
"""
import json
import re
import statistics
import sys
from collections import defaultdict
from pathlib import Path

RESULTS = Path(__file__).parent.parent / "bench-results"
jsonl = sys.argv[1] if len(sys.argv) > 1 else sorted(RESULTS.glob("ollama-coding-*.jsonl"))[-1]

rows = []
for line in open(jsonl):
    line = line.strip()
    if line:
        rows.append(json.loads(line))

ok_rows = [r for r in rows if "error" not in r and "skipped" not in r]
errors = {(r["model"], r.get("error", "")[:80]) for r in rows if "error" in r}
skipped = {r["model"] for r in rows if "skipped" in r}

# ---- per model / prompt medians -------------------------------------------
# Median across runs, then per prompt class. prompt t/s uses the median of the
# per-run t/s values, not a ratio of medians.
per = defaultdict(list)  # (model, prompt) -> [row]
for r in ok_rows:
    per[(r["model"], r["prompt"])].append(r)

def med(vals):
    vals = [v for v in vals if v is not None]
    return statistics.median(vals) if vals else None

model_prompt = {}
for (m, p), rs in per.items():
    model_prompt[(m, p)] = {
        "ttft": med([r["ttft_s"] for r in rs]),
        "pp": med([r["prompt_tps"] for r in rs]),
        "gen": med([r["gen_tps"] for r in rs]),
        "out": med([r["gen_tokens"] for r in rs]),
        "wall": med([r["wall_s"] for r in rs]),
    }

models = sorted({m for (m, _p) in model_prompt})

# ---- gates -----------------------------------------------------------------
def gate_results(m):
    out = {}
    cg = [r for r in ok_rows if r["model"] == m and r["prompt"] == "codegen"]
    js = [r for r in ok_rows if r["model"] == m and r["prompt"] == "json"]
    # codegen gate: emitted a plausible parse_kv (fn + Result) — a real verdict
    # needs rustc, but every candidate runs the same prompt so the shape check
    # is comparable. The top pick gets a full rustc check in the report.
    out["codegen"] = None
    if cg:
        text = " ".join(str(r.get("output_preview", "")) for r in cg)
        out["codegen"] = bool(re.search(r"fn\s+parse_kv", text)) and "Result" in text
    out["json"] = None
    if js:
        parsed = 0
        for r in js:
            t = r.get("output_preview", "")
            mm = re.search(r"\{.*\}", t, re.S)
            if mm:
                try:
                    d = json.loads(mm.group(0))
                    if {"file", "line", "severity"} <= set(d):
                        parsed += 1
                except Exception:
                    pass
        out["json"] = parsed, len(js)
    return out

gates = {m: gate_results(m) for m in models}

# ---- score -----------------------------------------------------------------
# Agent-weighted: prefill dominates agentic latency (long tool results),
# generation is the streaming experience, TTFT the short-turn feel.
PROMPTS_FOR_SCORE = ["short", "codegen", "longctx", "json"]

def model_metric(m, key):
    vals = [model_prompt[(m, p)][key] for p in PROMPTS_FOR_SCORE if (m, p) in model_prompt]
    vals = [v for v in vals if v is not None]
    if not vals:
        return None
    if key == "ttft":  # lower better: invert
        return 1.0 / statistics.mean(vals)
    return statistics.mean(vals)

metrics = {m: {k: model_metric(m, k) for k in ("pp", "gen", "ttft")} for m in models}

def norm(models, m, key):
    have = [(mm, metrics[mm][key]) for mm in models if metrics[mm][key] is not None]
    if len(have) < 2 or metrics[m][key] is None:
        return None
    lo = min(v for _, v in have)
    hi = max(v for _, v in have)
    if hi == lo:
        return 1.0
    return (metrics[m][key] - lo) / (hi - lo)

scores = {}
for m in models:
    n_pp = norm(models, m, "pp")
    n_gen = norm(models, m, "gen")
    n_ttft = norm(models, m, "ttft")
    parts = [v for v in (n_pp, n_gen, n_ttft) if v is not None]
    scores[m] = round(0.45 * (n_pp or 0) + 0.35 * (n_gen or 0) + 0.20 * (n_ttft or 0), 3) if parts else None

# ---- report ----------------------------------------------------------------
def fmt(v, nd=1, suffix=""):
    return f"{v:{nd}.1f}{suffix}" if v is not None else "  —  "

print(f"source: {jsonl}")
print(f"models measured: {len(models)}  errors: {len(errors)}  skipped: {len(skipped)}")
for m, e in sorted(errors):
    print(f"  ERROR {m}: {e}")
for m in sorted(skipped):
    print(f"  SKIP  {m}: not pulled")
print()
hdr = (f"{'model':40s} {'score':>5s} | {'pp t/s':>9s} {'gen t/s':>8s} {'ttft s':>7s} | "
       f"{'lc-pp':>8s} {'lc-ttft':>8s} | {'cg':>3s} {'json':>5s}")
print(hdr)
print("-" * len(hdr))
for m in sorted(models, key=lambda x: -(scores[x] or 0)):
    mp = model_prompt
    g = gates[m]
    cg = "✓" if g["codegen"] else ("✗" if g["codegen"] is False else "—")
    js = g["json"]
    jss = f"{js[0]}/{js[1]}" if js else "—"
    gen_mean = metrics[m]["gen"]
    pp_mean = metrics[m]["pp"]
    ttft_mean = med([model_prompt[(m, p)]["ttft"] for p in PROMPTS_FOR_SCORE if (m, p) in model_prompt])
    lc = model_prompt.get((m, "longctx"), {})
    print(f"{m:40s} {scores[m] or 0:5.3f} | {fmt(pp_mean,9,1):>9s} {fmt(gen_mean,8,1):>8s} "
          f"{fmt(ttft_mean,7,2):>7s} | {fmt(lc.get('pp'),8,1):>8s} {fmt(lc.get('ttft'),8,2):>8s} | "
          f"{cg:>3s} {jss:>5s}")
print()
print("per-prompt medians (pp t/s / gen t/s / ttft s):")
for m in models:
    parts = []
    for p in PROMPTS_FOR_SCORE:
        d = model_prompt.get((m, p))
        parts.append(f"{p}: {fmt(d['pp'])}/{fmt(d['gen'])}/{fmt(d['ttft'],2)}" if d else f"{p}: —")
    print(f"  {m:40s} " + "  ".join(parts))
