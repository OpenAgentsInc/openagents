#!/usr/bin/env bash
# The room server's math (room-math.c, #12893): the same bits natively and in wasm, and near the
# true values. It builds a driver with room-math.c for this machine and for wasm32 (run under node's
# WASI), has both print sinf, cosf, tanf, expf, logf, powf and their double forms over 20000 inputs
# and the edge cases, compares the two outputs bit for bit, and checks each against Python's math
# (float forms within 1 ulp of the rounded value, double forms within 1e-13 relative and 1e-15; an
# angle past 1e5 within x * 4e-17, as it is first taken modulo 2 pi).
#   coax/web/room/room-math-check.sh      prints "same bits" and exits 0, or what differed and 1
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"
root="$(cd "$here/../../.." && pwd -P)"
source "$root/coax/web/toolchain.sh"
work="$(mktemp -d "${TMPDIR:-/tmp}/room-math-check.XXXXXX")"
trap 'rm -rf "$work"' EXIT
strict=(-std=c11 -O2 -ffp-contract=off -fno-fast-math)
for f in sin cos tan exp log pow; do strict+=("-fno-builtin-$f" "-fno-builtin-${f}f"); done
cat > "$work/driver.c" <<'EOF'
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
static uint32_t seed = 12893;
static double next(void) { seed = seed * 1664525u + 1013904223u; return (double)(seed >> 8) / 16777216.0; }
static void f(const char *name, float x, float y, float v) { uint32_t b; memcpy(&b, &v, 4); printf("%s %.17g %.17g %08x\n", name, x, y, b); }
static void d(const char *name, double x, double y, double v) { uint64_t b; memcpy(&b, &v, 8); printf("%s %.17g %.17g %016llx\n", name, x, y, (unsigned long long)b); }
static void all(double x, double y) {
  float xf = (float)x, yf = (float)y;
  f("sinf", xf, 0, sinf(xf)); f("cosf", xf, 0, cosf(xf)); f("tanf", xf, 0, tanf(xf));
  f("expf", xf, 0, expf(xf)); f("logf", yf, 0, logf(yf)); f("powf", yf, xf, powf(yf, xf));
  d("sin", x, 0, sin(x)); d("cos", x, 0, cos(x)); d("tan", x, 0, tan(x));
  d("exp", x, 0, exp(x)); d("log", y, 0, log(y)); d("pow", y, x, pow(y, x));
}
int main(void) {
  const double edge[] = {0, -0.0, 1, -1, 0.5, 1.5707963267948966, 3.141592653589793, 1e-30, 88, -103, 700, -740, 1e5, -1e5, 1.7e9, -2.5e9};
  for (int i = 0; i < (int)(sizeof edge / sizeof edge[0]); i++) all(edge[i], edge[i] < 0 ? -edge[i] : edge[i]);
  for (int i = 0; i < 20000; i++) { double s = next(); all((next() - 0.5) * (s < 0.5 ? 20 : s < 0.9 ? 400 : 4e9), next() * (s < 0.5 ? 3 : 1000)); }
  d("pow", -2, 3, pow(-2, 3)); d("pow", -2, 0.5, pow(-2, 0.5)); d("pow", 0, -1, pow(0, -1)); d("pow", -0.0, 3, pow(-0.0, 3));
  d("pow", INFINITY, -2, pow(INFINITY, -2)); d("pow", 0.5, INFINITY, pow(0.5, INFINITY)); d("log", 0, 0, log(0)); d("log", 5e-324, 0, log(5e-324));
  d("exp", -1000, 0, exp(-1000)); d("exp", 1000, 0, exp(1000)); d("sin", INFINITY, 0, sin(INFINITY));
  return 0;
}
EOF
/usr/bin/cc "${strict[@]}" -o "$work/native" "$work/driver.c" "$here/room-math.c"
"$COAXIAL_CLANG" --target=wasm32-wasip1 --sysroot="$COAXIAL_SYSROOT" -resource-dir="$COAXIAL_RESOURCES" "${strict[@]}" \
  -o "$work/driver.wasm" "$work/driver.c" "$here/room-math.c"
cat > "$work/run.mjs" <<'EOF'
import { WASI } from "node:wasi"; import fs from "node:fs";
const w = new WASI({ version: "preview1" });
w.start(await WebAssembly.instantiate(await WebAssembly.compile(fs.readFileSync(process.argv[2])), w.getImportObject()));
EOF
"$work/native" > "$work/native.txt"
node --no-warnings "$work/run.mjs" "$work/driver.wasm" > "$work/wasm.txt"
python3 - "$work/native.txt" "$work/wasm.txt" <<'EOF'
import math, struct, sys
a = open(sys.argv[1]).read().splitlines(); b = open(sys.argv[2]).read().splitlines()
bad = [f"native {x} | wasm {y}" for x, y in zip(a, b) if x != y]
if len(a) != len(b) or not a: bad.append(f"{len(a)} lines natively, {len(b)} in wasm")
ref = {"sin": math.sin, "cos": math.cos, "tan": math.tan, "exp": math.exp, "log": math.log, "pow": math.pow}
def f32(v): return struct.unpack("<f", struct.pack("<f", v))[0]
def ulps(u, v):
  i = struct.unpack("<i", struct.pack("<f", u))[0]; j = struct.unpack("<i", struct.pack("<f", v))[0]
  i = i if i >= 0 else -(i & 0x7fffffff); j = j if j >= 0 else -(j & 0x7fffffff)
  return abs(i - j)
far = 0
for line in a:
  name, x, y, bits = line.split()
  x, y = float(x), float(y)
  fl = name.endswith("f"); fn = ref[name[:-1] if fl else name]
  got = struct.unpack("<f", bytes.fromhex(bits)[::-1])[0] if fl else struct.unpack("<d", bytes.fromhex(bits)[::-1])[0]
  try: want = fn(x, y) if name.startswith("pow") else fn(x)
  except (ValueError, OverflowError): continue   # C's NaN and infinity cases: the bits check covers them
  if math.isnan(got) or math.isinf(got) or math.isinf(want): continue
  if name.startswith("tan"):   # by the angle it names: a step near the pole moves tan by 1 + tan^2
    if abs(math.atan(got) - math.atan(want)) > (abs(x) * 4e-17 if abs(x) > 1e5 else 0) + (1e-7 if fl else 1e-13): bad.append(f"{name}({x}) = {got!r}, want {want!r}")
    continue
  if fl:
    w = f32(want) if abs(want) < 3.4e38 else math.inf
    if math.isinf(w) or abs(want) < 1e-37: continue
    # past 1e5 an angle is taken modulo a double's 2 pi first: off by up to x * 4e-17
    if abs(x) > 1e5 and not name.startswith("pow") and abs(got - w) <= abs(x) * 4e-17 + 1e-7: continue
    if ulps(got, w) > 1: bad.append(f"{name}({x}, {y}) = {got!r}, want {w!r}")
  # (a power's e^(b ln a) carries b ln a's rounding: 2e-15 of it, relative)
  elif abs(got - want) > (1e-13 + 2e-15 * abs(math.log(abs(want))) if name == "pow" and want != 0 else 1e-13) * abs(want) + (abs(x) * 4e-17 if abs(x) > 1e5 and not name.startswith("pow") else 1e-15):
    bad.append(f"{name}({x}, {y}) = {got!r}, want {want!r}")
for m in bad[:12]: print(m)
print("same bits" if not bad else f"fails: {len(bad)}")
sys.exit(1 if bad else 0)
EOF
