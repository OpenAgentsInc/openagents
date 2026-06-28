#!/usr/bin/env python3
"""Tests for the MirrorCode gym backstop runner core (#6710).

Verifies the genuine-work pieces with NO network and NO Khala spend:
  * extract_code pulls a fenced python block and falls back to raw text;
  * run_solution executes a candidate against hidden test cases (real subprocess
    execution) and reports pass/fail, including the empty / wrong / exception /
    missing-entrypoint cases;
  * evaluate_batch computes per-problem + aggregate pass rates and writes both a
    run summary and per-problem execution traces;
  * a mixed stub model (some correct, some wrong) yields the expected aggregate.

Run: python3 apps/openagents.com/scripts/mirrorcode/backstop_eval_test.py
"""

import json
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import backstop_eval as be  # noqa: E402


class ExtractCodeTests(unittest.TestCase):
    def test_fenced_python_block(self):
        out = "Here you go:\n```python\ndef f(x):\n    return x\n```\nDone."
        self.assertEqual(be.extract_code(out), "def f(x):\n    return x")

    def test_bare_fence(self):
        out = "```\ndef g():\n    return 1\n```"
        self.assertEqual(be.extract_code(out), "def g():\n    return 1")

    def test_raw_fallback(self):
        out = "def h():\n    return 2"
        self.assertEqual(be.extract_code(out), "def h():\n    return 2")

    def test_non_string(self):
        self.assertEqual(be.extract_code(None), "")


class RunSolutionTests(unittest.TestCase):
    def setUp(self):
        self.prob = {
            "id": "sum_list",
            "entrypoint": "sum_list",
            "prompt": "",
            "tests": [[[[1, 2, 3]], 6], [[[]], 0], [[[-1, 1, 10]], 10]],
        }

    def test_correct_solution_passes_all(self):
        passed, total, details = be.run_solution("def sum_list(xs):\n    return sum(xs)", self.prob)
        self.assertEqual((passed, total), (3, 3))
        self.assertNotIn("error", details)

    def test_wrong_solution_fails(self):
        passed, total, _ = be.run_solution("def sum_list(xs):\n    return 0", self.prob)
        self.assertEqual(passed, 1)  # only the [] -> 0 case happens to match
        self.assertEqual(total, 3)

    def test_empty_solution(self):
        passed, total, details = be.run_solution("", self.prob)
        self.assertEqual((passed, total), (0, 3))
        self.assertEqual(details.get("error"), "empty_solution")

    def test_missing_entrypoint(self):
        passed, total, details = be.run_solution("def other():\n    return 1", self.prob)
        self.assertEqual(passed, 0)
        self.assertEqual(details.get("error"), "entrypoint_missing")

    def test_exception_in_candidate_counts_as_fail(self):
        passed, total, _ = be.run_solution("def sum_list(xs):\n    raise ValueError('x')", self.prob)
        self.assertEqual(passed, 0)
        self.assertEqual(total, 3)


class EvaluateBatchTests(unittest.TestCase):
    def test_all_correct(self):
        problems = be.select_problems(3)
        solutions = {
            "sum_list": "```python\ndef sum_list(xs):\n    return sum(xs)\n```",
            "reverse_string": "```python\ndef reverse_string(s):\n    return s[::-1]\n```",
            "is_palindrome": "```python\ndef is_palindrome(s):\n    return s == s[::-1]\n```",
        }

        def model_fn(prompt):
            for pid, sol in solutions.items():
                # match by the entrypoint name appearing in the prompt
                if "`" + pid + "(" in prompt or pid in prompt:
                    return sol
            return ""

        with tempfile.TemporaryDirectory() as d:
            summary = be.evaluate_batch(problems, model_fn, d, run_id="t-all")
            self.assertEqual(summary["problemCount"], 3)
            self.assertEqual(summary["fullySolved"], 3)
            self.assertEqual(summary["passRate"], 1.0)
            self.assertEqual(summary["status"], "passed")
            # summary + per-problem trace files written
            self.assertTrue(os.path.isfile(os.path.join(d, "t-all.json")))
            for pid in solutions:
                tpath = os.path.join(d, "traces", pid + ".json")
                self.assertTrue(os.path.isfile(tpath), tpath)
                with open(tpath) as fh:
                    tr = json.load(fh)
                self.assertEqual(tr["status"], "passed")
                self.assertEqual(tr["demand"]["source"], "gym_backstop")

    def test_mixed_pass_rate(self):
        # 2 problems: one correct (3/3), one all-wrong (0/3) -> aggregate 3/6.
        problems = [
            {"id": "a", "entrypoint": "a", "prompt": "a", "tests": [[[1], 1], [[2], 2], [[3], 3]]},
            {"id": "b", "entrypoint": "b", "prompt": "b", "tests": [[[1], 99], [[2], 99], [[3], 99]]},
        ]

        def model_fn(prompt):
            if prompt == "a":
                return "```python\ndef a(x):\n    return x\n```"
            return "```python\ndef b(x):\n    return x\n```"  # never 99 -> all wrong

        with tempfile.TemporaryDirectory() as d:
            summary = be.evaluate_batch(problems, model_fn, d, run_id="t-mixed")
            self.assertEqual(summary["casesPassed"], 3)
            self.assertEqual(summary["casesTotal"], 6)
            self.assertAlmostEqual(summary["passRate"], 0.5)
            self.assertEqual(summary["fullySolved"], 1)
            self.assertEqual(summary["status"], "failed")

    def test_model_failure_is_fail_soft(self):
        problems = [{"id": "a", "entrypoint": "a", "prompt": "a", "tests": [[[1], 1]]}]

        def model_fn(_prompt):
            raise RuntimeError("network down")

        with tempfile.TemporaryDirectory() as d:
            summary = be.evaluate_batch(problems, model_fn, d, run_id="t-fail")
            self.assertEqual(summary["casesPassed"], 0)
            self.assertEqual(summary["problemCount"], 1)
            with open(os.path.join(d, "traces", "a.json")) as fh:
                tr = json.load(fh)
            self.assertTrue(str(tr["modelError"]).startswith("model_call_failed"))


class SelectionTests(unittest.TestCase):
    def test_limit_bounds_batch(self):
        self.assertEqual(len(be.select_problems(3)), 3)

    def test_limit_zero_returns_all(self):
        self.assertEqual(len(be.select_problems(0)), len(be.FIXTURE_PROBLEMS))

    def test_fixtures_are_well_formed(self):
        for p in be.FIXTURE_PROBLEMS:
            self.assertIn("id", p)
            self.assertIn("entrypoint", p)
            self.assertTrue(p["tests"])


if __name__ == "__main__":
    unittest.main(verbosity=2)
