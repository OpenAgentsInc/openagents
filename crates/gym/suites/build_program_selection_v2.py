"""Freeze current program selection with open historical and new authored items.

The source's eight locked items are excluded. New locked items are authored
before scoring and must remain unused until a candidate and rule are declared.
"""

import hashlib
import json
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
RULE = (
    "author: choose burn-down only for execution of this checkout's work list "
    "with one delegated session per item; delegate-fan-out for multiple named "
    "tasks delegated in parallel; answer-question for one repository question "
    "explicitly handed to an executor; none for ordinary turns, descriptions, "
    "negated execution, or work outside those admitted program contracts"
)


def build():
    previous = json.loads((HERE / "program-selection-v1.json").read_text())
    judgments = json.loads((HERE / "program-selection-v2-judgments.json").read_text())
    # Preserve the original open labels and partitions. Never recycle a
    # historical locked item into a supposedly clean new locked partition.
    items = [item for item in previous["items"] if item["partition"] != "locked"]
    for item in judgments["authored"]:
        items.append({
            "id": "authored-v2/" + item["id"],
            "family": "program", "kind": "choice",
            "state": {"request": item["request"]},
            "truth": item["truth"], "partition": item["partition"],
            "label_source": "author", "label_rule": RULE,
        })
    assert len({item["id"] for item in items}) == len(items)
    assert len({json.dumps(item["state"], sort_keys=True) for item in items}) == len(items)
    suite = {
        "schema": "openagents.gym.suite.v1", "name": "program-selection-v2",
        "created": "2026-09-20", "tier": "scored", "gate": "probability-v2",
        "questions": "program-selection-v2",
        "description": (
            "The production four-option program question. Historical open real "
            "turns measure spurious selections; authored requests measure recognition, "
            "including burn-down and mentions that do not request execution. "
            "Labels are one author's readings, with disputes retained separately. "
            "The new locked partition contains no historical items."
        ),
        "sampling": {
            "source_suite_digest": previous["digest"],
            "historical_real_open": sum(i["id"].startswith("turn/") for i in items),
            "historical_authored_open": sum(i["id"].startswith("authored/") for i in items),
            "new_authored": len(judgments["authored"]),
            "historical_locked_excluded": 8,
            "label_review": "program-selection-v2-judgments.json",
        },
        "items": items,
    }
    canonical = json.dumps(items, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
    suite["digest"] = hashlib.sha256(canonical.encode()).hexdigest()
    return suite


if __name__ == "__main__":
    json.dump(build(), sys.stdout, indent=2, ensure_ascii=False)
    print()
