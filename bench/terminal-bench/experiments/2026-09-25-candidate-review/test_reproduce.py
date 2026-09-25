"""The snapshot reviewer must refuse unsafe or unattributable evidence."""
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest

import reproduce


class RestoreTests(unittest.TestCase):
    def archive(self, root, name, kind=tarfile.REGTYPE):
        path = root / 'candidate.tar.gz'
        with tarfile.open(path, 'w:gz') as tf:
            entry = tarfile.TarInfo(name)
            entry.type = kind
            if kind == tarfile.REGTYPE:
                entry.size = 4
            tf.addfile(entry, io.BytesIO(b'test') if entry.size else None)
        return path

    def test_rejects_escaping_paths_and_links(self):
        for name, kind in [('../outside', tarfile.REGTYPE), ('/app/file', tarfile.REGTYPE),
                           ('app/link', tarfile.SYMTYPE), ('app/hard', tarfile.LNKTYPE)]:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                with self.assertRaises(ValueError):
                    reproduce.restore(self.archive(root, name, kind), root / 'restored')
                self.assertFalse((root / 'restored').exists())

    def test_retains_regular_files(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            reproduce.restore(self.archive(root, 'app/file'), root / 'restored')
            self.assertEqual((root / 'restored/app/file').read_bytes(), b'test')

    def test_declared_public_files_do_not_allow_other_paths_or_links(self):
        for name, kind in [('paper/other.json', tarfile.REGTYPE),
                           ('paper/examples.json', tarfile.SYMTYPE),
                           ('paper/../outside', tarfile.REGTYPE)]:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                with self.assertRaises(ValueError):
                    reproduce.restore(self.archive(root, name, kind), root / 'restored', ['paper/examples.json'])
                self.assertFalse((root / 'restored').exists())

    def test_exact_public_snapshot_files_are_restored_and_bound_to_image_hashes(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            archive = root / 'candidate.tar.gz'
            with tarfile.open(archive, 'w:gz') as tf:
                for name in ['app/main.py', 'paper/examples.json']:
                    info = tarfile.TarInfo(name)
                    info.size = 4
                    tf.addfile(info, io.BytesIO(b'test'))
            reproduce.restore(archive, root / 'restored', ['paper/examples.json'])
            expected = {'/paper/examples.json': reproduce.sha(root / 'restored/paper/examples.json')}
            valid = expected['/paper/examples.json'] + '  /paper/examples.json\n'
            self.assertTrue(reproduce.public_files_match(expected, valid))
            self.assertFalse(reproduce.public_files_match(expected, valid.replace('examples.json', 'other.json')))
            self.assertFalse(reproduce.public_files_match(expected, '0' * 64 + valid[64:]))
            self.assertFalse(reproduce.public_files_match(expected, valid + valid))

    def test_one_primary_dispatch_can_reach_snapshot_validation(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            path = root / 'agent/episode/artifacts/composition.json'
            path.parent.mkdir(parents=True)
            path.write_text(json.dumps({'branches': [{'role': 'primary'}]}))
            # The next missing input is the snapshot, not a fictitious later writer.
            with self.assertRaises(FileNotFoundError):
                reproduce.snapshot(root, root / 'restore')

    def test_does_not_inherit_snapshot_after_another_writer(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            path = root / 'agent/episode/artifacts/composition.json'
            path.parent.mkdir(parents=True)
            path.write_text(json.dumps({'repair': {'ran': True}}))
            with self.assertRaisesRegex(ValueError, 'another writer'):
                reproduce.snapshot(root, root / 'restore')


if __name__ == '__main__':
    unittest.main()
