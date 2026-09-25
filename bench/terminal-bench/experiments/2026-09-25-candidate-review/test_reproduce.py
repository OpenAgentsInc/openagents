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
