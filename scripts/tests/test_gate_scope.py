#!/usr/bin/env python3
"""Check the changed-path-to-crate resolution the scoped gate relies on."""
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import importlib.util

spec = importlib.util.spec_from_file_location(
    "verify_changed",
    Path(__file__).resolve().parent.parent / "verify-changed.py",
)
verify_changed = importlib.util.module_from_spec(spec)
spec.loader.exec_module(verify_changed)


class ResolveTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        manifest = self.root / "crates" / "example" / "Cargo.toml"
        manifest.parent.mkdir(parents=True)
        manifest.write_text('[package]\nname = "example-pkg"\n')

    def tearDown(self):
        self.tmp.cleanup()

    def test_crate_paths_map_to_the_package_the_manifest_names(self):
        plan = verify_changed.resolve(self.root, ["crates/example/src/lib.rs"])
        self.assertEqual(plan["crates"], ["example-pkg"])
        self.assertFalse(plan["workspace"])

    def test_workspace_files_escalate_out_of_crate_scope(self):
        for path in ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml",
                     "deny.toml", ".cargo/config.toml"]:
            plan = verify_changed.resolve(self.root, [path])
            self.assertTrue(plan["workspace"], path)

    def test_data_dirs_belong_to_the_crate_that_loads_them(self):
        for path in ["programs/x.json", "questions/x.json",
                     "capabilities/x.json", "sources/x.json"]:
            plan = verify_changed.resolve(self.root, [path])
            self.assertEqual(plan["crates"], ["coder"], path)

    def test_other_paths_are_reported_as_uncovered(self):
        plan = verify_changed.resolve(
            self.root, ["docs/a.md", "crates/example/src/lib.rs"])
        self.assertEqual(plan["crates"], ["example-pkg"])
        self.assertEqual(plan["uncovered"], ["docs/a.md"])


class CliTests(unittest.TestCase):
    def test_a_clean_tree_names_no_crates(self):
        root = Path(__file__).resolve().parent.parent.parent
        out = subprocess.run(
            [sys.executable, str(root / "scripts" / "verify-changed.py"),
             "--ref", "HEAD", "--committed-only", "--root", str(root)],
            capture_output=True, text=True)
        self.assertEqual(out.returncode, 0, out.stderr)
        plan = json.loads(out.stdout)
        self.assertEqual(plan["crates"], [])
        self.assertFalse(plan["workspace"])


if __name__ == "__main__":
    unittest.main()
