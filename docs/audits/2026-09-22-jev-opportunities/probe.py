#!/usr/bin/env python3
"""Validate research requests offline, or explicitly probe one hosted request."""

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import time
import urllib.error
import urllib.request


HERE = Path(__file__).resolve().parent
NAMES = ("evidence", "review", "project")
ENDPOINT = "https://api.typesafe.ai/v1/systemone"
MODEL = "jev-1.13.0"
PRICE_PER_MILLION = 0.042
# Conservative interpretation of the published 64k request limit.
MAX_INPUT_TOKENS = 65536
RESERVE_USD = MAX_INPUT_TOKENS * PRICE_PER_MILLION / 1_000_000


def validate(name):
    value = json.loads((HERE / f"{name}.request.json").read_text())
    if set(value) != {"model", "state", "questions"} or value["model"] != MODEL:
        raise ValueError("The request must use the pinned model and documented top-level fields.")
    if not isinstance(value["state"], (str, list, dict)) or not value["questions"]:
        raise ValueError("The request needs state and questions.")
    for question in value["questions"].values():
        if not isinstance(question.get("instructions"), (str, list, dict)):
            raise ValueError("Every question needs instructions.")
        kind = question.get("type")
        criteria = question.get("criteria")
        if kind == "choice":
            if not isinstance(criteria, dict) or not 1 <= len(criteria) <= 255:
                raise ValueError("Choice requires 1–255 options.")
        elif kind == "score":
            if not isinstance(criteria, list) or not 2 <= len(criteria) <= 10:
                raise ValueError("Score requires 2–10 levels.")
        elif kind != "noul":
            raise ValueError("Unknown primitive.")
    return value


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--live", action="store_true")
    parser.add_argument("--request", choices=NAMES)
    parser.add_argument("--budget-usd", type=float)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    requests = {name: validate(name) for name in NAMES}
    if not args.live:
        print(json.dumps({
            "mode": "offline_validation", "network_calls": 0,
            "requests": {name: len(value["questions"]) for name, value in requests.items()},
            "note": "Valid shapes do not establish model quality or exact token counts.",
        }, indent=2))
        return
    if not args.request or not args.output or args.budget_usd is None:
        parser.error("Live mode requires --request, --budget-usd, and a new --output path.")
    if not math.isfinite(args.budget_usd) or args.budget_usd < RESERVE_USD:
        parser.error(f"Reserve at least ${RESERVE_USD:.6f} for the one allowed attempt.")
    key = os.environ.get("TYPESAFE_API_KEY", "").strip()
    if not key:
        parser.error("Live mode requires TYPESAFE_API_KEY in the environment.")
    value = requests[args.request]
    body = json.dumps(value, ensure_ascii=False).encode()
    record = {
        "kind": "single_request_plumbing_probe_not_quality_evaluation",
        "request": value,
        "request_sha256": hashlib.sha256(body).hexdigest(),
        "endpoint": ENDPOINT,
        "attempts": 1,
        "retries": 0,
        "socket_timeout_seconds": 5,
        "budget_usd": args.budget_usd,
        "reserved_usd": RESERVE_USD,
        "rate_assumption_usd_per_million": PRICE_PER_MILLION,
        "rate_source": "https://docs.typesafe.ai/models",
        "actual_billed_usd": None,
        "note": "Reservation assumes the published input limit and price remain applicable. Unknown usage retains the reservation. This is not a provider billing limit.",
    }
    # Refuse an existing output before making any network request.
    with args.output.open("x") as destination:
        destination.write(json.dumps({**record, "status": "dispatching"}, indent=2) + "\n")
        destination.flush()
        request = urllib.request.Request(
            ENDPOINT, data=body, method="POST",
            headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json"},
        )
        opener = urllib.request.build_opener(NoRedirect())
        started = time.perf_counter()
        try:
            with opener.open(request, timeout=5) as response:
                raw = response.read(2_000_001)
                record["http_status"] = response.status
                record["request_id"] = response.headers.get("x-request-id")
            if len(raw) > 2_000_000:
                raise ValueError("Response exceeds the capture limit.")
            result = json.loads(raw)
            record["response"] = result
            record["status"] = "received"
            usage = result.get("usage", {}).get("input_tokens")
            if isinstance(usage, int) and not isinstance(usage, bool) and usage >= 0:
                record["estimated_usd_at_assumed_rate"] = usage * PRICE_PER_MILLION / 1_000_000
        except urllib.error.HTTPError as error:
            record["status"] = "http_error"
            record["http_status"] = error.code
            record["retry_after"] = error.headers.get("retry-after")
        except (urllib.error.URLError, TimeoutError, ValueError, OSError) as error:
            record["status"] = "unavailable_or_invalid"
            record["error_type"] = type(error).__name__
        record["elapsed_ms"] = (time.perf_counter() - started) * 1000
        destination.seek(0)
        destination.truncate()
        destination.write(json.dumps(record, indent=2) + "\n")
    print(json.dumps({"status": record["status"], "output": str(args.output),
                      "attempts": 1, "actual_billed_usd": None}))


if __name__ == "__main__":
    main()
