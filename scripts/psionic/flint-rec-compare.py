#!/usr/bin/env python3
"""Decode and prefill speed of rec and the Psionic Metal server, alternating runs (#12422).

Usage: flint-rec-compare.py SOURCE_FILE [METAL_URL] [ENGINES]
  SOURCE_FILE  a long text file for the medium and long prompts (the research
               note used llama.cpp's src/llama-model.cpp)
  METAL_URL    default http://127.0.0.1:18431/v1/chat/completions
  ENGINES      comma-separated: rec,psionic-metal (default both)
rec must listen on 127.0.0.1:11500 for the rec engine.

Method of docs/research/flint/README.md: card sampling (temperature 1, top-p
0.95, top-k 20), up to 512 output tokens, one warm-up and three measured runs
per prompt size, a unique tag at the start of every prompt so no prefix cache
helps. Decode rate is (output tokens - 1) / time after the first token.
"""
import json, subprocess, sys, time, urllib.request, uuid, statistics, pathlib
REC = "http://127.0.0.1:11500/v1/chat/completions"
SOURCE = sys.argv[1]
MINE = sys.argv[2] if len(sys.argv) > 2 else "http://127.0.0.1:18431/v1/chat/completions"
ENGINES = sys.argv[3].split(",") if len(sys.argv) > 3 else ["rec", "psionic-metal"]
KEY = subprocess.check_output([str(pathlib.Path.home() / ".local/bin/rec"), "auth", "token"], text=True).strip() if "rec" in ENGINES else ""
src = open(SOURCE, errors='replace').read()
SIZES = {"short": 60, "medium": 6900, "long": 62000}
SAMPLING = {"temperature": 1.0, "top_p": 0.95, "top_k": 20, "max_tokens": 512}

def messages(size):
    tag = uuid.uuid4().hex[:12]
    if size == "short":
        text = f"[{tag}] Write a haiku about a lighthouse keeper."
    else:
        text = f"[{tag}] Explain briefly what the following C++ code does.\n\n" + src[:SIZES[size]]
    return [{"role": "user", "content": text}]

def rec(size):
    body = dict(SAMPLING, model="flint", messages=messages(size), stream=True, stream_options={"include_usage": True})
    req = urllib.request.Request(REC, data=json.dumps(body).encode(), headers={"Content-Type": "application/json", "Authorization": "Bearer " + KEY})
    start = time.perf_counter(); first = None; usage = None
    with urllib.request.urlopen(req, timeout=3600) as r:
        for line in r:
            line = line.decode().strip()
            if not line.startswith("data:") or line == "data: [DONE]":
                continue
            d = json.loads(line[5:])
            if d.get("usage"): usage = d["usage"]
            for c in d.get("choices", []):
                delta = c.get("delta", {})
                if first is None and (delta.get("content") or delta.get("reasoning_content") or delta.get("reasoning")):
                    first = time.perf_counter()
    end = time.perf_counter()
    n = usage["completion_tokens"]; p = usage["prompt_tokens"]
    return {"prompt": p, "output": n, "ttft": first - start, "decode": (n - 1) / (end - first), "prefill": p / (first - start)}

def mine(size):
    body = dict(SAMPLING, model="flint", messages=messages(size), seed=int(time.time()))
    req = urllib.request.Request(MINE, data=json.dumps(body).encode(), headers={"Content-Type": "application/json"})
    start = time.perf_counter()
    d = json.load(urllib.request.urlopen(req, timeout=3600))
    wall = time.perf_counter() - start
    t = d["psionic"]["timing"]; n = d["usage"]["completion_tokens"]; p = d["usage"]["prompt_tokens"]
    return {"prompt": p, "output": n, "ttft": t["first_token_seconds"], "decode": t["decode_tokens_per_second"],
            "prefill": t["prefill_tokens_per_second"], "server_seconds": d["psionic"]["elapsed_seconds"], "wall": wall,
            "decode_gpu_ms_per_token": 1e3 * t["decode_gpu_seconds"] / max(1, n - 1)}

rows = []
for size in ("short", "medium", "long"):
    fns = [(n, f) for n, f in (("rec", rec), ("psionic-metal", mine)) if n in ENGINES]
    for _, fn in fns:
        fn(size)  # warm-up
    for run in range(3):
        for name, fn in fns:
            r = dict(fn(size), engine=name, size=size, run=run)
            rows.append(r); print(json.dumps(r), flush=True)
json.dump(rows, open("flint-rec-compare-rows.json", "w"), indent=1)
print("\nmedians")
for size in ("short", "medium", "long"):
    for name in ENGINES:
        sel = [r for r in rows if r["size"] == size and r["engine"] == name]
        med = lambda k: statistics.median(r[k] for r in sel)
        print(f"{size:6} {name:14} prompt {sel[0]['prompt']:6} ttft {med('ttft'):6.2f} s  prefill {med('prefill'):6.0f} tok/s  decode {med('decode'):6.1f} tok/s  (runs {[round(r['decode'],1) for r in sel]})")
