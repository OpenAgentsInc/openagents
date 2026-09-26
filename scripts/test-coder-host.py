#!/usr/bin/env python3
"""Acceptance fixtures for installation infrastructure; no product services."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import plistlib
import subprocess
import sys
import tempfile
import time
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location("coder_host", Path(__file__).with_name("coder-host.py"))
host = importlib.util.module_from_spec(spec)
spec.loader.exec_module(host)


class Packaging(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.base = Path(self.temp.name)
        self.tasks = self.base / "tasks"
        self.tasks.mkdir(mode=0o700)
        (self.tasks / "tasks.json").write_text(json.dumps({"schema": host.STATE, "evidence": "retain me"}))
        self.installation = host.Installation(self.base / "host", create=True)

    def tearDown(self):
        self.installation.lock.close()
        self.temp.cleanup()

    def binary(self, name, fail=False):
        path = self.base / name
        path.write_text("#!/bin/sh\n" + ("exit 1\n" if fail else f"# {name}\nprintf 'coder fixture; task execute\\n'\n"))
        path.chmod(0o700)
        return path

    def args(self, binary, schemas=None):
        return argparse.Namespace(binary=str(binary), sha256=host.digest(binary), source_revision="a" * 40,
                                  read_state_schema=schemas, uncommitted_source=True, tasks=str(self.tasks))

    def test_install_upgrade_duplicate_rollback_preserves_data(self):
        first = self.args(self.binary("one"))
        second = self.args(self.binary("two"))
        self.assertEqual(host.install(self.installation, first)["result"], "activated")
        before = self.installation.document.copy()
        self.assertEqual(host.install(self.installation, first)["result"], "unchanged")
        self.assertEqual(before, self.installation.document)
        host.install(self.installation, second)
        self.assertEqual(self.installation.document["previous"], first.sha256)
        self.installation.select(first.sha256, self.tasks, "rollback")
        self.assertEqual(host.read_json(self.tasks / "tasks.json")["evidence"], "retain me")

    def test_bad_digest_health_schema_and_disk_leave_active_unchanged(self):
        initial = self.args(self.binary("one"))
        host.install(self.installation, initial)
        for mode in ["digest", "health", "schema", "disk"]:
            args = self.args(self.binary(mode, fail=mode == "health"))
            if mode == "digest":
                args.sha256 = "b" * 64
            if mode == "schema":
                args.read_state_schema = ["openagents.coder.task-store.v99"]
            if mode == "disk":
                with mock.patch.object(host.shutil, "disk_usage", return_value=argparse.Namespace(free=0)):
                    with self.assertRaises(ValueError):
                        host.install(self.installation, args)
            else:
                with self.assertRaises(ValueError):
                    host.install(self.installation, args)
            self.assertEqual(self.installation.document["active"], initial.sha256)

    def test_interrupted_staging_never_changes_activation(self):
        first = self.args(self.binary("one"))
        host.install(self.installation, first)
        stage = self.installation.root / "versions" / ".staging-interrupted"
        stage.mkdir()
        (stage / "coder").write_bytes(b"torn")
        self.installation.lock.close()
        self.installation = host.Installation(self.base / "host")
        self.assertEqual(self.installation.document["active"], first.sha256)
        self.installation.release(first.sha256)

    def test_symlinks_missing_state_and_busy_lock_refuse(self):
        with self.assertRaises(ValueError):
            host.Installation(self.base / "host")
        self.installation.lock.close()
        (self.base / "alias").symlink_to(self.base / "host")
        with self.assertRaises(ValueError):
            host.Installation(self.base / "alias")
        (self.base / "host" / "active.json").unlink()
        with self.assertRaises(ValueError):
            host.Installation(self.base / "host")

    def test_changed_binary_or_provenance_refuse(self):
        args = self.args(self.binary("one"))
        host.install(self.installation, args)
        args.source_revision = "b" * 40
        with self.assertRaises(ValueError):
            host.install(self.installation, args)
        binary, _ = self.installation.release(args.sha256)
        binary.chmod(0o700)
        binary.write_bytes(b"changed")
        with self.assertRaises(ValueError):
            self.installation.release(args.sha256)

    def test_uninstall_preserves_task_bytes_and_immutable_bundles(self):
        args = self.args(self.binary("one"))
        host.install(self.installation, args)
        before = (self.tasks / "tasks.json").read_bytes()
        self.installation.lock.close()
        result = subprocess.run([sys.executable, str(Path(__file__).with_name("coder-host.py")),
                                 "--root", str(self.base / "host"), "--tasks", str(self.tasks),
                                 "uninstall"], capture_output=True, timeout=10)
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        self.assertIsNone(host.read_json(self.base / "host/active.json")["active"])
        self.assertEqual((self.tasks / "tasks.json").read_bytes(), before)
        self.assertTrue((self.base / "host/versions" / args.sha256 / "coder").is_file())

    def test_fifo_refuses_without_waiting_for_a_writer(self):
        fifo = self.base / "not-a-file"
        os.mkfifo(fifo)
        started = time.monotonic()
        with self.assertRaises(ValueError):
            host.digest(fifo)
        self.assertLess(time.monotonic() - started, 1)

    def test_health_reaps_group_after_parent_exits(self):
        fixture = self.base / "forking-health"
        pids = self.base / "child-pids"
        fixture.write_text("#!/bin/sh\n(sleep 30 </dev/null >/dev/null 2>&1) &\nprintf '%s\\n' \"$!\" >> '" + str(pids) + "'\nprintf 'coder fixture; task execute\\n'\n")
        fixture.chmod(0o700)
        host.health(fixture)
        children = [int(value) for value in pids.read_text().splitlines()]
        self.assertEqual(len(children), 2)
        for pid in children:
            state = subprocess.run(["/bin/ps", "-o", "state=", "-p", str(pid)], capture_output=True, timeout=2).stdout.decode().strip()
            self.assertTrue(not state or state.startswith("Z"), state)

    def test_service_pins_binary_grant_and_disables_restart(self):
        args = self.args(self.binary("one"))
        host.install(self.installation, args)
        grant = self.base / "grant.json"
        grant.write_text(json.dumps({"schema": "openagents.coder.task-execution-grant.v1", "wall_seconds": 10}))
        options = argparse.Namespace(tasks=str(self.tasks), grant=str(grant), platform="macos", label="org.openagents.fixture")
        plist = plistlib.loads(host.service(self.installation, options).encode())
        self.assertFalse(plist["KeepAlive"])
        self.assertIn(args.sha256, plist["ProgramArguments"][0])
        self.assertEqual(plist["ProgramArguments"][1:3], ["task", "execute"])
        options.platform = "linux"
        unit = host.service(self.installation, options)
        self.assertIn("Restart=no", unit)
        self.assertIn("KillMode=control-group", unit)
        self.assertIn("TimeoutStartSec=40", unit)
        original = host.read_json(self.tasks / "tasks.json")
        self.assertEqual(original["evidence"], "retain me")


if __name__ == "__main__":
    unittest.main(verbosity=2)
