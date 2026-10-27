#!/usr/bin/env python3
"""Exercise compile-time isolation, bounds, diagnostics and target artifacts."""

import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
COMPILER = ROOT / "bin/coax"


class CompileTimeTests(unittest.TestCase):
    def invoke(self, source, *args, env=None):
        with tempfile.TemporaryDirectory(prefix="coax-check-") as directory:
            path = Path(directory) / "test.coax"
            path.write_text(source)
            return subprocess.run(
                [str(COMPILER), *args, str(path)], capture_output=True,
                text=True, timeout=10, env=env,
            )

    def reject(self, source, code):
        # Compile-time mistakes remain errors in optimized builds; none reach C.
        for mode in ((), ("--release",)):
            with self.subTest(code=code, mode=mode):
                result = self.invoke(source, "c", *mode)
                self.assertEqual(result.returncode, 1, result.stderr)
                self.assertIn(f"error[{code}]", result.stderr)
                self.assertIn("test.coax:", result.stderr)
                self.assertEqual(result.stdout, "")

    def test_constant_dependencies_and_names(self):
        cases = [
            ("A :: B; B :: A; main :: () {}", "E0340"),
            ("A :: 1; A :: 2; main :: () {}", "E0345"),
            ("A :: 1; A :: () {} main :: () {}", "E0345"),
            ("A :: 1; main :: () { A = 2; }", "E0344"),
            ("A :: 1; main :: () { A := 2; }", "E0342"),
            ("f :: (n: int) { n += 1; } main :: () {}", "E0344"),
            ("A :: 1; main :: () { x := A(); }", "E0314"),
            ("type_name :: () {} main :: () {}", "E0335"),
        ]
        for source, code in cases:
            self.reject(source, code)

    def test_no_runtime_capture_even_when_called_at_compile_time(self):
        self.reject("main :: () { x := 2; y := #run x; }", "E0401")
        self.reject("A :: #run f(2); f :: (n: int) -> int { return #run n; } main :: () {}", "E0401")

    def test_no_host_output(self):
        self.reject('#import "Basic"; A :: #run f(); f :: () -> int { print("no"); return 1; } main :: () {}', "E0404")

    def test_explicit_execution_and_assertions(self):
        self.reject("A :: f(); f :: () -> int { return 1; } main :: () {}", "E0408")
        self.reject("#assert false; main :: () {}", "E0406")
        self.reject("#assert 1; main :: () {}", "E0346")
        self.reject("A :: #run f(); f :: () {} main :: () {}", "E0341")

    def test_overflow_and_division(self):
        for expr in ("9223372036854775807 + 1", "1 / 0", "1 % 0",
                     "(-9223372036854775807 - 1) / -1", "-(-9223372036854775807 - 1)"):
            self.reject(f"A :: {expr}; main :: () {{}}", "E0407")

    def test_uninitialized_reads(self):
        for operation in ("return x;", "x += 1; return x;"):
            self.reject(f"A :: #run f(); f :: () -> int {{ x: int = ---; {operation} }} main :: () {{}}", "E0405")

    def test_bounds_and_call_trace(self):
        self.reject("A :: #run f(); f :: () -> int { while true {} return 1; } main :: () {}", "E0400")
        source = "A :: #run f(); f :: () -> int { return f(); } main :: () {}"
        self.reject(source, "E0403")
        result = self.invoke(source, "check")
        self.assertIn("note: compile-time call at", result.stderr)
        self.assertLess(len(result.stderr), 20000)

    def test_cycles_through_function_bodies(self):
        self.reject("A :: #run f(); f :: () -> int { return A; } main :: () {}", "E0402")

    def test_budget_is_shared_and_constants_are_memoized(self):
        work = "work :: (n: int) -> int { i := 0; while i < n { i += 1; } return i; }"
        source = work + " A :: #run work(10000); B :: A + A; #assert B == 20000; main :: () {}"
        result = self.invoke(source, "check")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.reject(work + " A :: #run work(10000); B :: #run work(10000); main :: () {}", "E0400")

    def test_runtime_branch_does_not_hide_compile_time_failure(self):
        self.reject("main :: () { if false { x := #run (1 / 0); } }", "E0407")

    def test_check_does_not_invoke_c_or_run_main(self):
        source = '#import "Basic"; N :: #run f(); f :: () -> int { return 7; } #assert N == 7; main :: () { print("runtime only"); }'
        result = self.invoke(source, "check", env={**os.environ, "COAX_CC": "/missing/compiler"})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "")

    def test_generated_constants_are_literals(self):
        result = self.invoke("N :: #run square(16); square :: (n: int) -> int { return n * n; } main :: () { n := N; }", "c")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("l_n = INT64_C(256);", result.stdout)
        self.assertNotIn("#run", result.stdout)
        self.assertNotIn("Evaluator", result.stdout)

    def test_type_name_is_unevaluated_but_typechecked(self):
        source = '#assert type_name(1 / 0) == "int"; main :: () {}'
        result = self.invoke(source, "check")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.reject("main :: () { x := type_name(unknown); }", "E0306")
        self.reject("main :: () { x := type_name(); }", "E0343")

    def test_global_type_depth_is_bounded(self):
        source = "\n".join(f"C{i} :: C{i + 1};" for i in range(100)) + "\nC100 :: 1; main :: () {}"
        self.reject(source, "E0347")


if __name__ == "__main__":
    unittest.main()
