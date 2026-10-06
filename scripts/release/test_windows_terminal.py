"""Exercise Windows package admission and an isolated install using fixture binaries."""
import argparse
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch


def load(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + ".py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


release = load("windows-terminal")
installer = load("install-windows-terminal")


class WindowsTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.binaries = self.root / "bin"
        self.binaries.mkdir()
        for name in release.NAMES:
            path = self.binaries / name
            # Minimal x86-64 PE identity for package admission; no execution claim.
            import struct
            header = bytearray(70)
            header[:2] = b"MZ"
            struct.pack_into("<I", header, 60, 64)
            header[64:68] = b"PE\0\0"
            struct.pack_into("<H", header, 68, 0x8664)
            path.write_bytes(header + name.encode())
            path.chmod(0o755)
        self.record = {"schema": "openagents.native-terminal.windows-qualification.v1",
                       "version": "1.0.0-rc.2", "commit": "a" * 40, "platform": "windows-x86_64", "distribution": "fixture",
                       "backend": "conpty", "checks": dict.fromkeys(release.CHECKS, "passed"),
                       "executables": {name: release.digest(self.binaries / name) for name in release.NAMES}}
        self.qualification = self.root / "qualification.json"
        self.write_record()
        self.args = argparse.Namespace(binaries=self.binaries, qualification=self.qualification,
                                       version="1.0.0-rc.2", out=self.root / "out")
        self.stage = self.args.out / self.args.version / "windows-x86_64"

    def write_record(self):
        self.qualification.write_text(json.dumps(self.record))

    def package(self):
        with patch.object(release.subprocess, "check_output", return_value="a" * 40):
            release.package(self.args)

    def install(self):
        def fetch(url, destination):
            destination.write_bytes((self.stage / url.rsplit("/", 1)[1]).read_bytes())
        with patch.object(installer.platform, "system", return_value="Windows"), patch.object(installer.platform, "machine", return_value="x86_64"), patch.object(installer, "fetch", side_effect=fetch):
            installer.install("https://fixture.invalid", self.args.version, self.root / "installed")

    def test_isolated_install_preserves_three_helpers_and_no_services(self):
        self.package()
        self.install()
        self.assertEqual({path.name for path in (self.root / "installed").iterdir()}, {*release.NAMES, "release-manifest.json"})
        for name in release.NAMES:
            self.assertEqual((self.root / "installed" / name).read_bytes(), (self.binaries / name).read_bytes())

    def test_unqualified_or_wrong_commit_cannot_stage(self):
        self.record["checks"]["request_proposal_result"] = "not-run"
        self.write_record()
        with self.assertRaisesRegex(ValueError, "qualification"):
            self.package()
        self.assertFalse(self.stage.exists())
        self.record["checks"]["request_proposal_result"] = "passed"
        self.record["commit"] = "b" * 40
        self.write_record()
        with self.assertRaisesRegex(ValueError, "qualification"):
            self.package()

    def test_other_pe_architecture_refuses_package(self):
        path = self.binaries / "microcoder.exe"
        body = bytearray(path.read_bytes())
        body[68:70] = b"\x64\xaa"
        path.write_bytes(body)
        self.record["executables"][path.name] = release.digest(path)
        self.write_record()
        with self.assertRaisesRegex(ValueError, "x86-64"):
            self.package()
        self.assertFalse(self.stage.exists())

    def test_changed_tested_binary_refuses_package(self):
        (self.binaries / "microcoder.exe").write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "changed"):
            self.package()
        self.assertFalse(self.stage.exists())

    def test_changed_archive_refuses_install_without_partial_destination(self):
        self.package()
        (self.stage / "OpenAgents-Terminal-windows-x86_64.zip").write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "Archive checksum"):
            self.install()
        self.assertFalse((self.root / "installed").exists())

    def test_changed_qualification_refuses_install(self):
        self.package()
        (self.stage / "qualification.json").write_text("{}")
        with self.assertRaisesRegex(ValueError, "Qualification"):
            self.install()
        self.assertFalse((self.root / "installed").exists())

    def test_publish_refuses_corrupt_checksums_before_cloud(self):
        self.package()
        (self.stage / "SHA256SUMS").write_text("changed")
        with patch.object(release.subprocess, "run") as cloud, self.assertRaisesRegex(ValueError, "checksum"):
            release.publish(argparse.Namespace(stage=self.stage, bucket="fixture-bucket"))
        cloud.assert_not_called()


if __name__ == "__main__":
    unittest.main()
