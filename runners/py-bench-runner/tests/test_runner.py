import json
import tempfile
import unittest
from pathlib import Path

from openagents_bench.run_task import run_task


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "fixtures" / "tasks"


class RunnerTests(unittest.TestCase):
    def run_fixture(self, fixture_name):
        with tempfile.TemporaryDirectory() as tmp:
            artifact_dir = Path(tmp)
            result = run_task(
                run_id="run_test",
                task_run_id="taskrun_test",
                task_spec=FIXTURES / fixture_name,
                artifact_dir=artifact_dir,
                agent_slug="fake-agent",
                model="fake-model",
            )
            result_json = json.loads((artifact_dir / "result.json").read_text(encoding="utf-8"))
            events = (artifact_dir / "events.jsonl").read_text(encoding="utf-8")
            metadata = json.loads((artifact_dir / "metadata.json").read_text(encoding="utf-8"))
            manifest = json.loads((artifact_dir / "artifact_manifest.json").read_text(encoding="utf-8"))
            proof_bundle = json.loads((artifact_dir / "proof_bundle.json").read_text(encoding="utf-8"))
            closeout = json.loads((artifact_dir / "cloud_execution_closeout.json").read_text(encoding="utf-8"))
            resource_receipt = json.loads(
                (artifact_dir / "resource_usage_receipt.json").read_text(encoding="utf-8")
            )
            files = {path.name for path in artifact_dir.iterdir() if path.is_file()}
            return result, result_json, events, metadata, manifest, proof_bundle, closeout, resource_receipt, files

    def test_fake_pass_writes_required_and_optional_artifacts(self):
        (
            result,
            result_json,
            events,
            metadata,
            manifest,
            proof_bundle,
            closeout,
            resource_receipt,
            files,
        ) = self.run_fixture("fake-pass.json")

        self.assertEqual(result.status, "passed")
        self.assertEqual(result_json["status"], "passed")
        self.assertTrue(result_json["verifier"]["passed"])
        self.assertEqual(metadata["harnessVersion"], "oa-bench-runner/0.1.0")
        self.assertEqual(manifest["runId"], "run_test")
        self.assertEqual(proof_bundle["claimState"], "internal")
        self.assertFalse(proof_bundle["walletAuthority"])
        self.assertFalse(proof_bundle["payoutAuthority"])
        self.assertFalse(proof_bundle["publicClaimAuthority"])
        self.assertEqual(resource_receipt["schemaVersion"], "openagents.resource_usage_receipt.v1")
        self.assertEqual(resource_receipt["modelUsage"][0]["countSource"], "unavailable")
        self.assertEqual(
            resource_receipt["modelUsage"][0]["unavailableReason"],
            "runner_adapter_did_not_report_model_tokens",
        )
        self.assertEqual(proof_bundle["resourceUsageReceiptDigest"], resource_receipt["receiptDigest"])
        self.assertEqual(proof_bundle["cloudExecutionCloseoutDigest"], closeout["closeoutDigest"])
        self.assertEqual(closeout["schemaVersion"], "openagents.benchmark_cloud_closeout.v1")
        self.assertEqual(closeout["claimState"], "internal")
        self.assertTrue(closeout["publicSafe"])
        self.assertFalse(closeout["walletAuthority"])
        self.assertFalse(closeout["payoutAuthority"])
        self.assertFalse(closeout["publicClaimAuthority"])
        self.assertEqual(closeout["authorityOwner"], "omega")
        self.assertIn("task_started", events)
        self.assertIn("task_finished", events)
        self.assertIn("commands.jsonl", files)
        self.assertIn("transcript.md", files)
        self.assertIn("verifier_stdout.log", files)
        self.assertIn("resource_usage_receipt.json", files)
        self.assertIn("cloud_execution_closeout.json", files)
        self.assertTrue(any(artifact["path"] == "events.jsonl" for artifact in result_json["artifacts"]))
        self.assertTrue(any(artifact["path"] == "cloud_execution_closeout.json" for artifact in manifest["artifacts"]))

    def test_fake_timeout_still_writes_required_artifacts(self):
        (
            result,
            result_json,
            events,
            metadata,
            manifest,
            proof_bundle,
            closeout,
            resource_receipt,
            files,
        ) = self.run_fixture("fake-timeout.json")

        self.assertEqual(result.status, "timeout")
        self.assertEqual(result_json["status"], "timeout")
        self.assertEqual(metadata["dataset"], "openagents-fake")
        self.assertEqual(proof_bundle["status"], "timeout")
        self.assertEqual(closeout["status"], "timeout")
        self.assertTrue(closeout["executionEvidenceOnly"])
        self.assertEqual(resource_receipt["run"]["wallTimeMs"], result_json["usage"]["wallTimeMs"])
        self.assertIn("task_timeout", events)
        self.assertIn("result.json", files)
        self.assertIn("events.jsonl", files)
        self.assertIn("metadata.json", files)

    def test_fake_exception_still_writes_required_artifacts(self):
        (
            result,
            result_json,
            events,
            metadata,
            manifest,
            proof_bundle,
            closeout,
            resource_receipt,
            files,
        ) = self.run_fixture("fake-exception.json")

        self.assertEqual(result.status, "error")
        self.assertEqual(result_json["status"], "error")
        self.assertEqual(metadata["taskId"], "fake-exception")
        self.assertEqual(proof_bundle["status"], "error")
        self.assertEqual(closeout["status"], "error")
        self.assertTrue(resource_receipt["receiptDigest"].startswith("sha256:"))
        self.assertIn("task_exception", events)
        self.assertIn("result.json", files)
        self.assertIn("events.jsonl", files)
        self.assertIn("metadata.json", files)

    def test_secret_like_instruction_is_redacted_from_public_artifacts(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            task_spec = root / "secret-task.json"
            artifact_dir = root / "artifacts"
            task_spec.write_text(
                json.dumps(
                    {
                        "id": "fake-pass",
                        "dataset": "openagents-fake",
                        "version": "0.1",
                        "instruction": "Use token=supersecret password:do-not-log sk-testsecret0123456789.",
                        "environment": {"kind": "fake", "fakeBehavior": "pass"},
                        "verifier": {"kind": "fake"},
                        "limits": {"timeoutSeconds": 5},
                        "metadata": {},
                    }
                ),
                encoding="utf-8",
            )

            run_task(
                run_id="run_test",
                task_run_id="taskrun_test",
                task_spec=task_spec,
                artifact_dir=artifact_dir,
                agent_slug="fake-agent",
                model="fake-model",
            )

            transcript = (artifact_dir / "transcript.md").read_text(encoding="utf-8")
            self.assertNotIn("supersecret", transcript)
            self.assertNotIn("do-not-log", transcript)
            self.assertNotIn("sk-testsecret0123456789", transcript)
            self.assertIn("token=<redacted>", transcript)
            for artifact in artifact_dir.iterdir():
                if artifact.is_file():
                    raw = artifact.read_text(encoding="utf-8")
                    self.assertNotIn("supersecret", raw, artifact.name)
                    self.assertNotIn("do-not-log", raw, artifact.name)
                    self.assertNotIn("sk-testsecret0123456789", raw, artifact.name)


if __name__ == "__main__":
    unittest.main()
