#!/usr/bin/env python3
"""coax test and coax mutate: checks run with their own budget, failures
print counterexamples, #authority builds run the checks, and mutation
testing reports the changes no check notices."""

import os
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from quiet_crash import run as run_quiet_crash


ROOT = Path(__file__).resolve().parents[1]
COMPILER = ROOT / "bin/coax"


def coax(*args, expect_crash=False):
    runner = run_quiet_crash if expect_crash else subprocess.run
    return runner([str(COMPILER), *args], capture_output=True, text=True, timeout=120)


class Verify(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="coax-verify-")
        self.dir = Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()

    def write(self, text):
        path = self.dir / "checks.coax"
        path.write_text(text)
        return str(path)

    def test_forall_reports_the_first_counterexample(self):
        path = self.write(
            "#forall a: u8, b: bool { #assert a < 200 || b; }\n"
            "#test \"fine\" { #assert true; }\n"
            "main :: () { }\n")
        r = coax("test", path)
        self.assertEqual(r.returncode, 1)
        self.assertIn("FAIL #forall a: u8, b: bool", r.stdout)
        self.assertIn("ok   #test fine (1 case", r.stdout)
        self.assertIn("2 checks, 1 passed, 1 failed", r.stdout)
        self.assertIn("error[E0406]: assertion failed", r.stderr)
        self.assertIn("counterexample: a = 200, b = false", r.stderr)

    def test_only_runs_the_named_checks_and_list_names_them(self):
        path = self.write(
            "#authority;\n"
            "f :: (x: int) -> int { return x + 1; }\n"
            "g :: (x: int) -> int { return x * 2; }\n"
            "#test \"f adds one\" { #assert f(1) == 2; }\n"
            "#test \"g doubles\" { #assert g(2) == 4; }\n"
            "#forall x: 0..<4 { #assert f(x) > x; }\n"
            "main :: () { }\n")
        r = coax("test", path, "--list")
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(r.stdout.splitlines(), ["#test f adds one", "#test g doubles", "#forall x: 0..<4"])
        self.assertEqual(coax("test", path, "--list", "--only", "g ").stdout, "#test g doubles\n")
        # One check runs; g is never executed, but a filtered run doesn't judge coverage, and says so.
        r = coax("test", path, "--only", "adds")
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertRegex(r.stdout, r"ok   #test f adds one \(1 case, \d+ steps, \d+ ms\)")
        self.assertNotIn("g doubles", r.stdout)
        self.assertIn("1 of 3 checks (--only 'adds'), 1 passed, 0 failed", r.stdout)
        self.assertIn("authority execution coverage not checked", r.stdout)
        r = coax("test", path, "--only", "nothing like it")
        self.assertEqual(r.returncode, 1)
        self.assertIn("no check in", r.stderr)
        self.assertIn("3 checks, 3 passed, 0 failed", coax("test", path).stdout)

    def test_checks_have_larger_storage_limits_than_a_compile(self):
        # 2,000,000 slots: over a compile's 1,572,864, inside a check's own limit.
        path = self.write(
            "#test \"big\" { a: [2000000] u8; a[1999999] = 7; #assert a[1999999] == 7; }\n"
            "main :: () { }\n")
        r = coax("test", path)
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        r = coax("test", path, "--slots", "1572864")
        self.assertEqual(r.returncode, 1)
        self.assertIn("2000000 value slots needed, the limit is 1572864 (33554432 in coax test)", r.stderr)
        self.assertEqual(coax("test", path, "--slots", "33554433").returncode, 1)

    def test_domains_cover_every_value(self):
        path = self.write(
            "E :: enum s8 { a = -1; b = 7; }\n"
            "#forall x: s8, y: E, z: -2..<3 { #assert x >= -128; }\n"
            "main :: () { }\n")
        r = coax("test", path)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn("(2560 cases", r.stdout)

    def test_checks_have_their_own_budget(self):
        path = self.write(
            "#test \"spin\" { n := 0; while n < 1000000 { n += 1; } #assert n == 1000000; }\n"
            "main :: () { }\n")
        self.assertEqual(coax("test", path).returncode, 0)
        small = coax("test", path, "--steps", "1000")
        self.assertEqual(small.returncode, 1)
        self.assertIn("error[E0400]: compile-time evaluation exceeded 1000 steps", small.stderr)

    def test_failed_struct_expression_never_dereferences_empty_evaluator_value(self):
        for body in ('#assert false;', 'while true {}'):
            path = self.write(
                'Result :: struct { value: int; }\n'
                'bad :: () -> Result {' + body + ' return Result.{}; }\n'
                '#test "failed value" { #assert bad().value == 0; }\nmain :: () {}\n')
            r = coax('test', path, '--steps', '1000')
            self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
            self.assertIn('1 check, 0 passed, 1 failed', r.stdout)
            self.assertIn('error[E040', r.stderr)

    def test_checks_do_not_change_the_program(self):
        path = self.write(
            "#import \"Basic\";\n"
            "#test \"zebra\" { #assert false; }\n"
            "main :: () { print(\"ran\\n\"); }\n")
        r = coax("run", path)
        self.assertEqual((r.returncode, r.stdout), (0, "ran\n"), r.stderr)
        self.assertNotIn("zebra", coax("c", path).stdout)

    def test_authority_builds_run_the_checks(self):
        path = self.write(
            "#authority;\n"
            "inc :: (x: int) -> int { return x + 1; }\n"
            "#forall x: 0..<5 { #assert inc(x) > x; }\n"
            "main :: () { }\n")
        self.assertEqual(coax("check", path).returncode, 0)
        Path(path).write_text(Path(path).read_text().replace("x + 1", "x - 1"))
        r = coax("check", path)
        self.assertEqual(r.returncode, 1)
        self.assertIn("counterexample: x = 0", r.stderr)

    def test_the_money_example_kills_every_mutant(self):
        r = coax("mutate", str(ROOT / "examples/money.coax"))
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertRegex(r.stdout, r"\d+ mutants, (\d+) killed, 0 survived")

    def test_mutate_reports_survivors_and_equivalent_marks(self):
        path = self.write(
            "clamp :: (x: int, lo: int) -> int {\n"
            "  if x < lo { return lo; }  // mutant: equivalent\n"
            "  return x;\n"
            "}\n"
            "fee :: (x: int) -> int { return x / 100 + 1; }\n"
            "untested :: () -> int { return 3; }\n"
            "#forall x: -5..<5 { #assert clamp(x, 0) >= 0; #assert clamp(x, 0) == x || x < 0; }\n"
            "#test \"fee\" { #assert fee(1000) > 0; }\n"
            "main :: () { }\n")
        r = coax("mutate", path)
        self.assertEqual(r.returncode, 1)
        self.assertIn(":5:41: mutant survived: '+' became '-'", r.stdout)
        self.assertIn("not reached by any check: untested", r.stdout)
        self.assertIn("1 marked equivalent", r.stdout)
        self.assertNotIn(":2:", r.stdout.split("not reached")[0])

    def test_mutate_needs_green_checks(self):
        path = self.write("#test \"red\" { #assert false; }\nmain :: () { }\n")
        r = coax("mutate", path)
        self.assertEqual(r.returncode, 1)
        self.assertIn("make them pass first", r.stderr)

    def test_nonvacuous_domains_and_extreme_bounds(self):
        for domain,code in [('0..<0','E0379'),('3..<2','E0379'),
                            ('-9223372036854775808..<9223372036854775807','E0400'),
                            ('0..<16777217','E0400')]:
            path=self.write('#forall x: '+domain+' { #assert false; }\nmain :: () {}')
            r=coax('test',path);self.assertEqual(r.returncode,1);self.assertIn(code,r.stderr)
        path=self.write('#forall x: 9223372036854775806..<9223372036854775807 { #assert x==9223372036854775806; }\nmain :: () {}')
        r=coax('test',path);self.assertEqual(r.returncode,0,r.stderr)
        # The domain is represented lazily, so even its maximum cardinality
        # respects a tiny execution budget without first allocating 16M Values.
        path=self.write('#forall x: 0..<16777216 { #assert x>=0; }\nmain :: () {}')
        r=coax('test',path,'--steps','10');self.assertEqual(r.returncode,1);self.assertIn('E0400',r.stderr)

    def test_authority_requires_execution_not_dead_references(self):
        for body in ['if false { #assert f()==1; }', '#assert true || f()==1;', '#assert #run f()==1;']:
            path=self.write('#authority; f :: ()->int { return 1; }\n#test "dead" { '+body+' }\nmain :: () {}')
            for command in ('test','check','c','mutate'):
                r=coax(command,path);self.assertEqual(r.returncode,1,(command,r.stdout,r.stderr));self.assertIn('E0380',r.stderr)
            self.assertNotIn('-1 passed',coax('test',path).stdout)

    def test_empty_checks_and_uncovered_mutation_cannot_go_green(self):
        path=self.write('main :: () {}')
        r=coax('test',path);self.assertEqual(r.returncode,1);self.assertIn('no #test or #forall',r.stderr)
        path=self.write('f :: ()->int {return 1;} #test "empty" {#assert true;} main :: () {}')
        r=coax('mutate',path);self.assertEqual(r.returncode,1);self.assertIn('not reached by any check: f',r.stdout)
        path=self.write('#test "empty" {#assert true;} main :: () {}')
        r=coax('mutate',path);self.assertEqual(r.returncode,1);self.assertIn('no reachable mutation sites',r.stdout)

    def test_json_retains_counterexamples(self):
        path=self.write('#forall x: 0..<3 { #assert x<1; } main :: () {}')
        r=coax('test',path,'--json');self.assertEqual(r.returncode,1)
        diagnostic=json.loads(r.stderr.strip());self.assertEqual(diagnostic['code'],'E0406')
        self.assertIn('counterexample: x = 1',diagnostic['message'])

    def test_step_argument_is_strict(self):
        path=self.write('#test "ok" {#assert true;} main :: () {}')
        for value in ('0','-1','abc','2x','+1','18446744073709551616',' 100'):
            r=coax('test',path,'--steps',value);self.assertEqual(r.returncode,1);self.assertIn('positive uint64',r.stderr)

    def test_money_fee_preconditions_hold_in_release(self):
        source=(ROOT/'examples/money.coax').read_text().split('main :: ()')[0]
        for arguments in ('-1,1000','MAX_MSAT+1,1000','1,-1','MAX_MSAT,1000001'):
            path=self.write(source+'main :: () {fee:=fee_msat('+arguments+');}')
            for opts in ((),('--release',)):
                r=coax('run',path,*opts,expect_crash=True);self.assertNotEqual(r.returncode,0);self.assertIn('assertion failed',r.stderr)

    def test_equivalent_marks_must_be_actual_line_comments(self):
        for marker in ('note:="// mutant: equivalent";', '/* // mutant: equivalent */', '// mutant: equivalently'):
            # With a line comment, put it after the weak code; string/block
            # markers before it must not hide a surviving literal mutation.
            line='return x+1; '+marker if marker.startswith('//') else marker+' return x+1;'
            path=self.write('f :: (x:int)->int {'+line+'\n}\n#test "weak" {#assert f(0)>0;} main :: () {}')
            r=coax('mutate',path);self.assertEqual(r.returncode,1,r.stderr)
            self.assertIn('mutant survived',r.stdout);self.assertNotIn('marked equivalent',r.stdout)

    def test_runtime_assert_traps_in_release(self):
        path = self.write("main :: () { x := 3; #assert x == 4; }\n")
        r = coax("run", "--release", path, expect_crash=True)
        self.assertNotEqual(r.returncode, 0)
        self.assertIn(":1: assertion failed", r.stderr)


if __name__ == "__main__":
    unittest.main()
