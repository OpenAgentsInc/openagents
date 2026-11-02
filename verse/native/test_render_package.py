#!/usr/bin/env python3
"""Exercise native identity publication with an actual emitted package."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

from render_package import identity

PACKAGE = None


class IdentityChecks(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="aethyr-native-identity-")
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        self.package = self.directory / "render"
        shutil.copytree(PACKAGE.parent, self.package)
        self.manifest = self.package / "render.json"
        self.declaration = json.loads(self.manifest.read_text())

    def test_actual_package_identity_is_a_compilable_native_contract(self):
        d = self.declaration
        header = identity(self.manifest, d["app"], d["slug"])
        (self.directory / "RenderIdentity.h").write_bytes(header)
        (self.directory / "probe.c").write_text('#include "RenderIdentity.h"\n#include <stdio.h>\n#include <inttypes.h>\nint main(void){printf("%"PRId64"\\n%s\\n%s\\n%s\\n%s\\n",AETHYR_RENDER_PROFILE,AETHYR_RENDER_APP,AETHYR_RENDER_SLUG,AETHYR_RENDER_PACKAGE,AETHYR_RENDER_PACKAGE_SHA256);}\n')
        subprocess.run(["clang", "-std=c11", self.directory / "probe.c", "-o", self.directory / "probe"], check=True, capture_output=True, timeout=30)
        got = subprocess.run([self.directory / "probe"], check=True, capture_output=True, timeout=10).stdout.decode().splitlines()
        self.assertEqual(got, [str(d["profile_index"]), d["app"], d["slug"], "render/render.json", hashlib.sha256(self.manifest.read_bytes()).hexdigest()])

    def test_wrong_app_and_profile_are_refused(self):
        d = self.declaration
        for app, profile in (("other", d["slug"]), (d["app"], "other")):
            with self.assertRaises(ValueError): identity(self.manifest, app, profile)

    def test_every_product_class_is_authenticated(self):
        d = self.declaration
        for table, key in (("modules", "wgsl_file"), ("entries", "spirv_file"), ("entries", "tree_file")):
            product = self.package / d[table][0][key]
            original = product.read_bytes(); product.write_bytes(original + b"changed")
            with self.assertRaises(ValueError): identity(self.manifest, d["app"], d["slug"])
            product.write_bytes(original)

    def test_refusal_preserves_a_previously_published_header(self):
        output = self.directory / "RenderIdentity.h"; output.write_bytes(b"previous identity\n")
        driver = Path(__file__).with_name("render_package.py")
        r = subprocess.run(["python3", driver, "--package", self.manifest, "--app", "wrong", "--profile", self.declaration["slug"], "--output", output], capture_output=True, timeout=30)
        self.assertNotEqual(r.returncode, 0)
        self.assertEqual(output.read_bytes(), b"previous identity\n")
        self.assertFalse(output.with_suffix(".h.pending").exists())


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--package", type=Path, required=True)
    args, rest = parser.parse_known_args()
    PACKAGE = args.package.resolve()
    unittest.main(argv=[__file__, *rest])
