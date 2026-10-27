"""The foreign-language bundle must include and pin the top-level schemas."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class ExportTest(unittest.TestCase):
    def test_top_level_schema_export_and_dirty_source_guard(self):
        with tempfile.TemporaryDirectory() as directory:
            folder = Path(directory)
            repo = folder / 'repo'
            crate = repo / 'crates/engine-proto'
            shutil.copytree(ROOT, crate, ignore=shutil.ignore_patterns('__pycache__'))
            shutil.copytree(ROOT.parents[1] / 'proto', repo / 'proto')
            def git(*args):
                return subprocess.check_output(['git', '-C', str(repo), *args], stderr=subprocess.STDOUT)
            git('init', '-q')
            git('add', '.')
            git('-c', 'user.name=Proto Test', '-c', 'user.email=proto@example.invalid',
                '-c', 'core.hooksPath=/dev/null', 'commit', '-qm', 'Fixture')
            command = [sys.executable, str(crate / 'tools/export.py')]
            bundle = folder / 'bundle'
            subprocess.run([*command, str(bundle)], check=True, capture_output=True)
            manifest = json.loads((bundle / 'manifest.json').read_text())
            self.assertFalse(manifest['dirty'])
            self.assertEqual(manifest['revision'], git('rev-parse', 'HEAD').decode().strip())
            for suffix in ('proto', 'options'):
                name = f'proto/openagents/engine/v1/scene.{suffix}'
                expected = (repo / name).read_bytes()
                self.assertEqual((bundle / name).read_bytes(), expected)
                self.assertEqual(manifest['files'][name], hashlib.sha256(expected).hexdigest())
                # Both schema and codegen options are outside the crate now.
                # A dirty one must prevent an apparently committed export.
                source = repo / name
                source.write_bytes(expected + b'\n')
                rejected = folder / 'rejected'
                result = subprocess.run([*command, str(rejected)], capture_output=True, text=True)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn('Commit the contract', result.stderr)
                self.assertFalse(rejected.exists())
                dirty = folder / f'dirty-{suffix}'
                subprocess.run([*command, str(dirty), '--allow-dirty'], check=True, capture_output=True)
                self.assertTrue(json.loads((dirty / 'manifest.json').read_text())['dirty'])
                self.assertEqual((dirty / name).read_bytes(), source.read_bytes())
                source.write_bytes(expected)
