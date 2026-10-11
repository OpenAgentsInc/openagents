"""Tests for the Haiku briefer's answer handling (no model call).

    python3 -m unittest scripts/bench/briefed-ab/test_haiku_briefer.py
"""

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import haiku_briefer as hb  # noqa: E402


class AnswerTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        (self.root / "crates/a/src").mkdir(parents=True)
        (self.root / "crates/a/src/lib.rs").write_text("fn a() {}\n")
        (self.root / "crates/a/src/b.rs").write_text("fn b() {}\n")

    def tearDown(self):
        self.tmp.cleanup()

    def test_files_keep_rank_drop_missing_duplicates_and_escapes(self):
        answer = {"confidence": 0.8, "files": [
            {"path": "crates/a/src/lib.rs", "role": "change", "reason": "has a", "lines": [3, 1, 3, -2]},
            {"path": "crates/a/src/gone.rs", "role": "change", "reason": "made up"},
            {"path": "./crates/a/src/lib.rs", "role": "test", "reason": "again"},
            {"path": "../outside.rs", "role": "context", "reason": "escape"},
            {"path": "crates/a/src/b.rs", "role": "test", "reason": "tests"},
        ]}
        rows = hb.to_files(answer, self.root, 8)
        self.assertEqual([r["path"] for r in rows], ["crates/a/src/lib.rs", "crates/a/src/b.rs"])
        self.assertEqual(rows[0]["lines"], [1, 3])
        self.assertEqual(rows[0]["score"], 0.8)
        self.assertLess(rows[1]["score"], rows[0]["score"])
        self.assertIn("has a", rows[0]["why"][0])

    def test_files_capped_at_k(self):
        answer = {"files": [{"path": "crates/a/src/lib.rs", "role": "change", "reason": ""},
                            {"path": "crates/a/src/b.rs", "role": "change", "reason": ""}]}
        self.assertEqual(len(hb.to_files(answer, self.root, 1)), 1)

    def test_plan_lines(self):
        lines = hb.plan_lines({"likely_change": "Do x.", "acceptance": ["x holds"], "checks": ["cargo test -p a"]})
        self.assertEqual(lines, ["Likely change (Haiku briefer): Do x.", "Acceptance: x holds", "Check: `cargo test -p a`"])

    def test_prompt_lists_candidates(self):
        text = hb.prompt_for({"issue": 1, "title": "T", "body": "B"}, [{"path": "p.rs", "why": ["sym"]}])
        self.assertIn("`p.rs` (sym)", text)
        self.assertNotIn("Candidate", hb.prompt_for({"issue": 1, "title": "T", "body": "B"}, None))


if __name__ == "__main__":
    unittest.main()
