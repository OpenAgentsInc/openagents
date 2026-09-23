"""Freeze the five-option program selection: v2's items plus review-runs.

Every v2 item is kept with its label, partition, and id, locked ones
included: v3 asks a new question, so v2's locked items are unread for it,
and none of them asks about runs. The new authored items cover review-runs
and the run-adjacent requests that must stay none.
"""

import hashlib
import json
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent


def build():
    previous = json.loads((HERE / "program-selection-v2.json").read_text())
    judgments = json.loads((HERE / "program-selection-v3-judgments.json").read_text())
    items = list(previous["items"])
    for item in judgments["authored"]:
        items.append({
            "id": "authored-v3/" + item["id"],
            "family": "program", "kind": "choice",
            "state": {"request": item["request"]},
            "truth": item["truth"], "partition": item["partition"],
            "label_source": "author", "label_rule": judgments["rule"],
        })
    assert len({item["id"] for item in items}) == len(items)
    assert len({json.dumps(item["state"], sort_keys=True) for item in items}) == len(items)
    suite = {
        "schema": "openagents.gym.suite.v1", "name": "program-selection-v3",
        "created": "2026-09-23", "tier": "scored", "gate": "probability-v2",
        "questions": "program-selection-v3",
        "description": (
            "The production five-option program question, with review-runs added "
            "for questions about Terminal-Bench runs. v2's items keep their labels; "
            "new authored requests measure recognition of run questions and the "
            "run-adjacent requests that must stay none. Labels are one author's "
            "readings, with disputes retained separately."
        ),
        "sampling": {
            "source_suite_digest": previous["digest"],
            "v2_items": len(previous["items"]),
            "new_authored": len(judgments["authored"]),
            "label_review": "program-selection-v3-judgments.json",
        },
        "items": items,
    }
    canonical = json.dumps(items, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
    suite["digest"] = hashlib.sha256(canonical.encode()).hexdigest()
    return suite


if __name__ == "__main__":
    json.dump(build(), sys.stdout, indent=2, ensure_ascii=False)
    print()
