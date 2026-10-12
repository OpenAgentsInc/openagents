#!/usr/bin/env python3
"""Batch sweep: decisions per second as the number of questions batched into
ONE `/v1/systemone` request grows, across a few prompt sizes.

Complements harness.py, which sweeps concurrency and shapes at a fixed
question count. Here the axis is questions per request: a request answers
every question it holds, so decisions per second is the request rate times
the questions in it. Answers the question "how many decisions can this lane
sustain, and does batching more help?".

It reuses the harness's own request builder (harness.Corpus.relevance) and
HTTP caller (harness.http_call), so it sends the same real NIP-DEC requests
harness.py does. Run it with `python3 -I` against a `--corpus` directory
holding `dataset.json` and `router.jsonl` (see harness.py).

    python3 -I batch_sweep.py --corpus DIR --out batch.jsonl \
        --sizes 300,1000,4000 --qs 1,4,13,26,64 --duration 25

    # one probe per question count: status, answered count and server timing,
    # no sustained run (fast; good for finding the served cap)
    python3 -I batch_sweep.py --corpus DIR --out /dev/null --probe --qs 1,64,65
"""
import argparse, json, sys, time

HERE = __import__("os").path.dirname(__import__("os").path.abspath(__file__))
sys.path.insert(0, HERE)
import harness  # noqa: E402


def pct(xs, p):
    if not xs:
        return None
    xs = sorted(xs)
    return round(xs[min(len(xs) - 1, int(round(p * (len(xs) - 1))))], 3)


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--base", default="http://127.0.0.1:18098",
                    help="the /v1/systemone server (default: the Mac Metal lane)")
    ap.add_argument("--model", default="clef-flash")
    ap.add_argument("--corpus", required=True, help="directory with dataset.json and router.jsonl")
    ap.add_argument("--duration", type=float, default=25, help="seconds per point")
    ap.add_argument("--out", required=True, help="local JSONL, one line per point")
    ap.add_argument("--sizes", default="300,1000,4000", help="comma list of state token targets")
    ap.add_argument("--qs", default="1,4,13,26,64", help="comma list of questions per request")
    ap.add_argument("--timeout", type=float, default=600)
    ap.add_argument("--probe", action="store_true",
                    help="one request per question count; print status and timing, send nothing sustained")
    a = ap.parse_args()

    cfg = {"kind": "http", "base": a.base, "model": a.model, "timeout": a.timeout}
    corpus = harness.Corpus(a.corpus)
    sizes = [int(x) for x in a.sizes.split(",")]
    qs = [int(x) for x in a.qs.split(",")]

    if a.probe:
        for q in qs:
            body = corpus.relevance(0, 700, q, "probe-%d" % q, a.model)
            r = harness.http_call(cfg, body, a.timeout)
            print(json.dumps({"q": q, "status": r.get("status"), "answered": r.get("answered"),
                              "in_tokens": r.get("input_tokens"), "timing": r.get("timing"),
                              "error": r.get("error")}), flush=True)
        return

    for tokens in sizes:
        for q in qs:
            for w in range(2):  # warmup, discarded
                harness.http_call(cfg, corpus.relevance(w, tokens, q,
                                                       "warm-%d-%d-%d" % (tokens, q, w), a.model),
                                  a.timeout)
            nonce = "%d-%d-%d" % (tokens, q, int(time.time()))
            t0 = time.perf_counter()
            end = t0 + a.duration
            i = dec = req = fail = 0
            lat, timings, toks, errs = [], [], [], {}
            while time.perf_counter() < end:
                body = corpus.relevance(i, tokens, q, "%s-%d" % (nonce, i), a.model)
                i += 1
                s = time.perf_counter()
                r = harness.http_call(cfg, body, a.timeout)
                e = time.perf_counter()
                req += 1
                if r.get("status") == 200 and r.get("answered"):
                    dec += r["answered"]
                    lat.append(e - s)
                    if r.get("input_tokens"):
                        toks.append(r["input_tokens"])
                    if r.get("timing"):
                        timings.append(r["timing"])
                else:
                    fail += 1
                    k = "%s:%s" % (r.get("status"), r.get("error"))
                    errs[k] = errs.get(k, 0) + 1
            wall = time.perf_counter() - t0
            row = {"tokens": tokens, "q": q, "requests": req, "ok_decisions": dec, "failed": fail,
                   "wall_s": round(wall, 1), "dps": round(dec / wall, 3),
                   "in_tokens_mean": round(sum(toks) / len(toks), 1) if toks else None,
                   "p50_s": pct(lat, .5), "p90_s": pct(lat, .9), "errs": errs}
            if timings:
                row["prefill_p50"] = pct([t.get("prefill_ms", 0) for t in timings], .5)
                row["head_p50"] = pct([t.get("head_ms", 0) for t in timings], .5)
                row["queue_p50"] = pct([t.get("queue_ms", 0) for t in timings], .5)
                row["total_p50"] = pct([t.get("total_ms", 0) for t in timings], .5)
            print(json.dumps(row), flush=True)
            open(a.out, "a").write(json.dumps(row) + "\n")


if __name__ == "__main__":
    main()
