#!/usr/bin/env python3
"""Conformance of `POST /v1/decisions` with an OpenAI SDK client
(docs/api/decisions.md, method of docs/inference/2026-10-09-acceptance-run.md).

    scripts/dev/decisions-conformance.py --base-url https://api.openagents.com/v1 \
        [--model openagents/decide] [--json out.json]

The official OpenAI Python SDK (2.48.0, 2026-10-12) has no
`client.decisions` resource yet, so this drives the route through the SDK's
own transport (`client.post(path, body=..., cast_to=object)`): the SDK's
base_url, auth header, retries, and error classes, with the exact documented
request and response shapes. When `client.decisions` exists it is used
instead. Without the `openai` package it falls back to raw HTTP.

Checks: a predicate, a choice, and a score in one request; answers in
request order with every option, probabilities summing to 1, `confidence`
the top probability, `score` the weighted level index; the one error object
for a bad request; and an Idempotency-Key replay. Exit 1 on any failure.
"""

import argparse
import json
import os
import sys
import time
import urllib.error
import urllib.request
import uuid

QUESTIONS = [
    {
        "type": "predicate",
        "name": "money",
        "instructions": "Does the customer say they lost money?",
    },
    {
        "type": "choice",
        "name": "department",
        "instructions": "Which department should handle this complaint?",
        "choices": [
            {"value": "billing", "description": "Payments, invoices, and refunds."},
            {"value": "technical", "description": "Problems using the product."},
            {"value": "shipping", "description": "Delivery and tracking."},
            {"value": "other", "description": "Requests outside these categories."},
        ],
    },
    {
        "type": "score",
        "name": "severity",
        "instructions": "How severe is this issue for the customer?",
        "levels": [
            {"label": "Cosmetic", "description": "Appearance only; nothing lost."},
            {"label": "Workaround available", "description": "Something fails, but another way works."},
            {"label": "Fully blocked", "description": "Something fails with no way around it."},
        ],
    },
]
INPUT = "I was charged twice for my order and the second charge has not been refunded."


class Raw:
    """The same calls without the SDK."""

    def __init__(self, base_url, key):
        self.base_url = base_url.rstrip("/")
        self.key = key

    def post(self, body, headers=None):
        request = urllib.request.Request(
            f"{self.base_url}/decisions",
            data=json.dumps(body).encode(),
            headers={"content-type": "application/json", **(headers or {})},
            method="POST",
        )
        if self.key:
            request.add_header("authorization", f"Bearer {self.key}")
        try:
            with urllib.request.urlopen(request, timeout=60) as response:
                return response.status, json.load(response)
        except urllib.error.HTTPError as error:
            return error.code, json.load(error)


class Sdk:
    """The calls through the OpenAI SDK's transport."""

    def __init__(self, base_url, key):
        import openai

        self.openai = openai
        self.client = openai.OpenAI(base_url=base_url, api_key=key or "none", max_retries=0)
        self.native = hasattr(self.client, "decisions")

    def post(self, body, headers=None):
        try:
            if self.native:
                answer = self.client.decisions.create(**body, extra_headers=headers or {})
                return 200, answer.model_dump()
            return 200, self.client.post(
                "/decisions", body=body, cast_to=object, options={"headers": headers or {}}
            )
        except self.openai.APIStatusError as error:
            return error.status_code, error.response.json()


def check(name, ok, detail, results):
    results.append({"check": name, "ok": bool(ok), "detail": detail})
    print(f"{'PASS' if ok else 'FAIL'}  {name}  {detail}")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--base-url", default=os.environ.get("OPENAGENTS_BASE_URL", "http://127.0.0.1:8790/v1"))
    parser.add_argument("--model", default="openagents/decide")
    parser.add_argument("--json")
    parser.add_argument("--raw", action="store_true", help="skip the SDK")
    args = parser.parse_args()
    key = os.environ.get("OPENAGENTS_API_KEY", "")
    try:
        caller = Raw(args.base_url, key) if args.raw else Sdk(args.base_url, key)
        how = "raw HTTP" if args.raw else ("openai.decisions" if caller.native else f"openai {caller.openai.__version__} client.post")
    except ImportError:
        caller, how = Raw(args.base_url, key), "raw HTTP (no openai package)"
    print(f"{args.base_url} model {args.model} via {how}")
    results = []

    body = {"model": args.model, "input": INPUT, "questions": QUESTIONS, "metadata": {"run": "conformance"}}
    started = time.time()
    status, answer = caller.post(body)
    ms = round((time.time() - started) * 1000)
    check("status 200", status == 200, f"{status} in {ms} ms", results)
    if status == 200:
        check("id and object", answer.get("id", "").startswith("dec_") and answer.get("object") == "decision",
              f"{answer.get('id')} {answer.get('object')}", results)
        names = [a.get("name") for a in answer.get("answers", [])]
        check("answers in request order", names == [q["name"] for q in QUESTIONS], str(names), results)
        by = {a["name"]: a for a in answer.get("answers", [])}
        p = by.get("money", {}).get("probability")
        check("predicate probability", isinstance(p, (int, float)) and 0 <= p <= 1, f"{p}", results)
        for name, key_ in (("department", "value"), ("severity", "label")):
            a = by.get(name, {})
            probs = a.get("probabilities", [])
            total = sum(x["probability"] for x in probs)
            want = [c.get("value", c.get("label")) for c in next(q for q in QUESTIONS if q["name"] == name).get("choices", next(q for q in QUESTIONS if q["name"] == name).get("levels", []))]
            got = [x[key_] for x in probs]
            check(f"{name} lists every option in order", got == want, str(got), results)
            check(f"{name} sums to 1", abs(total - 1) <= 1e-6, f"{total:.9f}", results)
            top = max((x["probability"] for x in probs), default=None)
            check(f"{name} confidence is the top probability", top is not None and abs(a.get("confidence", -1) - top) < 1e-9,
                  f"{a.get('confidence')}", results)
        dep = by.get("department", {})
        check("choice picks the top option", dep.get("choice") == max(dep.get("probabilities", [{}]), key=lambda x: x.get("probability", 0)).get("value"),
              f"{dep.get('choice')}", results)
        sev = by.get("severity", {})
        weighted = sum(x["value"] * x["probability"] for x in sev.get("probabilities", []))
        check("score is the weighted level index", abs(sev.get("score", -1) - weighted) < 1e-6, f"{sev.get('score')}", results)
        check("usage.input_tokens", isinstance(answer.get("usage", {}).get("input_tokens"), int),
              f"{answer.get('usage')}", results)
        check("metadata echoed", answer.get("metadata") == {"run": "conformance"}, f"{answer.get('metadata')}", results)
        oa = answer.get("openagents", {})
        check("openagents.door and latency", oa.get("door") is not None and isinstance(oa.get("latency_ms"), int),
              f"door {oa.get('door')} model {answer.get('model')} latency {oa.get('latency_ms')} ms", results)

    bad = {"model": args.model, "input": INPUT, "questions": [{"type": "choice", "name": "x", "instructions": "?", "choices": [{"value": "only"}]}]}
    status, error = caller.post(bad)
    e = error.get("error", {}) if isinstance(error, dict) else {}
    check("bad request is 400 with the one error object",
          status == 400 and e.get("type") == "invalid_request" and e.get("code") == "invalid_request" and "param" in e,
          f"{status} {e.get('code')} param={e.get('param')}", results)

    key_ = f"conformance-{uuid.uuid4()}"
    s1, first = caller.post(body, {"Idempotency-Key": key_})
    s2, again = caller.post(body, {"Idempotency-Key": key_})
    check("Idempotency-Key replays the first answer", s1 == s2 == 200 and first.get("id") == again.get("id"),
          f"{first.get('id')} / {again.get('id')}", results)
    other = dict(body, input="Something else entirely.")
    s3, conflict = caller.post(other, {"Idempotency-Key": key_})
    check("another body under the same key is 409", s3 == 409 and conflict.get("error", {}).get("code") == "idempotency_conflict",
          f"{s3}", results)

    failed = [r for r in results if not r["ok"]]
    print(f"{len(results) - len(failed)}/{len(results)} passed")
    if args.json:
        with open(args.json, "w") as out:
            json.dump({"base_url": args.base_url, "model": args.model, "via": how, "results": results}, out, indent=2)
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
