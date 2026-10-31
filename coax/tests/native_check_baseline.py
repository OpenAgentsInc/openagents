#!/usr/bin/env python3
"""Strict baseline qualification, distinct from legacy subset agreement."""
import hashlib
import copy
import contextlib
import io
import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "tests/native-check-baseline"
SPEC = importlib.util.spec_from_file_location("coax_native_check_baseline", ROOT / "tools/native_check_baseline.py")
baseline = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = baseline
SPEC.loader.exec_module(baseline)


class NativeBaselineTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.shared = tempfile.TemporaryDirectory(prefix="coax-native-baseline-good-")
        cls.good = None
        cls.good_commands = None
        cls.authority_good = None

    @classmethod
    def tearDownClass(cls):
        cls.shared.cleanup()

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="coax-native-baseline-test-")
        self.dir = Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()

    def run_fixture(self, name, **options):
        return baseline.run([FIXTURES / (name + ".coax")], keep=self.dir / "native", **options)

    def known_good(self):
        """One real passing baseline/native receipt, reused as immutable evidence."""
        cls = type(self)
        if cls.good is None:
            commands = []
            execute = baseline.nc.execute
            source = FIXTURES / "simple-pass.coax"
            original = source.read_bytes()
            def observe(command, *args, **options):
                command = [str(part) for part in command]
                commands.append(command)
                if len(command) > 1 and command[1] == "test":
                    self.assertEqual(Path(command[2]).resolve(), source.resolve())
                    self.assertEqual(Path(command[2]).read_bytes(), original)
                return execute(command, *args, **options)
            with mock.patch.object(baseline.nc, "execute", side_effect=observe):
                cls.good = baseline.run([source], keep=Path(cls.shared.name) / "native")
            cls.good_commands = commands
            self.assertEqual(source.read_bytes(), original)
        self.assertEqual(cls.good["outcome"], "passed", cls.good)
        return copy.deepcopy(cls.good)

    def one(self, record):
        self.assertEqual(len(record["files"]), 1)
        item = record["files"][0]
        for entry in item["native_coverage"]:
            self.assertTrue({"kind", "name", "at", "outcome", "why"} <= entry.keys(), entry)
        return item

    def coverage(self, item, name):
        return [entry for entry in item["native_coverage"] if entry["name"] == name]

    def known_authority(self):
        """Capture once, or explicitly reuse a source/compiler-bound real receipt."""
        cls = type(self)
        source = FIXTURES / "successful-authority.coax"
        receipt = os.environ.get("COAX_BASELINE_AUTHORITY_RECEIPT")
        if cls.authority_good is None:
            if receipt and Path(receipt).exists():
                cls.authority_good = json.loads(Path(receipt).read_text())
            else:
                cls.authority_good = baseline.run(
                    [source], keep=Path(cls.shared.name) / "authority-native")
                if receipt:
                    Path(receipt).write_text(json.dumps(cls.authority_good, indent=2) + "\n")
        record = copy.deepcopy(cls.authority_good)
        self.assertEqual(record["outcome"], "passed", record)
        evidence = self.one(record)["baseline"]
        sources = {Path(s["path"]).resolve(): s["sha256"] for s in evidence["sources"]}
        self.assertEqual(sources[source.resolve()], hashlib.sha256(source.read_bytes()).hexdigest())
        self.assertEqual(evidence["compiler"]["sha256"],
                         hashlib.sha256(baseline.nc.compiler().read_bytes()).hexdigest())
        return record

    def nonpass(self, record):
        self.assertNotEqual(record["outcome"], "passed", record)
        self.assertNotEqual(self.one(record)["outcome"], "passed", record)

    def test_original_mixed_check_and_failing_forall_never_qualifies(self):
        source = FIXTURES / "mixed-quantified-failure.coax"
        self.assertEqual(hashlib.sha256(source.read_bytes()).hexdigest(),
                         "317218a3086427a7cf25ed5627eeff8b7eec414f8eae1cb8ba2c9ca2a4f7c976")
        record = self.run_fixture("mixed-quantified-failure")
        if os.environ.get("COAX_BASELINE_RED_RECORD"):
            Path(os.environ["COAX_BASELINE_RED_RECORD"]).write_text(json.dumps(record, indent=2) + "\n")
        self.assertEqual(record["schema"], "coax.native-checks.baseline.v1")
        self.assertNotEqual(record["outcome"], "passed", "A passing eligible #test must not hide the original failing #forall.")
        item = self.one(record)
        evidence = item["baseline"]
        self.assertEqual(evidence["schema"], "coax.evidence.v1")
        self.assertEqual(item["baseline_execution"]["returncode"], 1)
        failed = next(c for c in evidence["claims"] if c["kind"] == "#forall")
        self.assertEqual(failed["name"], "failing quantified baseline")
        self.assertEqual(failed["outcome"], "counterexample")
        self.assertEqual(failed["counterexample"], "i = 0")
        self.assertEqual(failed["at"]["line"], 2)
        sources = {Path(s["path"]).resolve(): s["sha256"] for s in evidence["sources"]}
        self.assertEqual(sources[source.resolve()], hashlib.sha256(source.read_bytes()).hexdigest())
        self.assertTrue(evidence["profile"]["library"])

    def test_supported_original_source_passes_with_one_default_budget_baseline(self):
        record = self.known_good()
        item = self.one(record)
        evidence = item["baseline"]
        self.assertEqual(evidence["verdict"], "passed")
        self.assertEqual(evidence["budgets"]["check_steps"], 20_000_000)
        self.assertIsNone(evidence["selection"]["only"])
        self.assertFalse(evidence["selection"]["list"])
        self.assertFalse(evidence["selection"]["root_only"])
        self.assertEqual(record["differential"]["schema"], "coax.native-checks.v1")
        self.assertEqual(record["differential"]["outcome"], "passed")
        native = self.coverage(item, "supported product")
        self.assertEqual(len(native), 1)
        self.assertEqual(native[0]["outcome"], "passed")
        self.assertEqual(native[0]["kind"], "#test")
        tests = [c for c in type(self).good_commands if len(c) > 1 and c[1] == "test"]
        self.assertEqual(len(tests), 1, "The baseline observation must reach legacy comparison without another evaluator run.")
        self.assertIn("--library", tests[0])
        self.assertNotIn("--steps", tests[0], "The ordinary default must not become the legacy helper's 400M override.")
        self.assertNotIn(".native-check-eval-", tests[0][2])
        self.assertEqual(Path(evidence["input"]).resolve(), (FIXTURES / "simple-pass.coax").resolve())

    def test_actual_tiny_budget_exhaustion_is_not_a_counterexample_or_pass(self):
        record = self.run_fixture("step-limit", steps=10)
        self.nonpass(record)
        evidence = self.one(record)["baseline"]
        self.assertEqual(evidence["budgets"]["check_steps"], 10)
        limited = next(c for c in evidence["claims"] if c["name"] == "bounded work exceeds tiny budget")
        self.assertEqual(limited["outcome"], "inconclusive")
        self.assertEqual(limited["code"], "E0400")
        self.assertNotEqual(evidence["verdict"], "passed")

    def test_loaded_quantified_failure_remains_in_the_original_baseline(self):
        record = self.run_fixture("imports-failure")
        self.nonpass(record)
        evidence = self.one(record)["baseline"]
        claim = next(c for c in evidence["claims"] if c["name"] == "loaded quantified failure")
        self.assertEqual(claim["outcome"], "counterexample")
        self.assertIn("i =", claim["counterexample"])
        self.assertEqual(Path(claim["at"]["file"]).name, "loaded-failure.coax")
        self.assertEqual({Path(s["path"]).name for s in evidence["sources"]},
                         {"imports-failure.coax", "loaded-failure.coax"})

    def test_authority_failure_is_preserved_without_an_invented_native_body(self):
        for fixture in ("authority-uncovered", "authority-dead-reference"):
            with self.subTest(fixture=fixture):
                record = self.run_fixture(fixture)
                self.nonpass(record)
                item = self.one(record)
                evidence = item["baseline"]
                self.assertIn("E0380", item["baseline_execution"]["stderr"])
                authority = [c for c in evidence["claims"] if c["kind"] == "#authority"]
                if fixture == "authority-uncovered":
                    # The compiler rejects this before it can run any check.
                    self.assertEqual(evidence["front_end"], "failed")
                    self.assertEqual(evidence["claims"], [])
                elif authority:
                    self.assertTrue(any(c["outcome"] != "passed" for c in authority))
                else:
                    self.assertEqual(evidence["front_end"], "failed")
                self.assertFalse(any(c["kind"] == "#authority" for c in item["native_coverage"]))

    def test_quantified_only_baseline_is_not_native_coverage(self):
        record = self.run_fixture("quantified-only")
        self.nonpass(record)
        item = self.one(record)
        self.assertEqual(item["baseline"]["verdict"], "passed")
        entries = self.coverage(item, "only quantified coverage")
        self.assertEqual(len(entries), 1)
        self.assertEqual(entries[0]["kind"], "#forall")
        self.assertNotEqual(entries[0]["outcome"], "passed")
        self.assertTrue(entries[0]["why"])

    def test_unsupported_directive_and_indented_check_are_explicit_gaps(self):
        for fixture, name in [("unsupported-run", "compile-time directive"),
                              ("indented", "indented passing check")]:
            with self.subTest(fixture=fixture):
                record = self.run_fixture(fixture)
                self.nonpass(record)
                item = self.one(record)
                self.assertEqual(item["baseline"]["verdict"], "passed")
                entries = self.coverage(item, name)
                self.assertEqual(len(entries), 1)
                self.assertNotEqual(entries[0]["outcome"], "passed")
                self.assertTrue(entries[0]["why"])
                if fixture == "indented":
                    self.assertEqual(entries[0]["at"]["line"], 1)
                    self.assertEqual(entries[0]["at"]["col"], 3)

    def test_duplicate_display_names_never_borrow_another_bodys_status(self):
        record = self.run_fixture("duplicate-names")
        self.nonpass(record)
        entries = self.coverage(self.one(record), "same display name")
        self.assertEqual(len(entries), 2)
        self.assertEqual({entry["at"]["line"] for entry in entries}, {1, 2})
        self.assertTrue(all(entry["outcome"] != "passed" and entry["why"] for entry in entries))

    def test_imported_passing_body_is_not_claimed_as_root_native_execution(self):
        record = self.run_fixture("imports-pass")
        self.nonpass(record)
        item = self.one(record)
        self.assertEqual(item["baseline"]["verdict"], "passed")
        entries = self.coverage(item, "loaded passing check")
        self.assertEqual(len(entries), 1)
        self.assertNotEqual(entries[0]["outcome"], "passed")
        self.assertEqual(Path(entries[0]["at"]["file"]).name, "loaded-pass.coax")

    def test_original_main_and_empty_suite_do_not_turn_into_native_passes(self):
        for fixture in ("own-main", "empty"):
            with self.subTest(fixture=fixture):
                self.nonpass(self.run_fixture(fixture))

    def invalid_observation(self, payload, execution=None):
        """Inject transport/evidence defects, never synthetic successful checks."""
        calls = []
        if execution is None:
            execution = copy.deepcopy(self.one(self.known_good())["baseline_execution"])
        def observe(command, *args, **options):
            command = [str(part) for part in command]
            calls.append(command)
            if "--evidence" in command and payload is not None:
                output = Path(command[command.index("--evidence") + 1])
                output.write_text(payload if isinstance(payload, str) else json.dumps(payload))
            return copy.deepcopy(execution)
        with mock.patch.object(baseline.nc, "execute", side_effect=observe):
            record = self.run_fixture("simple-pass")
        self.nonpass(record)
        self.assertEqual(len(calls), 1, "An invalid baseline must stop before native compilation or execution.")
        return record

    def test_missing_and_malformed_evidence_never_qualifies(self):
        good = self.one(self.known_good())["baseline"]
        malformed = [None, "{not-json", [], {"schema": "other"},
                     {k: v for k, v in good.items() if k != "claims"},
                     {**good, "claims": []}]
        for payload in malformed:
            with self.subTest(payload=repr(payload)[:70]):
                self.invalid_observation(payload)

    def test_a_previous_evidence_file_cannot_fill_in_for_a_missing_new_one(self):
        good_item = self.one(self.known_good())
        failed_execution = copy.deepcopy(good_item["baseline_execution"])
        failed_execution["returncode"] = 1
        # Leave a structurally valid old receipt in the same public keep dir.
        self.invalid_observation(good_item["baseline"], failed_execution)
        self.invalid_observation(None, good_item["baseline_execution"])

    def test_wrong_original_source_or_compiler_identity_is_not_accepted(self):
        good = self.one(self.known_good())["baseline"]
        changed = []
        for field in ("sha256", "path"):
            payload = copy.deepcopy(good)
            payload["sources"][0][field] = "0" * 64 if field == "sha256" else str(self.dir / "different.coax")
            changed.append(payload)
        payload = copy.deepcopy(good)
        payload["input"] = str(self.dir / "different-input.coax")
        changed.append(payload)
        payload = copy.deepcopy(good)
        payload["compiler"]["sha256"] = "0" * 64
        changed.append(payload)
        for payload in changed:
            with self.subTest(payload=payload):
                self.invalid_observation(payload)

    def test_partial_selection_or_process_verdict_inconsistency_is_not_complete(self):
        good_item = self.one(self.known_good())
        good = good_item["baseline"]
        for field, value in (("only", "supported"), ("list", True), ("root_only", True)):
            with self.subTest(selection=field):
                payload = copy.deepcopy(good)
                payload["selection"][field] = value
                self.invalid_observation(payload)
        for field, value in (("front_end", "error"), ("exit_status", 1), ("command", "check")):
            with self.subTest(field=field):
                payload = copy.deepcopy(good)
                payload[field] = value
                self.invalid_observation(payload)
        execution = copy.deepcopy(good_item["baseline_execution"])
        execution["returncode"] = 1
        self.invalid_observation(good, execution)

    def test_unavailable_compiler_and_timeout_remain_infrastructure_or_inconclusive(self):
        with mock.patch.dict(os.environ, {"COAX_TEST_COMPILER": str(self.dir / "missing-compiler")}):
            record = self.run_fixture("simple-pass")
        self.nonpass(record)
        self.assertEqual(record["outcome"], "infrastructure-error")
        execution = {"returncode": None, "stdout": "", "stderr": "timeout", "timeout": True}
        record = self.invalid_observation(None, execution)
        self.assertEqual(record["outcome"], "inconclusive")

    def test_missing_source_is_an_infrastructure_failure_without_a_fake_baseline(self):
        record = baseline.run([self.dir / "absent.coax"], keep=self.dir / "native")
        self.nonpass(record)
        self.assertEqual(record["outcome"], "infrastructure-error")
        self.assertIsNone(self.one(record)["baseline"])

    def test_cli_source_output_and_keep_aliases_refuse_before_execution(self):
        source = self.dir / "original.coax"
        original = (FIXTURES / "simple-pass.coax").read_bytes()
        source.write_bytes(original)
        alias = self.dir / "output-link"
        alias.symlink_to(source)
        for option, target in (("--out", source), ("--out", alias), ("--keep", source)):
            with self.subTest(option=option, target=target):
                with mock.patch.object(baseline.nc, "execute") as execute:
                    with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                        status = baseline.main([str(source), option, str(target)])
                self.assertNotEqual(status, 0)
                execute.assert_not_called()
                self.assertEqual(source.read_bytes(), original)

    def test_cli_explicit_budget_cannot_raise_or_disable_the_normal_limit(self):
        source = FIXTURES / "simple-pass.coax"
        for value in ("0", "-1", "20000001"):
            with self.subTest(steps=value):
                with mock.patch.object(baseline.nc, "execute") as execute:
                    with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                        status = baseline.main([str(source), "--steps", value])
                self.assertNotEqual(status, 0)
                execute.assert_not_called()

    def test_successful_authority_aggregate_stays_evaluator_only(self):
        item = self.one(self.known_authority())
        evidence = item["baseline"]
        self.assertEqual(evidence["verdict"], "passed")
        authority = [c for c in evidence["claims"] if c["kind"] == "#authority"]
        self.assertTrue(authority, evidence)
        for claim in authority:
            self.assertEqual(claim["outcome"], "passed")
            self.assertEqual(claim["cases"], 0)
            self.assertEqual(claim["steps"], 0)
            self.assertIsNone(claim["at"])
        self.assertFalse(any(c["kind"] == "#authority" for c in item["native_coverage"]))
        authored = self.coverage(item, "executed authority procedure")
        self.assertEqual(len(authored), 1)
        self.assertEqual(authored[0]["outcome"], "passed")

    def test_authored_claim_counts_are_required_nonboolean_and_nonnegative(self):
        good = self.known_authority()
        item = self.one(good)
        def valid_observation(command, *args, **options):
            command = [str(part) for part in command]
            output = Path(command[command.index("--evidence") + 1])
            output.write_text(json.dumps(item["baseline"]))
            return copy.deepcopy(item["baseline_execution"])
        # A positive transport control preserves the real zero-count authority
        # aggregate under the new validator, without rerunning native checks.
        with mock.patch.object(baseline.nc, "execute", side_effect=valid_observation):
            with mock.patch.object(baseline.nc, "run",
                                   return_value=copy.deepcopy(good["differential"])) as native:
                control = self.run_fixture("successful-authority")
        self.assertEqual(control["outcome"], "passed", control)
        native.assert_called_once()
        missing = object()
        defects = [(field, value) for field in ("cases", "steps")
                   for value in (missing, True, False, -1)]
        defects.append(("cases", 0))
        for kind in ("#test", "#forall"):
            for field, value in defects:
                with self.subTest(kind=kind, field=field, value=str(value)):
                    payload = copy.deepcopy(item["baseline"])
                    authored = next(c for c in payload["claims"] if c["kind"] == "#test")
                    authored["kind"] = kind
                    if value is missing:
                        del authored[field]
                    else:
                        authored[field] = value
                    def observe(command, *args, **options):
                        command = [str(part) for part in command]
                        output = Path(command[command.index("--evidence") + 1])
                        output.write_text(json.dumps(payload))
                        return copy.deepcopy(item["baseline_execution"])
                    # Reuse truthful native observations for this unchanged body.
                    # The invalid envelope must be rejected before using them.
                    with mock.patch.object(baseline.nc, "execute", side_effect=observe):
                        with mock.patch.object(baseline.nc, "run",
                                               return_value=copy.deepcopy(good["differential"])) as native:
                            record = self.run_fixture("successful-authority")
                    red = os.environ.get("COAX_BASELINE_COUNT_RED_RECORD")
                    if red and record["outcome"] == "passed" and not Path(red).exists():
                        Path(red).write_text(json.dumps(record, indent=2) + "\n")
                    self.nonpass(record)
                    self.assertEqual(record["outcome"], "infrastructure-error")
                    native.assert_not_called()


if __name__ == "__main__":
    unittest.main(verbosity=2)
