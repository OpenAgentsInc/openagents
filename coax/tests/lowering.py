#!/usr/bin/env python3
"""Ordered lowering (r1a-ordered-lowering): evaluator and native C agree on order.

The checked graph classifies each expression's observable effects
(CK_effects in lib/compiler/checker.coax: a call, a trap, an overflow trap
under checked arithmetic). The C emitter sequences operands and assignment
places from that classification, so native code never depends on C's
unspecified operand order. Each fixture runs in the evaluator (`coax test`)
and natively at -O0 and -O2, against expectations written here by hand.
A seeded emitter defect (reversed operand temporaries) must be caught.
"""
from pathlib import Path
import os
import re
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
COAX = Path(os.environ.get("COAX_TEST_COMPILER", ROOT / "bin/coax")).resolve()

# Each fixture: procedures, then (call, expected): a printed value, or the
# line (named by a unique "// @tag" comment) where the first trap stops.
PROGRAMS = {
    # Two trapping operands without explicit calls: the left one traps first.
    "two_traps": ("f :: (a: [] int, i: int, j: int) -> int {\n  return a[i]  // @f-left\n    + a[j];  // @f-right\n}\n"
                  "g :: (a: [] int, i: int, j: int) -> int {\n  return a[i] /  // @g-left\n    a[j];  // @g-right\n}\n",
                  [("f(v[..], 7, 9)", "@f-left"), ("f(v[..], 0, 9)", "@f-right"), ("g(z[..], 0, 1)", "@g-left"), ("f(v[..], 1, 2)", "5")]),
    # The destination is evaluated before the value; both may trap.
    "destination_first": ("at :: (trail: [] int, k: int) -> int { trail[trail[0]] = k; trail[0] += 1; return k; }\n"
                          "put :: (a: [] int, i: int, j: int) -> int {\n  a[i] =  // @put-place\n    a[j];  // @put-value\n  return 0;\n}\n"
                          "order :: (a: [] int, trail: [] int) -> int {\n  trail[0] = 1;\n  a[at(trail, 1)] = at(trail, 2);\n  return trail[1] * 10 + trail[2];\n}\n",
                          [("put(v[..], 5, 6)", "@put-place"), ("put(v[..], 0, 6)", "@put-value"), ("order(v[..], w[..])", "12")]),
    # A compound assignment evaluates its destination exactly once, then the value.
    "compound_once": ("next :: (calls: [] int) -> int { calls[0] += 1; return calls[0]; }\n"
                      "bump :: (a: [] int, i: int, j: int) -> int {\n  a[i] +=  // @bump-place\n    a[j];  // @bump-value\n  return 0;\n}\n"
                      "once :: (a: [] int, calls: [] int) -> int {\n  a[next(calls)] += 5;\n  return calls[0] * 100 + a[1];\n}\n",
                      [("bump(v[..], 4, 8)", "@bump-place"), ("bump(v[..], 0, 8)", "@bump-value"), ("once(v[..], w[..])", "107")]),
    # Caller writes are visible and deferred cleanups run in reverse order.
    "cleanup": ("trace :: (out: [] int) -> int {\n  defer { out[0] = out[0] * 10 + 1; }\n"
                "  defer { out[0] = out[0] * 10 + 2; }\n  out[1] = 9;\n  return out[0];\n}\n"
                "observe :: (v: [] int) -> int { r := trace(v); return r * 1000 + v[0] * 10 + v[1]; }\n",
                [("observe(v[..])", "2219")]),
}


def lower_source(body, call, want):
    setup = "  v := [3] int.[1, 2, 3];\n  z := [2] int.[4, 0];\n  w := [4] int.[0, 0, 0, 0];\n"
    check = f"  r := {call};\n" + ("" if want.startswith("@") else f"  #assert r == {want};\n")
    return ('#import "Basic";\n' + body + '#test "case" {\n' + setup + check + "}\n" +
            "main :: () {\n" + setup + f'  print("%\\n", {call});\n}}\n')


class OrderedLowering(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="coax-lowering-")
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name)
        self.cc = shutil.which("cc") or shutil.which("clang")

    def case(self, name, index):
        body, calls = PROGRAMS[name]
        call, want = calls[index]
        path = self.base / f"{name}_{index}.coax"
        text = lower_source(body, call, want)
        path.write_text(text)
        if want.startswith("@"):
            (line,) = [n for n, l in enumerate(text.splitlines(), 1) if l.endswith("// " + want)]
            return path, line
        return path, want

    def evaluator(self, path):
        p = subprocess.run([str(COAX), "test", str(path), "--json"], capture_output=True, text=True)
        trap = re.search(r'"line": (\d+), "col": \d+, "code": "E04(?!06)', p.stderr)
        if trap:
            return ("trap", int(trap.group(1)))
        self.assertEqual(p.returncode, 0, p.stderr)
        return ("value", p.stdout)

    def native(self, path, seed=None):
        c = subprocess.run([str(COAX), "c", str(path)], capture_output=True, text=True, check=True).stdout
        if seed:
            c = seed(c)
        source = path.with_suffix(".c")
        source.write_text(c)
        out = []
        for level in ("-O0", "-O2"):
            exe = path.with_name(path.stem + level)
            subprocess.run([self.cc, "-std=c11", level, str(source), "-lm", "-o", str(exe)], check=True)
            p = subprocess.run([str(exe)], capture_output=True, text=True)
            trap = re.search(r":(\d+): ", p.stderr)
            out.append(("trap", int(trap.group(1))) if p.returncode and trap else ("value", p.stdout.strip()))
        return out

    def expect(self, want):
        return ("value", want) if isinstance(want, str) else ("trap", want)

    def test_evaluator_and_native_agree_with_the_written_expectations(self):
        if not self.cc:
            self.skipTest("no native C compiler")
        for name, (_, calls) in PROGRAMS.items():
            for index in range(len(calls)):
                with self.subTest(name=name, index=index):
                    path, want = self.case(name, index)
                    kind, got = self.evaluator(path)
                    if kind == "trap":
                        self.assertEqual(("trap", got), self.expect(want))
                    else:
                        self.assertIn("ok   #test case", got)
                        self.assertFalse(want.startswith("@"), (name, index, "expected a trap"))
                    for observed in self.native(path):
                        self.assertEqual(observed, self.expect(want))

    def test_trapping_operands_never_rely_on_c_operand_order(self):
        path, _ = self.case("two_traps", 0)
        c = subprocess.run([str(COAX), "c", str(path)], capture_output=True, text=True, check=True).stdout
        f = c[c.index("coax_p_f("):]
        f = f[f.index("{"):f.index("\n}\n")]
        # Both trapping reads go through temporaries, in source order, before the add.
        self.assertRegex(f, r"coax_t0 = \(\*coax_elem_v_s64\(l_a, l_i, 3\)\), coax_t1 = \(\*coax_elem_v_s64\(l_a, l_j, 4\)\), coax_add_s64\(coax_t0, coax_t1, 4\)")
        # Wrapping arithmetic removes the overflow trap but not the index traps.
        w = subprocess.run([str(COAX), "c", str(path), "--arithmetic", "wrapping"], capture_output=True, text=True, check=True).stdout
        self.assertIn("coax_t0 = (*coax_elem_v_s64(l_a, l_i, 3))", w)
        # One trapping operand needs no temporaries.
        one = self.base / "one.coax"
        one.write_text('h :: (a: [] int, i: int, k: int) -> int { return a[i] + k; }\nmain :: () {}\n')
        h = subprocess.run([str(COAX), "c", str(one)], capture_output=True, text=True, check=True).stdout
        self.assertNotIn("coax_t0", h[h.index("coax_p_h("):])

    def test_a_reversed_operand_order_defect_is_caught(self):
        if not self.cc:
            self.skipTest("no native C compiler")

        def reverse(c):
            # Seeded emitter defect: evaluate the two sequenced operands right to left.
            pattern = re.compile(r"\(coax_t0 = (\(\*coax_elem_v_s64\(l_a, l_i, \d+\)\)), coax_t1 = (\(\*coax_elem_v_s64\(l_a, l_j, \d+\)\)), ")
            seeded, count = pattern.subn(lambda m: f"(coax_t1 = {m.group(2)}, coax_t0 = {m.group(1)}, ", c)
            self.assertGreater(count, 0)
            return seeded
        path, want = self.case("two_traps", 0)
        caught = [observed for observed in self.native(path, reverse) if observed != self.expect(want)]
        right = [n for n, l in enumerate(path.read_text().splitlines(), 1) if l.endswith("// @f-right")]
        self.assertEqual(caught, [("trap", right[0])] * 2)


if __name__ == "__main__":
    unittest.main()
