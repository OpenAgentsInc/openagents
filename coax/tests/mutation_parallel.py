#!/usr/bin/env python3
"""Parallel and sharded `coax mutate` must equal the serial run byte for byte.

For each source, compares `--jobs 1` (the serial in-process loop) with
`--jobs 8` and with the merge of every shard record for several shard
counts: stdout, the per-mutant trace, the scheduling statistics and the exit
status must all be identical.

  python3 coax/tests/mutation_parallel.py                  synthetic cases + quick real files
  python3 coax/tests/mutation_parallel.py FILE.coax...     those files only (e.g. a Psionic unit)
"""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[1]
REPO = ROOT.parent
COMPILER = Path(os.environ.get('COAX_TEST_COMPILER', ROOT / 'bin/coax'))
TIMEOUT = int(os.environ.get('COAX_MUTATION_TIMEOUT', '3600'))
QUICK_REAL = ['verse/wallet.coax', 'coax/lib/compiler/mutation-schedule.coax', 'coax/lib/compiler/budget.coax',
              'coax/lib/crypto/bytes.coax', 'coax/lib/nostr/hex.coax']

CASES = {
    'selective': '''
f :: (x:int)->int { return x+1; }
g :: (x:int)->int { return x*2; }
h :: (x:int)->int { if x>2 { return x-1; } return x+1; }
#test "first" { #assert f(2)==3; }
#test "second" { #assert g(3)==6; }
#forall "range" x:0..<5 { #assert f(x)>x; }
#test "branch" { #assert h(1)==2 && h(2)==3 && h(3)==2 && h(4)==3; }
main :: () {}
''',
    'borrowed': '''
Node :: struct { next:[] Node; value:int; }
f :: (x:int)->int { return x+1; }
#test "chain" { a:[1] Node; a[0].next=a[..]; #assert f(1)==2; }
#test "again" { #assert f(2)==3; }
main :: () {}
''',
    'strings': '''
f :: (x:int)->int { return x+1; }
#test "bytes" { a:[1]u8; a[0]=65; #assert cast(string)a[..]=="A" && f(1)==2; }
#test "again" { #assert f(2)==3; }
main :: () {}
''',
    'survivors-and-equivalent': '''
clamp :: (x:int, lo:int)->int {
 if x < lo { return lo; } // mutant: equivalent
 return x;
}
f :: (x:int)->int { if x>10 { return x-3; } return x+1; }
loop :: (n:int)->int { i:=0; s:=0; while i<n { s+=i; i+=1; } return s; }
hidden :: (x:int)->int { return x+1; }
g :: (x:int)->int { if x>100 { return hidden(x); } return x; }
#test "one" { #assert f(1)==2 && clamp(5,1)==5 && clamp(0,1)==1; }
#test "loop" { #assert loop(4)==6; }
#test "g" { #assert g(1)==1; }
main :: () {}
''',
    'unreached': '''
f :: (x:int)->int { return x*3; }
#test "f" { #assert f(2)==6; }
lonely :: ()->int { return 7; }
main :: () {}
''',
}


def run(cmd, timeout=TIMEOUT):
    return subprocess.run([str(c) for c in cmd], capture_output=True, text=True, timeout=timeout)


def observed(r):
    """What must not change: stdout, trace lines, stats line, status."""
    lines = r.stderr.splitlines()
    return (r.returncode, r.stdout,
            [x for x in lines if x.startswith('mutation-outcome ')],
            [x for x in lines if x.startswith('mutation-scheduling ')])


def mutate(source, *extra):
    return run([COMPILER, 'mutate', source, '--mutation-stats', '--mutation-trace', *extra])


def sharded(source, shards, workdir, jobs='2'):
    records = []
    for s in range(1, shards + 1):
        r = run([COMPILER, 'mutate', source, '--shard', f'{s}/{shards}', '--jobs', jobs])
        assert r.stdout.startswith('coax-mutation-shard v1\n'), (r.stdout, r.stderr)
        path = Path(workdir) / f'shard-{s}-of-{shards}.rec'
        path.write_text(r.stdout)
        records.append(path)
    merge = []
    for path in reversed(records):  # order of records does not matter
        merge += ['--merge', path]
    return mutate(source, *merge), records


class Conformance(unittest.TestCase):
    sources = None  # set from argv

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='coax-mutation-parallel-')
        self.addCleanup(self.tmp.cleanup)

    def check_equivalent(self, source, shard_counts=(1, 3, 7)):
        t0 = time.monotonic(); serial = mutate(source, '--jobs', '1'); t1 = time.monotonic()
        parallel = mutate(source, '--jobs', '8'); t2 = time.monotonic()
        self.assertTrue(observed(serial)[2], serial.stderr)
        self.assertEqual(observed(parallel), observed(serial))
        for n in shard_counts:
            merged, _ = sharded(source, n, self.tmp.name)
            self.assertEqual(observed(merged), observed(serial), f'{n} shards')
        return serial, t1 - t0, t2 - t1

    def test_synthetic_cases(self):
        if self.sources: self.skipTest('explicit files given')
        for name, text in CASES.items():
            with self.subTest(case=name):
                path = Path(self.tmp.name) / f'{name}.coax'
                path.write_text(text)
                serial, _, _ = self.check_equivalent(path, (1, 2, 3, 50))
                if name == 'survivors-and-equivalent':
                    self.assertIn('mutant survived', serial.stdout)
                    self.assertIn('marked equivalent', serial.stdout)
                if name == 'unreached':
                    self.assertIn('not reached by any check: lonely', serial.stdout)

    def test_real_files(self):
        for relative in self.sources or QUICK_REAL:
            with self.subTest(file=relative):
                path = (REPO / relative) if not Path(relative).is_absolute() else Path(relative)
                serial, ts, tp = self.check_equivalent(path, (2, 5))
                summary = serial.stdout.strip().splitlines()[-1]
                print(f'\n{relative}: {summary}; serial {ts:.1f}s, --jobs 8 {tp:.1f}s ({ts / max(tp, 1e-9):.1f}x)',
                      file=sys.stderr)

    def test_merge_refuses_incomplete_or_foreign_records(self):
        if self.sources: self.skipTest('explicit files given')
        a = Path(self.tmp.name) / 'a.coax'; a.write_text(CASES['selective'])
        b = Path(self.tmp.name) / 'b.coax'; b.write_text(CASES['selective'].replace('x+1', 'x+2').replace('f(2)==3', 'f(2)==4').replace('f(x)>x', 'f(x)>x+1'))
        _, records = sharded(a, 3, self.tmp.name)
        missing = mutate(a, '--merge', records[0], '--merge', records[1])
        self.assertNotEqual(missing.returncode, 0); self.assertIn('shard 3/3 is missing', missing.stderr)
        twice = mutate(a, '--merge', records[0], '--merge', records[0], '--merge', records[1])
        self.assertNotEqual(twice.returncode, 0); self.assertIn('appears twice', twice.stderr)
        foreign = mutate(b, *sum((['--merge', r] for r in records), []))
        self.assertNotEqual(foreign.returncode, 0); self.assertIn('different file', foreign.stderr)
        cut = Path(self.tmp.name) / 'cut.rec'
        cut.write_text('\n'.join(records[2].read_text().splitlines()[:-2]) + '\n')
        truncated = mutate(a, '--merge', records[0], '--merge', records[1], '--merge', cut)
        self.assertNotEqual(truncated.returncode, 0); self.assertIn('truncated', truncated.stderr)
        both = run([COMPILER, 'mutate', a, '--shard', '1/2', '--merge', records[0]])
        self.assertNotEqual(both.returncode, 0)
        for bad in ('0/2', '3/2', '1/0', '1', 'x/2'):
            r = run([COMPILER, 'mutate', a, '--shard', bad])
            self.assertNotEqual(r.returncode, 0, bad); self.assertIn('--shard requires', r.stderr)
        for bad in ('0', '1025', 'many'):
            r = run([COMPILER, 'mutate', a, '--jobs', bad])
            self.assertNotEqual(r.returncode, 0, bad); self.assertIn('--jobs requires', r.stderr)

    def test_shard_exit_status_reflects_its_own_survivors(self):
        if self.sources: self.skipTest('explicit files given')
        path = Path(self.tmp.name) / 'dead.coax'
        path.write_text('f :: (x:int)->int { if x>10 { return x-3; } return x+1; } '
                        '#test "one" { #assert f(1)==2; } main :: () {}')
        statuses = [run([COMPILER, 'mutate', path, '--shard', f'{s}/3']) for s in (1, 2, 3)]
        self.assertTrue(any(r.returncode for r in statuses))
        self.assertTrue(all(r.stdout.startswith('coax-mutation-shard v1') for r in statuses))
        self.assertTrue(any('mutant survived' in r.stderr for r in statuses))


if __name__ == '__main__':
    Conformance.sources = [a for a in sys.argv[1:] if a.endswith('.coax')]
    unittest.main(argv=[sys.argv[0]] + [a for a in sys.argv[1:] if not a.endswith('.coax')])
