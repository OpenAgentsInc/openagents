"""Synthetic evidence checks for the independent report; no inference or builds."""
import copy
import hashlib
import io
import tarfile
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import report

RATES = {"input": 1, "output": 2, "cache_write_5m": 2, "cache_write_1h": 4, "cache_read": .1}
PRICES = {"claude-opus-5-5": dict(zip(report.RATE_KEYS, [4, 20, 5, 8, .2])),
          "claude-sonnet-5-5": dict(zip(report.RATE_KEYS, [2, 10, 2.5, 4, .2]))}


class Fixture:
    def __init__(self, root):
        self.root = root
        rule = {"cost_ratio_max": .8, "time_ratio_max": .9,
                "cost_task_mean_wins_min": 3, "time_task_mean_wins_min": 3}
        self.protocol = {
            "registration": {"sealed": True}, "scored_execution_allowed": True,
            "design": {"tasks": 4, "core_arms": 6, "repetitions": 2},
            "arms": [{"id": a, "model_family": "Opus 5.5" if a in "ABC" else "Sonnet 5.5",
                      "requested_effort": "medium", "source_pack": "none" if a in "AD" else "ast",
                      "system_one": "jev_rerank" if a in "CF" else "none"} for a in "ABCDEF"],
            "executor_prices": copy.deepcopy(PRICES),
            "system_one": {"requested_model": "jev-1.13.0", "price": {"usd_per_million_input_tokens": .042}},
            "analysis": {"bootstrap": {"draws": 10000, "seed": 20261003}},
            "comparisons": {"F/E": {**rule, "cost_ratio_max": .9, "time_ratio_max": .95},
                            "F/A": rule, "C/B": {"same_rule_as": "F/E"}, "B/A": {"same_rule_as": "F/A"},
                            "E/D": {"same_rule_as": "F/A"}, "E/B": {"descriptive": True}, "F/C": {"descriptive": True}},
            "combined_gate_requires": ["F/E", "F/A"],
        }
        self.manifest = {"schema": report.MANIFEST_SCHEMA,
                         "protocol": self.write("protocol.json", self.protocol),
                         "tasks": [{"task_id": f"t{i}", "source_commit": str(i) * 40} for i in range(1, 5)],
                         "arms": {a: {"primary_model": "claude-opus-5-5" if a in "ABC" else "claude-sonnet-5-5",
                                      "allowed_models": ["claude-opus-5-5" if a in "ABC" else "claude-sonnet-5-5"], "effort": "medium",
                                      "argv_tail": ["-p", "--model", "claude-opus-5-5" if a in "ABC" else "claude-sonnet-5-5", "--effort", "medium"] + ([] if a in "AD" else ["--tools", "Bash,Read,Edit,Write,Glob,Grep"]),
                                      "system_prompt_sha256": None if a in "AD" else report.sha(b"lean system"),
                                      "preparation": a not in "AD", "system_one": a in "CF"} for a in "ABCDEF"},
                         "prices": copy.deepcopy(PRICES), "effective_schedule": [], "runs": []}
        counts = dict(zip("ABCDEF", [10000, 8000, 6000, 7000, 5000, 3000]))
        for task in self.manifest["tasks"]:
            task["source_archive_sha256"] = "a" * 64
            task["base_prompt"] = self.write(task["task_id"] + "/base.txt", b"public task and common instructions", raw=True)
            pool = self.write(task["task_id"] + "/pool.json", [{"id": task["task_id"] + "-unit"}])
            task["preparation"] = {"source_commit": task["source_commit"], "policy": "syntax-units-v2",
                                   "issue_sha256": "d" * 64, "index_sha256": "e" * 64, "script_sha256": "f" * 64,
                                   "candidate_sha256": pool["sha256"], "candidate_units": 1}
            for rep in (1, 2):
                for arm in "ABCDEF":
                    identity = f"{task['task_id']}-{arm}-{rep}"
                    self.manifest["effective_schedule"].append({"run_id": identity, "task_id": task["task_id"],
                                                              "arm": arm, "repetition": rep, "block": f"{task['task_id']}-{rep}"})
                    self.manifest["runs"].append(self.run(identity, task, arm, counts[arm]))
        self.seal()

    def seal(self):
        self.protocol["registration"]["report_bindings"] = copy.deepcopy({
            "arms": self.manifest["arms"], "prices": self.manifest["prices"], "tasks": self.manifest["tasks"],
            "schedule": self.manifest["effective_schedule"], "cli": {"sha256": "b" * 64, "version": "pinned"}})
        self.manifest["protocol"] = self.write("protocol.json", self.protocol)

    def write(self, name, value, raw=False):
        data = value if raw else (json.dumps(value, sort_keys=True) + "\n").encode()
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        return {"path": name, "sha256": hashlib.sha256(data).hexdigest()}

    def run(self, identity, task, arm, count):
        prefix = identity + "/"
        config = self.manifest["arms"][arm]
        model = config["primary_model"]
        pack = b"semantic source pack" if arm in "CF" else b"deterministic source pack"
        base = (self.root / task["base_prompt"]["path"]).read_bytes()
        prompt = base if arm in "AD" else base + b"\n\n" + pack
        payload = b"candidate " + identity.encode()
        stream = io.BytesIO()
        with tarfile.open(fileobj=stream, mode="w:gz") as tar:
            member = tarfile.TarInfo("src/lib.rs")
            member.mode, member.size = 0o644, len(payload)
            tar.addfile(member, io.BytesIO(payload))
        candidate_payload = self.write(prefix + "candidate.tar.gz", stream.getvalue(), raw=True)
        changes = {"src/lib.rs": {"before": None, "after": {"kind": "file", "mode": 0o644,
                     "bytes": len(payload), "sha256": report.sha(payload)}}}
        candidate_changes = self.write(prefix + "changes.json", changes)
        candidate = self.write(prefix + "candidate-manifest.json", {
            "schema": "openagents.delegation.candidate.v1", "source_commit": task["source_commit"],
            "source_archive_sha256": task["source_archive_sha256"], "changes": changes,
            "payload_sha256": candidate_payload["sha256"]})
        expected = {"source_commit": task["source_commit"], "source_archive_sha256": "a" * 64,
                    "cli_sha256": "b" * 64, "cli_hash_after": "b" * 64, "cli_version": "pinned",
                    "model": model, "effort": "medium", "prompt_sha256": report.sha(prompt)}
        native = {**expected, "run_id": identity, "model_completed": True, "exit_code": 0,
                  "argv": ["/opt/claude"] + config["argv_tail"],
                  "delivered_prompt_sha256": report.sha(("Benchmark run ID: " + identity + "\n\n").encode() + prompt),
                  "cost_usd": count / 1e6, "served_models": [model], "candidate_manifest_sha256": candidate["sha256"]}
        usage = {"input_tokens": int(count / PRICES[model]["input"]), "output_tokens": 0, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 0}
        provider = [
            {"run_id": identity, "phase": "admitted", "call_id": "call-" + identity, "model": model, "path": "/v1/messages"},
            {"run_id": identity, "phase": "finished", "call_id": "call-" + identity, "status": "complete", "http_status": 200,
             "served_model": model, "usage_status": "reported", "usage": usage, "cost_usd": count / 1e6},
        ]
        checks = {"schema": "openagents.delegation.final-checks.v1", "run_id": identity,
                  "candidate_manifest_sha256": candidate["sha256"], "completed": True, "execution_closed": True,
                  **{k: {"passed": True} for k in ("scope", "format", "ordinary", "independent")}}
        endpoint = {"schema": "openagents.delegation.endpoint.v1", "run_id": identity,
                    "candidate_manifest_sha256": candidate["sha256"], "clock_id": "one-clock", "start_monotonic_ns": 10,
                    "end_monotonic_ns": 10 + count * 10_000_000, "execution_closed": True}
        review = {"schema": "openagents.delegation.review.v1", "run_id": identity,
                  "candidate_manifest_sha256": candidate["sha256"], "completed": True,
                  "labels_hidden_at_judgment": True, "material_defect": False}
        refs = {name: self.write(prefix + name + ".json", value) for name, value in
                [("native", native), ("checks", checks), ("endpoint", endpoint), ("review", review)]}
        refs["candidate_manifest"] = candidate
        refs["candidate_payload"] = candidate_payload
        refs["candidate_changes"] = candidate_changes
        refs["prompt"] = self.write(prefix + "prompt.txt", prompt, raw=True)
        refs["provider_calls"] = self.write(prefix + "provider.jsonl", b"".join((json.dumps(v) + "\n").encode() for v in provider), raw=True)
        if arm not in "AD":
            prep = {**task["preparation"], "mode": "jev" if arm in "CF" else "deterministic",
                    "coverage": {"candidate_units": 1}, "briefing_sha256": report.sha(pack), "briefing_bytes": len(pack),
                    "system_one": None}
            if arm in "CF":
                prep["system_one"] = {"attempts": 1, "requested_model": "jev-1.13.0", "served_model": "jev-1.13.0",
                                      "usage": {"input_tokens": 100}, "cost_usd": .0000042, "selection": "system_one", "outcome": "answered"}
            refs["preparation"] = self.write(prefix + "preparation.json", prep)
            refs["candidate_pool"] = {"path": task["task_id"] + "/pool.json", "sha256": task["preparation"]["candidate_sha256"]}
            refs["briefing"] = self.write(prefix + "briefing.md", pack, raw=True)
            refs["system_prompt"] = self.write(prefix + "system.txt", b"lean system", raw=True)
        return {"run_id": identity, "arm": arm, "category": "scored", "bindings": {"native": expected, "preparation": {}}, "artifacts": refs}

    def change(self, identity, kind, update):
        entry = next(r for r in self.manifest["runs"] if r["run_id"] == identity)
        ref = entry["artifacts"][kind]
        value = json.loads((self.root / ref["path"]).read_text())
        update(value)
        entry["artifacts"][kind] = self.write(ref["path"], value)

    def build(self, draws=100):
        path = self.root / "study.json"
        path.write_text(json.dumps(self.manifest))
        return report.build(path, draws=draws)


class ReportTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.fixture = Fixture(Path(self.temp.name))

    def test_complete_panel_recomputes_cost_once_and_all_comparisons(self):
        result = self.fixture.build()
        self.assertEqual(result["combined_gate"], "pass")
        self.assertEqual(len(result["comparisons"]), 7)
        self.assertEqual(result["unique_accounting_runs"], 48)
        self.assertAlmostEqual(result["program_totals"]["cost_usd"], .312 + 16 * .0000042)
        self.assertEqual(result["arms"]["F"]["accepted"], 8)
        self.assertAlmostEqual(result["comparisons"]["F/A"]["time_ratio"], .3)

    def test_native_budget_exit_does_not_override_bound_acceptance(self):
        self.fixture.change("t1-F-1", "native", lambda r: r.update(model_completed=False, exit_code=1, timed_out=True))
        result = self.fixture.build()
        self.assertEqual(result["arms"]["F"]["accepted"], 8)
        self.assertEqual(result["combined_gate"], "pass")

    def test_failure_stays_in_cost_denominator_and_fails_quality_gate(self):
        self.fixture.change("t1-F-1", "checks", lambda r: r["independent"].update(passed=False))
        result = self.fixture.build()
        self.assertEqual(result["combined_gate"], "fail")
        self.assertEqual(result["arms"]["F"]["failed"], 1)
        self.assertAlmostEqual(result["arms"]["F"]["cost_per_accepted_lower_usd"], (8 * .0030042) / 7)

    def test_missing_row_never_becomes_zero_known_spend(self):
        self.fixture.manifest["runs"] = self.fixture.manifest["runs"][:-1]
        result = self.fixture.build()
        self.assertEqual(result["effective_assigned_runs"], 48)
        self.assertEqual(len(result["effective_rows"]), 48)
        self.assertIsNone(result["program_totals"]["cost_upper_usd"])
        self.assertEqual(result["combined_gate"], "not_evaluable")
        self.assertFalse(result["labels_released"])
        self.assertEqual(result["arms"], {})

    def test_wrong_candidate_digest_blocks_acceptance(self):
        self.fixture.change("t1-F-1", "checks", lambda r: r.update(candidate_manifest_sha256="0" * 64))
        result = self.fixture.build()
        self.assertEqual(result["combined_gate"], "not_evaluable")

    def test_worker_checks_without_final_execution_closure_cannot_pass(self):
        self.fixture.change('t1-F-1','checks',lambda r:r.pop('execution_closed'))
        result=self.fixture.build()
        row=next(r for r in result['effective_rows'] if r['run_id']=='t1-F-1')
        self.assertIsNone(row['accepted'])
        self.assertEqual(result['combined_gate'],'not_evaluable')

    def test_candidate_manifest_binds_deletions_and_change_sidecar(self):
        entry = self.fixture.manifest["runs"][-1]
        self.fixture.change(entry["run_id"], "candidate_changes", lambda r: r.update({"deleted.rs": {"before": {"kind": "file"}, "after": None}}))
        self.assertEqual(self.fixture.build()["combined_gate"], "not_evaluable")

    def test_candidate_payload_contents_are_validated_after_digest_rebinding(self):
        entry = self.fixture.manifest["runs"][-1]
        identity = entry["run_id"]
        ref = entry["artifacts"]["candidate_payload"]
        # An internally inconsistent but freshly hashed envelope must still fail.
        entry["artifacts"]["candidate_payload"] = self.fixture.write(ref["path"], b"not a gzip archive", raw=True)
        self.fixture.change(identity, "candidate_manifest", lambda r: r.update(payload_sha256=entry["artifacts"]["candidate_payload"]["sha256"]))
        changed = entry["artifacts"]["candidate_manifest"]["sha256"]
        for kind in ("native", "checks", "endpoint", "review"):
            self.fixture.change(identity, kind, lambda r: r.update(candidate_manifest_sha256=changed))
        self.assertEqual(self.fixture.build()["combined_gate"], "not_evaluable")

    def test_candidate_manifest_must_name_frozen_source(self):
        self.fixture.change("t1-F-1", "candidate_manifest", lambda r: r.update(source_commit="0" * 40))
        self.assertEqual(self.fixture.build()["combined_gate"], "not_evaluable")

    def test_unblinded_review_is_not_a_valid_quality_receipt(self):
        self.fixture.change("t1-F-1", "review", lambda r: r.update(labels_hidden_at_judgment=False))
        result = self.fixture.build()
        row = next(r for r in result["effective_rows"] if r["run_id"] == "t1-F-1")
        self.assertTrue(row["accepted"])
        self.assertIsNone(row["review_passed"])
        self.assertIsNone(row["cell"])
        self.assertEqual(row["provider"]["served_models"], [])
        self.assertEqual(result["combined_gate"], "not_evaluable")

    def test_final_source_review_defect_blocks_recommendation(self):
        self.fixture.change("t1-F-1", "review", lambda r: r.update(material_defect=True))
        self.assertEqual(self.fixture.build()["combined_gate"], "fail")

    def test_unknown_provider_completion_does_not_use_native_cost(self):
        entry = self.fixture.manifest["runs"][-1]
        ref = entry["artifacts"]["provider_calls"]
        first = (self.fixture.root / ref["path"]).read_bytes().splitlines(keepends=True)[0]
        entry["artifacts"]["provider_calls"] = self.fixture.write(ref["path"], first, raw=True)
        result = self.fixture.build()
        self.assertIsNone(result["program_totals"]["cost_upper_usd"])
        self.assertEqual(result["combined_gate"], "not_evaluable")

    def test_duplicate_run_identity_cannot_double_charge(self):
        self.fixture.manifest["runs"].append(copy.deepcopy(self.fixture.manifest["runs"][0]))
        result = self.fixture.build()
        self.assertEqual(result["unique_accounting_runs"], 48)
        self.assertIsNone(result["program_totals"]["cost_usd"])
        self.assertEqual(result["combined_gate"], "not_evaluable")

    def test_changed_pool_is_not_an_isolated_ranking_comparison(self):
        self.fixture.change("t1-F-1", "preparation", lambda r: r.update(candidate_sha256="f" * 64))
        self.assertEqual(self.fixture.build()["combined_gate"], "not_evaluable")

    def test_effort_and_source_bindings_are_checked(self):
        self.fixture.change("t1-F-1", "native", lambda r: r.update(effort="high"))
        self.assertEqual(self.fixture.build()["combined_gate"], "not_evaluable")

    def test_unsealed_protocol_keeps_gate_unevaluable(self):
        self.fixture.protocol["registration"]["sealed"] = False
        self.fixture.manifest["protocol"] = self.fixture.write("protocol.json", self.fixture.protocol)
        self.assertEqual(self.fixture.build()["combined_gate"], "not_evaluable")

    def test_artifact_digest_and_escape_are_rejected(self):
        entry = self.fixture.manifest["runs"][-1]
        entry["artifacts"]["checks"]["sha256"] = "0" * 64
        self.assertEqual(self.fixture.build()["combined_gate"], "not_evaluable")
        with self.assertRaises(ValueError):
            report.artifact(self.fixture.root, {"path": "../secret", "sha256": "0" * 64})

    def test_source_archive_has_a_separate_streamed_two_gib_bound(self):
        ref = self.fixture.write("size-role.bin", b"small test payload", raw=True)
        target = (self.fixture.root / ref["path"]).resolve()
        original_stat = Path.stat
        def fake_size(size):
            def stat(path, *args, **kwargs):
                result = original_stat(path, *args, **kwargs)
                if path == target:
                    fields = list(result)
                    fields[6] = size
                    return os.stat_result(fields)
                return result
            return stat
        # Only the stat size is large; the test reads a few real bytes.
        with mock.patch.object(Path, "stat", fake_size(2 * 1024**3)):
            self.assertEqual(report.artifact(self.fixture.root, ref, "source_archive"), ref["sha256"])
            with self.assertRaisesRegex(ValueError, "reader bound"):
                report.artifact(self.fixture.root, ref, "binary")
        for kind, limit in report.ARTIFACT_LIMITS.items():
            with self.subTest(kind=kind), mock.patch.object(Path, "stat", fake_size(limit + 1)):
                with self.assertRaisesRegex(ValueError, "reader bound"):
                    report.artifact(self.fixture.root, ref, kind)
        with self.assertRaisesRegex(ValueError, "unknown artifact reader role"):
            report.artifact(self.fixture.root, ref, "unbounded")

    def test_streamed_reader_stops_growth_past_role_bound(self):
        ref = self.fixture.write("growing.bin", b"12345", raw=True)
        original_stat = Path.stat
        target = (self.fixture.root / ref["path"]).resolve()
        def small_stat(path, *args, **kwargs):
            result = original_stat(path, *args, **kwargs)
            if path == target:
                fields = list(result)
                fields[6] = 1
                return os.stat_result(fields)
            return result
        with mock.patch.dict(report.ARTIFACT_LIMITS, {"source_archive": 4}), mock.patch.object(Path, "stat", small_stat):
            with self.assertRaisesRegex(ValueError, "reader bound"):
                report.artifact(self.fixture.root, ref, "source_archive")

    def test_unknown_cache_lifetime_has_bounds_not_point_cost(self):
        usage = {"input_tokens": 100, "output_tokens": 10, "cache_creation_input_tokens": 1000}
        low, high, unknown = report.price_bounds(usage, RATES)
        self.assertEqual(unknown, 1000)
        self.assertAlmostEqual(low, .00212)
        self.assertAlmostEqual(high, .00412)
        with self.assertRaises(ValueError):
            report.price_bounds({**usage, "input_tokens": True}, RATES)

    def set_arm_cache(self, arm, tokens):
        for entry in self.fixture.manifest["runs"]:
            if entry["arm"] != arm:
                continue
            ref = entry["artifacts"]["provider_calls"]
            values = [json.loads(line) for line in (self.fixture.root / ref["path"]).read_text().splitlines()]
            values[1]["usage"] = {"input_tokens": 0, "output_tokens": 0, "cache_creation_input_tokens": tokens}
            values[1]["cost_usd"] = tokens * 4 / 1e6
            entry["artifacts"]["provider_calls"] = self.fixture.write(
                ref["path"], b"".join((json.dumps(v) + "\n").encode() for v in values), raw=True)

    def test_uncertain_cache_gate_uses_conservative_bounds(self):
        self.set_arm_cache("F", 1000)
        result = self.fixture.build()
        self.assertIsNone(result["arms"]["F"]["cost_usd"])
        self.assertEqual(result["combined_gate"], "pass")
        self.assertLess(result["comparisons"]["F/E"]["cost_ratio_upper"], .9)

    def test_uncertain_cache_cannot_win_from_midpoint_or_upper_upper_ratio(self):
        self.set_arm_cache("F", 1500)
        result = self.fixture.build()
        value = result["comparisons"]["F/E"]
        self.assertLess(value["cost_ratio_lower"], .9)
        self.assertGreater(value["cost_ratio_upper"], .9)
        self.assertIsNone(value["cost_ratio"])
        self.assertEqual(result["combined_gate"], "not_evaluable")

    def test_extra_paid_run_is_charged_without_becoming_an_observation(self):
        before = self.fixture.build()
        entry = self.fixture.run("probe-unique", self.fixture.manifest["tasks"][0], "A", 2000)
        entry["category"] = "capability_probe"
        self.fixture.manifest["runs"].append(entry)
        after = self.fixture.build()
        self.assertEqual(after["unique_accounting_runs"], 49)
        self.assertEqual(after["effective_totals"], before["effective_totals"])
        self.assertAlmostEqual(after["program_totals"]["cost_usd"] - before["program_totals"]["cost_usd"], .002)

    def test_replacement_keeps_original_cost_and_uses_only_replacement_cell(self):
        before = self.fixture.build()
        entry = self.fixture.run("replacement-unique", self.fixture.manifest["tasks"][0], "A", 20000)
        entry["category"] = "replacement"
        self.fixture.manifest["runs"][0]["category"] = "invalidated"
        self.fixture.manifest["runs"].append(entry)
        self.fixture.manifest["effective_schedule"][0]["run_id"] = entry["run_id"]
        after = self.fixture.build()
        self.assertAlmostEqual(after["program_totals"]["cost_usd"] - before["program_totals"]["cost_usd"], .020)
        self.assertAlmostEqual(after["effective_totals"]["cost_usd"] - before["effective_totals"]["cost_usd"], .010)
        self.assertEqual(after["combined_gate"], "not_evaluable")
        self.assertTrue(any("replacement" in error for error in after["errors"]))

    def test_auxiliary_model_requires_explicit_permission_and_is_charged(self):
        self.fixture.manifest["prices"]["aux"] = PRICES["claude-sonnet-5-5"]
        self.fixture.manifest["arms"]["F"]["allowed_models"].append("aux")
        self.fixture.seal()
        entry = self.fixture.manifest["runs"][-1]
        ref = entry["artifacts"]["provider_calls"]
        values = [json.loads(line) for line in (self.fixture.root / ref["path"]).read_text().splitlines()]
        extra = copy.deepcopy(values)
        for item in extra:
            item["call_id"] += "-aux"
            if item["phase"] == "admitted":
                item["model"] = "aux"
            else:
                item["served_model"] = "aux"
        entry["artifacts"]["provider_calls"] = self.fixture.write(
            ref["path"], b"".join((json.dumps(v) + "\n").encode() for v in values + extra), raw=True)
        result = self.fixture.build()
        row = next(r for r in result["effective_rows"] if r["run_id"] == entry["run_id"])
        self.assertAlmostEqual(row["cost_usd"], .0060042)
        self.assertEqual(row["provider"]["served_models"], ["aux", "claude-sonnet-5-5"])
        self.fixture.manifest["arms"]["F"]["allowed_models"].remove("aux")
        self.assertEqual(self.fixture.build()["combined_gate"], "not_evaluable")

    def test_unfinished_probe_blocks_complete_program_claim(self):
        entry = self.fixture.run("probe-unknown", self.fixture.manifest["tasks"][0], "A", 2000)
        entry["category"] = "capability_probe"
        ref = entry["artifacts"]["provider_calls"]
        first = (self.fixture.root / ref["path"]).read_bytes().splitlines(keepends=True)[0]
        entry["artifacts"]["provider_calls"] = self.fixture.write(ref["path"], first, raw=True)
        self.fixture.manifest["runs"].append(entry)
        result = self.fixture.build()
        self.assertIsNone(result["program_totals"]["cost_upper_usd"])
        self.assertEqual(result["comparisons"]["F/A"]["gate"], "pass")
        self.assertEqual(result["combined_gate"], "not_evaluable")

    def test_wrong_primary_family_cannot_be_registered_after_the_fact(self):
        for arm in self.fixture.manifest["arms"].values():
            arm["primary_model"] = "model"
            arm["allowed_models"] = ["model"]
        self.fixture.seal()
        result = self.fixture.build()
        self.assertEqual(result["combined_gate"], "not_evaluable")
        self.assertIn("arm configuration differs from protocol", result["errors"])

    def test_prices_must_match_protocol_and_sealed_schedule(self):
        self.fixture.manifest["prices"]["claude-sonnet-5-5"]["input"] = 1
        self.fixture.seal()
        self.assertEqual(self.fixture.build()["combined_gate"], "not_evaluable")

    def test_native_charge_missing_from_broker_makes_accounting_unknown(self):
        self.fixture.change("t1-F-1", "native", lambda r: r.update(cost_usd=123))
        result = self.fixture.build()
        self.assertIsNone(result["program_totals"]["cost_upper_usd"])
        self.assertEqual(result["combined_gate"], "not_evaluable")
        row = next(r for r in result["effective_rows"] if r["run_id"] == "t1-F-1")
        self.assertIn("native cumulative cost exceeds broker upper bound", row["errors"])

    def test_sealed_bindings_are_required_and_run_cannot_redefine_cli(self):
        entry = self.fixture.manifest["runs"][-1]
        self.fixture.change(entry["run_id"], "native", lambda r: r.update(cli_sha256="1" * 64, cli_hash_after="1" * 64))
        entry["bindings"]["native"].update(cli_sha256="1" * 64, cli_hash_after="1" * 64)
        self.assertEqual(self.fixture.build()["combined_gate"], "not_evaluable")
        del self.fixture.protocol["registration"]["report_bindings"]
        self.fixture.manifest["protocol"] = self.fixture.write("protocol.json", self.fixture.protocol)
        self.assertIn("sealed reporting bindings missing", self.fixture.build()["errors"])

    def test_nonempty_pool_requires_digest_and_registered_input_policy(self):
        for field in report.PREPARATION_BINDINGS:
            with self.subTest(field=field):
                fixture = Fixture(Path(self.temp.name))
                fixture.change("t1-F-1", "preparation", lambda r: r.pop(field))
                self.assertEqual(fixture.build()["combined_gate"], "not_evaluable")

    def test_tools_and_base_prompt_are_bound(self):
        self.fixture.change("t1-F-1", "native", lambda r: r["argv"].extend(["--tools", "all"]))
        self.assertEqual(self.fixture.build()["combined_gate"], "not_evaluable")
        ref = self.fixture.manifest["tasks"][0]["base_prompt"]
        (self.fixture.root / ref["path"]).write_bytes(b"changed instructions")
        self.assertEqual(self.fixture.build()["combined_gate"], "not_evaluable")

    def test_dynamic_pack_must_match_actual_prompt_and_digest(self):
        entry = self.fixture.manifest["runs"][-1]
        ref = entry["artifacts"]["briefing"]
        entry["artifacts"]["briefing"] = self.fixture.write(ref["path"], b"other bytes", raw=True)
        self.fixture.change(entry["run_id"], "preparation", lambda r: r.update(briefing_sha256=report.sha(b"other bytes"), briefing_bytes=11))
        self.assertEqual(self.fixture.build()["combined_gate"], "not_evaluable")

    def test_schedule_order_cannot_change_after_seal(self):
        schedule = self.fixture.manifest["effective_schedule"]
        schedule[0], schedule[1] = schedule[1], schedule[0]
        self.assertIn("effective schedule order differs from registration", self.fixture.build()["errors"])

    def test_all_fallback_keeps_intention_to_treat_but_no_mechanism_claim(self):
        for entry in self.fixture.manifest["runs"]:
            if entry["arm"] == "F":
                self.fixture.change(entry["run_id"], "preparation", lambda r: r["system_one"].update(selection="deterministic_fallback"))
        result = self.fixture.build()
        self.assertEqual(result["arms"]["F"]["accepted"], 8)
        self.assertEqual(result["arms"]["F"]["system_one_delivered"], 0)
        self.assertEqual(result["arms"]["F"]["system_one_fallback"], 8)
        self.assertEqual(result["comparisons"]["F/E"]["gate"], "not_evaluable")
        self.assertLess(result["comparisons"]["F/E"]["cost_ratio"], .9)

    def test_valid_semantic_answer_without_changed_pack_has_no_mechanism_claim(self):
        for entry in self.fixture.manifest["runs"]:
            if entry["arm"] != "F":
                continue
            pack = b"deterministic source pack"
            prompt = b"public task and common instructions\n\n" + pack
            entry["artifacts"]["briefing"] = self.fixture.write(entry["run_id"] + "/briefing.md", pack, raw=True)
            entry["artifacts"]["prompt"] = self.fixture.write(entry["run_id"] + "/prompt.txt", prompt, raw=True)
            self.fixture.change(entry["run_id"], "preparation", lambda r: r.update(briefing_sha256=report.sha(pack), briefing_bytes=len(pack)))
            self.fixture.change(entry["run_id"], "native", lambda r: r.update(prompt_sha256=report.sha(prompt), delivered_prompt_sha256=report.sha(("Benchmark run ID: " + r["run_id"] + "\n\n").encode() + prompt)))
            entry["bindings"]["native"]["prompt_sha256"] = report.sha(prompt)
        result = self.fixture.build()
        self.assertEqual(result["arms"]["F"]["system_one_delivered"], 8)
        self.assertEqual(result["comparisons"]["F/E"]["treatment_delivery"]["changed_packs"], 0)
        self.assertEqual(result["combined_gate"], "not_evaluable")

    def test_bootstrap_pairs_repetitions_and_is_reproducible(self):
        # Every treatment is exactly half its paired control despite large
        # variation within and between tasks. Independent arm sampling breaks it.
        pairs = []
        for amount in (1, 10, 100, 1000):
            group = []
            for scale in (1, 100):
                a = {"cost_lower_usd": amount * scale, "cost_upper_usd": amount * scale, "wall_s": amount * scale}
                b = {k: v * 2 for k, v in a.items()}
                group.append((a, b))
            pairs.append(group)
        result = report.bootstrap(pairs, 10000, 20261003)
        self.assertEqual(result, report.bootstrap(pairs, 10000, 20261003))
        for interval in result["intervals"].values():
            self.assertEqual(interval["p025"], .5)
            self.assertEqual(interval["p975"], .5)
            self.assertEqual(interval["defined_draws"], 10000)


if __name__ == "__main__":
    unittest.main()
