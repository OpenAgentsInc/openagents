import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'generated/python'))
from portable import decode_scene, validate_manifest, verify_files
from openagents.engine.v1.scene_pb2 import AssetManifest
import json


class PortableTest(unittest.TestCase):
    def test_shared_corpus(self):
        folder = ROOT / 'fixtures/portable'
        for name, valid in json.loads((folder/'cases.json').read_text()).items():
            with self.subTest(name=name):
                def decode():
                    data = (folder/name).read_bytes()
                    return validate_manifest(AssetManifest.FromString(data)) if name.startswith('manifest') else decode_scene(data)
                if valid:
                    decode()
                else:
                    with self.assertRaises(ValueError): decode()

    def test_manifest_binds_exact_files(self):
        folder = ROOT / 'fixtures/portable'
        manifest = AssetManifest.FromString((folder/'manifest.pb').read_bytes())
        verify_files(manifest, folder)
        manifest.artifacts[0].sha256 = bytes(32)
        with self.assertRaisesRegex(ValueError, 'hash mismatch'): verify_files(manifest, folder)
