#!/usr/bin/env python3
"""Runs the independence-eval-v1 suite under both coder question sets.

Each of the 12 items is asked once per set, with the set's full three
questions, the way the decide step sends them — including the locked item,
which `gym eval` never opens. Output is one JSON object per line: set id,
item id, truth, partition, the door's answers or the error. One retry per
call, then the failure is recorded.

    TYPESAFE_API_KEY=... python3 crates/gym/suites/eval_independence_v1.py \
        docs/coder/measurements/2026-09-21-independence-v2-eval.raw.jsonl
"""

import json
import os
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
BASE = os.environ.get("TYPESAFE_BASE_URL", "https://api.typesafe.ai").rstrip("/")
KEY = os.environ["TYPESAFE_API_KEY"]
MODEL = os.environ.get("TYPESAFE_DEFAULT_MODEL", "jev-latest")


def ask(state, questions):
    body = {"model": MODEL, "state": state, "questions": questions}
    req = urllib.request.Request(
        BASE + "/v1/systemone",
        data=json.dumps(body).encode(),
        headers={
            "Authorization": f"Bearer {KEY}",
            "Content-Type": "application/json",
        },
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=60) as resp:
            return json.loads(resp.read()), None
    except urllib.error.HTTPError as e:
        detail = e.read().decode(errors="replace")[:300]
        return None, f"HTTP {e.code}: {detail}"
    except Exception as e:
        return None, str(e)


def main():
    suite = json.load(open(ROOT / "crates/gym/suites/independence-eval-v1.json"))
    sets = []
    for path in ("questions/independence.json", "questions/independence-v2.json"):
        sets.append(json.load(open(ROOT / path)))

    out = open(sys.argv[1], "w")
    for qset in sets:
        for item in suite["items"]:
            rec = {
                "set": qset["id"],
                "item": item["id"],
                "truth": item["truth"],
                "partition": item["partition"],
            }
            response, error = ask(item["state"], qset["questions"])
            if response is None:
                time.sleep(3)
                response, error = ask(item["state"], qset["questions"])
            if response is None:
                rec["error"] = error
            else:
                rec["model"] = response.get("model")
                rec["answers"] = response.get("answers")
            print(json.dumps(rec), file=out, flush=True)
            print(
                f"{qset['id']} {item['id']}: "
                + (
                    f"independent={rec['answers']['independent'].get('noul')}"
                    if "answers" in rec and "independent" in rec["answers"]
                    else f"error={error}"
                ),
                file=sys.stderr,
            )


if __name__ == "__main__":
    main()
