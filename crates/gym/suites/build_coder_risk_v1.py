#!/usr/bin/env python3
"""Derives `coder-risk-v1` from the committed `coder-turns-v1` suite.

`coder-risk-v1` holds the 40 `risk` items of `coder-turns-v1`, byte for
byte: the same states, the same outcome labels, the same partitions. It
exists so that the `risk` question can be reworded and scored on unchanged
items. Each candidate wording is its own question set in
`crates/gym/questions/coder-risk-*.json`, served with `--questions`, and a
row recorded under one candidate is comparable with a row under another
because the suite digest is the same.

The label is v1's outcome rule, read from what the agent did before the
next human turn: `2` if it wrote a file or ran a writing command, `1` if it
only read, `0` if it ran nothing. A candidate is scored on how well it
predicts that rule. The reversibility-and-reach rubric proposed in
`docs/decision-models/2026-09-20-coder-question-baselines.md` has no label
under this rule, and this suite does not pretend otherwise.

The v1 suite is built from a session archive that is not in the repository,
so this builder reads the committed v1 file rather than the archive.

    python3 crates/gym/suites/build_coder_risk_v1.py \
        --from crates/gym/suites/coder-turns-v1.json \
        --out crates/gym/suites/coder-risk-v1.json
"""

import argparse
import hashlib
import json

FAMILY = "risk"
QUESTIONS = "coder-risk-control-v1"


def build(source):
    items = [item for item in source["items"] if item["family"] == FAMILY]
    suite = {
        "schema": source["schema"],
        "name": "coder-risk-v1",
        "created": "2026-09-20",
        "description": (
            "The 40 `risk` items of coder-turns-v1, unchanged, so that "
            "candidate wordings of the `risk` question can be scored on the "
            "same states and labels. The label is v1's outcome rule: 2 if "
            "the agent then wrote a file or ran a writing command, 1 if it "
            "only read, 0 if it ran nothing. The default question set is "
            "the v1 wording; each candidate is a separate set in "
            "crates/gym/questions/coder-risk-*.json, served with "
            "--questions. Partitions are the v1 items' own."
        ),
        "tier": source["tier"],
        "gate": source["gate"],
        "questions": QUESTIONS,
        "derived_from": {
            "suite": source["name"],
            "digest": source["digest"],
            "kept": [FAMILY],
        },
        "items": items,
    }
    blob = json.dumps(items, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
    suite["digest"] = hashlib.sha256(blob.encode()).hexdigest()
    return suite


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--from", dest="source", default="crates/gym/suites/coder-turns-v1.json")
    parser.add_argument("--out", default="crates/gym/suites/coder-risk-v1.json")
    args = parser.parse_args()
    with open(args.source, encoding="utf-8") as handle:
        source = json.load(handle)
    suite = build(source)
    with open(args.out, "w", encoding="utf-8") as handle:
        json.dump(suite, handle, indent=2, ensure_ascii=False)
        handle.write("\n")
    counts = {}
    for item in suite["items"]:
        key = (item["partition"], item["truth"])
        counts[key] = counts.get(key, 0) + 1
    print("{} items, digest {}".format(len(suite["items"]), suite["digest"][:16]))
    for key in sorted(counts):
        print("  {} truth {}: {}".format(*key, counts[key]))


if __name__ == "__main__":
    main()
