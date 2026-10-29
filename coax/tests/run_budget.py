#!/usr/bin/env python3
"""Compile-time work and storage limits through the actual compiler driver."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
COMPILER = ROOT / 'bin/coax'

class RunBudgetTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='coax-run-budget-')
        self.path = Path(self.tmp.name) / 'case.coax'
        self.addCleanup(self.tmp.cleanup)

    def invoke(self, source, *args):
        self.path.write_text(source)
        return subprocess.run([str(COMPILER), *args, str(self.path)], text=True,
                              capture_output=True, timeout=30)

    def test_input_validation_precedes_source_access(self):
        for value in ('', '0', '-1', '+1', '250000001', '999999999999999999999999', '1x'):
            r = subprocess.run([str(COMPILER), 'check', '/missing/no.coax', '--run-steps', value],
                               text=True, capture_output=True, timeout=10)
            self.assertNotEqual(r.returncode, 0)
            self.assertIn('--run-steps requires', r.stderr)
        for args in (('--run-steps',), ('--run-steps','1','--run-steps','1')):
            r = subprocess.run([str(COMPILER), 'check', '/missing/no.coax', *args],
                               text=True, capture_output=True, timeout=10)
            self.assertNotEqual(r.returncode, 0)
            self.assertIn('--run-steps requires', r.stderr)

    def test_shared_counter_and_checked_release(self):
        f = 'f :: () -> int { n:=0; while n<10000 { n+=1; } return n; }\n'
        one = f + 'A :: #run f(); main :: () {}'
        two = f + 'A :: #run f(); B :: #run f(); main :: () {}'
        for profile in ((), ('--release',)):
            self.assertEqual(self.invoke(one, 'check', *profile).returncode, 0)
            r = self.invoke(two, 'check', *profile)
            self.assertEqual(r.returncode, 1)
            self.assertIn('exceeded 100000 steps', r.stderr)
            self.assertEqual(self.invoke(two, 'check', *profile, '--run-steps', '200000').returncode, 0)

    def test_all_compiler_commands_receive_work_limit(self):
        source = ('f :: () -> int { n:=0; while n<20 { n+=1; } return n; } '
                  'A :: #run f(); #test "a" { #assert A==20; } '
                  'api :: () -> int #export_c { return A; } main :: () {}')
        for command in ('check', 'c', 'header', 'layout', 'deps', 'metal', 'build', 'lib', 'run', 'test', 'mutate'):
            with self.subTest(command=command):
                program = 'f :: () -> f32 { return 1.0f32 + 2.0f32; } A :: #run f(); api :: () -> f32 #export_c { return A; }' if command == 'metal' else source
                r = self.invoke(program, command, '--run-steps', '1', '-o', str(Path(self.tmp.name) / 'out')) if command in ('build', 'lib') else self.invoke(program, command, '--run-steps', '1')
                self.assertEqual(r.returncode, 1, r.stderr)
                self.assertIn('exceeded 1 steps', r.stderr)

    def test_retained_constants_share_storage_budget(self):
        head = 'f :: () -> [500000] u8 { a:[500000] u8; return a; }\n'
        one = head + 'A :: #run f(); main :: () {}'
        two = head + 'A :: #run f(); B :: #run f(); main :: () {}'
        for profile in ((), ('--release',)):
            self.assertEqual(self.invoke(one, 'check', *profile, '--run-steps', '250000000').returncode, 0)
            r = self.invoke(two, 'check', *profile, '--run-steps', '250000000')
            self.assertEqual(r.returncode, 1)
            self.assertIn('aggregate storage limit exceeded', r.stderr)

    def test_check_step_budget_stays_independent(self):
        source = '#test "loop" { n:=0; while n<100 { n+=1; } #assert n==100; } main :: () {}'
        r = self.invoke(source, 'test', '--run-steps', '250000000', '--steps', '10')
        self.assertNotEqual(r.returncode, 0)
        self.assertIn('exceeded 10 steps', r.stderr)
        self.assertEqual(self.invoke(source, 'test', '--run-steps', '1', '--steps', '10000').returncode, 0)

    def test_existing_nostr_json_boundary_check_fits(self):
        # This graph contains a real one-megabyte input-boundary check. A
        # capacity chosen below its actual live working set breaks consumers.
        r = subprocess.run([str(COMPILER), 'test', str(ROOT / 'lib/nostr/json.coax'), '--library'],
                           text=True, capture_output=True, timeout=120)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn('original input budget is inclusive', r.stdout)

    def test_nested_allocation_refuses_in_all_profiles(self):
        source = 'f :: () -> int { a:[1500][1500] u8; return cast(int) a[0][0]; } A :: #run f(); main :: () {}'
        for profile in ((), ('--release',)):
            r = self.invoke(source, 'check', *profile, '--run-steps', '250000000')
            self.assertEqual(r.returncode, 1)
            self.assertIn('aggregate storage limit exceeded', r.stderr)

    def test_allocator_ownership_and_partial_failure_under_sanitizers(self):
        # The same implementation as the driver, linked to the checked-in
        # stage-zero seeds. ASan/UBSan detect invalid lifetime accounting.
        directory = Path(self.tmp.name)
        objects = []
        for name in ('budget', 'lexer', 'parser', 'checker', 'sources'):
            obj = directory / (name + '.o')
            subprocess.run(['cc', '-std=c11', '-O1', '-g', '-fsanitize=address,undefined',
                            '-c', str(ROOT / 'bootstrap' / (name + '.c')), '-o', str(obj)],
                           check=True, capture_output=True, timeout=120)
            objects.append(str(obj))
        exe = directory / 'storage-test'
        subprocess.run(['c++', '-std=c++20', '-O1', '-g', '-fsanitize=address,undefined',
                        '-DCOAX_BOOTSTRAP_IMPORTER', '-I', str(ROOT / 'bootstrap'),
                        '-I', str(ROOT / 'bin'), str(ROOT / 'tests/ct_storage.cpp'),
                        *objects, '-o', str(exe)], check=True, capture_output=True, timeout=120)
        result = subprocess.run([str(exe)], capture_output=True, text=True, timeout=60,
                                env={**__import__('os').environ, 'ASAN_OPTIONS':'detect_leaks=0'})
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_timings_record_work_and_peaks(self):
        report = Path(self.tmp.name) / 'timings.json'
        source = 'f :: () -> int { a:[12] u8; return cast(int) a[0]; } A :: #run f(); main :: () {}'
        r = self.invoke(source, 'check', '--run-steps', '123456', '--timings', str(report))
        self.assertEqual(r.returncode, 0, r.stderr)
        data = json.loads(report.read_text())
        self.assertEqual(data['run_steps'], 123456)
        self.assertGreater(data['evaluated_steps'], 12)
        self.assertGreaterEqual(data['aggregate_peak_slots'], 12)
        self.assertGreater(data['aggregate_peak_containers'], 0)

    def test_timings_keep_selected_limit_when_frontend_fails(self):
        report = Path(self.tmp.name) / 'failed-timings.json'
        r = self.invoke('this is not valid coax', 'check', '--run-steps', '123456', '--timings', str(report))
        self.assertNotEqual(r.returncode, 0)
        data = json.loads(report.read_text())
        self.assertEqual(data['run_steps'], 123456)
        self.assertEqual(data['evaluated_steps'], 0)

    def test_deps_records_selected_profile(self):
        r = self.invoke('main :: () {}', 'deps', '--run-steps', '123456')
        self.assertEqual(r.returncode, 0, r.stderr)
        d = json.loads(r.stdout)
        self.assertEqual(d['run_steps'], 123456)
        self.assertEqual(d['aggregate_slot_limit'], 1572864)
        self.assertEqual(d['aggregate_container_limit'], 131072)

if __name__ == '__main__': unittest.main()
