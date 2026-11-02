"""Failure controls for the catalog qualification driver; no engine reruns."""
import contextlib
import importlib.util
import io
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock

PATH = Path(__file__).with_name("check_catalog.py")
SPEC = importlib.util.spec_from_file_location("catalog_check", PATH)
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)


class CatalogTooling(unittest.TestCase):
    def test_stale_compiler_is_rejected_before_any_emission_or_native_command(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            binary = root / "coax/bin/coax"
            binary.parent.mkdir(parents=True)
            binary.write_bytes(b"controlled compiler identity")
            binary.with_name("source-digest").write_text("old\n")
            calls = []

            def run(argv, **kwargs):
                calls.append(argv)
                if argv[1] == "version":
                    return subprocess.CompletedProcess(argv, 0, b"coax0 fixture", b"")
                if argv[1] == "source-digest":
                    return subprocess.CompletedProcess(argv, 0, b"changed\n", b"")
                self.fail("stale compiler reached an emission or native command")

            args = [str(PATH), "--work-dir", str(root / "work"), "--receipt", str(root / "receipt.json")]
            with mock.patch.object(CHECK, "ROOT", root), mock.patch("sys.argv", args), mock.patch.object(CHECK.subprocess, "run", side_effect=run):
                with self.assertRaisesRegex(RuntimeError, "recorded/current source digests differ"):
                    CHECK.main()
            self.assertEqual([v[1] for v in calls], ["version", "source-digest"])
            self.assertFalse((root / "receipt.json").exists())

    def test_added_or_omitted_abi_fields_cannot_silently_escape_the_inventory(self):
        header = "\n".join("struct coax_t_" + name + " {" + ";".join("int " + field for field in fields) + ";};" for name, fields in CHECK.FIELDS.items())
        CHECK.header_inventory(header)
        with self.assertRaises(AssertionError):
            CHECK.header_inventory(header.replace("int stages;", "int stages;int unrecorded;"))
        with self.assertRaises(AssertionError):
            CHECK.header_inventory(header.replace("int assets;", ""))


if __name__ == "__main__":
    unittest.main()
