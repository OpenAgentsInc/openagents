#!/usr/bin/env python3
"""One POST /v1/systemone call, standard library only.

Reads OPENAGENTS_BASE_URL and OPENAGENTS_API_KEY from the environment —
the key stays out of every command line. Run:

    python3 ask.py "I was charged twice on the March invoice."
"""

import json
import os
import sys
import urllib.error
import urllib.request

QUESTIONS = {
    "refund": {
        "type": "noul",
        "instructions": "Does the customer ask for money back?",
    },
    "department": {
        "type": "choice",
        "instructions": "Which team should handle this request?",
        "criteria": {
            "billing": "Charges, invoices, and refunds",
            "technical": "Bugs and outages",
            "none": "No team fits this request",
        },
    },
}


def main() -> int:
    base = os.environ.get("OPENAGENTS_BASE_URL", "").rstrip("/")
    key = os.environ.get("OPENAGENTS_API_KEY", "")
    if not base or not key:
        print("set OPENAGENTS_BASE_URL and OPENAGENTS_API_KEY", file=sys.stderr)
        return 2
    state = sys.argv[1] if len(sys.argv) > 1 else "I was charged twice."

    body = json.dumps(
        {"model": "shared-kev", "state": state, "questions": QUESTIONS}
    ).encode()
    request = urllib.request.Request(
        f"{base}/v1/systemone",
        data=body,
        headers={
            "authorization": f"Bearer {key}",
            "content-type": "application/json",
            "idempotency-key": "example-ask-1",
            "x-attempt": "1",
        },
        method="POST",
    )
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            payload = json.loads(response.read())
    except urllib.error.HTTPError as error:
        # Errors are typed: {"error": {"code", "message"}}.
        print(error.read().decode() or str(error), file=sys.stderr)
        return 3 if error.code < 500 else 4

    for name, answer in payload["answers"].items():
        print(f"{name}: {json.dumps(answer)}")
    print(f"model: {payload['model']}  usage: {json.dumps(payload.get('usage'))}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
