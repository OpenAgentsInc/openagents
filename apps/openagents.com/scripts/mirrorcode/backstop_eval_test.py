#!/usr/bin/env python3
"""Tests for the MirrorCode gym backstop runner core (#6923).

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


class Issue6735BurnGuardTests(unittest.TestCase):
    """Issue #6735: own-capacity auth + fail-loud + burn smoke."""

    def test_public_base_strips_v1_suffix(self):
        self.assertEqual(be._public_base("https://openagents.com/api/v1"), "https://openagents.com")
        self.assertEqual(be._public_base("https://openagents.com/api/v1/"), "https://openagents.com")
        self.assertEqual(be._public_base("https://x.test/v1"), "https://x.test")
        self.assertEqual(be._public_base("https://x.test"), "https://x.test")

    def test_summary_reports_model_error_count(self):
        problems = [{"id": "a", "entrypoint": "a", "prompt": "a", "tests": [[[1], 1]]}]

        def model_fn(_prompt):
            raise RuntimeError("network down")

        with tempfile.TemporaryDirectory() as d:
            summary = be.evaluate_batch(problems, model_fn, d, run_id="t-errcount")
            self.assertEqual(summary["modelErrorCount"], 1)

    def test_preflight_raises_loud_on_http_error(self):
        def fake_chat(url, api_key, model, prompt, timeout):
            raise be.KhalaCallError("khala_http_error status=403", status=403, body="error code: 1010")

        orig = be._khala_chat_raw
        be._khala_chat_raw = fake_chat
        try:
            with self.assertRaises(be.KhalaCallError) as ctx:
                be.live_preflight(base_url="https://x.test/api/v1", api_key="oa_agent_x", model="openagents/khala")
            self.assertEqual(ctx.exception.status, 403)
        finally:
            be._khala_chat_raw = orig

    def test_preflight_raises_loud_on_zero_usage(self):
        def fake_chat(url, api_key, model, prompt, timeout):
            return {"choices": [{"message": {"content": "ok"}}], "usage": {"total_tokens": 0}}

        orig_chat, orig_counter = be._khala_chat_raw, be.fetch_tokens_served
        be._khala_chat_raw = fake_chat
        be.fetch_tokens_served = lambda *a, **k: 100
        try:
            with self.assertRaises(be.KhalaCallError):
                be.live_preflight(base_url="https://x.test/api/v1", api_key="oa_agent_x")
        finally:
            be._khala_chat_raw, be.fetch_tokens_served = orig_chat, orig_counter

    def test_preflight_ok_reports_usage_and_counter_delta(self):
        def fake_chat(url, api_key, model, prompt, timeout):
            return {"choices": [{"message": {"content": "```python\ndef ping():\n    return 'pong'\n```"}}], "usage": {"total_tokens": 42, "prompt_tokens": 30, "completion_tokens": 12}}

        counters = iter([1000, 1042])
        orig_chat, orig_counter = be._khala_chat_raw, be.fetch_tokens_served
        be._khala_chat_raw = fake_chat
        be.fetch_tokens_served = lambda *a, **k: next(counters)
        try:
            pf = be.live_preflight(base_url="https://x.test/api/v1", api_key="oa_agent_x")
            self.assertTrue(pf["ok"])
            self.assertEqual(pf["usageTotalTokens"], 42)
            self.assertEqual(pf["counterDelta"], 42)
        finally:
            be._khala_chat_raw, be.fetch_tokens_served = orig_chat, orig_counter

    def test_main_live_fails_loud_when_preflight_unauthorized(self):
        def boom(*a, **k):
            raise be.KhalaCallError("khala_http_error status=401", status=401, body='{"error":"unauthorized"}')

        orig = be.live_preflight
        be.live_preflight = boom
        try:
            rc = be.main(["--live", "--limit", "1"])
            self.assertEqual(rc, 1)
        finally:
            be.live_preflight = orig

    def test_main_smoke_returns_zero_on_real_burn(self):
        orig = be.live_preflight
        be.live_preflight = lambda **k: {"ok": True, "usageTotalTokens": 7, "promptTokens": 4, "completionTokens": 3, "contentChars": 5, "counterBefore": 1, "counterAfter": 8, "counterDelta": 7}
        try:
            rc = be.main(["--smoke"])
            self.assertEqual(rc, 0)
        finally:
            be.live_preflight = orig


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
