#!/usr/bin/env python3
"""Tables for the file-relevance bench (scripts/bench/clef-relevance-bench.py).

    python3 -I scripts/bench/clef-relevance-report.py R1.jsonl [R2.jsonl ...]

Prints a throughput table (one row per run) and a quality table (labeled
decisions only, threshold 0.5) per backend and mode, plus calibration bins
and the open-issue (unlabeled) picks. Reads only the result files, which hold
paths, labels, probabilities and usage, never file contents.
"""
import json, sys
from collections import defaultdict

JEV_USD_PER_MTOK = 0.042  # crates/jev/src/nip_dec.rs USD_PER_MILLION_INPUT (TypeSafe bills input tokens)

runs = []
for path in sys.argv[1:]:
    for line in open(path):
        runs.append(json.loads(line))


def f(v, d=2):
    return "—" if v is None else (f"{v:.{d}f}" if isinstance(v, float) else str(v))


print("| Backend | Mode | Note | Load (1m before→after) | OK/req | Decisions/s | p50 s | p90 s | p99 s | Input tok/req | Output tok | Prefill tok/s (median) | Jev $/1k decisions |")
print("|---|---|---|---|---|---|---|---|---|---|---|---|---|")
for r in runs:
    s = r["summary"]
    cost = None
    if s["backend"] == "jev" and s["input_tokens_mean"] and s["decisions"]:
        cost = s["input_tokens_mean"] * s["ok"] / s["decisions"] * JEV_USD_PER_MTOK / 1e6 * 1000
    lb = s.get("load_before") or [None]
    la = s.get("load_after") or [None]
    mode = s["mode"] + (f" c={s['conc']}" if s["mode"] == "conc" else "")
    print(f"| {s['backend']} | {mode} | {s.get('note','')} | {f(lb[0],0)}→{f(la[0],0)} | {s['ok']}/{s['requests']} | "
          f"**{f(s['decisions_per_s'])}** | {f(s['p50_s'])} | {f(s['p90_s'])} | {f(s['p99_s'])} | {f(s['input_tokens_mean'],0)} | "
          f"{f(s['output_tokens_mean'],0)} | {f(s['prefill_tok_s_median'],0)} | {f(cost,3)} |")
    if s["errors"]:
        print(f"|  | errors: {'; '.join(e[:120] for e in s['errors'])} |||||||||||")

# quality: pool decisions per (backend, mode-family)
pool = defaultdict(list)
opens = defaultdict(dict)
for r in runs:
    s = r["summary"]
    fam = "batch" if s["mode"] == "batch" else "per-file"
    for row in r["rows"]:
        for d in row["decisions"]:
            if d["p"] is None:
                continue
            if d["label"] is None:
                opens[(s["backend"], fam)].setdefault((row["issue"], d["path"]), []).append(d["p"])
            else:
                pool[(s["backend"], fam)].append((d["label"], d["p"], row["issue"], d["path"]))

# one decision per (issue, file): the mean p over repeated runs; spread = mean |p - mean|
spread = {}
for key, xs in list(pool.items()):
    g = defaultdict(list)
    for l, p, iss, path in xs:
        g[(iss, path, l)].append(p)
    spread[key] = sum(sum(abs(p - sum(ps) / len(ps)) for p in ps) / len(ps) for ps in g.values()) / len(g)
    pool[key] = [(l, sum(ps) / len(ps), iss, path) for (iss, path, l), ps in g.items()]

print()
print("| Backend | Prompt | Labeled decisions | Precision | Recall | F1 | Accuracy | Mean p (relevant) | Mean p (not) | Brier | AUC | p spread across repeats |")
print("|---|---|---|---|---|---|---|---|---|---|---|---|")
calib = {}
for (b, fam), xs in sorted(pool.items()):
    tp = sum(1 for l, p, *_ in xs if l and p >= .5)
    fp = sum(1 for l, p, *_ in xs if not l and p >= .5)
    fn = sum(1 for l, p, *_ in xs if l and p < .5)
    tn = sum(1 for l, p, *_ in xs if not l and p < .5)
    prec = tp / (tp + fp) if tp + fp else None
    rec = tp / (tp + fn) if tp + fn else None
    f1 = 2 * prec * rec / (prec + rec) if prec and rec else None
    acc = (tp + tn) / len(xs)
    mp = [p for l, p, *_ in xs if l]
    mn = [p for l, p, *_ in xs if not l]
    brier = sum((p - (1 if l else 0)) ** 2 for l, p, *_ in xs) / len(xs)
    auc = sum((a > c) + 0.5 * (a == c) for a in mp for c in mn) / (len(mp) * len(mn)) if mp and mn else None
    print(f"| {b} | {fam} | {len(xs)} | {f(prec)} | {f(rec)} | {f(f1)} | {f(acc)} | {f(sum(mp)/len(mp))} | {f(sum(mn)/len(mn))} | {f(brier,3)} | {f(auc)} | {f(spread[(b, fam)],3)} |")
    bins = defaultdict(lambda: [0, 0])
    for l, p, *_ in xs:
        k = min(4, int(p * 5))
        bins[k][0] += 1
        bins[k][1] += 1 if l else 0
    calib[(b, fam)] = bins

print()
print("Calibration (share actually relevant per probability bin, n in brackets):")
print()
print("| Backend | Prompt | 0–0.2 | 0.2–0.4 | 0.4–0.6 | 0.6–0.8 | 0.8–1.0 |")
print("|---|---|---|---|---|---|---|")
for (b, fam), bins in sorted(calib.items()):
    cells = []
    for k in range(5):
        n, y = bins.get(k, [0, 0])
        cells.append(f"{y/n:.2f} ({n})" if n else "— (0)")
    print(f"| {b} | {fam} | " + " | ".join(cells) + " |")

print()
print("Open issues (unlabeled), mean p per file:")
for (b, fam), d in sorted(opens.items()):
    print(f"\n{b} {fam}:")
    for (iss, path), ps in sorted(d.items()):
        m = sum(ps) / len(ps)
        print(f"  #{iss} {path} {m:.2f}{' *' if m >= .5 else ''}")

# pairwise answer comparison between backends (all decisions, labeled and open)
allp = defaultdict(lambda: defaultdict(list))
for r in runs:
    s = r["summary"]
    fam = "batch" if s["mode"] == "batch" else "per-file"
    for row in r["rows"]:
        for d in row["decisions"]:
            if d["p"] is not None:
                allp[(s["backend"], fam)][(row["issue"], d["path"])].append(d["p"])
means = {k: {kk: sum(v) / len(v) for kk, v in d.items()} for k, d in allp.items()}
print()
print("Pairwise answers, same prompt form (median and max |Δp|, share on the same side of 0.5):")
print()
print("| A | B | Prompt | n | median abs dp | max abs dp | same side of 0.5 |")
print("|---|---|---|---|---|---|---|")
keys = sorted(means)
for i, a in enumerate(keys):
    for b in keys[i + 1:]:
        if a[1] != b[1]:
            continue
        common = sorted(set(means[a]) & set(means[b]))
        if not common:
            continue
        ds = sorted(abs(means[a][k] - means[b][k]) for k in common)
        same = sum((means[a][k] >= .5) == (means[b][k] >= .5) for k in common) / len(common)
        print(f"| {a[0]} | {b[0]} | {a[1]} | {len(common)} | {ds[len(ds)//2]:.3f} | {ds[-1]:.3f} | {same:.2f} |")
