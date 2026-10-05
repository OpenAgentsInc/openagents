"""Check native release admission and immutable publication without Apple or cloud calls."""
import argparse
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("native_release", Path(__file__).with_name("native-terminal.py"))
release = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(release)


class AdmissionTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.stage = Path(self.directory.name)
        binaries = self.stage / release.APP / "Contents/MacOS"
        binaries.mkdir(parents=True)
        names = ("openagents-terminal", "openagents", "microcoder")
        for name in names:
            (binaries / name).write_bytes(name.encode())
        (self.stage / "OpenAgents-Terminal.zip").write_bytes(b"fixture archive; not a signed release")
        (self.stage / "install-native-terminal.py").write_bytes((release.REPO / "scripts/release/install-native-terminal.py").read_bytes())
        self.manifest = {"schema": "openagents.native-terminal.release.v1", "prefix": release.PREFIX,
                         "platform": "darwin-arm64", "version": "1.0.0-rc.2", "commit": "fixture",
                         "executables": {name: release.digest(binaries / name) for name in names},
                         "archive_sha256": release.digest(self.stage / "OpenAgents-Terminal.zip"),
                         "installer_sha256": release.digest(self.stage / "install-native-terminal.py"),
                         "signing": "passed", "notarization": "passed", "gatekeeper": "passed"}
        self.seal()

    def seal(self):
        release.save_manifest(self.stage, self.manifest)
        (self.stage / "SHA256SUMS").write_text(f"{self.manifest['archive_sha256']}  OpenAgents-Terminal.zip\n{release.digest(self.stage / 'release-manifest.json')}  release-manifest.json\n{self.manifest['installer_sha256']}  install-native-terminal.py\n")

    def test_missing_verdict_refuses_publication_before_any_cloud_call(self):
        self.manifest["notarization"] = "not-run"
        self.seal()
        with patch.object(release, "run") as command, self.assertRaisesRegex(ValueError, "passed"):
            release.publish(argparse.Namespace(stage=str(self.stage), bucket="fixture-bucket"))
        command.assert_not_called()

    def test_changed_helpers_archive_or_sums_are_refused(self):
        release.verify_release(self.stage)
        binary = self.stage / release.APP / "Contents/MacOS/microcoder"
        binary.write_bytes(b"another commit")
        with self.assertRaisesRegex(ValueError, "Executable changed"):
            release.verify_release(self.stage)
        binary.write_bytes(b"microcoder")
        (self.stage / "OpenAgents-Terminal.zip").write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "Archive changed"):
            release.verify_release(self.stage)
        (self.stage / "OpenAgents-Terminal.zip").write_bytes(b"fixture archive; not a signed release")
        (self.stage / "SHA256SUMS").write_text("changed")
        with self.assertRaisesRegex(ValueError, "Checksum file changed"):
            release.verify_release(self.stage)

    def test_existing_version_is_never_overwritten(self):
        with patch.object(release.subprocess, "check_output", side_effect=[
            "gs://fixture-bucket/openagents-terminal/\n", "gs://fixture-bucket/openagents-terminal/1.0.0-rc.2/\n"]), patch.object(release, "run") as command:
            with self.assertRaisesRegex(ValueError, "already present"):
                release.publish(argparse.Namespace(stage=str(self.stage), bucket="fixture-bucket"))
        command.assert_not_called()

    def test_readback_failure_does_not_write_a_success_receipt(self):
        with patch.object(release.subprocess, "check_output", return_value="gs://fixture-bucket/openagents/\n"), patch.object(release, "run") as command, patch.object(release, "public_digest", return_value="wrong"):
            with self.assertRaisesRegex(ValueError, "Public readback failed"):
                release.publish(argparse.Namespace(stage=str(self.stage), bucket="fixture-bucket"))
        self.assertFalse((self.stage / "publication-receipt.json").exists())
        for call in command.call_args_list:
            self.assertIn("--if-generation-match=0", call.args)
            self.assertIn("/openagents-terminal/1.0.0-rc.2/", call.args[-1])

    def test_publication_receipt_labels_unrun_install_and_demo(self):
        checksums = iter([release.digest(self.stage / name) for name in ("OpenAgents-Terminal.zip", "release-manifest.json", "SHA256SUMS")] + [release.digest(release.REPO / "scripts/release/install-native-terminal.py")])
        with patch.object(release.subprocess, "check_output", return_value="gs://fixture-bucket/openagents/\n"), patch.object(release, "run"), patch.object(release, "public_digest", side_effect=lambda _: next(checksums)):
            release.publish(argparse.Namespace(stage=str(self.stage), bucket="fixture-bucket"))
        receipt = json.loads((self.stage / "publication-receipt.json").read_text())
        self.assertEqual(receipt["public_readback"], "passed")
        self.assertEqual(receipt["installed_flow"], "not-run")
        self.assertEqual(receipt["both_surface_demo"], "not-run")

    def installer(self):
        spec = importlib.util.spec_from_file_location("native_install", Path(__file__).with_name("install-native-terminal.py"))
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module

    def test_installer_refuses_corrupt_archive_before_extraction(self):
        install = self.installer()
        destination = self.stage / "installed"
        def fetch(url, path):
            name = url.rsplit("/", 1)[1]
            path.write_bytes((self.stage / name).read_bytes())
            if name.endswith(".zip"):
                path.write_bytes(b"corrupt archive")
        with patch.object(install.platform, "system", return_value="Darwin"), patch.object(install.platform, "machine", return_value="arm64"), patch.object(install, "fetch", side_effect=fetch), patch.object(install.subprocess, "run") as command, patch("sys.argv", ["install", "--version", "1.0.0-rc.2", "--destination", str(destination)]), self.assertRaises(SystemExit):
            install.main()
        command.assert_not_called()
        self.assertFalse(destination.exists())

    def test_installer_does_not_leave_a_partial_install_for_changed_helper(self):
        install = self.installer()
        destination = self.stage / "installed"
        def fetch(url, path):
            path.write_bytes((self.stage / url.rsplit("/", 1)[1]).read_bytes())
        def extract(command, **_):
            if command[0] == "ditto":
                binaries = Path(command[-1]) / release.APP / "Contents/MacOS"
                binaries.mkdir(parents=True)
                for name in self.manifest["executables"]:
                    (binaries / name).write_bytes(b"changed" if name == "microcoder" else name.encode())
        with patch.object(install.platform, "system", return_value="Darwin"), patch.object(install.platform, "machine", return_value="arm64"), patch.object(install, "fetch", side_effect=fetch), patch.object(install.subprocess, "run", side_effect=extract), patch("sys.argv", ["install", "--version", "1.0.0-rc.2", "--destination", str(destination)]), self.assertRaises(SystemExit):
            install.main()
        self.assertFalse(destination.exists())


if __name__ == "__main__":
    unittest.main()
