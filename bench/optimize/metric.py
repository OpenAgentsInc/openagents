"""Gym metric: verifier acceptance minus token and wall terms (ledger O4).

The published evolve-the-harness loop used
`pooled_criterion_rate − 0.005 × tokens_per_million`. The same token
coefficient is the cost term here. Wall clock from the trial's
`agent_execution` timestamps (ATIF does not yet carry `duration_ms`) is a
smaller tie-breaker so the optimizer cannot buy score with time either.

Unknown USD stays unknown (ledger M2). It is recorded on the breakdown and
is not treated as zero. Tokens are the cost that lives inside the score.
"""

from __future__ import annotations

from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any

from .job import HarborJob, HarborTrial, load_job, wall_seconds_of

# Ledger O4 / published loop.
TOKEN_PENALTY_PER_MILLION = 0.005
# Tie-breaker: one hour of wall clock costs 0.001 of a success.
WALL_PENALTY_PER_HOUR = 0.001

DEV_SUITE_ID = "tb2-quick"
HOLDOUT_SUITE_ID = "tb2-cross-section"


@dataclass(frozen=True)
class TrialScore:
    task: str
    accepted: bool
    ungraded: bool
    prompt_tokens: int
    completion_tokens: int
    tokens: int
    wall_seconds: float
    cost_usd: float | None
    score: float
    token_penalty: float
    wall_penalty: float


@dataclass(frozen=True)
class JobScore:
    job_id: str | None
    suite: str
    trials: tuple[TrialScore, ...]
    accepted: int
    graded: int
    ungraded: int
    success_rate: float | None
    mean_score: float
    prompt_tokens: int
    completion_tokens: int
    wall_seconds: float
    cost_usd: float | None
    cost_disposition: str
    metric_calls: int

    def as_dict(self) -> dict[str, Any]:
        payload = asdict(self)
        payload["trials"] = [asdict(trial) for trial in self.trials]
        return payload


def _tokens_of(trial: HarborTrial) -> tuple[int, int]:
    metrics = (trial.trajectory or {}).get("final_metrics") or {}
    prompt = int(metrics.get("total_prompt_tokens") or 0)
    completion = int(metrics.get("total_completion_tokens") or 0)
    return prompt, completion


def score_trial(trial: HarborTrial, cost_usd: float | None = None) -> TrialScore:
    """One Harbor trial. Ungraded is not a success (ledger V4)."""
    ungraded = not trial.verifier_ran
    accepted = False if ungraded else trial.passed
    prompt, completion = _tokens_of(trial)
    tokens = prompt + completion
    wall_seconds = wall_seconds_of(trial)
    token_penalty = TOKEN_PENALTY_PER_MILLION * (tokens / 1_000_000)
    wall_penalty = WALL_PENALTY_PER_HOUR * (wall_seconds / 3600.0)
    acceptance = 1.0 if accepted else 0.0
    # cost_usd is reported, never substituted with 0 when missing.
    score = acceptance - token_penalty - wall_penalty
    return TrialScore(
        task=trial.task,
        accepted=accepted,
        ungraded=ungraded,
        prompt_tokens=prompt,
        completion_tokens=completion,
        tokens=tokens,
        wall_seconds=wall_seconds,
        cost_usd=cost_usd,
        score=score,
        token_penalty=token_penalty,
        wall_penalty=wall_penalty,
    )


def score_job(
    job: HarborJob | Path,
    *,
    suite: str = DEV_SUITE_ID,
    max_metric_calls: int | None = None,
    cost_usd: float | None = None,
    tasks: list[str] | None = None,
) -> JobScore:
    """Score a Harbor job directory or a loaded job.

    `max_metric_calls` caps how many trials enter the mean (one trial = one
    metric call). Remaining trials are not read as successes. `tasks` selects
    and orders trials by task id (suite order), not directory sort order.
    """
    loaded = job if isinstance(job, HarborJob) else load_job(job)
    if tasks:
        by_id = {trial.task: trial for trial in loaded.trials}
        trials = [by_id[task] for task in tasks if task in by_id]
    else:
        trials = list(loaded.trials)
    if max_metric_calls is not None:
        if max_metric_calls < 0:
            raise ValueError("max_metric_calls must be >= 0")
        trials = trials[:max_metric_calls]
    scored = tuple(score_trial(trial, cost_usd=None) for trial in trials)
    graded = sum(1 for trial in scored if not trial.ungraded)
    accepted = sum(1 for trial in scored if trial.accepted)
    ungraded = sum(1 for trial in scored if trial.ungraded)
    success_rate = (accepted / graded) if graded else None
    mean_score = (
        sum(trial.score for trial in scored) / len(scored) if scored else 0.0
    )
    prompt_tokens = sum(trial.prompt_tokens for trial in scored)
    completion_tokens = sum(trial.completion_tokens for trial in scored)
    wall_seconds = sum(trial.wall_seconds for trial in scored)
    disposition = "unmetered_or_unknown"
    if cost_usd is not None:
        disposition = "priced"
    elif accepted == 0 and scored:
        disposition = "no_accepted_outcomes"
    else:
        disposition = "cost_unknown"
    return JobScore(
        job_id=loaded.job_id,
        suite=suite,
        trials=scored,
        accepted=accepted,
        graded=graded,
        ungraded=ungraded,
        success_rate=success_rate,
        mean_score=mean_score,
        prompt_tokens=prompt_tokens,
        completion_tokens=completion_tokens,
        wall_seconds=wall_seconds,
        cost_usd=cost_usd,
        cost_disposition=disposition,
        metric_calls=len(scored),
    )
