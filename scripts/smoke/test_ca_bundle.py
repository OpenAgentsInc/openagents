"""Check smoke CA selection without network access or deployment.

Run with OPENAGENTS_SCRATCH set to the session's scratch directory.
"""

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
SYSTEM_CA = Path("/etc/ssl/cert.pem")
RECORDER = """import json, os, sys
from pathlib import Path
Path(os.environ["CA_TEST_CAPTURE"]).write_text(json.dumps({
    "file": os.environ.get("SSL_CERT_FILE"),
    "directory": os.environ.get("SSL_CERT_DIR"),
    "args": sys.argv[2:],
}))
sys.exit(int(sys.argv[1]))
"""


class SmokeCaBundle(unittest.TestCase):
    def check_entrypoints(self, setting):
        for script, mode in [
            ("scripts/smoke/staging.sh", "--production"),
            ("scripts/smoke/staging.sh", "--candidate"),
            ("scripts/deploy/web.sh", "stage"),
            ("scripts/deploy/web.sh", "promote"),
        ]:
            with self.subTest(script=script, mode=mode, setting=setting):
                with tempfile.TemporaryDirectory(
                    prefix="smoke-ca-", dir=os.environ["OPENAGENTS_SCRATCH"]
                ) as directory:
                    root = Path(directory)
                    tools = root / "bin"
                    tools.mkdir()
                    recorder = root / "record.py"
                    recorder.write_text(RECORDER)
                    # Deploy stops at its first rev-parse, before mkdir,
                    # cloud commands, builds, or any deployment writes.
                    for name, code in [("git", 42), ("python3.13", 0)]:
                        tool = tools / name
                        tool.write_text(
                            '#!/bin/sh\nexec "$CA_TEST_PYTHON" -I '
                            f'"$CA_TEST_RECORDER" {code} "$@"\n'
                        )
                        tool.chmod(0o755)
                    capture = root / "capture.json"
                    state = root / "state-must-not-exist"
                    environment = {
                        **os.environ,
                        "HOME": str(root),
                        "PATH": str(tools) + os.pathsep + "/usr/bin:/bin",
                        "XDG_STATE_HOME": str(state),
                        "CA_TEST_PYTHON": sys.executable,
                        "CA_TEST_RECORDER": str(recorder),
                        "CA_TEST_CAPTURE": str(capture),
                    }
                    environment.pop("SSL_CERT_FILE", None)
                    environment.pop("SSL_CERT_DIR", None)
                    expected_file = str(SYSTEM_CA)
                    expected_directory = None
                    if setting == "empty":
                        environment.update(SSL_CERT_FILE="", SSL_CERT_DIR="")
                        expected_directory = ""
                    elif setting == "file":
                        custom = root / "custom-ca.pem"
                        custom.write_text("private fixture CA bundle\n")
                        expected_file = str(custom)
                        environment["SSL_CERT_FILE"] = expected_file
                    elif setting == "directory":
                        custom = root / "custom-ca"
                        custom.mkdir()
                        expected_file = None
                        expected_directory = str(custom)
                        environment["SSL_CERT_DIR"] = expected_directory
                    result = subprocess.run(
                        ["/bin/sh", str(ROOT / script), mode],
                        env=environment, text=True, capture_output=True, timeout=10,
                    )
                    deployment = script.startswith("scripts/deploy/")
                    self.assertEqual(result.returncode, 42 if deployment else 0, result.stderr)
                    observed = json.loads(capture.read_text())
                    self.assertEqual(observed["file"], expected_file)
                    self.assertEqual(observed["directory"], expected_directory)
                    if deployment:
                        self.assertEqual(observed["args"], ["rev-parse", "--show-toplevel"])
                        self.assertFalse(state.exists(), "deploy probe must stop before state writes")
                    else:
                        self.assertEqual(observed["args"][0], "-I")
                        self.assertEqual(observed["args"][-1], mode)

    @unittest.skipUnless(os.access(SYSTEM_CA, os.R_OK), "system CA bundle is unavailable")
    def test_unset_ca_uses_system_bundle(self):
        self.check_entrypoints("unset")

    @unittest.skipUnless(os.access(SYSTEM_CA, os.R_OK), "system CA bundle is unavailable")
    def test_empty_ca_uses_system_bundle(self):
        self.check_entrypoints("empty")

    def test_explicit_ca_file_is_preserved(self):
        self.check_entrypoints("file")

    def test_explicit_ca_directory_is_preserved(self):
        self.check_entrypoints("directory")


if __name__ == "__main__":
    unittest.main()
