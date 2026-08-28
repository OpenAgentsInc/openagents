"""Surface diffs and run envelope writes stay under --output."""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

_BENCH = Path(__file__).resolve().parents[2]
if str(_BENCH) not in sys.path:
    sys.path.insert(0, str(_BENCH))

from optimize.emit import build_candidate, surface_diffs, write_run  # noqa: E402
from optimize.job import load_job  # noqa: E402
from optimize.metric import score_job  # noqa: E402

_FIXTURE = Path(__file__).resolve().parents[1] / "fixtures" / "tb2-quick-job"


class EmitTests(unittest.TestCase):
    def test_surface_diffs_only_for_changed_components(self) -> None:
        seed = {"system-prompt": "alpha\n", "tool-descriptions": "beta\n", "catalog-lines": "gamma\n"}
        proposed = dict(seed)
        proposed["system-prompt"] = "alpha\nconcise\n"
        diffs = surface_diffs(seed, proposed)
        self.assertEqual([item["surface"] for item in diffs], ["system-prompt"])
        self.assertIn("concise", diffs[0]["diff"])
        self.assertIn("surfaces/coder/system-prompt.v1.json", diffs[0]["diff"])

    def test_mutated_candidate_carries_diff_and_acceptance(self) -> None:
        seed = {"system-prompt": "alpha\n", "tool-descriptions": "beta\n", "catalog-lines": "gamma\n"}
        proposed = dict(seed)
        proposed["tool-descriptions"] = "beta\ncapability-first\n"
        job_score = score_job(
            load_job(_FIXTURE),
            suite="tb2-quick",
            max_metric_calls=1,
            tasks=["regex-log"],
        )
        candidate = build_candidate(
            seed=seed,
            proposed=proposed,
            job_score=job_score,
            produced_by="test",
            transfer_label={"modelFamily": "fixture", "lane": "fixture"},
            parent=None,
            summary="Steer the shell description toward capability-first.",
            risk="Unmeasured on a live suite.",
        )
        self.assertEqual(len(candidate["surfaces"]), 1)
        self.assertEqual(candidate["surfaces"][0]["surface"], "tool-descriptions")
        with tempfile.TemporaryDirectory() as tmp:
            output = Path(tmp)
            write_run(
                output,
                run={"schema": "openagents.coder_optimizer_run.v1", "disposition": "candidate_emitted"},
                candidates=[candidate],
            )
            run = json.loads((output / "run.json").read_text())
            self.assertEqual(run["candidates"], [candidate["candidateId"]])
            files = list((output / "candidates").glob("*.json"))
            self.assertEqual(len(files), 1)
            self.assertEqual(json.loads(files[0].read_text())["candidateId"], candidate["candidateId"])


if __name__ == "__main__":
    unittest.main()
