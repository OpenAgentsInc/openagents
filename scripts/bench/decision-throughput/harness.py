#!/usr/bin/env python3
"""Decision throughput harness: how many decisions per second a decision
backend sustains when hammered with jobs back to back.

One run sends a stream of real NIP-DEC `/v1/systemone` requests to one
backend for each point of a plan and appends one JSON line per point to
--out (and, with --gcs, to that day's JSONL in Cloud Storage). The requests
come from two real corpora (corpus.py):

  relevance  Coder's file-relevance triage: an issue and K candidate files,
             one `noul` per file ("is file fK relevant?"), sized to a target
             prompt length (0.5k / 1.5k / 4k / 16k tokens). Questions per
             request 1, 4 or 13 is the batched form.
  router     the chat router's split requests, recorded from the web chat
             goldens (main, `answer`, `cli_group`): 3k-12k tokens each.

Cold: every request carries a fresh nonce, so no backend serves it from a
cache. Warm: a run cycles through a small fixed set of requests, so a
backend with an exact or prefix cache (Psionic's logit cache) can use it.

A decision is one answered question. Per point the harness records the
decisions per second sustained over the whole window and at peak (best 10 s
window), request latency p50 / p90 / p99, Psionic's server timing
(`psionic.timing`: queue, prefill, head) when the answer carries it, the
door that answered (gateway), status and refusal counts (429, 503 busy,
timeouts), GPU utilization and memory when --gpu-cmd can read them, input
tokens, and the cost per 1,000 decisions: list price for hosted backends,
the machine's hourly price divided by throughput for ours.

Backends (--backend):
  http    any POST {base}/v1/systemone: Jev, the OpenAgents gateway,
          Psionic Clef, llama.cpp's and Ollama's decision servers
  vertex  Gemini on Vertex AI with structured output, the same prompt and
          schema the gateway's Vertex door sends
  att     a sealed NIP-ATT round per request through `oa-att round`
          (verify the TEE endpoint, seal with NIP-44, relay, open, check the
          receipt), one question per request

Keys come from the environment only (--key-env names the variable) and are
never printed or written.

    python3 -I harness.py --target jev --plan full --corpus DIR --out runs.jsonl
    python3 -I harness.py --target coderos-clef --plan daily --corpus DIR --out runs.jsonl \
        --gpu-cmd 'nvidia-smi --query-gpu=utilization.gpu,memory.used,memory.total --format=csv,noheader,nounits'

`--target NAME` takes a backend, model and price from targets.json; every
field can be overridden on the command line.
"""
import argparse, json, os, random, statistics, subprocess, sys, threading, time, uuid
import urllib.error, urllib.request
from concurrent.futures import ThreadPoolExecutor

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import gcs  # noqa: E402

BYTES_PER_TOKEN = 3.8  # Clef tokens of issue text + Rust; 16k states measured short at 3.3 (2026-10-11)
UA = "openagents-decision-throughput/1"
WARM_POOL = 8


# ---------------------------------------------------------------- corpus

class Corpus:
    def __init__(self, path):
        self.dataset = json.load(open(os.path.join(path, "dataset.json")))
        self.cases = self.dataset["cases"]
        self.files = [(c, f) for c in self.cases for f in c["files"]]
        rp = os.path.join(path, "router.jsonl")
        self.router = []
        if os.path.exists(rp):
            for line in open(rp):
                for r in json.loads(line)["requests"]:
                    self.router.append({"id": r["id"], "body": r["body"]})

    def relevance(self, i, tokens, q, nonce, model):
        """Request i of a relevance point: an issue and q candidate files,
        state about `tokens` long, one noul per file."""
        case = self.cases[i % len(self.cases)]
        files = list(case["files"])
        k = 1
        while len(files) < q:  # q = 13: fill from the next issues' files
            files += self.cases[(i + k) % len(self.cases)]["files"]
            k += 1
        rng = random.Random(i)
        rng.shuffle(files)
        files = files[:q]
        budget = int(tokens * BYTES_PER_TOKEN)
        issue = f"ISSUE #{case['issue']}: {case['title']}\n\n{case['body'].strip()}\n"
        issue = issue[: max(200, budget // 4)]
        head = f"RUN {nonce}\n\n{issue}\nCANDIDATE FILES:\n"
        per = max(64, (budget - len(head)) // q - 40)
        parts, questions = [head], {}
        for n, f in enumerate(files, 1):
            parts.append(f"FILE f{n}: {f['path']}\n```rust\n{f['content'][:per]}\n```\n")
            questions[f"f{n}"] = {"type": "noul",
                                  "instructions": f"Is file f{n} ({f['path']}) relevant to solving the issue?"}
        # A short file leaves its share unused: pad with context files that
        # are not asked about until the state reaches its size.
        used = sum(len(x) for x in parts)
        k = 0
        while used < budget * 0.95 and k < len(self.files):
            _, f = self.files[(i * 7 + k) % len(self.files)]
            k += 1
            if any(f is g for g in files):
                continue
            block = f"CONTEXT FILE: {f['path']}\n```rust\n{f['content'][:budget - used]}\n```\n"
            parts.append(block)
            used += len(block)
        return {"model": model, "state": "\n".join(parts), "questions": questions}

    def routed(self, i, nonce, model):
        r = self.router[i % len(self.router)]
        body = json.loads(json.dumps(r["body"]))
        body["model"] = model
        st = body.get("state")
        if isinstance(st, dict):
            st = dict(st)
            st["run"] = nonce
            body["state"] = st
        else:
            body["state"] = f"RUN {nonce}\n\n{st}"
        return body


def make_request(corpus, point, i, run_nonce, model):
    """The i-th request of a point: cold gets a fresh nonce per request,
    warm cycles WARM_POOL fixed requests under one nonce."""
    if point.get("cache") == "warm":
        i, nonce = i % WARM_POOL, run_nonce
    else:
        nonce = f"{run_nonce}-{i}"
    if point.get("workload") == "router":
        return corpus.routed(i, nonce, model)
    return corpus.relevance(i, point["tokens"], point["questions"], nonce, model)


# ---------------------------------------------------------------- backends

def http_call(cfg, body, timeout):
    data = json.dumps(body).encode()
    h = {"Content-Type": "application/json", "User-Agent": UA}
    if cfg.get("key"):
        h["Authorization"] = f"Bearer {cfg['key']}"
    for k, v in (cfg.get("headers") or {}).items():
        h[k] = v
    req = urllib.request.Request(cfg["base"].rstrip("/") + "/v1/systemone", data=data, headers=h, method="POST")
    status, raw, hdrs = None, b"", {}
    try:
        with urllib.request.urlopen(req, timeout=timeout) as x:
            status, raw, hdrs = x.status, x.read(), dict(x.headers)
    except urllib.error.HTTPError as e:
        status, raw, hdrs = e.code, e.read(), dict(e.headers)
    except Exception as e:  # connection, timeout
        return {"status": 0, "error": type(e).__name__}
    try:
        resp = json.loads(raw)
    except Exception:
        resp = {}
    out = {"status": status, "bytes": len(data)}
    ra = {k.lower(): v for k, v in hdrs.items()}.get("retry-after")
    if ra:
        out["retry_after"] = ra
    if status != 200 or not isinstance(resp, dict):
        err = resp.get("error") if isinstance(resp, dict) else None
        if isinstance(err, dict):
            err = err.get("code") or err.get("message")
        out["error"] = str(err or resp.get("code") or f"http_{status}")[:80]
        return out
    answers = resp.get("answers") or {}
    out["answered"] = sum(1 for k in body["questions"] if k in answers)
    usage = resp.get("usage") or {}
    out["input_tokens"] = usage.get("input_tokens")
    out["output_tokens"] = usage.get("output_tokens")
    timing = (resp.get("psionic") or {}).get("timing")
    if timing:
        out["timing"] = timing
    svc = resp.get("service") or {}
    if svc.get("door"):
        out["door"] = svc["door"]
    out["model"] = resp.get("model")
    return out


VERTEX_SYSTEM = (
    "You are a decision model. Read the state, then answer every typed question about it "
    "with calibrated probabilities. A noul question asks yes or no: give p_yes, the "
    "probability the answer is yes, read with its instructions and its true and false "
    "descriptions. A choice question: give a probability for every option id, read with "
    "each option's description. A score question: give a probability for every level "
    "index, level 0 first. The probabilities of one question sum to 1. Questions do not "
    "read each other's answers. Answer only with JSON matching the schema.")


def vertex_body(body):
    """crates/gateway/src/decision_dispatch.rs `vertex_body`, in Python."""
    props, asked = {}, {}
    for qid, q in body["questions"].items():
        kind = q.get("type", "noul")
        if kind == "choice":
            keys = list((q.get("criteria") or {}).keys())
        elif kind == "score":
            keys = [str(i) for i in range(len(q.get("criteria") or []))]
        else:
            keys = None
        if keys is None:
            schema = {"type": "object", "properties": {"p_yes": {"type": "number"}}, "required": ["p_yes"]}
        else:
            schema = {"type": "object", "required": ["probabilities"], "properties": {"probabilities": {
                "type": "object", "properties": {k: {"type": "number"} for k in keys}, "required": keys}}}
        props[qid] = schema
        asked[qid] = {"type": kind, "instructions": q.get("instructions"), "criteria": q.get("criteria")}
    prompt = json.dumps({"state": body["state"], "questions": asked})
    return {"systemInstruction": {"parts": [{"text": VERTEX_SYSTEM}]},
            "contents": [{"role": "user", "parts": [{"text": prompt}]}],
            "generationConfig": {"responseMimeType": "application/json",
                                 "responseJsonSchema": {"type": "object", "properties": props,
                                                        "required": list(body["questions"])},
                                 "temperature": 0, "thinkingConfig": {"thinkingLevel": "low"}}}


def vertex_call(cfg, body, timeout):
    project = cfg.get("project") or "openagentsgemini"
    loc = cfg.get("location") or "global"
    host = "https://aiplatform.googleapis.com" if loc == "global" else f"https://{loc}-aiplatform.googleapis.com"
    url = f"{host}/v1/projects/{project}/locations/{loc}/publishers/google/models/{cfg['model']}:generateContent"
    data = json.dumps(vertex_body(body)).encode()
    req = urllib.request.Request(url, data=data, method="POST", headers={
        "Content-Type": "application/json", "Authorization": f"Bearer {gcs.token()}", "User-Agent": UA})
    try:
        with urllib.request.urlopen(req, timeout=timeout) as x:
            status, raw = x.status, x.read()
    except urllib.error.HTTPError as e:
        return {"status": e.code, "error": f"vertex_{e.code}", "retry_after": e.headers.get("retry-after")}
    except Exception as e:
        return {"status": 0, "error": type(e).__name__}
    resp = json.loads(raw)
    parts = ((resp.get("candidates") or [{}])[0].get("content") or {}).get("parts") or []
    text = "".join(p.get("text", "") for p in parts if not p.get("thought"))
    try:
        got = json.loads(text)
    except Exception:
        return {"status": 200, "error": "not_json"}
    um = resp.get("usageMetadata") or {}
    return {"status": 200, "answered": sum(1 for k in body["questions"] if k in got),
            "input_tokens": um.get("promptTokenCount"),
            "output_tokens": (um.get("candidatesTokenCount") or 0) + (um.get("thoughtsTokenCount") or 0),
            "model": cfg["model"]}


def att_call(cfg, body, timeout):
    """One sealed round: `oa-att round` verifies the endpoint (release,
    head, Google's token, measurement, key binding), seals the question
    with NIP-44 to the endpoint key, sends it over the relay, and opens and
    checks the sealed answer and its receipt."""
    (qid, q), = list(body["questions"].items())[:1]
    state = body["state"] if isinstance(body["state"], str) else json.dumps(body["state"])
    question = q.get("instructions") if isinstance(q.get("instructions"), str) else json.dumps(q.get("instructions"))
    cmd = [cfg["oa_att"], "round", "--publisher", cfg["publisher"], "--state", state, "--question", question]
    if cfg.get("workload"):
        cmd += ["--workload", cfg["workload"]]
    if cfg.get("relay"):
        cmd += ["--relay", cfg["relay"]]
    try:
        p = subprocess.run(cmd, capture_output=True, timeout=timeout, text=True)
    except subprocess.TimeoutExpired:
        return {"status": 0, "error": "timeout"}
    try:
        out = json.loads(p.stdout)
    except Exception:
        return {"status": 0, "error": (p.stderr.strip().splitlines() or ["no output"])[-1][:80]}
    if out.get("refused"):
        return {"status": 403, "error": f"refused at {out['refused'].get('step')}"}
    steps = {s.get("step"): s for s in out.get("steps", [])}
    ans = (steps.get("answer") or {}).get("response") or {}
    if not ans.get("answers"):
        st = steps.get("status") or {}
        return {"status": 503, "error": str(st.get("refusal") or st.get("word") or "no answer")[:80]}
    r = {"status": 200, "answered": 1, "input_tokens": (ans.get("usage") or {}).get("input_tokens"),
         "att_ms": dict({k: steps[k].get("ms") for k in ("fetch", "chain", "measure", "bind", "encrypt") if k in steps},
                               relay=(steps.get("relay") or {}).get("round_ms"))}
    timing = (ans.get("psionic") or {}).get("timing")
    if timing:
        r["timing"] = timing
    return r


CALLS = {"http": http_call, "vertex": vertex_call, "att": att_call}


# ---------------------------------------------------------------- GPU sampler

class GpuSampler(threading.Thread):
    """Runs --gpu-cmd every 5 s; expects `util, mem_used_mib, mem_total_mib`
    (nvidia-smi --query-gpu=utilization.gpu,memory.used,memory.total
    --format=csv,noheader,nounits), first GPU."""

    def __init__(self, cmd):
        super().__init__(daemon=True)
        self.cmd, self.samples, self.stop = cmd, [], threading.Event()

    def run(self):
        while not self.stop.is_set():
            try:
                out = subprocess.run(self.cmd, shell=True, capture_output=True, text=True, timeout=10).stdout
                u, m, t = [float(x) for x in out.strip().splitlines()[0].split(",")[:3]]
                self.samples.append((u, m, t))
            except Exception:
                pass
            self.stop.wait(5)

    def summary(self):
        if not self.samples:
            return None
        us = [s[0] for s in self.samples]
        return {"util_mean": round(statistics.mean(us), 1), "util_max": max(us),
                "mem_max_mib": max(s[1] for s in self.samples), "mem_total_mib": self.samples[0][2],
                "samples": len(self.samples)}


# ---------------------------------------------------------------- one point

def pct(xs, q):
    if not xs:
        return None
    xs = sorted(xs)
    return round(xs[min(len(xs) - 1, int(round(q * (len(xs) - 1))))], 4)


class Spend:
    def __init__(self, cap, price):
        self.cap, self.price, self.usd, self.lock = cap, price or {}, 0.0, threading.Lock()

    def add(self, r):
        p_in, p_out = self.price.get("usd_per_m_input"), self.price.get("usd_per_m_output", 0)
        if not p_in:
            return
        with self.lock:
            self.usd += ((r.get("input_tokens") or 0) * p_in + (r.get("output_tokens") or 0) * p_out) / 1e6

    def over(self):
        return self.cap is not None and self.usd >= self.cap


def run_point(cfg, corpus, point, args, spend):
    call = CALLS[cfg["kind"]]
    run_nonce = uuid.uuid4().hex[:10]
    counter = iter(range(10 ** 9))
    lock = threading.Lock()
    rows = []
    conc, duration = point["conc"], point["duration"]
    busy_retry = args.busy_retry_ms / 1000.0

    def next_i():
        with lock:
            return next(counter)

    # warmup, discarded: a few requests at the point's concurrency
    if point.get("warmup", args.warmup):
        n = max(conc, point.get("warmup", args.warmup))
        with ThreadPoolExecutor(min(conc, 64)) as ex:
            list(ex.map(lambda i: call(cfg, make_request(corpus, point, i, "w" + run_nonce, cfg["model"]), args.timeout),
                        range(n)))
    gpu = GpuSampler(args.gpu_cmd) if args.gpu_cmd else None
    if gpu:
        gpu.start()
    t0 = time.perf_counter()
    end = t0 + duration
    stopped = {"budget": False, "billing": False}

    def worker():
        while time.perf_counter() < end and not stopped["budget"] and not stopped["billing"]:
            i = next_i()
            body = make_request(corpus, point, i, run_nonce, cfg["model"])
            s = time.perf_counter()
            busy = 0
            while True:
                r = call(cfg, body, args.timeout)
                if (r.get("status") == 503 and busy_retry and cfg["kind"] == "http"
                        and time.perf_counter() < end + args.timeout):
                    busy += 1  # Psionic's queue is full (`busy`): back off and ask again
                    time.sleep(busy_retry)
                    continue
                break
            e = time.perf_counter()
            r.update({"t": round(s - t0, 4), "latency_s": round(e - s, 4), "busy_retries": busy,
                      "questions": len(body["questions"])})
            spend.add(r)
            if spend.over():
                stopped["budget"] = True
            if r.get("status") == 402:  # out of credit: asking again only adds refusals
                stopped["billing"] = True
            with lock:
                rows.append(r)

    with ThreadPoolExecutor(conc) as ex:
        for f in [ex.submit(worker) for _ in range(conc)]:
            f.result()
    wall = time.perf_counter() - t0
    if gpu:
        gpu.stop.set()
    s = summarize(rows, wall, point, cfg, gpu.summary() if gpu else None, stopped["budget"])
    s["billing_stop"] = stopped["billing"]
    return s


def summarize(rows, wall, point, cfg, gpu, budget_stop):
    ok = [r for r in rows if r.get("status") == 200 and r.get("answered")]
    dec = sum(r["answered"] for r in ok)
    lat = [r["latency_s"] for r in ok]
    # peak: best 10 s window of completions
    buckets = {}
    for r in ok:
        b = int((r["t"] + r["latency_s"]) // 10)
        buckets[b] = buckets.get(b, 0) + r["answered"]
    full = [v for b, v in buckets.items() if (b + 1) * 10 <= wall]  # whole windows only
    peak = max(full) / 10.0 if full else (dec / wall if wall else 0.0)
    errors = {}
    for r in rows:
        if not (r.get("status") == 200 and r.get("answered")):
            k = f"{r.get('status')}:{r.get('error', 'unanswered')}"
            errors[k] = errors.get(k, 0) + 1
    s = {
        "requests": len(rows), "ok": len(ok), "failed": len(rows) - len(ok),
        "error_rate": round((len(rows) - len(ok)) / len(rows), 4) if rows else None,
        "busy_retries": sum(r.get("busy_retries", 0) for r in rows),
        "busy_refusal_rate": round(sum(r.get("busy_retries", 0) for r in rows) /
                                   max(1, len(rows) + sum(r.get("busy_retries", 0) for r in rows)), 4),
        "rate_limited": sum(1 for r in rows if r.get("status") == 429),
        "errors": dict(sorted(errors.items(), key=lambda kv: -kv[1])[:6]),
        "decisions": dec, "wall_s": round(wall, 2),
        "dps_sustained": round(dec / wall, 3) if wall else None, "dps_peak_10s": round(peak, 3),
        "rps": round(len(ok) / wall, 3) if wall else None,
        "p50_s": pct(lat, .5), "p90_s": pct(lat, .9), "p99_s": pct(lat, .99),
        "input_tokens_mean": round(statistics.mean([r["input_tokens"] for r in ok if r.get("input_tokens")]), 1)
        if any(r.get("input_tokens") for r in ok) else None,
        "output_tokens_mean": round(statistics.mean([r.get("output_tokens") or 0 for r in ok]), 1) if ok else None,
        "budget_stop": budget_stop,
    }
    timed = [r["timing"] for r in ok if r.get("timing")]
    if timed:
        s["timing"] = {k: {"p50": pct([t.get(k, 0) for t in timed], .5), "p90": pct([t.get(k, 0) for t in timed], .9)}
                       for k in ("queue_ms", "prefill_ms", "head_ms", "total_ms")}
        s["timing"]["cached_share"] = round(sum(1 for t in timed if t.get("cached")) / len(timed), 3)
        s["timing"]["transport_ms_p50"] = pct([r["latency_s"] * 1e3 - r["timing"].get("total_ms", 0)
                                               for r in ok if r.get("timing")], .5)
    att = [r["att_ms"] for r in ok if r.get("att_ms")]
    if att:
        s["att_ms_p50"] = {k: pct([a.get(k) or 0 for a in att], .5) for k in att[0]}
    doors = {}
    for r in ok:
        if r.get("door"):
            d = r["door"].split(":")[0] if not r["door"].startswith("pylon:") else r["door"]
            doors[d] = doors.get(d, 0) + 1
    if doors:
        s["doors"] = doors
    if gpu:
        s["gpu"] = gpu
    s["cost_per_1k_usd"] = cost_per_1k(cfg, ok, dec, s["dps_sustained"])
    return s


def cost_per_1k(cfg, ok, dec, dps):
    price = cfg.get("price") or {}
    if price.get("usd_per_m_input") and dec:
        usd = sum((r.get("input_tokens") or 0) * price["usd_per_m_input"] +
                  (r.get("output_tokens") or 0) * price.get("usd_per_m_output", 0) for r in ok) / 1e6
        return round(usd / dec * 1000, 5)
    if price.get("usd_per_hour") and dps:
        return round(price["usd_per_hour"] / (dps * 3600) * 1000, 5)
    return None


# ---------------------------------------------------------------- plans

def plan_points(name, cfg):
    """Each plan is one-factor-at-a-time around a base point (1.5k tokens,
    one question, cold): a concurrency sweep to the knee, then prompt size,
    questions per request, warm and router at the best concurrency."""
    caps = cfg.get("max_conc", 64)
    base = {"workload": "relevance", "tokens": cfg.get("base_tokens", 1500), "questions": 1, "cache": "cold"}
    if name == "full":
        conc = [c for c in cfg.get("conc_list", (1, 2, 4, 8, 16, 32, 64)) if c <= caps]
        rest = [dict(base, tokens=500), dict(base, tokens=4000), dict(base, tokens=16000),
                dict(base, tokens=4000, questions=4), dict(base, tokens=4000, questions=13),
                dict(base, cache="warm"), {"workload": "router", "cache": "cold"}]
    elif name == "daily":
        conc = [c for c in cfg.get("daily_conc", (1, 4, 16)) if c <= caps]
        rest = [dict(base, tokens=16000), dict(base, tokens=4000, questions=13), {"workload": "router", "cache": "cold"}]
        if cfg.get("daily_skip_long"):  # hosted daily runs: the long prompt is weekly material
            rest = rest[1:]
    elif name == "smoke":
        conc = [1, min(4, caps)]
        rest = [dict(base, tokens=4000, questions=4)]
    else:
        raise SystemExit(f"unknown plan {name}")
    return [dict(base, conc=c, axis="concurrency") for c in conc], [dict(p, axis="shape") for p in rest]


def knee(rows):
    """The concurrency past which more clients stop buying throughput but
    latency keeps growing: the first step where sustained decisions/s grows
    under 10 % while p50 grows over 50 %, or where errors pass 5 %."""
    prev = None
    for r in rows:
        if prev and r["dps_sustained"] is not None and prev["dps_sustained"]:
            gain = r["dps_sustained"] / prev["dps_sustained"] - 1
            slower = (r["p50_s"] or 0) / (prev["p50_s"] or 1e-9) - 1
            if (gain < 0.10 and slower > 0.5) or (r["error_rate"] or 0) > 0.05:
                return prev["conc"]
        prev = r
    return None


# ---------------------------------------------------------------- main

def load_target(name):
    targets = json.load(open(os.path.join(HERE, "targets.json")))
    if name not in targets:
        raise SystemExit(f"unknown target {name}; see targets.json: {', '.join(sorted(targets))}")
    t = dict(targets[name])
    t["target"] = name
    return t


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--target", required=True, help="a name in targets.json")
    ap.add_argument("--corpus", required=True, help="directory with dataset.json and router.jsonl, or gs://…/")
    ap.add_argument("--out", required=True, help="local JSONL to append one line per point to")
    ap.add_argument("--gcs", help="gs://bucket/prefix: also append to <prefix>/YYYY-MM-DD.jsonl")
    ap.add_argument("--plan", default="full", help="full | daily | smoke")
    ap.add_argument("--only", help="comma list of point labels to run (e.g. c1,c8,t16000)")
    ap.add_argument("--duration", type=float, help="seconds per point (default: the target's, else 150)")
    ap.add_argument("--base", help="override the base URL")
    ap.add_argument("--model", help="override the model")
    ap.add_argument("--header", action="append", default=[], help="K: V, repeatable")
    ap.add_argument("--max-conc", type=int, help="cap concurrency (protect a shared server)")
    ap.add_argument("--budget-usd", type=float, help="stop when the estimated hosted spend passes this")
    ap.add_argument("--usd-per-hour", type=float, help="machine price for our hardware")
    ap.add_argument("--warmup", type=int, default=4)
    ap.add_argument("--timeout", type=float, help="seconds per request (default: the target's, else 120)")
    ap.add_argument("--busy-retry-ms", type=float, default=50, help="0 counts 503 busy as a failure")
    ap.add_argument("--gpu-cmd", help="shell command printing util,mem_used,mem_total (nvidia-smi csv)")
    ap.add_argument("--no-knee-stop", action="store_true", help="run every concurrency even past the knee")
    ap.add_argument("--note", default="")
    a = ap.parse_args()

    cfg = load_target(a.target)
    if a.base:
        cfg["base"] = a.base
    if a.model:
        cfg["model"] = a.model
    for h in a.header:
        k, v = h.split(":", 1)
        cfg.setdefault("headers", {})[k.strip()] = v.strip()
    if a.max_conc:
        cfg["max_conc"] = a.max_conc
    if a.usd_per_hour:
        cfg.setdefault("price", {})["usd_per_hour"] = a.usd_per_hour
    if cfg.get("key_env"):
        cfg["key"] = os.environ.get(cfg["key_env"])
        if not cfg["key"]:
            raise SystemExit(f"{cfg['key_env']} is not set")
    if cfg["kind"] == "att":
        cfg["oa_att"] = os.environ.get("OA_ATT", cfg.get("oa_att", "oa-att"))
    a.timeout = a.timeout or cfg.get("timeout", 120)
    budget = a.budget_usd if a.budget_usd is not None else (
        cfg.get("daily_budget_usd", cfg.get("budget_usd")) if a.plan == "daily" else cfg.get("budget_usd"))
    spend = Spend(budget, cfg.get("price"))
    duration = a.duration or cfg.get("duration", 150)

    corpus_dir = gcs.local_corpus(a.corpus)
    corpus = Corpus(corpus_dir)
    sweep, shapes = plan_points(a.plan, cfg)
    if cfg.get("questions_max"):  # sealed rounds ask one question
        shapes = [p for p in shapes if p.get("questions", 1) <= cfg["questions_max"] and p["workload"] != "router"]
    if cfg.get("tokens_max"):
        shapes = [p for p in shapes if p.get("tokens", 0) <= cfg["tokens_max"]]
    only = set(a.only.split(",")) if a.only else None
    run_id = time.strftime("%Y%m%dT%H%M%SZ", time.gmtime()) + "-" + uuid.uuid4().hex[:6]
    meta = {"run": run_id, "target": a.target, "kind": cfg["kind"], "model": cfg.get("model"),
            "base": cfg.get("base") if cfg["kind"] == "http" else None, "plan": a.plan,
            "host": os.uname().nodename, "note": a.note, "price": cfg.get("price"),
            "hardware": cfg.get("hardware"), "harness": 1}

    def label(p):
        if p["workload"] == "router":
            return f"router-{p['cache']}"
        return f"c{p['conc']}" if p.get("axis") == "concurrency" else f"t{p['tokens']}q{p['questions']}-{p['cache']}"

    def emit(p, s):
        line = dict(meta, time=time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()), point=label(p),
                    conc=p["conc"], workload=p["workload"], tokens=p.get("tokens"), questions=p.get("questions"),
                    cache=p["cache"], duration_s=p["duration"], axis=p.get("axis"), **s)
        with open(a.out, "a") as fh:
            fh.write(json.dumps(line) + "\n")
        if a.gcs:
            try:
                gcs.append_jsonl(a.gcs, line)
            except Exception as e:  # keep running; the local file has it
                print(f"gcs append failed: {e}", file=sys.stderr)
        short = {k: line[k] for k in ("point", "ok", "failed", "dps_sustained", "dps_peak_10s", "p50_s", "p99_s",
                                      "cost_per_1k_usd") if k in line}
        if line.get("timing"):
            short["queue_p50"] = line["timing"]["queue_ms"]["p50"]
            short["prefill_p50"] = line["timing"]["prefill_ms"]["p50"]
        if line.get("doors"):
            short["doors"] = line["doors"]
        if line.get("errors"):
            short["errors"] = line["errors"]
        print(json.dumps(short), flush=True)
        return line

    done = []
    for p in sweep:
        p["duration"] = duration
        if only and label(p) not in only:
            continue
        if spend.over():
            print("budget reached; stopping", flush=True)
            break
        done.append(emit(p, run_point(cfg, corpus, p, a, spend)))
        if done[-1].get("billing_stop"):
            print("402: the account is out of credit; stopping this target", flush=True)
            return
        k = knee(done)
        if k is not None and not a.no_knee_stop and done[-1]["conc"] >= 4 * k:
            print(f"past the knee (c={k}); skipping higher concurrency", flush=True)
            break
    best = max(done, key=lambda r: r["dps_sustained"] or 0)["conc"] if done else cfg.get("shape_conc", 4)
    # the shape points run at the best concurrency found, capped by the target
    shape_conc = min(best, cfg.get("shape_conc_max", best))
    if a.plan == "daily" and cfg.get("daily_shape_conc"):
        shape_conc = cfg["daily_shape_conc"]
    for p in shapes:
        p["conc"], p["duration"] = shape_conc, duration
        if only and label(p) not in only:
            continue
        if spend.over():
            print("budget reached; stopping", flush=True)
            break
        if emit(p, run_point(cfg, corpus, p, a, spend)).get("billing_stop"):
            print("402: the account is out of credit; stopping this target", flush=True)
            return
    print(json.dumps({"run": run_id, "target": a.target, "spend_usd_est": round(spend.usd, 3),
                      "knee_conc": knee(done)}), flush=True)


if __name__ == "__main__":
    main()
