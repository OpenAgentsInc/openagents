"""Tests for the ranker activation gate (LEARN-01).

    python3 -m unittest scripts/filefind/test_ranker_gate.py
"""

import importlib.util
import json
import os
import random
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import ranker_gate as gate  # noqa: E402


def rows(recalls, brier=0.05, seed=0):
    """Per-case rows with recall@50 ~ recalls[i] over 10 files each."""
    rng = random.Random(seed)
    out = []
    for i, r in enumerate(recalls):
        hit = max(0, min(10, round(r * 10)))
        out.append({"issue": 9000 + i, "n": 10, "r20": max(0, hit - 2), "r50": hit, "r100": min(10, hit + 1),
                    "brier_top100": brier + rng.uniform(-0.002, 0.002)})
    return out


def model(issues, name="m"):
    return {"schema": "openagents.filefind.model.v1", "name": name, "stage1": {}, "stage2": {},
            "trained_on": {"cases": len(issues), "issue_list": sorted(issues)}}


class GateTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.d = Path(self.tmp.name)
        self.active = self.d / "model.json"
        self.active.write_text(json.dumps(model(range(1, 100), "active")))
        self.active_bytes = self.active.read_bytes()
        self.eval = list(range(9000, 9060))
        base = [0.5 + 0.1 * ((i * 7) % 5 - 2) / 2 for i in range(60)]
        self.base_rows = rows(base)
        self.better_rows = rows([min(1.0, b + 0.2) for b in base], seed=1)
        self.worse_rows = rows([max(0.0, b - 0.2) for b in base], seed=2)

    def tearDown(self):
        self.tmp.cleanup()

    def candidate(self, name):
        p = self.d / f"{name}.json"
        p.write_text(json.dumps(model(range(1, 200), name)))
        return gate.immutable_copy(str(p), str(self.d / "candidates"))

    def receipt(self, cand, cand_rows, baseline=None):
        r = gate.receipt(str(baseline or self.active), cand, self.eval, self.base_rows, cand_rows, [])
        p = self.d / f"receipt-{os.path.basename(str(baseline or self.active))}-{os.path.basename(cand)}"
        p.write_text(json.dumps(r))
        return str(p), r

    def test_overlap_is_read_from_the_list_or_the_legacy_range(self):
        self.assertEqual(gate.overlap(model([5, 9001]), [9000, 9001]), [9001])
        legacy = {"trained_on": {"cases": 1692, "issues": "#1..#11208"}}
        self.assertEqual(gate.overlap(legacy, [10074, 11300]), [10074])
        self.assertEqual(gate.overlap({}, [1, 2]), [1, 2])  # unknown training set: all overlap

    def test_a_worse_candidate_leaves_the_active_model_unchanged(self):
        cand = self.candidate("worse")
        path, r = self.receipt(cand, self.worse_rows)
        self.assertFalse(r["pass"])
        with self.assertRaises(gate.Refused):
            gate.promote(cand, path, str(self.active))
        self.assertEqual(self.active.read_bytes(), self.active_bytes)
        # An equal candidate does not pass either: the gain must be >= 2 SE.
        same = self.candidate("same")
        self.assertFalse(gate.decide(self.base_rows, self.base_rows)["pass"])
        path, _ = self.receipt(same, self.base_rows)
        with self.assertRaises(gate.Refused):
            gate.promote(same, path, str(self.active))
        self.assertEqual(self.active.read_bytes(), self.active_bytes)

    def test_a_calibration_regression_fails_even_with_better_recall(self):
        worse_cal = [dict(r, brier_top100=r["brier_top100"] + 0.05) for r in self.better_rows]
        v = gate.decide(self.base_rows, worse_cal)
        self.assertFalse(v["pass"])
        self.assertTrue(any("calibration" in w for w in v["why"]))

    def test_a_passing_receipt_promotes_only_its_own_candidate_against_the_active_model(self):
        cand = self.candidate("better")
        path, r = self.receipt(cand, self.better_rows)
        self.assertTrue(r["pass"], r["why"])
        other = self.candidate("other")
        with self.assertRaises(gate.Refused):
            gate.promote(other, path, str(self.active))  # the receipt names another candidate
        stale = self.d / "stale.json"
        stale.write_text(json.dumps(model([1], "stale")))
        stale_path, _ = self.receipt(cand, self.better_rows, baseline=stale)
        with self.assertRaises(gate.Refused):
            gate.promote(cand, stale_path, str(self.active))  # compared against another baseline
        self.assertEqual(self.active.read_bytes(), self.active_bytes)
        gate.promote(cand, path, str(self.active))
        self.assertEqual(gate.digest_file(str(self.active)), r["candidate"]["digest"])
        side = json.loads((self.d / "model.receipt.json").read_text())
        self.assertEqual(side["candidate"]["digest"], r["candidate"]["digest"])

    def test_candidates_are_immutable(self):
        cand = self.candidate("better")
        self.assertEqual(self.candidate("better"), cand)  # same bytes: same file
        Path(cand).chmod(0o644)
        Path(cand).write_text("{}")
        with self.assertRaises(SystemExit):
            self.candidate("better")

    def test_train_refuses_to_write_the_active_model(self):
        sys.path.insert(0, str(HERE.parent / "bench"))
        spec = importlib.util.spec_from_file_location("ffbench", HERE.parent / "bench" / "file-finding-bench.py")
        bench = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(bench)
        a = type("NS", (), {"model": bench.ff.MODEL_PATH, "dataset": str(self.active), "all": False})()
        before = Path(bench.ff.MODEL_PATH).read_bytes()
        with self.assertRaises(SystemExit):
            bench.save_model(a, [{"issue": 1}], {}, {})
        self.assertEqual(Path(bench.ff.MODEL_PATH).read_bytes(), before)
        out = self.d / "cand.json"
        a.model = str(out)
        bench.save_model(a, [{"issue": 3}, {"issue": 1}], {}, {})
        self.assertEqual(json.loads(out.read_text())["trained_on"]["issue_list"], [1, 3])


if __name__ == "__main__":
    unittest.main()
