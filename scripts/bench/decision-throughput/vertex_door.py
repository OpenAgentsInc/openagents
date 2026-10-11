#!/usr/bin/env python3
"""The gateway's Vertex decision door alone, as a local `/v1/systemone`.

`crates/gateway/src/decision_dispatch.rs` answers a decision at Gemini on
Vertex (`ask_vertex`) when Jev and every Pylon could not. This server does
only that step, byte for byte: the same `generateContent` body
(`vertex_body`: compact JSON prompt, response schema, temperature 0,
thinking low), the same reading of the answer (`vertex_answers`: clamp to
[0, 1], normalize, highest-probability option with ties to the lower id)
and the same envelope (`model`, `answers`, `usage`, `service.door =
vertex`). So the router eval and the chat goldens can measure the
production judge's last fallback through `CODER_DECISION_PROFILE` pointed
here (#11302):

    CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config \
      python3 -I vertex_door.py --port 18199 --log vertex-door.jsonl --budget-usd 4

Every call appends its token counts and estimated cost to --log; once the
estimate passes --budget-usd the server answers 402 and asks nothing more.
With --cache DIR an identical request (same state and questions) is
answered from Gemini's first answer to it, so a second pass over the same
rows (another calibration map, the goldens again) costs nothing and reads
exactly the same probabilities.
The access token comes from `gcloud auth print-access-token` and is never
printed.
"""
import argparse, hashlib, json, os, subprocess, threading, time, urllib.error, urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

SYSTEM = (
    "You are a decision model. Read the state, then answer every typed question about it "
    "with calibrated probabilities. A noul question asks yes or no: give p_yes, the "
    "probability the answer is yes, read with its instructions and its true and false "
    "descriptions. A choice question: give a probability for every option id, read with "
    "each option's description. A score question: give a probability for every level "
    "index, level 0 first. The probabilities of one question sum to 1. Questions do not "
    "read each other's answers. Answer only with JSON matching the schema.")
# crates/inference/src/upstream/vertex.rs list price for gemini-3.8-flash;
# output includes thinking tokens.
USD_PER_M_IN, USD_PER_M_OUT = 0.75, 3.75


def compact(v):
    """serde_json's `to_string`: no spaces, non-ASCII kept."""
    return json.dumps(v, separators=(",", ":"), ensure_ascii=False)


def probabilities_schema(keys):
    return {"type": "object",
            "properties": {"probabilities": {"type": "object", "properties": {k: {"type": "number"} for k in keys},
                                             "required": keys}},
            "required": ["probabilities"]}


def vertex_body(state, questions):
    properties, asked = {}, {}
    for qid, q in questions.items():
        kind = q.get("type") if isinstance(q.get("type"), str) else "noul"
        if kind == "choice":
            schema = probabilities_schema(list((q.get("criteria") or {}).keys()))
        elif kind == "score":
            schema = probabilities_schema([str(i) for i in range(len(q.get("criteria") or []))])
        else:
            schema = {"type": "object", "properties": {"p_yes": {"type": "number"}}, "required": ["p_yes"]}
        properties[qid] = schema
        asked[qid] = {"type": kind, "instructions": q.get("instructions"), "criteria": q.get("criteria")}
    prompt = {"state": state, "questions": asked}
    return {
        "systemInstruction": {"parts": [{"text": SYSTEM}]},
        "contents": [{"role": "user", "parts": [{"text": compact(prompt)}]}],
        "generationConfig": {
            "responseMimeType": "application/json",
            "responseJsonSchema": {"type": "object", "properties": properties, "required": list(questions)},
            "temperature": 0,
            "thinkingConfig": {"thinkingLevel": "low"},
        },
    }


def fnum(v):
    return float(v) if isinstance(v, (int, float)) and not isinstance(v, bool) and v == v and abs(v) != float("inf") else None


def normalized(keys, got):
    got = got if isinstance(got, dict) else {}
    raw = [min(1.0, max(0.0, fnum(got.get(k)) if fnum(got.get(k)) is not None else 0.0)) for k in keys]
    s = sum(raw)
    if s <= 0:
        n = max(1, len(keys))
        return [1.0 / n] * len(keys)
    return [p / s for p in raw]


def vertex_answers(questions, raw):
    answers = {}
    for qid, q in questions.items():
        got = raw.get(qid) if isinstance(raw, dict) else None
        got = got if isinstance(got, dict) else {}
        kind = q.get("type") if isinstance(q.get("type"), str) else "noul"
        if kind == "choice":
            options = list((q.get("criteria") or {}).keys())
            p = normalized(options, got.get("probabilities"))
            best = None
            for o, v in zip(options, p):  # the gateway's fold: ties go to the lower id
                if best is not None and (best[1] > v or (best[1] == v and best[0] <= o)):
                    continue
                best = (o, v)
            if best is None:
                raise ValueError(f"question `{qid}` has no options")
            answers[qid] = {"type": "choice", "choice": best[0], "confidence": best[1],
                            "probabilities": dict(zip(options, p))}
        elif kind == "score":
            levels = q.get("criteria") or []
            keys = [str(i) for i in range(len(levels))]
            p = normalized(keys, got.get("probabilities"))
            answers[qid] = {"type": "score", "score": sum(i * v for i, v in enumerate(p)),
                            "confidence": max([0.0] + p), "legend": dict(zip(keys, levels)),
                            "probabilities": dict(zip(keys, p))}
        else:
            v = fnum(got.get("p_yes"))
            if v is None:
                raise ValueError(f"question `{qid}` has no p_yes")
            answers[qid] = {"type": "noul", "noul": min(1.0, max(0.0, v))}
    return answers


class Door:
    def __init__(self, a):
        self.a, self.lock = a, threading.Lock()
        self.token, self.until, self.usd, self.calls, self.hits = None, 0.0, 0.0, 0, 0

    def bearer(self, fresh=False):
        with self.lock:
            if fresh or not self.token or time.time() > self.until:
                self.token = subprocess.run(["gcloud", "auth", "print-access-token"], capture_output=True,
                                            text=True, check=True).stdout.strip()
                self.until = time.time() + 600  # gcloud may hand back a token near its end
            return self.token

    def ask(self, body):
        status, out, log = self.ask_once(body, False)
        if log and log.get("status") == 401:  # an expired token: mint one and ask again
            status, out, log = self.ask_once(body, True)
        return status, out, log

    def ask_once(self, body, fresh):
        state, questions = body.get("state"), body.get("questions") or {}
        loc = self.a.location
        host = "https://aiplatform.googleapis.com" if loc == "global" else f"https://{loc}-aiplatform.googleapis.com"
        url = (f"{host}/v1/projects/{self.a.project}/locations/{loc}/publishers/google/models/"
               f"{self.a.model}:generateContent")
        req = urllib.request.Request(url, data=compact(vertex_body(state, questions)).encode(), method="POST",
                                     headers={"Content-Type": "application/json",
                                              "Authorization": f"Bearer {self.bearer(fresh)}"})
        t = time.time()
        try:
            with urllib.request.urlopen(req, timeout=self.a.timeout) as x:
                status, resp = x.status, json.loads(x.read())
        except urllib.error.HTTPError as e:
            status, resp = e.code, {}
        except Exception as e:
            return 503, {"error": {"code": "unavailable", "message": type(e).__name__}}, None
        ms = int((time.time() - t) * 1000)
        if status != 200:
            return 503, {"error": {"code": "unavailable", "message": f"vertex_{status}"}}, {"ms": ms, "status": status}
        um = resp.get("usageMetadata") or {}
        tin = um.get("promptTokenCount") or 0
        tout = (um.get("candidatesTokenCount") or 0) + (um.get("thoughtsTokenCount") or 0)
        usd = tin * USD_PER_M_IN / 1e6 + tout * USD_PER_M_OUT / 1e6
        log = {"ms": ms, "status": 200, "in": tin, "out": tout, "thoughts": um.get("thoughtsTokenCount"),
               "usd": round(usd, 6), "questions": len(questions)}
        with self.lock:
            self.usd += usd
            self.calls += 1
            log["usd_total"] = round(self.usd, 4)
        parts = ((resp.get("candidates") or [{}])[0].get("content") or {}).get("parts") or []
        text = "".join(p.get("text", "") for p in parts if p.get("thought") is not True and isinstance(p.get("text"), str))
        try:
            answers = vertex_answers(questions, json.loads(text))
        except Exception as e:
            log["invalid"] = str(e)[:80]
            return 503, {"error": {"code": "unavailable", "message": "invalid vertex answer"}}, log
        out = {"model": self.a.model, "answers": answers,
               "usage": {"input_tokens": um.get("promptTokenCount"), "output_tokens": um.get("candidatesTokenCount")},
               "service": {"door": "vertex", "version": "vertex_door.py", "model": self.a.model},
               "latency_ms": ms}
        return 200, out, log


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--port", type=int, default=18199)
    ap.add_argument("--project", default="openagentsgemini")
    ap.add_argument("--location", default="global")
    ap.add_argument("--model", default="gemini-3.8-flash")
    ap.add_argument("--timeout", type=float, default=25.0, help="the gateway's whole-decision deadline")
    ap.add_argument("--budget-usd", type=float, default=4.0)
    ap.add_argument("--log", required=True)
    ap.add_argument("--cache", help="directory of answers by request digest")
    a = ap.parse_args()
    door = Door(a)

    class H(BaseHTTPRequestHandler):
        def log_message(self, *args):
            pass

        def reply(self, status, body):
            data = compact(body).encode()
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)

        def do_GET(self):
            self.reply(200, {"status": "ok", "usd": round(door.usd, 4), "calls": door.calls,
                                  "cache_hits": door.hits})

        def do_POST(self):
            if not self.path.rstrip("/").endswith("/v1/systemone"):
                return self.reply(404, {"error": {"code": "not_found"}})
            body = json.loads(self.rfile.read(int(self.headers.get("Content-Length") or 0)) or b"{}")
            key = None
            if a.cache:
                key = hashlib.sha256(compact({"state": body.get("state"),
                                              "questions": body.get("questions")}).encode()).hexdigest()
                path = os.path.join(a.cache, key + ".json")
                if os.path.exists(path):
                    door.hits += 1
                    return self.reply(200, json.load(open(path)))
            if door.usd >= a.budget_usd:
                return self.reply(402, {"error": {"code": "payment_required", "message": "vertex_door budget spent"}})
            status, out, log = door.ask(body)
            if log is not None:
                with door.lock, open(a.log, "a") as fh:
                    fh.write(json.dumps(dict(log, at=round(time.time(), 3))) + "\n")
            if key and status == 200:
                with open(os.path.join(a.cache, key + ".json"), "w") as fh:
                    fh.write(compact(out))
            self.reply(status, out)

    if a.cache:
        os.makedirs(a.cache, exist_ok=True)
    srv = ThreadingHTTPServer(("127.0.0.1", a.port), H)
    print(f"vertex door on 127.0.0.1:{a.port} -> {a.model} ({a.project}/{a.location}), budget ${a.budget_usd}",
          flush=True)
    srv.serve_forever()


if __name__ == "__main__":
    main()
