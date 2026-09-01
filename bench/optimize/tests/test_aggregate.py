"""Multi-trial aggregation: mean ± spread, within-noise A/B, honest unknowns."""

from __future__ import annotations

import statistics
import sys
import unittest
from pathlib import Path

_BENCH = Path(__file__).resolve().parents[2]
if str(_BENCH) not in sys.path:
    sys.path.insert(0, str(_BENCH))

from optimize.aggregate import (  # noqa: E402
    DEFAULT_N_TRIALS,
    Aggregate,
    aggregate_side,
    aggregate_values,
    compare_sides,
)
from optimize.engine import OptimizeConfig  # noqa: E402
from optimize.job import HarborJob, HarborTrial  # noqa: E402
from optimize.metric import score_job  # noqa: E402


def _trial(*, task: str, reward: float | None, prompt: int, completion: int) -> HarborTrial:
    result = {
        "task_name": task,
        "agent_execution": {
            "started_at": "2026-09-01T00:00:00Z",
            "finished_at": "2026-09-01T00:00:00Z",
        },
        "verifier_result": {"rewards": {"reward": reward}},
    }
    trajectory = {
        "final_metrics": {
            "total_prompt_tokens": prompt,
            "total_completion_tokens": completion,
        }
    }
    return HarborTrial(path=Path(task), task=task, result=result, trajectory=trajectory, coder_log=None)


def _job(*, reward: float | None, prompt: int = 0, completion: int = 0) -> HarborJob:
    return HarborJob(
        path=Path("."),
        job_id="x",
        result={},
        config={},
        trials=(_trial(task="a", reward=reward, prompt=prompt, completion=completion),),
    )


def _agg(mean: float, stddev: float) -> Aggregate:
    return Aggregate(n=3, n_unknown=0, mean=mean, stddev=stddev, minimum=mean, maximum=mean)


class AggregateValuesTests(unittest.TestCase):
    def test_mean_and_spread_over_multiple_trials(self) -> None:
        agg = aggregate_values([0.0, 1.0, 2.0])
        self.assertEqual(agg.n, 3)
        self.assertEqual(agg.n_unknown, 0)
        self.assertAlmostEqual(agg.mean, 1.0)
        self.assertAlmostEqual(agg.stddev, statistics.stdev([0.0, 1.0, 2.0]))
        self.assertAlmostEqual(agg.stddev, 1.0)
        self.assertEqual(agg.minimum, 0.0)
        self.assertEqual(agg.maximum, 2.0)

    def test_identical_trials_have_zero_spread(self) -> None:
        agg = aggregate_values([1.0, 1.0, 1.0])
        self.assertAlmostEqual(agg.mean, 1.0)
        self.assertEqual(agg.stddev, 0.0)

    def test_single_known_value_has_zero_spread(self) -> None:
        agg = aggregate_values([0.75])
        self.assertEqual(agg.n, 1)
        self.assertAlmostEqual(agg.mean, 0.75)
        self.assertEqual(agg.stddev, 0.0)

    def test_unknown_trials_are_carried_not_zeroed(self) -> None:
        agg = aggregate_values([1.0, None, 3.0])
        # The None is excluded from the mean, never read as a 0.
        self.assertEqual(agg.n, 2)
        self.assertEqual(agg.n_unknown, 1)
        self.assertAlmostEqual(agg.mean, 2.0)
        self.assertAlmostEqual(agg.stddev, statistics.stdev([1.0, 3.0]))

    def test_all_unknown_stays_unknown(self) -> None:
        agg = aggregate_values([None, None])
        self.assertEqual(agg.n, 0)
        self.assertEqual(agg.n_unknown, 2)
        self.assertIsNone(agg.mean)
        self.assertIsNone(agg.stddev)


class AggregateSideTests(unittest.TestCase):
    def test_side_aggregates_objective_score_over_trials(self) -> None:
        # Three trials: two accepted (score ~1.0), one failed (score ~0.0).
        scores = [
            score_job(_job(reward=1), suite="tb2-quick"),
            score_job(_job(reward=1), suite="tb2-quick"),
            score_job(_job(reward=0), suite="tb2-quick"),
        ]
        side = aggregate_side("candidate", scores)
        self.assertEqual(side.n_trials, 3)
        self.assertEqual(side.objective_score.n, 3)
        self.assertAlmostEqual(side.objective_score.mean, statistics.fmean([1.0, 1.0, 0.0]))
        self.assertAlmostEqual(side.objective_score.stddev, statistics.stdev([1.0, 1.0, 0.0]))
        self.assertEqual(side.objective_score.minimum, 0.0)
        self.assertEqual(side.objective_score.maximum, 1.0)

    def test_side_carries_unknown_cost_not_zero(self) -> None:
        scores = [score_job(_job(reward=1), suite="tb2-quick") for _ in range(3)]
        side = aggregate_side("incumbent", scores)
        # score_job leaves cost_usd unpriced; the aggregate must not read 0.
        self.assertIsNone(side.cost_usd.mean)
        self.assertEqual(side.cost_usd.n, 0)
        self.assertEqual(side.cost_usd.n_unknown, 3)
        self.assertEqual(side.cost_disposition, "cost_unknown")

    def test_side_requires_at_least_one_score(self) -> None:
        with self.assertRaises(ValueError):
            aggregate_side("candidate", [])


class CompareSidesTests(unittest.TestCase):
    def test_within_noise_fires_when_spreads_overlap(self) -> None:
        control = _agg(mean=1.0, stddev=0.5)
        candidate = _agg(mean=1.2, stddev=0.5)
        result = compare_sides(control, candidate)
        # |0.2| <= 0.5 + 0.5, so the delta is inside the combined spread.
        self.assertTrue(result.within_noise)
        self.assertEqual(result.disposition, "within_noise")
        self.assertAlmostEqual(result.delta, 0.2)
        self.assertAlmostEqual(result.combined_spread, 1.0)
        # No spurious percentage on a within-noise delta.
        self.assertIsNone(result.delta_pct)

    def test_within_noise_does_not_fire_on_clear_separation(self) -> None:
        control = _agg(mean=1.0, stddev=0.05)
        candidate = _agg(mean=2.0, stddev=0.05)
        result = compare_sides(control, candidate)
        # |1.0| > 0.05 + 0.05, a clean separation.
        self.assertFalse(result.within_noise)
        self.assertEqual(result.disposition, "candidate_higher")
        self.assertAlmostEqual(result.delta, 1.0)
        self.assertAlmostEqual(result.combined_spread, 0.1)
        self.assertAlmostEqual(result.delta_pct, 100.0)

    def test_clear_regression_reports_lower(self) -> None:
        control = _agg(mean=2.0, stddev=0.05)
        candidate = _agg(mean=1.0, stddev=0.05)
        result = compare_sides(control, candidate)
        self.assertFalse(result.within_noise)
        self.assertEqual(result.disposition, "candidate_lower")
        self.assertAlmostEqual(result.delta, -1.0)

    def test_unknown_mean_is_not_a_comparison(self) -> None:
        control = Aggregate(n=0, n_unknown=3, mean=None, stddev=None, minimum=None, maximum=None)
        candidate = _agg(mean=1.0, stddev=0.05)
        result = compare_sides(control, candidate)
        self.assertEqual(result.disposition, "unknown")
        self.assertIsNone(result.delta)
        self.assertFalse(result.within_noise)

    def test_negligible_baseline_withholds_percentage(self) -> None:
        control = _agg(mean=0.0, stddev=0.001)
        candidate = _agg(mean=0.5, stddev=0.001)
        result = compare_sides(control, candidate)
        self.assertFalse(result.within_noise)
        self.assertEqual(result.disposition, "candidate_higher")
        # A percentage off a ~0 baseline is meaningless; it is withheld.
        self.assertIsNone(result.delta_pct)


class DefaultTrialsTests(unittest.TestCase):
    def test_default_n_trials_is_at_least_three(self) -> None:
        self.assertGreaterEqual(DEFAULT_N_TRIALS, 3)

    def test_optimize_config_defaults_to_multi_trial(self) -> None:
        config = OptimizeConfig(output=Path("."))
        self.assertGreaterEqual(config.n_trials, 3)
        self.assertEqual(config.n_trials, DEFAULT_N_TRIALS)


if __name__ == "__main__":
    unittest.main()
