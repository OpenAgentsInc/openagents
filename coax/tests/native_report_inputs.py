#!/usr/bin/env python3
"""Report destinations preserve explicit argv file inputs; no compiler runs."""
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

TOOLS = Path(__file__).resolve().parents[1] / "tools"
sys.path.insert(0, str(TOOLS))
import native_check_execution as execution


class ReportInputTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="coax-report-inputs-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.sequence = 0

    def preserved(self, relative, make_record, *, executable=False):
        for alias in ("same", "hardlink", "symlink"):
            with self.subTest(relative=relative, alias=alias):
                directory = self.root / str(self.sequence)
                self.sequence += 1
                actual = directory / "process"
                actual.mkdir(parents=True)
                source = actual / relative
                source.parent.mkdir(parents=True, exist_ok=True)
                payload = b"explicit compiler input; preserve these bytes\n"
                source.write_bytes(payload)
                if executable:
                    source.chmod(0o700)
                destination = source if alias == "same" else directory / "report.json"
                if alias == "hardlink":
                    os.link(source, destination)
                elif alias == "symlink":
                    destination.symlink_to(source)
                record, settings = make_record(actual)
                with patch.object(execution, "REPO", settings.get("repo", actual)), \
                     patch.dict(os.environ, settings.get("env", {})):
                    try:
                        with self.assertRaises(ValueError):
                            execution.write_report(destination, record)
                    finally:
                        self.assertEqual(source.read_bytes(), payload)
                        self.assertEqual(destination.read_bytes(), payload)

    def test_bare_and_subdirectory_inputs_follow_recorded_process_cwd(self):
        for relative in ("header.h", "sub/header.h"):
            def record(actual):
                # The parent cwd is deliberately different from the process.
                return ({"files": [], "cwd": str(actual.parent), "observation": {
                    "cwd": str(actual), "nested": {"argv": ["missing-compiler", "-include", relative]}}},
                    {"repo": actual.parent})
            self.preserved(relative, record)

    def test_compiler_configuration_inputs_use_repository_cwd(self):
        for key in ("native_cc", "host_cc"):
            for relative in ("header.h", "sub/header.h"):
                def record(actual):
                    return ({"files": [], "cwd": str(actual.parent),
                             "nested": {key: ["missing-compiler", "-include", relative]}},
                            {"repo": actual})
                self.preserved(relative, record)

    def test_missing_cwd_defaults_to_repository(self):
        self.preserved("header.h", lambda actual: (
            {"files": [], "observation": {"launched_argv": ["missing-compiler", "header.h"]}},
            {"repo": actual}))

    def test_bare_path_executable_remains_protected(self):
        self.preserved("lookup-compiler", lambda actual: (
            {"files": [], "observation": {"cwd": str(actual.parent), "argv": ["lookup-compiler"]}},
            {"repo": actual.parent, "env": {"PATH": str(actual)}}), executable=True)

    def test_bare_token_protects_both_cwd_file_and_path_executable(self):
        for role in ("file", "program"):
            def record(actual):
                other = actual.parent / "other"
                other.mkdir()
                counterpart = other / "lookup-compiler"
                counterpart.write_bytes(b"the other regular-file interpretation\n")
                if role == "file":
                    counterpart.chmod(0o700)
                cwd = actual if role == "file" else other
                programs = other if role == "file" else actual
                return ({"files": [], "observation": {"cwd": str(cwd), "argv": ["lookup-compiler"]}},
                        {"repo": cwd, "env": {"PATH": str(programs)}})
            self.preserved("lookup-compiler", record, executable=role == "program")


if __name__ == "__main__":
    unittest.main()
