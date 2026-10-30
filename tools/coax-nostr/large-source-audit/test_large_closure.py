#!/usr/bin/env python3
"""Adversarial physical-closure checks for the provisional large profile."""
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from aggregate import AggregateAudit
from check import ROOT


class LargeClosure(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix='coax-large-closure-')
        cls.directory = Path(cls.temp.name)
        cls.tool = AggregateAudit()

    @classmethod
    def tearDownClass(cls):
        cls.tool.close()
        cls.temp.cleanup()

    def graph(self, units, branch=False):
        self.assertGreaterEqual(units, 2)
        for i in range(units - 1):
            body = 'if x==0 {return 1;} return 2;' if branch and i == units - 2 else 'return x ^ 1;'
            (self.directory / f'unit{i}.coax').write_text(f'helper{i} :: (x:u64)->u64 {{{body}}}\n')
        root = self.directory / 'root.coax'
        loads = ''.join(f'#load "unit{i}.coax";\n' for i in range(units - 1))
        root.write_text(loads + f'candidate :: (x:u64)->u64 #export_c {{return helper{units-2}(x);}}\n')
        return root

    def test_old_limit_and_new_complete_last_unit(self):
        root = self.graph(27)
        old = subprocess.run([sys.executable, str(ROOT/'tools/coax-nostr/public-view-check/aggregate.py'),
                              str(root), '--entry', 'candidate'], capture_output=True, text=True)
        self.assertEqual(old.returncode, 1, old.stderr)
        self.assertEqual(json.loads(old.stdout)['error_name'], 'limit')
        report = self.tool.audit(root, 'candidate')
        self.assertTrue(report['accepted'], report)
        self.assertEqual(len(report['source_sha256']), 27)
        self.assertEqual(report['profile'], 'large-aggregate-source-v3')
        self.assertEqual(report['return_dependency_mask'], 1)

        root = self.graph(27, branch=True)
        report = self.tool.audit(root, 'candidate')
        self.assertFalse(report['accepted'], report)
        self.assertEqual(report['error_name'], 'secret-branch')
        self.assertTrue(report['file'].endswith('/unit25.coax'), report)

    def test_unit_cap_and_last_source_drift_fail_closed(self):
        root = self.graph(32)
        self.assertTrue(self.tool.audit(root, 'candidate')['accepted'])
        root = self.graph(33)
        self.assertEqual(self.tool.audit(root, 'candidate')['error_name'], 'limit')
        root = self.graph(27)
        last = self.directory / 'unit25.coax'
        def drift():
            last.write_text('helper25 :: (x:u64)->u64 {return x ^ 2;}\n')
        report = self.tool.audit(root, 'candidate', after_compile=drift)
        self.assertFalse(report['accepted'], report)
        self.assertEqual(report['error_name'], 'source-drift')


if __name__ == '__main__':
    unittest.main()
