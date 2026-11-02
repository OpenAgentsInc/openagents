import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from render_stage import stage_package


class RenderStage(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="render-stage-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.app = self.root / "app"
        self.stage = self.root / "stage"
        self.source = self.app / "build/render/the-grid"
        self.source.mkdir(parents=True)
        self.products = {"m.wgsl": b"fn foo() {}\n", "p.spv": b"binary byte transport\x00", "p.tree": b"2 6 0 1 2 3 4 5\n"}
        for name, data in self.products.items():
            (self.source / name).write_bytes(data)
        self.manifest = dict(schema="aethyr.render.package.v1",
                             modules=[dict(wgsl_file="m.wgsl", wgsl_sha256=hashlib.sha256(self.products["m.wgsl"]).hexdigest())],
                             entries=[dict(spirv_file="p.spv", spirv_sha256=hashlib.sha256(self.products["p.spv"]).hexdigest(),
                                           tree_file="p.tree",tree_sha256=hashlib.sha256(self.products["p.tree"]).hexdigest())])
        self.write_manifest()

    def write_manifest(self):
        self.raw = json.dumps(self.manifest).encode()
        (self.source / "render.json").write_bytes(self.raw)

    def test_relative_products_and_manifest_are_byte_identical_in_both_trees(self):
        url = stage_package(self.app, "/build/render/the-grid/render.json", self.stage)
        self.assertEqual(url, "/build/render/" + hashlib.sha256(self.raw).hexdigest() + "/render.json")
        for tree in ("assets", "site"):
            manifest = self.stage / tree / url.lstrip("/")
            self.assertEqual(manifest.read_bytes(), self.raw)
            for name, data in self.products.items():
                self.assertEqual((manifest.parent / name).read_bytes(), data)

    def test_changed_product_is_refused_before_copying(self):
        (self.source / "m.wgsl").write_bytes(b"wrong bytes")
        with self.assertRaisesRegex(ValueError, "hash differs"):
            stage_package(self.app, "/build/render/the-grid/render.json", self.stage)
        self.assertFalse(self.stage.exists())

    def test_paths_and_unknown_schema_refuse(self):
        for url in ("/build/render/../render.json", "/elsewhere/render.json", "https://example.com/render.json"):
            with self.assertRaisesRegex(ValueError, "unsafe"):
                stage_package(self.app, url, self.stage)
        self.manifest["modules"][0]["wgsl_file"] = "../secret.wgsl"
        self.write_manifest()
        with self.assertRaisesRegex(ValueError, "sibling"):
            stage_package(self.app, "/build/render/the-grid/render.json", self.stage)
        self.manifest["schema"] = "future"
        self.write_manifest()
        with self.assertRaisesRegex(ValueError, "schema"):
            stage_package(self.app, "/build/render/the-grid/render.json", self.stage)


if __name__ == "__main__":
    unittest.main()
