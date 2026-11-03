#!/usr/bin/env python3
# Firm reason: 2 (conformance fixtures for the independent Python pin-policy oracle; production checks are Coax)
"""check_pins.py refuses each kind of disagreement (Bitcoin chapter 116)."""
import copy
import json
from pathlib import Path
import unittest

import check_pins as cp

HERE = Path(__file__).resolve().parent


class PinTests(unittest.TestCase):
    def setUp(self):
        self.manifest = json.loads((HERE / 'manifest.json').read_text())
        self.pins = json.loads((HERE / 'pins.json').read_text())
        self.sources = (HERE / 'SOURCES.md').read_text()

    def core_entry(self, manifest):
        target = 'git:' + self.pins['targets'][cp.CORE]['revision']
        return next(e for e in manifest['files'] if cp.repository(e['url']) == cp.CORE and e['revision'] == target)

    def test_the_committed_pins_agree(self):
        problems, report = cp.core_problems(self.manifest, self.pins)
        self.assertEqual(problems, [])
        self.assertEqual([r[0] for r in report], ['upstream/bitcoin-core/chainparams_v22.cpp'])
        self.assertEqual(cp.inventory_problems(self.manifest, self.sources), [])
        self.assertEqual(cp.index_problems(cp.REPO, self.manifest, self.sources), [])

    def test_a_core_file_at_another_revision_is_reported(self):
        manifest = copy.deepcopy(self.manifest)
        entry = self.core_entry(manifest)
        entry['revision'] = 'git:' + '1' * 40
        entry['url'] = entry['url'].replace(self.pins['targets'][cp.CORE]['revision'], '1' * 40)
        problems, report = cp.core_problems(manifest, self.pins)
        self.assertEqual(len(problems), 1)
        self.assertIn(entry['path'], problems[0])
        self.assertIn((entry['path'], '11111111', None), report)

    def test_an_exception_needs_a_reason_and_a_pinned_file(self):
        pins = copy.deepcopy(self.pins)
        pins['targets'][cp.CORE]['exceptions']['upstream/bitcoin-core/chainparams_v22.cpp'] = ' '
        self.assertEqual(len(cp.core_problems(self.manifest, pins)[0]), 1)
        pins = copy.deepcopy(self.pins)
        pins['targets'][cp.CORE]['exceptions']['upstream/bitcoin-core/gone.cpp'] = 'a reason'
        self.assertIn('does not pin', cp.core_problems(self.manifest, pins)[0][0])
        pins = copy.deepcopy(self.pins)
        pins['targets'][cp.CORE]['exceptions'][self.core_entry(self.manifest)['path']] = 'a reason'
        self.assertIn('at the target', cp.core_problems(self.manifest, pins)[0][0])

    def test_inventory_rows_are_exact(self):
        entry = self.core_entry(self.manifest)
        row = next(l for l in self.sources.splitlines() if l.startswith('| [`%s`]' % entry['path']))
        self.assertIn('0 inventory rows', cp.inventory_problems(self.manifest, self.sources.replace(row + '\n', ''))[0])
        self.assertIn('2 inventory rows', cp.inventory_problems(self.manifest, self.sources.replace(row, row + '\n' + row))[0])
        changed = row.replace(entry['sha256'], '0' * 64)
        self.assertIn('another SHA-256', cp.inventory_problems(self.manifest, self.sources.replace(row, changed))[0])
        manifest = copy.deepcopy(self.manifest)
        manifest['files'] = [e for e in manifest['files'] if e['path'] != entry['path']]
        self.assertIn('does not pin', cp.inventory_problems(manifest, self.sources)[0])

    def test_a_citation_of_an_unpinned_file_fails(self):
        cited = {('upstream/bip340/test-vectors.csv', 'bitcoin/book/a.md'), ('upstream/bip340', 'bitcoin/book/b.md')}
        self.assertEqual(cp.citation_problems(self.manifest, cited), [])
        cited.add(('upstream/bip340/edited.csv', 'bitcoin/book/c.md'))
        self.assertEqual(len(cp.citation_problems(self.manifest, cited)), 1)

    def test_a_stale_index_fails(self):
        stale = self.sources.replace('- Bitcoin chapter 19: ', '- Bitcoin chapter 19: `upstream/extra`, ', 1)
        self.assertNotEqual(stale, self.sources)
        self.assertIn('stale', cp.index_problems(cp.REPO, self.manifest, stale)[0])
        missing = self.sources.replace(cp.INDEX_BEGIN, '')
        self.assertIn('no pins-by-chapter', cp.index_problems(cp.REPO, self.manifest, missing)[0])


if __name__ == '__main__':
    unittest.main()
