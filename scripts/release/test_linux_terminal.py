"""Exercise Linux package admission and an isolated install using fixture binaries."""
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


release = load("linux-terminal")
installer = load("install-linux-terminal")


class LinuxTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.binaries = self.root / "bin"
        self.binaries.mkdir()
        for name in release.NAMES:
            path = self.binaries / name
            path.write_bytes(name.encode())
            path.chmod(0o755)
        self.record = {"schema": "openagents.native-terminal.linux-qualification.v1",
                       "commit": "a" * 40, "platform": "linux-x86_64", "distribution": "fixture",
                       "backend": "x11", "checks": dict.fromkeys(release.CHECKS, "passed"),
                       "executables": {name: release.digest(self.binaries / name) for name in release.NAMES}}
        self.qualification = self.root / "qualification.json"
        self.write_record()
        self.args = argparse.Namespace(binaries=self.binaries, qualification=self.qualification,
                                       version="1.0.0-rc.2", out=self.root / "out")
        self.stage = self.args.out / self.args.version / "linux-x86_64"

    def write_record(self):
        self.qualification.write_text(json.dumps(self.record))

    def package(self):
        with patch.object(release.subprocess, "check_output", side_effect=["a" * 40, *(["ELF 64-bit x86-64"] * 3)]):
            release.package(self.args)

    def install(self):
        def fetch(url, destination):
            destination.write_bytes((self.stage / url.rsplit("/", 1)[1]).read_bytes())
        with patch.object(installer.platform, "system", return_value="Linux"), patch.object(installer.platform, "machine", return_value="x86_64"), patch.object(installer, "fetch", side_effect=fetch):
            installer.install("https://fixture.invalid", self.args.version, self.root / "installed")

    def test_isolated_install_preserves_three_helpers_and_no_services(self):
        self.package()
        self.install()
        self.assertEqual({path.name for path in (self.root / "installed").iterdir()}, {*release.NAMES, "release-manifest.json"})
        for name in release.NAMES:
            self.assertEqual((self.root / "installed" / name).read_bytes(), name.encode())
            self.assertTrue((self.root / "installed" / name).stat().st_mode & 0o111)

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

    def test_changed_tested_binary_refuses_package(self):
        (self.binaries / "microcoder").write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "changed"):
            self.package()
        self.assertFalse(self.stage.exists())

    def test_changed_archive_refuses_install_without_partial_destination(self):
        self.package()
        (self.stage / "OpenAgents-Terminal-linux-x86_64.tar.gz").write_bytes(b"changed")
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
