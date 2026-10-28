#!/usr/bin/env python3
"""Focused checks for the optional valid-program differential tool."""

from dataclasses import asdict
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('coax_differential_tool', ROOT / 'tools/differential.py')
diff = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = diff
SPEC.loader.exec_module(diff)


class Differential(unittest.TestCase):
    def test_generation_is_deterministic_and_bounded(self):
        frozen = json.dumps(asdict(diff.generate(20261028, 0, 12)),
                            sort_keys=True, separators=(",", ":")).encode()
        self.assertEqual(hashlib.sha256(frozen).hexdigest(),
                         "470a92057c1d0f01e502dc712c352b0f89f6f19e463b9e2bb7cabc82ae4d6a4a")
        for index in range(24):
            program = diff.generate(20261028, index, 12)
            self.assertEqual(program, diff.generate(20261028, index, 12))
            self.assertEqual({step.kind for step in program.steps}, set(diff.KINDS))
            for inputs in program.inputs:
                observed = diff.oracle(program, inputs)
                self.assertEqual(len(observed), 12)
                self.assertEqual((observed[5], observed[10]), (701, -907))
                self.assertTrue(all(abs(value) < 1000000 for value in observed))
        self.assertNotEqual(diff.generate(1, 0, 12), diff.generate(2, 0, 12))

    def test_oracle_uses_truncating_signed_division(self):
        self.assertEqual(diff.quotient(-8, 3), -2)
        self.assertEqual(diff.quotient(8, -3), -2)
        self.assertEqual(diff.remainder(-8, 3), -2)
        self.assertEqual(diff.remainder(8, -3), 2)
        for operator, expected in (("+", -14), ("-", -20), ("*", -51), ("/", -5),
                                   ("%", -2), ("&", 3), ("|", -17), ("^", -20)):
            program = diff.Program(1, 0, (diff.Step("arith", op=operator, value=3),), ((-1, 0),))
            self.assertEqual(diff.oracle(program, (-1, 0))[0], expected)

    def test_retained_seeded_witness_has_a_source_bound_oracle(self):
        fixture = ROOT / 'tests/differential/seeded-multiply'
        program = diff.load_program(fixture / 'program.json')
        self.assertEqual(diff.render(program), (fixture / 'source.coax').read_text())
        result = json.loads((fixture / 'counterexample.json').read_text())
        self.assertEqual(result['source_sha256'], diff.digest(fixture / 'source.coax'))
        self.assertEqual(result['expected'], [diff.oracle(program, row) for row in program.inputs])
        self.assertEqual(result['seeded_defect'], diff.DEFECT)
        self.assertTrue(all(native['observed'] != result['expected'] for native in result['native']))

    def test_sample_result_is_bound_to_reproducible_sources(self):
        sample = json.loads((ROOT / 'tests/differential/sample-results.json').read_text())
        self.assertEqual(sample['outcome'], 'passed')
        self.assertEqual(len(sample['cases']), 24)
        for case in sample['cases']:
            program = diff.generate(sample['selection']['seed'], case['case_index'],
                                    sample['selection']['steps'])
            self.assertEqual(case['source_sha256'],
                             hashlib.sha256(diff.render(program).encode()).hexdigest())
            expected = [diff.oracle(program, row) for row in program.inputs]
            encoded = json.dumps(expected, separators=(',', ':')).encode()
            self.assertEqual(case['expected_sha256'], hashlib.sha256(encoded).hexdigest())
            self.assertEqual(case['inputs'], [list(row) for row in program.inputs])
            self.assertEqual(case['evaluator_outcome'], 'passed')
            self.assertTrue(all(native['outcome'] == 'passed' for native in case['native']))

    def test_replay_rejects_unsupported_or_undefined_recipes(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / 'program.json'
            for change in ('zero_divisor', 'wide_input', 'unknown_schema'):
                record = {'schema_version': 1, 'generator': diff.GENERATOR,
                          'program': asdict(diff.generate(1, 0, 12))}
                if change == 'zero_divisor':
                    record['program']['steps'][0].update(kind='arith', op='/', value=0)
                elif change == 'wide_input':
                    record['program']['inputs'] = [[13, 0]]
                else:
                    record['schema_version'] = 99
                path.write_text(json.dumps(record))
                with self.assertRaises(ValueError):
                    diff.load_program(path)

    def test_oracle_orders_indexes_and_reverse_cleanup(self):
        program = diff.Program(1, 0, (
            diff.Step("assign", index=0, other=0),
            diff.Step("defer", value=5, index=0),
            diff.Step("defer", value=7, index=1),
        ), ((1, 2),))
        # Initial total=23. LHS index is 1, RHS index is 2: out[1]=out[2]+23.
        # Deferred +7 goes to index 0, then deferred +5 also goes to index 0.
        self.assertEqual(diff.oracle(program, (1, 2)),
                         [23, 0, 0, 0, 0, 701, 13, 22, -1, -2, -907, 4])

    def test_reducer_preserves_the_predicate_and_reports_its_budget(self):
        program = diff.Program(1, 0, (
            diff.Step("branch", value=2), diff.Step("defer", value=3),
            diff.Step("arith", op="*", value=7),
        ), ((8, -3), (7, 2)))
        predicate = lambda p: any(step.kind == "arith" for step in p.steps)
        reduced, report = diff.minimize(program, predicate, 80)
        self.assertTrue(predicate(reduced))
        self.assertEqual(len(reduced.steps), 1)
        self.assertEqual(reduced.inputs, ((0, 0),))
        self.assertFalse(report["budget_exhausted"])
        _, bounded = diff.minimize(program, predicate, 1)
        self.assertTrue(bounded["budget_exhausted"])

    def test_timeout_and_missing_executable_are_not_passes(self):
        timed = diff.execute([sys.executable, "-c", "import time; time.sleep(2)"], 0.01)
        self.assertEqual(timed["outcome"], "inconclusive")
        missing = diff.execute(["/no-such-coax-differential-executable"], 1)
        self.assertEqual(missing["outcome"], "infrastructure_error")

    def test_missing_compiler_retains_an_explicit_infrastructure_result(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / 'missing'
            run = subprocess.run([sys.executable, str(ROOT / 'tools/differential.py'),
                                  '--coax', '/no-such-coax-differential-compiler', '--cases', '1',
                                  '--output', str(output)], capture_output=True, text=True, timeout=30)
            self.assertEqual(run.returncode, 1, run.stdout + run.stderr)
            result = json.loads((output / 'results.json').read_text())
            self.assertEqual(result['outcome'], 'infrastructure_error')
            self.assertTrue(all(native['outcome'] == 'not_checked'
                                for native in result['cases'][0]['native']))

    def test_real_native_profiles_and_seeded_defect_retention(self):
        compiler = Path(os.environ.get("COAX_DIFFERENTIAL_COMPILER", ROOT / "bin/coax"))
        self.assertTrue(compiler.is_file(), "supply an already-built compiler")
        with tempfile.TemporaryDirectory(prefix="coax-differential-test-") as temp:
            directory = Path(temp)
            command = [sys.executable, str(ROOT / "tools/differential.py"),
                       "--coax", str(compiler), "--seed", "20261028", "--cases", "2",
                       "--steps", "12", "--reduce-budget", "32"]
            normal = subprocess.run(command + ["--output", str(directory / "normal")],
                                    capture_output=True, text=True, timeout=120)
            self.assertEqual(normal.returncode, 0, normal.stdout + normal.stderr)
            result = json.loads((directory / "normal/results.json").read_text())
            self.assertEqual(result["schema_version"], 1)
            self.assertEqual(result["outcome"], "passed")
            self.assertEqual(result["semantic_profile"]["integer_arithmetic"], "checked")
            self.assertEqual(len(result["compiler"]["sha256"]), 64)
            for case in result["cases"]:
                self.assertEqual(case["evaluator"]["outcome"], "passed")
                self.assertEqual([native["optimization"] for native in case["native"]],
                                 ["-O0", "-O2"])
                self.assertTrue(all(native["outcome"] == "passed" for native in case["native"]))
            seeded = subprocess.run(command + ["--output", str(directory / "seeded"),
                                                "--seed-defect"],
                                    capture_output=True, text=True, timeout=120)
            self.assertEqual(seeded.returncode, 1, seeded.stdout + seeded.stderr)
            result = json.loads((directory / "seeded/results.json").read_text())
            self.assertEqual(result["outcome"], "counterexample")
            for case in result["cases"]:
                self.assertEqual(case["evaluator"]["outcome"], "passed")
                self.assertTrue(all(native['outcome'] == 'counterexample' for native in case['native']))
                reduced = directory / "seeded" / case["minimized"]["directory"]
                self.assertTrue((reduced / "source.coax").is_file())
                retained = json.loads((reduced / "program.json").read_text())
                self.assertLess(len(retained["program"]["steps"]), 12)
                self.assertEqual(len(retained["program"]["inputs"]), 1)
                observation = json.loads((reduced / "result.json").read_text())
                self.assertEqual(observation["outcome"], "counterexample")
                self.assertNotEqual(observation["expected"], observation["native"][0]["observed"])
                self.assertTrue((reduced / "emitted.c").is_file())
                self.assertTrue((reduced / "emitted-seeded.c").is_file())
            replay = subprocess.run([sys.executable, str(ROOT / 'tools/differential.py'),
                                     '--coax', str(compiler), '--replay',
                                     str(ROOT / 'tests/differential/seeded-multiply/program.json'),
                                     '--output', str(directory / 'replayed')],
                                    capture_output=True, text=True, timeout=60)
            self.assertEqual(replay.returncode, 0, replay.stdout + replay.stderr)
            result = json.loads((directory / 'replayed/results.json').read_text())
            self.assertEqual(result['bounds']['steps_per_case'], 0)
            self.assertEqual(result['bounds']['inputs_per_case'], 1)


if __name__ == "__main__":
    unittest.main()
