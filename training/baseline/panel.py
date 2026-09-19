"""The metric panel every door in this repository is scored on.

This is a port of `score` in `crates/gym/src/calibrate.rs`, and it is a port
rather than a fresh implementation on purpose: a baseline whose numbers are
computed differently from the numbers it is compared against measures the
difference between two scorers, not the difference between two doors.

Every metric reads the *winning option's* reported probability and whether
that option was the labelled answer. That is the same reduction the Rust
`Observation` makes, so a three-way Choice and a two-way Noul land on the
same axis.

`check_panel.py` runs the fixtures from the Rust unit tests through this
module and asserts the values those tests assert.
"""

from __future__ import annotations

import math
from dataclasses import dataclass, asdict

# The Rust scorer clamps before taking a logarithm so a reported zero is a
# large finite penalty rather than an infinity.
_FLOOR = 1e-12

# Expected calibration error is computed over ten equal-width bins, matching
# `Metrics::ece`.
_BINS = 10

# An error reported at this probability or above is a confident error: the
# failure a threshold cannot catch.
_CONFIDENT = 0.9


@dataclass(frozen=True)
class Observation:
    """What the winning option carried, and whether it was right."""

    raw: float
    correct: bool


@dataclass(frozen=True)
class Panel:
    """Accuracy, ECE, Brier, log loss, confident errors, items."""

    accuracy: float
    ece: float
    brier: float
    nll: float
    confident_errors: int
    items: int

    def as_dict(self) -> dict:
        return asdict(self)


def score(observations: list[Observation]) -> Panel:
    """Scores observations whose `raw` is already a reported probability."""
    if not observations:
        return Panel(0.0, 0.0, 0.0, 0.0, 0, 0)

    n = float(len(observations))
    accuracy = sum(1 for o in observations if o.correct) / n
    brier = sum((o.raw - (1.0 if o.correct else 0.0)) ** 2 for o in observations) / n
    nll = (
        sum(-math.log(min(max(o.raw if o.correct else 1.0 - o.raw, _FLOOR), 1.0)) for o in observations)
        / n
    )
    confident_errors = sum(1 for o in observations if not o.correct and o.raw >= _CONFIDENT)

    ece = 0.0
    for index in range(_BINS):
        lo = index / _BINS
        hi = (index + 1) / _BINS
        inside = [
            o for o in observations if o.raw >= lo and (o.raw < hi or (index == _BINS - 1 and o.raw <= hi))
        ]
        if not inside:
            continue
        share = len(inside) / n
        mean_p = sum(o.raw for o in inside) / len(inside)
        mean_correct = sum(1 for o in inside if o.correct) / len(inside)
        ece += share * abs(mean_p - mean_correct)

    return Panel(accuracy, ece, brier, nll, confident_errors, len(observations))


def binomial_standard_error(accuracy: float, items: int) -> float:
    """The standard error of an accuracy estimated on this many items.

    The item set is itself a sample. This is the interval that comes with
    that, and it is the larger of the two intervals on this suite: the seed
    variance record puts resampling noise at about half of it.
    """
    if items <= 0:
        return 0.0
    return math.sqrt(max(accuracy * (1.0 - accuracy), 0.0) / items)


def accuracy_interval(observations: list[Observation], draws: int = 10_000, seed: int = 0) -> tuple[float, float]:
    """A percentile bootstrap interval on accuracy, over the scored items.

    Reported next to the point estimate because a point estimate on tens of
    items is not a number anyone should compare against another one.
    """
    import random

    if not observations:
        return (0.0, 0.0)
    rng = random.Random(seed)
    outcomes = [1.0 if o.correct else 0.0 for o in observations]
    n = len(outcomes)
    means = []
    for _ in range(draws):
        means.append(sum(outcomes[rng.randrange(n)] for _ in range(n)) / n)
    means.sort()
    return (means[int(0.025 * draws)], means[int(0.975 * draws) - 1])
