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


def fake_metadata(root, packages, deps, resolve=None, paths=None):
    """A `verify_changed.Metadata` with cargo output supplied in-process.

    `packages` are member names; `deps` maps a member to the names it
    depends on directly; `resolve` maps any package name to its resolved
    dependency names; `paths` maps a non-member path package to its
    manifest directory.
    """
    members = [{"id": f"{n} 0.1.0", "name": n,
                "manifest_path": str(root / "crates" / n / "Cargo.toml"),
                "dependencies": [{"name": d} for d in deps.get(n, [])]}
               for n in packages]
    direct = {"workspace_members": [m["id"] for m in members],
              "packages": members}
    extra = [{"id": f"{n} 1.0.0", "name": n, "source": None,
              "manifest_path": str(root / d / "Cargo.toml")}
             for n, d in (paths or {}).items()]
    names = set(packages) | set(resolve or {}) | {
        d for ds in (resolve or {}).values() for d in ds}
    ids = {n: f"{n} 0.1.0" if n in packages else f"{n} 1.0.0" for n in names}
    third = [{"id": ids[n], "name": n, "source": "registry",
              "manifest_path": "/registry/" + n + "/Cargo.toml"}
             for n in names if n not in packages and n not in (paths or {})]
    full = {"workspace_members": direct["workspace_members"],
            "packages": members + extra + third,
            "resolve": {"nodes": [
                {"id": ids[n], "deps": [{"pkg": ids[d]} for d in ds]}
                for n, ds in (resolve or {}).items()]}}
    return verify_changed.Metadata(root, direct=direct, full=full)


class NestedAndDependentTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.write("Cargo.toml", ROOT_MANIFEST)
        self.write("crates/shared/Cargo.toml", '[package]\nname = "shared"\n')
        self.write("crates/app/Cargo.toml", '[package]\nname = "app"\n')
        self.write("crates/psionic/Cargo.toml",
                   '[workspace]\nmembers = ["crates/*"]\n')
        self.write("crates/psionic/crates/psionic-serve/Cargo.toml",
                   '[package]\nname = "psionic-serve"\n')
        self.write("crates/openagents-mobile/Cargo.toml",
                   '[package]\nname = "openagents-mobile"\n')
        self.write("vendor/boltz-client/crates/lib/Cargo.toml",
                   '[package]\nname = "boltz-client"\n')
        self.metadata = fake_metadata(
            self.root, ["shared", "app", "wallet"],
            {"app": ["shared"], "wallet": ["boltz-client", "bech32"]},
            resolve={"app": ["shared", "serde"], "shared": ["serde"],
                     "wallet": ["boltz-client", "bech32"],
                     "boltz-client": ["serde"], "bech32": [], "serde": []},
            paths={"boltz-client": "vendor/boltz-client/crates/lib"})

    def tearDown(self):
        self.tmp.cleanup()

    def write(self, path, text):
        target = self.root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text)

    def resolve(self, paths, base=None):
        return verify_changed.resolve(
            self.root, paths, read_base=(base or {}).get,
            metadata=self.metadata)

    def test_nested_workspace_paths_use_their_own_manifest(self):
        plan = self.resolve([
            "crates/psionic/crates/psionic-serve/src/lib.rs",
            "crates/openagents-mobile/src/lib.rs",
        ])
        self.assertEqual(plan["crates"], [])
        self.assertEqual(plan["nested"], [
            {"manifest": "crates/openagents-mobile/Cargo.toml",
             "packages": ["openagents-mobile"], "workspace": False},
            {"manifest": "crates/psionic/Cargo.toml",
             "packages": ["psionic-serve"], "workspace": False},
        ])

    def test_a_nested_root_manifest_marks_that_workspace_only(self):
        plan = self.resolve(["crates/psionic/Cargo.lock"])
        self.assertFalse(plan["workspace"])
        self.assertEqual(plan["nested"][0]["manifest"], "crates/psionic/Cargo.toml")
        self.assertTrue(plan["nested"][0]["workspace"])

    def test_a_directory_without_a_package_is_not_a_package_name(self):
        self.write("crates/notes/README.md", "prose\n")
        plan = self.resolve(["crates/notes/README.md"])
        self.assertEqual(plan["crates"], [])
        self.assertEqual(plan["uncovered"], ["crates/notes/README.md"])

    def test_patched_vendor_paths_map_to_their_users(self):
        plan = self.resolve(["vendor/boltz-client/crates/lib/src/lib.rs"])
        self.assertEqual(plan["crates"], ["wallet"])
        self.assertEqual(plan["uncovered"], [])

    def test_a_changed_crate_names_its_direct_dependents(self):
        plan = self.resolve(["crates/shared/src/lib.rs"])
        self.assertEqual(plan["crates"], ["shared"])
        self.assertEqual(plan["dependents"], ["app"])

    def test_a_lockfile_change_maps_to_the_members_using_the_package(self):
        before = LOCK.format(bech32="0.9.0")
        self.write("Cargo.lock", LOCK.format(bech32="0.11.0"))
        plan = self.resolve(["Cargo.lock"], {"Cargo.lock": before})
        self.assertEqual(plan["lock_packages"], ["bech32"])
        self.assertEqual(plan["crates"], ["wallet"])
        self.assertFalse(plan["workspace"])

    def test_a_lockfile_change_is_never_an_empty_plan(self):
        self.write("Cargo.lock", LOCK.format(bech32="0.11.0"))
        for base in ({}, {"Cargo.lock": LOCK.format(bech32="0.9.0")
                          .replace("bech32", "unknown-crate")}):
            plan = self.resolve(["Cargo.lock"], base)
            self.assertTrue(plan["crates"] or plan["workspace"], base)

    def test_a_workspace_dependency_edit_maps_to_its_users(self):
        after = ROOT_MANIFEST.replace('bech32 = "0.9"', 'bech32 = "0.11"')
        self.write("Cargo.toml", after)
        plan = self.resolve(["Cargo.toml"], {"Cargo.toml": ROOT_MANIFEST})
        self.assertEqual(plan["crates"], ["wallet"])
        self.assertFalse(plan["workspace"])

    def test_other_root_manifest_edits_still_widen(self):
        self.write("Cargo.toml", ROOT_MANIFEST + "\n[profile.release]\nlto = true\n")
        plan = self.resolve(["Cargo.toml"], {"Cargo.toml": ROOT_MANIFEST})
        self.assertTrue(plan["workspace"])


ROOT_MANIFEST = """[workspace]
members = ["crates/*"]
exclude = [
    # Its own workspace.
    "crates/openagents-mobile",
    "crates/psionic",
    "vendor",
]

[workspace.dependencies]
bech32 = "0.9"  # reviewed
serde = "1"
"""

LOCK = """version = 4

[[package]]
name = "bech32"
version = "{bech32}"

[[package]]
name = "serde"
version = "1.0.0"
"""


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
