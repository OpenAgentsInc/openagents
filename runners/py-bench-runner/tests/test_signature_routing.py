import json
import os
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from openagents_bench.evaluate_signatures import evaluate_fixture_dir
from openagents_bench.run_task import run_task
from openagents_bench.schemas import BenchmarkTask
from openagents_bench.signature_routing import build_signature_selector_trace, signature_prompt_addendum


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "fixtures" / "signature-routing"


EXPECTED_FAMILIES = {
    "service_readiness": "coding.service_readiness",
    "local_pypi_simple_index": "coding.python_package_index",
    "query_optimizer_workflow": "coding.query_optimizer_workflow",
    "sqlite_wal_recovery": "coding.sqlite_wal_recovery",
    "gcode_parser_guard": "coding.gcode_parser_guard",
    "xss_sanitizer_policy": "coding.xss_sanitizer_policy",
    "runner_stall_after_verifier": "benchmark.runner_supervisor",
}


class SignatureRoutingTests(unittest.TestCase):
    def load_fixture(self, path: Path) -> BenchmarkTask:
        return BenchmarkTask.from_dict(json.loads(path.read_text(encoding="utf-8")))

    def test_retained_terminal_bench_failure_fixtures_cover_expected_families(self):
        seen = {}

        for path in sorted(FIXTURES.glob("terminal-bench-retained-*.json")):
            task = self.load_fixture(path)
            trace = build_signature_selector_trace(task, "probe-codex")
            self.assertIsNotNone(trace, path.name)
            assert trace is not None

            family = trace["retainedFailureFamily"]
            seen[family] = trace["selectedSignatureIds"]
            self.assertEqual(task.dataset, "terminal-bench")
            self.assertEqual(task.version, "2.0")
            self.assertTrue(trace["selectionEnabled"])
            self.assertTrue(trace["routingPassed"])
            self.assertEqual(trace["selectedForbiddenSignatureIds"], [])
            self.assertGreater(len(trace["candidateSignatureIds"]), 0)
            self.assertGreater(len(trace["requiredEvidence"]), 0)
            self.assertGreater(len(trace["closeoutArtifacts"]), 0)

        self.assertEqual(set(seen), set(EXPECTED_FAMILIES))
        for family, expected_signature_id in EXPECTED_FAMILIES.items():
            self.assertEqual(seen[family], [expected_signature_id])

    def test_raw_codex_baseline_disables_signature_selection(self):
        task = self.load_fixture(FIXTURES / "terminal-bench-retained-configure-git-webserver.json")
        trace = build_signature_selector_trace(task, "codex")

        self.assertIsNotNone(trace)
        assert trace is not None
        self.assertFalse(trace["selectionEnabled"])
        self.assertEqual(trace["selectorMode"], "baseline_disabled")
        self.assertEqual(trace["selectedSignatureIds"], [])
        self.assertEqual(trace["expectedSignatureIds"], ["coding.service_readiness"])

    def test_probe_codex_prompt_addendum_carries_selected_signature_evidence(self):
        task = self.load_fixture(FIXTURES / "terminal-bench-retained-pypi-server.json")
        addendum = signature_prompt_addendum(task, "probe-codex")

        self.assertIn("coding.python_package_index", addendum)
        self.assertIn("pep503_index_contract", addendum)
        self.assertIn("PEP 503 Simple Repository API", addendum)
        self.assertIn("Run a local pip install smoke", addendum)
        self.assertIn("simple-index-tree.txt", addendum)
        self.assertEqual(signature_prompt_addendum(task, "codex"), "")

    def test_sqlite_wal_signature_carries_copy_before_open_playbook(self):
        task = self.load_fixture(FIXTURES / "terminal-bench-retained-db-wal-recovery.json")
        trace = build_signature_selector_trace(task, "probe-codex")
        addendum = signature_prompt_addendum(task, "probe-codex")

        self.assertIsNotNone(trace)
        assert trace is not None
        self.assertEqual(trace["rawCodexReward"], 0.0)
        self.assertEqual(trace["expectedProbeSignatureReward"], 1.0)
        self.assertEqual(trace["expectedRewardDelta"], 1.0)
        self.assertIn("copy the database, WAL, and SHM files as a matched set", addendum)
        self.assertIn("Open only the copied database", addendum)

    def test_retained_fixture_eval_shows_expected_probe_signature_improvement(self):
        result = evaluate_fixture_dir(FIXTURES, "probe-codex")

        self.assertEqual(result["fixtureCount"], len(list(FIXTURES.glob("terminal-bench-retained-*.json"))))
        self.assertGreater(result["improvedFixtureCount"], 0)
        self.assertGreater(result["expectedProbeSignatureMeanReward"], result["rawCodexMeanReward"])
        self.assertGreater(result["expectedMeanRewardDelta"], 0)

    def test_service_readiness_probe_codex_dry_run_preserves_selector_trace(self):
        with tempfile.TemporaryDirectory() as tmp:
            artifact_dir = Path(tmp) / "probe"
            with mock.patch.dict(os.environ, {"OPENAGENTS_BENCH_CODEX_DRY_RUN": "1"}):
                result = run_task(
                    run_id="run_signature_fixture",
                    task_run_id="taskrun_service_readiness",
                    task_spec=FIXTURES / "terminal-bench-retained-configure-git-webserver.json",
                    artifact_dir=artifact_dir,
                    agent_slug="probe-codex",
                    model="codex-account",
                )

            selector_trace = json.loads((artifact_dir / "signature_selector_trace.json").read_text(encoding="utf-8"))
            result_json = json.loads((artifact_dir / "result.json").read_text(encoding="utf-8"))
            events = (artifact_dir / "events.jsonl").read_text(encoding="utf-8")
            transcript = (artifact_dir / "transcript.md").read_text(encoding="utf-8")

            self.assertEqual(result.status, "passed")
            self.assertEqual(result_json["agentSlug"], "probe-codex")
            self.assertEqual(selector_trace["selectedSignatureIds"], ["coding.service_readiness"])
            self.assertTrue(selector_trace["routingPassed"])
            self.assertIn("signature_selector_trace_recorded", events)
            self.assertIn("coding.service_readiness", transcript)
            self.assertIn("port_probe", transcript)

    def test_raw_and_probe_codex_artifacts_are_comparable_for_same_fixture(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            fixture = FIXTURES / "terminal-bench-retained-configure-git-webserver.json"
            with mock.patch.dict(os.environ, {"OPENAGENTS_BENCH_CODEX_DRY_RUN": "1"}):
                run_task(
                    run_id="run_signature_fixture_raw",
                    task_run_id="taskrun_service_readiness_raw",
                    task_spec=fixture,
                    artifact_dir=root / "raw",
                    agent_slug="codex",
                    model="codex-account",
                )
                run_task(
                    run_id="run_signature_fixture_probe",
                    task_run_id="taskrun_service_readiness_probe",
                    task_spec=fixture,
                    artifact_dir=root / "probe",
                    agent_slug="probe-codex",
                    model="codex-account",
                )

            raw_trace = json.loads((root / "raw" / "signature_selector_trace.json").read_text(encoding="utf-8"))
            probe_trace = json.loads((root / "probe" / "signature_selector_trace.json").read_text(encoding="utf-8"))
            raw_result = json.loads((root / "raw" / "result.json").read_text(encoding="utf-8"))
            probe_result = json.loads((root / "probe" / "result.json").read_text(encoding="utf-8"))

            self.assertEqual(raw_result["taskId"], probe_result["taskId"])
            self.assertEqual(raw_trace["terminalBenchTaskId"], probe_trace["terminalBenchTaskId"])
            self.assertEqual(raw_trace["selectedSignatureIds"], [])
            self.assertEqual(probe_trace["selectedSignatureIds"], ["coding.service_readiness"])


if __name__ == "__main__":
    unittest.main()
