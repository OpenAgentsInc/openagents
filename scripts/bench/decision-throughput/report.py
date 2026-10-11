#!/usr/bin/env python3
"""Turn decision-throughput JSONL into the tables of
docs/inference/decision-throughput.md.

    python3 -I report.py --gcs gs://openagentsgemini-decision-throughput/runs --doc docs/inference/decision-throughput.md
    python3 -I report.py results/*.jsonl            # print the tables

Each table goes between `<!-- dt:NAME:start -->` and `<!-- dt:NAME:end -->`
markers in the doc; text outside the markers is never touched.

- summary: per target, its latest day with a full plan (else its latest
  day): the best sustained decisions/s at the base shape (one question,
  ~1.5k tokens, cold), the concurrency it took, its p99 and $ per 1,000; the
  best over every shape (batched questions); the knee.
- backends: one table per target, every point of that day.
- trend: per target and day, the best sustained decisions/s at the base
  shape, its p99 and $ per 1,000, so drift is visible.
"""
import argparse, glob, json, os, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

ORDER = ["jev", "gateway-jev", "gateway-pylon", "vertex-gemini", "coderos-clef", "coderos-llamacpp",
         "coderos-ollama", "h100-clef-flash", "h100-clef-27b", "sealed-gpu", "sealed-cpu", "mac-clef"]


def load(paths, gcs_prefix):
    rows = []
    for p in paths:
        for f in glob.glob(p):
            rows += [json.loads(l) for l in open(f) if l.strip()]
    if gcs_prefix:
        import gcs
        for uri in gcs.list_names(gcs_prefix.rstrip("/") + "/"):
            if uri.endswith(".jsonl"):
                data, _ = gcs.get(uri)
                rows += [json.loads(l) for l in data.decode().splitlines() if l.strip()]
    seen, out = set(), []
    for r in rows:  # the same line may be in a local file and in GCS
        k = (r.get("run"), r.get("point"), r.get("time"))
        if k not in seen and r.get("plan") != "smoke" and not r.get("note", "").startswith("smoke"):
            seen.add(k)
            out.append(r)
    return out


def day(r):
    return r["time"][:10]


def f(x, nd=2):
    if x is None:
        return "–"
    if isinstance(x, float):
        if abs(x) >= 100:
            return f"{x:,.0f}"
        return f"{x:.{nd}f}"
    return str(x)


def money(x):
    if x is None:
        return "–"
    return f"${x:.4f}" if x < 0.1 else f"${x:.3f}" if x < 10 else f"${x:,.2f}"


def base_shape(r):
    return (r.get("workload") == "relevance" and r.get("questions") == 1 and r.get("cache") == "cold"
            and (r.get("tokens") == 1500 or (r.get("tokens") == 500 and r.get("target") == "sealed-cpu"))
            and r.get("ok"))


def knee(rows):
    rows = sorted([r for r in rows if r.get("axis") == "concurrency" and r.get("ok")], key=lambda r: r["conc"])
    prev = None
    for r in rows:
        if prev and prev["dps_sustained"]:
            gain = r["dps_sustained"] / prev["dps_sustained"] - 1
            slower = (r["p50_s"] or 0) / (prev["p50_s"] or 1e-9) - 1
            if (gain < 0.10 and slower > 0.5) or (r["error_rate"] or 0) > 0.05:
                return prev["conc"]
        prev = r
    return None


def latest_day(rows):
    full = [r for r in rows if r.get("plan") == "full"]
    pick = full or rows
    d = max(day(r) for r in pick)
    newest = {}  # a point run again the same day replaces the earlier one
    for r in sorted((r for r in pick if day(r) == d), key=lambda r: r["time"]):
        newest[r["point"]] = r
    return d, list(newest.values())


def shape(r):
    if r.get("workload") == "router":
        return f"router ({f(r.get('input_tokens_mean'), 0)} tok)"
    measured = r.get("input_tokens_mean")
    size = f"{r['tokens'] / 1000:g}k" + (f" ({measured / 1000:.1f}k measured)" if measured else "")
    return f"{size} tok × {r['questions']} q" + (" warm" if r.get("cache") == "warm" else "")


def efficient(rows):
    """The best sustained decisions/s, taken at the lowest concurrency that
    reaches 97 % of it: more clients for the same throughput only add wait."""
    if not rows:
        return None
    top = max(r["dps_sustained"] for r in rows)
    return min((r for r in rows if r["dps_sustained"] >= 0.97 * top), key=lambda r: (r["conc"], -r["dps_sustained"]))


def summary(by):
    out = ["| Backend | Best sustained decisions/s, 1 question | At concurrency | p99 | $ / 1,000 | Best over all shapes | Shape, concurrency | Knee | Day |",
           "| --- | ---: | ---: | ---: | ---: | ---: | --- | ---: | --- |"]
    for t in sorted(by, key=lambda t: ORDER.index(t) if t in ORDER else 99):
        d, rows = latest_day(by[t])
        b = [r for r in rows if base_shape(r)]
        best_b = efficient(b)
        # warm points answer from an exact cache: not throughput anyone can count on
        ok = [r for r in rows if r.get("ok") and r.get("cache") != "warm"]
        best = efficient(ok)
        k = knee(rows)
        out.append("| {} | {} | {} | {} | {} | {} | {} | {} | {} |".format(
            t, f(best_b and best_b["dps_sustained"]), best_b["conc"] if best_b else "–",
            f"{best_b['p99_s']:.2f} s" if best_b and best_b.get("p99_s") else "–",
            money(best_b and best_b.get("cost_per_1k_usd")),
            f(best and best["dps_sustained"]), f"{shape(best)}, c={best['conc']}" if best else "–",
            k if k else ("none seen" if b else "–"), d))
    return "\n".join(out)


def backend_tables(by):
    parts = []
    for t in sorted(by, key=lambda t: ORDER.index(t) if t in ORDER else 99):
        d, rows = latest_day(by[t])
        r0 = rows[0]
        price = (r0.get("price") or {})
        parts.append(f"### {t}\n\n{r0.get('hardware') or ''}. Model `{r0.get('model')}`, plan `{r0.get('plan')}`, "
                     f"{d}, run on `{r0.get('host')}`. Price: {price.get('source', '–')}.\n")
        timed = any(r.get("timing") for r in rows)
        gpu = any(r.get("gpu") for r in rows)
        doors = any(r.get("doors") for r in rows)
        att = any(r.get("att_ms_p50") for r in rows)
        head = ["Point", "Shape", "Conc", "OK / failed", "Decisions/s", "Peak 10 s", "p50 s", "p90 s", "p99 s"]
        if timed:
            head += ["queue / prefill / head ms (p50)"]
        if att:
            head += ["verify / seal / relay ms (p50)"]
        head += ["busy retries", "$ / 1,000"]
        if gpu:
            head += ["GPU util mean / max, mem max"]
        if doors:
            head += ["doors"]
        lines = ["| " + " | ".join(head) + " |", "|" + "---|" * len(head)]
        for r in sorted(rows, key=lambda r: (r.get("axis") != "concurrency", r["conc"], r.get("time"))):
            cells = [r["point"], shape(r), str(r["conc"]), f"{r['ok']} / {r['failed']}", f(r["dps_sustained"]),
                     f(r.get("dps_peak_10s")), f(r.get("p50_s")), f(r.get("p90_s")), f(r.get("p99_s"))]
            if timed:
                tm = r.get("timing")
                cells.append(" / ".join(f(tm[k]["p50"], 0) for k in ("queue_ms", "prefill_ms", "head_ms"))
                             + (f", cached {tm['cached_share']:.0%}" if tm.get("cached_share") else "") if tm else "–")
            if att:
                a = r.get("att_ms_p50") or {}
                ver = sum((a.get(k) or 0) for k in ("fetch", "chain", "measure", "bind"))
                cells.append(f"{ver:.0f} / {f(a.get('encrypt'), 0)} / {f(a.get('relay'), 0)}" if a else "–")
            cells += [str(r.get("busy_retries", 0)), money(r.get("cost_per_1k_usd"))]
            if gpu:
                g = r.get("gpu")
                cells.append(f"{g['util_mean']:.0f} % / {g['util_max']:.0f} %, {g['mem_max_mib'] / 1024:.1f} GB" if g else "–")
            if doors:
                cells.append(", ".join(f"{k} {v}" for k, v in (r.get("doors") or {}).items()) or "–")
            errs = r.get("errors") or {}
            if errs:
                cells[3] += " (" + ", ".join(f"{k} ×{v}" for k, v in errs.items()) + ")"
            lines.append("| " + " | ".join(cells) + " |")
        parts.append("\n".join(lines) + "\n")
    return "\n".join(parts)


def trend(by):
    out = ["| Backend | Day | Plan | Best decisions/s, 1 question | Concurrency | p99 | $ / 1,000 |",
           "| --- | --- | --- | ---: | ---: | ---: | ---: |"]
    for t in sorted(by, key=lambda t: ORDER.index(t) if t in ORDER else 99):
        days = sorted({day(r) for r in by[t]})
        for d in days[-30:]:
            rows = [r for r in by[t] if day(r) == d and base_shape(r)]
            if not rows:
                continue
            b = efficient(rows)
            out.append(f"| {t} | {d} | {b.get('plan')} | {f(b['dps_sustained'])} | {b['conc']} | "
                       f"{f(b.get('p99_s'))} s | {money(b.get('cost_per_1k_usd'))} |")
    return "\n".join(out)


def splice(doc, name, body):
    start, end = f"<!-- dt:{name}:start -->", f"<!-- dt:{name}:end -->"
    pat = re.compile(re.escape(start) + r".*?" + re.escape(end), re.S)
    if not pat.search(doc):
        raise SystemExit(f"the doc has no {start} … {end} markers")
    return pat.sub(lambda _: f"{start}\n{body}\n{end}", doc)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("files", nargs="*")
    ap.add_argument("--gcs", help="gs://bucket/prefix holding the dated JSONL files")
    ap.add_argument("--doc", help="rewrite the tables in this markdown file")
    a = ap.parse_args()
    rows = load(a.files, a.gcs)
    by = {}
    for r in rows:
        by.setdefault(r["target"], []).append(r)
    tables = {"summary": summary(by), "backends": backend_tables(by), "trend": trend(by)}
    if a.doc:
        doc = open(a.doc).read()
        for k, v in tables.items():
            doc = splice(doc, k, v)
        open(a.doc, "w").write(doc)
        print(f"wrote {a.doc} from {len(rows)} points")
    else:
        for k, v in tables.items():
            print(f"## {k}\n\n{v}\n")


if __name__ == "__main__":
    main()
