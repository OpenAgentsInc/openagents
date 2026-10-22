"""Prevent incomplete or misaligned captures from producing a quality card."""

import tempfile
import unittest
from pathlib import Path

import numpy as np

from consumer_q_card import captured, log_softmax


class CardInputs(unittest.TestCase):
    def test_capture_prefix_must_be_recorded_prefill_and_answers_have_no_gaps(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder)
            with self.assertRaises(ValueError):
                captured(path)
            np.array([1, 2], dtype=np.float32).tofile(path / "step-1.logits.f32")
            with self.assertRaises(ValueError):
                captured(path)
            (path / "step-0.json").write_text('{"emit": false}')
            self.assertEqual(len(captured(path)), 1)
            (path / "step-0.json").write_text('{"emit": true}')
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


if __name__ == "__main__":
    unittest.main()
