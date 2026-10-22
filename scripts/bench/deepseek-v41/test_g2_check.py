"""Negative and positive controls for the G2 scorer (#12405 review).

Run: python3 -I scripts/bench/deepseek-v41/test_g2_check.py
"""

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import g2_check  # noqa: E402

GCD = {"kind": "rust_tests", "function": "gcd", "cases": [["12, 18", "6"], ["7, 5", "1"]], "must_pass": True}
PAL = {"kind": "python_tests", "function": "is_palindrome",
       "cases": [["'racecar'", "True"], ["'ab'", "False"]], "must_pass": True}


def rust(body):
    return f"```rust\n{body}\n```"


def py(body):
    return f"```python\n{body}\n```"


class Controls(unittest.TestCase):
    def test_rust_early_exit_fails(self):
        ok, why = g2_check.check(rust("fn gcd(a: u64, b: u64) -> u64 { std::process::exit(0); }"), GCD)
        self.assertFalse(ok, why)

    def test_rust_partial_run_fails(self):
        body = "fn gcd(a: u64, b: u64) -> u64 { if a == 7 { std::process::exit(0) } if b == 0 { a } else { gcd(b, a % b) } }"
        self.assertFalse(g2_check.check(rust(body), GCD)[0])

    def test_rust_wrong_answer_fails(self):
        self.assertFalse(g2_check.check(rust("fn gcd(a: u64, b: u64) -> u64 { 1 }"), GCD)[0])

    def test_rust_answer_main_is_not_run(self):
        body = ("fn gcd(a: u64, b: u64) -> u64 { if b == 0 { a } else { gcd(b, a % b) } }\n"
                "fn main() { println!(\"G2-RESULT 0 ok\"); }")
        self.assertTrue(g2_check.check(rust(body), GCD)[0])

    def test_rust_forged_result_line_fails(self):
        body = "fn gcd(a: u64, b: u64) -> u64 { println!(\"G2-RESULT 1 ok\"); 0 }"
        self.assertFalse(g2_check.check(rust(body), GCD)[0])

    def test_rust_correct_passes(self):
        body = "fn gcd(a: u64, b: u64) -> u64 { if b == 0 { a } else { gcd(b, a % b) } }"
        self.assertTrue(g2_check.check(rust(body), GCD)[0])

    def test_python_system_exit_fails(self):
        self.assertFalse(g2_check.check(py("def is_palindrome(s):\n    raise SystemExit(0)"), PAL)[0])

    def test_python_silent_success_fails(self):
        self.assertFalse(g2_check.check(py("import os\nos._exit(0)\ndef is_palindrome(s):\n    return True"), PAL)[0])

    def test_python_failed_assertion_fails(self):
        self.assertFalse(g2_check.check(py("def is_palindrome(s):\n    return True"), PAL)[0])

    def test_python_correct_passes(self):
        self.assertTrue(g2_check.check(py("def is_palindrome(s):\n    return s == s[::-1]"), PAL)[0])

    def test_json_only_rejects_prose(self):
        a = {"kind": "json_only", "fields": {"name": "Armstrong", "year": 1969}}
        self.assertFalse(g2_check.check('Sure: {"name": "Neil Armstrong", "year": 1969}', a)[0])
        self.assertTrue(g2_check.check('{"name": "Neil Armstrong", "year": 1969}', a)[0])
        self.assertTrue(g2_check.check('```json\n{"name": "Neil Armstrong", "year": 1969}\n```', a)[0])

    def test_last_line_exact(self):
        a = {"kind": "last_line_exact", "pattern": "^72(\\.0+)?( ?km/h)?$"}
        self.assertTrue(g2_check.check("120 km in 100 min.\n72 km/h", a)[0])
        self.assertFalse(g2_check.check("72 km/h, or maybe 80", a)[0])

    def test_empty_answers_fail_the_suite(self):
        suite = Path(__file__).parents[3] / ("crates/psionic/crates/psionic-models/src/testdata/"
                                             "deepseek_v41/g2_v2/task-suite-2.json")
        cases = json.loads(suite.read_text())["cases"]
        answers = {"answers": [{"case": c["id"], "ended": False, "reasoning": "", "content": "", "tokens": 0,
                                "oracle": {"finished": False, "parsed": {}}} for c in cases]}
        with tempfile.TemporaryDirectory() as d:
            ap, out = Path(d) / "a.json", Path(d) / "r.json"
            ap.write_text(json.dumps(answers))
            sys.argv = ["g2_check.py", str(suite), str(ap), str(out)]
            g2_check.main()
            report = json.loads(out.read_text())
        self.assertFalse(report["floor_held"])
        self.assertFalse(report["passed"])


if __name__ == "__main__":
    unittest.main()
