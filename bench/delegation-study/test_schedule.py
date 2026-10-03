"""Offline schedule, provenance, and admission checks; no models or executors."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import report
import schedule
from test_report import Fixture

STUDY = "61a4028f-7b59-45bd-8513-25d30526d584"


class ScheduleTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.fixture = Fixture(self.root)

    def registration(self):
        fixture = self.fixture
        global_refs = {role: fixture.write("registered/" + role, role.encode(), raw=True) for role in schedule.GLOBAL_ROLES}
        modules = {name:fixture.write('harness/'+name,name.encode(),raw=True) for name in sorted(schedule.HARNESS_MODULES)}
        for role,name in schedule.HARNESS_ROLES.items():
            global_refs[role] = modules[name]
        global_refs['harness_manifest'] = fixture.write('registered/harness.json',{'schema':'openagents.delegation.harness-files.v1','files':modules})
        task_refs = {}
        for task in fixture.manifest["tasks"]:
            refs = {role: fixture.write("registered/" + task["task_id"] + "/" + role, role.encode(), raw=True)
                    for role in schedule.TASK_ROLES}
            refs["base_prompt"] = task["base_prompt"]
            task["source_archive_sha256"] = refs["source_archive"]["sha256"]
            for field, role in (("issue_sha256", "issue"), ("index_sha256", "index")):
                task["preparation"][field] = refs[role]["sha256"]
            task["preparation"]["script_sha256"] = global_refs["preparer"]["sha256"]
            task_refs[task["task_id"]] = refs
        rows = schedule.make_schedule(STUDY, [t["task_id"] for t in fixture.manifest["tasks"]], 20261003)
        fixture.manifest["effective_schedule"] = rows
        fixture.protocol["source_pool"] = {"policy": "syntax-units-v2"}
        fixture.protocol["design"]["sessions"] = 48
        fixture.seal()
        fixture.protocol["registration"]["report_bindings"]["cli"]["sha256"] = global_refs["native_cli"]["sha256"]
        protocol_ref = fixture.write("protocol.json", fixture.protocol)
        hashes = {role: ref["sha256"] for role, ref in global_refs.items()}
        preflights = {"jev": fixture.write("preflight/jev.json", {
            "transport": "live", "outcome": "answered", "selection": "system_one", "attempts": 1,
            "requested_model": "jev-1.13.0", "served_model": "jev-1.13.0", "usage": {"input_tokens": 100},
            "cost_usd": .0000042, "preparer_sha256": hashes["preparer"]})}
        for name, model in (("isolation", None), ("native_opus", "claude-opus-5-5"), ("native_sonnet", "claude-sonnet-5-5")):
            preflights[name] = fixture.write("preflight/" + name + ".json", {
                "passed": True, "transport": "live", "model": model, "infrastructure_sha256": hashes})
        return {"schema": schedule.SCHEMA, "sealed": True, "protocol": protocol_ref,
                "study_id": STUDY, "seed": 20261003, "schedule": rows, "artifacts": global_refs,
                "task_artifacts": task_refs, "preflights": preflights,
                "integration": {"paid_dispatch": True, "final_acceptance": True},
                "accounting": [{"run_id": "prior-probe", "cost_upper_usd": .5}],
                "next_block_preparation_reserve_usd": 1}

    def validate(self, registration):
        path = self.root / "registration.json"
        path.write_text(json.dumps(registration))
        return schedule.validate(path)

    def test_schedule_has_eight_complete_blocks_and_48_reproducible_uuids(self):
        rows = schedule.make_schedule(STUDY, ["a", "b", "c", "d"], 20261003)
        self.assertEqual(rows, schedule.make_schedule(STUDY, ["a", "b", "c", "d"], 20261003))
        self.assertEqual(len({row["run_id"] for row in rows}), 48)
        self.assertNotEqual(rows, schedule.make_schedule(STUDY, ["a", "b", "c", "d"], 7))
        for start in range(0, 48, 6):
            block = rows[start:start + 6]
            self.assertEqual({r["arm"] for r in block}, set("ABCDEF"))
            self.assertEqual(len({r["block"] for r in block}), 1)
        with self.assertRaises(ValueError):
            schedule.make_schedule(STUDY, ["a"] * 4, 0)

    def test_synthetic_complete_registration_validates_but_executes_nothing(self):
        result = self.validate(self.registration())
        self.assertTrue(result["ready_for_external_dispatch"], result)
        self.assertFalse(result["executes_anything"])

    def test_draft_and_missing_integrations_refuse_dispatch(self):
        value = self.registration()
        value["sealed"] = False
        value["integration"] = {"paid_dispatch": False, "final_acceptance": False}
        result = self.validate(value)
        self.assertFalse(result["ready_for_external_dispatch"])
        self.assertIn("paid_dispatch integration remains incomplete", result["errors"])
        self.assertIn("final_acceptance integration remains incomplete", result["errors"])

    def test_http_402_and_mock_jev_do_not_establish_live_feasibility(self):
        for update in ({"outcome": "failed", "http_status": 402}, {"transport": "mock"}, {"served_model": "other"}):
            with self.subTest(update=update):
                value = self.registration()
                ref = value["preflights"]["jev"]
                receipt = json.loads((self.root / ref["path"]).read_text())
                receipt.update(update)
                value["preflights"]["jev"] = self.fixture.write(ref["path"], receipt)
                self.assertFalse(self.validate(value)["ready_for_external_dispatch"])

    def test_tampered_tool_or_missing_checker_artifact_refuses(self):
        value = self.registration()
        (self.root / value["artifacts"]["broker"]["path"]).write_text("changed")
        self.assertFalse(self.validate(value)["ready_for_external_dispatch"])
        value = self.registration()
        del value["task_artifacts"]["t1"]["checker"]
        self.assertFalse(self.validate(value)["ready_for_external_dispatch"])

    def test_only_source_archives_use_the_larger_streamed_reader(self):
        value = self.registration()
        with mock.patch.object(schedule, "artifact", wraps=report.artifact) as reader:
            self.assertTrue(self.validate(value)["ready_for_external_dispatch"])
        calls = reader.call_args_list
        for role in schedule.TASK_ROLES:
            ref = value["task_artifacts"]["t1"][role]
            expected_kind = "source_archive" if role == "source_archive" else "binary"
            self.assertTrue(any(call.args[1:] == (ref, expected_kind) for call in calls), role)
        large_refs = [call.args[1] for call in calls if len(call.args) == 3 and call.args[2] == "source_archive"]
        self.assertEqual(large_refs, [refs["source_archive"] for refs in value["task_artifacts"].values()])

    def test_missing_or_invalid_registered_model_prices_refuse(self):
        for change in ("missing", "nan", "missing_cache_rate"):
            with self.subTest(change=change):
                value = self.registration()
                protocol_ref = value["protocol"]
                protocol = json.loads((self.root / protocol_ref["path"]).read_text())
                for prices in (protocol["executor_prices"], protocol["registration"]["report_bindings"]["prices"]):
                    if change == "missing":
                        del prices["claude-opus-5-5"]
                    elif change == "nan":
                        prices["claude-opus-5-5"]["input"] = float("nan")
                    else:
                        del prices["claude-opus-5-5"]["cache_read"]
                value["protocol"] = self.fixture.write(protocol_ref["path"], protocol)
                self.assertFalse(self.validate(value)["ready_for_external_dispatch"])

    def test_mutated_imported_helper_is_not_covered_by_unchanged_entry_script(self):
        value=self.registration()
        (self.root/'harness'/'capture_limits.py').write_text('changed')
        self.assertFalse(self.validate(value)['ready_for_external_dispatch'])

    def test_selectively_permuted_schedule_refuses(self):
        value = self.registration()
        value["schedule"][0], value["schedule"][1] = value["schedule"][1], value["schedule"][0]
        self.assertFalse(self.validate(value)["ready_for_external_dispatch"])

    def test_reserves_full_block_and_charges_prior_invalidated_work(self):
        rows = [{"run_id": "original", "cost_upper_usd": 40}, {"run_id": "replacement", "cost_upper_usd": 30}]
        self.assertTrue(schedule.admission(rows, 2)["admissible"])
        self.assertFalse(schedule.admission(rows, 3)["admissible"])
        rows[0]["unresolved_reservation_usd"] = 1
        self.assertFalse(schedule.admission(rows, 2)["admissible"])

    def test_unknown_or_duplicate_ledger_identity_refuses(self):
        self.assertFalse(schedule.admission([{"run_id": "unknown", "cost_upper_usd": None}], 1)["admissible"])
        row = {"run_id": "one", "cost_upper_usd": 1}
        self.assertFalse(schedule.admission([row, copy.deepcopy(row)], 1)["admissible"])
        with self.assertRaises(ValueError):
            schedule.admission([], float("nan"))


if __name__ == "__main__":
    unittest.main()
