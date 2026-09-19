"""Read the probe's answers and compute the three statistics, per door.

Monotonicity is measured on the ramps alone, because only there is the true
level known to walk with everything else held fixed. Confusion and bimodality
are measured on both families, and reported separately, because the 36-item
severity family is the one this repository already publishes Score numbers
for.

Every rule below is fixed here rather than chosen after the numbers arrived:

- **Monotonicity.** Within a ramp, take every pair of levels `i < j` and ask
  whether the reported score is greater at `j`. Ties count as half. Twelve
  ramps give 120 pairs. `tau = 2 * concordant - 1`, so 1.0 is a perfect walk,
  0.0 is a coin toss, and a negative value walks backwards. Spearman's rho
  over the 60 ramp items is reported beside it.
- **Confusion.** An item is wrong when the argmax level is not the true
  level. Among wrong items, report the fraction that err to an adjacent
  level and the mean distance. Both are compared with the null that puts the
  error uniformly over the other levels, computed from the true levels the
  items actually carry: an ordered model should beat that null, an unordered
  one should sit on it.
- **Bimodality.** A level is a peak when it is a strict local maximum
  carrying at least 0.15. A distribution is bimodal when it has two or more
  peaks and the lowest level between the two largest peaks carries at most
  0.6 of the smaller peak. Separately, the mean is *unsupported* when the
  level the reported score rounds to carries less probability than some level
  below it and less than some level above it — the weighted mean lands in the
  trough, and reports a level no part of the distribution supports.

Usage:

    python3 analyze.py
    python3 analyze.py --door kev-0.5b --verbose
"""

import argparse
import json
import math
import pathlib

HERE = pathlib.Path(__file__).resolve().parent
RESULTS = HERE / "results"

PEAK_MASS = 0.15
TROUGH_RATIO = 0.6


def load(name):
    path = RESULTS / f"{name}.jsonl"
    return [json.loads(line) for line in path.read_text().splitlines() if line]


def dist(row):
    """The answer's probabilities as a list indexed by level."""
    return [float(row["probabilities"][str(i)]) for i in range(row["levels"])]


def argmax(values):
    return max(range(len(values)), key=lambda i: values[i])


def ranks(values):
    """Ranks with ties averaged."""
    order = sorted(range(len(values)), key=lambda i: values[i])
    out = [0.0] * len(values)
    i = 0
    while i < len(order):
        j = i
        while j + 1 < len(order) and values[order[j + 1]] == values[order[i]]:
            j += 1
        mean = (i + j) / 2 + 1
        for k in range(i, j + 1):
            out[order[k]] = mean
        i = j + 1
    return out


def pearson(xs, ys):
    n = len(xs)
    mx = sum(xs) / n
    my = sum(ys) / n
    num = sum((x - mx) * (y - my) for x, y in zip(xs, ys))
    dx = math.sqrt(sum((x - mx) ** 2 for x in xs))
    dy = math.sqrt(sum((y - my) ** 2 for y in ys))
    return num / (dx * dy) if dx and dy else 0.0


def spearman(xs, ys):
    return pearson(ranks(xs), ranks(ys))


def wilson(successes, total, z=1.96):
    """A 95% Wilson interval, so a count on 36 items carries its width."""
    if total == 0:
        return (0.0, 0.0)
    p = successes / total
    d = 1 + z * z / total
    center = (p + z * z / (2 * total)) / d
    half = z * math.sqrt(p * (1 - p) / total + z * z / (4 * total * total)) / d
    return (max(0.0, center - half), min(1.0, center + half))


def by_level(rows):
    """Mean reported score at each true level."""
    means = {}
    for row in rows:
        means.setdefault(row["truth"], []).append(row["score"])
    return {k: sum(v) / len(v) for k, v in sorted(means.items())}


def monotonicity(rows):
    """The within-ramp pair statistics, on the ramp family alone."""
    by_ramp = {}
    for row in rows:
        if row["family"] != "ramp":
            continue
        by_ramp.setdefault(row["ramp"], {})[row["truth"]] = row["score"]
    concordant = 0.0
    pairs = 0
    steps_up = 0
    steps = 0
    per_ramp = []
    for scores in by_ramp.values():
        levels = sorted(scores)
        for a in range(len(levels)):
            for b in range(a + 1, len(levels)):
                lo, hi = scores[levels[a]], scores[levels[b]]
                concordant += 1.0 if hi > lo else 0.5 if hi == lo else 0.0
                pairs += 1
        for a in range(len(levels) - 1):
            steps += 1
            if scores[levels[a + 1]] > scores[levels[a]]:
                steps_up += 1
        per_ramp.append(spearman(levels, [scores[k] for k in levels]))
    flat = [(row["truth"], row["score"]) for row in rows if row["family"] == "ramp"]
    means = {}
    for truth, score in flat:
        means.setdefault(truth, []).append(score)
    return {
        "ramps": len(by_ramp),
        "pairs": pairs,
        "tau": 2 * (concordant / pairs) - 1 if pairs else 0.0,
        "steps_up": steps_up,
        "steps": steps,
        "rho": spearman([t for t, _ in flat], [s for _, s in flat]),
        "rho_per_ramp": sum(per_ramp) / len(per_ramp) if per_ramp else 0.0,
        "mean_by_level": {k: sum(v) / len(v) for k, v in sorted(means.items())},
    }


def confusion(rows):
    """Accuracy, and whether the errors that remain cluster near the truth."""
    wrong = []
    for row in rows:
        picked = argmax(dist(row))
        if picked != row["truth"]:
            wrong.append((row, picked))
    total = len(rows)
    if not wrong:
        return {
            "n": total,
            "accuracy": 1.0,
            "errors": 0,
            "adjacent": None,
            "null_adjacent": None,
            "distance": None,
            "null_distance": None,
            "mae": mae(rows),
        }
    adjacent = sum(1 for row, p in wrong if abs(p - row["truth"]) == 1)
    distance = sum(abs(p - row["truth"]) for row, p in wrong) / len(wrong)
    null_adj = 0.0
    null_dist = 0.0
    for row, _ in wrong:
        others = [i for i in range(row["levels"]) if i != row["truth"]]
        null_adj += sum(1 for i in others if abs(i - row["truth"]) == 1) / len(others)
        null_dist += sum(abs(i - row["truth"]) for i in others) / len(others)
    return {
        "n": total,
        "accuracy": 1 - len(wrong) / total,
        "errors": len(wrong),
        "adjacent": adjacent / len(wrong),
        "adjacent_count": adjacent,
        "adjacent_ci": wilson(adjacent, len(wrong)),
        "null_adjacent": null_adj / len(wrong),
        "distance": distance,
        "null_distance": null_dist / len(wrong),
        "mae": mae(rows),
    }


def mae(rows):
    return sum(abs(row["score"] - row["truth"]) for row in rows) / len(rows)


def peaks(probabilities):
    out = []
    for i, p in enumerate(probabilities):
        left = probabilities[i - 1] if i > 0 else -1.0
        right = probabilities[i + 1] if i + 1 < len(probabilities) else -1.0
        if p > left and p > right and p >= PEAK_MASS:
            out.append(i)
    return out


def is_bimodal(probabilities):
    found = peaks(probabilities)
    if len(found) < 2:
        return False
    found.sort(key=lambda i: probabilities[i], reverse=True)
    a, b = sorted(found[:2])
    trough = min(probabilities[a + 1 : b])
    return trough <= TROUGH_RATIO * min(probabilities[a], probabilities[b])


def nearest(row):
    """The level the reported score rounds to, halves rounding up.

    Python's `round` is ties-to-even, which would send 1.5 to 2 and 2.5 to
    2. A caller reading a score off a page rounds halves up, and a score of
    exactly 1.50 is common because the doors round the field to two decimal
    places.
    """
    return min(row["levels"] - 1, max(0, math.floor(row["score"] + 0.5)))


def unsupported(row):
    """The reported mean rounds to a level the distribution dips at."""
    probabilities = dist(row)
    mean = nearest(row)
    below = any(p > probabilities[mean] for p in probabilities[:mean])
    above = any(p > probabilities[mean] for p in probabilities[mean + 1 :])
    return below and above


def disagrees(row):
    """The level the score rounds to is not the level the door picked."""
    return nearest(row) != argmax(dist(row))


def bimodality(rows):
    bimodal = [row for row in rows if is_bimodal(dist(row))]
    unsup = [row for row in rows if unsupported(row)]
    split = [row for row in rows if disagrees(row)]
    return {
        "n": len(rows),
        "disagree": len(split),
        "disagree_ci": wilson(len(split), len(rows)),
        "bimodal": len(bimodal),
        "bimodal_ci": wilson(len(bimodal), len(rows)),
        "unsupported": len(unsup),
        "unsupported_ci": wilson(len(unsup), len(rows)),
        "ids": [row["id"] for row in unsup],
    }


def report(name, verbose=False):
    rows = load(name)
    ramp = [r for r in rows if r["family"] == "ramp"]
    severity = [r for r in rows if r["family"] == "severity"]
    mono = monotonicity(rows)
    print(f"\n=== {name} ===")
    print(
        f"  monotonicity  tau {mono['tau']:+.2f} over {mono['pairs']} ramp pairs"
        f" | rho {mono['rho']:+.2f} | per-ramp rho {mono['rho_per_ramp']:+.2f}"
        f" | steps up {mono['steps_up']}/{mono['steps']}"
    )
    print(
        "  ramp mean score by true level      "
        + "  ".join(f"{k}:{v:.2f}" for k, v in mono["mean_by_level"].items())
    )
    print(
        "  severity mean score by true level  "
        + "  ".join(f"{k}:{v:.2f}" for k, v in by_level(severity).items())
    )
    for label, subset in (("ramp", ramp), ("severity", severity), ("all", rows)):
        c = confusion(subset)
        line = (
            f"  confusion {label:<9} n {c['n']:>3} acc {c['accuracy']:.2f}"
            f" MAE {c['mae']:.2f} levels"
        )
        if c["errors"]:
            alo, ahi = c["adjacent_ci"]
            line += (
                f" | errors {c['errors']:>2}"
                f" adjacent {c['adjacent']:.2f} [{alo:.2f}, {ahi:.2f}]"
                f" (null {c['null_adjacent']:.2f})"
                f" distance {c['distance']:.2f} (null {c['null_distance']:.2f})"
            )
        print(line)
    for label, subset in (("ramp", ramp), ("severity", severity), ("all", rows)):
        b = bimodality(subset)
        lo, hi = b["bimodal_ci"]
        ulo, uhi = b["unsupported_ci"]
        dlo, dhi = b["disagree_ci"]
        print(
            f"  bimodality {label:<9} n {b['n']:>3}"
            f" bimodal {b['bimodal']:>2} [{lo:.2f}, {hi:.2f}]"
            f" | unsupported mean {b['unsupported']:>2} [{ulo:.2f}, {uhi:.2f}]"
            f" | mean not the argmax {b['disagree']:>2} [{dlo:.2f}, {dhi:.2f}]"
        )
    if verbose:
        for row in rows:
            probabilities = dist(row)
            flags = []
            if is_bimodal(probabilities):
                flags.append("bimodal")
            if unsupported(row):
                flags.append("unsupported")
            if flags:
                print(
                    f"    {row['id']:<28} truth {row['truth']}"
                    f" score {row['score']:.2f}"
                    f" p {[round(p, 3) for p in probabilities]}"
                    f"  {' '.join(flags)}"
                )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--door", default=None, help="one door, or every one")
    parser.add_argument("--verbose", action="store_true", help="list the cases")
    args = parser.parse_args()
    names = (
        [args.door]
        if args.door
        else sorted(p.stem for p in RESULTS.glob("*.jsonl"))
    )
    for name in names:
        report(name, verbose=args.verbose)


if __name__ == "__main__":
    main()
