#!/usr/bin/env python3
"""Test the hosted Coder installer with scratch files and a loopback server."""

import functools
import hashlib
import http.server
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import threading
import unittest


INSTALLER = Path(__file__).resolve().parent / "install" / "coder.sh"


class QuietHandler(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *args):
        pass


class HostedInstallerTests(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory(prefix="coder-hosted-install-")
        self.root = Path(self.scratch.name)
        self.release = self.root / "release"
        self.release.mkdir()
        self.bin = self.root / "bin"
        self.profile_root = self.root / "profiles"
        self.profile_root.mkdir()
        self.profile = self.profile_root / ".zshrc"
        self.profile.write_text("# Existing shell settings\n")
        self.fake_bin = self.root / "fake-bin"
        self.fake_bin.mkdir()
        self.stub("uname", 'case "$1" in -s) echo Darwin;; -m) echo arm64;; esac\n')
        self.handler = functools.partial(QuietHandler, directory=str(self.release))
        self.server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), self.handler)
        self.server_thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.server_thread.start()
        self.env = {
            **os.environ,
            "CODER_BASE_URL": f"http://127.0.0.1:{self.server.server_port}",
            "CODER_BIN_DIR": str(self.bin),
            "SHELL": "/bin/zsh",
            "ZDOTDIR": str(self.profile_root),
            "PATH": str(self.fake_bin) + os.pathsep + os.environ["PATH"],
        }
        for name in ("CODER_CHANNEL", "CODER_VERSION", "CODER_NO_PATH_UPDATE"):
            self.env.pop(name, None)

    def tearDown(self):
        self.server.shutdown()
        self.server.server_close()
        self.server_thread.join()
        self.scratch.cleanup()

    def stub(self, name, source):
        path = self.fake_bin / name
        path.write_text("#!/bin/sh\n" + source)
        path.chmod(0o755)

    def publish(self, version, platform="macos-aarch64", bad_command=None, wrong_version_command=None,
                separate=False, omit=None):
        """Publish one archive per platform, or (`separate`) the per-command
        files releases up to 1.0.0-rc.5 used."""
        sums = []
        staging = self.root / "staging" / f"{version}-{platform}"
        staging.mkdir(parents=True, exist_ok=True)
        for command in ("coder", "openagents", "microcoder"):
            if command == omit:
                continue
            path = staging / command
            reported_version = "0.0.0" if command == wrong_version_command else version
            text = f"#!/bin/sh\nprintf '%s\\n' '{command} {reported_version}'\n"
            if command == bad_command:
                text += "exit 7\n"
            path.write_text(text)
            path.chmod(0o755)
            if separate:
                artifact = self.release / f"{command}-{version}-{platform}"
                shutil.copyfile(path, artifact)
                sums.append(f"{hashlib.sha256(artifact.read_bytes()).hexdigest()}  {artifact.name}\n")
        if not separate:
            archive = self.release / f"coder-{version}-{platform}.tar.gz"
            with tarfile.open(archive, "w:gz") as bundle:
                for path in sorted(staging.iterdir()):
                    bundle.add(path, arcname=path.name)
            sums.append(f"{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}\n")
        (self.release / f"SHA256SUMS-coder-{version}").write_text("".join(sums))

    def run_install(self, *args, **overrides):
        return subprocess.run(
            ["sh", str(INSTALLER), *args],
            env={**self.env, **overrides},
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
            timeout=15,
        )

    def assert_installed(self, version, bin_dir=None):
        installed = bin_dir or self.bin
        for command in ("coder", "openagents", "microcoder"):
            result = subprocess.run([str(installed / command), "--version"], capture_output=True, text=True, check=True)
            self.assertEqual(result.stdout.strip(), f"{command} {version}")
        self.assertFalse(list(installed.glob(".coder-install.*")))

    def install_initial(self):
        self.publish("1.0.0-rc.1")
        (self.release / "coder.rc").write_text("1.0.0-rc.1\n")
        result = self.run_install()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assert_installed("1.0.0-rc.1")

    def test_a_verified_artifact_reporting_the_wrong_version_preserves_the_bundle(self):
        self.install_initial()
        for command in ("coder", "openagents", "microcoder"):
            with self.subTest(command=command):
                self.publish("1.0.0-rc.2", wrong_version_command=command)
                result = self.run_install("1.0.0-rc.2")
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(f"Downloaded {command} does not report version 1.0.0-rc.2", result.stderr)
                self.assert_installed("1.0.0-rc.1")

    def test_default_rc_and_rerun_update_all_companions(self):
        self.install_initial()
        self.publish("1.0.0-rc.3")
        (self.release / "coder.rc").write_text("1.0.0-rc.3\n")
        for _ in range(2):
            result = self.run_install()
            self.assertEqual(result.returncode, 0, result.stderr)
        self.assert_installed("1.0.0-rc.3")
        self.assertEqual(self.profile.read_text().count("export PATH="), 1)
        self.assertTrue(self.profile.read_text().startswith("# Existing shell settings\n"))

    def test_exact_version_and_stable_channel_selection(self):
        self.publish("1.0.0")
        self.publish("1.0.0-rc.3")
        (self.release / "coder.stable").write_text("1.0.0\n")
        result = self.run_install("stable")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assert_installed("1.0.0")
        result = self.run_install("1.0.0-rc.3", CODER_VERSION="9.9.9")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assert_installed("1.0.0-rc.3")

    def test_hash_and_version_failures_keep_the_existing_bundle(self):
        self.install_initial()
        self.publish("1.0.0-rc.2")
        (self.release / "coder-1.0.0-rc.2-macos-aarch64.tar.gz").write_text("tampered\n")
        result = self.run_install("1.0.0-rc.2")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Checksum mismatch for coder-1.0.0-rc.2-macos-aarch64.tar.gz", result.stderr)
        self.assert_installed("1.0.0-rc.1")
        self.publish("1.0.0-rc.2", separate=True)
        (self.release / "microcoder-1.0.0-rc.2-macos-aarch64").write_text("tampered\n")
        result = self.run_install("1.0.0-rc.2")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Checksum mismatch for microcoder", result.stderr)
        self.assert_installed("1.0.0-rc.1")
        self.publish("1.0.0-rc.2", bad_command="microcoder")
        result = self.run_install("1.0.0-rc.2")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("does not run", result.stderr)
        self.assert_installed("1.0.0-rc.1")

    def test_missing_or_duplicate_checksum_entries_refuse_installation(self):
        self.install_initial()
        self.publish("1.0.0-rc.2")
        sums = self.release / "SHA256SUMS-coder-1.0.0-rc.2"
        lines = sums.read_text().splitlines(keepends=True)
        for text in ("".join(lines[:-1]), "".join(lines + [lines[-1]])):
            sums.write_text(text)
            result = self.run_install("1.0.0-rc.2")
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("no verified", result.stderr)
            self.assert_installed("1.0.0-rc.1")

    def test_a_failed_atomic_rename_restores_regular_files_and_symlinks(self):
        self.install_initial()
        coder_bytes = (self.bin / "coder").read_bytes()
        (self.root / "old-coder").write_bytes(coder_bytes)
        (self.root / "old-coder").chmod(0o755)
        (self.bin / "coder").unlink()
        (self.bin / "coder").symlink_to("../old-coder")
        self.publish("1.0.0-rc.2")
        real_mv = shutil.which("mv")
        self.stub("mv", '''
case "$1 $2" in "-f "*/.coder-install.*/microcoder) exit 9;; esac
exec "$CODER_TEST_REAL_MV" "$@"
''')
        result = self.run_install("1.0.0-rc.2", CODER_TEST_REAL_MV=real_mv)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("restoring the previous commands", result.stderr)
        self.assertEqual(os.readlink(self.bin / "coder"), "../old-coder")
        self.assert_installed("1.0.0-rc.1")

    def test_platform_detection_handles_rosetta_and_musl(self):
        self.publish("1.0.0-rc.3", "macos-aarch64")
        self.stub("uname", 'case "$1" in -s) echo Darwin;; -m) echo x86_64;; esac\n')
        self.stub("sysctl", "echo 1\n")
        result = self.run_install("1.0.0-rc.3")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Rosetta", result.stderr)
        self.publish("1.0.0-rc.3", "linux-aarch64-musl")
        self.stub("uname", 'case "$1" in -s) echo Linux;; -m) echo aarch64;; esac\n')
        self.stub("ldd", "echo musl\n")
        result = self.run_install("1.0.0-rc.3")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("linux-aarch64-musl", result.stderr)

    def test_path_setup_preserves_symlinked_profiles_and_quotes_literal_paths(self):
        self.publish("1.0.0-rc.3")
        target = self.root / "shared-profile"
        target.write_text("# Retained settings\n")
        self.profile.unlink()
        self.profile.symlink_to(target)
        odd_bin = self.root / "odd ' $(not-a-command) bin"
        result = self.run_install("1.0.0-rc.3", CODER_BIN_DIR=str(odd_bin))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(self.profile.is_symlink())
        self.assert_installed("1.0.0-rc.3", odd_bin)
        path = subprocess.run(["sh", "-c", '. "$1"; printf %s "$PATH"', "sh", str(self.profile)], env=self.env, text=True, capture_output=True, check=True)
        self.assertTrue(path.stdout.startswith(str(odd_bin) + os.pathsep), path.stdout)
        self.assertFalse(path.stderr)

    def test_invalid_versions_channels_and_unpublished_platforms_fail_clearly(self):
        for version in ("../escape", "1.0.0-rc.01", "1.0", "1.0.0;echo nope"):
            result = self.run_install(version)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("Not a version", result.stderr)
        result = self.run_install(CODER_CHANNEL="unknown")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Unknown channel", result.stderr)
        self.publish("1.0.0-rc.3", "some-other-platform")
        result = self.run_install("1.0.0-rc.3")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("no verified macos-aarch64 build", result.stderr)
        self.assertFalse(list(self.bin.glob(".coder-install.*")))

    def test_one_archive_installs_every_command(self):
        self.publish("1.0.0-rc.6")
        self.assertEqual(
            sorted(path.name for path in self.release.iterdir() if "1.0.0-rc.6" in path.name),
            ["SHA256SUMS-coder-1.0.0-rc.6", "coder-1.0.0-rc.6-macos-aarch64.tar.gz"],
        )
        result = self.run_install("1.0.0-rc.6")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assert_installed("1.0.0-rc.6")
        self.assertNotIn("microcoder", result.stderr)

    def test_releases_published_as_separate_commands_still_install(self):
        self.publish("1.0.0-rc.5", separate=True)
        result = self.run_install("1.0.0-rc.5")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assert_installed("1.0.0-rc.5")

    def test_an_archive_missing_a_command_preserves_the_bundle(self):
        self.install_initial()
        self.publish("1.0.0-rc.6", omit="microcoder")
        result = self.run_install("1.0.0-rc.6")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("has no microcoder", result.stderr)
        self.assert_installed("1.0.0-rc.1")

    def test_skipping_path_setup_leaves_the_profile_unchanged(self):
        self.publish("1.0.0-rc.3")
        result = self.run_install("1.0.0-rc.3", CODER_NO_PATH_UPDATE="1")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.profile.read_text(), "# Existing shell settings\n")


if __name__ == "__main__":
    unittest.main()
