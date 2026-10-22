"""Local recovery and admission controls; no cloud calls or real services.

Run: python3 scripts/pylon/test_v41_live.py
"""
import importlib.util
import json
import math
from pathlib import Path
import subprocess
import tempfile
import types
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("watcher", Path(__file__).with_name("v41-live.py"))
watcher = importlib.util.module_from_spec(spec)
spec.loader.exec_module(watcher)


class Recovery(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.ledger = self.root / "deepseek-v41.jsonl"
        self.ledger.touch()
        self.state = {"run_id": "my-run", "ledger_ids": [], "transfer_usd": 0,
                      "zones": ["asia-northeast1-b", "asia-northeast1-a"],
                      "cap": 300.0, "until": 361000.0}
        self.now = 1000.0
        self.stopped = False
        for name, value in {
            "LEDGER_DIR": self.root, "HOME": self.root / "pylon-home",
            "save_state": lambda state: None, "log": lambda line: None,
            "STOP": types.SimpleNamespace(exists=lambda: self.stopped),
            "time": types.SimpleNamespace(time=lambda: self.now, sleep=self.sleep),
        }.items():
            p = patch.object(watcher, name, value)
            p.start()
            self.addCleanup(p.stop)

    def sleep(self, seconds):
        self.stopped = True

    def append(self, value):
        with self.ledger.open("a") as stream:
            stream.write(json.dumps(value) + "\n")

    def start(self, ident="ours", run="my-run", zone="asia-northeast1-b", hours=2):
        record = {"event": "start", "id": ident, "run_id": run,
                  "vm": "same-name", "zone": zone, "project": "test",
                  "price_per_hour": 6.654, "hours_requested": hours,
                  "start_epoch": 1000.0, "expires_epoch": 1000.0 + hours * 3600}
        self.append(record)
        return record

    def test_lost_create_reply_counts_without_cloud_json_or_ready(self):
        self.start()
        self.start("unrelated", run="another-run")
        self.assertAlmostEqual(watcher.spend(self.state, self.now), 13.308)
        self.assertEqual(self.state["ledger_ids"], ["ours"])

    def test_old_cloud_json_does_not_select_another_runs_allocation(self):
        watcher.HOME.mkdir()
        (watcher.HOME / "cloud.json").write_text('{"machines":[{"ledger_id":"unrelated"}]}')
        self.start("unrelated", run="another-run")
        self.assertEqual(watcher.spend(self.state, self.now), 0)

    def test_transfer_is_counted_once_even_before_ready(self):
        self.start(zone="us-east1-b")
        self.assertAlmostEqual(watcher.spend(self.state, self.now), 53.308)
        self.assertAlmostEqual(watcher.spend(self.state, self.now), 53.308)

    def test_settlement_and_correction_use_the_guard_cost_rules(self):
        self.start()
        self.append({"event": "stop", "id": "ours", "stop_epoch": 4600.0, "reason": "stopped"})
        self.assertAlmostEqual(watcher.spend(self.state, self.now), 6.654)
        self.append({"event": "correction", "id": "ours", "stop_epoch": 1000.0,
                     "reason": "never-started"})
        self.assertEqual(watcher.spend(self.state, self.now), 0)

    def test_corrupt_ledger_refuses_instead_of_hiding_spend(self):
        self.ledger.write_text('not-json\n{"event":"budget","cap":3000}\n')
        with self.assertRaises(watcher.budget_module().Refused):
            watcher.spend(self.state, self.now)

    def test_cleanup_pins_the_allocation_and_ignores_other_runs(self):
        self.start()
        self.start("unrelated", run="another-run")
        calls = []

        def guard(state, *args):
            calls.append(args)
            if args[0] == "stop":
                self.assertEqual(args, ("stop", "same-name", "--record", "ours"))
                self.append({"event": "stop", "id": "ours", "stop_epoch": 1100.0, "reason": "stopped"})
            return subprocess.CompletedProcess(args, 0, "", "")

        with patch.object(watcher, "run_guard", guard):
            self.assertTrue(watcher.cleanup_owned(self.state))
        self.assertEqual(len(calls), 2)
        self.assertEqual(self.state["cleanup_pending"], [])

    def test_unsettled_cleanup_blocks_a_new_generation(self):
        self.start()
        with patch.object(watcher, "run_guard", return_value=subprocess.CompletedProcess([], 1, "", "unknown")), \
             patch.object(watcher, "launch") as launch, \
             patch.object(watcher, "load_state", return_value=self.state):
            watcher.serve(types.SimpleNamespace())
        launch.assert_not_called()

    def test_allowance_is_recomputed_and_rounded_down_for_every_zone(self):
        spent, attempts = [0.0], []

        def launch(state, zone, hours):
            booked = max(60, math.ceil(round(hours * 3600) / 60) * 60) / 3600
            attempts.append(booked)
            if len(attempts) == 1:
                spent[0] = 10.0
                return "refused", "lost reply after paid setup"
            spent[0] += booked * 6.654
            self.stopped = True
            return "ended", "finished"

        with patch.object(watcher, "load_state", return_value=self.state), \
             patch.object(watcher, "spot_price", return_value=6.654), \
             patch.object(watcher, "spend", side_effect=lambda *args: spent[0]), \
             patch.object(watcher, "launch", launch):
            watcher.serve(types.SimpleNamespace())
        self.assertEqual(len(attempts), 2)
        self.assertLess(attempts[1], attempts[0])
        self.assertLessEqual(spent[0], self.state["cap"])

    def test_an_expired_end_time_prevents_a_fallback_launch(self):
        attempts = []

        def launch(state, zone, hours):
            attempts.append(zone)
            self.now = state["until"] + 1
            return "refused", "setup exhausted the remaining time"

        with patch.object(watcher, "load_state", return_value=self.state), \
             patch.object(watcher, "launch", launch):
            watcher.serve(types.SimpleNamespace())
        self.assertEqual(attempts, [self.state["zones"][0]])


if __name__ == "__main__":
    unittest.main()
