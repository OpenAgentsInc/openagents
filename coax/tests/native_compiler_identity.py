#!/usr/bin/env python3
"""Compiler path observations use the runner context; no programs execute."""
import hashlib
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

TOOLS = Path(__file__).resolve().parents[1] / "tools"
sys.path.insert(0, str(TOOLS))
import native_execution_identities as identities


class CompilerIdentityTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="coax-compiler-identity-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.caller, self.runner = self.root / "caller", self.root / "runner"
        self.caller.mkdir()
        self.runner.mkdir()
        previous = Path.cwd()
        os.chdir(self.caller)
        self.addCleanup(os.chdir, previous)
        self.calls = []

    def compiler(self, directory, relative, content):
        path = directory / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content)
        path.chmod(0o700)
        return path

    def query(self, argv, **bounds):
        self.calls.append((list(argv), bounds))
        return {"argv": list(argv), "returncode": 0, "stdout": b"recorded query\n"}

    def observed(self, argv, **context):
        with patch.object(identities.platform, "system", return_value="Linux"):
            return identities.inspect(argv, self.query, **context)

    def target(self, result, path):
        self.assertEqual(result["resolved_program"], str(path.resolve()))
        self.assertEqual(result["program_sha256"], hashlib.sha256(path.read_bytes()).hexdigest())
        self.assertIsNone(result["resolution_error"])

    def test_relative_program_uses_explicit_runner_cwd(self):
        self.compiler(self.caller, "cc", b"caller compiler bytes\n")
        expected = self.compiler(self.runner, "cc", b"runner compiler bytes\n")
        result = self.observed(["./cc", "-some-option"], cwd=self.runner)
        self.target(result, expected)
        self.assertEqual([call[0] for call in self.calls],
                         [["./cc", "-some-option", "--version"],
                          ["./cc", "-some-option", "-dumpmachine"]])

    def test_relative_path_entries_resolve_in_runner_cwd(self):
        for entry in ("tools", "./tools", ""):
            with self.subTest(entry=entry):
                relative = "tools/cc" if entry else "cc"
                self.compiler(self.caller, relative, b"caller PATH executable\n")
                expected = self.compiler(self.runner, relative, b"runner PATH executable\n")
                result = self.observed(["cc"], cwd=self.runner, environment={"PATH": entry})
                self.target(result, expected)

    def test_omitted_cwd_preserves_caller_context(self):
        expected = self.compiler(self.caller, "cc", b"backward-compatible caller bytes\n")
        self.compiler(self.runner, "cc", b"different runner bytes\n")
        self.target(self.observed(["./cc"]), expected)

    def test_explicit_environment_without_path_uses_exec_default(self):
        self.compiler(self.caller, "fallback/cc", b"caller fallback bytes\n")
        expected = self.compiler(self.runner, "fallback/cc", b"runner fallback bytes\n")
        with patch.object(os, "defpath", "fallback"):
            self.target(self.observed(["cc"], cwd=self.runner, environment={}), expected)

    def test_only_approved_settings_are_observed_from_runner_environment(self):
        self.compiler(self.runner, "cc", b"runner bytes\n")
        environment = {"PATH": "", "SDKROOT": "runner-sdk", "DEVELOPER_DIR": "runner-developer",
                       "UNRELATED_SETTING": "not part of the observation"}
        with patch.dict(os.environ, {"SDKROOT": "caller-sdk"}):
            result = self.observed(["cc"], cwd=self.runner, environment=environment)
        self.assertEqual(set(result["settings"]), set(identities._SETTINGS))
        self.assertEqual(result["settings"]["SDKROOT"], "runner-sdk")
        self.assertEqual(result["settings"]["DEVELOPER_DIR"], "runner-developer")
        self.assertNotIn("UNRELATED_SETTING", result["settings"])


if __name__ == "__main__":
    unittest.main()
