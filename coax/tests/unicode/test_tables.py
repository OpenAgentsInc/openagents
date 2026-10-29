#!/usr/bin/env python3
"""Pinned-source integrity and deterministic generation, not algorithm evidence."""
if not __debug__:raise RuntimeError('tests require Python assertions')
import importlib.util,shutil,tempfile,unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
spec=importlib.util.spec_from_file_location('unicode_tables',ROOT/'tools/coax-unicode/tables.py');tables=importlib.util.module_from_spec(spec);spec.loader.exec_module(tables)
class Tables(unittest.TestCase):
 def test_regeneration(self):
  generated,stats=tables.generate(tables.read_pinned());self.assertEqual(tables.TARGET.read_text(),generated);self.assertEqual(stats['version'],'18.0.0');self.assertEqual(stats['max_decomposition'],18)
 def test_missing_tampered_and_unlisted_sources_refuse(self):
  with tempfile.TemporaryDirectory()as td:
   folder=Path(td)/'sources';shutil.copytree(tables.CORPUS,folder)
   target=folder/'UnicodeData.txt.gz';original=target.read_bytes();target.write_bytes(original[:-1]+bytes([original[-1]^1]))
   with self.assertRaisesRegex(ValueError,'checksum'):tables.read_pinned(folder)
   target.write_bytes(original);target.unlink()
   with self.assertRaises(FileNotFoundError):tables.read_pinned(folder)
   target.write_bytes(original);(folder/'extra.txt').write_text('unlisted')
   with self.assertRaisesRegex(ValueError,'unlisted'):tables.read_pinned(folder)
 def test_data_model_covers_exclusions_hangul_and_compatibility(self):
  m=tables.model(tables.read_pinned());self.assertEqual(m['decomposition'][0xfdfa],(0x635,0x644,0x649,0x20,0x627,0x644,0x644,0x647,0x20,0x639,0x644,0x64a,0x647,0x20,0x648,0x633,0x644,0x645))
  self.assertEqual(m['decomposition'][0x212b],(0x41,0x30a));self.assertEqual(m['composition'][(0x41<<21)|0x30a],0xc5)
  self.assertNotIn((0x915<<21)|0x93c,m['composition']);self.assertIn(0xac00,m['assigned']);self.assertEqual(m['ccc'][0x315],232)
if __name__=='__main__':unittest.main()
