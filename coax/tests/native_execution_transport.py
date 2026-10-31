#!/usr/bin/env python3
"""Small host transport checks; no policy compilation or native execution."""
from pathlib import Path
import sys
import unittest
from unittest.mock import patch


TOOLS = Path(__file__).resolve().parents[1] / "tools"
sys.path.insert(0, str(TOOLS))
import native_check_execution as execution
from native_execution_policy import process_fact


class PhaseSpy:
    def __init__(self):
        self.observations = []

    def phase(self, observation):
        self.observations.append(observation)
        return 0

    def integrity(self, expected, observed):
        return 0


class TransportShapeTests(unittest.TestCase):
    def test_missing_or_nondict_process_is_explicitly_nonqualifying(self):
        for observation in (None, [], "", b"", 0, False):
            with self.subTest(observation=observation):
                fact = process_fact(observation)
                self.assertFalse(fact.launched)
                self.assertFalse(fact.finished)
                self.assertFalse(fact.stdout_eof)
                self.assertFalse(fact.stderr_eof)
                self.assertNotEqual(fact.read_errno, 0)

    def chain(self, platform, version="compiler-version", target="compiler-target"):
        return {"version_observation": version, "target_observation": target,
                "program_sha256": "observed-digest", "resolved_program": "/observed-cc",
                "sdk": {"platform": platform, "path_observation": None,
                        "version_observation": None}}

    def phases(self, chain):
        spy = PhaseSpy()
        with patch.object(execution, "_sha", return_value="observed-digest"):
            execution._toolchain_outcomes(spy, chain)
        return spy.observations

    def test_nondarwin_skips_only_absent_optional_sdk_queries(self):
        self.assertEqual(self.phases(self.chain("Linux")),
                         ["compiler-version", "compiler-target"])
        chain = self.chain("Linux")
        chain["sdk"]["path_observation"] = "observed-sdk-path"
        self.assertEqual(self.phases(chain),
                         ["compiler-version", "compiler-target", "observed-sdk-path"])

    def test_darwin_keeps_both_required_missing_sdk_observations(self):
        self.assertEqual(self.phases(self.chain("Darwin")),
                         ["compiler-version", "compiler-target", None, None])

    def test_missing_required_compiler_observations_are_not_skipped(self):
        self.assertEqual(self.phases(self.chain("Linux", version=None, target=None)),
                         [None, None])


if __name__ == "__main__":
    unittest.main()
