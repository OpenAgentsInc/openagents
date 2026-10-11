#!/usr/bin/env python3
"""A second, independent judge for the briefed-agent bench (#11309).

    judge2.py judge --tag TAG [--model gemini-3.1-pro-preview] [--workers 8]
    judge2.py report --tag TAG --base A --cand B0

`judge` scores every trial of TAG with Gemini on Vertex, using ab.py's
JUDGE prompt word for word (issue, merged change, candidate diff, nothing
else), temperature 0, and writes `judge2.json` next to the trial's
`judge.json`. A trial with no change scores 1 without a call, as in ab.py.

`report` sets the two judges side by side: acceptance per arm under each,
their agreement (raw and Cohen's kappa) per arm, and the cand - base
difference in judge acceptance under each judge and under both (accept
only when both accept), each with the same issue-level bootstrap as ab.py
(10000 resamples, seed 11211).

The access token comes from `gcloud auth print-access-token` and is never
printed.
"""

from __future__ import annotations

import argparse
import json
import random
import subprocess
import threading
import time
import urllib.request
from collections import defaultdict
from pathlib import Path

from ab import JUDGE
from common import WORK, dump_json, load_json
from prepare import TASKS

PROJECT = "openagentsgemini"
# Estimated list price per million tokens (Gemini 3 Pro preview, <= 200k
# prompt); thinking tokens bill as output.
USD_PER_M_IN, USD_PER_M_OUT = 2.0, 12.0


class Token:
    def __init__(self):
        self.lock, self.value, self.at = threading.Lock(), "", 0.0

    def get(self) -> str:
        with self.lock:
            if time.time() - self.at > 1500:
                self.value = subprocess.run(["gcloud", "auth", "print-access-token"], capture_output=True,
                                            text=True, check=True).stdout.strip()
                self.at = time.time()
            return self.value


def ask(token: Token, model: str, location: str, prompt: str) -> dict:
    host = "https://aiplatform.googleapis.com" if location == "global" else f"https://{location}-aiplatform.googleapis.com"
    url = f"{host}/v1/projects/{PROJECT}/locations/{location}/publishers/google/models/{model}:generateContent"
    body = {
        "systemInstruction": {"parts": [{"text": "You review code changes and answer only with the JSON asked for."}]},
        "contents": [{"role": "user", "parts": [{"text": prompt}]}],
        "generationConfig": {
            "temperature": 0,
            "maxOutputTokens": 8192,
            "responseMimeType": "application/json",
            "responseJsonSchema": {
                "type": "object",
                "properties": {"score": {"type": "integer"}, "accept": {"type": "boolean"}, "reason": {"type": "string"}},
                "required": ["score", "accept", "reason"],
            },
        },
    }
    for attempt in range(6):
        req = urllib.request.Request(url, data=json.dumps(body).encode(), method="POST", headers={
            "Authorization": f"Bearer {token.get()}", "Content-Type": "application/json"})
        try:
            with urllib.request.urlopen(req, timeout=300) as r:
                out = json.loads(r.read())
            text = "".join(p.get("text", "") for p in out["candidates"][0]["content"]["parts"] if not p.get("thought"))
            verdict = json.loads(text[text.find("{"): text.rfind("}") + 1])
            u = out.get("usageMetadata", {})
            tin = u.get("promptTokenCount", 0)
            tout = u.get("candidatesTokenCount", 0) + u.get("thoughtsTokenCount", 0)
            verdict.update({"model": model, "tokens_in": tin, "tokens_out": tout,
                            "cost_usd_est": round((tin * USD_PER_M_IN + tout * USD_PER_M_OUT) / 1e6, 5)})
            return verdict
        except Exception as error:  # noqa: BLE001 - network, HTTP or a malformed answer: retry
            last = str(error)
            time.sleep(min(60, 5 * 2 ** attempt))
    return {"error": last[-500:], "model": model}


def judge(args) -> None:
    token = Token()
    dirs = sorted(p for p in (WORK / "results" / args.tag).iterdir() if (p / "result.json").exists())
    pending = [d for d in dirs if not ((d / "judge2.json").exists() and "error" not in load_json(d / "judge2.json"))]
    lock = threading.Lock()

    def worker():
        while True:
            with lock:
                if not pending:
                    return
                d = pending.pop()
            r = load_json(d / "result.json")
            candidate = (d / "change.patch").read_text() if (d / "change.patch").exists() else ""
            if not candidate.strip():
                v = {"score": 1, "accept": False, "reason": "no change", "model": args.model, "cost_usd_est": 0}
            else:
                task = load_json(TASKS / str(r["issue"]) / "task.json")
                tdir = TASKS / str(r["issue"])
                reference = (tdir / "source.patch").read_text() + (tdir / "tests.patch").read_text()
                prompt = JUDGE.format(issue=task["issue"], title=task["title"], body=(task["body"] or "")[:6000],
                                      reference=reference[:30000], candidate=candidate[:30000])
                v = ask(token, args.model, args.location, prompt)
            dump_json(d / "judge2.json", v)
            print(d.name, v.get("score"), v.get("accept"), v.get("error", "")[:80], flush=True)

    threads = [threading.Thread(target=worker) for _ in range(args.workers)]
    for t in threads:
        t.start()
    for t in threads:
        t.join()


def rows(tag: str) -> list[dict]:
    out = []
    for d in sorted((WORK / "results" / tag).iterdir()):
        if not (d / "result.json").exists():
            continue
        r = load_json(d / "result.json")
        j1 = load_json(d / "judge.json") if (d / "judge.json").exists() else {}
        j2 = load_json(d / "judge2.json") if (d / "judge2.json").exists() else {}
        if "accept" not in j1 or "accept" not in j2:
            continue
        out.append({"issue": r["issue"], "arm": r["arm"], "j1": bool(j1["accept"]), "j2": bool(j2["accept"]),
                    "s1": j1.get("score"), "s2": j2.get("score"), "tests": bool(r.get("tests_pass")),
                    "cost2": j2.get("cost_usd_est") or 0, "name": d.name})
    return out


def kappa(pairs: list[tuple[bool, bool]]) -> float:
    n = len(pairs)
    if not n:
        return float("nan")
    po = sum(a == b for a, b in pairs) / n
    p1, p2 = sum(a for a, _ in pairs) / n, sum(b for _, b in pairs) / n
    pe = p1 * p2 + (1 - p1) * (1 - p2)
    return (po - pe) / (1 - pe) if pe < 1 else 1.0


def diff_ci(rs: list[dict], base: str, cand: str, key, seed=11211, n_boot=10000):
    by = defaultdict(lambda: defaultdict(list))
    for r in rs:
        by[r["issue"]][r["arm"]].append(key(r))
    issues = [i for i in by if by[i][base] and by[i][cand]]

    def d(iss):
        return sum(sum(by[i][cand]) / len(by[i][cand]) - sum(by[i][base]) / len(by[i][base]) for i in iss) / len(iss)

    rng = random.Random(seed)
    boots = sorted(d([rng.choice(issues) for _ in issues]) for _ in range(n_boot))
    rate = lambda arm: sum(sum(by[i][arm]) / len(by[i][arm]) for i in issues) / len(issues)  # noqa: E731
    return rate(base), rate(cand), d(issues), boots[int(0.025 * n_boot)], boots[int(0.975 * n_boot) - 1], len(issues)


def report(args) -> None:
    rs = rows(args.tag)
    print(f"Trials with both verdicts: {len(rs)}; second judge est. cost ${sum(r['cost2'] for r in rs):.2f}\n")
    print("| Arm | Trials | Judge 1 accepts | Judge 2 accepts | Both accept | Agreement | Cohen's kappa |")
    print("|---|---|---|---|---|---|---|")
    for arm in (args.base, args.cand):
        a = [r for r in rs if r["arm"] == arm]
        n = len(a) or 1
        print(f"| {arm} | {len(a)} | {sum(r['j1'] for r in a) / n:.3f} | {sum(r['j2'] for r in a) / n:.3f} | "
              f"{sum(r['j1'] and r['j2'] for r in a) / n:.3f} | {sum(r['j1'] == r['j2'] for r in a) / n:.3f} | "
              f"{kappa([(r['j1'], r['j2']) for r in a]):.2f} |")
    print(f"\n| Judge acceptance ({args.cand} vs {args.base}) | {args.base} | {args.cand} | Difference | 95% CI | Issues |")
    print("|---|---|---|---|---|---|")
    for label, key in (("judge 1 (Claude Opus 5.5)", lambda r: r["j1"]), ("judge 2 (Gemini)", lambda r: r["j2"]),
                       ("both judges", lambda r: r["j1"] and r["j2"]),
                       ("judge 2 and fix tests", lambda r: r["j2"] and r["tests"])):
        b, c, d, lo, hi, n = diff_ci(rs, args.base, args.cand, key)
        print(f"| {label} | {b:.3f} | {c:.3f} | {d:+.3f} | [{lo:+.3f}, {hi:+.3f}] | {n} |")


def main() -> None:
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    j = sub.add_parser("judge")
    j.add_argument("--tag", required=True)
    j.add_argument("--model", default="gemini-3.1-pro-preview")
    j.add_argument("--location", default="global")
    j.add_argument("--workers", type=int, default=8)
    r = sub.add_parser("report")
    r.add_argument("--tag", required=True)
    r.add_argument("--base", default="A")
    r.add_argument("--cand", default="B0")
    a = ap.parse_args()
    judge(a) if a.cmd == "judge" else report(a)


if __name__ == "__main__":
    main()
