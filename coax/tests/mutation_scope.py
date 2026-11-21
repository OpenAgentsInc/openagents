#!/usr/bin/env python3
# Firm reason: 2 (independent compiler, ABI, diagnostic and native/Wasm-effect oracles; no product implementation)
"""Root mutation scope retains baseline checks and binds shard receipts."""
import os
from pathlib import Path
import re
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
COMPILER = Path(os.environ.get('COAX_TEST_COMPILER', ROOT / 'bin/coax'))
BASELINE = Path(os.environ.get('COAX_BASELINE_COMPILER', COMPILER))

class Scope(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='coax-mutation-scope-')
        self.addCleanup(self.tmp.cleanup)
        self.d = Path(self.tmp.name)
        (self.d / 'deps').mkdir()
        self.entry = self.d / 'case.coax'
        self.dependency = self.d / 'deps/case.coax'
        self.dependency.write_text('''
#load "leaf.coax";
dep :: (x:int)->int #effects(all) { if x>100 { return x-7; } return leaf(x); }
#test "imported" { #assert dep(2)==6; }
''')
        (self.d / 'deps/leaf.coax').write_text('leaf :: (x:int)->int #effects(all) { return x*3; }\n')
        self.entry.write_text('''
#authority;
#load "deps/case.coax";
root :: (x:int)->int #effects(all) { return dep(x)+1; }
#test "root" { #assert root(2)==7 && root(3)==10; }
main :: () {}
''')

    def run_mutate(self, *flags, compiler=COMPILER, source=None, jobs='1'):
        env = {**os.environ, 'COAX_JOBS_QUIET': '1'}
        return subprocess.run([str(compiler), 'mutate', str(source or self.entry), '--jobs', jobs, *flags],
                              capture_output=True, text=True, timeout=90, env=env)

    @staticmethod
    def timeless(result):
        # The baseline line holds a wall time (and a frozen compiler may not print it).
        return '\n'.join(x for x in result.stderr.splitlines() if not x.startswith('mutation-baseline '))

    def count(self, result):
        m = re.search(r'^(\d+) mutants?, (\d+) killed, (\d+) survived', result.stdout, re.M)
        self.assertIsNotNone(m, result.stdout + result.stderr)
        return tuple(map(int, m.groups()))

    def test_default_inventory_and_diagnostics_match_frozen_compiler(self):
        flags = ('--mutation-trace', '--mutation-stats')
        old = self.run_mutate(*flags, compiler=BASELINE)
        current = self.run_mutate(*flags)
        self.assertEqual((current.returncode, current.stdout, self.timeless(current)),
                         (old.returncode, old.stdout, self.timeless(old)))
        self.assertIn(str(self.dependency), current.stdout)
        self.assertEqual(self.count(current), (9, 5, 4))

    def test_default_receipts_match_frozen_compiler_and_remain_mergeable(self):
        records = []
        for shard in (1, 2):
            flags = ('--shard', f'{shard}/2')
            old = self.run_mutate(*flags, compiler=BASELINE)
            current = self.run_mutate(*flags)
            self.assertEqual((current.returncode, current.stdout, current.stderr),
                             (old.returncode, old.stdout, old.stderr))
            path = self.d / f'old-{shard}.rec';path.write_text(old.stdout);records.append(path)
        merged = self.merge(self.entry, records, False)
        self.assertEqual(self.count(merged), (9, 5, 4))

    def test_root_scope_excludes_nested_same_basename_dependencies_and_kills_root_mutants(self):
        full = self.run_mutate()
        root = self.run_mutate('--root-only')
        self.assertEqual(root.returncode, 0, root.stdout + root.stderr)
        self.assertIn('scope: root-only', root.stdout)
        total, killed, survived = self.count(root)
        self.assertEqual(total, 2);self.assertEqual((killed, survived), (total, 0))
        self.assertGreater(self.count(full)[0], total)
        self.assertNotIn(str(self.dependency), root.stdout)
        # A failing root implementation remains a failing baseline, not a pass.
        self.entry.write_text(self.entry.read_text().replace('dep(x)+1', 'dep(x)+2'))
        broken = self.run_mutate('--root-only')
        self.assertNotEqual(broken.returncode, 0);self.assertIn('checks fail before any mutation', broken.stderr)

    def test_empty_root_inventory_is_not_a_passing_gate(self):
        self.entry.write_text('#load "deps/case.coax"; #test "only dependency" { #assert dep(2)==6; } main :: () {}')
        r = self.run_mutate('--root-only')
        self.assertNotEqual(r.returncode, 0)
        self.assertEqual(self.count(r), (0, 0, 0));self.assertIn('no reachable mutation sites', r.stdout)

    def test_imported_failing_baseline_is_not_filtered(self):
        self.dependency.write_text(self.dependency.read_text().replace('dep(2)==6', 'dep(2)==99'))
        r = self.run_mutate('--root-only')
        self.assertNotEqual(r.returncode, 0)
        self.assertIn('checks fail before any mutation', r.stderr)
        self.assertIn(str(self.dependency), r.stderr)
        self.assertNotIn('mutation-shard', r.stdout)

    def records(self, source, root_only, name):
        paths = []
        for shard in (1, 2):
            flags = ['--shard', f'{shard}/2'] + (['--root-only'] if root_only else [])
            r = self.run_mutate(*flags, source=source)
            self.assertTrue(r.stdout.startswith('coax-mutation-shard v1\n'), r.stdout + r.stderr)
            p = self.d / f'{name}-{shard}.rec';p.write_text(r.stdout);paths.append(p)
        return paths

    def merge(self, source, paths, root_only):
        flags = ['--root-only'] if root_only else []
        for p in paths: flags += ['--merge', str(p)]
        return self.run_mutate(*flags, source=source)

    def test_root_shards_merge_to_same_report(self):
        direct = self.run_mutate('--root-only')
        merged = self.merge(self.entry, self.records(self.entry, True, 'root'), True)
        self.assertEqual((direct.returncode, direct.stdout), (merged.returncode, merged.stdout))

    def test_root_scope_parallel_report_matches_serial(self):
        flags = ['--root-only', '--mutation-trace', '--mutation-stats']
        serial = self.run_mutate(*flags)
        parallel = self.run_mutate(*flags, jobs='4')
        self.assertEqual((serial.returncode, serial.stdout, self.timeless(serial)),
                         (parallel.returncode, parallel.stdout, self.timeless(parallel)))

    def test_equal_inventory_scopes_still_cannot_mix_receipts(self):
        # No imports: both modes have exactly the same mutation sites.
        path = self.d / 'solo.coax'
        path.write_text('f :: (x:int)->int #effects(all) { return x+1; } #test "f" { #assert f(2)==3; } main :: () {}')
        full = self.records(path, False, 'full')
        root = self.records(path, True, 'root-solo')
        self.assertEqual(self.count(self.run_mutate(source=path)), self.count(self.run_mutate('--root-only', source=path)))
        for records, mode in [(full, True), (root, False), ([full[0], root[1]], False), ([root[0], full[1]], True)]:
            with self.subTest(mode=mode, records=records):
                r = self.merge(path, records, mode)
                self.assertNotEqual(r.returncode, 0);self.assertIn('different file, compiler or mutant set', r.stderr)
        self.assertEqual(self.merge(path, root, True).returncode, 0)
        self.assertEqual(self.merge(path, full, False).returncode, 0)

    def test_unit_scope_covers_one_unit_and_the_units_add_up(self):
        full = self.count(self.run_mutate())
        totals = 0
        for unit in (self.entry, self.dependency, self.d / 'deps/leaf.coax'):
            r = self.run_mutate('--unit', str(unit))
            self.assertIn('mutation scope: unit', r.stdout)
            for other in {self.entry, self.dependency, self.d / 'deps/leaf.coax'} - {unit}:
                self.assertNotIn(str(other) + ':', r.stdout)
            totals += self.count(r)[0]
        self.assertEqual(totals, full[0])
        # A spelling through another directory names the same unit.
        r = self.run_mutate('--unit', str(self.d / 'deps/../deps/case.coax'))
        self.assertEqual(self.count(r), self.count(self.run_mutate('--unit', str(self.dependency))))

    def test_unit_must_be_a_unit_of_the_graph_and_excludes_root_only(self):
        stray = self.d / 'stray.coax';stray.write_text('main :: () {}\n')
        r = self.run_mutate('--unit', str(stray))
        self.assertNotEqual(r.returncode, 0);self.assertIn('is not a source unit', r.stderr)
        r = self.run_mutate('--root-only', '--unit', str(self.dependency))
        self.assertNotEqual(r.returncode, 0)

    def test_unit_records_cannot_merge_with_full_ones(self):
        paths = []
        for shard in (1, 2):
            r = self.run_mutate('--shard', f'{shard}/2', '--unit', str(self.dependency))
            p = self.d / f'unit-{shard}.rec';p.write_text(r.stdout);paths.append(p)
        full = self.records(self.entry, False, 'full-entry')
        mixed = self.merge(self.entry, [paths[0], full[1]], False)
        self.assertNotEqual(mixed.returncode, 0)
        merged = self.run_mutate('--unit', str(self.dependency), '--merge', str(paths[0]), '--merge', str(paths[1]))
        direct = self.run_mutate('--unit', str(self.dependency))
        self.assertEqual((merged.returncode, merged.stdout), (direct.returncode, direct.stdout))

    def checks_run(self, result):
        m = re.search(r'mutation-scheduling \S+ checks=(\d+)', result.stderr)
        self.assertIsNotNone(m, result.stderr)
        return int(m.group(1))

    def test_killers_change_the_work_not_the_verdicts(self):
        path = self.d / 'killers.coax'
        path.write_text('f :: (x:int)->int #effects(all) { return x*3+1; }\n#test "weak one" { #assert f(0)>=0; }\n'
                        '#test "weak two" { #assert f(0)>=0; }\n#test "strong" { #assert f(2)==7 && f(0)==1; }\n'
                        'g :: (x:int)->int #effects(all) { if x>5 { return 1; } return 0; }\n#test "g" { #assert g(0)==0; }\nmain :: () {}\n')
        killers = self.d / 'killers.txt'
        plain = self.run_mutate('--mutation-stats', source=path)
        first = self.run_mutate('--mutation-stats', '--killers', str(killers), source=path)
        self.assertEqual((first.returncode, first.stdout), (plain.returncode, plain.stdout))
        self.assertTrue(killers.exists())
        recorded = killers.read_text()
        self.assertIn('\tstrong\n', recorded)
        self.assertNotEqual(plain.returncode, 0)  # g's x>5 survives: a survivor stays one
        hinted = self.run_mutate('--mutation-stats', '--killers', str(killers), source=path)
        self.assertEqual((hinted.returncode, hinted.stdout), (plain.returncode, plain.stdout))
        self.assertLess(self.checks_run(hinted), self.checks_run(plain))
        # Parallel and hinted: still the same report.
        parallel = self.run_mutate('--killers', str(killers), source=path, jobs='4')
        self.assertEqual((parallel.returncode, parallel.stdout), (plain.returncode, plain.stdout))
        # A killer recorded for a check that no longer kills (here: weakened)
        # falls back to the whole schedule; the survivor it leaves is reported.
        path.write_text(path.read_text().replace('f(2)==7 && f(0)==1', 'f(2)>=0'))
        weak = self.run_mutate('--killers', str(killers), source=path)
        self.assertEqual(weak.stdout, self.run_mutate(source=path).stdout)
        self.assertGreater(self.count(weak)[2], self.count(plain)[2])
        # A file of nonsense, or killers named for other checks, changes nothing.
        path.write_text(path.read_text().replace('f(2)>=0', 'f(2)==7 && f(0)==1'))
        killers.write_text('junk\n\t\t\t\t\t\nf\tf\t0\tx\tnowhere\tstrong\n')
        self.assertEqual(self.run_mutate('--killers', str(killers), source=path).stdout, plain.stdout)
        misled = ''.join(line.rsplit('\t', 1)[0] + '\tg\n' for line in recorded.splitlines())
        self.assertIn('\tg\n', misled)
        killers.write_text(misled)
        self.assertEqual(self.run_mutate('--killers', str(killers), source=path).stdout, plain.stdout)

    def git(self, *args):
        subprocess.run(['git', '-C', str(self.d), *args], check=True, capture_output=True, text=True, timeout=60,
                       env={**os.environ, 'GIT_AUTHOR_NAME': 't', 'GIT_AUTHOR_EMAIL': 't@t', 'GIT_COMMITTER_NAME': 't',
                            'GIT_COMMITTER_EMAIL': 't@t', 'GIT_CONFIG_GLOBAL': '/dev/null'})

    def test_changed_mutates_only_procedures_whose_text_differs(self):
        # #13583: --changed REF keeps every check and mutates what the change touched.
        self.git('init', '-q'); self.git('add', '-A'); self.git('commit', '-q', '-m', 'base')
        same = self.run_mutate('--changed', 'HEAD')
        self.assertEqual(same.returncode, 0, same.stdout + same.stderr)
        self.assertIn('mutation scope: changed since HEAD (0 of 3 procedures; every check retained)', same.stdout)
        self.assertIn('nothing to mutate', same.stdout)
        # #13610: with no mutation site in scope the baseline is not run at all.
        quiet = self.run_mutate('--changed', 'HEAD', '--mutation-stats')
        self.assertEqual(quiet.stdout, same.stdout); self.assertNotIn('mutation-baseline', quiet.stderr)
        # A comment or layout edit is no change.
        self.dependency.write_text(self.dependency.read_text().replace('dep :: (x:int)->int #effects(all) {',
                                                                       '// a note\ndep :: (x:int)->int   #effects(all) {\n'))
        self.assertIn('nothing to mutate', self.run_mutate('--changed', 'HEAD').stdout)
        # One procedure's body changes: only its mutants, and it still survives where it did.
        self.dependency.write_text(self.dependency.read_text().replace('x-7', 'x-8'))
        one = self.run_mutate('--changed', 'HEAD')
        self.assertIn('(1 of 3 procedures', one.stdout)
        self.assertIn('mutation-baseline', self.run_mutate('--changed', 'HEAD', '--mutation-stats').stderr)
        full = self.run_mutate()
        self.assertGreater(self.count(full)[0], self.count(one)[0])
        self.assertGreater(self.count(one)[2], 0)  # dep's x>100 branch has no check
        self.assertNotIn(str(self.d / 'deps/leaf.coax'), one.stdout)
        # Composes with --unit: the root unit has no change.
        root = self.run_mutate('--changed', 'HEAD', '--unit', str(self.entry))
        self.assertEqual(root.returncode, 0, root.stdout); self.assertIn('nothing to mutate', root.stdout)
        # A new unit is all changed; a bad REF is refused before the baseline.
        (self.d / 'deps/extra.coax').write_text('extra :: (x:int)->int #effects(all) { return x+5; }\n')
        self.dependency.write_text('#load "extra.coax";\n' + self.dependency.read_text().replace('dep(2)==6', 'dep(2)==6 && extra(1)==6'))
        new = self.run_mutate('--changed', 'HEAD', '--unit', str(self.d / 'deps/extra.coax'))
        self.assertEqual(new.returncode, 0, new.stdout + new.stderr); self.assertIn('(1 of 1 procedure;', new.stdout)
        bad = self.run_mutate('--changed', 'no-such-ref')
        self.assertNotEqual(bad.returncode, 0); self.assertIn('no such commit', bad.stderr)

    def test_changed_counts_generated_procedures_by_their_directive_and_descriptor(self):
        # #13614: procedures #import_proto generates have no text of their own. They are
        # changed only when the directive or the descriptor it reads changed since REF.
        pb = self.d / 'wire.pb'
        pb.write_bytes((ROOT / 'tests/proto/types.pb').read_bytes())
        source = self.d / 'wire.coax'
        source.write_text('''#import_proto "wire.pb" as X;
own :: (x:int)->int #effects(all) { return x+1; }
#test "decodes" { w:[1]u8; #assert X_Scalars_decode(w[0..<0]).error==0 && own(1)==2; }
main :: () {}
''')
        self.git('init', '-q'); self.git('add', '-A'); self.git('commit', '-q', '-m', 'base')
        scope = re.compile(r'mutation scope: changed since HEAD \((\d+) of (\d+) procedures?;')
        def changed():
            r = self.run_mutate('--changed', 'HEAD', source=source)
            self.assertNotEqual(r.returncode, 2, r.stdout + r.stderr)
            m = scope.search(r.stdout)
            self.assertIsNotNone(m, r.stdout + r.stderr)
            return int(m.group(1)), int(m.group(2))
        # Nothing changed: none of the generated procedures is in scope.
        none, total = changed()
        self.assertEqual(none, 0)
        self.assertGreater(total, 1)
        # A comment or layout edit of the directive is no change.
        source.write_text(source.read_text().replace('#import_proto "wire.pb" as X;', '// the wire\n#import_proto  "wire.pb"\n  as X;'))
        self.assertEqual(changed(), (0, total))
        # The descriptor's bytes change (its file name inside, the same messages): every
        # generated procedure is in scope, and own is not.
        pb.write_bytes(pb.read_bytes().replace(b'types.proto', b'typez.proto'))
        self.assertEqual(changed(), (total - 1, total))
        pb.write_bytes((ROOT / 'tests/proto/types.pb').read_bytes())
        self.assertEqual(changed(), (0, total))
        # The directive's text changes (the same descriptor by another path): in scope.
        source.write_text(source.read_text().replace('"wire.pb"', '"./wire.pb"'))
        self.assertEqual(changed(), (total - 1, total))
        # A descriptor REF doesn't have is changed.
        (self.d / 'other.pb').write_bytes(pb.read_bytes())
        source.write_text(source.read_text().replace('"./wire.pb"', '"other.pb"'))
        self.assertEqual(changed(), (total - 1, total))
        # Only own's body changes: only own.
        source.write_text(source.read_text().replace('"other.pb"', '"wire.pb"').replace('x+1', 'x+1+0'))
        self.assertEqual(changed(), (1, total))

    def test_flag_is_not_accepted_by_other_commands(self):
        r = subprocess.run([str(COMPILER), 'test', str(self.entry), '--root-only'], capture_output=True, text=True, timeout=90)
        self.assertNotEqual(r.returncode, 0)

if __name__ == '__main__': unittest.main()
