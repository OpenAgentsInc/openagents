#!/usr/bin/env python3
"""Compile-time work and storage limits through the actual compiler driver."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
COMPILER = Path(os.environ.get('COAX', ROOT / 'bin/coax')).resolve()

class RunBudgetTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='coax-run-budget-')
        self.path = Path(self.tmp.name) / 'case.coax'
        self.addCleanup(self.tmp.cleanup)

    def invoke(self, source, *args):
        self.path.write_text(source)
        return subprocess.run([str(COMPILER), *args, str(self.path)], text=True,
                              capture_output=True, timeout=30)

    def test_authority_compilation_uses_check_storage_limits(self):
        source = ('#authority; answer :: ()->int {return 4;} '
                  '#test "large check" {words:[1572865]u8; words[1572864]=7; '
                  '#assert words[1572864]==7 && answer()==4;} main :: () {}')
        for command in ('check', 'c', 'header', 'deps'):
            with self.subTest(command=command):
                r = self.invoke(source, command)
                self.assertEqual(r.returncode, 0, r.stderr)

    def test_authority_library_checks_get_the_same_storage_limits(self):
        library = Path(self.tmp.name) / 'large-library.coax'
        library.write_text('CheckOnlyLarge :: struct($N) {words:[$N]u8;} '
                           '#test "large library check" {x:CheckOnlyLarge(1572865); '
                           'x.words[1572864]=7; #assert x.words[1572864]==7;} '
                           'library_answer :: ()->int {return 4;}')
        source = ('#load "large-library.coax"; #authority; '
                  'answer :: ()->int {return library_answer();} '
                  '#test "answer" {#assert answer()==4;} main :: () {}')
        r = self.invoke(source, 'c')
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertNotIn('CheckOnlyLarge', r.stdout)

    def test_authority_checks_keep_the_published_upper_storage_bound(self):
        source = ('#authority; #test "too large" #steps 100000000 {words:[33554433]u8; '
                  '#assert words[33554432]==0;} main :: () {}')
        r = self.invoke(source, 'check')
        self.assertEqual(r.returncode, 1, r.stderr)
        self.assertIn('the limit is 33554432', r.stderr)

    def test_authority_does_not_enlarge_constant_evaluation_storage(self):
        source = ('#authority; large :: ()->u8 {words:[1572865]u8; return words[1572864];} '
                  'A :: #run large(); #test "constant" {#assert A==0 && large()==0;} main :: () {}')
        r = self.invoke(source, 'check', '--run-steps', '20000000')
        self.assertEqual(r.returncode, 1, r.stderr)
        self.assertIn('the limit is 1572864', r.stderr)

    def test_explicit_test_storage_limit_stays_lowered(self):
        source = ('#authority; answer :: ()->int {return 4;} '
                  '#test "limited" {words:[1000]u8; '
                  '#assert words[999]==0 && answer()==4;} main :: () {}')
        r = self.invoke(source, 'test', '--slots', '100')
        self.assertEqual(r.returncode, 1, r.stderr)
        self.assertIn('the limit is 100', r.stderr)

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

    def test_failed_constant_clone_restores_cycle_state(self):
        source = ("make :: () -> [500000] u8 { a:[500000] u8; return a; } "
                  "A :: #run make(); "
                  '#test "fails allocation" { big:[400000] u8; copy:=A; #assert big[0]==copy[0]; } '
                  '#test "later constant read" { #assert A[0]==0; } main :: () {}')
        # A compile's slot limit for the checks (they have a larger one of their own).
        r = self.invoke(source, 'test', '--run-steps', '250000000', '--slots', '1572864')
        self.assertEqual(r.returncode, 1)
        self.assertIn('error[E0400]', r.stderr)
        self.assertRegex(r.stderr, r'aggregate storage limit exceeded: \d+ value slots needed, the limit is 1572864')
        self.assertNotIn('error[E0402]', r.stderr)
        self.assertIn('ok   #test later constant read', r.stdout)
        self.assertIn('2 checks, 1 passed, 1 failed', r.stdout)

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
        for name in ('budget', 'check-domains', 'mutation-schedule', 'profiles', 'lexer', 'parser', 'checker', 'sources'):
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

    # GitHub #12660: a program whose tables need more than the default
    # budget (the openagents CLI) declares it in its entry file.
    WORK = ('spin :: (n:int)->int {s:=0; for i: 0..<n {s+=i%3;} return s;} '
            'A :: #run spin(60000); main :: () {}')

    def test_entry_file_declares_its_run_budget(self):
        r = self.invoke(self.WORK, 'check')
        self.assertEqual(r.returncode, 1, r.stderr)
        self.assertIn('exceeded 100000 steps', r.stderr)
        report = Path(self.tmp.name) / 'declared.json'
        r = self.invoke('#run_steps 1000000; ' + self.WORK, 'check', '--timings', str(report))
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(json.loads(report.read_text())['run_steps'], 1000000)
        r = self.invoke('#run_steps 1000000; main :: () {}', 'deps')
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(json.loads(r.stdout)['run_steps'], 1000000)

    def test_command_line_budget_overrides_the_declared_one(self):
        r = self.invoke('#run_steps 1000000; ' + self.WORK, 'check', '--run-steps', '100000')
        self.assertEqual(r.returncode, 1, r.stderr)
        self.assertIn('exceeded 100000 steps', r.stderr)
        r = self.invoke('#run_steps 1000; ' + self.WORK, 'check', '--run-steps', '1000000')
        self.assertEqual(r.returncode, 0, r.stderr)

    def test_declared_budget_is_bounded_and_entry_only(self):
        for bad in ('0', '250000001', '1.5', 'x'):
            with self.subTest(value=bad):
                r = self.invoke('#run_steps %s; main :: () {}' % bad, 'check')
                self.assertEqual(r.returncode, 1, r.stderr)
                self.assertIn('E0224', r.stderr)
        r = self.invoke('#run_steps 9; #run_steps 9; main :: () {}', 'check')
        self.assertEqual(r.returncode, 1, r.stderr)
        self.assertIn('E0224', r.stderr)

    # GitHub #12679: a library declares the budget it needs when it is
    # checked alone; loaded, its declaration never raises the loader's.
    def test_library_budget_applies_only_when_it_is_the_entry(self):
        tables = Path(self.tmp.name) / 'tables.coax'
        tables.write_text('#run_steps 1000000; ' + self.WORK.replace(' main :: () {}', ''))
        report = Path(self.tmp.name) / 'alone.json'
        r = subprocess.run([str(COMPILER), 'check', '--library', str(tables), '--timings', str(report)],
                           text=True, capture_output=True, timeout=30)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(json.loads(report.read_text())['run_steps'], 1000000)
        # Loaded, it is ignored: the loader keeps the default and runs out.
        r = self.invoke('#load "tables.coax"; main :: () {}', 'check')
        self.assertEqual(r.returncode, 1, r.stderr)
        self.assertIn('exceeded 100000 steps', r.stderr)
        self.assertNotIn('E0224', r.stderr)
        # The loader's own declaration, smaller or larger, is the one used,
        # before or after the #load; two are not "declared more than once".
        for entry in ('#run_steps 1000; #load "tables.coax"; main :: () {}',
                      '#load "tables.coax"; #run_steps 1000; main :: () {}'):
            with self.subTest(entry=entry):
                r = self.invoke(entry, 'check')
                self.assertEqual(r.returncode, 1, r.stderr)
                self.assertIn('exceeded 1000 steps', r.stderr)
        report = Path(self.tmp.name) / 'loader.json'
        r = self.invoke('#load "tables.coax"; #run_steps 2000000; main :: () {}', 'check', '--timings', str(report))
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(json.loads(report.read_text())['run_steps'], 2000000)
        # --run-steps still overrides whatever the entry declares.
        r = self.invoke('#load "tables.coax"; main :: () {}', 'check', '--run-steps', '1000000')
        self.assertEqual(r.returncode, 0, r.stderr)

    def test_loaded_declaration_is_still_well_formed(self):
        for bad in ('0', '250000001', '1.5', 'x', '9; #run_steps 9'):
            with self.subTest(value=bad):
                (Path(self.tmp.name) / 'tables.coax').write_text('#run_steps %s; T :: 1;' % bad)
                r = self.invoke('#load "tables.coax"; main :: () {}', 'check')
                self.assertEqual(r.returncode, 1, r.stderr)
                self.assertIn('E0224', r.stderr)

    def test_call_trace_names_the_loaded_file_and_its_line(self):
        (Path(self.tmp.name) / 'tables.coax').write_text('\n\n' + self.WORK.replace(' main :: () {}', ''))
        r = self.invoke('#load "tables.coax";\nmain :: () {}', 'check')
        self.assertEqual(r.returncode, 1, r.stderr)
        self.assertIn('exceeded 100000 steps', r.stderr)
        self.assertIn('note: compile-time call at ' + str(Path(self.tmp.name) / 'tables.coax') + ':3:', r.stderr)

    def test_deps_records_selected_profile(self):
        r = self.invoke('main :: () {}', 'deps', '--run-steps', '123456')
        self.assertEqual(r.returncode, 0, r.stderr)
        d = json.loads(r.stdout)
        self.assertEqual(d['run_steps'], 123456)
        self.assertEqual(d['aggregate_slot_limit'], 1572864)
        self.assertEqual(d['aggregate_container_limit'], 131072)

class CapacityTests(unittest.TestCase):
    """The near-limit report (spec 16.2) at 75% and 100% of each compile-time
    and nesting limit: a note above three quarters, success at the limit,
    the limit's error one past it."""
    setUp = RunBudgetTests.setUp

    def capacities(self, source, *args):
        self.path.write_text(source)
        timings = Path(self.tmp.name) / 'timings.json'
        r = subprocess.run([str(COMPILER), 'check', str(self.path), '--timings', str(timings), *args],
                           text=True, capture_output=True, timeout=60)
        rows = {x['name']: x for x in json.loads(timings.read_text())['capacities']}
        notes = {}
        for line in r.stderr.splitlines():
            if line.startswith('coax: note['):
                code, text = line[len('coax: note['):].split(']: ', 1)
                notes[text.split(':')[0]] = (code, text)
        for name, row in rows.items():
            self.assertEqual(row['near'], row['used'] * 4 > row['limit'] * 3, row)
            self.assertEqual(name in notes, row['near'], (name, row, r.stderr))
            if row['near']:
                self.assertEqual(notes[name][0], row['code'])
        j = subprocess.run([str(COMPILER), 'check', str(self.path), '--diagnostics', 'json', *args],
                           text=True, capture_output=True, timeout=60)
        records = [json.loads(x) for x in j.stderr.splitlines() if x.startswith('{')]
        self.assertEqual({x['message'].split(':')[0] for x in records if x['severity'] == 'note' and x['span'] is None},
                         set(notes), records)
        return r, rows


    def use(self, source, name, *args):
        r, rows = self.capacities(source, *args)
        return r, rows.get(name)

    def assert_boundaries(self, name, make, limit, code, at_three_quarters, error_code=None):
        """make(k) uses k of `name`'s limit... at_three_quarters is the k that uses exactly 3/4."""
        quarter = limit - limit * 3 // 4
        for k, used, near in [(at_three_quarters, limit * 3 // 4, False), (at_three_quarters + 1, limit * 3 // 4 + 1, True),
                              (at_three_quarters + quarter, limit, True)]:
            r, row = self.use(make(k), name)
            self.assertEqual(r.returncode, 0, r.stderr)
            self.assertEqual((row['used'], row['limit'], row['near'], row['code']), (used, limit, near, code), k)
        r, _ = self.use(make(at_three_quarters + quarter + 1), name)
        self.assertNotEqual(r.returncode, 0)
        self.assertIn(f'error[{error_code or code}]', r.stderr)

    def test_compile_time_call_depth(self):
        # down(n) is n+1 calls deep
        self.assert_boundaries('compile-time call depth',
            lambda k: f'down :: (n:int)->int {{if n==0 {{return 0;}} return down(n-1)+1;}}\nX :: #run down({k - 1});\nmain :: () {{}}\n',
            64, 'E0403', 48)

    def test_compile_time_dependency_depth(self):
        # C0 needs C1 ... needs C(k-1): k constants evaluated inside one another. Past
        # 64, the checker's own constant dependency bound (E0347) refuses first.
        def chain(k):
            return ('inc :: (a:int)->int {return a+1;}\n' + ''.join(f'C{i} :: #run inc(C{i+1});\n' for i in range(k - 1)) +
                    f'C{k-1} :: #run inc(0);\nmain :: () {{}}\n')
        self.assert_boundaries('compile-time dependency depth', chain, 64, 'E0403', 48, 'E0347')

    def test_parser_nesting(self):
        # k nested blocks are k active grammar entries
        self.assert_boundaries('parser nesting', lambda k: 'main :: () ' + '{' * k + '}' * k + '\n', 128, 'E0221', 96)

    def test_checker_nesting(self):
        # 1+1+...: the parser loops, the checker recurses once per operator, from 3
        self.assert_boundaries('checker nesting', lambda k: 'main :: () { x := 1' + '+1' * (k - 3) + '; }\n', 256, 'E0381', 192)

    def test_compile_time_steps(self):
        source = 'spin :: (n:int)->int {s:=0; for i:0..<n {s+=i;} return s;}\nX :: #run spin(1000);\nmain :: () {}\n'
        r, row = self.use(source, 'compile-time steps')
        self.assertEqual(r.returncode, 0, r.stderr)
        used = row['used']
        # A limit L reports when 4*used > 3*L: the largest L that does not is 4*used//3.
        quiet = used * 4 // 3
        for limit, near in [(quiet, False), (quiet - 1, True), (used, True)]:
            r, row = self.use(source, 'compile-time steps', '--run-steps', str(limit))
            self.assertEqual(r.returncode, 0, r.stderr)
            self.assertEqual((row['used'], row['limit'], row['near']), (used, limit, near))
        r, row = self.use(source, 'compile-time steps', '--run-steps', str(used - 1))
        self.assertNotEqual(r.returncode, 0)
        self.assertIn('error[E0400]', r.stderr)

    def test_compile_time_value_slots(self):
        # a [n] u8 local is n value slots
        def program(n):
            return f'f :: ()->int {{ a: [{n}] u8; return 1; }}\nX :: #run f();\nmain :: () {{}}\n'
        for n, near in [(1179648, False), (1179649, True), (1572864, True)]:
            r, row = self.use(program(n), 'compile-time value slots', '--run-steps', '250000000')
            self.assertEqual(r.returncode, 0, r.stderr)
            self.assertEqual((row['used'], row['limit'], row['near']), (n, 1572864, near))
        r, _ = self.use(program(1572865), 'compile-time value slots', '--run-steps', '250000000')
        self.assertNotEqual(r.returncode, 0)
        self.assertIn('error[E0400]', r.stderr)

    def test_one_line_json_and_quiet_limits_print_no_notes(self):
        source = 'main :: () { x := 1' + '+1' * 200 + '; }\n'
        self.path.write_text(source)
        r = subprocess.run([str(COMPILER), 'check', str(self.path), '--json'], text=True, capture_output=True, timeout=60)
        self.assertEqual((r.returncode, r.stderr), (0, ''))
        r, row = self.use(source, 'checker nesting')
        self.assertTrue(row['near'])

if __name__ == '__main__': unittest.main()
