#!/usr/bin/env python3
"""Oracle tests for tails.py: quantiles recomputed by hand, and every way a
capture can fail to be a device pass."""
import copy, json, unittest
from pathlib import Path
import importlib.util

BASE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("tails", BASE / "tails.py")
tails = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tails)
BUDGET = json.loads((BASE / "frame-budget.json").read_text())


def capture(environment="iPhone 17 Pro (physical)", value=1000, n=540):
    return {"environment": environment, "gpu_errors": 0, "completed_commands": n, "requested_samples": n,
            "metrics": {name: {"samples_ns": [value + i for i in range(n)]} for name in BUDGET["metrics"]}}


class Tails(unittest.TestCase):
    def test_quantiles_are_nearest_rank(self):
        s = tails.stats(list(range(1, 101)))
        self.assertEqual((s["median_ns"], s["p95_ns"], s["p99_ns"], s["max_ns"]), (50.5, 95, 99, 100))
        self.assertEqual(tails.stats([3, 1, 2])["median_ns"], 2)
        self.assertEqual(tails.stats([5, 5, 5, 9])["mad_ns"], 0)

    def test_device_pass_needs_a_physical_device(self):
        self.assertEqual(tails.check(capture(), BUDGET)["verdict"], "device-pass")
        self.assertEqual(tails.check(capture("iOS Simulator"), BUDGET)["verdict"], "simulator-only")
        self.assertEqual(tails.check(capture(""), BUDGET)["verdict"], "simulator-only")

    def test_a_single_long_frame_is_over_budget(self):
        c = capture()
        c["metrics"]["frame_cpu"]["samples_ns"][7] = BUDGET["metrics"]["frame_cpu"]["max_ns"] + 1
        r = tails.check(c, BUDGET)
        self.assertEqual((r["verdict"], r["metrics"]["frame_cpu"]["over"]), ("over-budget", ["max_ns"]))

    def test_missing_execution_is_never_a_pass(self):
        c = capture()
        del c["metrics"]["gpu"]
        self.assertEqual(tails.check(c, BUDGET)["verdict"], "incomplete")
        c = capture(n=100)
        self.assertEqual(tails.check(c, BUDGET)["metrics"]["player_update"]["outcome"], "not-measured")
        c = capture()
        c["gpu_errors"] = 1
        self.assertEqual(tails.check(c, BUDGET)["verdict"], "infrastructure-error")
        c = capture()
        c["completed_commands"] -= 1
        self.assertEqual(tails.check(c, BUDGET)["verdict"], "infrastructure-error")

    def test_enforce_refuses_the_simulator_capture(self):
        sim = BASE.parent.parent / "docs/verification/data/coax-r0-pose-ios-2026-10-28.json.gz"
        self.assertEqual(tails.main([str(sim)]), 0)
        self.assertEqual(tails.main([str(sim), "--enforce"]), 1)


if __name__ == "__main__":
    unittest.main()
