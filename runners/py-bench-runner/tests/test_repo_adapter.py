import json
import os
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from openagents_bench.repo_adapter import is_repo_dataset, verifier_commands
from openagents_bench.run_task import run_task
from openagents_bench.schemas import BenchmarkTask


ROOT = Path(__file__).resolve().parents[1]
CUSTOM_FIXTURE = ROOT / "fixtures" / "tasks" / "custom-repo-dry-run.json"
SWE_FIXTURE = ROOT / "fixtures" / "tasks" / "swe-bench-dry-run.json"


class RepoAdapterTests(unittest.TestCase):
    def test_repo_dataset_slugs_are_explicit(self):
        self.assertTrue(is_repo_dataset("custom-repo"))
        self.assertTrue(is_repo_dataset("swe-bench"))
        self.assertTrue(is_repo_dataset("swt-bench"))
        self.assertFalse(is_repo_dataset("terminal-bench"))

    def test_verifier_commands_are_normalized(self):
        task = BenchmarkTask.from_dict(json.loads(CUSTOM_FIXTURE.read_text(encoding="utf-8")))
        self.assertEqual(verifier_commands(task), ["test -f README.md", "test -f solution.txt"])

    def test_custom_repo_dry_run_writes_patch_diff_and_manifest(self):
        with tempfile.TemporaryDirectory() as tmp:
            artifact_dir = Path(tmp)
            with mock.patch.dict(os.environ, {"OPENAGENTS_BENCH_REPO_DRY_RUN": "1"}):
                result = run_task(
                    run_id="run_custom_repo_dry",
                    task_run_id="taskrun_custom_repo",
                    task_spec=CUSTOM_FIXTURE,
                    artifact_dir=artifact_dir,
                    agent_slug="openagents-codex",
                    model="codex-account",
                )

            result_json = json.loads((artifact_dir / "result.json").read_text(encoding="utf-8"))
            proof_bundle = json.loads((artifact_dir / "proof_bundle.json").read_text(encoding="utf-8"))
            manifest = json.loads((artifact_dir / "artifact_manifest.json").read_text(encoding="utf-8"))

            self.assertEqual(result.status, "passed")
            self.assertEqual(result_json["dataset"], "custom-repo")
            self.assertEqual(result_json["taskId"], "custom-repo-smoke")
            self.assertEqual(proof_bundle["taskSelector"], "custom-repo-smoke")
            self.assertEqual(proof_bundle["provider"], "openagents-codex")
            self.assertTrue((artifact_dir / "workspace.diff").exists())
            self.assertTrue((artifact_dir / "patch.diff").exists())
            self.assertTrue((artifact_dir / "repo_result.json").exists())
            self.assertTrue(any(artifact["path"] == "patch.diff" for artifact in manifest["artifacts"]))

    def test_swe_bench_dry_run_uses_same_normalized_contract(self):
        with tempfile.TemporaryDirectory() as tmp:
            artifact_dir = Path(tmp)
            with mock.patch.dict(os.environ, {"OPENAGENTS_BENCH_REPO_DRY_RUN": "1"}):
                result = run_task(
                    run_id="run_swe_dry",
                    task_run_id="taskrun_swe",
                    task_spec=SWE_FIXTURE,
                    artifact_dir=artifact_dir,
                    agent_slug="openagents-codex",
                    model="codex-account",
                )

            result_json = json.loads((artifact_dir / "result.json").read_text(encoding="utf-8"))
            proof_bundle = json.loads((artifact_dir / "proof_bundle.json").read_text(encoding="utf-8"))

            self.assertEqual(result.status, "passed")
            self.assertEqual(result_json["dataset"], "swe-bench")
            self.assertEqual(result_json["datasetVersion"], "verified-smoke")
            self.assertEqual(proof_bundle["taskSelector"], "swe-bench-smoke")
            self.assertGreater(len(proof_bundle["artifactDigests"]), 0)


if __name__ == "__main__":
    unittest.main()
