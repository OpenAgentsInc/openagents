#!/usr/bin/env python3
"""Check the run record a gate leaves behind."""
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

RECORD = Path(__file__).resolve().parent.parent / "gate-record.py"


def call(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, str(RECORD), *args],
        capture_output=True, text=True)


class RecordTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.dir = self.tmp.name

    def tearDown(self):
        self.tmp.cleanup()

    def begin(self) -> str:
        out = call("begin", "--dir", self.dir, "--root", self.dir,
                   "--requested", '{"phases":"all"}')
        self.assertEqual(out.returncode, 0, out.stderr)
        return json.loads(out.stdout)["run_id"]

    def read(self, run_id: str) -> dict:
        return json.loads(
            (Path(self.dir) / run_id / "run.json").read_text())

    def test_a_run_record_accumulates_phases_and_closes(self):
        run_id = self.begin()
        out = call("phase", "--dir", self.dir, "--run-id", run_id,
                   "--slug", "fmt", "--name", "Workspace formatting",
                   "--status", "passed", "--exit", "0", "--elapsed", "0.6",
                   "--command", '["cargo","fmt","--all","--check"]')
        self.assertEqual(out.returncode, 0, out.stderr)
        out = call("skip", "--dir", self.dir, "--run-id", run_id,
                   "--slug", "soak", "--reason", "use --with-soak")
        self.assertEqual(out.returncode, 0, out.stderr)
        out = call("finish", "--dir", self.dir, "--run-id", run_id,
                   "--result", "partial")
        self.assertEqual(out.returncode, 0, out.stderr)
        record = self.read(run_id)
        self.assertEqual(record["schema"], "openagents.gate-run.v1")
        self.assertEqual(record["run_id"], run_id)
        self.assertEqual(record["requested"], {"phases": "all"})
        self.assertEqual(record["phases"][0]["slug"], "fmt")
        self.assertEqual(record["phases"][0]["exit"], 0)
        self.assertEqual(record["skipped"][0]["slug"], "soak")
        self.assertEqual(record["result"], "partial")
        self.assertIsNotNone(record["finished_utc"])

    def test_a_missing_record_refuses_a_phase_write(self):
        out = call("phase", "--dir", self.dir, "--run-id", "never-begun",
                   "--slug", "fmt", "--name", "x", "--status", "passed")
        self.assertNotEqual(out.returncode, 0)
        self.assertIn("no gate record", out.stderr)

    def test_begin_binds_the_record_to_the_tree(self):
        run_id = self.begin()
        record = self.read(run_id)
        tree = record["tree"]
        self.assertIn("head", tree)
        self.assertIn("dirty", tree)
        self.assertIn("diff_digest", tree)


if __name__ == "__main__":
    unittest.main()
