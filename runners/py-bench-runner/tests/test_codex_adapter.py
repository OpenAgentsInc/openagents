import json
import os
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from openagents_bench.codex_adapter import codex_command, is_codex_agent
from openagents_bench.run_task import run_task
from openagents_bench.schemas import BenchmarkTask


ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "fixtures" / "tasks" / "terminal-bench-codex-dry-run.json"


class CodexAdapterTests(unittest.TestCase):
    def test_codex_agent_slugs_are_explicit(self):
        self.assertTrue(is_codex_agent("codex"))
        self.assertTrue(is_codex_agent("openagents-codex"))
        self.assertTrue(is_codex_agent("openagents-coder"))
        self.assertTrue(is_codex_agent("probe-codex"))
        self.assertFalse(is_codex_agent("oracle"))

    def test_codex_command_uses_declared_sandbox_without_auth_material(self):
        task = BenchmarkTask.from_dict(json.loads(FIXTURE.read_text(encoding="utf-8")))
        command = codex_command(task)
        joined = " ".join(command)
        self.assertIn("--skip-git-repo-check", joined)
        self.assertIn("--json", joined)
        self.assertIn("danger-full-access", joined)
        self.assertNotIn("auth.json", joined)
        self.assertNotIn("token=secret-demo", joined)

    def test_codex_dry_run_writes_proof_bundle_ready_artifacts(self):
        with tempfile.TemporaryDirectory() as tmp:
            artifact_dir = Path(tmp)
            with mock.patch.dict(os.environ, {"OPENAGENTS_BENCH_CODEX_DRY_RUN": "1"}):
                result = run_task(
                    run_id="run_tb2_codex_dry",
                    task_run_id="taskrun_tb2_codex",
                    task_spec=FIXTURE,
                    artifact_dir=artifact_dir,
                    agent_slug="openagents-codex",
                    model="codex-account",
                )

            result_json = json.loads((artifact_dir / "result.json").read_text(encoding="utf-8"))
            proof_bundle = json.loads((artifact_dir / "proof_bundle.json").read_text(encoding="utf-8"))
            transcript = (artifact_dir / "transcript.md").read_text(encoding="utf-8")
            events = (artifact_dir / "events.jsonl").read_text(encoding="utf-8")

            self.assertEqual(result.status, "passed")
            self.assertEqual(result_json["agentSlug"], "openagents-codex")
            self.assertEqual(proof_bundle["provider"], "openagents-codex")
            self.assertEqual(proof_bundle["taskSelector"], "tb2-smoke-oracle")
            self.assertEqual(proof_bundle["timeoutSeconds"], 7200)
            self.assertGreater(len(proof_bundle["artifactDigests"]), 0)
            self.assertTrue((artifact_dir / "codex_stdout.jsonl").exists())
            self.assertTrue((artifact_dir / "workspace.diff").exists())
            self.assertIn("openagents_codex_queued", events)
            self.assertIn("openagents_codex_completed", events)
            self.assertNotIn("secret-demo", transcript)
            self.assertNotIn("auth.json", json.dumps(proof_bundle))


if __name__ == "__main__":
    unittest.main()
