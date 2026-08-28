"""Adapter: fixture scores, no holdout screening, no writes to surfaces/coder."""

from __future__ import annotations

import hashlib
import sys
import tempfile
import unittest
from pathlib import Path

_BENCH = Path(__file__).resolve().parents[2]
if str(_BENCH) not in sys.path:
    sys.path.insert(0, str(_BENCH))

from optimize.adapter import GymAdapter, GymTask, HoldoutScreenError  # noqa: E402
from optimize.surfaces import load_seed_candidate, repo_root_from, surfaces_dir  # noqa: E402

_FIXTURE = Path(__file__).resolve().parents[1] / "fixtures" / "tb2-quick-job"


def _digests(directory: Path) -> dict[str, str]:
    out: dict[str, str] = {}
    for path in sorted(directory.glob("*.json")):
        out[path.name] = hashlib.sha256(path.read_bytes()).hexdigest()
    return out


class AdapterTests(unittest.TestCase):
    def test_fixture_evaluate_scores_one_call(self) -> None:
        adapter = GymAdapter(
            fixture_job_dir=_FIXTURE,
            max_metric_calls=1,
            live=False,
        )
        batch = [
            GymTask(task_id="regex-log", suite="tb2-quick"),
            GymTask(task_id="openssl-selfsigned-cert", suite="tb2-quick"),
        ]
        result = adapter.evaluate(batch, {"system-prompt": "seed"}, capture_traces=True)
        self.assertEqual(len(result.scores), 2)
        self.assertEqual(result.num_metric_calls, 1)
        self.assertLess(result.scores[0], 0.0)
        self.assertEqual(result.scores[1], 0.0)
        self.assertEqual(result.trajectories[1]["skipped"], "max_metric_calls")
        self.assertEqual(adapter.calls_used, 1)

    def test_holdout_screening_is_refused(self) -> None:
        adapter = GymAdapter(fixture_job_dir=_FIXTURE, max_metric_calls=1)
        with self.assertRaises(HoldoutScreenError):
            adapter.evaluate(
                [GymTask(task_id="git-leak-recovery", suite="tb2-cross-section")],
                {"system-prompt": "seed"},
            )

    def test_does_not_write_surfaces(self) -> None:
        root = repo_root_from()
        before = _digests(surfaces_dir(root))
        seed = load_seed_candidate(root)
        adapter = GymAdapter(fixture_job_dir=_FIXTURE, max_metric_calls=1, repo_root=root)
        adapter.evaluate(
            [GymTask(task_id="regex-log", suite="tb2-quick")],
            seed,
            capture_traces=True,
        )
        after = _digests(surfaces_dir(root))
        self.assertEqual(before, after)

    def test_reflective_dataset_carries_atif_feedback(self) -> None:
        adapter = GymAdapter(fixture_job_dir=_FIXTURE, max_metric_calls=1)
        seed = {"system-prompt": '{"text":{}}'}
        batch = adapter.evaluate(
            [GymTask(task_id="regex-log", suite="tb2-quick")],
            seed,
            capture_traces=True,
        )
        dataset = adapter.make_reflective_dataset(seed, batch, ["system-prompt"])
        self.assertIn("system-prompt", dataset)
        self.assertEqual(len(dataset["system-prompt"]), 1)
        record = dataset["system-prompt"][0]
        self.assertIn("accepted=False", record["Feedback"])
        self.assertEqual(record["Inputs"]["task"], "regex-log")

    def test_output_dir_is_the_only_write_target(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            adapter = GymAdapter(
                fixture_job_dir=_FIXTURE,
                max_metric_calls=1,
                jobs_parent=Path(tmp),
            )
            adapter.evaluate(
                [GymTask(task_id="regex-log", suite="tb2-quick")],
                {"system-prompt": "x"},
            )
            # Fixture path does not create jobs; live would write under tmp.
            self.assertEqual(list(Path(tmp).iterdir()), [])


if __name__ == "__main__":
    unittest.main()
