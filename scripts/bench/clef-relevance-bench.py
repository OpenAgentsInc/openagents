#!/usr/bin/env python3
"""File-relevance decision throughput: Clef (Ollama / llama.cpp) vs hosted Jev.

Task: "given a GitHub issue and one file's contents, is this file relevant to
solving the issue?" asked as a System One `noul`. The request and response
JSON match `crates/jev` (see crates/jev/tests/fixtures/systemone-*.json).

Modes
  seq    one file per request, back to back
  batch  one request per issue, one noul per candidate file (keyed f1..fk)
  conc   one file per request from --conc concurrent clients

Every run puts a fresh nonce at the start of the state, so nothing is served
from a previous run's prompt cache. Within a run, files of the same issue
share the issue prefix (as a real triage loop would); Ollama reports reuse as
`prompt_eval_cached_count` and we record it.

Keys come from the environment only (TYPESAFE_API_KEY for the TypeSafe
backend); nothing is printed or written that contains a key. The dataset file
holds repository contents: keep it and the results in a scratch directory.

    python3 -I scripts/bench/clef-relevance-bench.py --dataset D --out R.jsonl \
        --backend ollama-flash --mode seq --warmup 3
"""
import argparse, json, os, statistics, subprocess, sys, time, uuid
import urllib.request, urllib.error
from concurrent.futures import ThreadPoolExecutor

BACKENDS = {
    # name: (base url, model, key env var or None)
    "ollama-flash": ("http://127.0.0.1:11434", "clef-flash", None),
    "ollama-clef": ("http://127.0.0.1:11434", "clef", None),
    "ollama-flash-np4": ("http://127.0.0.1:11435", "clef-flash", None),
    "ollama-clef-np4": ("http://127.0.0.1:11435", "clef", None),
    # coderos-4080 over an ssh tunnel (see the doc for the user-level server)
    "coderos-ollama-flash": ("http://127.0.0.1:21436", "clef-flash", None),
    "coderos-ollama-clef": ("http://127.0.0.1:21436", "clef", None),
    "coderos-llamacpp-flash": ("http://127.0.0.1:21091", "clef-flash", None),
    # llama.cpp b11538 llama-server (Metal) on this Mac
    "mac-llamacpp-flash": ("http://127.0.0.1:18093", "clef-flash", None),
    "mac-llamacpp-27b": ("http://127.0.0.1:18094", "clef", None),
    # Our decision API (#11225): connected Pylons first, keyless.
    "openagents": ("https://openagents.com/api", "jev-latest", None),
    "jev": ("https://api.typesafe.ai", "jev-latest", "TYPESAFE_API_KEY"),
}
QUESTION = "Is this file relevant to solving the issue?"


def issue_text(c):
    return f"ISSUE #{c['issue']}: {c['title']}\n\n{c['body'].strip()}\n"


def file_block(f, label="FILE"):
    return f"{label}: {f['path']}\n```rust\n{f['content']}\n```\n"


def per_file_requests(case, model, nonce):
    for f in case["files"]:
        state = f"RUN {nonce}\n\n{issue_text(case)}\n{file_block(f)}"
        body = {"model": model, "state": state,
                "questions": {"relevant": {"type": "noul", "instructions": QUESTION}}}
        yield {"issue": case["issue"], "files": [f], "body": body}


def batch_request(case, model, nonce):
    parts = [f"RUN {nonce}\n\n{issue_text(case)}\nCANDIDATE FILES:\n"]
    questions = {}
    for i, f in enumerate(case["files"], 1):
        parts.append(file_block(f, f"FILE f{i}"))
        questions[f"f{i}"] = {"type": "noul",
                              "instructions": f"Is file f{i} ({f['path']}) relevant to solving the issue?"}
    body = {"model": model, "state": "\n".join(parts), "questions": questions}
    return {"issue": case["issue"], "files": case["files"], "body": body}


def call(base, key, req, timeout):
    data = json.dumps(req["body"]).encode()
    h = {"Content-Type": "application/json", "User-Agent": "openagents-clef-relevance-bench/1"}
    if key:
        h["Authorization"] = f"Bearer {key}"
    r = urllib.request.Request(base + "/v1/systemone", data=data, headers=h, method="POST")
    t0 = time.perf_counter()
    status, resp, hdrs = None, None, {}
    try:
        with urllib.request.urlopen(r, timeout=timeout) as x:
            status, raw, hdrs = x.status, x.read(), dict(x.headers)
    except urllib.error.HTTPError as e:
        status, raw, hdrs = e.code, e.read(), dict(e.headers)
    except Exception as e:  # connection / timeout
        raw = json.dumps({"error": type(e).__name__ + ": " + str(e)[:200]}).encode()
    dt = time.perf_counter() - t0
    try:
        resp = json.loads(raw)
    except Exception:
        resp = {"error": raw[:300].decode("utf-8", "replace")}
    keep = {k: v for k, v in hdrs.items() if k.lower().startswith(("x-ratelimit", "retry-after", "x-request-id", "request-id"))}
    out = {"issue": req["issue"], "latency_s": dt, "status": status, "bytes": len(data),
           "headers": keep, "usage": resp.get("usage") if isinstance(resp, dict) else None,
           "model_echo": resp.get("model") if isinstance(resp, dict) else None, "decisions": []}
    if status != 200:
        out["error"] = (resp.get("error") or resp.get("message") or resp) if isinstance(resp, dict) else str(resp)
        out["error"] = str(out["error"])[:300]
        return out
    answers = resp.get("answers", {})
    keys = list(req["body"]["questions"].keys())
    for k, f in zip(keys, req["files"]):
        a = answers.get(k, {})
        out["decisions"].append({"path": f["path"], "label": f["relevant"], "p": a.get("noul")})
    return out


def loadavg():
    try:
        return [float(x) for x in os.getloadavg()]
    except Exception:
        return None


def pct(xs, q):
    if not xs:
        return None
    xs = sorted(xs)
    return xs[min(len(xs) - 1, int(round(q * (len(xs) - 1))))]


def summarize(rows, wall, label):
    ok = [r for r in rows if r["status"] == 200]
    lat = [r["latency_s"] for r in ok]
    dec = sum(len(r["decisions"]) for r in ok)
    itok = [r["usage"].get("input_tokens") for r in ok if r.get("usage") and r["usage"].get("input_tokens")]
    otok = [r["usage"].get("output_tokens", 0) for r in ok if r.get("usage")]
    cost = [r["usage"].get("cost") for r in ok if r.get("usage") and r["usage"].get("cost") is not None]
    cached = [r["usage"].get("prompt_eval_cached_count") for r in ok if r.get("usage") and r["usage"].get("prompt_eval_cached_count") is not None]
    s = {"label": label, "requests": len(rows), "ok": len(ok), "refused": len(rows) - len(ok),
         "decisions": dec, "wall_s": wall, "decisions_per_s": dec / wall if wall else None,
         "p50_s": pct(lat, .5), "p90_s": pct(lat, .9), "p99_s": pct(lat, .99),
         "input_tokens_mean": statistics.mean(itok) if itok else None,
         "output_tokens_mean": statistics.mean(otok) if otok else None,
         "prefill_tok_s_median": statistics.median([r["usage"]["input_tokens"] / r["latency_s"] for r in ok
                                                    if r.get("usage") and r["usage"].get("input_tokens")]) if itok else None,
         "cached_tokens_mean": statistics.mean(cached) if cached else None,
         "cost_total": sum(cost) if cost else None,
         "cost_per_1k_decisions": (sum(cost) / dec * 1000) if cost and dec else None,
         "errors": sorted({r.get("error", "") for r in rows if r["status"] != 200})[:3]}
    return s


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dataset", required=True)
    ap.add_argument("--out", required=True, help="JSONL: one line per run (summary + per-call rows, no contents)")
    ap.add_argument("--backend", required=True, choices=sorted(BACKENDS))
    ap.add_argument("--base-url", help="override the backend's base URL")
    ap.add_argument("--model", help="override the backend's model (e.g. a num_gpu variant)")
    ap.add_argument("--mode", choices=["seq", "batch", "conc"], default="seq")
    ap.add_argument("--conc", type=int, default=1)
    ap.add_argument("--warmup", type=int, default=3)
    ap.add_argument("--rounds", type=int, default=1, help="passes over the dataset (fresh nonce each)")
    ap.add_argument("--labeled-only", action="store_true")
    ap.add_argument("--max-requests", type=int, default=0)
    ap.add_argument("--timeout", type=float, default=300)
    ap.add_argument("--pace", type=float, default=0, help="seconds to sleep between sequential calls")
    ap.add_argument("--note", default="")
    a = ap.parse_args()
    base, model, keyvar = BACKENDS[a.backend]
    base = a.base_url or base
    model = a.model or model
    key = os.environ.get(keyvar) if keyvar else None
    if keyvar and not key:
        sys.exit(f"{keyvar} is not set")
    ds = json.load(open(a.dataset))
    cases = [c for c in ds["cases"] if not a.labeled_only or c["commit"]]

    def build(nonce):
        reqs = []
        for c in cases:
            if a.mode == "batch":
                reqs.append(batch_request(c, model, nonce))
            else:
                reqs.extend(per_file_requests(c, model, nonce))
        return reqs

    # warmup: a different nonce, discarded
    warm = build("warm-" + uuid.uuid4().hex[:8])[-a.warmup:] if a.warmup else []
    for r in warm:
        call(base, key, r, a.timeout)
    reqs = []
    for _ in range(a.rounds):
        reqs.extend(build(uuid.uuid4().hex[:12]))
    if a.max_requests:
        reqs = reqs[:a.max_requests]
    load0 = loadavg()
    t0 = time.perf_counter()
    if a.mode == "conc" and a.conc > 1:
        with ThreadPoolExecutor(a.conc) as ex:
            rows = list(ex.map(lambda r: call(base, key, r, a.timeout), reqs))
    else:
        rows = []
        for r in reqs:
            rows.append(call(base, key, r, a.timeout))
            if a.pace:
                time.sleep(a.pace)
    wall = time.perf_counter() - t0
    load1 = loadavg()
    label = f"{a.backend} {a.mode}" + (f" c={a.conc}" if a.mode == "conc" else "")
    s = summarize(rows, wall, label)
    s.update({"backend": a.backend, "model": model, "mode": a.mode, "conc": a.conc,
              "load_before": load0, "load_after": load1, "note": a.note,
              "time": time.strftime("%Y-%m-%dT%H:%M:%S"), "warmup": a.warmup,
              "cap_bytes": ds.get("cap_bytes")})
    with open(a.out, "a") as fh:
        fh.write(json.dumps({"summary": s, "rows": rows}) + "\n")
    print(json.dumps({k: (round(v, 3) if isinstance(v, float) else v) for k, v in s.items()}))


main()
