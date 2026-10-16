"""Ensure registry metadata can only advance the single stable SDK pin."""
import contextlib
import io
import json
import os
from pathlib import Path
import runpy
import tempfile
import unittest
from unittest.mock import patch

SCRIPT = Path(__file__).with_name('update-cursor-sdk.py').resolve()

class UpdateTests(unittest.TestCase):
    def run_update(self, version):
        with tempfile.TemporaryDirectory() as directory:
            previous = Path.cwd()
            try:
                os.chdir(directory)
                pin = Path('crates/harness/src/cursor/mod.rs')
                pin.parent.mkdir(parents=True)
                original = '// preserve this\nconst CURSOR_SDK_PIN: &str = "@cursor/sdk@1.0.31";\n'
                pin.write_text(original)
                with patch('subprocess.check_output', return_value=json.dumps(version)), contextlib.redirect_stdout(io.StringIO()):
                    runpy.run_path(str(SCRIPT), run_name='__main__')
                return pin.read_text(), original
            finally:
                os.chdir(previous)

    def test_new_stable_changes_only_pin(self):
        updated, original = self.run_update('1.0.32')
        self.assertEqual(updated, original.replace('1.0.31','1.0.32'))

    def test_never_downgrades_or_rewrites_current(self):
        for version in ['1.0.31', '1.0.9']:
            updated, original = self.run_update(version)
            self.assertEqual(updated, original)

    def test_rejects_prerelease_or_nonversion_registry_data(self):
        for value in ['1.0.32-beta.1', '1.0.32; echo injected', {'version':'1.0.32'}]:
            with self.assertRaises(SystemExit):
                self.run_update(value)

if __name__ == '__main__':
    unittest.main()
