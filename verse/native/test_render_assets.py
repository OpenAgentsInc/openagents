import json
from pathlib import Path
import tempfile
import unittest

from render_assets import stage


class AssetStagingChecks(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="aethyr-asset-stage-"); self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name); self.web = self.root / "web"; self.web.mkdir()
        self.output = self.root / "out"; self.package = self.root / "render.json"; self.table = self.root / "table.json"
        self.identity = dict(profile_index=7, slug="fixture", app="fixture"); self.package.write_text(json.dumps(self.identity))

    def write_table(self, rows):
        self.table.write_text(json.dumps(dict(schema="aethyr.render.assets.v1", **self.identity, rows=rows)))

    def test_expanded_pack_rows_retain_identity_and_copy_the_container_once(self):
        (self.web / "world.pack").write_bytes(b"container")
        rows = [dict(row=i, pack_part=i+1, initial=True, gpu_required=True, urls=["/world.pack"]) for i in range(4)]
        self.write_table(rows); receipt = stage(self.table, self.package, self.web, self.output)
        self.assertEqual(len(receipt["files"]), 1)
        self.assertEqual((self.output / "world.pack").read_bytes(), b"container")
        self.assertEqual((self.output / "assets.json").read_bytes(), self.table.read_bytes())

    def test_core_flags_control_copy_and_unrelated_texture_observations_survive(self):
        (self.web / "needed.bin").write_bytes(b"needed")
        self.write_table([dict(initial=True, gpu_required=False, urls=["/needed.bin"]), dict(initial=False, gpu_required=False, urls=["/not-local-stream"])])
        self.output.mkdir(); (self.output / "textures.config").write_bytes(b"measured mips")
        stage(self.table, self.package, self.web, self.output)
        self.assertEqual((self.output / "textures.config").read_bytes(), b"measured mips")
        self.assertFalse((self.output / "not-local-stream").exists())

    def test_invalid_or_missing_inputs_preserve_previous_published_observations(self):
        self.output.mkdir(); (self.output / "assets.json").write_bytes(b"previous")
        for url in ("/../outside", "//outside", "/a/./b", "/a?b", "/missing"):
            self.write_table([dict(initial=True, gpu_required=True, urls=[url])])
            with self.assertRaises((ValueError, FileNotFoundError)): stage(self.table, self.package, self.web, self.output)
            self.assertEqual((self.output / "assets.json").read_bytes(), b"previous")

    def test_profile_mismatch_is_refused_before_publication(self):
        self.write_table([]); self.package.write_text(json.dumps(dict(self.identity, profile_index=8)))
        with self.assertRaises(ValueError): stage(self.table, self.package, self.web, self.output)
        self.assertFalse(self.output.exists())


if __name__ == "__main__": unittest.main()
