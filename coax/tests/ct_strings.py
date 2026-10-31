#!/usr/bin/env python3
"""String snapshots in checks and emitted native programs."""
from pathlib import Path
import os
import json
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
COMPILER = Path(os.environ.get('COAX_TEST_COMPILER', ROOT / 'bin/coax'))
SOURCE = r'''
#import "Basic";
Box :: struct { text:string; }
make :: (ch:u8)->Box { bytes:=[3]u8.[ch,66,67]; b:Box; b.text=cast(string)bytes[..]; return b; }
A :: #run make(65).text;
B :: #run make(68).text;
C :: #run make(69);
DefaultBox :: struct { text:string = A; }
default_box :: ()->DefaultBox { d:DefaultBox; return d; }
D :: #run default_box();
as_text :: (a:[]u8)->string { return cast(string)a; }
#assert A=="ABC" && B=="DBC" && C.text=="EBC" && D.text=="ABC";
#test "snapshot and sliced aliases" { bytes:=[3]u8.[65,66,67]; s:=as_text(bytes[..]); part:=s[1..<3]; b:Box; b.text=s; bytes[1]=90; #assert s=="ABC" && part=="BC" && b.text=="ABC"; }
main :: () { print("% % %\n", A,B,C.text); }
'''

class StringTests(unittest.TestCase):
    def test_real_snapshots_and_frozen_constants(self):
        with tempfile.TemporaryDirectory(prefix='coax-strings-') as d:
            source = Path(d) / 'case.coax'
            source.write_text(SOURCE)
            for profile in ((), ('--release',)):
                for command in ('test', 'run'):
                    r = subprocess.run([str(COMPILER), command, str(source), *profile],
                                       text=True, capture_output=True, timeout=120)
                    self.assertEqual(r.returncode, 0, r.stderr)
                    if command == 'run': self.assertEqual(r.stdout, 'ABC DBC EBC\n')

    def test_declared_limits_and_frontend_peak_are_reported(self):
        with tempfile.TemporaryDirectory(prefix='coax-strings-metadata-') as d:
            source = Path(d) / 'case.coax'; source.write_text(SOURCE)
            report = Path(d) / 'timings.json'
            deps = subprocess.run([str(COMPILER), 'deps', str(source)], text=True,
                                  capture_output=True, timeout=120)
            self.assertEqual(deps.returncode, 0, deps.stderr)
            data = json.loads(deps.stdout)
            self.assertEqual(data['string_live_byte_limit'], 67108864)
            self.assertEqual(data['string_materialization_work_limit'], 268435456)
            result = subprocess.run([str(COMPILER), 'check', str(source), '--timings', str(report)],
                                    text=True, capture_output=True, timeout=120)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertGreaterEqual(json.loads(report.read_text())['string_peak_bytes'], 12)

    def test_repeated_casts(self):
        source = """#test "repeated snapshots" {
          input:[32768]u8; i:=0;
          while i<4096 { s:=cast(string)input[..]; #assert s.count==32768; i+=1; }
        } main :: () {}"""
        with tempfile.TemporaryDirectory(prefix='coax-strings-stress-') as d:
            path = Path(d) / 'stress.coax'; path.write_text(source)
            result = subprocess.run([str(COMPILER), 'test', str(path)], text=True,
                                    capture_output=True, timeout=120)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn('1 check, 1 passed, 0 failed', result.stdout)

    def test_real_check_rejects_excess_live_and_work_then_recovers(self):
        programs = {
            'live': """#test "too many live strings" { input:[1048576]u8; strings:[64]string; i:=0;
                         while i<64 { strings[i]=cast(string)input[..]; i+=1; } }
                         #test "later" { #assert true; } main :: () {}""",
            'work': """#test "too much copying" { input:[32768]u8; i:=0;
                         while i<8192 { s:=cast(string)input[..]; #assert s.count==32768; i+=1; } }
                         #test "later" { #assert true; } main :: () {}""",
        }
        with tempfile.TemporaryDirectory(prefix='coax-string-limits-') as d:
            for label, source in programs.items():
                with self.subTest(limit=label):
                    path = Path(d) / (label + '.coax'); path.write_text(source)
                    result = subprocess.run([str(COMPILER), 'test', str(path)], text=True,
                                            capture_output=True, timeout=180)
                    self.assertEqual(result.returncode, 1)
                    self.assertIn('compile-time string storage or work limit exceeded', result.stderr)
                    self.assertIn('ok   #test later', result.stdout)
                    self.assertNotIn('error[E0402]', result.stderr)

    def test_actual_owner_under_sanitizers(self):
        with tempfile.TemporaryDirectory(prefix='coax-strings-asan-') as d:
            directory = Path(d); objects = []
            for name in ('budget', 'profiles', 'mutation-schedule', 'lexer', 'parser', 'checker', 'sources'):
                obj = directory / (name + '.o')
                subprocess.run(['cc', '-std=c11', '-O1', '-g', '-fsanitize=address,undefined',
                                '-c', str(ROOT / 'bootstrap' / (name + '.c')), '-o', str(obj)],
                               check=True, capture_output=True, timeout=120)
                objects.append(str(obj))
            exe = directory / 'strings-test'
            runtime = os.environ.get('COAX_TEST_RUNTIME_DIR', str(ROOT / 'bin'))
            subprocess.run(['c++', '-std=c++20', '-O1', '-g', '-fsanitize=address,undefined',
                            '-DCOAX_BOOTSTRAP_IMPORTER', '-I', str(ROOT / 'bootstrap'),
                            '-I', runtime, str(ROOT / 'tests/ct_strings.cpp'), *objects,
                            '-o', str(exe)], check=True, capture_output=True, timeout=120)
            result = subprocess.run([str(exe)], text=True, capture_output=True, timeout=180,
                                    env={**os.environ, 'ASAN_OPTIONS': 'detect_leaks=0'})
            self.assertEqual(result.returncode, 0, result.stderr)

if __name__ == "__main__":
    unittest.main()
