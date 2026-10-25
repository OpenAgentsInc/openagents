"""Check the shipped Grid pin and reject changed packaged bytes."""
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec=importlib.util.spec_from_file_location('grid_bundle',Path(__file__).with_name('sync-aethyr-grid.py'))
bundle=importlib.util.module_from_spec(spec);spec.loader.exec_module(bundle)


class GridBundleTest(unittest.TestCase):
    def test_the_shipped_grid_matches_its_source_pin(self):
        result=bundle.check(bundle.DEST)
        self.assertEqual(len(result['revision']),40)
        self.assertGreater(result['files']['index.wasm']['bytes'],100_000)

    def test_changed_asset_bytes_are_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);files={}
            for name in bundle.FILES+['Info.plist']:
                data=b'owned-fixture';(root/name).write_bytes(data)
                files[name]={'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()}
            (root/'manifest.json').write_text(json.dumps({'format':'openagents.aethyr-grid.bundle.v1','repository':'https://github.com/OpenAgentsInc/aethyr-cpp','revision':'a'*40,'files':files}))
            bundle.check(root)
            (root/'index.wasm').write_bytes(b'changed-bytes')
            with self.assertRaisesRegex(ValueError,'hash mismatch'):bundle.check(root)
