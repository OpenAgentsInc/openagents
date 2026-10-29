#!/usr/bin/env python3
"""Compare each mutant's result against the original full-check runner."""
import os
from pathlib import Path
import re
import random
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
COMPILER = Path(os.environ.get('COAX_TEST_COMPILER', ROOT / 'bin/coax'))
SAFE = '''
f :: (x:int)->int { return x+1; }
g :: (x:int)->int { return x*2; }
h :: (x:int)->int { if x>2 { return x-1; } return x+1; }
#test "first" { #assert f(2)==3; }
#test "second" { #assert g(3)==6; }
#forall "range" x:0..<5 { #assert f(x)>x; }
#test "branch" { #assert h(1)==2 && h(2)==3 && h(3)==2 && h(4)==3; }
main :: () {}
'''
BORROWED = '''
Node :: struct { next:[] Node; value:int; }
f :: (x:int)->int { return x+1; }
#test "chain" { a:[1] Node; a[0].next=a[..]; #assert f(1)==2; }
#test "again" { #assert f(2)==3; }
main :: () {}
'''
STRINGS = '''
f :: (x:int)->int { return x+1; }
#test "bytes" { a:[1]u8; a[0]=65; #assert cast(string)a[..]=="A" && f(1)==2; }
#test "again" { #assert f(2)==3; }
main :: () {}
'''

class MutationScheduleTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='coax-mutation-schedule-')
        self.addCleanup(self.tmp.cleanup)
        self.source = Path(self.tmp.name) / 'case.coax'

    def run_pair(self, text, *extra):
        self.source.write_text(text)
        results = []
        for reference in (False, True):
            cmd = [str(COMPILER), 'mutate', str(self.source), '--mutation-stats', '--mutation-trace', *extra]
            if reference: cmd.append('--mutation-reference')
            results.append(subprocess.run(cmd, capture_output=True, text=True, timeout=120))
        fast, full = results
        self.assertEqual(fast.returncode, full.returncode, (fast.stderr, full.stderr))
        self.assertEqual(fast.stdout, full.stdout)
        trace = lambda r: [x for x in r.stderr.splitlines() if x.startswith('mutation-outcome ')]
        self.assertTrue(trace(fast))
        self.assertEqual(trace(fast), trace(full))
        return fast, full

    def test_isolated_graph_preserves_every_mutant_and_reduces_checks(self):
        fast, full = self.run_pair(SAFE)
        self.assertEqual(fast.returncode, 0, fast.stdout + fast.stderr)
        self.assertIn('mutation-scheduling selective', fast.stderr)
        self.assertIn('mutation-scheduling full', full.stderr)
        numbers = lambda r: tuple(map(int, re.search(r'checks=(\d+) skipped=(\d+)', r.stderr).groups()))
        self.assertLess(numbers(fast)[0], numbers(full)[0])
        self.assertGreater(numbers(fast)[1], 0)

    def test_seeded_call_graphs_match_complete_reference(self):
        rng = random.Random(20261029)
        for case in range(12):
            factors = [rng.randrange(1, 5) for _ in range(5)]
            functions = ['f0 :: (x:int)->int { return x+%d; }' % factors[0]]
            for i in range(1, 5):
                functions.append('f%d :: (x:int)->int { if x>%d { return f%d(x)+%d; } return x+%d; }' %
                                 (i, i, i-1, factors[i], factors[i]))
            def expected(i, x):
                if i and x > i: return expected(i-1, x) + factors[i]
                return x + factors[i]
            checks = []
            for j in range(5):
                fn = rng.randrange(5); value = rng.randrange(7)
                checks.append('#test "case%d" { #assert f%d(%d)==%d; }' %
                              (j, fn, value, expected(fn, value)))
            source = ' '.join(functions + checks + ['main :: () {}'])
            with self.subTest(case=case):
                fast, _ = self.run_pair(source)
                self.assertIn('mutation-scheduling selective', fast.stderr)

    def test_real_crypto_and_string_graphs_match_reference(self):
        for relative, mode in [('lib/crypto/bytes.coax', 'selective'),
                               ('lib/nostr/hex.coax', 'full')]:
            with self.subTest(graph=relative):
                base = [str(COMPILER), 'mutate', str(ROOT / relative), '--mutation-stats', '--mutation-trace']
                auto = subprocess.run(base, capture_output=True, text=True, timeout=120)
                full = subprocess.run([*base, '--mutation-reference'], capture_output=True, text=True, timeout=120)
                self.assertEqual(auto.returncode, full.returncode)
                self.assertEqual(auto.stdout, full.stdout)
                self.assertEqual([x for x in auto.stderr.splitlines() if x.startswith('mutation-outcome ')],
                                 [x for x in full.stderr.splitlines() if x.startswith('mutation-outcome ')])
                self.assertIn('mutation-scheduling ' + mode, auto.stderr)

    def test_borrowed_aggregate_and_cyclic_storage_fall_back(self):
        fast, full = self.run_pair(BORROWED)
        self.assertIn('mutation-scheduling full', fast.stderr)
        self.assertIn('borrowed=1', fast.stderr)
        self.assertIn('unstable=1', fast.stderr)

    def test_array_of_views_falls_back(self):
        source = ('f :: (x:int)->int { return x+1; } '
                  '#test "array view" { a:[1]u8; v:[1][]u8; v[0]=a[..]; #assert f(1)==2; } '
                  'main :: () {}')
        fast, _ = self.run_pair(source)
        self.assertIn('mutation-scheduling full', fast.stderr)
        self.assertIn('borrowed=1', fast.stderr)

    def test_dead_branches_survive_in_both_runners(self):
        source = ('f :: (x:int)->int { if x>10 { return x-3; } return x+1; } '
                  '#test "one" { #assert f(1)==2; } main :: () {}')
        fast, _ = self.run_pair(source)
        self.assertEqual(fast.returncode, 1)
        self.assertIn('mutant survived', fast.stdout)
        self.assertIn('mutation-scheduling selective', fast.stderr)

    def test_string_allocation_falls_back(self):
        fast, full = self.run_pair(STRINGS)
        self.assertIn('mutation-scheduling full', fast.stderr)
        self.assertIn('strings=1', fast.stderr)

    def test_unentered_helper_mutants_remain_survivors(self):
        source = ('hidden :: (x:int)->int { return x+1; } '
                  'f :: (x:int)->int { if x>10 { return hidden(x); } return x+1; } '
                  '#test "f" { #assert f(1)==2; } main :: () {}')
        fast, _ = self.run_pair(source)
        self.assertEqual(fast.returncode, 1)
        self.assertIn('hidden', fast.stdout)
        self.assertIn('mutation-scheduling selective', fast.stderr)

    def test_authority_execution_coverage_still_fails(self):
        self.source.write_text('#authority; hidden :: ()->int { return 1; } '
                               'f :: (x:int)->int { if x>10 { return hidden(); } return x; } '
                               '#test "f" { #assert f(1)==1; } main :: () {}')
        for extra in ([], ['--mutation-reference']):
            r = subprocess.run([str(COMPILER), 'mutate', str(self.source), *extra],
                               text=True, capture_output=True, timeout=120)
            self.assertNotEqual(r.returncode, 0)
            self.assertIn('error[E0380]', r.stderr)
            self.assertIn('checks fail before any mutation', r.stderr)

    def test_mutant_allocation_failure_and_later_constant_read_match(self):
        source = ('make :: () -> [500000] u8 { a:[500000] u8; return a; } A :: #run make(); '
                  'f :: (n:int)->int { if n>0 { big:[400000]u8; copy:=A; return cast(int)big[0]+cast(int)copy[0]; } return n; } '
                  '#test "f" { #assert f(0)==0; } '
                  '#test "constant" { #assert A[0]==0; } main :: () {}')
        fast, _ = self.run_pair(source, '--run-steps', '250000000')
        self.assertIn('mutation-scheduling selective', fast.stderr)
        self.assertNotIn('error[E0402]', fast.stderr)

if __name__ == '__main__': unittest.main()
