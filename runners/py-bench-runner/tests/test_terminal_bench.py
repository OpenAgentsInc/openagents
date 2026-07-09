import json
import os
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from openagents_bench.run_task import run_task
from openagents_bench.terminal_bench import harbor_command
from openagents_bench.schemas import BenchmarkTask


ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "fixtures" / "tasks" / "terminal-bench-oracle-dry-run.json"


class TerminalBenchTests(unittest.TestCase):
    def test_harbor_command_targets_terminal_bench_version_and_task(self):
        task = BenchmarkTask.from_dict(json.loads(FIXTURE.read_text(encoding="utf-8")))
        self.assertEqual(
            harbor_command(task, "oracle"),
            [
                "harbor",
                "run",
                "--dataset",
                "terminal-bench@2.0",
                "--agent",
                "oracle",
                "--n-concurrent",
                "1",
                "--task-id",
                "tb2-smoke-oracle",
            ],
        )

    def test_terminal_bench_dry_run_writes_normalized_and_raw_artifacts(self):
        with tempfile.TemporaryDirectory() as tmp:
            artifact_dir = Path(tmp)
            with mock.patch.dict(os.environ, {"OPENAGENTS_BENCH_HARBOR_DRY_RUN": "1"}):
                result = run_task(
                    run_id="run_tb2_dry",
                    task_run_id="taskrun_tb2_oracle",
                    task_spec=FIXTURE,
                    artifact_dir=artifact_dir,
                    agent_slug="oracle",
                    model="terminal-bench-oracle",
                )

            result_json = json.loads((artifact_dir / "result.json").read_text(encoding="utf-8"))
            closeout = json.loads((artifact_dir / "cloud_execution_closeout.json").read_text(encoding="utf-8"))
            events = (artifact_dir / "events.jsonl").read_text(encoding="utf-8")

            self.assertEqual(result.status, "passed")
            self.assertEqual(result_json["dataset"], "terminal-bench")
            self.assertEqual(result_json["datasetVersion"], "2.0")
            self.assertEqual(result_json["agentSlug"], "oracle")
            self.assertTrue(result_json["verifier"]["passed"])
            self.assertEqual(closeout["dataset"], "terminal-bench")
            self.assertEqual(closeout["datasetVersion"], "2.0")
            self.assertFalse(closeout["walletAuthority"])
            self.assertFalse(closeout["publicClaimAuthority"])
            self.assertTrue(closeout["executionEvidenceOnly"])
            self.assertTrue((artifact_dir / "raw_harbor_result.json").exists())
            self.assertTrue((artifact_dir / "harbor_stdout.log").exists())
            self.assertIn("terminal_bench_harbor_queued", events)
            self.assertIn("terminal_bench_harbor_completed", events)


if __name__ == "__main__":
    unittest.main()
