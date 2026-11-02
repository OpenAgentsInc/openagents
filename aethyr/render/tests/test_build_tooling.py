#!/usr/bin/env python3
"""Poison retained emitted products, without rebuilding unchanged shaders.

  python3 test_build_tooling.py --package /path/to/qualified/profile
"""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import shutil
import struct
import tempfile
import unittest

from check_build import HERE, verify_package

PACKAGE = None
SNAPSHOT = json.loads((HERE / "catalog-6445ee0050.json").read_text())


class PackageChecks(unittest.TestCase):
    def check_poison(self, poison):
        with tempfile.TemporaryDirectory(prefix="aethyr-build-control-") as name:
            dest = Path(name) / "package"
            shutil.copytree(PACKAGE, dest)
            manifest = dest / "render.json"
            data = json.loads(manifest.read_text())
            poison(dest, data)
            manifest.write_text(json.dumps(data))
            with self.assertRaises(AssertionError):
                verify_package(dest, SNAPSHOT)

    def test_metadata_contract_and_complete_table(self):
        for field, value in (("name", "wrong"), ("slot", 1), ("native_kind", 2), ("shape", 1), ("stride", 8), ("stages", 1)):
            with self.subTest(field=field):
                self.check_poison(lambda directory, data: data["entries"][0]["uses"][0].__setitem__(field, value))
        self.check_poison(lambda directory, data: data["entries"].pop())
        self.check_poison(lambda directory, data: data.__setitem__("slug", "wrong-profile"))

    def test_vertex_offsets_are_observed(self):
        def poison(directory, data):
            entry = next(row for row in data["entries"] if row["attributes"])
            entry["attributes"][0]["offset"] += 4
        self.check_poison(poison)

    def test_native_capability_is_read_from_rehashed_binary(self):
        def poison(directory, data):
            entry = data["entries"][0]
            path = directory / entry["spirv_file"]
            original = path.read_bytes()
            words = list(struct.unpack("<" + str(len(original) // 4) + "I", original))
            at, changed = 5, False
            while at < len(words):
                count = words[at] >> 16
                if words[at] & 65535 == 17 and words[at + 1] == 11:
                    words[at + 1] = 1
                    changed = True
                    break
                at += count
            self.assertTrue(changed)
            modified = struct.pack("<" + str(len(words)) + "I", *words)
            path.write_bytes(modified)
            entry["spirv_sha256"] = hashlib.sha256(modified).hexdigest()
        self.check_poison(poison)

    def test_module_entry_is_read_from_rehashed_wgsl(self):
        def poison(directory, data):
            entry = data["entries"][0]
            module = next(row for row in data["modules"] if row["binding_set"] == entry["binding_set"])
            path = directory / module["wgsl_file"]
            original = path.read_bytes()
            modified = original.replace(("fn " + entry["wgsl_vertex"] + "(").encode(), b"fn planted_wrong_entry(")
            self.assertNotEqual(original, modified)
            path.write_bytes(modified)
            module["wgsl_sha256"] = hashlib.sha256(modified).hexdigest()
        self.check_poison(poison)

    def test_reflection_output_and_profiles_are_observed(self):
        for field, value in (("fragment_targets", 2), ("fragment_depth", True), ("native_int64", False), ("derivatives", 2), ("control", 2)):
            with self.subTest(field=field):
                self.check_poison(lambda directory, data: data["entries"][0].__setitem__(field, value))

    def test_source_hash_and_product_hash_are_observed(self):
        self.check_poison(lambda directory, data: data["source_inputs"][0].__setitem__("sha256", "0" * 64))
        self.check_poison(lambda directory, data: data["entries"][0].__setitem__("spirv_sha256", "0" * 64))
        self.check_poison(lambda directory, data: data["modules"][0].__setitem__("wgsl_sha256", "0" * 64))
        self.check_poison(lambda directory, data: data["entries"][0].__setitem__("tree_sha256", "0" * 64))

    def test_rehashed_tree_version_and_word_bounds_are_observed(self):
        def poison(directory, data, version=False):
            entry = data["entries"][0]
            path = directory / entry["tree_file"]
            words = path.read_bytes().split()
            if version:
                words[0] = b"0"
            else:
                words.append(b"4294967296")
            modified = b" ".join(words)
            path.write_bytes(modified)
            entry["tree_sha256"] = hashlib.sha256(modified).hexdigest()
        self.check_poison(lambda directory, data: poison(directory, data, True))
        self.check_poison(poison)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--package", type=Path, required=True)
    args = parser.parse_args()
    PACKAGE = args.package.resolve()
    unittest.main(argv=[__file__], verbosity=2)
