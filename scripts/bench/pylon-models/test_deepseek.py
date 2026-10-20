import importlib.util
import json
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
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

    def test_no_public_model_names_deepseek_v41_yet(self):
        # V4.1 is protocol-qualified only (#12372); no catalog may offer it.
        root = Path(__file__).resolve().parents[3]
        sources = [Path(__file__).with_name("models.json")] + sorted((root / "crates/pylon/src").glob("*.rs"))
        for source in sources:
            text = source.read_text().lower()
            for name in ("v4.1", "v41", "deepseek-v4"):
                self.assertNotIn(name, text, f"{source} names {name}")

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

    def test_boxed_math_preserves_strict_format_failures(self):
        boxed = response("The answer is \\boxed{2}.")
        self.assertTrue(qualification.deepseek_boxed_response_check(boxed, "2"))
        self.assertFalse(qualification.deepseek_response_check(boxed, "2"))
        for candidate in [response("\\boxed{3}"), response("2"),
                          response("\\boxed{2} \\boxed{3}"), response("\\boxed{2}", reasoning=""),
                          response("\\boxed{2}", finish="length"), response("<think>work</think>\\boxed{2}")]:
            self.assertFalse(qualification.deepseek_boxed_response_check(candidate, "2"))
        self.assertFalse(qualification.deepseek_response_check(response(0, "work", "length"), truncated=True))

    def test_runtime_uses_actual_history_and_recommended_sampling(self):
        first = response("Here is \\boxed{2}", "First thought")
        continuation = response("Result: \\boxed{3}")
        truncated = response(None, "First", "length")
        stopped = response(None, "First thought", "stop")
        events = {"done": True, "events": [{"choices": [{"delta": {"reasoning_content": "First"},
                                                         "finish_reason": "length"}]}]}
        with patch.object(qualification, "read_json", side_effect=[first, continuation, truncated, truncated, stopped]) as read, \
             patch.object(qualification, "read_chat_stream", return_value=events):
            report = qualification.qualify_deepseek_runtime("http://127.0.0.1:1", "r1", "psionic", 2048)
        self.assertTrue(report["passed"])
        payloads = [call.args[1] for call in read.call_args_list]
        self.assertEqual(payloads[1]["messages"][1]["content"], first["choices"][0]["message"]["content"])
        self.assertEqual(payloads[0]["temperature"], 0.6)
        self.assertEqual(payloads[0]["top_p"], 0.95)
        self.assertEqual(payloads[-1]["stop"], ["</think>"])
        self.assertEqual(len(report["checks"]), 6)

    def test_runtime_rejects_stream_reasoning_leak_and_changed_request_state(self):
        truncated = response(None, "First", "length")
        events = {"done": True, "events": [{"choices": [{"delta": {"content": "First"}, "finish_reason": "length"}]}]}
        with patch.object(qualification, "read_json", side_effect=[response("\\boxed{2}"), response("\\boxed{3}"),
                          truncated, response(None, "Changed", "length"), response(None, "work")]), \
             patch.object(qualification, "read_chat_stream", return_value=events):
            report = qualification.qualify_deepseek_runtime("http://127.0.0.1:1", "r1", "llama-cpp", 2048)
        self.assertFalse(report["passed"])
        self.assertFalse(report["checks"][3]["passed"])
        self.assertFalse(report["checks"][-1]["passed"])

    def test_greedy_reference_keeps_numeric_format_failure_visible(self):
        truncated = response(None, "First", "length")
        events = {"done": True, "events": [{"choices": [{"delta": {"reasoning_content": "First"}, "finish_reason": "length"}]}]}
        saved = []
        with patch.object(qualification, "read_json", side_effect=[response("\\boxed{2}"), response("\\boxed{3}"),
                          truncated, truncated, response(None, "work")]) as read, \
             patch.object(qualification, "read_chat_stream", return_value=events):
            report = qualification.qualify_deepseek_runtime("http://127.0.0.1:1", "r1", "psionic", 2048,
                                                           saved.append, "greedy")
        self.assertTrue(report["passed"])
        self.assertEqual(read.call_args_list[0].args[1]["temperature"], 0)
        self.assertFalse(report["checks"][0]["strict_numeric_only"])
        self.assertFalse(report["checks"][1]["strict_numeric_only"])
        self.assertEqual(len(saved), 6)

    def test_gpu_qualification_refuses_unobserved_cuda_execution(self):
        row = next(row for row in json.loads(Path(__file__).with_name("models.json").read_text())
                   if row["id"] == "deepseek-r1-distill-qwen-1.5b")
        with tempfile.TemporaryDirectory() as directory:
            args = SimpleNamespace(model_dir=Path(directory), max_tokens=128, thinking="on", engine="llama-cpp",
                                   llama_cpp_bin=Path("server"), llama_cpp_device="CUDA0", port=1, threads=8,
                                   start_timeout=1, deepseek_sampling="recommended", direct_only=True)
            engine = SimpleNamespace(pid=1, poll=lambda: None)
            with patch.object(qualification.subprocess, "Popen", return_value=engine), \
                 patch.object(qualification, "stop_process"), \
                 patch.object(qualification, "read_json", return_value={"data": [{"id": "r1"}]}), \
                 patch.object(qualification, "qualify_deepseek_runtime", return_value={"passed": True, "scope": "text_runtime_math"}), \
                 patch.object(qualification, "engine_memory", return_value={}):
                report = qualification.qualify(row, args, Path(directory))
            self.assertFalse(report["qualified"])
            self.assertIn("observed GPU memory", report["error"])
            with patch.object(qualification.subprocess, "Popen", return_value=engine), \
                 patch.object(qualification, "stop_process"), \
                 patch.object(qualification, "read_json", return_value={"data": [{"id": "r1"}]}), \
                 patch.object(qualification, "qualify_deepseek_runtime", return_value={"passed": True, "scope": "text_runtime_math"}), \
                 patch.object(qualification, "engine_memory", return_value={"gpu_mib": 1}):
                report = qualification.qualify(row, args, Path(directory))
            self.assertFalse(report["qualified"])
            self.assertIn("all 29 layers", report["error"])
            def launch(*args, **kwargs):
                (Path(directory)/row["id"]/"engine.log").write_text("offloaded 29/29 layers to GPU\n")
                return engine
            with patch.object(qualification.subprocess, "Popen", side_effect=launch), \
                 patch.object(qualification, "stop_process"), \
                 patch.object(qualification, "read_json", return_value={"data": [{"id": "r1"}]}), \
                 patch.object(qualification, "qualify_deepseek_runtime", return_value={"passed": True, "scope": "text_runtime_math"}), \
                 patch.object(qualification, "engine_memory", return_value={"gpu_mib": 1390}):
                report = qualification.qualify(row, args, Path(directory))
            self.assertTrue(report["qualified"])
            self.assertEqual(report["gpu_offload"]["offloaded_layers"], 29)

    def test_native_cuda_requires_gpu_memory_without_llama_offload_logs(self):
        row = next(row for row in json.loads(Path(__file__).with_name("models.json").read_text())
                   if row["id"] == "deepseek-r1-distill-qwen-1.5b")
        with tempfile.TemporaryDirectory() as directory:
            args = SimpleNamespace(model_dir=Path(directory), max_tokens=2048, thinking="off", engine="psionic",
                                   psionic_bin=Path("psionic"), native_backend="cuda", port=1, threads=8,
                                   start_timeout=1, deepseek_sampling="recommended", direct_only=True)
            engine = SimpleNamespace(pid=1, poll=lambda: None)
            for memory, expected in [({}, False), ({"gpu_mib": 900}, True)]:
                with patch.object(qualification.subprocess, "Popen", return_value=engine), \
                     patch.object(qualification, "stop_process"), \
                     patch.object(qualification, "read_json", return_value={"data": [{"id": "r1"}]}), \
                     patch.object(qualification, "qualify_deepseek_runtime", return_value={"passed": True, "scope": "text_runtime_math"}), \
                     patch.object(qualification, "engine_memory", return_value=memory):
                    report = qualification.qualify(row, args, Path(directory))
                self.assertEqual(report["qualified"], expected)
                self.assertEqual(report["execution_backend"], "cuda")
                self.assertNotIn("gpu_offload", report)
                self.assertIn("--single-generation", report["server_command"])


if __name__ == "__main__":
    unittest.main()
