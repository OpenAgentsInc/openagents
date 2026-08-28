"""Metric: acceptance minus token and wall terms; unknown USD is not zero."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

_BENCH = Path(__file__).resolve().parents[2]
if str(_BENCH) not in sys.path:
    sys.path.insert(0, str(_BENCH))

from optimize.job import HarborJob, HarborTrial, load_job  # noqa: E402
from optimize.metric import (  # noqa: E402
    TOKEN_PENALTY_PER_MILLION,
    WALL_PENALTY_PER_HOUR,
    score_job,
    score_trial,
)

_FIXTURE = Path(__file__).resolve().parents[1] / "fixtures" / "tb2-quick-job"


def _trial(
    *,
    task: str,
    reward: float | None,
    prompt: int,
    completion: int,
    wall_seconds: int,
    verifier_ran: bool = True,
) -> HarborTrial:
    finished_minute, finished_second = divmod(wall_seconds, 60)
    finished_hour, finished_minute = divmod(finished_minute, 60)
    result = {
        "task_name": task,
        "agent_execution": {
            "started_at": "2026-08-28T00:00:00Z",
            "finished_at": f"2026-08-28T{finished_hour:02d}:{finished_minute:02d}:{finished_second:02d}Z",
        },
        "verifier_result": None
        if not verifier_ran
        else {"rewards": {"reward": reward}},
    }
    trajectory = {
        "final_metrics": {
            "total_prompt_tokens": prompt,
            "total_completion_tokens": completion,
        }
    }
    return HarborTrial(
        path=Path(task),
        task=task,
        result=result,
        trajectory=trajectory,
        coder_log=None,
    )


class MetricTests(unittest.TestCase):
    def test_acceptance_and_token_penalty_live_inside_score(self) -> None:
        cheap = score_trial(_trial(task="a", reward=1, prompt=1_000_000, completion=0, wall_seconds=0))
        dear = score_trial(_trial(task="a", reward=1, prompt=2_000_000, completion=0, wall_seconds=0))
        self.assertTrue(cheap.accepted)
        self.assertAlmostEqual(cheap.score, 1.0 - TOKEN_PENALTY_PER_MILLION)
        self.assertAlmostEqual(dear.score, 1.0 - 2 * TOKEN_PENALTY_PER_MILLION)
        self.assertLess(dear.score, cheap.score)

    def test_wall_penalty_lives_inside_score(self) -> None:
        short = score_trial(_trial(task="a", reward=1, prompt=0, completion=0, wall_seconds=0))
        hour = score_trial(_trial(task="a", reward=1, prompt=0, completion=0, wall_seconds=3600))
        self.assertAlmostEqual(short.score, 1.0)
        self.assertAlmostEqual(hour.score, 1.0 - WALL_PENALTY_PER_HOUR)
        self.assertLess(hour.score, short.score)

    def test_failed_trial_is_zero_acceptance_minus_penalties(self) -> None:
        scored = score_trial(_trial(task="a", reward=0, prompt=1_000_000, completion=0, wall_seconds=0))
        self.assertFalse(scored.accepted)
        self.assertFalse(scored.ungraded)
        self.assertAlmostEqual(scored.score, -TOKEN_PENALTY_PER_MILLION)

    def test_ungraded_is_not_a_success(self) -> None:
        scored = score_trial(
            _trial(task="a", reward=1, prompt=0, completion=0, wall_seconds=0, verifier_ran=False)
        )
        self.assertTrue(scored.ungraded)
        self.assertFalse(scored.accepted)
        self.assertEqual(scored.score, 0.0)

    def test_unknown_usd_is_not_zero(self) -> None:
        scored = score_trial(_trial(task="a", reward=1, prompt=0, completion=0, wall_seconds=0))
        self.assertIsNone(scored.cost_usd)
        job = HarborJob(
            path=Path("."),
            job_id="x",
            result={},
            config={},
            trials=(
                _trial(task="a", reward=1, prompt=0, completion=0, wall_seconds=0),
            ),
        )
        summary = score_job(job, cost_usd=None)
        self.assertIsNone(summary.cost_usd)
        self.assertEqual(summary.cost_disposition, "cost_unknown")

    def test_fixture_job_two_trials_and_max_metric_calls_one(self) -> None:
        job = load_job(_FIXTURE)
        order = ["regex-log", "openssl-selfsigned-cert"]
        full = score_job(job, suite="tb2-quick", tasks=order)
        self.assertEqual(full.metric_calls, 2)
        self.assertEqual(full.accepted, 1)
        self.assertEqual(full.graded, 2)
        self.assertEqual(full.success_rate, 0.5)
        self.assertEqual(full.prompt_tokens, 20000)
        self.assertEqual(full.completion_tokens, 600)
        one = score_job(job, suite="tb2-quick", max_metric_calls=1, tasks=order)
        self.assertEqual(one.metric_calls, 1)
        self.assertEqual(one.trials[0].task, "regex-log")
        self.assertFalse(one.trials[0].accepted)
        self.assertLess(one.mean_score, 0.0)


if __name__ == "__main__":
    unittest.main()
