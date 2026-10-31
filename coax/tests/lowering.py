#!/usr/bin/env python3
"""Ordered lowering (r1a-ordered-lowering): evaluator and native C agree on order.

The checked graph classifies each expression's observable effects
(CK_effects in lib/compiler/checker.coax: a call, a trap, an overflow trap
under checked arithmetic). The C emitter sequences operands and assignment
places from that classification, so native code never depends on C's
unspecified operand order. Each fixture runs in the evaluator (`coax test`)
and natively at -O0 and -O2, against expectations written here by hand.
A seeded emitter defect (reversed operand temporaries) must be caught and
reduced by the existing differential reducer (tools/differential.py
minimize). `coax deps FILE --lowering` publishes the same decisions as the
coax.lowering.v1 inspection record.
"""
from pathlib import Path
import os
import re
import shutil
import subprocess
import tempfile
import unittest
import importlib.util
import json
import sys
from dataclasses import replace

ROOT = Path(__file__).resolve().parents[1]
COAX = Path(os.environ.get("COAX_TEST_COMPILER", ROOT / "bin/coax")).resolve()
SPEC = importlib.util.spec_from_file_location("coax_differential_tool", ROOT / "tools/differential.py")
diff = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = diff
SPEC.loader.exec_module(diff)

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

    def lowering(self, path, *args):
        p = subprocess.run([str(COAX), "deps", str(path), "--lowering", *args], capture_output=True, text=True)
        self.assertEqual(p.returncode, 0, p.stderr)
        r = json.loads(p.stdout)
        self.assertEqual(r["schema"], "coax.lowering.v1")
        return r

    def test_the_inspection_record_is_the_executed_order(self):
        path, _ = self.case("two_traps", 0)
        r = self.lowering(path, "--proc", "f")
        (f,) = r["procedures"]
        ops = [(o["op"], o.get("effects"), o.get("sequenced")) for o in f["operations"]]
        self.assertEqual(ops, [("index", ["trap"], False), ("index", ["trap"], False), ("binary +", ["overflow"], True), ("return", None, None)])
        w = self.lowering(path, "--proc", "f", "--arithmetic", "wrapping")
        self.assertEqual([o.get("effects") for o in w["procedures"][0]["operations"]][:3], [["trap"], ["trap"], []])
        self.assertEqual(w["arithmetic"], "wrapping")
        compound, _ = self.case("compound_once", 0)
        bump = self.lowering(compound, "--proc", "bump")["procedures"][0]["operations"]
        (update,) = [o for o in bump if o["op"] == "compound +="]
        self.assertEqual((update["place_once"], update["staged"], update["effects"]), (True, True, ["overflow"]))
        cleanup, _ = self.case("cleanup", 0)
        trace = self.lowering(cleanup, "--proc", "trace")["procedures"][0]["operations"]
        lines = cleanup.read_text().splitlines()
        defers = [o["line"] for o in trace if o["op"] == "defer"]
        (done,) = [o for o in trace if o["op"] == "cleanup"]
        self.assertEqual(done["defers"], defers[::-1])
        self.assertTrue(all("defer" in lines[n - 1] for n in defers))
        self.assertEqual([o["op"] for o in trace][-2:], ["cleanup", "return"])
        # Procedures outside the subset are labelled, never reported as lowered.
        loop = self.base / "loop.coax"
        loop.write_text("l :: (a: [] int) -> int { s := 0; for x: a { s += x; } return s; }\nmain :: () {}\n")
        (l,) = self.lowering(loop, "--proc", "l")["procedures"]
        self.assertEqual((l["eligible"], l["reason"]), (False, "for loops are not in the lowering subset"))
        self.assertNotIn("operations", l)

    def test_the_hud_slot_lowering_names_its_checked_declaration(self):
        hud = ROOT.parent / "verse/hud.coax"
        (slot,) = self.lowering(hud, "--proc", "verse_hud_slot")["procedures"]
        decls = subprocess.run([str(COAX), "deps", str(hud), "--declarations"], capture_output=True, text=True, check=True)
        (row,) = [d for d in json.loads(decls.stdout)["declarations"] if d["name"] == "verse_hud_slot"]
        self.assertEqual((slot["id"], slot["span"]), (row["id"], row["span"]))
        self.assertEqual(slot["id"], "coax.decl.v1:verse.hud:proc:verse_hud_slot")
        self.assertTrue(slot["eligible"])
        ops = slot["operations"]
        self.assertTrue(all(Path(o["unit"]).name == "hud.coax" for o in ops))
        text = hud.read_text().splitlines()
        self.assertTrue(all(slot["span"]["start_line"] <= o["line"] <= slot["span"]["end_line"] for o in ops))
        self.assertEqual([o["op"] for o in ops].count("return"), 2)
        # Its authored behavior is unchanged: the HUD checks still pass.
        p = subprocess.run([str(COAX), "test", str(hud)], capture_output=True, text=True)
        self.assertIn("3 checks, 3 passed", p.stdout)

    def test_the_seeded_defect_is_reduced_by_the_differential_reducer(self):
        if not self.cc:
            self.skipTest("no native C compiler")
        # Noise statements around the two-trap read; inputs are (i, j) rows.
        noise = tuple(f"  w[{k % 4}] = w[{k % 4}] + {k};" for k in range(6))
        program = diff.Program(1, 0, noise[:3] + ("  r := a[i]\\n    + a[j];  // @read",) + noise[3:], ((1, 2), (7, 9), (0, 9)))
        seeded = [0]

        def render(candidate):
            lines = ['#import "Basic";', "probe :: (a: [] int, w: [] int, i: int, j: int) -> int {", "  r := 0;"]
            lines += [s.replace("r := ", "r = ").replace("\\n", "\n") for s in candidate.steps]
            lines += ["  return r;", "}", "main :: () {", "  v := [3] int.[1, 2, 3];", "  w: [4] int;"]
            lines += [f'  print("%\\n", probe(v[..], w[..], {i}, {j}));' for i, j in candidate.inputs]
            return "\n".join(lines + ["}", ""])

        def reverse(c):
            out, n = re.subn(r"\((coax_t\d+) = (\(\*coax_elem_v_s64\(l_a, l_i, \d+\)\)), (coax_t\d+) = (\(\*coax_elem_v_s64\(l_a, l_j, \d+\)\)), ",
                             lambda m: f"({m.group(3)} = {m.group(4)}, {m.group(1)} = {m.group(2)}, ", c)
            return out

        def mismatch(candidate):
            seeded[0] += 1
            path = self.base / f"reduce_{seeded[0]}.coax"
            path.write_text(render(candidate))
            plain = self.native(path)
            broken = self.native(path, reverse)
            return plain != broken
        self.assertTrue(mismatch(program))
        reduced, record = diff.minimize(program, mismatch, 48)
        self.assertFalse(record["budget_exhausted"])
        self.assertEqual(reduced.steps, (program.steps[3],))
        self.assertEqual(len(reduced.inputs), 1)
        self.assertTrue(mismatch(reduced))


if __name__ == "__main__":
    unittest.main()
