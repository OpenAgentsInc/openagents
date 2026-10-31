#!/usr/bin/env python3
"""Borrowed numeric views across #foreign (spec 7.1, r3-foreign-borrowed-buffers).

A #foreign parameter may be a view of integers, floats or bool. It crosses
the C ABI as the same `{T *data; int64_t count;}` struct #export_c uses, by
value; the host reads and writes data[0..count) during the call and keeps
nothing after it returns. These fixtures run the real driver and C compilers:

- C11 and C++ hosts called from Coax with views, and calling back into an
  #export_c procedure that takes a view;
- zero-length and default (null, count 0) views; a short destination slice
  whose guard elements stay untouched; two overlapping views of one array;
- results, writes and host effects in Coax's left-to-right operand order;
- refusal of views of strings, views, fixed arrays and view results (E0376),
  and of executing a host effect at compile time or in a check (E0404);
- the ported entropy adapter (cxrand_draw) under ASan/UBSan: one call per
  draw and the bytes land in the caller's view, against the per-byte bridge.
"""
from pathlib import Path
import os
import platform
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
COAX = Path(os.environ.get("COAX_TEST_COMPILER", ROOT / "bin/coax")).resolve()
CC = os.environ.get("COAX_NATIVE_CC", "/usr/bin/clang" if platform.system() == "Darwin" else "cc")
CXX = os.environ.get("COAX_NATIVE_CXX", "/usr/bin/clang++" if platform.system() == "Darwin" else "c++")

PROGRAM = '''#import "Basic";
host_sum :: (xs: [] f32) -> f32 #foreign;
host_fill :: (out: [] u8, v: int) -> int #foreign;
host_move :: (dst: [] s16, src: [] s16) -> int #foreign;
host_any :: (k: [] bool) -> bool #foreign;
host_tag :: (t: int) -> int #foreign;
host_pair :: (a: int, xs: [] u8, b: int) -> int #foreign;
host_back :: (xs: [] s64) -> s64 #foreign;

// The host calls this one back with its own buffer.
sum_back :: (xs: [] s64) -> s64 #export_c {
  s: s64 = 0;
  for x: xs { s += x; }
  return s;
}

main :: () {
  f := [4] f32.[1.5, 2.5, -1.0, 4.0];
  print("sum %\\n", host_sum(f[..]));

  buf := [6] u8.[7, 0, 0, 0, 0, 9];
  n := host_fill(buf[1..<5], 200);
  print("fill % % % % % % %\\n", n, buf[0], buf[1], buf[2], buf[3], buf[4], buf[5]);
  print("empty %\\n", host_fill(buf[2..<2], 1));
  none: [] u8;
  print("none %\\n", host_fill(none, 1));
  print("after % % %\\n", buf[0], buf[2], buf[5]);

  a := [5] s16.[10, 20, 30, 40, 50];
  m := host_move(a[1..<5], a[0..<4]);
  print("move % % % % % %\\n", m, a[0], a[1], a[2], a[3], a[4]);

  k := [3] bool.[false, false, true];
  print("any % %\\n", host_any(k[..]), host_any(k[0..<2]));

  w := [2] u8.[0, 0];
  print("order %\\n", host_pair(host_tag(1), w[..], host_tag(2)));
  print("w % %\\n", w[0], w[1]);

  x := [3] s64.[5, 6, 7];
  print("back %\\n", host_back(x[..]));
}
'''

HOST = r'''
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "prog.h"

static int tags[8], ntags;

float host_sum(coax_v_f32 xs) {
  float s = 0;
  for (int64_t i = 0; i < xs.count; i++) s += xs.data[i];
  return s;
}
int64_t host_fill(coax_v_u8 out, int64_t v) {
  /* Writes exactly out.count elements; a null view has count 0. */
  for (int64_t i = 0; i < out.count; i++) out.data[i] = (uint8_t)(v + i);
  return out.count;
}
int64_t host_move(coax_v_s16 dst, coax_v_s16 src) {
  /* Views may overlap: copy as memmove does. */
  int64_t n = dst.count < src.count ? dst.count : src.count;
  memmove(dst.data, src.data, (size_t)n * sizeof *dst.data);
  return n;
}
BOOL host_any(coax_v_bool k) {
  for (int64_t i = 0; i < k.count; i++) if (k.data[i]) return 1;
  return 0;
}
int64_t host_tag(int64_t t) { tags[ntags++] = (int)t; return t * 10; }
int64_t host_pair(int64_t a, coax_v_u8 xs, int64_t b) {
  /* Both operands ran first, in order, and their effects are visible. */
  xs.data[0] = (uint8_t)tags[0];
  xs.data[1] = (uint8_t)tags[1];
  return a + b + ntags * 1000;
}
int64_t host_back(coax_v_s64 xs) {
  int64_t mine[2] = {100, 200};
  coax_v_s64 v = {mine, 2};
  return sum_back(xs) * 1000 + sum_back(v);
}
'''

EXPECTED = [
    "sum 7.0",
    "fill 4 7 200 201 202 203 9",
    "empty 0",
    "none 0",
    "after 7 201 9",
    "move 4 10 10 20 30 40",
    "any true false",
    "order 2030",
    "w 1 2",
    "back 18300",
]

REFUSED = [
    ("h :: (s: [] string) -> int #foreign;", "parameter 's' is [] string; #foreign procedures pass integers, floats, bool and views of them"),
    ("h :: (v: [] [] u8) -> int #foreign;", "parameter 'v' is [] [] u8; #foreign procedures pass integers, floats, bool and views of them"),
    ("h :: (a: [4] u8) -> int #foreign;", "parameter 'a' is [4] u8; #foreign procedures pass integers, floats, bool and views of them"),
    ("P :: struct { x: int; }\nh :: (p: [] P) -> int #foreign;", "parameter 'p' is [] P; #foreign procedures pass integers, floats, bool and views of them"),
    ("h :: (n: int) -> [] u8 #foreign;", "'h' returns [] u8; #foreign procedures return integers, floats, bool or nothing"),
]


def run(cmd, cwd=None, env=None):
    return subprocess.run([str(c) for c in cmd], cwd=cwd, capture_output=True, text=True, timeout=300, env=env)


class ForeignBuffers(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.dir = Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()

    def build(self, host_lang: str, opt: str) -> list:
        (self.dir / "prog.coax").write_text(PROGRAM)
        e = run([COAX, "c", "prog.coax", "--header", "prog.h"], cwd=self.dir)
        self.assertEqual(e.returncode, 0, e.stderr)
        (self.dir / "prog.c").write_text(e.stdout)
        header = (self.dir / "prog.h").read_text()
        self.assertIn("host_fill(coax_v_u8", header, "the header declares the host's view parameters")
        self.assertIn("typedef struct { uint8_t *data; int64_t count; } coax_v_u8;", e.stdout + header)
        bool_t = "bool" if host_lang == "c++" else "_Bool"
        host = HOST.replace("BOOL", bool_t)
        src = self.dir / ("host.cpp" if host_lang == "c++" else "host.c")
        src.write_text(('extern "C" {\n' if host_lang == "c++" else "") + host + ("}\n" if host_lang == "c++" else ""))
        objs = []
        c = run([CC, "-std=c11", opt, "-Wall", "-c", "prog.c", "-o", "prog.o"], cwd=self.dir)
        self.assertEqual(c.returncode, 0, c.stderr)
        compiler = [CXX, "-std=c++17"] if host_lang == "c++" else [CC, "-std=c11"]
        h = run([*compiler, opt, "-Wall", "-Werror", "-c", src.name, "-o", "host.o"], cwd=self.dir)
        self.assertEqual(h.returncode, 0, h.stderr)
        link = run([CXX if host_lang == "c++" else CC, "prog.o", "host.o", "-lm", "-o", "prog"], cwd=self.dir)
        self.assertEqual(link.returncode, 0, link.stderr)
        r = run([self.dir / "prog"])
        self.assertEqual(r.returncode, 0, r.stderr)
        return r.stdout.splitlines()

    def test_c11_host_both_directions(self):
        for opt in ("-O0", "-O2"):
            self.assertEqual(self.build("c", opt), EXPECTED, opt)

    def test_cxx_host_both_directions(self):
        if not shutil.which(CXX):
            self.skipTest("no C++ compiler")
        self.assertEqual(self.build("c++", "-O2"), EXPECTED)

    def test_refused_representations(self):
        for source, message in REFUSED:
            p = self.dir / "bad.coax"
            p.write_text(source + "\nmain :: () {}\n")
            r = run([COAX, "check", "bad.coax"], cwd=self.dir)
            self.assertNotEqual(r.returncode, 0, source)
            self.assertIn("error[E0376]: " + message, r.stdout + r.stderr, source)

    def test_host_effects_never_run_at_compile_time_or_in_checks(self):
        (self.dir / "ct.coax").write_text(
            "h :: (b: [] u8) -> int #foreign;\n"
            "N :: #run h(u8_bytes());\n"
            "u8_bytes :: () -> [] u8 { x: [] u8; return x; }\n"
            "main :: () {}\n")
        r = run([COAX, "check", "ct.coax"], cwd=self.dir)
        self.assertIn("error[E0404]: compile-time execution cannot call the host procedure 'h'", r.stdout + r.stderr)
        (self.dir / "t.coax").write_text(
            "h :: (b: [] u8) -> int #foreign;\n"
            "use :: (b: [] u8) -> int { return h(b); }\n"
            '#test "a check cannot reach the host" { b := [2] u8.[1, 2]; #assert use(b[..]) == 0; }\n'
            "main :: () {}\n")
        r = run([COAX, "test", "t.coax"], cwd=self.dir)
        self.assertNotEqual(r.returncode, 0)
        self.assertIn("error[E0404]: compile-time execution cannot call the host procedure 'h'", r.stdout + r.stderr)

    def test_entropy_adapter_under_sanitizers(self):
        """cxrand_draw writes the caller's view in one call (the per-byte bridge
        took n + 1 calls and copied through a static buffer)."""
        plat = ROOT / "platform"
        e = run([COAX, "c", ROOT / "lib/crypto/entropy.coax", "--library", "--header", self.dir / "entropy-policy.h"])
        self.assertEqual(e.returncode, 0, e.stderr)
        (self.dir / "entropy.c").write_text(e.stdout)
        prog = '''#import "Basic";
cxrand_draw :: (out: [] u8, n: int) -> int #foreign;
cxrand_fill :: (n: int) -> int #foreign;
cxrand_byte :: (i: int) -> int #foreign;
main :: () {
  a: [34] u8;
  a[0] = 77; a[33] = 88;
  r := cxrand_draw(a[1..<33], 32);
  nonzero := 0;
  for i: 1..<33 { if a[i] != 0 { nonzero += 1; } }
  print("draw % % % %\\n", r, a[0], a[33], nonzero > 16);
  print("short %\\n", cxrand_draw(a[1..<5], 8));
  print("empty %\\n", cxrand_draw(a[1..<1], 0));
  print("count %\\n", cxrand_draw(a[..], -1));
  big: [80] u8;
  print("scratch %\\n", cxrand_draw(big[..], 80));
  // the per-byte bridge for the same 32 bytes: 33 calls
  calls := 1; ok := cxrand_fill(32) == 0;
  for i: 0..<32 { b := cxrand_byte(i); calls += 1; ok = ok && b >= 0 && b <= 255; }
  print("bridge % %\\n", ok, calls);
}
'''
        (self.dir / "draw.coax").write_text(prog)
        c = run([COAX, "c", "draw.coax"], cwd=self.dir)
        self.assertEqual(c.returncode, 0, c.stderr)
        (self.dir / "draw.c").write_text(c.stdout)
        san = ["-fsanitize=address,undefined", "-fno-sanitize-recover=undefined", "-g"]
        b = run([CC, "-std=c11", "-O1", *san, f"-I{self.dir}", f"-I{plat}", "draw.c", "entropy.c",
                 plat / "entropy-native.c", plat / "entropy-foreign.c", "-lm",
                 *(["-framework", "Security"] if platform.system() == "Darwin" else []), "-o", "draw"], cwd=self.dir)
        self.assertEqual(b.returncode, 0, b.stderr)
        env = dict(os.environ, ASAN_OPTIONS="detect_leaks=0:abort_on_error=1")
        r = run([self.dir / "draw"], env=env)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(r.stdout.splitlines(), [
            "draw 0 77 88 true",   # one call, the guards kept
            "short 2",             # ENTROPY_CAPACITY: n > out.count, nothing written
            "empty 0",
            "count 1",             # ENTROPY_COUNT
            "scratch 2",           # n > CXRAND_MAX scratch: ENTROPY_CAPACITY
            "bridge true 33",
        ])


if __name__ == "__main__":
    unittest.main()
