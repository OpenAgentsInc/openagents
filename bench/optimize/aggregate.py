"""Aggregate repeated per-side trials into mean ± spread, and A/B two sides.

Item 2 of `docs/coderbench.md` "What to do next": a single trial per side is
not statistically honest, so a reported delta must carry a mean and a spread,
not one sample. Every side of an A/B runs `DEFAULT_N_TRIALS` (>= 3) trials;
the per-trial metrics collapse to a mean and a spread (stddev, with min/max
carried for context), and a delta whose magnitude is inside the combined
spread is reported as "within noise / not significant" rather than a spurious
percentage.

Unknown stays unknown (ledger M2). An unpriced trial contributes to
`n_unknown`, never a fabricated 0, so a cost mean over all-unknown trials is
`None`, not zero — the same honesty the metric keeps on a single trial.
"""

from __future__ import annotations

import statistics
from collections.abc import Sequence
from dataclasses import asdict, dataclass
from typing import Any

from .metric import JobScore

# Trials per side of an A/B. Three is the floor that makes a spread meaningful;
# a single trial has no spread and reads every delta as significant (the
# dishonesty this module removes).
DEFAULT_N_TRIALS = 3


@dataclass(frozen=True)
class Aggregate:
    """Mean and spread of one metric across trials; unknown stays unknown.

    `n` is the count of trials that carried a numeric value; `n_unknown` is the
    count that did not (e.g. an unpriced trial). The mean and spread are over
    the known values only — an unknown is never read as 0.
    """

    n: int
    n_unknown: int
    mean: float | None
    stddev: float | None
    minimum: float | None
    maximum: float | None

    @property
    def spread(self) -> float | None:
        """The spread reported for the ± term (population/sample stddev)."""
        return self.stddev


def aggregate_values(values: Sequence[float | None]) -> Aggregate:
    """Collapse per-trial values to mean ± stddev; carry unknowns honestly.

    `None` marks a trial that did not carry the metric (e.g. unpriced cost). It
    is excluded from the mean and counted in `n_unknown` rather than treated as
    a 0. The stddev is the sample stddev when at least two values are known and
    0.0 for a single known value (a lone sample has no measurable spread).
    """
    known = [float(value) for value in values if value is not None]
    n_unknown = len(values) - len(known)
    if not known:
        return Aggregate(
            n=0, n_unknown=n_unknown, mean=None, stddev=None, minimum=None, maximum=None
        )
    stddev = statistics.stdev(known) if len(known) >= 2 else 0.0
    return Aggregate(
        n=len(known),
        n_unknown=n_unknown,
        mean=statistics.fmean(known),
        stddev=stddev,
        minimum=min(known),
        maximum=max(known),
    )


@dataclass(frozen=True)
class SideAggregate:
    """One side of an A/B: its objective score and cost terms over n trials."""

    label: str
    n_trials: int
    objective_score: Aggregate
    success_rate: Aggregate
    tokens: Aggregate
    wall_seconds: Aggregate
    cost_usd: Aggregate
    cost_disposition: str

    def as_dict(self) -> dict[str, Any]:
        payload = asdict(self)
        # asdict already expands the nested Aggregate dataclasses.
        return payload


def aggregate_side(label: str, job_scores: Sequence[JobScore]) -> SideAggregate:
    """Aggregate one side's per-trial `JobScore`s into mean ± spread.

    Tokens are prompt+completion summed within a trial, then averaged across
    trials. `success_rate` and `cost_usd` carry unknowns: an ungraded side has
    a `None` success rate and an unpriced side a `None` cost, and neither is
    read as 0.
    """
    if not job_scores:
        raise ValueError("aggregate_side requires at least one JobScore")
    objective = aggregate_values([score.mean_score for score in job_scores])
    success = aggregate_values([score.success_rate for score in job_scores])
    tokens = aggregate_values(
        [score.prompt_tokens + score.completion_tokens for score in job_scores]
    )
    wall = aggregate_values([score.wall_seconds for score in job_scores])
    cost = aggregate_values([score.cost_usd for score in job_scores])
    if cost.n == 0:
        cost_disposition = "cost_unknown"
    elif cost.n_unknown == 0:
        cost_disposition = "priced"
    else:
        cost_disposition = "partially_priced"
    return SideAggregate(
        label=label,
        n_trials=len(job_scores),
        objective_score=objective,
        success_rate=success,
        tokens=tokens,
        wall_seconds=wall,
        cost_usd=cost,
        cost_disposition=cost_disposition,
    )


@dataclass(frozen=True)
class Comparison:
    """A/B of two aggregated metrics with a within-noise verdict.

    `delta` is candidate.mean − control.mean. `combined_spread` is the sum of
    the two sides' spreads; when `|delta|` is inside it the delta is "within
    noise" and no percentage is reported (a spurious percentage is the exact
    dishonesty this guards against). A percentage is reported only for a delta
    that clears the noise band against a non-negligible baseline.
    """

    metric: str
    control: Aggregate
    candidate: Aggregate
    delta: float | None
    delta_pct: float | None
    combined_spread: float | None
    within_noise: bool
    disposition: str

    def as_dict(self) -> dict[str, Any]:
        return asdict(self)


# A baseline whose magnitude is under this is treated as ~0 for a percentage:
# dividing a delta by it yields a meaningless number, so the percentage is
# withheld (None) rather than fabricated.
_PCT_BASELINE_EPSILON = 1e-9


def compare_sides(
    control: Aggregate, candidate: Aggregate, *, metric: str = "objectiveScore"
) -> Comparison:
    """Compare a control and candidate aggregate; flag within-noise deltas.

    Rule (kept deliberately simple): the combined spread is the sum of the two
    sides' stddevs, and a delta with `|delta| <= combined_spread` is reported as
    "within noise / not significant". Only a delta that clears the band gets a
    direction and — against a non-negligible baseline — a percentage. If either
    mean is unknown the comparison is `unknown` and no delta is computed.
    """
    if control.mean is None or candidate.mean is None:
        return Comparison(
            metric=metric,
            control=control,
            candidate=candidate,
            delta=None,
            delta_pct=None,
            combined_spread=None,
            within_noise=False,
            disposition="unknown",
        )
    delta = candidate.mean - control.mean
    combined_spread = (control.stddev or 0.0) + (candidate.stddev or 0.0)
    within_noise = abs(delta) <= combined_spread
    if within_noise:
        return Comparison(
            metric=metric,
            control=control,
            candidate=candidate,
            delta=delta,
            delta_pct=None,
            combined_spread=combined_spread,
            within_noise=True,
            disposition="within_noise",
        )
    delta_pct = None
    if abs(control.mean) > _PCT_BASELINE_EPSILON:
        delta_pct = delta / control.mean * 100.0
    disposition = "candidate_higher" if delta > 0 else "candidate_lower"
    return Comparison(
        metric=metric,
        control=control,
        candidate=candidate,
        delta=delta,
        delta_pct=delta_pct,
        combined_spread=combined_spread,
        within_noise=False,
        disposition=disposition,
    )
