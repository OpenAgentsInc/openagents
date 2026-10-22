"""Prevent incomplete, changed, or misaligned inputs from producing final cards."""

import argparse
import copy
import hashlib
import json
import struct
import tempfile
import unittest
from pathlib import Path

import numpy as np

import consumer_q_card as checker
from consumer_q_card import build_card, captured, log_softmax, sha256, validate_request


def write_json(path, value):
    path.write_text(json.dumps(value))


class CardInputs(unittest.TestCase):
    def test_capture_prefix_must_be_recorded_prefill_and_answers_have_no_gaps(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder)
            with self.assertRaises(ValueError):
                captured(path)
            np.array([1, 2], dtype=np.float32).tofile(path / "step-1.logits.f32")
            write_json(path / "step-1.json", {"emit": True})
            with self.assertRaises(ValueError):
                captured(path)
            write_json(path / "step-0.json", {"emit": False})
            self.assertEqual(len(captured(path)), 1)
            write_json(path / "step-0.json", {"emit": True})
            with self.assertRaises(ValueError):
                captured(path)
            np.array([2, 3], dtype=np.float32).tofile(path / "step-0.logits.f32")
            self.assertEqual(len(captured(path)), 2)
            np.array([3, 4], dtype=np.float32).tofile(path / "step-3.logits.f32")
            with self.assertRaises(ValueError):
                captured(path)

    def test_nonfinite_and_empty_logits_are_rejected(self):
        for values in ([], [float("nan")], [float("inf")], [float("-inf")]):
            with self.assertRaises(ValueError):
                log_softmax(np.array(values))
        np.testing.assert_allclose(np.exp(log_softmax(np.array([1000.0, 1000.0]))), [0.5, 0.5])

    def test_prompt_forced_history_and_requested_limit_must_match(self):
        oracle = {"prompt_token_ids": [1, 2], "generated_ids": [3, 4]}
        request = {"prompt_ids": [1, 2], "force_ids": [3, 4], "max_tokens": 2}
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "request.json"
            write_json(path, request)
            validate_request(folder, oracle, 2)
            for change in ({"prompt_ids": [1, 5]}, {"force_ids": [4, 3]},
                           {"force_ids": None}, {"max_tokens": 1}, {"max_tokens": None}):
                write_json(path, request | change)
                with self.assertRaises(ValueError):
                    validate_request(folder, oracle, 2)


class FrozenCard(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.oracle = self.root / "oracle"
        self.case = "case"
        (self.oracle / self.case).mkdir(parents=True)
        self.vectors = [np.array(values, dtype="<f4") for values in
                        ([8, 0, -8], [0, 8, -8], [-8, 0, 8])]
        header, data, tensors = {}, b"", []
        for label, vector in zip(("last", "g1", "g2"), self.vectors):
            name = f"case.decode.{label}.logits"
            raw = vector.tobytes()
            header[name] = {"dtype": "F32", "shape": [3], "data_offsets": [len(data), len(data) + len(raw)]}
            tensors.append({"name": name, "file": "case/tensors-rank0.safetensors", "shape": [3],
                            "dtype": "float32", "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()})
            data += raw
        encoded = json.dumps(header).encode()
        self.tensor_path = self.oracle / tensors[0]["file"]
        self.tensor_path.write_bytes(struct.pack("<Q", len(encoded)) + encoded + data)
        write_json(self.oracle / "manifest.json", {"revision": checker.REVISION, "tensors": tensors})
        write_json(self.oracle / "holdout-corpus.json", {"cases": [{"id": "case", "n_gen": 2}]})
        write_json(self.oracle / "case/run.json", {
            "id": "case", "n_gen": 2, "prompt_token_ids": [2], "generated_ids": [0, 1],
            "steps": [{"position": k, "input_token": token, "argmax": k,
                       "logits_sha256": tensors[k]["sha256"]} for k, token in enumerate((2, 0))],
        })
        self.checkpoint = "a" * 64
        self.artifact = self.root / "artifact.json"
        write_json(self.artifact, {"schema": "openagents.deepseek_v41.consumer_q.v1", "profile": "consumer-q",
                                  "lossy": True, "qualified": False,
                                  "source": {"repo": checker.REPO, "revision": checker.REVISION}})
        self.evaluation = self.root / "evaluation.json"
        write_json(self.evaluation, {"schema": "openagents.deepseek_v41.consumer_q_evaluation.v1",
                                    "checkpoint_index_sha256": self.checkpoint,
                                    "oracle_manifest_sha256": sha256(self.oracle / "manifest.json"),
                                    "oracle_corpus_sha256": sha256(self.oracle / "holdout-corpus.json"),
                                    "cases": [{"id": "case", "positions": 2}]})
        self.folders = {}
        for label in ("exact", "consumer-q"):
            folder = self.root / label
            folder.mkdir()
            self.folders[label] = folder
            write_json(folder / "manifest.json", {"index_sha256": self.checkpoint})
            write_json(folder / "request.json", {"prompt_ids": [2], "force_ids": [0, 1], "max_tokens": 2,
                                                "runtime": "native" if label == "exact" else "consumer-q-fast",
                                                "checkpoint_index_sha256": self.checkpoint,
                                                "consumer_q_manifest_sha256": None if label == "exact" else sha256(self.artifact)})
            for k, vector in enumerate(self.vectors[:2]):
                vector.tofile(folder / f"step-{k}.logits.f32")
                write_json(folder / f"step-{k}.json", {"position": k, "tokens": 1, "emit": True})
                (folder / f"step-{k}.msg").write_bytes(bytes([2 if k == 0 else 0]))
        self.args = argparse.Namespace(final=True, evaluation=self.evaluation,
                                       evaluation_sha256=sha256(self.evaluation),
                                       checker_sha256=sha256(Path(checker.__file__)),
                                       artifact_manifest=self.artifact, oracle_dir=self.oracle,
                                       specs=[f"case={folder},{label}" for label, folder in self.folders.items()])

    def test_first_logits_predict_token_zero_before_forced_continuation(self):
        card = build_card(self.args)
        exact = [row for row in card["rows"] if row["label"] == "exact"]
        self.assertEqual([row["oracle_tensor"] for row in exact], ["case.decode.last.logits", "case.decode.g1.logits"])
        self.assertEqual([(r["input_token"], r["reference_token"]) for r in exact], [(2, 0), (0, 1)])
        self.assertEqual(card["summary"]["exact"]["kl_mean"], 0)
        self.assertEqual(card["summary"]["exact"]["top1_agreement"], 1)
        self.assertTrue(card["final"])
        self.assertFalse(card["partial"])
        self.assertEqual(card["captures"][0]["files"]["step-0.msg"], sha256(self.folders["exact"] / "step-0.msg"))
        self.assertIn("case/run.json", card["oracle_files"])
        self.assertEqual(set(card["oracle_tensors"]), {"case.decode.last.logits", "case.decode.g1.logits"})

    def test_changed_frozen_inputs_or_checker_pin_are_refused(self):
        for path in (self.evaluation, self.oracle / "manifest.json", self.oracle / "holdout-corpus.json"):
            original = path.read_bytes()
            path.write_bytes(original + b" ")
            with self.subTest(path=path), self.assertRaisesRegex(ValueError, "SHA-256"):
                build_card(self.args)
            path.write_bytes(original)
        args = copy.copy(self.args)
        args.checker_sha256 = "b" * 64
        with self.assertRaisesRegex(ValueError, "SHA-256"):
            build_card(args)

    def test_missing_case_duplicate_label_and_shorter_capture_are_refused(self):
        for specs in (self.args.specs[:1], [self.args.specs[0]] * 2):
            args = copy.copy(self.args)
            args.specs = specs
            with self.assertRaises(ValueError):
                build_card(args)
        folder = self.folders["consumer-q"]
        (folder / "step-1.logits.f32").unlink()
        (folder / "step-1.json").unlink()
        with self.assertRaisesRegex(ValueError, "frozen case"):
            build_card(self.args)

    def test_stale_extra_logits_cannot_exceed_a_shorter_request(self):
        folder = self.folders["exact"]
        self.vectors[2].tofile(folder / "step-2.logits.f32")
        write_json(folder / "step-2.json", {"position": 2, "tokens": 1, "emit": True})
        with self.assertRaisesRegex(ValueError, "request.max_tokens"):
            build_card(self.args)

    def test_labels_cannot_claim_the_wrong_artifact_or_checkpoint(self):
        for label, changes in (("exact", {"consumer_q_manifest_sha256": sha256(self.artifact)}),
                               ("consumer-q", {"consumer_q_manifest_sha256": "b" * 64}),
                               ("consumer-q", {"checkpoint_index_sha256": "b" * 64})):
            path = self.folders[label] / "request.json"
            original = path.read_bytes()
            write_json(path, json.loads(original) | changes)
            with self.subTest(changes=changes), self.assertRaisesRegex(ValueError, "declared label"):
                build_card(self.args)
            path.write_bytes(original)
        write_json(self.folders["exact"] / "manifest.json", {"index_sha256": "b" * 64})
        with self.assertRaisesRegex(ValueError, "capture manifest"):
            build_card(self.args)

    def test_changed_oracle_tensor_or_native_position_is_refused(self):
        original = self.tensor_path.read_bytes()
        self.tensor_path.write_bytes(original[:-25] + b"\x01" + original[-24:])
        with self.assertRaisesRegex(ValueError, "oracle tensor"):
            build_card(self.args)
        self.tensor_path.write_bytes(original)
        write_json(self.folders["exact"] / "step-0.json", {"position": 1, "tokens": 1, "emit": True})
        with self.assertRaisesRegex(ValueError, "token position"):
            build_card(self.args)

    def test_unfrozen_runs_are_explicitly_partial_diagnostics(self):
        args = copy.copy(self.args)
        args.final = False
        args.specs = args.specs[:1]
        card = build_card(args)
        self.assertFalse(card["final"])
        self.assertTrue(card["diagnostic"])
        self.assertTrue(card["partial"])


if __name__ == "__main__":
    unittest.main()
