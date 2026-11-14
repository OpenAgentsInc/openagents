#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: the host C compiler decides whether the emitted check binary compiles, and the process's stderr shows what coax said)
"""Native checks that negate negative constants compile, and a check binary
the C compiler rejects is said, not a silent move to the evaluator (#13570).

The C emitter once wrote `(-` before a negative literal operand, making
`(--0x1.99999ap-2f)`: C's decrement token. The check binary did not compile,
and every check of the unit ran in the slow evaluator without a word.
"""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

COAX = Path(__file__).resolve().parents[1]
COMPILER = Path(os.environ.get("COAX_TEST_COMPILER", COAX / "bin/coax")).resolve()
FAILED = "the C compiler failed on the check binary"

SOURCE = '''BACK :: -0.4f32;
FAR :: -2.5;
AHEAD :: -BACK;
ahead :: (x: f32) -> f32 { return x * -BACK; }
#test "a negated negative f32 constant" {
  #assert -BACK == 0.4f32;
  #assert AHEAD == 0.4f32;
  #assert ahead(10) == 4;
}
#test "a negated negative f64 constant and literal" {
  #assert -FAR == 2.5;
  #assert -(-2.5) == 2.5;
  #assert -(-(-2.5)) == FAR;
}
'''


class NativeCheckFallback(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.path = Path(self.tmp.name) / "negate.coax"
        self.path.write_text(SOURCE)

    def tearDown(self):
        self.tmp.cleanup()

    def coax_test(self, **env):
        return subprocess.run([str(COMPILER), "test", str(self.path), "--library"], capture_output=True, text=True,
                              timeout=120, env={**os.environ, "COAX_NATIVE_CHECKS": "1", "COAX_NATIVE_CHECKS_TRACE": "1",
                                                "TMPDIR": self.tmp.name, **env})

    def test_negated_negative_constants_pass_as_native_code(self):
        r = self.coax_test()
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertNotIn(FAILED, r.stderr)
        self.assertIn("2 of 2 eligible, 2 passed", r.stderr)

    def test_a_check_binary_the_c_compiler_rejects_is_said(self):
        false = shutil.which("false")
        self.assertIsNotNone(false)
        r = self.coax_test(COAX_CC=false)
        # The evaluator still runs every check, and they pass there.
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(r.stderr.count(FAILED), 2, r.stderr)  # the line, and the trace's reason
        self.assertIn("the evaluator runs every check (", r.stderr)
        self.assertIn("0 passed", r.stderr)


if __name__ == "__main__":
    unittest.main()
