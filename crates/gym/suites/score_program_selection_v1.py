#!/usr/bin/env python3
"""Reports what `program-selection-v1` scored, with the two errors apart.

`gym eval` prints accuracy, ECE, Brier and NLL per partition. For this
question those four say less than two counts do, for two reasons.

**The baseline is nearly the ceiling.** Almost every turn asks for no
program. Answering `none` every time scores whatever the negative class's
share is, and the headroom above that is what any door can possibly win. A
partition whose headroom is smaller than the noise floor cannot register a
win at any strength, which is the finding
[#9392](https://github.com/OpenAgentsInc/openagents/issues/9392) landed on
after spending a full experiment on one. So the baseline is printed first,
and the accuracy underneath it.

**The two errors cost different amounts.** A missed program is a turn that
answers normally: the operator asks again. A spurious program runs a program
nobody asked for, and `delegate-fan-out` runs it as subprocesses. One
accuracy figure averages those together.

It also splits real turns from written ones. Real turns say how often the
question fires when nobody asked; written ones say whether it recognises a
request when it sees one. Neither answers the other's question.

Usage
-----

    python3 crates/gym/suites/score_program_selection_v1.py \\
        crates/gym/results/program-selection-v1.jsonl
"""

import argparse
import collections
import json
import os
import statistics
import sys

#: The answer that means the turn asks for no program.
NONE = "none"


def rows(path, door):
    """Every recorded row for one door, by item id, latest run last."""
    found = {}
    with open(path, encoding="utf-8") as handle:
        for line in handle:
            line = line.strip()
            if not line:
                continue
            row = json.loads(line)
            if door and row.get("door") != door:
                continue
            if not row.get("answered"):
                continue
            found[row["item_id"]] = row
    return found


def chosen(row):
    """The option a row's distribution names."""
    distribution = row.get("distribution") or {}
    if not distribution:
        return None
    return max(distribution.items(), key=lambda pair: pair[1])[0]


def group(item):
    return "real turns" if item["id"].startswith("turn/") else "written"


def report(suite, answers, out):
    items = [item for item in suite["items"] if item["partition"] != "locked"]
    scored = [item for item in items if item["id"] in answers]
    missing = len(items) - len(scored)

    out.write("# program-selection-v1\n\n")
    out.write(
        "suite digest `{}`, {} items scored, {} unanswered, locked partition unread\n\n".format(
            suite["digest"][:16], len(scored), missing
        )
    )

    out.write("## The baseline, before any accuracy\n\n")
    out.write("| Set | Items | Asks for a program | Constant `none` | Headroom |\n")
    out.write("| --- | --- | --- | --- | --- |\n")
    sets = [("real turns", None), ("written", None), ("both", None)]
    for name, _ in sets:
        chosen_items = [
            item for item in scored if name == "both" or group(item) == name
        ]
        if not chosen_items:
            continue
        positives = [item for item in chosen_items if item["truth"] != NONE]
        baseline = 1 - len(positives) / len(chosen_items)
        out.write(
            "| {} | {} | {} | {:.3f} | {:.3f} |\n".format(
                name,
                len(chosen_items),
                len(positives),
                baseline,
                1 - baseline,
            )
        )
    out.write("\n")

    out.write("## What the door did\n\n")
    out.write(
        "| Set | Items | Accuracy | False positives | False negatives | Wrong program |\n"
    )
    out.write("| --- | --- | --- | --- | --- | --- |\n")
    for name, _ in sets:
        chosen_items = [
            item for item in scored if name == "both" or group(item) == name
        ]
        if not chosen_items:
            continue
        right = false_positive = false_negative = misroute = 0
        for item in chosen_items:
            answer = chosen(answers[item["id"]])
            if answer == item["truth"]:
                right += 1
            elif item["truth"] == NONE:
                false_positive += 1
            elif answer == NONE:
                false_negative += 1
            else:
                misroute += 1
        negatives = len([item for item in chosen_items if item["truth"] == NONE])
        positives = len(chosen_items) - negatives
        out.write(
            "| {} | {} | {:.3f} | {} of {} ({:.3f}) | {} of {} ({}) | {} |\n".format(
                name,
                len(chosen_items),
                right / len(chosen_items),
                false_positive,
                negatives,
                false_positive / negatives if negatives else 0.0,
                false_negative,
                positives,
                "{:.3f}".format(false_negative / positives) if positives else "n/a",
                misroute,
            )
        )
    out.write("\n")

    wrong = [
        (item, chosen(answers[item["id"]]), answers[item["id"]]["raw_top"])
        for item in scored
        if chosen(answers[item["id"]]) != item["truth"]
    ]
    out.write("## Every item it got wrong\n\n")
    if not wrong:
        out.write("None.\n\n")
    for item, answer, confidence in wrong:
        out.write(
            "- `{}` — answered `{}` at {:.2f}, labelled `{}`: {}\n".format(
                item["id"],
                answer,
                confidence,
                item["truth"],
                json.dumps(item["state"]["request"][:160]),
            )
        )
    out.write("\n")

    latencies = sorted(answers[item["id"]]["latency_ms"] for item in scored)
    out.write("## What it cost\n\n")
    out.write(
        "median {:.0f} ms, slowest {:.0f} ms, over {} calls — one per turn, on every turn.\n".format(
            statistics.median(latencies), latencies[-1], len(latencies)
        )
    )

    counts = collections.Counter(
        chosen(answers[item["id"]]) for item in scored
    )
    out.write("\nAnswers given: {}\n".format(dict(counts)))


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("results", help="the store `gym eval --record` wrote")
    parser.add_argument(
        "--suite",
        default=os.path.join(here, "program-selection-v1.json"),
        help="the suite the rows were scored against",
    )
    parser.add_argument("--door", default=None, help="one door's rows, by name")
    args = parser.parse_args()

    with open(args.suite, encoding="utf-8") as handle:
        suite = json.load(handle)
    report(suite, rows(args.results, args.door), sys.stdout)


if __name__ == "__main__":
    main()
