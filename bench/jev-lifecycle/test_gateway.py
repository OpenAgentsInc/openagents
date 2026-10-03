"""Exercise the gateway helper without network access or paid inference."""

import copy
import hashlib
import io
import json
import os
from pathlib import Path
import stat
import tempfile
import unittest
from unittest.mock import patch
import urllib.error
import urllib.request

import gateway


QUESTIONS = {
    "present": {"type": "noul", "instructions": "Is evidence present?"},
    "kind": {"type": "choice", "instructions": "Select its kind.",
             "criteria": {"a": "First", "b": "Second"}},
    "rank": {"type": "score", "instructions": "Rank the evidence.",
             "criteria": ["Absent", "Partial", "Complete"]},
}


def answer():
    return {
        "model": gateway.MODEL,
        "answers": {
            "present": {"type": "noul", "noul": 0.8},
            "kind": {"type": "choice", "choice": "a", "confidence": 0.4,
                     "probabilities": {"a": 0.8, "b": 0.2}},
            "rank": {"type": "score", "score": 1.75, "confidence": 0.5,
                     "probabilities": {"0": 0, "1": 0.25, "2": 0.75},
                     "legend": {"0": "Absent", "1": "Partial", "2": "Complete"}},
        },
        "usage": {"input_tokens": 275, "output_tokens": 20},
        "provider_metadata": {"gateway": {
            "cost": "0.00001155", "marketCost": "0.00001155",
            "gatewayCost": "0.00001155", "surchargeCost": "0",
            "generationId": "synthetic-generation",
            "routing": {"originalModelId": gateway.MODEL, "finalProvider": "typesafe-ai"},
        }},
    }


class Response(io.BytesIO):
    def __init__(self, raw, code=200, headers=None):
        super().__init__(raw)
        self.code = code
        self.headers = headers or {}


class GatewayTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name) / "call"
        self.secret = "synthetic-private-credential"

    def invoke(self, data=None, *, raw=None, code=200, headers=None, error=None, env=None):
        raw = raw if raw is not None else json.dumps(data if data is not None else answer()).encode()
        response = Response(raw, code, headers)
        with patch.dict(os.environ, {"AI_GATEWAY_API_KEY": self.secret} if env is None else env, clear=True):
            with patch("urllib.request.OpenerDirector.open", side_effect=error, return_value=response) as opened:
                receipt, parsed = gateway.call({"text": "Synthetic input"}, QUESTIONS, self.directory)
        self.assertEqual(opened.call_count, receipt["attempts"])
        self.assertEqual(json.loads((self.directory / "receipt.json").read_text()), receipt)
        for path in self.directory.iterdir():
            self.assertNotIn(self.secret.encode(), path.read_bytes())
        return receipt, parsed, opened

    def test_fixed_request_and_exact_private_artifacts(self):
        raw = json.dumps(answer(), indent=3).encode() + b"\n"
        receipt, parsed, opened = self.invoke(raw=raw)
        request = opened.call_args.args[0]
        self.assertEqual(request.full_url, gateway.ENDPOINT)
        self.assertEqual(request.method, "POST")
        self.assertEqual(request.get_header("Authorization"), "Bearer " + self.secret)
        self.assertEqual(opened.call_args.kwargs["timeout"], 30)
        self.assertEqual((self.directory / "request.json").read_bytes(), request.data)
        self.assertEqual(set(json.loads(request.data)), {"model", "state", "questions"})
        self.assertEqual(json.loads(request.data)["model"], gateway.MODEL)
        self.assertEqual((self.directory / "response.json").read_bytes(), raw)
        self.assertEqual(receipt["response_sha256"], hashlib.sha256(raw).hexdigest())
        self.assertEqual(receipt["gateway_metadata"], answer()["provider_metadata"]["gateway"])
        self.assertEqual(receipt["cost_usd_decimal"], "0.00001155")
        self.assertEqual(receipt["cost_usd"], 0.00001155)
        self.assertTrue(receipt["answers_valid"])
        self.assertEqual(receipt["status"], "answered")
        self.assertTrue(receipt["started_at"].endswith("Z"))
        self.assertLessEqual(receipt["started_at"], receipt["launch_intent_at"])
        self.assertLessEqual(receipt["launch_intent_at"], receipt["finished_at"])
        self.assertFalse(receipt["version_pinned"])
        self.assertEqual(parsed, answer())
        self.assertEqual(stat.S_IMODE(self.directory.stat().st_mode), 0o700)
        for path in self.directory.iterdir():
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o600)

    def test_missing_credential_is_zero_attempts(self):
        receipt, parsed, _ = self.invoke(env={})
        self.assertEqual(receipt["outcome"], "not_sent")
        self.assertEqual(receipt["cost_status"], "no_call")
        self.assertEqual(receipt["cost_usd"], 0)
        self.assertIsNone(parsed)

    def test_timeout_is_not_retried_and_cost_is_unknown(self):
        receipt, parsed, _ = self.invoke(error=TimeoutError(self.secret))
        self.assertEqual(receipt["outcome"], "transport_error")
        self.assertEqual(receipt["attempts"], 1)
        self.assertEqual(receipt["cost_status"], "unknown")
        self.assertIsNone(receipt["cost_usd"])
        self.assertIsNone(parsed)

    def test_launch_intent_is_durable_before_http_and_replaced_afterward(self):
        def inspect(_request, **_kwargs):
            checkpoint = json.loads((self.directory / "receipt.json").read_text())
            self.assertEqual(checkpoint["status"], "launch_intent")
            self.assertEqual(checkpoint["cost_status"], "unknown")
            self.assertIsNone(checkpoint["cost_usd"])
            self.assertIsNone(checkpoint["finished_at"])
            self.assertEqual(checkpoint["attempts"], 1)
            return Response(json.dumps(answer()).encode())
        receipt, _, _ = self.invoke(error=inspect)
        self.assertEqual(receipt["status"], "answered")
        self.assertFalse((self.directory / ".receipt.json.tmp").exists())

    def test_http_refusal_retains_body_without_retry(self):
        raw = b'{"message":"refused","error_type":"payment_required"}'
        error = urllib.error.HTTPError(gateway.ENDPOINT, 402, self.secret,
                                      {"retry-after": "60"}, io.BytesIO(raw))
        receipt, parsed, _ = self.invoke(error=error)
        self.assertEqual(receipt["http_status"], 402)
        self.assertEqual(receipt["outcome"], "http_error")
        self.assertEqual(receipt["response_headers"]["retry-after"], "60")
        self.assertEqual((self.directory / "response.json").read_bytes(), raw)
        self.assertIsNone(receipt["cost_usd"])
        self.assertIsNone(parsed)

    def test_redirect_handler_and_proxy_policy_are_explicit(self):
        real_builder = urllib.request.build_opener
        with patch("urllib.request.build_opener", wraps=real_builder) as builder:
            self.invoke()
        handlers = builder.call_args.args
        self.assertEqual(handlers[0].proxies, {})
        self.assertIsInstance(handlers[1], gateway.NoRedirect)
        for status in (301, 302, 303, 307, 308):
            with self.subTest(status=status), self.assertRaises(urllib.error.HTTPError):
                handlers[1].redirect_request(urllib.request.Request(gateway.ENDPOINT),
                                            None, status, "Redirect", {}, "https://example.invalid")

    def test_invalid_answers_do_not_erase_reported_charge(self):
        data = answer()
        data["answers"]["rank"]["score"] = True
        receipt, parsed, _ = self.invoke(data)
        self.assertEqual(receipt["outcome"], "invalid_response")
        self.assertEqual(receipt["cost_status"], "gateway_reported")
        self.assertEqual(receipt["cost_usd"], 0.00001155)
        self.assertIsNone(parsed)

    def test_missing_cost_does_not_become_free_inference(self):
        data = answer()
        del data["provider_metadata"]["gateway"]["cost"]
        receipt, parsed, _ = self.invoke(data)
        self.assertTrue(receipt["answers_valid"])
        self.assertEqual(receipt["cost_status"], "unknown")
        self.assertIsNone(receipt["cost_usd"])
        self.assertIsNotNone(parsed)

    def test_duplicate_and_nonfinite_json_rejected_with_artifacts(self):
        for index, raw in enumerate((b'{"model":"a","model":"b"}', b'null',
                                     b'{"usage":NaN}', b'{"usage":1e999}', b'not JSON')):
            self.directory = Path(self.temporary.name) / str(index)
            receipt, parsed, _ = self.invoke(raw=raw)
            self.assertEqual(receipt["outcome"], "invalid_response")
            self.assertIsNone(parsed)
            self.assertEqual((self.directory / "response.json").read_bytes(), raw)

    def test_credential_echo_is_not_saved_even_unicode_escaped(self):
        for index, secret in enumerate((self.secret, "\\u0073" + self.secret[1:])):
            self.directory = Path(self.temporary.name) / str(index)
            receipt, parsed, _ = self.invoke(raw=('{"error":"' + secret + '"}').encode())
            self.assertEqual(receipt["error_type"], "credential_echo_suppressed")
            self.assertFalse((self.directory / "response.json").exists())
            self.assertIsNone(parsed)

    def test_alias_mismatch_and_gateway_fallback_are_invalid(self):
        data = answer()
        data["model"] = "jev-1.13.0"
        receipt, parsed, _ = self.invoke(data)
        self.assertEqual(receipt["returned_model"], "jev-1.13.0")
        self.assertFalse(receipt["answers_valid"])
        self.assertIsNone(parsed)
        self.directory = Path(self.temporary.name) / "fallback"
        receipt, parsed, _ = self.invoke(headers={"x-ai-gateway-evaluation-fallback-triggered": "true"})
        self.assertFalse(receipt["answers_valid"])
        self.assertEqual(receipt["cost_status"], "gateway_reported")
        self.assertIsNone(parsed)

    def test_oversized_response_retains_failure_not_a_truncated_artifact(self):
        with patch.object(gateway, "MAX_RESPONSE_BYTES", 32):
            receipt, parsed, _ = self.invoke(raw=b"x" * 100)
        self.assertEqual(receipt["response_bytes"], 33)
        self.assertEqual(receipt["outcome"], "invalid_response")
        self.assertFalse((self.directory / "response.json").exists())
        self.assertIsNone(parsed)

    def test_input_rejection_precedes_http_and_existing_output_is_preserved(self):
        with patch.dict(os.environ, {"AI_GATEWAY_API_KEY": self.secret}, clear=True):
            with patch("urllib.request.OpenerDirector.open") as opened:
                bad = copy.deepcopy(QUESTIONS)
                bad["present"]["providerOptions"] = {"gateway": {"models": ["other"]}}
                for state, questions, timeout in (("x", bad, 30), ("x", QUESTIONS, True),
                                                  (self.secret, QUESTIONS, 30),
                                                  ("x" * gateway.MAX_REQUEST_BYTES, QUESTIONS, 30)):
                    with self.assertRaises(ValueError):
                        gateway.call(state, questions, self.directory, timeout)
                self.directory.mkdir()
                (self.directory / "preserved").write_text("unchanged")
                with self.assertRaises(FileExistsError):
                    gateway.call("x", QUESTIONS, self.directory)
                self.assertEqual((self.directory / "preserved").read_text(), "unchanged")
                self.assertEqual(opened.call_count, 0)


class ValidationTests(unittest.TestCase):
    def test_documented_question_bounds_and_instructions(self):
        for levels in (1, 11):
            questions = copy.deepcopy(QUESTIONS)
            questions["rank"]["criteria"] = ["Level"] * levels
            with self.assertRaises(ValueError):
                gateway.validate_questions(questions)
        for options in (0, 256):
            questions = copy.deepcopy(QUESTIONS)
            questions["kind"]["criteria"] = {str(index): "Option" for index in range(options)}
            with self.assertRaises(ValueError):
                gateway.validate_questions(questions)
        questions = copy.deepcopy(QUESTIONS)
        questions["kind"]["criteria"] = {"only": "Only option"}
        gateway.validate_questions(questions)
        for instructions in (None, "", "  ", [], {}, True):
            questions = copy.deepcopy(QUESTIONS)
            questions["present"]["instructions"] = instructions
            with self.assertRaises(ValueError):
                gateway.validate_questions(questions)
        for instructions in ({"task": "Decide"}, ["Decide"]):
            questions = copy.deepcopy(QUESTIONS)
            questions["present"]["instructions"] = instructions
            gateway.validate_questions(questions)

    def test_all_three_native_types(self):
        self.assertEqual(gateway.validate_answers(answer(), QUESTIONS), answer()["answers"])

    def test_answer_id_and_type_mismatch(self):
        for mutation in (lambda data: data["answers"].pop("present"),
                         lambda data: data["answers"].update(extra={}),
                         lambda data: data["answers"]["rank"].update(type="noul")):
            data = answer()
            mutation(data)
            with self.assertRaises(ValueError):
                gateway.validate_answers(data, QUESTIONS)
        with self.assertRaises(ValueError):
            gateway.loads('{"answers":{"q":{},"q":{}}}')
        with self.assertRaises(ValueError):
            gateway.validate_questions({1: {"type": "noul"}, "1": {"type": "noul"}})

    def test_bool_nonfinite_out_of_range_and_wrong_distributions(self):
        mutations = [
            ("present", "noul", True), ("present", "noul", -0.01),
            ("present", "noul", float("nan")), ("kind", "confidence", False),
            ("kind", "choice", "unknown"), ("kind", "choice", "b"),
            ("kind", "probabilities", {"a": 0.5}),
            ("kind", "probabilities", {"a": True, "b": 0}),
            ("kind", "probabilities", {"a": 0.1, "b": 0.1}),
            ("rank", "score", 0), ("rank", "score", 3),
            ("rank", "probabilities", {}), ("rank", "legend", {"0": "different"}),
        ]
        for question, field, value in mutations:
            with self.subTest(question=question, field=field, value=value):
                data = answer()
                data["answers"][question][field] = value
                with self.assertRaises(ValueError):
                    gateway.validate_answers(data, QUESTIONS)

    def test_cost_parsing_zero_and_no_double_counting(self):
        data = answer()
        for value in ("0", 0, 0.00001155, "0.00001155"):
            data["provider_metadata"]["gateway"]["cost"] = value
            result = gateway.accounting(data)
            self.assertEqual(result["cost_status"], "gateway_reported")
            self.assertEqual(result["cost_usd"], float(value))

    def test_invalid_cost_and_conflicts_never_report_zero(self):
        for value in (True, None, [], -1, "-1", "NaN", "Infinity", "1e999", "1e-999"):
            data = answer()
            data["provider_metadata"]["gateway"]["cost"] = value
            result = gateway.accounting(data)
            self.assertEqual(result["cost_status"], "unknown")
            self.assertIsNone(result["cost_usd"])
        data = answer()
        data["usage"]["cost"] = "0.1"
        self.assertEqual(gateway.accounting(data)["cost_status"], "conflict")
        data["usage"]["cost"] = "0.000011550"
        self.assertEqual(gateway.accounting(data)["cost_status"], "gateway_reported")
        data["usage"]["input_tokens"] = True
        self.assertIsNone(gateway.accounting(data)["cost_usd"])


if __name__ == "__main__":
    unittest.main()
