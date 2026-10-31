#!/usr/bin/env python3
"""Offline book recipes: authored checks and exact checked C11 sanitizer runs.

Select individual methods with unittest arguments; no model, network, downloads
or existing full-library gate runs are part of this focused runner.
"""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
COMPILER = Path(os.environ.get("COAX", ROOT / "coax/bin/coax")).resolve()
EXAMPLES = ROOT / "coax/book/examples"


class LibraryRecipesTests(unittest.TestCase):
    def checked(self, command, **options):
        result = subprocess.run([str(x) for x in command], cwd=ROOT,
                                capture_output=True, timeout=120, **options)
        self.assertEqual(result.returncode, 0,
                         result.stderr.decode(errors="replace") + result.stdout.decode(errors="replace")[:4000])
        return result

    def checks(self, recipe):
        self.checked([COMPILER, "test", EXAMPLES / (recipe + ".coax")])

    def native(self, recipe, optimization):
        source = EXAMPLES / (recipe + ".coax")
        expected = source.with_suffix(".out").read_bytes()
        emitted = self.checked([COMPILER, "c", source, "--arithmetic", "checked"])
        with tempfile.TemporaryDirectory(prefix="coax-library-recipe-") as directory:
            folder = Path(directory)
            c = folder / "recipe.c"
            c.write_bytes(emitted.stdout)
            executable = folder / "recipe"
            self.checked([os.environ.get("CC", "cc"), "-std=c11", optimization,
                          "-g", "-ffp-contract=off", "-fno-fast-math",
                          "-fsanitize=address,undefined", "-fno-sanitize-recover=all",
                          "-fno-omit-frame-pointer", c, "-lm", "-o", executable])
            result = self.checked([executable], env={
                **os.environ,
                "ASAN_OPTIONS": "detect_leaks=0:halt_on_error=1",
                "UBSAN_OPTIONS": "halt_on_error=1:print_stacktrace=1",
            })
            self.assertEqual(result.stderr, b"")
            self.assertEqual(result.stdout, expected)

    def test_protocol_checks(self):
        self.checks("ch28_protocol")

    def test_psionic_checks(self):
        self.checks("ch28_psionic")

    def test_protocol_checked_o0_native_sanitized(self):
        self.native("ch28_protocol", "-O0")

    def test_protocol_checked_o2_native_sanitized(self):
        self.native("ch28_protocol", "-O2")

    def test_psionic_checked_o0_native_sanitized(self):
        self.native("ch28_psionic", "-O0")

    def test_psionic_checked_o2_native_sanitized(self):
        self.native("ch28_psionic", "-O2")


if __name__ == "__main__":
    unittest.main(verbosity=2)
