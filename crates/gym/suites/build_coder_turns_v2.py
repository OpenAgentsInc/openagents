#!/usr/bin/env python3
"""Derives `coder-turns-v2` from the committed `coder-turns-v1` suite.

`coder-turns-v2` asks the two questions a decision in
`crates/coder/src/classify.rs` reads, `action` and `shell_outcome`, and
nothing else. Its items are the v1 items of those two families, byte for
byte: the same states, the same outcome labels, the same partitions. A v2
score on a family is therefore a score on the same items a v1 score used,
and the suite digest moves only because five families are gone.

The v1 suite is built from a session archive that is not in the repository,
so this builder reads the committed v1 file rather than the archive.
Rebuilding v1 and then rerunning this keeps the two in step.

    python3 crates/gym/suites/build_coder_turns_v2.py \
        --from crates/gym/suites/coder-turns-v1.json \
        --out crates/gym/suites/coder-turns-v2.json
"""

import argparse
import hashlib
import json

KEPT = ("action", "shell_outcome")
QUESTIONS = "coder-turns-v2"


def build(source):
    items = [item for item in source["items"] if item["family"] in KEPT]
    dropped = sorted({item["family"] for item in source["items"]} - set(KEPT))
    suite = {
        "schema": source["schema"],
        "name": "coder-turns-v2",
        "created": "2026-09-20",
        "description": (
            "The coder-turns-v1 items of the two families a decision reads, "
            "`action` and `shell_outcome`, asked the same text under the "
            "coder-turns-v2 question set. The five families v1 also asked, "
            + ", ".join("`{}`".format(family) for family in dropped)
            + ", are retired by docs/decision-models/"
            "2026-09-20-coder-question-baselines.md: each scores no better "
            "than its majority class on real turns, and no decision reads "
            "its answer. States, labels, label sources, and partitions are "
            "the v1 items' own, unchanged, so a family's score here is a "
            "score on the same items as under v1."
        ),
        "tier": source["tier"],
        "gate": source["gate"],
        "questions": QUESTIONS,
        "sampling": source["sampling"],
        "derived_from": {
            "suite": source["name"],
            "digest": source["digest"],
            "kept": list(KEPT),
            "retired": dropped,
        },
        "items": items,
    }
    blob = json.dumps(items, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
    suite["digest"] = hashlib.sha256(blob.encode()).hexdigest()
    return suite


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--from", dest="source", default="crates/gym/suites/coder-turns-v1.json")
    parser.add_argument("--out", default="crates/gym/suites/coder-turns-v2.json")
    args = parser.parse_args()
    with open(args.source, encoding="utf-8") as handle:
        source = json.load(handle)
    suite = build(source)
    with open(args.out, "w", encoding="utf-8") as handle:
        json.dump(suite, handle, indent=2, ensure_ascii=False)
        handle.write("\n")
    counts = {}
    for item in suite["items"]:
        key = (item["family"], item["partition"])
        counts[key] = counts.get(key, 0) + 1
    print("{} items, digest {}".format(len(suite["items"]), suite["digest"][:16]))
    for key in sorted(counts):
        print("  {}/{}: {}".format(*key, counts[key]))


if __name__ == "__main__":
    main()
