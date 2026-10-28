#!/usr/bin/env python3
"""Exercise the tools around the compiler: explain, JSON diagnostics,
layout reports, libraries and headers, and deterministic output."""

import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
COMPILER = ROOT / "bin/coax"
CC = os.environ.get("COAX_CC", "cc")


def coax(*args, cwd=None):
    return subprocess.run([str(COMPILER), *args], capture_output=True, text=True, timeout=30, cwd=cwd)


class Tools(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="coax-tools-")
        self.dir = Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()

    def write(self, name, text):
        path = self.dir / name
        path.write_text(text)
        return str(path)

    def test_every_error_code_is_explained(self):
        used = set()
        for source in (ROOT / "src").glob("*.cpp"):
            if source.name == "errors.cpp":
                continue
            used |= set(re.findall(r'"(E0\d{3})"', source.read_text()))
        listed = set(re.findall(r"^(E0\d{3}):", coax("explain", "--all").stdout, re.M))
        self.assertFalse(used - listed, f"codes without an explanation: {sorted(used - listed)}")
        result = coax("explain", "E0350")
        self.assertEqual(result.returncode, 0)
        self.assertIn("does not fit", result.stdout)
        self.assertEqual(coax("explain", "E9999").returncode, 1)

    def test_markdown_table_has_every_code(self):
        table = coax("explain", "--all", "--markdown").stdout
        self.assertEqual(len(re.findall(r"^\| `E0\d{3}` \|", table, re.M)),
                         len(re.findall(r"^E0\d{3}:", coax("explain", "--all").stdout, re.M)))

    def test_json_diagnostics(self):
        path = self.write("bad.coax", "main :: () {\n  x: u8 = 300;\n  y := missing;\n}\n")
        result = coax("check", "--json", path)
        self.assertEqual(result.returncode, 1)
        lines = [json.loads(line) for line in result.stderr.splitlines()]
        self.assertEqual([d["code"] for d in lines], ["E0350", "E0306"])
        self.assertEqual((lines[0]["line"], lines[0]["col"]), (2, 11))
        self.assertTrue(lines[0]["fix"])

    def test_layout_report_shows_padding(self):
        path = self.write("layout.coax", "Mixed :: struct { tag: u8; value: f64; flag: bool; }\nmain :: () {}\n")
        report = coax("layout", path).stdout
        self.assertIn("Mixed  size 24  align 8", report)
        self.assertIn("(padding)", report)
        self.assertIn("8     8  value: f64", report)

    def test_library_header_and_c_plus_plus(self):
        source = self.write("kernel.coax", """
Sample :: struct { value: f32; weight: s32 = 1; }
Mode :: enum u8 { off; on = 3; }
weigh :: (samples: [] Sample, mode: Mode) -> f64 #export_c {
  total := 0.0;
  if mode == Mode.off { return total; }
  for s: samples { total += cast(f64) s.value * cast(f64) s.weight; }
  return total;
}
""")
        self.assertIn("E0336", coax("c", source).stderr)  # a program needs main
        self.assertEqual(coax("check", source).returncode, 0)  # a file that exports does not
        header = self.dir / "kernel.h"
        obj = self.dir / "kernel.o"
        self.assertEqual(coax("header", source, "-o", str(header)).returncode, 0)
        self.assertEqual(coax("lib", source, "-o", str(obj)).returncode, 0)
        host = self.write("host.c", """
#include <stdio.h>
#include "kernel.h"
int main(void) {
  coax_t_Sample s[3] = {{1.5f, 2}, {2.0f, 1}, {0.25f, 4}};
  printf("%.2f %.2f\\n", weigh((coax_v_t_Sample){s, 3}, COAX_Mode_on), weigh((coax_v_t_Sample){s, 3}, COAX_Mode_off));
  return 0;
}
""")
        exe = self.dir / "host"
        build = subprocess.run([CC, "-std=c11", "-I", str(self.dir), "-o", str(exe), host, str(obj)],
                               capture_output=True, text=True)
        self.assertEqual(build.returncode, 0, build.stderr)
        self.assertEqual(subprocess.run([str(exe)], capture_output=True, text=True).stdout, "6.00 0.00\n")
        cxx = subprocess.run(["c++", "-std=c++17", "-fsyntax-only", "-x", "c++", "-I", str(self.dir), "-"],
                             input='#include "kernel.h"\nint main() { return 0; }\n', capture_output=True, text=True)
        self.assertEqual(cxx.returncode, 0, cxx.stderr)

    def test_output_is_deterministic(self):
        for example in ("crc32", "life", "mandelbrot"):
            path = str(ROOT / "examples" / f"{example}.coax")
            self.assertEqual(coax("c", path).stdout, coax("c", path).stdout)

    def test_untyped_constants_take_their_use_type(self):
        ok = self.write("ok.coax", "N :: 200; F :: 0.5;\nmain :: () { a: u8 = N; b: f32 = F; c: [N] u8; d := N; }\n")
        self.assertEqual(coax("check", ok).returncode, 0, coax("check", ok).stderr)
        bad = self.write("bad.coax", "N :: 300;\nmain :: () { a: u8 = N; }\n")
        self.assertIn("error[E0350]", coax("check", bad).stderr)


if __name__ == "__main__":
    unittest.main()
