import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("qualification", Path(__file__).with_name("run.py"))
qualification = importlib.util.module_from_spec(spec)
spec.loader.exec_module(qualification)


def response(content="2", reasoning="One plus one is two.", finish="stop"):
    return {"choices": [{"message": {"content": content, "reasoning_content": reasoning}, "finish_reason": finish}],
            "usage": {"completion_tokens": 1}}


class DeepSeekQualificationTests(unittest.TestCase):
    def test_refuses_successful_requests_without_qualified_answers(self):
        for candidate in [response("3"), response(reasoning=""), response(finish="length"),
                          response("<think>work</think>2"), response("2<｜end▁of▁sentence｜>"), {},
                          response(content=2), {"choices": []}]:
            self.assertFalse(qualification.deepseek_response_check(candidate, "2"))
        self.assertTrue(qualification.deepseek_response_check(response(), "2"))

    def test_truncation_requires_reasoning_without_a_final_answer(self):
        self.assertTrue(qualification.deepseek_response_check(response(None, "Let", "length"), truncated=True))
        self.assertTrue(qualification.deepseek_response_check(response(None, "\n", "length"), truncated=True))
        self.assertFalse(qualification.deepseek_response_check(response("Let", None, "length"), truncated=True))
        self.assertFalse(qualification.deepseek_response_check(response(None, "Let", "stop"), truncated=True))

    def test_continuation_and_one_token_budget_are_sent_to_both_engines(self):
        for engine in ("psionic", "llama-cpp"):
            with patch.object(qualification, "read_json", side_effect=[response(), response("3"), response(None, "Let", "length")]) as read:
                report = qualification.qualify_deepseek("http://127.0.0.1:1", "r1", engine, 512)
            self.assertTrue(report["passed"])
            payloads = [call.args[1] for call in read.call_args_list]
            self.assertEqual(payloads[-1]["max_tokens"], 1)
            self.assertEqual(payloads[1]["messages"][1]["content"], "2")
            self.assertEqual("psionic_reasoning" in payloads[0], engine == "psionic")

    def test_any_failed_check_fails_the_run(self):
        with patch.object(qualification, "read_json", side_effect=[response(), response("4"), response(None, "Let", "length")]):
            self.assertFalse(qualification.qualify_deepseek("http://127.0.0.1:1", "r1", "psionic", 512)["passed"])

    def test_runner_returns_nonzero_for_a_failed_report(self):
        row = next(row for row in json.loads(Path(__file__).with_name("models.json").read_text())
                   if row["id"] == "deepseek-r1-distill-qwen-1.5b")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            model = root / row["id"] / row["file"]
            model.parent.mkdir()
            with model.open("wb") as file:
                file.truncate(row["bytes"])
            argv = ["run.py", "--pylon-bin", "pylon", "--psionic-bin", "psionic", "--gpt-oss-bin", "gpt-oss",
                    "--model-dir", str(root), "--output", str(root / "output"), "--models", row["id"], "--no-verify"]
            with patch.object(sys, "argv", argv), patch.object(qualification, "qualify", return_value={"qualified": False}):
                self.assertEqual(qualification.main(), 1)


if __name__ == "__main__":
    unittest.main()
