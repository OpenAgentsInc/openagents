#!/usr/bin/env python3
"""Selector and stop-proof tests for the Python managed-sandbox drivers."""

from __future__ import annotations

import importlib.util
import os
import sys
import tempfile
import unittest
from pathlib import Path


HERE = Path(__file__).resolve().parent


def load(name: str, filename: str):
    spec = importlib.util.spec_from_file_location(name, HERE / filename)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


interrupt = load("guest_interrupt", "managed-sandbox-guest-interrupt.py")
forensic = load("forensic_worker", "forensic-worker-driver.py")


def observing(processes, process_groups=None, inaccessible=0):
    process_groups = [] if process_groups is None else process_groups

    def _observe(_roots):
        return {
            "supported": True,
            "inaccessible": inaccessible,
            "processes": processes,
            "processGroups": process_groups,
        }

    return _observe


class InterruptSelectors(unittest.TestCase):
    def test_escapee_is_attributed_to_the_turn(self) -> None:
        processes = [
            {"pid": 100, "processGroupId": 100, "sessionId": 100},
            {"pid": 101, "processGroupId": 100, "sessionId": 100},
            {"pid": 102, "processGroupId": 102, "sessionId": 100},
            {"pid": 200, "processGroupId": 200, "sessionId": 200},
        ]
        sessions = interrupt.sessions_in(processes, 100)
        self.assertEqual(sessions, [100])
        escaped = interrupt.escaped_descendants_in(processes, 100, sessions, 999)
        self.assertEqual([item["pid"] for item in escaped], [102])

    def test_observer_and_session_zero_are_excluded(self) -> None:
        processes = [
            {"pid": 100, "processGroupId": 100, "sessionId": 100},
            {"pid": 555, "processGroupId": 555, "sessionId": 100},
            {"pid": 300, "processGroupId": 300, "sessionId": 0},
        ]
        escaped = interrupt.escaped_descendants_in(
            processes, 100, interrupt.sessions_in(processes, 100), 555
        )
        self.assertEqual(escaped, [])

    def test_intact_group_has_no_escapees(self) -> None:
        processes = [
            {"pid": 100, "processGroupId": 100, "sessionId": 100},
            {"pid": 101, "processGroupId": 100, "sessionId": 100},
        ]
        escaped = interrupt.escaped_descendants_in(
            processes, 100, interrupt.sessions_in(processes, 100), 999
        )
        self.assertEqual(escaped, [])


class ForensicProofs(unittest.TestCase):
    def test_stop_removes_scratch_roots(self) -> None:
        with tempfile.TemporaryDirectory(prefix="oa-forensic-stop-") as root:
            roots = {
                "turn_root": str(Path(root) / "turns"),
                "source_root": str(Path(root) / "source"),
                "artifact_path": str(Path(root) / "forensic-artifact.tar.zst"),
                "runtime_root": str(Path(root) / "run"),
            }
            Path(roots["turn_root"], "turn-fixture").mkdir(parents=True)
            Path(roots["turn_root"], "io-fixture").mkdir(parents=True)
            Path(roots["turn_root"], "turn-fixture", "pid").write_text("2147483647")
            Path(roots["source_root"]).mkdir()
            Path(roots["runtime_root"]).mkdir()
            Path(roots["artifact_path"]).write_text("artifact")
            proof = forensic.prepare_stop_at(
                **roots, observe_guarded_processes=observing([])
            )
            self.assertEqual(
                proof,
                {
                    "schema": "openagents.forensic_worker_prepare_stop.v1",
                    "driverRef": "driver.openagents.forensic-worker.v1",
                    "processObservation": "proc",
                    "zeroProcess": True,
                    "zeroScratch": True,
                    "activeProcessGroups": 0,
                    "scratchPathsRemaining": 0,
                },
            )
            for path in roots.values():
                self.assertFalse(Path(path).exists())

    def test_live_process_refuses_stop(self) -> None:
        with tempfile.TemporaryDirectory(prefix="oa-forensic-guarded-") as root:
            roots = {
                "turn_root": str(Path(root) / "turns"),
                "source_root": str(Path(root) / "source"),
                "artifact_path": str(Path(root) / "forensic-artifact.tar.zst"),
                "runtime_root": str(Path(root) / "run"),
            }
            Path(roots["turn_root"]).mkdir()
            Path(roots["source_root"]).mkdir()
            Path(roots["runtime_root"]).mkdir()
            Path(roots["artifact_path"]).write_text("artifact")
            with self.assertRaises(SystemExit) as raised:
                forensic.prepare_stop_at(
                    **roots,
                    observe_guarded_processes=observing([4242], [4242]),
                )
            self.assertEqual(raised.exception.code, 1)
            self.assertTrue(Path(roots["source_root"]).exists())

    def test_missing_identity_cannot_prove_zero(self) -> None:
        with tempfile.TemporaryDirectory(prefix="oa-forensic-identity-") as root:
            missing = Path(root) / "turn-missing"
            malformed = Path(root) / "turn-malformed"
            io_scratch = Path(root) / "io-scratch"
            missing.mkdir()
            malformed.mkdir()
            io_scratch.mkdir()
            (malformed / "pgid").write_text("not-a-process-group")
            self.assertTrue(forensic.workload_is_live_at(str(missing)))
            self.assertTrue(forensic.workload_is_live_at(str(malformed)))
            self.assertFalse(forensic.workload_is_live_at(str(io_scratch)))

    def test_observation_off_linux(self) -> None:
        observation = forensic.observe_guarded_processes_at(
            ["/var/lib/openagents/managed-sandbox-turns"]
        )
        if sys.platform != "linux":
            self.assertFalse(observation["supported"])
            self.assertEqual(observation["processes"], [])
        else:
            self.assertTrue(observation["supported"])

    def test_io_and_phase2_drivers_refuse_a_wrong_flag(self) -> None:
        for name in (
            "managed-sandbox-io-driver.py",
            "managed-sandbox-turn-driver.py",
            "managed-sandbox-phase2-driver.py",
        ):
            completed = os.system(f"{sys.executable} {HERE / name} --wrong-flag </dev/null")
            self.assertNotEqual(completed, 0, name)


if __name__ == "__main__":
    unittest.main()
