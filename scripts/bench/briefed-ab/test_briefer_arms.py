"""Tests for the Briefer arms measurement (no model call).

    python3 -m unittest scripts/bench/briefed-ab/test_briefer_arms.py
"""

import json
import os
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import briefer_arms as arms  # noqa: E402


class KeepTests(unittest.TestCase):
    def test_keeps_over_threshold_and_tops_up_to_three(self):
        self.assertEqual(arms.keep_set([("a", 0.9), ("b", 0.1), ("c", 0.05), ("d", 0.2)]), {"a", "d", "b"})
        self.assertEqual(arms.keep_set([("a", 0.9), ("b", 0.8), ("c", 0.7), ("d", 0.6)]), {"a", "b", "c", "d"})


class SummaryTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.old = arms.WORK
        arms.WORK = Path(self.tmp.name)

    def tearDown(self):
        arms.WORK = self.old
        self.tmp.cleanup()

    def write(self, arm, row):
        d = arms.WORK / "briefer-arms" / "t" / arm
        d.mkdir(parents=True, exist_ok=True)
        (d / f"{row['issue']}.json").write_text(json.dumps(row))

    def test_accuracy_agreement_and_divergence_are_recorded(self):
        base = {"issue": 1, "candidates": ["a", "b", "c"], "fix": ["a", "z"], "extra": [], "kind": None}
        self.write("clef", dict(base, arm="clef", ok=True, keep=["a", "b"], secs=1, cost_usd=0.0, unpriced=0,
                                errors=[], judgments=[{"path": "a", "p": 0.9, "decision": "keep"},
                                                      {"path": "b", "p": 0.5, "decision": "keep"},
                                                      {"path": "c", "p": 0.1, "decision": "drop"}]))
        self.write("haiku", dict(base, arm="haiku", ok=True, keep=["a"], secs=2, cost_usd=0.01, unpriced=0,
                                 errors=[], extra=["z"], answer={"files": [{"path": "a"}]},
                                 judgments=[{"path": "a", "decision": "keep"}, {"path": "b", "decision": "drop"},
                                            {"path": "c", "decision": "drop"}]))
        self.write("jev", dict(base, arm="jev", ok=False, keep=[], secs=0, cost_usd=0.0, unpriced=0,
                               errors=["kind: HTTP 402"], judgments=[]))
        s = arms.summarize({"tag": "t", "issues": [1]})
        self.assertEqual(s["per_arm"]["clef"]["accuracy"], round(2 / 3, 3))
        self.assertEqual(s["per_arm"]["haiku"]["accuracy"], 1.0)
        self.assertEqual(s["per_arm"]["haiku"]["fix_recall"], 1.0)  # z named outside the candidates
        self.assertEqual(s["per_arm"]["clef"]["fix_recall"], 0.5)
        self.assertEqual(s["per_arm"]["jev"]["answered"], 0)
        self.assertEqual(s["agreement"]["clef~haiku"], {"pairs": 3, "agree": round(2 / 3, 3)})
        self.assertEqual(len(s["divergences"]), 1)
        self.assertEqual(s["divergences"][0]["path"], "b")
        self.assertFalse(s["divergences"][0]["fix_changed_it"])


if __name__ == "__main__":
    unittest.main()
