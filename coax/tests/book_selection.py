#!/usr/bin/env python3
"""Check the book runner's selection contract without executing real examples."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
RUNNER = ROOT / 'book' / 'check-examples.sh'


class BookSelection(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='coax-book-selection-')
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.book = self.base / 'coax' / 'book'
        self.book.mkdir(parents=True)
        shutil.copyfile(RUNNER, self.book / 'check-examples.sh')
        self.log = self.base / 'executed.jsonl'
        self.env = dict(os.environ, BOOK_SELECTION_LOG=str(self.log))
        for name, text in {
            'examples/alpha.coax': '#test "alpha check" {}\nmain :: () {}\n',
            'examples/alpha.out': 'alpha\n',
            'examples/alpha.release.out': 'alpha release\n',
            'examples/beta.coax': 'main :: () {}\n',
            'examples/beta.out': 'beta\n',
            'examples/beta.status': '7\n',
            'examples/compile_only.coax': 'main :: () {}\n',
            'examples/ch12_debug/main.coax': 'main :: () {}\n',
            'examples/ch12_debug/main.out': 'debug-main\n',
            'examples/lib/ch17_glow.coax': 'glow :: () {}\n',
            'examples/ch27_frontend/driver.coax': 'main :: () {}\n',
            'examples/ch27_frontend/driver.out': 'frontend\n',
            'examples/ch20_interop/run.out': 'interop\n',
            'examples/ch24_workflow/run.out': 'workflow\n',
        }.items():
            path = self.book / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text)
        compiler = self.base / 'coax' / 'bin' / 'coax'
        compiler.parent.mkdir()
        compiler.write_text('''#!/usr/bin/env python3
import json, os, pathlib, sys
args = sys.argv[1:]
with open(os.environ['BOOK_SELECTION_LOG'], 'a') as out:
    out.write(json.dumps(['compiler', *args]) + '\\n')
if args[0] == 'run':
    source = args[1]
    if source.endswith('/frontend.coax'):
        print('frontend')
    elif source.endswith('ch12_debug/main.coax'):
        print('debug-main')
    else:
        name = pathlib.Path(source).stem
        print(name + (' release' if name == 'alpha' and '--release' in args else ''))
        if name == 'beta':
            sys.exit(7)
''')
        compiler.chmod(0o755)
        build = self.base / 'coax' / 'build.sh'
        build.write_text('''#!/usr/bin/env python3
import json, os
with open(os.environ['BOOK_SELECTION_LOG'], 'a') as out:
    out.write(json.dumps(['build']) + '\\n')
raise SystemExit(1)
''')
        build.chmod(0o755)
        for relative, name in [('examples/ch20_interop/run.sh', 'interop'),
                               ('examples/ch24_workflow/run.sh', 'workflow')]:
            script = self.book / relative
            script.write_text(f'''#!/usr/bin/env python3
import json, os
with open(os.environ['BOOK_SELECTION_LOG'], 'a') as out:
    out.write(json.dumps(['{name}']) + '\\n')
print('{name}')
''')
            script.chmod(0o755)
        (self.book / 'check-structure.py').write_text('''import json, os
with open(os.environ['BOOK_SELECTION_LOG'], 'a') as out:
    out.write(json.dumps(['structure']) + '\\n')
print('structure')
''')

    def run_book(self, *args):
        return subprocess.run(['bash', str(self.book / 'check-examples.sh'), *args],
                              env=self.env, capture_output=True, text=True, timeout=10)

    def executed(self):
        return [json.loads(line) for line in self.log.read_text().splitlines()] if self.log.exists() else []

    def test_list_has_exact_names_and_executes_nothing(self):
        result = self.run_book('--list')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.splitlines(), [
            'alpha', 'beta', 'compile_only', 'ch12_debug/main', 'ch17_glow',
            'ch27_frontend', 'ch20_interop', 'ch15_requests.mutation',
            'ch24_workflow', 'structure'])
        self.assertEqual(self.executed(), [])

    def test_exact_selection_preserves_modes_and_authored_checks(self):
        result = self.run_book('--only', 'alpha')
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.executed(), [
            ['compiler', 'run', 'examples/alpha.coax'],
            ['compiler', 'run', 'examples/alpha.coax', '--release'],
            ['compiler', 'test', 'examples/alpha.coax']])
        self.assertIn('partial', result.stdout)
        self.assertNotIn('all book examples pass', result.stdout)

    def test_duplicate_names_execute_one_fixture_once_per_mode(self):
        result = self.run_book('--only', 'alpha', 'alpha')
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(len(self.executed()), 3)

    def test_invalid_selection_refuses_before_compiler_build_or_any_fixture(self):
        (self.base / 'coax' / 'bin' / 'coax').unlink()
        for args in [('--only',), ('--only', ''), ('--only', 'absent'),
                     ('--only', 'alpha', 'absent'), ('--only', 'al'),
                     ('--only', 'examples/alpha.coax'), ('--wat',)]:
            with self.subTest(args=args):
                result = self.run_book(*args)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(self.executed(), [], result.stdout + result.stderr)

    def test_list_can_filter_without_execution_or_a_compiler(self):
        (self.base / 'coax' / 'bin' / 'coax').unlink()
        result = self.run_book('--list', '--only', 'alpha', 'alpha')
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(result.stdout, 'alpha\n')
        self.assertEqual(self.executed(), [])

    def test_no_arguments_preserve_full_sequence_and_declared_exit_status(self):
        result = self.run_book()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        calls = self.executed()
        self.assertEqual(sum(call[:3] == ['compiler', 'run', 'examples/beta.coax'] for call in calls), 2)
        self.assertIn(['compiler', 'c', 'examples/compile_only.coax'], calls)
        self.assertIn(['compiler', 'test', 'examples/lib/ch17_glow.coax'], calls)
        self.assertIn(['compiler', 'metal', 'examples/lib/ch17_glow.coax'], calls)
        self.assertIn(['compiler', 'mutate', 'examples/ch15_requests.coax'], calls)
        self.assertEqual(calls[-3:], [['compiler', 'mutate', 'examples/ch15_requests.coax'],
                                     ['workflow'], ['structure']])
        self.assertIn('all book examples pass', result.stdout)

    def test_special_selection_does_not_run_other_hosts_or_mutation(self):
        result = self.run_book('--only', 'ch17_glow')
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.executed(), [['compiler', 'test', 'examples/lib/ch17_glow.coax'],
                                          ['compiler', 'metal', 'examples/lib/ch17_glow.coax']])

    def test_compile_only_and_structure_are_selectable(self):
        result = self.run_book('--only', 'compile_only', 'structure')
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.executed(), [['compiler', 'c', 'examples/compile_only.coax'], ['structure']])

    def test_output_mismatch_is_a_failure_in_selected_mode(self):
        (self.book / 'examples' / 'alpha.out').write_text('wrong\n')
        result = self.run_book('--only', 'alpha')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('FAIL examples/alpha.coax', result.stdout)
        self.assertNotIn('all book examples pass', result.stdout)


if __name__ == '__main__':
    unittest.main()
