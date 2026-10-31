#!/usr/bin/env python3
"""coax test/mutate --evidence FILE writes one coax.evidence.v1 record.

Each claim is bound to the source units, compiler, evaluator target, profile,
selection and budgets, and ends passed, counterexample, inconclusive,
infrastructure-error or not-checked. The verdict is the worst outcome
(EV_verdict in lib/compiler/mutation-schedule.coax): missing execution is
never a pass. The oracle here is written independently from the compiler:
expected outcomes per fixture and SHA-256 of the files on disk.
"""
import hashlib
import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
COAX = Path(os.environ.get("COAX_TEST_COMPILER", ROOT / "bin/coax")).resolve()

MIXED = '''f :: (x: int) -> int { return x + 1; }
#test "one" { #assert f(1) == 2; }
#forall "bad" x: 0..<4 { #assert f(x) < 3; }
#test "spin" { i := 0; while true { i += 1; } }
main :: () {}
'''
GOOD = '''f :: (x: int) -> int { return x + 1; }
#test "one" { #assert f(1) == 2; }
main :: () {}
'''
WEAK = '''f :: (x: int) -> int { return x * 2; }
#test "zero" { #assert f(0) == 0; }
main :: () {}
'''


class Evidence(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.base = Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()

    def run_with(self, text, *args, name="a"):
        src = self.base / f"{name}.coax"
        src.write_text(text)
        out = self.base / f"{name}-{len(list(self.base.iterdir()))}.json"
        p = subprocess.run([str(COAX), *args[:1], str(src), *args[1:], "--evidence", str(out)], capture_output=True, text=True)
        record = json.loads(out.read_text())
        self.assertEqual(record["schema"], "coax.evidence.v1")
        self.assertEqual(record["exit_status"], p.returncode)
        return record, src, p

    def outcomes(self, record):
        return {c["name"]: c["outcome"] for c in record["claims"]}

    def test_each_check_ends_in_its_own_outcome(self):
        r, src, p = self.run_with(MIXED, "test")
        self.assertEqual(p.returncode, 1)
        self.assertEqual(self.outcomes(r), {"one": "passed", "bad": "counterexample", "spin": "inconclusive"})
        bad = next(c for c in r["claims"] if c["name"] == "bad")
        self.assertEqual((bad["counterexample"], bad["at"]["line"]), ("x = 2", 3))
        self.assertEqual(next(c for c in r["claims"] if c["name"] == "spin")["code"], "E0400")
        self.assertEqual(r["verdict"], "counterexample")
        # The record binds what was run: sources, compiler, target and budgets.
        (unit,) = r["sources"]
        self.assertEqual(Path(unit["path"]).name, src.name)
        if "COAX_TEST_COMPILER" not in os.environ:  # the dev stage0 has no hashing
            self.assertEqual(unit["sha256"], hashlib.sha256(src.read_bytes()).hexdigest())
            self.assertEqual(r["compiler"]["sha256"], hashlib.sha256(COAX.read_bytes()).hexdigest())
        version = subprocess.run([str(COAX), "version"], capture_output=True, text=True).stdout.strip()
        self.assertEqual(r["compiler"]["version"], version)
        self.assertEqual(r["target"]["name"], "coax-evaluator")
        self.assertEqual(r["budgets"]["check_steps"], 20000000)
        r, _, _ = self.run_with(MIXED, "test", "--steps", "5000")
        self.assertEqual(r["budgets"]["check_steps"], 5000)

    def test_missing_execution_is_never_a_pass(self):
        r, _, p = self.run_with(MIXED, "test", "--only", "one")
        self.assertEqual(p.returncode, 0)
        self.assertEqual(self.outcomes(r), {"one": "passed", "bad": "not-checked", "spin": "not-checked"})
        self.assertEqual((r["verdict"], r["selection"]["only"]), ("not-checked", "one"))
        r, _, _ = self.run_with(MIXED, "test", "--list")
        self.assertEqual(r["verdict"], "not-checked")
        r, _, p = self.run_with("f :: (x: int) -> int { return x + ; }\n", "test")
        self.assertEqual((p.returncode, r["front_end"], r["claims"], r["verdict"]), (1, "failed", [], "not-checked"))
        r, _, _ = self.run_with(GOOD, "test")
        self.assertEqual(r["verdict"], "passed")

    def test_authority_coverage_is_a_claim(self):
        text = "#authority;\nf :: (x: int) -> int { return x + 1; }\ng :: () -> int { return 3; }\n#test \"f\" { if f(0) == 7 { y := g(); } #assert f(1) == 2; }\nmain :: () {}\n"
        r, _, _ = self.run_with(text, "test", "--lib")
        cover = next(c for c in r["claims"] if c["kind"] == "#authority")
        self.assertEqual((cover["outcome"], cover["code"]), ("counterexample", "E0380"))
        self.assertIn("g", cover["message"])
        r, _, _ = self.run_with(text, "test", "--lib", "--only", "f")
        self.assertEqual(next(c for c in r["claims"] if c["kind"] == "#authority")["outcome"], "not-checked")

    def test_mutation_outcomes(self):
        r, _, p = self.run_with(GOOD, "mutate")
        self.assertEqual((p.returncode, r["mutation"]["outcome"], r["verdict"]), (0, "passed", "passed"))
        self.assertEqual(r["mutation"]["killed"], r["mutation"]["mutants"])
        r, _, p = self.run_with(WEAK, "mutate")
        self.assertEqual((p.returncode, r["mutation"]["outcome"], r["verdict"]), (1, "counterexample", "counterexample"))
        self.assertTrue(r["mutation"]["survived"])
        self.assertTrue(all(s["line"] == 1 for s in r["mutation"]["survived"]))
        r, _, p = self.run_with(GOOD, "mutate", "--shard", "1/2")
        self.assertEqual((r["mutation"]["outcome"], r["mutation"]["shard"], r["verdict"]), ("inconclusive", [1, 2], "inconclusive"))
        r, _, p = self.run_with(MIXED, "mutate")
        self.assertEqual((p.returncode, r["mutation"]["outcome"], r["verdict"]), (1, "not-checked", "counterexample"))
        r, _, p = self.run_with(GOOD, "mutate", "--merge", str(self.base / "missing-record"))
        self.assertEqual((p.returncode, r["mutation"]["outcome"], r["verdict"]), (1, "infrastructure-error", "infrastructure-error"))

    def test_an_unwritable_record_fails_the_run(self):
        src = self.base / "g.coax"
        src.write_text(GOOD)
        p = subprocess.run([str(COAX), "test", str(src), "--evidence", str(self.base / "no/such/dir/r.json")], capture_output=True, text=True)
        self.assertEqual(p.returncode, 1)
        self.assertIn("cannot write evidence record", p.stderr)


if __name__ == "__main__":
    unittest.main()
