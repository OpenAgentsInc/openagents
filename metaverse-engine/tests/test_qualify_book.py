#!/usr/bin/env python3
"""The evidence cache refuses drift and incomplete or unpublished proofs."""
import copy
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('qualify_book', Path(__file__).with_name('qualify_book.py'))
q = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(q)


class EvidenceCache(unittest.TestCase):
    def setUp(self):
        self.item = {'source': 'example.coax', 'stdout': 'example.out', 'exit_status': 0,
                     'modes': ['debug', 'release']}
        self.inputs = {'example.coax': 'source-hash', 'example.out': 'fixture-hash'}
        self.old = {'status': 'passed', 'inputs': self.inputs.copy(), 'item': copy.deepcopy(self.item),
                    'compiler_sha256': 'compiler',
                    'runs': [{'mode': mode, 'status': 'passed'} for mode in self.item['modes']]}

    def test_only_exact_complete_run_can_be_retained(self):
        self.assertTrue(q.reusable(self.old, self.inputs, self.item, 'compiler'))
        for field, value in [('status', 'failed'), ('inputs', dict(self.inputs, **{'example.out': 'changed'})),
                             ('compiler_sha256', 'changed'), ('runs', self.old['runs'][:1])]:
            old = dict(self.old, **{field: value})
            self.assertFalse(q.reusable(old, self.inputs, self.item, 'compiler'), field)
        self.assertFalse(q.reusable(self.old, self.inputs, dict(self.item, run_steps=100), 'compiler'))
        failed_mode = copy.deepcopy(self.old)
        failed_mode['runs'][1]['status'] = 'failed'
        self.assertFalse(q.reusable(failed_mode, self.inputs, self.item, 'compiler'))

    def test_legacy_evidence_requires_published_identity(self):
        with tempfile.TemporaryDirectory(dir=q.ROOT) as folder:
            path = Path(folder) / 'old.json'
            path.write_text(json.dumps({'schema': 1}))
            with self.assertRaisesRegex(ValueError, 'published'):
                q.previous_rows(path, None)
            with patch.object(q, 'execute', return_value=subprocess.CompletedProcess([], 0, b'changed', b'')):
                with self.assertRaisesRegex(ValueError, 'differs'):
                    q.previous_rows(path, 'published-commit')

    def test_transcript_exit_and_diagnostics_are_all_gates(self):
        item = dict(self.item, stdout='fixture')
        with patch.object(q.Path, 'read_bytes', return_value=b'expected'):
            for code, out, err in [(1, b'expected', b''), (0, b'wrong', b''), (0, b'expected', b'warning')]:
                with patch.object(q, 'execute', return_value=subprocess.CompletedProcess([], code, out, err)):
                    self.assertEqual(q.run_example(Path('coax'), item, 'release')['status'], 'failed')
            with patch.object(q, 'execute', return_value=subprocess.CompletedProcess([], 0, b'expected', b'')):
                self.assertEqual(q.run_example(Path('coax'), item, 'debug')['status'], 'passed')

    def test_timeout_is_a_failure_receipt(self):
        with patch.object(q, 'execute', side_effect=subprocess.TimeoutExpired(['coax'], 60)):
            result = q.run_example(Path('coax'), self.item, 'debug')
        self.assertEqual(result['status'], 'failed')
        self.assertEqual(result['timeout'], 60)

    def test_timeout_reaps_a_real_child_and_its_grandchild(self):
        with tempfile.TemporaryDirectory() as folder:
            pids = Path(folder) / 'pids'
            code = ('import os,subprocess,sys,time; '
                    'child=subprocess.Popen([sys.executable,"-c","import time; time.sleep(60)"]); '
                    'open(sys.argv[1],"w").write(str(os.getpid())+" "+str(child.pid)); '
                    'time.sleep(60)')
            with self.assertRaises(subprocess.TimeoutExpired):
                q.execute([sys.executable, '-c', code, str(pids)], 1)
            for pid in pids.read_text().split():
                result = subprocess.run(['ps', '-o', 'stat=', '-p', pid], capture_output=True, timeout=5)
                self.assertTrue(not result.stdout.strip() or result.stdout.strip().startswith(b'Z'),
                                f'compiler descendant {pid} is still running')


if __name__ == '__main__':
    unittest.main()
