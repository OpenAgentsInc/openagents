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

    def test_compiled_profiles_bind_exact_files_and_buffer_semantics(self):
        from portable import consumer_profile,validate_profile
        for identity in ('aethyr-grid-v1','aethyr-town-v1','openagents-grid-pack-v1'):
            profile=consumer_profile(identity)
            manifest=AssetManifest(schema_version=1,scene_id=profile.scene_id,profile=identity)
            for file in profile.files:
                asset=manifest.artifacts.add(id=file.id,path=file.path,format=file.format,dependencies=file.dependencies,byte_length=128,sha256=bytes(32))
                for buffer in file.buffers:
                    asset.buffers.add(semantic=buffer.semantic,byte_stride=buffer.byte_stride,byte_length=buffer.byte_stride,element_count=1)
            validate_profile(manifest)
            manifest.artifacts[0].path='wrong-file.bin'
            with self.assertRaisesRegex(ValueError,'required file'):validate_profile(manifest)
        with self.assertRaisesRegex(ValueError,'Unknown'):consumer_profile('unknown-profile')


    def test_published_profile_requirements_are_immutable(self):
        from profiles import check_compatibility
        from openagents.engine.v1.assets_pb2 import AssetProfileCatalog
        old=AssetProfileCatalog.FromString((ROOT/'data/profiles.pb').read_bytes())
        new=AssetProfileCatalog();new.CopyFrom(old)
        new.profiles[0].files[0].path='different.pb'
        with self.assertRaisesRegex(ValueError,'Published asset profile'):check_compatibility(old,new)
        new.CopyFrom(old);new.profiles.add().CopyFrom(old.profiles[0]);new.profiles[-1].id='new-profile-v2'
        check_compatibility(old,new)
