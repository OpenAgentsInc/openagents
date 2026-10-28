"""Compatibility/migration/alpha conformance against newly compiled schemas."""
import math
from pathlib import Path
import unittest
from google.protobuf import descriptor_pb2
from openagents.engine.v1.assets_pb2 import AssetProfileCatalog
from openagents.verse.presence.v1alpha1.presence_pb2 import PresenceSnapshot
from openagents.metareal.home.v1alpha1.home_pb2 import HomeConfiguration
from compatibility import check, parse
from protocol import ROOT, REGISTRY, CONFIG, canonical_id, migrate_catalog, check_profile
from validate import decode_presence, validate


class ProtocolTests(unittest.TestCase):
    def test_home_profile_and_explicit_zero(self):
        value = HomeConfiguration(schema_version=1, seed=42727, mote_count=0, satellite_count=0)
        value.start.id = 'home.start'
        value.start.label = 'Start'
        value.start.enabled = value.start.shown = True
        validate(value)
        value.ClearField('mote_count')
        with self.assertRaises(ValueError):
            validate(value)

    def snapshot(self):
        value = PresenceSnapshot(observed_at_ms=0)
        value.world.world_id = 'grid'
        value.world.revision = 0
        peer = value.peers.add(actor_id='peer-1', sequence=0, heading_rad=0)
        peer.position.SetInParent()
        return value

    def test_zero_is_present_not_missing(self):
        value = self.snapshot()
        self.assertTrue(decode_presence(value.SerializeToString()).peers[0].HasField('sequence'))
        value.peers[0].ClearField('sequence')
        with self.assertRaises(ValueError):
            validate(value)

    def test_required_message_and_identifier(self):
        for mutate in (lambda v: v.ClearField('world'), lambda v: v.world.ClearField('world_id'),
                       lambda v: setattr(v.world, 'world_id', ''),
                       lambda v: setattr(v.world, 'world_id', 'grid\0'),
                       lambda v: v.peers[0].ClearField('position')):
            value = self.snapshot()
            mutate(value)
            with self.assertRaises(ValueError):
                validate(value)

    def test_limits_and_coordinates(self):
        for mutate in (lambda v: setattr(v.world, 'world_id', 'é'*65),
                       lambda v: setattr(v.peers[0].position, 'x', math.nan),
                       lambda v: setattr(v.peers[0].position, 'y', math.inf),
                       lambda v: setattr(v.peers[0], 'heading_rad', 4)):
            value = self.snapshot()
            mutate(value)
            with self.assertRaises(ValueError):
                validate(value)
        with self.assertRaises(ValueError):
            decode_presence(b'\0' * 262145)
        with self.assertRaises(ValueError):
            decode_presence(b'\x0a\xff')

    def test_duplicate_and_excess_peers(self):
        value = self.snapshot()
        value.peers.add().CopyFrom(value.peers[0])
        with self.assertRaises(ValueError):
            validate(value)
        for i in range(256):
            value.peers.add(actor_id=str(i), sequence=0).position.SetInParent()
        with self.assertRaises(ValueError):
            validate(value)

    def test_alpha_unknown_fields_remain_transient(self):
        data = self.snapshot().SerializeToString() + b'\x98\x06\x01'
        decoded = decode_presence(data)
        self.assertIn(b'\x98\x06\x01', decoded.SerializeToString())

    def test_real_legacy_catalog_round_trip(self):
        original_bytes = (ROOT/'compatibility/asset-profiles-v1-20261027.pb').read_bytes()
        old = AssetProfileCatalog.FromString(original_bytes)
        new = migrate_catalog(old)
        names = [p.id for p in new.profiles]
        self.assertIn('metareal-grid-v1', names)
        self.assertIn('metareal-town-v1', names)
        self.assertEqual(migrate_catalog(new), new)
        self.assertEqual(old.SerializeToString(deterministic=True), original_bytes)
        for before, after in zip(old.profiles, new.profiles):
            self.assertEqual(before.files, after.files)
            self.assertEqual(before.scene_id, after.scene_id)

    def test_aliases_are_exact_and_do_not_rewrite_signatures(self):
        for row in REGISTRY['entries']:
            if row['kind'] == 'signature-domain':
                with self.assertRaises(ValueError):
                    canonical_id(row['legacy_id'])
            else:
                self.assertEqual(canonical_id(row['legacy_id']), row['canonical_id'])
                self.assertEqual(canonical_id(row['canonical_id']), row['canonical_id'])
        with self.assertRaises(ValueError):
            canonical_id('not-a-registered-id')

    def test_catalog_collision_and_unknown_evidence(self):
        old = AssetProfileCatalog.FromString((ROOT/'compatibility/asset-profiles-v1-20261027.pb').read_bytes())
        canonical = migrate_catalog(old)
        old.profiles.add().CopyFrom(canonical.profiles[0])
        with self.assertRaises(ValueError):
            migrate_catalog(old)
        data = canonical.SerializeToString() + b'\x98\x06\x01'
        copied = migrate_catalog(AssetProfileCatalog.FromString(data))
        self.assertIn(b'\x98\x06\x01', copied.SerializeToString())

    def test_breaking_presence_or_field_type_is_rejected(self):
        baseline = parse((ROOT/'compatibility/host-reach-v1-20261027.descriptor.pb').read_bytes())
        candidate = descriptor_pb2.FileDescriptorSet()
        candidate.CopyFrom(baseline)
        field = next(f for file in candidate.file for msg in file.message_type for f in msg.field)
        field.type = descriptor_pb2.FieldDescriptorProto.TYPE_BYTES if field.type != 12 else 9
        self.assertTrue(check(baseline, candidate))
        candidate.CopyFrom(baseline)
        field = next(f for file in candidate.file for msg in file.message_type for f in msg.field if f.proto3_optional)
        field.proto3_optional = False
        self.assertTrue(check(baseline, candidate))

    def test_consumer_profiles_are_selected(self):
        for name, roots in CONFIG['profiles'].items():
            self.assertTrue(roots)
            self.assertTrue(all(path.startswith(('openagents/', 'nostr/')) for path in roots), name)
        self.assertFalse(any('third_party' in root for root in CONFIG['profiles']['metareal']))

    def test_foreign_dependency_cannot_enter_engine_profile(self):
        descriptor = descriptor_pb2.FileDescriptorSet()
        for root in CONFIG['profiles']['metareal']:
            descriptor.file.add(name=root, package='openagents.engine.v1')
        descriptor.file.add(name='lightning.proto', package='lnrpc')
        with self.assertRaises(ValueError):
            check_profile('metareal', descriptor)
