"""CLI dry-run: max_metric_calls=1 completes, writes only under --output."""

from __future__ import annotations

import io
import json
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path

_BENCH = Path(__file__).resolve().parents[2]
if str(_BENCH) not in sys.path:
    sys.path.insert(0, str(_BENCH))

from optimize.cli import main  # noqa: E402
from optimize.surfaces import repo_root_from, surfaces_dir  # noqa: E402
from optimize.tests.test_adapter import _digests  # noqa: E402


def _main(argv: list[str]) -> int:
    stdout = io.StringIO()
    stderr = io.StringIO()
    with redirect_stdout(stdout), redirect_stderr(stderr):
        return main(argv)


class CliTests(unittest.TestCase):
    def test_dry_run_max_metric_calls_one(self) -> None:
        root = repo_root_from()
        before = _digests(surfaces_dir(root))
        with tempfile.TemporaryDirectory() as tmp:
            output = Path(tmp) / "run"
            code = _main(
                [
                    "--dry-run",
                    "--max-metric-calls",
                    "1",
                    "--output",
                    str(output),
                ]
            )
            self.assertEqual(code, 0)
            run_path = output / "run.json"
            self.assertTrue(run_path.is_file())
            run = json.loads(run_path.read_text())
            self.assertEqual(run["schema"], "openagents.coder_optimizer_run.v1")
            self.assertEqual(run["disposition"], "no_beat_incumbent")
            self.assertEqual(run["harbor"], "fixture")
            self.assertEqual(run["metricCalls"], 1)
            self.assertEqual(run["maxMetricCalls"], 1)
            self.assertFalse(run["holdoutRan"])
            self.assertEqual(run["devSuite"], "tb2-quick")
            self.assertEqual(run["holdoutSuite"], "tb2-cross-section")
            self.assertEqual(run["residual"], "no live Harbor cycle in this packet")
            self.assertEqual(run["acceptance"]["trials"], 1)
            self.assertEqual(run["acceptance"]["floorKind"], "structural")
            self.assertEqual(run["transferLabel"]["modelFamily"], "fixture")
            candidates = list((output / "candidates").glob("*.json"))
            self.assertEqual(len(candidates), 1)
            candidate = json.loads(candidates[0].read_text())
            self.assertEqual(candidate["schema"], "openagents.coder_candidate.v1")
            self.assertEqual(candidate["lineage"]["origin"], "optimizer")
            self.assertEqual(candidate["lever"]["axis"], "optimizer")
            self.assertEqual(candidate["surfaces"], [])
            refs = {entry["ref"] for entry in candidate["evidence"]}
            self.assertIn("trial:regex-log#outcome", refs)
            self.assertIn("ledger:O1", refs)
            self.assertIn("ledger:O4", refs)
            self.assertTrue(candidate["candidateId"].startswith("candidate:"))
            self.assertEqual(_digests(surfaces_dir(root)), before)
            # Nothing escaped the output dir besides what we wrote.
            written = {path.relative_to(output) for path in output.rglob("*") if path.is_file()}
            self.assertIn(Path("run.json"), written)
            self.assertTrue(any(path.parts[0] == "candidates" for path in written))

    def test_holdout_as_dev_suite_is_refused(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            code = _main(
                [
                    "--dry-run",
                    "--max-metric-calls",
                    "1",
                    "--dev-suite",
                    "tb2-cross-section",
                    "--output",
                    str(Path(tmp) / "run"),
                ]
            )
            self.assertEqual(code, 3)

    def test_confirm_holdout_is_refused(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            code = _main(
                [
                    "--dry-run",
                    "--max-metric-calls",
                    "1",
                    "--confirm-holdout",
                    "--output",
                    str(Path(tmp) / "run"),
                ]
            )
            self.assertEqual(code, 2)

    def test_gepa_engine_without_live_is_refused(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            code = _main(
                [
                    "--dry-run",
                    "--engine",
                    "gepa",
                    "--max-metric-calls",
                    "1",
                    "--output",
                    str(Path(tmp) / "run"),
                ]
            )
            self.assertEqual(code, 2)


if __name__ == "__main__":
    unittest.main()
