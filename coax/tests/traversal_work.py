#!/usr/bin/env python3
"""Aggregate traversal is separately metered from expression steps."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
COMPILER = Path(os.environ.get('COAX_TEST_COMPILER', ROOT / 'bin/coax'))
SOURCE = '''copy_many :: ()->int {
  a:[10000]u8; total:=0;
  for i:0..<100 { copy:=a; total+=cast(int)copy[0]; }
  return total;
}
A :: #run copy_many();
#assert A==0;
main :: () {}
'''

class TraversalTests(unittest.TestCase):
    def test_real_copies_are_counted_separately_from_steps(self):
        with tempfile.TemporaryDirectory(prefix='coax-traversal-') as d:
            source = Path(d) / 'case.coax'; source.write_text(SOURCE)
            report = Path(d) / 'timings.json'
            result = subprocess.run([str(COMPILER),'check',str(source),'--timings',str(report)],
                                    capture_output=True,text=True,timeout=120)
            self.assertEqual(result.returncode,0,result.stderr)
            evidence = json.loads(report.read_text())
            self.assertEqual(evidence['evaluated_steps'],10915)
            self.assertEqual(evidence['aggregate_traversal_work'],1010100)
            deps = subprocess.run([str(COMPILER),'deps',str(source)],capture_output=True,text=True,timeout=120)
            self.assertEqual(deps.returncode,0,deps.stderr)
            self.assertGreater(json.loads(deps.stdout)['aggregate_traversal_limit'], 1000000)

    def test_real_frontend_and_check_refuse_then_recover(self):
        function = """copy_until_limit :: ()->int { a:[32768]u8; total:=0;
          for i:0..<4096 { copy:=a; total+=cast(int)copy[0]; } return total; }"""
        cases = {
          'frontend': function + 'A :: #run copy_until_limit(); main :: () {}',
          'check': function + '#test "bounded" { #assert copy_until_limit()==0; }'
                    '#test "later" { #assert true; } main :: () {}',
        }
        with tempfile.TemporaryDirectory(prefix='coax-traversal-limit-') as d:
            for label, text in cases.items():
                with self.subTest(path=label):
                    source = Path(d) / (label + '.coax'); source.write_text(text)
                    report = Path(d) / (label + '.json')
                    command = 'check' if label == 'frontend' else 'test'
                    result = subprocess.run([str(COMPILER),command,str(source),'--run-steps','250000000',
                                             '--timings',str(report)],capture_output=True,text=True,timeout=300)
                    self.assertEqual(result.returncode,1)
                    self.assertIn('compile-time aggregate traversal limit exceeded',result.stderr)
                    if label == 'check': self.assertIn('ok   #test later',result.stdout)
                    else: self.assertEqual(json.loads(report.read_text())['aggregate_traversal_work'],134217728)
                    self.assertNotIn('error[E0402]',result.stderr)

    def test_actual_evaluator_under_sanitizers(self):
        with tempfile.TemporaryDirectory(prefix='coax-traversal-asan-') as d:
            directory=Path(d); objects=[]
            for name in ('budget','mutation-schedule','lexer','parser','checker','sources'):
                obj=directory/(name+'.o')
                subprocess.run(['cc','-std=c11','-O1','-g','-fsanitize=address,undefined',
                                '-c',str(ROOT/'bootstrap'/(name+'.c')),'-o',str(obj)],
                               check=True,capture_output=True,timeout=120)
                objects.append(str(obj))
            exe=directory/'traversal-test'
            runtime=os.environ.get('COAX_TEST_RUNTIME_DIR',str(ROOT/'bin'))
            subprocess.run(['c++','-std=c++20','-O1','-g','-fsanitize=address,undefined',
                            '-DCOAX_BOOTSTRAP_IMPORTER','-I',str(ROOT/'bootstrap'),'-I',runtime,
                            str(ROOT/'tests/ct_traversal.cpp'),*objects,'-o',str(exe)],
                           check=True,capture_output=True,timeout=120)
            result=subprocess.run([str(exe)],capture_output=True,text=True,timeout=180,
                                  env={**os.environ,'ASAN_OPTIONS':'detect_leaks=0'})
            self.assertEqual(result.returncode,0,result.stderr)

if __name__ == '__main__':
    unittest.main()
