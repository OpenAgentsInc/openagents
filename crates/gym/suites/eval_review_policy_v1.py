#!/usr/bin/env python3
"""Runs the review-policy-eval-v1 suite through `POST /v1/classify` twice.

Each of the 20 items is one classify call — one input, one unit, the
item's typed question under the mode its kind maps to (`noul` ->
`binary`, `choice` -> `single-label`, `score` -> `score`). The suite
runs once under the plain selection policy and once under the same
policy plus a declared `review` block (`trigger: uncertain` by default),
so every case records the primary answer and — when the declared
`uncertain_below` cut flags it — the reviewer's independent re-judgment.
The run measures the review policy, not the model: under `uncertain`, a
confident error the gate never flagged is never dispatched to the
reviewer, and the summary reports that gap rather than hiding it.

    OPENAGENTS_API_KEY=oak_... OPENAGENTS_BASE_URL=http://127.0.0.1:8080 \
    python3 crates/gym/suites/eval_review_policy_v1.py OUT_DIR \
        --model shared-kev --capacity shared

OUT_DIR receives `raw.jsonl` — one JSON object per exchange — and
`results.json`, the summary. `--check` validates the suite and exits
without calling the door. `--reviewer` defaults to `--model`: a second
read through the same door still exercises the machinery, while a
distinct reviewer door is the stronger measurement. Credentials come
from `OPENAGENTS_API_KEY`, the endpoint from `OPENAGENTS_BASE_URL`, and
`OPENAGENTS_WORKSPACE` becomes `X-Workspace-Id` — never argv.
"""

import argparse
import hashlib
import json
import os
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
SUITE = ROOT / "crates/gym/suites/review-policy-eval-v1.json"
BASE = os.environ.get("OPENAGENTS_BASE_URL", "http://127.0.0.1:8080").rstrip("/")
WORKSPACE = os.environ.get("OPENAGENTS_WORKSPACE")
UNKNOWN = "unknown"

KIND_MODE = {"noul": "binary", "choice": "single-label", "score": "score"}
PARTITIONS = {"answerable", "ambiguous", "near-threshold"}
FLAGS = {"confident", "uncertain", "either"}
RETRYABLE = {408, 429, 500, 502, 503, 504}


def digest(value):
    blob = json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(blob).hexdigest()


def load_suite(path):
    suite = json.load(open(path))
    got = digest(suite["items"])
    if suite.get("digest") != got:
        sys.exit(f"suite digest mismatch: file says {suite.get('digest')}, items hash to {got}")
    return suite


def resolve(item, suite):
    """Fill an item's `labels` or `levels` from the suite's shared `sets`."""
    if "labels" not in item and "levels" not in item:
        pool = (suite.get("sets") or {}).get(item.get("set")) or []
        item["labels" if item["kind"] != "score" else "levels"] = pool


def check_item(item):
    kind, labels, levels = item["kind"], item.get("labels", []), item.get("levels", [])
    assert kind in KIND_MODE, item["id"]
    assert item["partition"] in PARTITIONS and item["flag"] in FLAGS, item["id"]
    assert item.get("label_source") == "author" and item.get("label_rule"), item["id"]
    assert "state" in item, item["id"]
    if kind == "noul":
        assert len(labels) == 1 and not levels, item["id"]
        assert item["expected"] in ("yes", "no"), item["id"]
    elif kind == "choice":
        ids = [label["id"] for label in labels]
        assert len(ids) >= 2 and len(set(ids)) == len(ids) and not levels, item["id"]
        assert item["expected"] is None or item["expected"] in ids, item["id"]
    else:
        expected = item["expected"]
        assert 2 <= len(levels) <= 10 and not labels, item["id"]
        assert expected is None or (
            isinstance(expected, int) and 0 <= expected < len(levels)
        ), item["id"]


def policy_doc(cut, review=None):
    doc = {
        "v": "openagents.classify-policy.v1",
        "name": "review-policy-eval-v1",
        "select": {
            "single_label": {"ties": "no-match", "no_match": {"kind": "null"},
                             "uncertain_below": cut},
            "binary": {"threshold": 0.5, "uncertain_below": cut},
            "score": {"order": "descending", "uncertain_below": cut},
        },
    }
    if review is not None:
        doc["review"] = review
    return doc


def review_block(reviewer, trigger, latency_ms):
    return {"v": "openagents.classify-review.v1", "reviewer": reviewer,
            "trigger": trigger, "on_failure": "keep-original",
            "max_items": 1, "max_attempts": 1, "latency_ms": latency_ms}


def envelope(item, model, capacity, policy):
    env = {
        "v": "openagents.classify.v1",
        "model": model,
        "capacity": capacity,
        "policy": policy,
        "inputs": [{"id": item["id"], "text": item["state"]}]
        if isinstance(item["state"], str)
        else [{"id": item["id"], "record": item["state"]}],
        "mode": KIND_MODE[item["kind"]],
    }
    if item.get("instructions"):
        env["instructions"] = item["instructions"]
    env["levels" if item["kind"] == "score" else "labels"] = (
        item["levels"] if item["kind"] == "score" else item["labels"]
    )
    return env


def post(body, key, request_id, attempt, timeout):
    headers = {"Authorization": f"Bearer {key}", "Content-Type": "application/json",
               "Idempotency-Key": request_id, "X-Attempt": str(attempt)}
    if WORKSPACE:
        headers["X-Workspace-Id"] = WORKSPACE
    req = urllib.request.Request(BASE + "/v1/classify", data=json.dumps(body).encode(),
                                 headers=headers, method="POST")
    started = time.monotonic()
    latency = lambda: int((time.monotonic() - started) * 1000)
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            return resp.status, json.loads(resp.read()), None, latency(), None
    except urllib.error.HTTPError as e:
        detail = e.read().decode(errors="replace")[:400]
        try:
            parsed = json.loads(detail)
        except ValueError:
            parsed = None
        return e.code, parsed, f"HTTP {e.code}: {detail}", latency(), e.headers.get(
            "Retry-After")
    except Exception as e:
        return None, None, str(e), latency(), None


def call(body, key, request_id, timeout):
    """One classify call with one bounded retry, the eval's own loop."""
    rec = {"attempts_sent": 0, "latency_ms": 0}
    for attempt in (1, 2):
        rec["attempts_sent"] += 1
        status, response, error, latency, retry_after = post(
            body, key, request_id, attempt, timeout)
        rec["latency_ms"] += latency
        if error is None or (status is not None and status not in RETRYABLE):
            break
        try:
            time.sleep(min(float(retry_after), 60.0) if retry_after else 3.0)
        except ValueError:
            time.sleep(3.0)
    rec.update(status=status, response=response, error=error)
    return rec


def dig(response, *path):
    """One response's nested value, or None when any step is missing."""
    node = response
    for key in path:
        if isinstance(node, list) and isinstance(key, int):
            node = node[key] if key < len(node) else None
        elif isinstance(node, dict):
            node = node.get(key)
        else:
            return None
        if node is None:
            return None
    return node


def first_unit(response):
    return dig(response or {}, "results", 0, "units", 0)


def expected_selected(item):
    if item["kind"] == "noul":
        return item["labels"][0]["id"] if item["expected"] == "yes" else None
    return item["expected"]


def evidence(unit):
    """The winning-side probability the unit's own `uncertain` flag read."""
    raw = unit.get("raw") if isinstance(unit, dict) else None
    if not isinstance(raw, dict):
        return None
    if unit.get("mode") == "binary":
        p = raw.get("noul")
        return max(p, 1.0 - p) if isinstance(p, (int, float)) else None
    probs = raw.get("probabilities")
    return max(probs.values()) if isinstance(probs, dict) and probs else None


def reading(item, unit):
    """The fields a case record keeps off one unit result."""
    unit = unit if isinstance(unit, dict) else {}
    selected = unit.get("selected")
    return {
        "outcome": unit.get("outcome", UNKNOWN),
        "selected": selected,
        "uncertain": unit.get("uncertain") is True,
        "no_match": unit.get("no_match") is True,
        "evidence": evidence(unit),
        "correct": unit.get("outcome") == "answered"
        and selected == expected_selected(item),
    }


def review_reading(item, unit):
    review = unit.get("review") if isinstance(unit, dict) else None
    if not isinstance(review, dict):
        return None
    return {
        "reason": review.get("reason"),
        "outcome": review.get("outcome", UNKNOWN),
        "selected": review.get("selected"),
        "changed": review.get("changed") is True,
        "uncertain": review.get("uncertain") is True,
        "correct": review.get("outcome") == "answered"
        and review.get("selected") == expected_selected(item),
        "latency_ms": review.get("latency_ms"),
    }


def secondary_counts(response):
    """Attempt counts by role off one classify response's items."""
    counts = {"primary": 0, "review": 0, "fallback": 0}
    for item in (response or {}).get("results") or []:
        for attempt in item.get("attempts") or []:
            role = attempt.get("role", "primary")
            counts[role] = counts.get(role, 0) + 1
    return counts


def token_sum(usages, key):
    """A counter summed only when every call reported it, else unknown."""
    total = 0
    for usage in usages:
        value = usage.get(key) if isinstance(usage, dict) else None
        if not isinstance(value, int):
            return UNKNOWN
        total += value
    return total


def run_pass(items, policy, key, args, run, out):
    """One pass over the suite: one classify call per item."""
    rows = []
    for item in items:
        body = envelope(item, args.model, args.capacity, policy)
        request_id = f"review-policy-eval-v1:{args.run_id}:{run}:{item['id']}"
        result = call(body, key, request_id, args.timeout)
        row = {"run": run, "item": item["id"], "request": body,
               "http_status": result["status"], "response": result["response"],
               "error": result["error"], "latency_ms": result["latency_ms"],
               "attempts_sent": result["attempts_sent"]}
        rows.append(row)
        print(json.dumps(row), file=out, flush=True)
        unit = first_unit(result["response"])
        note = (f"selected={unit.get('selected')} uncertain={unit.get('uncertain')}"
                if isinstance(unit, dict)
                else f"error={result['error'] or result['status']}")
        print(f"{run} {item['id']}: {note}", file=sys.stderr)
        if args.sleep:
            time.sleep(args.sleep)
    return rows


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("out_dir", nargs="?", help="directory for raw.jsonl and results.json")
    parser.add_argument("--suite", default=str(SUITE), help="suite JSON path")
    parser.add_argument("--model", default=os.environ.get("OPENAGENTS_MODEL"))
    parser.add_argument("--capacity", default=os.environ.get("OPENAGENTS_CAPACITY", "shared"))
    parser.add_argument("--reviewer", default=None, help="review door; defaults to --model")
    parser.add_argument("--uncertain-below", type=float, default=0.9,
                        help="the declared review cut on every mode's rule")
    parser.add_argument("--review-trigger", default="uncertain",
                        choices=["uncertain", "no-match", "always"])
    parser.add_argument("--review-latency-ms", type=int, default=30000)
    parser.add_argument("--timeout", type=float, default=120.0)
    parser.add_argument("--sleep", type=float, default=0.0, help="pause between calls")
    parser.add_argument("--run-id", default=str(int(time.time())),
                        help="nonce inside each Idempotency-Key")
    parser.add_argument("--check", action="store_true",
                        help="validate the suite and exit without calling the door")
    args = parser.parse_args()

    suite = load_suite(args.suite)
    items = suite["items"]
    for item in items:
        resolve(item, suite)
        check_item(item)

    if args.check:
        kinds = {k: sum(1 for i in items if i["kind"] == k) for k in KIND_MODE}
        parts = {p: sum(1 for i in items if i["partition"] == p) for p in PARTITIONS}
        print(f"{suite['name']}: {len(items)} items, digest ok")
        print(f"kinds: {kinds}  partitions: {parts}")
        return

    if not args.out_dir:
        parser.error("out_dir is required unless --check")
    key = os.environ.get("OPENAGENTS_API_KEY")
    if not key:
        sys.exit("OPENAGENTS_API_KEY is not set")
    if not args.model:
        sys.exit("--model or OPENAGENTS_MODEL is required")
    if not 0.0 <= args.uncertain_below <= 1.0:
        sys.exit("--uncertain-below must be a probability")
    reviewer = args.reviewer or args.model

    block = review_block(reviewer, args.review_trigger, args.review_latency_ms)
    baseline = policy_doc(args.uncertain_below)
    reviewed = policy_doc(args.uncertain_below, block)
    digests = {name: f"sha256:{digest(doc)}"
               for name, doc in (("baseline", baseline), ("review", reviewed))}

    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    raw = open(out_dir / "raw.jsonl", "w")
    started = time.monotonic()
    runs = {
        "baseline": run_pass(items, baseline, key, args, "baseline", raw),
        "review": run_pass(items, reviewed, key, args, "review", raw),
    }
    raw.close()
    wall_ms = int((time.monotonic() - started) * 1000)

    cases = []
    totals = {
        "answered": {"baseline": 0, "review": 0},
        "correct": {"baseline": 0, "review_final": 0},
        "uncertain": {"baseline": 0, "review_primary": 0},
        "confident_errors": 0, "review_triggered": 0, "review_answered": 0,
        "review_changed": 0, "wrong_reviewed": 0, "caught_by_review": 0,
        "high_confidence_wrong_caught": 0, "rescued": 0, "regressed": 0,
        "attempts": {"primary": 0, "review": 0, "fallback": 0}, "calls_sent": 0,
    }
    usages, review_usages, reserved = [], [], []
    gateway_digests = set()

    for item, base_row, rev_row in zip(items, runs["baseline"], runs["review"]):
        totals["calls_sent"] += base_row["attempts_sent"] + rev_row["attempts_sent"]
        base_resp, rev_resp = base_row["response"], rev_row["response"]
        for resp in (base_resp, rev_resp):
            if isinstance(resp, dict):
                for role, n in secondary_counts(resp).items():
                    totals["attempts"][role] += n
            usage = resp.get("usage") if isinstance(resp, dict) else None
            usages.append(usage if isinstance(usage, dict) else None)
        sub_usage, spend = None, None
        if isinstance(rev_resp, dict):
            summary = rev_resp.get("review")
            if isinstance(summary, dict):
                if isinstance(summary.get("policy_digest"), str):
                    gateway_digests.add(summary["policy_digest"])
                spend = summary.get("reserved_spend")
            usage = rev_resp.get("usage")
            if isinstance(usage, dict) and isinstance(usage.get("review"), dict):
                sub_usage = usage["review"]
        reserved.append(spend if isinstance(spend, int) else None)
        review_usages.append(sub_usage)

        # Post-review the unit already holds the final selection; the
        # primary's whole result survives under `original` when a review
        # rewrote it, and stays the unit itself when nothing triggered.
        base = reading(item, first_unit(base_resp))
        rev_unit = first_unit(rev_resp)
        final = reading(item, rev_unit)
        original = rev_unit.get("original") if isinstance(rev_unit, dict) else None
        primary = reading(item, original) if isinstance(original, dict) else dict(final)
        review = review_reading(item, rev_unit)

        totals["answered"]["baseline"] += base["outcome"] == "answered"
        totals["answered"]["review"] += final["outcome"] == "answered"
        totals["correct"]["baseline"] += base["correct"] is True
        totals["correct"]["review_final"] += final["correct"] is True
        totals["uncertain"]["baseline"] += base["uncertain"]
        totals["uncertain"]["review_primary"] += primary["uncertain"]
        if base["outcome"] == "answered" and not base["correct"] and not base["uncertain"]:
            totals["confident_errors"] += 1
        if review is not None:
            totals["review_triggered"] += 1
            totals["wrong_reviewed"] += not primary["correct"]
            if review["outcome"] == "answered":
                totals["review_answered"] += 1
                totals["review_changed"] += review["changed"]
                if not primary["correct"] and review["correct"]:
                    totals["caught_by_review"] += 1
                    if isinstance(primary["evidence"], (int, float)) and (
                        primary["evidence"] >= args.uncertain_below
                    ):
                        totals["high_confidence_wrong_caught"] += 1
            totals["rescued"] += not primary["correct"] and final["correct"]
            totals["regressed"] += primary["correct"] and not final["correct"]

        cases.append({
            "id": item["id"], "kind": item["kind"], "partition": item["partition"],
            "expected": item["expected"], "flag": item["flag"],
            "baseline": base,
            "reviewed_run": {
                "primary": primary,
                "review": review,
                "final": {"selected": final["selected"], "correct": final["correct"],
                          "final_source": rev_unit.get("final_source")
                          if isinstance(rev_unit, dict) else None},
                "review_status": (dig(rev_resp, "results", 0) or {}).get(
                    "review_status", UNKNOWN),
            },
            "latency_ms": {"baseline_call": base_row["latency_ms"],
                           "review_call": rev_row["latency_ms"],
                           "review_dispatch": review["latency_ms"] if review else None},
            "errors": [e for e in (base_row["error"], rev_row["error"]) if e],
        })

    answered_b = totals["answered"]["baseline"]
    answered_p = totals["answered"]["review"]
    errors_b = answered_b - totals["correct"]["baseline"]
    errors_f = answered_p - totals["correct"]["review_final"]

    results = {
        "schema": "openagents.gym.review-policy-eval.v1",
        "suite": {"name": suite["name"], "digest": suite["digest"], "items": len(items)},
        "created": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "door": {"base": BASE, "model": args.model, "capacity": args.capacity,
                 "reviewer": reviewer},
        "uncertain_below": args.uncertain_below,
        "policy_digest": digests["review"],
        "baseline_policy_digest": digests["baseline"],
        "review_block_digest": f"sha256:{digest(block)}",
        "gateway_policy_digest": sorted(gateway_digests) or UNKNOWN,
        "labeling": suite["labeling"],
        "definitions": {
            "confident_errors": "baseline cases answered wrong with no `uncertain` flag at the declared cut",
            "review_coverage": "share of cases the review pass dispatched a re-judgment for",
            "caught_by_review": "reviewed cases whose primary was wrong and whose review answered with the expected selection",
            "high_confidence_wrong_caught": "caught cases whose primary evidence met the cut — under `trigger: uncertain` the pass never sees unflagged units, so this measures what that trigger cannot rescue",
            "rescued": "reviewed cases wrong under the primary and right under the reviewer's final selection",
            "regressed": "reviewed cases right under the primary and wrong under the reviewer's final selection",
        },
        "totals": {
            "cases": len(items),
            "answered": totals["answered"],
            "primary_correct": totals["correct"]["baseline"],
            "final_correct": totals["correct"]["review_final"],
            "uncertain_flagged": totals["uncertain"],
            "confident_errors": totals["confident_errors"],
            "review_coverage": totals["review_triggered"] / len(items) if items else 0,
            "review_triggered": totals["review_triggered"],
            "review_answered": totals["review_answered"],
            "review_changed": totals["review_changed"],
            "wrong_answers_reviewed": totals["wrong_reviewed"],
            "caught_by_review": totals["caught_by_review"],
            "high_confidence_wrong_caught": totals["high_confidence_wrong_caught"],
            "rescued": totals["rescued"],
            "regressed": totals["regressed"],
        },
        "risk_coverage": {
            "primary_error_rate": errors_b / answered_b if answered_b else UNKNOWN,
            "final_error_rate": errors_f / answered_p if answered_p else UNKNOWN,
            "coverage_reviewed": totals["review_triggered"] / len(items) if items else 0,
            "extra_dispatches": totals["attempts"]["review"] + totals["attempts"]["fallback"],
            "errors_rescued": totals["rescued"],
            "errors_regressed": totals["regressed"],
        },
        "attempts": {"http_calls_sent": totals["calls_sent"],
                     "dispatches": totals["attempts"]},
        "latency": {
            "wall_ms": wall_ms,
            "baseline_calls_ms": sum(r["latency_ms"] for r in runs["baseline"]),
            "review_calls_ms": sum(r["latency_ms"] for r in runs["review"]),
            "review_dispatches_ms": sum(
                c["latency_ms"]["review_dispatch"] or 0 for c in cases),
        },
        "cost": {
            "spend": UNKNOWN,
            "reserved_spend_millionths": sum(reserved)
            if reserved and all(isinstance(x, int) for x in reserved) else UNKNOWN,
            "input_tokens": token_sum(usages, "input_tokens"),
            "output_tokens": token_sum(usages, "output_tokens"),
            "review_input_tokens": token_sum(review_usages, "input_tokens"),
            "review_output_tokens": token_sum(review_usages, "output_tokens"),
        },
        "cases": cases,
    }
    with open(out_dir / "results.json", "w") as f:
        json.dump(results, f, indent=1)
        f.write("\n")
    print(
        f"wrote {out_dir/'raw.jsonl'} and {out_dir/'results.json'}: "
        f"{totals['confident_errors']} confident errors, "
        f"{totals['review_triggered']}/{len(items)} reviewed, "
        f"{totals['rescued']} rescued, {totals['regressed']} regressed",
        file=sys.stderr,
    )


if __name__ == "__main__":
    main()
