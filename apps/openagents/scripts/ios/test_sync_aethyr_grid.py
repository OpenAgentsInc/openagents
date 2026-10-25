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
                data=b'\x00asm\x01\x00\x00\x00'+bytes(100_000) if name=='index.wasm' else b'owned-fixture';(root/name).write_bytes(data)
                files[name]={'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()}
            (root/'manifest.json').write_text(json.dumps({'format':'openagents.aethyr-grid.bundle.v1','repository':'https://github.com/OpenAgentsInc/aethyr-cpp','revision':'a'*40,'files':files}))
            bundle.check(root)
            (root/'index.wasm').write_bytes(b'changed-bytes')
            with self.assertRaisesRegex(ValueError,'hash mismatch'):bundle.check(root)

    def test_matching_hashes_do_not_admit_a_linker_placeholder(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            import shutil
            shutil.copytree(bundle.DEST,root,dirs_exist_ok=True)
            data=b'\x00asm\x01\x00\x00\x00'
            (root/'index.wasm').write_bytes(data)
            manifest=json.loads((root/'manifest.json').read_text())
            manifest['files']['index.wasm']={'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()}
            (root/'manifest.json').write_text(json.dumps(manifest))
            with self.assertRaisesRegex(ValueError,'Incomplete Grid WebAssembly'):bundle.check(root)
