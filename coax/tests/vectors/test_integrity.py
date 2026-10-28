#!/usr/bin/env python3
"""Offline provenance/integrity checks, not algorithm conformance tests."""
from pathlib import Path
import hashlib
import json
import shutil
import tempfile
import unittest
from unittest.mock import patch

from check_vectors import audit, IntegrityError

HERE = Path(__file__).resolve().parent

class IntegrityTests(unittest.TestCase):
    def corpus(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        target = Path(temp.name) / 'vectors'
        shutil.copytree(HERE, target, ignore=shutil.ignore_patterns('__pycache__'))
        return target

    def manifest(self, root):
        return json.loads((root / 'manifest.json').read_text())

    def write(self, root, value):
        (root / 'manifest.json').write_text(json.dumps(value))

    def test_pinned_corpus_is_complete_and_offline(self):
        with patch('urllib.request.urlopen', side_effect=AssertionError('offline audit attempted network')):
            result = audit(HERE)
        self.assertGreaterEqual(result['upstream_files'], 10)
        self.assertGreater(result['bytes'], 10000)

    def test_source_catalog_covers_complete_bip340_and_extended_nip44(self):
        value = json.loads((HERE/'derived/catalog.json').read_text())
        self.assertEqual([x['id'] for x in value['bip340']], list(range(19)))
        self.assertTrue({0, 1, 17, 100}.issubset({x['message_bytes'] for x in value['bip340']}))
        self.assertEqual(value['wycheproof']['source_cases'], 752)
        self.assertEqual([x['plaintext_len'] for x in value['nip44_extended']], [65535, 65536, 65537])

    def test_generator_and_derived_output_drift_fail(self):
        root = self.corpus()
        with (root/'make_catalog.py').open('a') as out:
            out.write('\n# unexpected generator edit\n')
        with self.assertRaisesRegex(IntegrityError, 'generator'):
            audit(root)
        root = self.corpus(); value = self.manifest(root)
        item = next(x for x in value['files'] if x['kind'] == 'derived')
        path = root/item['path']; data = json.loads(path.read_text()); data['bip340'][0]['verification'] = False
        raw = json.dumps(data).encode(); path.write_bytes(raw)
        item['sha256'] = hashlib.sha256(raw).hexdigest(); item['bytes'] = len(raw)
        self.write(root, value)
        with self.assertRaisesRegex(IntegrityError, 'derived catalog'):
            audit(root)

    def test_tampered_upstream_bytes_fail(self):
        root = self.corpus(); item = self.manifest(root)['files'][0]
        path = root / item['path']; data = bytearray(path.read_bytes()); data[0] ^= 1
        path.write_bytes(data)
        with self.assertRaisesRegex(IntegrityError, 'digest'):
            audit(root)

    def test_missing_upstream_file_fails(self):
        root = self.corpus(); item = self.manifest(root)['files'][0]
        (root / item['path']).unlink()
        with self.assertRaisesRegex(IntegrityError, 'missing'):
            audit(root)

    def test_source_and_license_are_required(self):
        for field in ['url', 'revision', 'retrieved', 'license']:
            with self.subTest(field=field):
                root = self.corpus(); value = self.manifest(root)
                value['files'][0].pop(field)
                self.write(root, value)
                with self.assertRaisesRegex(IntegrityError, field):
                    audit(root)

    def test_mutable_git_url_is_refused(self):
        root = self.corpus(); value = self.manifest(root)
        entry = next(x for x in value['files'] if x['revision'].startswith('git:'))
        entry['url'] = entry['url'].replace(entry['revision'][4:], 'master')
        self.write(root, value)
        with self.assertRaisesRegex(IntegrityError, 'revision'):
            audit(root)

    def test_traversal_duplicate_and_symlink_are_refused(self):
        root = self.corpus(); value = self.manifest(root)
        value['files'][0]['path'] = '../outside'
        self.write(root, value)
        with self.assertRaisesRegex(IntegrityError, 'path'):
            audit(root)
        root = self.corpus(); value = self.manifest(root)
        value['files'].append(value['files'][0].copy())
        self.write(root, value)
        with self.assertRaisesRegex(IntegrityError, 'duplicate'):
            audit(root)
        root = self.corpus(); value = self.manifest(root)
        path = root / value['files'][0]['path']; backup = path.with_name(path.name+'.target')
        path.rename(backup); path.symlink_to(backup.name)
        with self.assertRaisesRegex(IntegrityError, 'symlink'):
            audit(root)

    def test_derived_source_mismatch_fails(self):
        root = self.corpus(); value = self.manifest(root)
        derived = next(x for x in value['files'] if x['kind'] == 'derived')
        derived['inputs'][0]['sha256'] = '0' * 64
        self.write(root, value)
        with self.assertRaisesRegex(IntegrityError, 'input'):
            audit(root)

    def test_unlisted_corpus_file_fails(self):
        root = self.corpus()
        (root/'upstream/unlisted.txt').write_text('unreviewed vector')
        with self.assertRaisesRegex(IntegrityError, 'unlisted'):
            audit(root)

if __name__ == '__main__':
    unittest.main()
