"""Check the credential adapter's manifest export without applying it to SPACE."""

import importlib.util
from pathlib import Path
import unittest


spec = importlib.util.spec_from_file_location(
    "space_credentials_apply",
    Path(__file__).resolve().parents[2] / "deploy/space-credentials/apply.py",
)
adapter = importlib.util.module_from_spec(spec)
spec.loader.exec_module(adapter)


class ManifestExportTests(unittest.TestCase):
    def test_export_removes_only_generated_hakari_edge(self):
        text = '''[package]
name = "computer-credentials"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
publish.workspace = true

[dependencies]
serde = { version = "1", features = ["derive"] }

[target.'cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))'.dependencies]
openagents-workspace-hack = { version = "0.1" }
libc = "0.2"
'''
        exported = adapter.standalone_manifest(text)
        self.assertNotIn('openagents-workspace-hack', exported)
        self.assertIn('version = "1.0.0"\nedition = "2024"\nrust-version = "1.97"\npublish = false', exported)
        self.assertIn('serde = { version = "1", features = ["derive"] }', exported)
        self.assertTrue(exported.endswith('libc = "0.2"\n'))

    def test_export_does_not_silently_strip_custom_hack_dependencies(self):
        text = '[dependencies]\nopenagents-workspace-hack = { version = "0.2", optional = true }\n'
        self.assertEqual(adapter.standalone_manifest(text), text)


if __name__ == '__main__':
    unittest.main()
