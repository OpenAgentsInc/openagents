"""Check source binding, context bounds, and refusal handling without inference."""
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
import urllib.error
import urllib.request

import prepare


def row(name, line, text):
    return {"id": name, "path": "src/lib.rs", "file_sha256": "a" * 64,
            "name": name, "kind": "function_item", "start_line": line,
            "end_line": line, "text": text, "lexical_score": 10 - line}


class PreparationTests(unittest.TestCase):
    def test_semantic_rank_changes_order_without_expanding_budget(self):
        rows = [row("c01", 1, "fn one() {}\n"), row("c02", 2, "fn two() {}\n")]
        a, order_a = prepare.render("a" * 40, rows)
        b, order_b = prepare.render("a" * 40, rows, {"c01": 0.3, "c02": 2.9})
        self.assertEqual(order_a, ["c01", "c02"])
        self.assertEqual(order_b, ["c02", "c01"])
        self.assertLessEqual(len(a.encode()), prepare.PACK_BYTES)
        self.assertIn("fn one()", b)

    def test_complete_unit_omission_and_markdown_fence(self):
        rows = [row("c01", 1, "// ```\nfn one() {}\n"),
                row("c02", 2, "x" * prepare.PACK_BYTES)]
        text, selected = prepare.render("a" * 40, rows)
        self.assertEqual(selected, ["c01"])
        self.assertIn("````rust", text)
        self.assertNotIn("xxxx", text)

    def test_invalid_semantic_response_cannot_control_selection(self):
        rows = [row("c01", 1, "fn one() {}")]
        good = {"model": prepare.MODEL, "answers": {"c01": {
            "type": "score", "score": 2.0,
            "probabilities": {"0": 0, "1": 0, "2": 1, "3": 0}}}}
        self.assertEqual(prepare.scores(good, rows), {"c01": 2.0})
        for field, value in [("model", "changed"), ("answers", {})]:
            with self.assertRaises(ValueError):
                prepare.scores({**good, field: value}, rows)
        good["answers"]["c01"]["score"] = float("nan")
        with self.assertRaises(ValueError):
            prepare.scores(good, rows)

    def test_ambiguous_network_failure_keeps_unknown_cost(self):
        with tempfile.TemporaryDirectory() as directory:
            with patch.dict("os.environ", {"TYPESAFE_API_KEY": "test-credential"}):
                with patch("urllib.request.OpenerDirector.open", side_effect=TimeoutError("test-credential")):
                    selected, receipt = prepare.system_one(
                        {"title": "Synthetic check"}, [row("c01", 1, "fn one() {}")], Path(directory))
            self.assertEqual(selected, {})
            self.assertIsNone(receipt["cost_usd"])
            self.assertEqual(receipt["attempts"], 1)
            self.assertEqual(receipt["selection"], "deterministic_fallback")
            self.assertNotIn("test-credential", (Path(directory) / "jev-call.json").read_text())

    def test_path_binding_rejects_a_valid_blob_from_another_path(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            (root / "a.rs").write_text("fn a() {}\n")
            (root / "b.rs").write_text("fn b() {}\n")
            subprocess.run(["git", "-C", str(root), "add", "."], check=True)
            subprocess.run(["git", "-C", str(root), "-c", "user.name=Test", "-c", "user.email=test@example.test", "commit", "-qm", "Fixture"], check=True)
            rev = prepare.git(root, "rev-parse", "HEAD").decode().strip()
            raw = (root / "a.rs").read_bytes()
            entry = {"path": "a.rs", "blob": prepare.git(root, "rev-parse", rev + ":a.rs").decode().strip(),
                     "size": len(raw), "sha256": prepare.digest(raw)}
            self.assertEqual(prepare.source_text(root, rev, entry), raw.decode())
            entry["path"] = "b.rs"
            with self.assertRaises(ValueError):
                prepare.source_text(root, rev, entry)


class AdversarialPreparationTests(unittest.TestCase):
    def test_candidate_pool_is_stable_across_python_hash_seeds(self):
        query = ["alpha" + str(i) for i in range(30)]
        files = []
        for n in range(30):
            words = [word for i, word in enumerate(query) if (i + n) % 3 != 0 or i % 5 == 0]
            files.append({
                "path": f"src/unit{n}.rs", "terms": words, "sha256": "a" * 64,
                "text": f"fn unit{n}() {{ /* " + " ".join(words) + " */ }\n",
                "syntax": {"declarations": [{
                    "kind": "function_item", "parse_has_error": False,
                    "declaration": {"start_line": 1, "end_line": 1},
                    "qualified_name": f"unit{n}", "name": f"unit{n}",
                }]},
            })
        fixture = {"commit": "a" * 40,
                   "syntax": {"extractor_version": "briefing-lab-rust-v1"}, "files": files}
        child = """
import json, sys
from unittest.mock import patch
sys.path.insert(0, sys.argv[1])
import prepare
index, issue = json.load(sys.stdin)
with patch.object(prepare, 'source_texts', side_effect=lambda repo, rev, entries: [entry['text'] for entry in entries]):
    rows, coverage = prepare.candidates(None, index['commit'], index, issue)
print(json.dumps([rows, coverage], sort_keys=True))
"""
        outputs = []
        for seed in [1, 2, 3, 4]:
            outputs.append(subprocess.check_output(
                [sys.executable, "-c", child, str(Path(prepare.__file__).parent)],
                input=json.dumps([fixture, {"title": " ".join(query)}]).encode(),
                env={**os.environ, "PYTHONHASHSEED": str(seed)},
            ))
        self.assertEqual(len(set(outputs)), 1, "Candidate identities and scores must be reproducible")

    def test_render_preserves_source_bytes_with_crlf_and_trailing_spaces(self):
        source = "/// A source comment.  \r\nfn one() { }\t \r\n"
        payload, selected = prepare.render("a" * 40, [row("c01", 1, source)])
        self.assertEqual(selected, ["c01"])
        rendered_source = payload.split("```rust\n", 1)[1].split("```\n", 1)[0]
        self.assertEqual(rendered_source.encode(), source.encode())
        # A Markdown separator is outside the source when it has no final newline.
        source = "fn one() { }  "
        payload, _ = prepare.render("a" * 40, [row("c01", 1, source)])
        self.assertIn("```rust\n" + source + "\n```", payload)

    def test_cross_origin_redirect_cannot_forward_authorization(self):
        request = urllib.request.Request(
            "https://api.typesafe.ai/v1/systemone", data=b"{}",
            headers={"Authorization": "Bearer synthetic-credential"})
        for status in [301, 302, 303, 307, 308]:
            with self.subTest(status=status), self.assertRaises(urllib.error.HTTPError):
                prepare.NoRedirect().redirect_request(
                    request, None, status, "redirect", {}, "https://other.example/collect")

    def test_invalid_response_containers_fall_back_without_network_retry(self):
        rows = [row("c01", 1, "fn one() {}")]
        responses = [[], None, {"usage": None},
                     {"model": prepare.MODEL, "usage": {}, "answers": []},
                     {"model": prepare.MODEL, "usage": {}, "answers": {"c01": []}}]
        for value in responses:
            with self.subTest(value=value), tempfile.TemporaryDirectory() as directory:
                response = io.BytesIO(json.dumps(value).encode())
                response.headers = {}
                with patch.dict("os.environ", {"TYPESAFE_API_KEY": "synthetic-credential"}), \
                        patch("urllib.request.build_opener") as opener:
                    opener.return_value.open.return_value = response
                    selected, receipt = prepare.system_one(
                        {"title": "Synthetic check"}, rows, Path(directory))
                    self.assertEqual(opener.return_value.open.call_count, 1)
                self.assertEqual(selected, {})
                self.assertEqual(receipt["selection"], "deterministic_fallback")
                self.assertIsNone(receipt["cost_usd"])
                self.assertEqual(receipt["attempts"], 1)
                self.assertTrue((Path(directory) / "jev-call.json").is_file())

    def test_oversized_response_is_bounded_and_remains_unknown_cost(self):
        with tempfile.TemporaryDirectory() as directory:
            response = io.BytesIO(b" " * (1024 * 1024 + 2))
            response.headers = {}
            with patch.dict("os.environ", {"TYPESAFE_API_KEY": "synthetic-credential"}), \
                    patch("urllib.request.build_opener") as opener:
                opener.return_value.open.return_value = response
                selected, receipt = prepare.system_one(
                    {"title": "Synthetic check"}, [row("c01", 1, "fn one() {}")], Path(directory))
            self.assertEqual(selected, {})
            self.assertEqual(receipt["selection"], "deterministic_fallback")
            self.assertIsNone(receipt["cost_usd"])
            self.assertFalse((Path(directory) / "jev-response.json").exists())

    def test_scores_reject_inconsistent_expectations_and_boolean_numbers(self):
        rows = [row("c01", 1, "fn one() {}")]
        malformed = [
            {"type": "score", "score": 3, "probabilities": {"0": 1, "1": 0, "2": 0, "3": 0}},
            {"type": "score", "score": True, "probabilities": {"0": 0, "1": 1, "2": 0, "3": 0}},
            {"type": "score", "score": 1, "probabilities": {"0": False, "1": True, "2": False, "3": False}},
            {"type": "score", "score": 1, "probabilities": []},
        ]
        for answer in malformed:
            with self.subTest(answer=answer), self.assertRaises(ValueError):
                prepare.scores({"model": prepare.MODEL, "answers": {"c01": answer}}, rows)

    def test_committed_symlink_cannot_be_a_source_unit(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            (root / "link.rs").symlink_to("outside.rs")
            subprocess.run(["git", "-C", str(root), "add", "link.rs"], check=True)
            subprocess.run(["git", "-C", str(root), "-c", "user.name=Test", "-c", "user.email=test@example.test", "commit", "-qm", "Fixture"], check=True)
            rev = prepare.git(root, "rev-parse", "HEAD").decode().strip()
            raw = b"outside.rs"
            entry = {"path": "link.rs", "blob": prepare.git(root, "rev-parse", rev + ":link.rs").decode().strip(),
                     "size": len(raw), "sha256": prepare.digest(raw)}
            with self.assertRaisesRegex(ValueError, "regular file"):
                prepare.source_text(root, rev, entry)


if __name__ == "__main__":
    unittest.main()
