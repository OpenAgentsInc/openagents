#!/usr/bin/env python3
"""A host constructs a library's state with the compiler's coax_default_T.

A `--library` unit defines, and its header declares, coax_default_T(T *p) for
each header type with a declared default. It writes those defaults in place
over any bytes, so a host's zeroed (or reused) state starts with the same bytes
that `s: [1] T` gives natively. This checks that natively and, when the wasm
toolchain (coax/web/toolchain.sh) and bun are present, in WebAssembly memory.
It also checks that two libraries sharing a defaulted type link together.
"""

import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
COMPILER = Path(os.environ.get("COAX_BIN", ROOT / "bin/coax"))
CC = os.environ.get("COAX_CC", "cc")

SHARED = """
Clock :: struct {
  step_ns: s64 = 15625000;
  left_ns: s64;
  max_steps: s64 = 8;
}
Mode :: enum u8 { idle; walking; }
"""

STATE = """#load "shared.coax";
Vec :: struct { x: f32; y: f32 = 1.5; z: f32; }
Body :: struct { at: Vec; mass: f64 = 2; alive: bool = true; }
Plain :: struct { a: s64; b: f32; }
State :: struct {
  gravity: f64 = 9.81;
  mode: Mode = Mode.walking;
  clock: Clock;
  bodies: [3] Body;
  plain: Plain;
  count: s32;
  name: u8 = 7;
}
state_gravity :: (s: [] State) -> f64 #export_c { return s[0].gravity; }
plain_sum :: (p: Plain) -> s64 #export_c { return p.a; }
"""

OTHER = """#load "shared.coax";
other_step :: (c: Clock) -> s64 #export_c { return c.step_ns; }
"""

# The bytes `v = coax_init_t_State()` gives natively, and coax_default_t_State
# over a buffer of 0xab bytes, from the same unit.
HARNESS = """#include "state.c"
#include <stdio.h>
#include <string.h>
static coax_t_State want, got;
int main(void) {
  want = coax_init_t_State();
  memset(&got, 0xab, sizeof got);
  coax_default_t_State(&got);
  fwrite(&want, sizeof want, 1, stdout);
  fwrite(&got, sizeof got, 1, stdout);
  return 0;
}
"""

FACE = """#include "state.h"
#include "other.h"
#include <string.h>
static coax_t_State s;
__attribute__((export_name("size"))) unsigned size(void) { return sizeof s; }
__attribute__((export_name("init"))) unsigned init(void) {
  memset(&s, 0xab, sizeof s);
  coax_default_t_State(&s);
  return (unsigned)(unsigned long)&s;
}
__attribute__((export_name("step"))) long long step(void) { return other_step(s.clock); }
"""

RUN = """const x = (await WebAssembly.instantiate(require("fs").readFileSync(process.argv[2]), {})).instance.exports
const at = x.init(), n = x.size()
process.stdout.write(Buffer.from(new Uint8Array(x.memory.buffer, at, n)).toString("hex") + " " + x.step())
"""


def wasm_toolchain():
    script = ROOT / "web/toolchain.sh"
    if not script.exists() or not shutil.which("bun"):
        return None
    out = subprocess.run(["bash", "-c", f'source "{script}" >/dev/null 2>&1 && echo "$COAXIAL_CLANG|$COAXIAL_SYSROOT|$COAXIAL_RESOURCES|$PATH"'],
                         capture_output=True, text=True, timeout=120)
    parts = out.stdout.strip().split("|")
    if out.returncode != 0 or len(parts) != 4 or not Path(parts[0]).exists():
        return None
    return parts


class StateDefaults(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="coax-state-defaults-")
        self.dir = Path(self.tmp.name)
        for name, text in (("shared.coax", SHARED), ("state.coax", STATE), ("other.coax", OTHER)):
            (self.dir / name).write_text(text)
        for name in ("state", "other"):
            result = subprocess.run([str(COMPILER), "c", name + ".coax", "--library", "--header", name + ".h"],
                                    cwd=self.dir, capture_output=True, text=True, timeout=60)
            self.assertEqual(result.returncode, 0, result.stderr)
            (self.dir / (name + ".c")).write_text(result.stdout)

    def tearDown(self):
        self.tmp.cleanup()

    def native_want(self):
        (self.dir / "harness.c").write_text(HARNESS)
        result = subprocess.run([CC, "-std=c11", "-O1", "-w", "harness.c", "-o", "harness", "-lm"],
                                cwd=self.dir, capture_output=True, text=True, timeout=60)
        self.assertEqual(result.returncode, 0, result.stderr)
        out = subprocess.run([str(self.dir / "harness")], capture_output=True, timeout=30).stdout
        half = len(out) // 2
        return out[:half], out[half:]

    def test_header_declares_defaulted_types_only(self):
        state_h = (self.dir / "state.h").read_text()
        for t in ("State", "Clock", "Body", "Vec", "a3_t_Body"):
            spelled = f"coax_{t}" if t.startswith("a3") else f"coax_t_{t}"
            name = t if t.startswith("a3") else "t_" + t
            self.assertIn(f"void coax_default_{name}({spelled} *p);", state_h)
        self.assertNotIn("coax_default_t_Plain", state_h)  # no declared default: zero is its value
        self.assertIn("void coax_default_t_Clock(coax_t_Clock *p);", (self.dir / "other.h").read_text())
        (self.dir / "program.coax").write_text('#load "state.coax";\nmain :: () { s: [1] State; }\n')
        program = subprocess.run([str(COMPILER), "c", "program.coax"], cwd=self.dir, capture_output=True, text=True, timeout=60)
        self.assertEqual(program.returncode, 0, program.stderr)
        self.assertNotIn("coax_default_", program.stdout)  # a program constructs its own state; only libraries export this

    def test_native_in_place_defaults_match_coax_init(self):
        want, got = self.native_want()
        self.assertGreater(len(want), 0)
        self.assertTrue(any(want), "the state's default must not be all zeros")
        self.assertEqual(want, got)

    def test_two_libraries_sharing_a_defaulted_type_link(self):
        (self.dir / "host.c").write_text('#include "state.h"\n#include "other.h"\n#include <stdio.h>\n'
                                         'int main(void) { coax_t_Clock c; coax_default_t_Clock(&c);\n'
                                         '  printf("%lld %lld\\n", (long long)other_step(c), (long long)c.max_steps); return 0; }\n')
        objects = []
        for name in ("state", "other"):
            result = subprocess.run([CC, "-std=c11", "-O2", "-c", name + ".c", "-o", name + ".o"],
                                    cwd=self.dir, capture_output=True, text=True, timeout=60)
            self.assertEqual(result.returncode, 0, result.stderr)
            objects.append(name + ".o")
        result = subprocess.run([CC, "-std=c11", "-Werror", "-pedantic-errors", "host.c", *objects, "-lm", "-o", "host"],
                                cwd=self.dir, capture_output=True, text=True, timeout=60)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(subprocess.run([str(self.dir / "host")], capture_output=True, text=True, timeout=30).stdout,
                         "15625000 8\n")

    def test_wasm_state_starts_with_native_bytes(self):
        toolchain = wasm_toolchain()
        if not toolchain:
            self.skipTest("no wasm toolchain (coax/web/toolchain.sh) or bun")
        clang, sysroot, resources, path = toolchain
        env = dict(os.environ, PATH=path)
        (self.dir / "face.c").write_text(FACE)
        flags = ["--target=wasm32-wasip1", f"--sysroot={sysroot}", f"-resource-dir={resources}", "-std=c11", "-O2",
                 "-ffp-contract=off", "-w"]
        for name in ("state", "other", "face"):
            result = subprocess.run([clang, *flags, "-c", name + ".c", "-o", name + ".o"], cwd=self.dir,
                                    capture_output=True, text=True, timeout=120, env=env)
            self.assertEqual(result.returncode, 0, result.stderr)
        result = subprocess.run([clang, *flags, "-mexec-model=reactor", "state.o", "other.o", "face.o", "-o", "face.wasm",
                                 "-Wl,--export-memory,--gc-sections"], cwd=self.dir, capture_output=True, text=True,
                                timeout=120, env=env)
        self.assertEqual(result.returncode, 0, result.stderr)
        (self.dir / "run.js").write_text(RUN)
        out = subprocess.run(["bun", "run.js", "face.wasm"], cwd=self.dir, capture_output=True, text=True, timeout=60)
        self.assertEqual(out.returncode, 0, out.stderr)
        state, step = out.stdout.split()
        want, _ = self.native_want()
        self.assertEqual(bytes.fromhex(state), want)
        self.assertEqual(step, "15625000")


if __name__ == "__main__":
    unittest.main()
