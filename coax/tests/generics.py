#!/usr/bin/env python3
"""Generic procedure signatures through the real driver (spec 7.2, r4a-type-parameters).

TypeParameters: `$T` binds a placeholder type at its first parameter use and
every other use is checked against it; each refused form reports its own
explained code; a generic procedure is never emitted, so a program's C is
unchanged by one, and every call is E0397 rather than a guessed instance.
The binding rules are checked and mutation-tested in lib/compiler/checker.coax
(the $ token in lexer.coax, the type node in parser.coax); these fixtures pin
the composed behavior of lexer, parser, checker, bridges and emitters.
"""
from pathlib import Path
import json
import os
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
COAX = Path(os.environ.get("COAX_TEST_COMPILER", ROOT / "bin/coax")).resolve()

GENERIC = (
    "first :: (v: [] $T) -> $T { return v[0]; }\n"
    "swap :: (a: [] $T, i: int, j: int) { t := a[i]; a[i] = a[j]; a[j] = t; }\n"
    "pick :: (a: $T, b: $T, second: bool) -> $T { r: $T = a; if second { r = b; } return r; }\n"
    "count :: (v: [] $T) -> int { n := 0; for x, i: v { n += 1; } return n; }\n"
)
PROGRAM = (
    '#import "Basic";\n'
    "total :: (v: [] int) -> int { s := 0; for x: v { s += x; } return s; }\n"
    'main :: () { a := [3] int.[4, 5, 6]; print("%\\n", total(a[..])); }\n'
)
REFUSED = [
    ("make :: () -> $T { x: $T; return x; }", ["E0392", "E0393"]),
    ("f :: (a: $T) { b: $U; }", ["E0393"]),
    ("K :: size_of($T);", ["E0393"]),
    ('#test "t" { x: $T; }', ["E0393"]),
    ("f :: (a: [4] $T) {}", ["E0394"]),
    ("f :: (a: [] [] $T) {}", ["E0394"]),
    ("S :: struct { x: $T; }", ["E0395"]),
    ("f :: (a: $T) -> $T { return f(a); }", ["E0396"]),
    ("f :: (a: $T) -> $T { return a; } g :: () -> int { return f(1); }", ["E0397"]),
    ("f :: (a: $T) -> $T #export_c { return a; }", ["E0398"]),
    ("f :: (a: $T) -> int { return size_of($T); }", ["E0399"]),
    ("f :: (a: $T) -> string { return type_name(a); }", ["E0399"]),
    ("f :: (a: $T) -> $T { return 1; }", ["E0330"]),
    ("f :: (a: $T, b: $U) { c := a; c = b; }", ["E0327"]),
    ("g :: (x: int) {} f :: (a: $T) { g(a); }", ["E0317"]),
    ("f :: (a: $T, b: $T) -> bool { return a == b; }", ["E0311"]),
    ("f :: (a: $T, b: $T) -> bool { return a < b; }", ["E0310"]),
    ("f :: (a: $T) { x := a + a; }", ["E0309"]),
    ("f :: (a: $ T) {}", ["E0108"]),
    ("f :: () { x := $T; }", ["E0205"]),
]


class TypeParameters(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="coax-generics-")
        self.addCleanup(self.tmp.cleanup)
        self.d = Path(self.tmp.name)

    def write(self, name, text):
        path = self.d / name
        path.write_text(text)
        return path

    def coax(self, *args):
        return subprocess.run([str(COAX), *map(str, args)], capture_output=True, text=True, timeout=120)

    def codes(self, path, *extra):
        r = self.coax("check", path, "--json", *extra)
        return r.returncode, [json.loads(line)["code"] for line in r.stderr.splitlines() if line.startswith("{")]

    def test_generic_procedures_check_and_leave_the_program_unchanged(self):
        (self.d / "plain").mkdir()
        plain = self.write("plain/program.coax", PROGRAM)
        generic = self.write("program.coax", PROGRAM + GENERIC)
        self.assertEqual(self.coax("check", generic).returncode, 0, self.coax("check", generic).stderr)
        run = self.coax("run", generic)
        self.assertEqual((run.returncode, run.stdout), (0, "15\n"), run.stderr)
        c_plain, c_generic = self.coax("c", plain), self.coax("c", generic)
        self.assertEqual(c_generic.returncode, 0, c_generic.stderr)
        # No instance exists, so nothing of a generic procedure reaches C.
        self.assertEqual(c_plain.stdout.replace(str(plain), "PROGRAM"), c_generic.stdout.replace(str(generic), "PROGRAM"))
        for name in ("first", "swap", "pick", "count"):
            self.assertNotIn("coax_p_" + name, c_generic.stdout)
        layout = self.coax("layout", generic)
        self.assertEqual(layout.returncode, 0, layout.stderr)
        self.assertNotIn("$T", layout.stdout)

    def test_declarations_list_generic_procedures(self):
        path = self.write("decls.coax", '#module "app.generic";\n' + PROGRAM + GENERIC)
        r = self.coax("deps", path, "--declarations")
        self.assertEqual(r.returncode, 0, r.stderr)
        ids = [d["id"] for d in json.loads(r.stdout)["declarations"]]
        for name in ("main", "total", "first", "swap", "pick", "count"):
            self.assertIn(f"coax.decl.v1:app.generic:proc:{name}", ids)

    def test_checks_metal_and_mutation_ignore_generic_procedures(self):
        source = GENERIC + "double :: (x: int) -> int { return x * 2; }\nmain :: () {}\n" + '#test "double" { #assert double(3) == 6; }\n'
        path = self.write("checks.coax", source)
        r = self.coax("test", path)
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertIn("1 check, 1 passed, 0 failed", r.stdout)
        r = self.coax("mutate", path)
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertRegex(r.stdout + r.stderr, r"(\d+) mutants, \1 killed, 0 survived")
        metal = self.write("metal.coax", "scale :: (x: f32) -> f32 { return x * 2.0; }\n" + GENERIC)
        r = self.coax("metal", metal)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn("scale", r.stdout)
        self.assertNotIn("first", r.stdout)

    def test_authority_files_still_require_every_procedure_checked(self):
        path = self.write("authority.coax", "#authority;\n" + GENERIC)
        code, codes = self.codes(path, "--library")
        self.assertNotEqual(code, 0)
        self.assertEqual(set(codes), {"E0380"})

    def test_refused_forms_report_their_codes(self):
        for i, (source, want) in enumerate(REFUSED):
            with self.subTest(source=source):
                path = self.write(f"refused{i}.coax", source + "\nmain :: () {}\n")
                code, codes = self.codes(path)
                self.assertNotEqual(code, 0)
                self.assertEqual(codes, want)

    def test_every_new_code_is_explained(self):
        for code in ("E0392", "E0393", "E0394", "E0395", "E0396", "E0397", "E0398", "E0399"):
            r = self.coax("explain", code)
            self.assertEqual(r.returncode, 0, r.stderr)
            self.assertTrue(r.stdout.startswith(code + ": "), r.stdout)
            self.assertGreater(len(r.stdout), 80)


if __name__ == "__main__":
    unittest.main()
