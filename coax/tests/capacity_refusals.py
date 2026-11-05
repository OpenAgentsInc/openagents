#!/usr/bin/env python3
# Firm reason: 2 (independent capacity-diagnostic oracle using authored limits and emitted JSON).
"""Capacity refusals and near-limit notes name the resource, its limit and use,
the top consumers with their source locations, and one reduction (#12926).

Each limit has a program that exceeds it: compile-time steps (a compile's and a
check's), aggregate traversal, value slots (a compile's and the checks'),
generic instances (E0339) and the checker's node store (E0381). Each is read
in text, in --diagnostics json and in the one-line --json format, and the
near-limit note appears at its threshold (--near-limit or COAX_NEAR_LIMIT).
"""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
COMPILER = Path(os.environ.get('COAX', ROOT / 'bin/coax')).resolve()

STEPS = ('build :: (n: int) -> int { s := 0; for i: 0..<n { s += i; } return s; }\n'
         'small :: () -> int { return 1; }\n'
         'total :: () -> int { return build(40000) + small(); }\n'
         'X :: #run total();\n'
         'main :: () {}\n')

GAME = ('Game :: struct { cells: [1024] int; tick: int; }\n'
        'step :: (g: Game) -> int { return g.tick + 1; }\n'
        '#test "page" {\n'
        '  g: Game;\n'
        '  t := 0;\n'
        '  for i: 0..<2000 { t += step(g); }\n'
        '  #assert t == 2000;\n'
        '}\n'
        'main :: () {}\n')

SLOTS = ('#test "big tables" {\n'
         '  a: [60000] u8;\n'
         '  b: [30000] u8;\n'
         '  #assert a[0] == b[0];\n'
         '}\n'
         'main :: () {}\n')


def instances(count):
    params = ', '.join(f'a{i}: S({i})' for i in range(1, count + 1))
    return 'S :: struct($N) { v: [$N] int; }\nf :: (' + params + ') {}\nmain :: () {}\n'


def node_store(count, statements):
    """count instances of a generic with a long body fill the checker's node store."""
    body = ''.join(f'  x = x + cast(int) a[{i % 1}];\n' for i in range(statements))
    calls = ''.join(f'  b{i}: [{i}] int; y += sum(b{i});\n' for i in range(1, count + 1))
    return ('sum :: (a: [$N] int) -> int {\n  x := 0;\n' + body + '  return x;\n}\n'
            'main :: () {\n  y := 0;\n' + calls + '}\n')


class CapacityRefusals(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='coax-capacity-')
        self.addCleanup(self.tmp.cleanup)
        self.path = Path(self.tmp.name) / 'case.coax'

    def run_coax(self, command, source, *args, env=None):
        self.path.write_text(source)
        environment = dict(os.environ)
        environment.pop('COAX_NEAR_LIMIT', None)
        # A fresh check cache each run: a cached verdict runs no checks, so it reports no use.
        environment['COAX_CHECK_CACHE'] = tempfile.mkdtemp(dir=self.tmp.name)
        environment.update(env or {})
        return subprocess.run([str(COMPILER), command, str(self.path), *args], text=True, capture_output=True,
                              timeout=120, env=environment)

    def line_of(self, source, text):
        for number, line in enumerate(source.splitlines(), 1):
            if text in line:
                return number
        raise AssertionError(text)

    def refusal(self, command, source, resource, code, *args):
        """The refusal in all three formats; returns the --diagnostics json capacity object."""
        text = self.run_coax(command, source, *args)
        self.assertNotEqual(text.returncode, 0, text.stderr)
        self.assertIn(f'error[{code}]', text.stderr)
        self.assertIn(f'  {resource}: ', text.stderr)
        self.assertIn(f'  top users of {resource}:', text.stderr)
        self.assertIn('  fix: ', text.stderr)
        rich = self.run_coax(command, source, *args, '--diagnostics', 'json')
        records = [json.loads(x) for x in rich.stderr.splitlines() if x.startswith('{')]
        errors = [r for r in records if r['severity'] == 'error' and 'capacity' in r]
        self.assertEqual(len(errors), 1, rich.stderr)
        capacity = errors[0]['capacity']
        self.assertEqual(errors[0]['code'], code)
        self.assertEqual(capacity['resource'], resource)
        self.assertGreater(capacity['used'], capacity['limit'])
        self.assertTrue(capacity['consumers'], capacity)
        self.assertLessEqual(len(capacity['consumers']), 5)
        amounts = [c['amount'] for c in capacity['consumers']]
        self.assertEqual(amounts, sorted(amounts, reverse=True))
        self.assertTrue(capacity['suggestion'])
        self.assertIn(capacity['suggestion'], errors[0]['fixes'][0]['message'])
        for consumer in capacity['consumers']:
            self.assertEqual(Path(consumer['span']['file']).name, 'case.coax')
        line = self.run_coax(command, source, *args, '--json')
        rows = [json.loads(x) for x in line.stderr.splitlines() if x.startswith('{') and '"capacity"' in x]
        self.assertEqual(len(rows), 1, line.stderr)
        self.assertEqual(rows[0]['capacity']['resource'], resource)
        self.assertEqual([c['amount'] for c in rows[0]['capacity']['consumers']], amounts)
        self.assertEqual([c['line'] for c in rows[0]['capacity']['consumers']],
                         [c['span']['line'] for c in capacity['consumers']])
        return capacity, text.stderr

    def near(self, command, source, resource, *args, env=None):
        """The near-limit note in text and --diagnostics json; returns the json capacity object."""
        text = self.run_coax(command, source, *args, env=env)
        self.assertEqual(text.returncode, 0, text.stderr)
        notes = [line for line in text.stderr.splitlines() if line.startswith('coax: note[') and f']: {resource}: ' in line]
        self.assertEqual(len(notes), 1, text.stderr)
        self.assertIn(f'  top users of {resource}:', text.stderr)
        rich = self.run_coax(command, source, *args, '--diagnostics', 'json', env=env)
        records = [json.loads(x) for x in rich.stderr.splitlines() if x.startswith('{')]
        mine = [r for r in records if r['severity'] == 'note' and r['message'].startswith(resource + ':')]
        self.assertEqual(len(mine), 1, rich.stderr)
        self.assertIsNone(mine[0]['span'])
        capacity = mine[0]['capacity']
        self.assertEqual(capacity['resource'], resource)
        self.assertTrue(capacity['consumers'], capacity)
        self.assertTrue(capacity['suggestion'])
        return capacity

    def quiet(self, command, source, resource, *args, env=None):
        r = self.run_coax(command, source, *args, env=env)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertNotIn(f']: {resource}: ', r.stderr)

    # ---- compile-time steps ----

    def test_compile_steps_refusal_names_the_procedure(self):
        capacity, text = self.refusal('check', STEPS, 'compile-time steps', 'E0400')
        self.assertEqual(capacity['limit'], 100000)
        top = capacity['consumers'][0]
        self.assertIn("'build'", top['what'])
        self.assertEqual(top['span']['line'], self.line_of(STEPS, 'build ::'))
        self.assertIn('#run_steps', capacity['suggestion'])
        self.assertIn("compute 'build'", capacity['suggestion'])
        self.assertIn('steps  ', text)

    def test_compile_steps_near_limit_and_threshold(self):
        r = self.run_coax('check', STEPS, '--run-steps', '250000000', '--timings', str(Path(self.tmp.name) / 't.json'))
        self.assertEqual(r.returncode, 0, r.stderr)
        used = {x['name']: x for x in json.loads((Path(self.tmp.name) / 't.json').read_text())['capacities']}['compile-time steps']['used']
        limit = used * 10 // 8  # 80% used
        capacity = self.near('check', STEPS, 'compile-time steps', '--run-steps', str(limit))
        self.assertEqual((capacity['used'], capacity['limit'], capacity['near_percent']), (used, limit, 75))
        self.assertIn("'build'", capacity['consumers'][0]['what'])
        # The threshold is configurable: above 80% says nothing, 50% reports.
        self.quiet('check', STEPS, 'compile-time steps', '--run-steps', str(limit), '--near-limit', '90')
        self.quiet('check', STEPS, 'compile-time steps', '--run-steps', str(limit), env={'COAX_NEAR_LIMIT': '90'})
        loose = self.near('check', STEPS, 'compile-time steps', '--run-steps', str(used * 2), '--near-limit', '40')
        self.assertEqual(loose['near_percent'], 40)
        self.near('check', STEPS, 'compile-time steps', '--run-steps', str(used * 2), env={'COAX_NEAR_LIMIT': '40'})

    def test_check_steps_near_limit_from_a_parallel_check(self):
        """#authority checks run in child processes report only their totals: the note names the check."""
        source = ('#authority;\n'
                  'spin :: (n: int) -> int { s := 0; for i: 0..<n { s += i; } return s; }\n'
                  '#test "heavy" #steps 40000 { #assert spin(8000) > 0; }\n'
                  '#test "light" { #assert spin(3) == 3; }\n'
                  'main :: () {}\n')
        timings = Path(self.tmp.name) / 't.json'
        capacity = self.near('check', source, 'compile-time steps', '--timings', str(timings), env={'COAX_CHECK_JOBS': '2'})
        self.assertNotIn('sequential', json.loads(timings.read_text())['parallel_checks'])
        self.assertEqual(capacity['limit'], 40000)
        self.assertEqual(capacity['consumers'][0]['what'], "#test 'heavy'")
        self.assertEqual(capacity['consumers'][0]['span']['line'], 3)
        self.assertIn("coax test --only 'heavy'", capacity['suggestion'])

    def test_near_limit_option_is_validated(self):
        for value in ('0', '101', '', 'x', '7.5'):
            r = self.run_coax('check', STEPS, '--near-limit', value)
            self.assertNotEqual(r.returncode, 0)
            self.assertIn('--near-limit requires a whole percent from 1 to 100', r.stderr)

    def test_check_steps_refusal_names_the_procedure(self):
        source = STEPS.replace('X :: #run total();\n', '') + \
            '#test "slow" #steps 5000 { #assert build(4000) > small(); }\n'
        capacity, text = self.refusal('test', source, 'compile-time steps', 'E0400')
        self.assertEqual(capacity['limit'], 5000)
        self.assertIn("'build'", capacity['consumers'][0]['what'])
        self.assertIn('larger #steps N', capacity['suggestion'])
        self.assertIn('note: in #test slow', text)

    # ---- aggregate traversal ----

    def test_traversal_refusal_names_the_copy_at_its_call_site(self):
        capacity, text = self.refusal('test', GAME, 'compile-time traversal work', 'E0400', '--traversal', '1000000')
        self.assertEqual(capacity['limit'], 1000000)
        top = capacity['consumers'][0]
        self.assertTrue(top['what'].startswith("copying arguments into 'step'"), top)
        self.assertEqual(top['span']['line'], self.line_of(GAME, 't += step(g)'))
        self.assertIn('pass a view, an index or only the fields it needs', capacity['suggestion'])
        self.assertIn('#traversal N', capacity['suggestion'])
        self.assertIn('visits needed, the limit is 1000000', text)

    def test_traversal_budget_is_adjustable(self):
        r = self.run_coax('test', GAME, '--traversal', '3000000')
        self.assertEqual(r.returncode, 0, r.stderr)
        for value in ('0', '', 'x', '68719476737'):
            r = self.run_coax('test', GAME, '--traversal', value)
            self.assertNotEqual(r.returncode, 0)
            self.assertIn('--traversal requires one positive count', r.stderr)
        own = GAME.replace('#test "page" {', '#test "page" #traversal 1000000 {')
        r = self.run_coax('test', own)
        self.assertNotEqual(r.returncode, 0)
        self.assertIn('the limit is 1000000', r.stderr)
        r = self.run_coax('test', own.replace('#traversal 1000000', '#traversal 3000000 #steps 20000000'), '--traversal', '1000')
        self.assertEqual(r.returncode, 0, r.stderr)

    def test_traversal_near_limit(self):
        # 2000 copies of 1025 values, about 2.05 million visits: 80% of 2.6 million.
        capacity = self.near('test', GAME, 'compile-time traversal work', '--traversal', '2600000')
        self.assertTrue(capacity['consumers'][0]['what'].startswith("copying arguments into 'step'"))
        self.quiet('test', GAME, 'compile-time traversal work', '--traversal', '2600000', '--near-limit', '90')

    # ---- value slots ----

    def test_check_slots_refusal_names_the_arrays(self):
        capacity, _ = self.refusal('test', SLOTS, 'compile-time value slots', 'E0400', '--slots', '80000')
        self.assertEqual(capacity['limit'], 80000)
        lines = [c['span']['line'] for c in capacity['consumers']]
        self.assertEqual(lines[0], self.line_of(SLOTS, 'a: [60000]'))
        self.assertIn(self.line_of(SLOTS, 'b: [30000]'), lines)
        self.assertEqual(capacity['consumers'][0]['amount'], 60000)
        self.assertIn('split the check', capacity['suggestion'])

    def test_check_slots_near_limit(self):
        capacity = self.near('test', SLOTS, 'compile-time value slots', '--slots', '100000')
        self.assertEqual(capacity['consumers'][0]['amount'], 60000)
        self.assertEqual(capacity['consumers'][0]['span']['line'], self.line_of(SLOTS, 'a: [60000]'))
        self.quiet('test', SLOTS, 'compile-time value slots', '--slots', '100000', '--near-limit', '95')

    def test_compile_slots_refusal_and_near_limit(self):
        def program(n):
            return f'table :: () -> int {{ a: [{n}] u8; return 1; }}\nX :: #run table();\nmain :: () {{}}\n'
        capacity, _ = self.refusal('check', program(1572865), 'compile-time value slots', 'E0400', '--run-steps', '250000000')
        self.assertEqual(capacity['limit'], 1572864)
        self.assertIn("in 'table'", capacity['consumers'][0]['what'])
        self.assertIn('at run time', capacity['suggestion'])
        near = self.near('check', program(1300000), 'compile-time value slots', '--run-steps', '250000000')
        self.assertEqual(near['consumers'][0]['amount'], 1300000)
        self.assertEqual(near['consumers'][0]['span']['line'], 1)

    # ---- generic instances (E0339) ----

    def test_generic_instance_refusal_names_the_template(self):
        source = instances(257)
        capacity, text = self.refusal('check', source, 'generic instances', 'E0339')
        self.assertEqual((capacity['limit'], capacity['consumers'][0]['what']), (256, "'S'"))
        self.assertEqual(capacity['consumers'][0]['amount'], 256)
        self.assertEqual(capacity['consumers'][0]['span']['line'], 1)
        self.assertIn("bind 'S' to fewer distinct types or sizes", capacity['suggestion'])
        self.assertIn('a chain of generic uses', text)  # the checker's own fix stays

    def test_generic_instance_near_limit(self):
        capacity = self.near('check', instances(200), 'generic instances')
        self.assertEqual((capacity['used'], capacity['limit']), (200, 256))
        self.assertEqual(capacity['consumers'][0]['what'], "'S'")
        self.quiet('check', instances(200), 'generic instances', '--near-limit', '80')

    # ---- the checker's node store (E0381) ----

    def test_node_store_refusal_names_the_template(self):
        source = node_store(120, 120)
        capacity, _ = self.refusal('check', source, 'checker node store', 'E0381')
        self.assertTrue(capacity['consumers'][0]['what'].startswith("'sum'"), capacity)
        self.assertEqual(capacity['consumers'][0]['span']['line'], 1)
        self.assertIn("make 'sum''s body smaller", capacity['suggestion'])

    def test_node_store_near_limit(self):
        capacity = self.near('check', node_store(20, 120), 'checker node store', '--near-limit', '1')
        self.assertTrue(capacity['consumers'][0]['what'].startswith("'sum' (20 instances)"), capacity)


if __name__ == '__main__':
    unittest.main()
