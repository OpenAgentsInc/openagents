#!/usr/bin/env python3
"""Prints the majority-class baseline per family of a coder-turns suite.

The baseline is the accuracy a router gets by answering every item of a
family with that family's most common label. A question is worth asking
only when a door beats this number on the same items, so the record beside
the suite publishes it before any door's score.

Counting labels does not read the locked partition in the sense the ledger
guards: no state is shown to a door, and no answer is scored. The counts
say how the labels fall, which is what the suite's `sampling` field already
says for the shell rounds.

    python3 crates/gym/suites/baseline_coder_turns.py \
        crates/gym/suites/coder-turns-v1.json

Pass `--partition calibration` or `--partition development` to count one
partition, or `--store crates/gym/results/coder-turns-v1.jsonl --door
"jev (hosted)"` to print a door's accuracy from recorded rows beside the
constant on the same items. A refused row stays in the denominator, as
`gym compare` keeps it, and a permuted row is left out.
"""

import argparse
import collections
import json
import math


def wilson(hits, total, z=1.96):
    """The Wilson score interval, the same one the decision-model records use."""
    if total == 0:
        return (0.0, 0.0)
    p = hits / total
    denominator = 1 + z * z / total
    centre = (p + z * z / (2 * total)) / denominator
    half = z * math.sqrt(p * (1 - p) / total + z * z / (4 * total * total)) / denominator
    return (centre - half, centre + half)


def label_of(truth):
    return json.dumps(truth, sort_keys=True) if not isinstance(truth, str) else truth


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("suite")
    parser.add_argument("--partition", action="append", default=None)
    parser.add_argument("--store", default=None)
    parser.add_argument("--door", default=None, help="the door name rows carry in `door`")
    args = parser.parse_args()

    with open(args.suite, encoding="utf-8") as handle:
        suite = json.load(handle)
    items = suite["items"]
    if args.partition:
        items = [item for item in items if item["partition"] in args.partition]

    by_family = collections.defaultdict(collections.Counter)
    for item in items:
        by_family[item["family"]][label_of(item["truth"])] += 1

    correct = collections.Counter()
    scored = collections.Counter()
    if args.store:
        wanted = {item["id"] for item in items}
        with open(args.store, encoding="utf-8") as handle:
            for line in handle:
                line = line.strip()
                if not line:
                    continue
                row = json.loads(line)
                if row.get("item_id") not in wanted:
                    continue
                if args.door and row.get("door") != args.door:
                    continue
                if row.get("permutation") is not None:
                    continue
                family = row["family"]
                scored[family] += 1
                if row.get("correct") is True:
                    correct[family] += 1

    print("suite {} digest {}".format(suite["name"], suite["digest"][:16]))
    print("partitions: {}".format(", ".join(args.partition) if args.partition else "all"))
    header = "| Family | Items | Labels | Majority | Wilson 95% |"
    if args.store:
        header += " Door scored | Door accuracy | Door minus constant |"
    print(header)
    print("| --- | --- | --- | --- | --- |" + (" --- | --- | --- |" if args.store else ""))
    for family in sorted(by_family, key=lambda f: -max(by_family[f].values()) / sum(by_family[f].values())):
        counts = by_family[family]
        total = sum(counts.values())
        top = max(counts.values())
        low, high = wilson(top, total)
        labels = ", ".join(
            "{} `{}`".format(n, label) for label, n in counts.most_common()
        )
        line = "| `{}` | {} | {} | {:.3f} | {:.3f} to {:.3f} |".format(
            family, total, labels, top / total, low, high
        )
        if args.store:
            n = scored[family]
            acc = correct[family] / n if n else float("nan")
            line += " {} | {:.3f} | {:+.3f} |".format(n, acc, acc - top / total)
        print(line)


if __name__ == "__main__":
    main()
