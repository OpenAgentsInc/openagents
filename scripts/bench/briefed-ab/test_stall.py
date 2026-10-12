#!/usr/bin/env python3
"""Tests for the bench stall detector (#11433).

    python3 -m pytest -q scripts/bench/briefed-ab/test_stall.py
"""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import stall  # noqa: E402


class Clock:
    def __init__(self, t: float):
        self.t = t

    def __call__(self) -> float:
        return self.t


class StallWatchTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.tag = Path(self.tmp.name)
        self.clock = Clock(time.time())
        self.fired: list[str] = []

    def tearDown(self):
        self.tmp.cleanup()

    def watch(self, minutes: float) -> stall.StallWatch:
        return stall.StallWatch(self.tag, minutes, clock=self.clock, on_stall=self.fired.append)

    def row(self, name: str, at: float) -> None:
        d = self.tag / f"1-A-{len(list(self.tag.iterdir()))}"
        d.mkdir()
        (d / name).write_text("{}")
        os.utime(d / name, (at, at))

    def test_fires_after_the_interval_without_a_row(self):
        w = self.watch(30)
        self.clock.t += 29 * 60
        self.assertFalse(w.check())
        self.clock.t += 61
        self.assertTrue(w.check())
        self.assertEqual(len(self.fired), 1)
        self.assertIn("no result row in 30 minutes", self.fired[0])

    def test_a_new_row_resets_the_interval(self):
        w = self.watch(30)
        self.clock.t += 25 * 60
        self.row("result.json", self.clock.t)
        self.clock.t += 25 * 60
        self.assertFalse(w.check())
        self.row("error.txt", self.clock.t)  # a harness error is progress too
        self.clock.t += 29 * 60
        self.assertFalse(w.check())
        self.clock.t += 2 * 60
        self.assertTrue(w.check())

    def test_rows_older_than_the_start_do_not_count(self):
        self.row("result.json", self.clock.t - 10 * 3600)  # a resumed tag's old rows
        w = self.watch(30)
        self.clock.t += 31 * 60
        self.assertTrue(w.check())

    def test_other_files_are_not_progress(self):
        w = self.watch(30)
        self.clock.t += 20 * 60
        self.row("events.jsonl", self.clock.t)  # a trial still streaming
        self.clock.t += 11 * 60
        self.assertTrue(w.check())

    def test_zero_turns_it_off(self):
        w = self.watch(0)
        self.clock.t += 100 * 3600
        self.assertFalse(w.check())

    def test_the_interval_comes_from_flag_then_plan_then_env(self):
        old = os.environ.pop("AB_STALL_MIN", None)
        try:
            self.assertEqual(stall.stall_minutes(None, None), stall.DEFAULT_MIN)
            os.environ["AB_STALL_MIN"] = "45"
            self.assertEqual(stall.stall_minutes(None, None), 45)
            self.assertEqual(stall.stall_minutes(None, {"stall_min": 90}), 90)
            self.assertEqual(stall.stall_minutes(10, {"stall_min": 90}), 10)
        finally:
            os.environ.pop("AB_STALL_MIN", None)
            if old is not None:
                os.environ["AB_STALL_MIN"] = old

    def test_the_thread_aborts_a_real_stalled_process(self):
        # A child process that never writes a row, with its own sleeping
        # child: the watch kills the tree and exits 3.
        code = (
            "import sys, subprocess, time; sys.path.insert(0, sys.argv[1]); import stall\n"
            "kid = subprocess.Popen(['sleep', '300']); print(kid.pid, flush=True)\n"
            "stall.StallWatch(sys.argv[2], 0.02).start(every=0.2)\n"
            "time.sleep(30)\n"
        )
        t0 = time.time()
        proc = subprocess.run([sys.executable, "-c", code, str(HERE), str(self.tag)],
                              capture_output=True, text=True, timeout=25)
        self.assertEqual(proc.returncode, stall.STALL_EXIT, proc.stderr)
        self.assertLess(time.time() - t0, 20)
        self.assertIn("stall: no result row", proc.stdout)
        self.assertTrue((self.tag / "stalled.txt").exists())
        kid = int(proc.stdout.split()[0])
        time.sleep(0.2)
        with self.assertRaises(ProcessLookupError):
            os.kill(kid, 0)


if __name__ == "__main__":
    unittest.main()
