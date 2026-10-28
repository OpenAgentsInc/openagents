#!/usr/bin/env python3
"""coax test and coax mutate: checks run with their own budget, failures
print counterexamples, #authority builds run the checks, and mutation
testing reports the changes no check notices."""

import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
COMPILER = ROOT / "bin/coax"


def coax(*args):
    return subprocess.run([str(COMPILER), *args], capture_output=True, text=True, timeout=120)


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

    def test_runtime_assert_traps_in_release(self):
        path = self.write("main :: () { x := 3; #assert x == 4; }\n")
        r = coax("run", "--release", path)
        self.assertNotEqual(r.returncode, 0)
        self.assertIn(":1: assertion failed", r.stderr)


if __name__ == "__main__":
    unittest.main()
