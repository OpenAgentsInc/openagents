"""The public NIP catalog must match real sources and every generated route."""
import json
from pathlib import Path
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[3]
APP = ROOT/'apps/openagents-web'


class NipPages(unittest.TestCase):
    def test_catalog_covers_coax_and_production_relay(self):
        catalog = json.loads((APP/'nip/catalog.json').read_text())
        rows = catalog['pages']; numbers = [r['number'] for r in rows]
        self.assertEqual(numbers, sorted(set(numbers)))
        self.assertEqual(len(rows), 22)
        self.assertLessEqual(set(catalog['relay_supported_nips']), set(numbers))
        self.assertLessEqual({1,10,11,19,22,34,40,42,44,49,67,98}, set(numbers))
        for row in rows:
            with self.subTest(nip=row['number']):
                self.assertTrue((ROOT/f"nips/official/{row['number']:02}.md").is_file())
                for path in row['sources']+row['evidence']:
                    self.assertTrue((ROOT/path).exists(), path)
                self.assertEqual(len(row['cases']), 3)
                self.assertEqual(len(row['expected']), 3)
                self.assertTrue(row['gap'] and row['demo_scope'])
                if row['status']=='relay':
                    self.assertEqual(row['demo'], 'relay')
                    self.assertIn('does not execute', row['demo_scope'])

    def test_generated_pages_and_coax_are_current(self):
        subprocess.run(['python3', str(ROOT/'coax/web/nip-pages.py'), '--check'], check=True)
        for row in json.loads((APP/'nip/catalog.json').read_text())['pages']:
            route = APP/f"nip/{row['number']:02}"
            html = (route/'index.html').read_text(); md = (route/'index.md').read_text()
            self.assertIn('data-app="nip01"' if row['number']==1 else 'data-app="nip-guide"', html)
            self.assertIn('href="/nip"', html)
            self.assertIn(row['gap'], md)
            self.assertIn(row['demo_scope'], md)
            self.assertIn('## Implementation', md)


if __name__ == '__main__':
    unittest.main()
