#!/usr/bin/env python3
"""The native-check runner (coax/tools/native_checks.py, r0-native-check-exe).

Each fixture's expected outcome is written here, independently of the runner:
authored checks agree across the evaluator, checked -O0 and checked -O2
(passed, the same failing assertion, the same trap); seeded evaluator and
emitter defects are counterexamples, minimised to one test cut after the
disagreeing assertion; ineligible checks, files that don't run, a missing
compiler and timeouts are never "passed"; the harness never reaches product
emission. The last test runs the documented selection of real Verse and
Psionic checks (coax/tests/native-checks.json).
"""
import importlib.util
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("coax_native_checks", ROOT / "tools/native_checks.py")
nc = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = nc
SPEC.loader.exec_module(nc)

AGREE = '''scale :: (a: int, b: int) -> int { return a * b; }
fill :: (out: [] int, v: int) { defer { out[0] += 1; } for i: 0..<out.count { out[i] = v; } }
#test "products" {
  #assert scale(6, 7) == 42;
  #assert scale(-3, 5) == -15;
}
#test "a caller's buffer: its view written, the guards kept, the cleanup last" {
  buf := [5] int.[-9, 0, 0, 0, -8];
  fill(buf[1..<4], 4);
  #assert buf[0] == -9 && buf[4] == -8;
  #assert buf[1] == 5 && buf[2] == 4 && buf[3] == 4;
}
#test "fails at its second assertion" {
  #assert scale(2, 2) == 4;
  #assert scale(2, 3) == 7;
  #assert scale(1, 1) == 1;
}
#test "traps: checked overflow" {
  big := 9223372036854775807;
  #assert scale(big, 2) > 0;
}
#test "runs after a trap" {
  #assert scale(3, 3) == 9;
}
#forall "not native" x: 0..<3 { #assert scale(x, 1) == x; }
#test "uses #run" { #assert #run scale(2, 2) == 4; }
'''


class NativeChecks(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.dir = Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()

    def fixture(self, text: str, name: str = "fixture.coax") -> Path:
        p = self.dir / name
        p.write_text(text)
        return p

    def by_name(self, rec: dict) -> dict:
        return {c["name"]: c for f in rec["files"] for c in f["checks"]}

    def test_authored_checks_agree_across_the_evaluator_and_both_profiles(self):
        rec = nc.run([self.fixture(AGREE)], keep=self.dir / "keep")
        c = self.by_name(rec)
        self.assertEqual(c["products"]["outcome"], "passed")
        self.assertEqual(c["a caller's buffer: its view written, the guards kept, the cleanup last"]["outcome"], "passed")
        self.assertEqual(c["fails at its second assertion"]["outcome"], "agreed-assert")
        for who in ("evaluator", "-O0", "-O2"):
            self.assertEqual(c["fails at its second assertion"][who], ["assert", 2])
            self.assertEqual(c["traps: checked overflow"][who][0], "trap")
        self.assertEqual(c["traps: checked overflow"]["outcome"], "agreed-trap")
        self.assertEqual(c["runs after a trap"]["outcome"], "passed", "a trap doesn't end the others")
        self.assertEqual(c["uses #run"]["outcome"], "rejected")
        self.assertEqual(rec["outcome"], "passed")
        self.assertEqual(rec["profiles"]["arithmetic"], "checked")
        self.assertEqual(sorted(rec["files"][0]["native_identities"]), ["-O0", "-O2"])
        self.assertNotIn("not native", c, "a #forall is not a #test: it isn't selected")

    def test_a_seeded_emitter_defect_is_a_minimised_counterexample(self):
        keep = self.dir / "keep"
        rec = nc.run([self.fixture(AGREE)], seed="emitter", keep=keep)
        c = self.by_name(rec)
        self.assertEqual(rec["outcome"], "counterexample")
        self.assertEqual(c["products"]["outcome"], "counterexample")
        self.assertEqual(c["products"]["evaluator"], ["passed"])
        self.assertEqual(c["products"]["-O0"], ["assert", 1])
        small = Path(c["products"]["minimised"]).read_text()
        tail = small.split("// minimised by native_checks.py")[1]
        self.assertIn('#test "products"', tail)
        self.assertIn("scale(6, 7) == 42", tail)
        self.assertNotIn("scale(-3, 5)", tail, "cut after the first disagreeing assertion")
        self.assertNotIn('#test "runs after a trap"', tail)

    def test_a_seeded_evaluator_defect_is_a_counterexample(self):
        rec = nc.run([self.fixture(AGREE)], seed="evaluator", keep=self.dir / "keep")
        self.assertEqual(rec["outcome"], "counterexample")
        self.assertEqual(self.by_name(rec)["products"]["evaluator"], ["assert", 1])

    def test_nothing_unchecked_passes(self):
        # only ineligible checks
        rec = nc.run([self.fixture('f :: () -> int { return 1; }\n#test "r" { #assert #run f() == 1; }\n', "only.coax")])
        self.assertEqual(rec["outcome"], "not-checked")
        # a file the evaluator can't run
        rec = nc.run([self.fixture('#test "x" { #assert nope() == 1; }\n', "broken.coax")])
        self.assertEqual(rec["outcome"], "not-checked")
        self.assertTrue(all(c["outcome"] in ("not-checked", "rejected") for f in rec["files"] for c in f["checks"]))
        # a program with its own main isn't replaced
        rec = nc.run([self.fixture('main :: () {}\nf :: () -> int { return 1; }\n#test "m" { #assert f() == 1; }\n', "prog.coax")])
        self.assertEqual(rec["files"][0]["outcome"], "rejected")
        self.assertNotEqual(rec["outcome"], "passed")
        # no compiler
        saved = os.environ.get("COAX_TEST_COMPILER")
        os.environ["COAX_TEST_COMPILER"] = str(self.dir / "no-coax")
        try:
            rec = nc.run([self.fixture(AGREE, "nocoax.coax")])
        finally:
            if saved is None:
                os.environ.pop("COAX_TEST_COMPILER")
            else:
                os.environ["COAX_TEST_COMPILER"] = saved
        self.assertEqual(rec["outcome"], "infrastructure-error")
        # out of time
        saved_t = nc.TIMEOUT
        nc.TIMEOUT = 0.001
        try:
            rec = nc.run([self.fixture(AGREE, "slow.coax")])
        finally:
            nc.TIMEOUT = saved_t
        self.assertEqual(rec["outcome"], "inconclusive")

    def test_the_harness_never_reaches_product_emission(self):
        p = self.fixture(AGREE.replace('#test "uses #run" { #assert #run scale(2, 2) == 4; }\n', ""), "product.coax")
        nc.run([p], keep=self.dir / "keep")
        self.assertEqual(sorted(x.name for x in self.dir.iterdir() if x.name.startswith(".native-check")), [], "temporary sources removed")
        with_main = self.fixture(p.read_text() + "main :: () {}\n", "product-main.coax")
        c = subprocess.run([str(nc.compiler()), "c", with_main.name], cwd=self.dir, capture_output=True, text=True)
        self.assertEqual(c.returncode, 0, c.stderr)
        self.assertNotIn("__native_check", c.stdout)

    def test_the_selection_of_real_verse_and_psionic_checks_agrees(self):
        sel = json.loads((ROOT / "tests/native-checks.json").read_text())
        self.assertTrue(any(f.startswith("verse/") for f in sel["files"]) and any(f.startswith("psionic/") for f in sel["files"]))
        rec = nc.run([ROOT.parent / f for f in sel["files"]], keep=self.dir / "keep")
        bad = [(f["file"], c["name"], c["outcome"]) for f in rec["files"] for c in f["checks"]
               if c["outcome"] not in ("passed", "rejected")]
        self.assertEqual(bad, [])
        self.assertEqual(rec["counts"].get("passed", 0), sel["passed"], "the selection's eligible checks all ran")
        self.assertEqual(rec["counts"].get("rejected", 0), sel["rejected"])

    def test_the_record_names_unselected_checks(self):
        out = self.dir / "rec.json"
        sel = self.dir / "sel.json"
        sel.write_text(json.dumps({"files": ["verse/look.coax"], "roots": ["verse"]}))
        code = nc.main(["--selection", str(sel), "--out", str(out), "--keep", str(self.dir / "keep")])
        rec = json.loads(out.read_text())
        self.assertEqual(code, 0)
        files = {u["file"] for u in rec["unselected"]}
        self.assertIn("verse/stars.coax", files)
        self.assertNotIn("verse/look.coax", files)
        self.assertTrue(all(u["checks"] for u in rec["unselected"]))

    def test_kept_regressions_agree(self):
        for f in sorted((ROOT / "tests/native-checks").glob("*.coax")):
            rec = nc.run([f], keep=self.dir / "keep")
            self.assertEqual(rec["outcome"], "passed", f.name)


if __name__ == "__main__":
    unittest.main()
