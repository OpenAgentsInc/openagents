#!/usr/bin/env python3
"""Check run activity hooks with an isolated home and process table."""

from __future__ import annotations

import importlib.util
import io
import json
import os
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
import unittest
from unittest.mock import patch


SCRIPT = Path(__file__).resolve().parents[1] / "dev" / "agent-run.py"


class RunActivityTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory(dir=os.environ.get("OPENAGENTS_SCRATCH"))
        self.addCleanup(self.temp.cleanup)
        self.home = Path(self.temp.name)
        self.env = patch.dict(os.environ, {
            "HOME": str(self.home),
            "OA_RUNS_DIR": str(self.home / "runs"),
            "OA_RUN_REAP": "off",
            "OA_RUN_HOOKS": "on",
        })
        self.env.start()
        self.addCleanup(self.env.stop)
        for name in ("OA_RUN_ID", "CLAUDE_PROJECT_DIR", "CLAUDE_ENV_FILE"):
            os.environ.pop(name, None)
        spec = importlib.util.spec_from_file_location("agent_run_activity_test", SCRIPT)
        self.run = importlib.util.module_from_spec(spec)
        sys.modules[spec.name] = self.run
        self.addCleanup(sys.modules.pop, spec.name, None)
        spec.loader.exec_module(self.run)
        self.procs = {4711: self.run.Proc(4711, 1, 4711, "??", 1000, "claude")}
        self.rid = self.run.session_run_id(self.procs, 4711)

    def hook(self, event: str, payload: dict, now: int) -> None:
        with patch.object(self.run, "ps", return_value=self.procs), \
                patch.object(self.run, "claude_pid", return_value=4711), \
                patch.object(self.run.time, "time", return_value=now), \
                patch.object(sys, "stdin", io.StringIO(json.dumps(payload))):
            self.assertEqual(self.run.hook(event), 0)

    def test_create_records_initial_activity_and_workspace(self) -> None:
        with patch.object(self.run.time, "time", return_value=2000):
            meta = self.run.create("coder", 4711, procs=self.procs)
        self.assertEqual(meta["created"], 2000)
        self.assertEqual(meta["last_activity"], 2000)
        self.assertEqual(meta["workspace_root"], str(Path.cwd()))

    def test_session_start_records_harness_context(self) -> None:
        workspace = str(self.home / "project")
        transcript = str(self.home / "session.jsonl")
        self.hook("session-start", {
            "session_id": "session-1", "cwd": workspace, "transcript_path": transcript,
        }, 2000)
        meta = self.run.load(self.rid)
        self.assertEqual(meta["sessions"], ["session-1"])
        self.assertEqual(meta["workspace_root"], workspace)
        self.assertEqual(meta["transcript_path"], transcript)
        self.assertEqual(meta["last_activity"], 2000)

    def test_foreground_bash_updates_activity_without_recording_children(self) -> None:
        self.hook("session-start", {"session_id": "session-1"}, 2000)
        with patch.object(self.run, "descendants") as descendants:
            self.hook("bash-post", {"tool_input": {"command": "cargo test"}}, 2300)
        descendants.assert_not_called()
        self.assertEqual(self.run.load(self.rid)["last_activity"], 2300)
        self.assertEqual(self.run.load(self.rid)["sessions"], ["session-1"])

    def test_clear_updates_activity_without_ending_the_run(self) -> None:
        self.hook("session-start", {"session_id": "session-1"}, 2000)
        with patch.object(self.run, "reap") as reap:
            self.hook("session-end", {"reason": "clear"}, 2300)
        reap.assert_not_called()
        meta = self.run.load(self.rid)
        self.assertEqual(meta["last_activity"], 2300)
        self.assertIsNone(meta["ended"])

    def test_resuming_reopens_run_and_preserves_transcript(self) -> None:
        self.hook("session-start", {
            "session_id": "session-1", "transcript_path": "/fake/session.jsonl",
        }, 2000)
        meta = self.run.load(self.rid)
        meta["ended"] = 2100
        self.run.save(meta)
        self.hook("session-start", {"session_id": "session-1"}, 2300)
        meta = self.run.load(self.rid)
        self.assertIsNone(meta["ended"])
        self.assertEqual(meta["transcript_path"], "/fake/session.jsonl")
        self.assertEqual(meta["sessions"], ["session-1"])
        self.assertEqual(meta["last_activity"], 2300)

    def test_loading_run_does_not_refresh_activity(self) -> None:
        self.hook("session-start", {}, 2000)
        with patch.object(self.run.time, "time", return_value=3000):
            self.assertEqual(self.run.load(self.rid)["last_activity"], 2000)
            self.assertEqual(self.run.all_runs()[0]["last_activity"], 2000)

    def test_concurrent_saves_use_distinct_temporary_files(self) -> None:
        barrier = threading.Barrier(2)
        temporary_paths = []
        replace = Path.replace

        def replace_together(path: Path, target: Path) -> Path:
            temporary_paths.append(path)
            barrier.wait(timeout=5)
            return replace(path, target)

        records = [{"id": "concurrent", "last_activity": value} for value in (2000, 2300)]
        with patch.object(Path, "replace", replace_together), ThreadPoolExecutor(2) as pool:
            futures = [pool.submit(self.run.save, record) for record in records]
            for future in futures:
                future.result()
        self.assertEqual(len(set(temporary_paths)), 2)
        self.assertIn(self.run.load("concurrent"), records)
        self.assertEqual(
            [path.name for path in self.run.run_dir("concurrent").iterdir()], ["run.json"],
        )

    def test_failed_save_cleans_up_its_temporary_file(self) -> None:
        with patch.object(Path, "replace", side_effect=OSError("fixture failure")):
            with self.assertRaises(OSError):
                self.run.save({"id": "failed"})
        self.assertEqual(list(self.run.run_dir("failed").iterdir()), [])

    def test_shell_hook_forwards_foreground_bash_activity(self) -> None:
        fixture = self.home / "processes.py"
        fixture.write_text('print("4711 1 4711 ?? Thu Jan 01 00:16:40 1970 claude")\n')
        env = dict(os.environ, OA_PS_CMD=f"{sys.executable} {fixture}", CLAUDE_PID="4711")
        wrapper = SCRIPT.with_name("agent-run-hook.sh")
        subprocess.run(["sh", str(wrapper), "session-start"], input="{}", text=True,
                       env=env, check=True, capture_output=True)
        meta = self.run.all_runs()[0]
        meta["last_activity"] = 1
        self.run.save(meta)
        subprocess.run(["sh", str(wrapper), "bash-post"],
                       input='{"tool_input":{"command":"true"}}', text=True,
                       env=env, check=True, capture_output=True)
        self.assertGreater(self.run.load(meta["id"])["last_activity"], 1)


if __name__ == "__main__":
    unittest.main()
