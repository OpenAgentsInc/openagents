#!/usr/bin/env python3
"""Compares the baseline against the doors, against the measured noise floor.

A raw difference is not a finding. `docs/lev/measurements/2026-09-19-seed-variance.md`
puts the smallest difference this suite can resolve between two doors measured
on one seed block each at **0.056 accuracy, 7.2% relative**, and two published
comparisons were withdrawn this week for not clearing it.

That floor covers seed resampling on 98 items and nothing else. The same
record says the item set is the larger of the two intervals: the binomial
standard error of an accuracy near 0.78 on 98 items is about 0.042, twice the
seed noise. The routing family is 50 items, so this script reports a
comparison three ways and lets the reader see how much the verdict depends on
which interval is honoured:

1. Against the recorded 0.056 floor, which is what the acceptance criterion in
   issue #9377 asks for.
2. Against two standard errors of the unpaired difference, which adds the item
   sample both sides were drawn from.
3. Both sides' own intervals, printed, because a point estimate on fifty items
   should never be quoted alone.

The door rows are transcribed from the per-family table in
`docs/lev/measurements/2026-09-19-suite-v2-scores.md`, which scored the same
`routing` items of `support-v2` (digest `6877c24bf261d5bd`) that this
baseline's `support-v2` runs score.

    python3 compare.py
"""

from __future__ import annotations

import glob
import json
import math
import sys
from pathlib import Path

# The measured floor for a two-door comparison on this suite, one seed block
# per side. Source: docs/lev/measurements/2026-09-19-seed-variance.md.
FLOOR = 0.056

# The `routing` rows of docs/lev/measurements/2026-09-19-suite-v2-scores.md,
# evaluation split of `support-v2`, 50 items each. `lev, calibrated` is the
# admitted map; it is the only family map in the repository that passed the
# admission gate, and it changes calibration without changing accuracy.
DOORS = {
    "jev (hosted)": {"accuracy": 0.94, "ece": 0.060, "brier": 0.039, "nll": 0.122, "items": 50},
    "lev, raw": {"accuracy": 0.82, "ece": 0.140, "brier": 0.135, "nll": 2.860, "items": 50},
    "lev, calibrated": {"accuracy": 0.82, "ece": 0.024, "brier": 0.133, "nll": 0.431, "items": 50},
    "kev-0.5b": {"accuracy": 0.78, "ece": 0.120, "brier": 0.170, "nll": 0.502, "items": 50},
}


def standard_error(accuracy: float, items: int) -> float:
    return math.sqrt(max(accuracy * (1.0 - accuracy), 0.0) / items)


def main() -> int:
    here = Path(__file__).parent
    runs = {}
    for path in sorted(glob.glob(str(here / "runs" / "*.json"))):
        record = json.loads(Path(path).read_text())
        family = next(f for f in record["served"] if f["served"])
        runs[Path(path).stem] = {
            "suite": record["suite"]["name"],
            "encoder": record["encoder"]["name"],
            "floor": family["majority_class_floor"],
            "by_rule": family["by_rule"],
            "refused": sum(r["items_refused"] for r in record["refused"]),
        }
    if not runs:
        print("no runs found; run ./run.sh first")
        return 1

    print("## The panel\n")
    print("| Run | Rule | Accuracy | SE | ECE | Brier | NLL | Confident errors | Items |")
    print("| --- | --- | --- | --- | --- | --- | --- | --- | --- |")
    for name, run in runs.items():
        for rule, result in run["by_rule"].items():
            p = result["panel"]
            print(
                f"| `{name}` | {rule} | {p['accuracy']:.3f} | "
                f"{result['accuracy_standard_error']:.3f} | {p['ece']:.3f} | {p['brier']:.3f} | "
                f"{p['nll']:.3f} | {p['confident_errors']} | {p['items']} |"
            )

    print("\n## Against the doors, on the same 50 `routing` items of `support-v2`\n")
    print(
        "| Comparison | Difference | As a multiple of the 0.056 floor | Two unpaired sigma | Clears the floor | Clears both intervals |"
    )
    print("| --- | --- | --- | --- | --- | --- |")
    for name, run in runs.items():
        if run["suite"] != "support-v2":
            continue
        for rule, result in run["by_rule"].items():
            if rule != "argmin":
                continue
            ours = result["panel"]["accuracy"]
            our_se = result["accuracy_standard_error"]
            for door, numbers in DOORS.items():
                if door == "lev, calibrated":
                    continue
                theirs = numbers["accuracy"]
                difference = ours - theirs
                their_se = standard_error(theirs, numbers["items"])
                two_sigma = 2.0 * math.sqrt(our_se**2 + their_se**2)
                print(
                    f"| `{name}` against {door} | {difference:+.3f} | {abs(difference) / FLOOR:.2f} | "
                    f"{two_sigma:.3f} | {'yes' if abs(difference) > FLOOR else 'no'} | "
                    f"{'yes' if abs(difference) > two_sigma else 'no'} |"
                )

    encoders = [
        run["by_rule"]["argmin"]["panel"]["accuracy"]
        for name, run in runs.items()
        if run["suite"] == "support-v2" and run["encoder"] != "tfidf"
    ]
    if encoders:
        spread = max(encoders) - min(encoders)
        print(
            f"\nEncoder spread on the same items, same rule: {min(encoders):.3f} to {max(encoders):.3f}, "
            f"a range of {spread:.3f}, which is {spread / FLOOR:.2f} times the floor."
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
