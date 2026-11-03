#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: a Python transliteration of Bitcoin Core's retargeting code decides each case outside Coax)
"""Retargeting against a transliteration of Bitcoin Core, natively and in
Wasm.

Bitcoin From Scratch, chapter 48. Compiles coax/lib/bitcoin/retarget.coax
(through the flat test entry point coax/tests/bitcoin/retarget_report.coax)
to C in checked and release builds and runs:

- every mainnet retarget from block 2,016 to block 973,728 in the pinned
  boundary headers, through CalculateNextWorkRequired, through
  GetNextWorkRequired on a whole period, and through
  PermittedDifficultyTransition, with one nBits more and less;
- Core's pow_tests.cpp cases;
- seeded periods for mainnet, testnet3, testnet4 and regtest: random
  times (including a last block earlier than the first), nBits near and
  far from powLimit, minimum-difficulty blocks anywhere in the period, the
  20-minute rule on both sides, views of the wrong length and negative
  heights;
- seeded CalculateNextWorkRequired cases with arbitrary nBits, including
  ones whose product wraps at 2^256;
- seeded PermittedDifficultyTransition cases at and off boundaries, at the
  edges of the permitted range;
- the parameters' sanity check on Core's networks and broken ones;
- multiplication and division by a word and chainwork sums.

Each report must equal the one built from retarget_reference.py, which
shares no code with Coax, and the output buffer past it must be untouched.
Then it builds the same C for wasm32 and runs every case again in Node
(pow_wasm.cjs, chapter 47's loader). From the repository root:

    python3 coax/tests/bitcoin/retarget.py
"""
import ctypes as C
import json
import os
from pathlib import Path
import random
import subprocess
import sys
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(ROOT / "coax/tests/nostr"))
import pow_reference as P  # noqa: E402
import retarget_reference as R  # noqa: E402
from wasm_toolchain import WASM_CLANG, WASM_SYSROOT, WASM_RESOURCES  # noqa: E402

COMPILER = Path(os.environ.get("COAX_TEST_COMPILER", ROOT / "coax/bin/coax"))
SOURCE = HERE / "retarget_report.coax"
PIN = ROOT / "coax/tests/vectors/upstream/mainnet/retarget-boundaries.txt"
CAP = 65536
M256 = (1 << 256) - 1
LIMIT = P.get_compact(P.POW_LIMIT_MAIN)


def le(v):
    return (v & (1 << 64) - 1).to_bytes(8, "little")


def u32(v):
    return (v & 0xFFFFFFFF).to_bytes(4, "little")


def num(v):
    return (v & M256).to_bytes(32, "little")


def params(p):
    flags = int(p.allow_min) | 2 * int(p.bip94) | 4 * int(p.no_retargeting)
    return num(p.pow_limit) + le(p.spacing) + le(p.timespan) + le(flags)


def case(op, data, report):
    return dict(op=op, input=data.hex(), index=0, report=report.hex())


def next_case(p, period, height, new_time, new_bits):
    data = params(p) + le(height) + le(new_time) + le(new_bits) + b"".join(u32(t) + u32(b) for t, b in period)
    code, bits = R.next_bits(p, period, height, new_time)
    return case(0, data, le(code) + le(bits) + le(R.check(p, period, height, new_time, new_bits)))


def calc_case(p, last, first):
    assert R.params_ok(p)
    want = R.calculate_next_work_required(R.Index(2015, last[0], last[1], R.Index(0, first[0], first[1])), first[0], p)
    return case(1, params(p) + u32(last[0]) + u32(last[1]) + u32(first[0]) + u32(first[1]), le(want))


def permitted_case(p, height, old, new):
    return case(2, params(p) + le(height) + u32(old) + u32(new), le(R.permitted_code(p, height, old, new)))


def params_case(p):
    interval = p.timespan // p.spacing if p.spacing > 0 else 0
    if p.spacing > 0 and p.timespan < 0:
        interval = -((-p.timespan) // p.spacing)    # C's division truncates toward zero
    return case(3, params(p), le(R.OK if R.params_ok(p) else R.PARAMS) + le(interval))


def arith_case(a, w, bits, count):
    q = R.divide(a, w) if w else 0
    out = num(R.mul32(a, w)) + le(int(w != 0)) + num(q) + num(a + R.mul32(P.proof(bits), count))
    return case(4, num(a) + u32(w) + u32(bits) + u32(count), out)


def mainnet():
    """(first time, last time, nBits) of each period, and nBits after the last."""
    h = {}
    for line in PIN.read_text().splitlines():
        height, _, hexed = line.split(" ")
        h[int(height)] = P.read_header(bytes.fromhex(hexed))
    top = max(x for x in h if x % 2016 == 0) // 2016
    periods = [(h[2016 * k]["time"], h[2016 * k + 2015]["time"], h[2016 * k]["bits"]) for k in range(top)]
    return periods, h[2016 * top]["bits"]


def random_bits(rng):
    pick = rng.random()
    if pick < 0.3:
        return LIMIT
    if pick < 0.6:
        return rng.choice([0x1c05a3f4, 0x17023774, 0x1d00d86a, 0x1b04864c, 0x1d00fffe])
    if pick < 0.8:
        return (rng.randrange(3, 30) << 24) | rng.randrange(1, 1 << 23)
    return rng.getrandbits(32)


def cases():
    rng = random.Random(48)
    out = []
    periods, after = mainnet()
    bits = [b for _, _, b in periods] + [after]
    for k in range(1, len(periods) + 1):
        first, last, b = periods[k - 1]
        out.append(calc_case(R.MAIN, (last, b), (first, b)))
        view = [(first, b)] + [(last, b)] * 2015
        out.append(next_case(R.MAIN, view, 2016 * k - 1, last + 600, bits[k] + (k % 3) - 1))
        for d in (-1, 0, 1):
            out.append(permitted_case(R.MAIN, 2016 * k, b, (bits[k] + d) & 0xFFFFFFFF))
    # Core's cases.
    for height, t, b, first, _ in [(32255, 1262152739, 0x1d00ffff, 1261130161, 0), (2015, 1233061996, 0x1d00ffff, 1231006505, 0),
                                   (68543, 1279297671, 0x1c05a3f4, 1279008237, 0), (68543, 1279297671, 0x1c05a3f5, 1279297671 - R.TWO_WEEKS // 4, 0),
                                   (68543, 1279008237, 0x1c05a3f4, 1279297671, 0), (46367, 1269211443, 0x1c387f6f, 1263163443, 0)]:
        out.append(calc_case(R.MAIN, (t, b), (first, b)))
    # Seeded periods on every network.
    nets = [R.MAIN, R.TESTNET3, R.TESTNET4, R.REGTEST]
    for _ in range(400):
        p = rng.choice(nets)
        interval = p.interval()
        pos = rng.choice([0, 1, interval - 2, interval - 1, rng.randrange(interval)])
        height = interval * rng.randrange(0, 50) + pos
        base = rng.randrange(1_000_000, 2_000_000_000)
        period, t = [], base
        for i in range(pos + 1):
            t = max(0, min(0xFFFFFFFF, t + rng.randrange(-3000, 4000)))
            period.append((t, LIMIT if rng.random() < 0.3 else rng.choice([random_bits(rng), 0x1c05a3f4])))
        if rng.random() < 0.2:
            period[0] = (period[-1][0] + rng.randrange(0, 5_000_000), period[0][1])
        last_time = period[-1][0]
        new_time = (last_time + rng.choice([1199, 1200, 1201, 1202, -5, 30000, 600])) & 0xFFFFFFFF
        new_bits = rng.choice([LIMIT, period[-1][1], random_bits(rng)])
        out.append(next_case(p, period, height, new_time, new_bits))
        if rng.random() < 0.1:
            out.append(next_case(p, period[:-1] if len(period) > 1 else period + period, height, new_time, new_bits))
    p = R.MAIN
    out.append(next_case(p, [(5, LIMIT)], -1, 0, LIMIT))
    out.append(next_case(p, [], 0, 0, LIMIT))
    # CalculateNextWorkRequired on arbitrary nBits and times.
    for _ in range(400):
        p = rng.choice(nets)
        last = (rng.getrandbits(32), random_bits(rng))
        first = (rng.choice([(last[0] - rng.randrange(-R.TWO_WEEKS, 6 * R.TWO_WEEKS)) & 0xFFFFFFFF, rng.getrandbits(32)]), random_bits(rng))
        out.append(calc_case(p, last, first))
    # PermittedDifficultyTransition near the edges.
    for _ in range(400):
        p = rng.choice(nets)
        old = random_bits(rng)
        height = rng.choice([p.interval() * rng.randrange(1, 500), rng.randrange(0, 10**6), -rng.randrange(1, 5)])
        t = P.set_compact(old)[0]
        guess = rng.choice([t * 4 // 1, t // 4, t, t * 2, P.POW_LIMIT_MAIN, rng.getrandbits(256)])
        new = (P.get_compact(guess & M256) + rng.choice([-1, 0, 0, 1])) & 0xFFFFFFFF
        out.append(permitted_case(p, height, old, rng.choice([new, old, random_bits(rng)])))
    # The sanity check.
    for p in nets + [R.Params(P.POW_LIMIT_MAIN, 0, R.TWO_WEEKS, False, False, False),
                     R.Params(P.POW_LIMIT_MAIN, -600, R.TWO_WEEKS, False, False, False),
                     R.Params(P.POW_LIMIT_MAIN, 600, 0, False, False, False),
                     R.Params(P.POW_LIMIT_MAIN, 600, -R.TWO_WEEKS, False, False, False),
                     R.Params(P.POW_LIMIT_MAIN, 600, R.TWO_WEEKS + 1, False, False, False),
                     R.Params(0, 600, R.TWO_WEEKS, False, False, False),
                     R.Params(P.POW_LIMIT_REGTEST, 600, 86400, True, False, False),
                     R.Params(P.POW_LIMIT_REGTEST, 1, 0x3FFFFFFF, False, False, True),
                     R.Params(P.POW_LIMIT_REGTEST, 1, 0x40000000, False, False, True),
                     R.Params(R.divide(M256, 4 * R.TWO_WEEKS), 600, R.TWO_WEEKS, False, False, False),
                     R.Params(R.divide(M256, 4 * R.TWO_WEEKS) - 1, 600, R.TWO_WEEKS, False, False, False)]:
        out.append(params_case(p))
    bad = R.Params(P.POW_LIMIT_MAIN, 600, R.TWO_WEEKS + 1, False, False, False)
    out.append(next_case(bad, [(1, LIMIT)], 0, 0, LIMIT))
    out.append(permitted_case(bad, 2016, LIMIT, LIMIT))
    # Words and chainwork.
    for _ in range(300):
        a = rng.getrandbits(rng.randrange(0, 257))
        w = rng.choice([0, 1, 2016, R.TWO_WEEKS, 0xFFFFFFFF, rng.getrandbits(32)])
        out.append(arith_case(a, w, random_bits(rng), rng.choice([0, 1, 2016, rng.getrandbits(32)])))
    out.append(arith_case(M256, 2, 0x2100ffff, 1))
    return out


class View(C.Structure):
    _fields_ = [("data", C.POINTER(C.c_uint8)), ("count", C.c_int64)]


def view(buffer, n):
    return View(C.cast(buffer, C.POINTER(C.c_uint8)), n)


def run(*args, **kw):
    r = subprocess.run([str(a) for a in args], capture_output=True, text=True, **kw)
    if r.returncode:
        raise RuntimeError(f"{args[0]} failed ({r.returncode}): {r.stderr or r.stdout}")
    return r


class Retarget(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix="coax-retarget-")
        cls.dir = Path(cls.temp.name)
        cls.libs = []
        for label, mode in [("checked", []), ("release", ["--release"])]:
            c = cls.dir / (label + ".c")
            c.write_text(run(COMPILER, "c", SOURCE, "--library", "--header", cls.dir / "retarget.h", *mode).stdout)
            run("cc", "-std=c11", "-O2", "-shared", "-fPIC", c, "-lm", "-o", cls.dir / (label + ".dylib"))
            lib = C.CDLL(str(cls.dir / (label + ".dylib")))
            lib.retarget_report_vartime.argtypes = [C.c_int64, View, View]
            lib.retarget_report_vartime.restype = C.c_int64
            cls.libs.append((label, lib))
        cls.cases = cases()
        cls.out = (C.c_uint8 * CAP)()

    @classmethod
    def tearDownClass(cls):
        cls.temp.cleanup()

    def native(self, lib, c):
        data = bytes.fromhex(c["input"])
        inp = (C.c_uint8 * max(1, len(data))).from_buffer_copy(data.ljust(1, b"\0"))
        C.memset(self.out, 0xA5, CAP)
        r = lib.retarget_report_vartime(c["op"], view(inp, len(data)), view(self.out, CAP))
        self.assertGreaterEqual(r, 8)
        rest = C.string_at(C.addressof(self.out) + r, CAP - r)
        self.assertEqual(rest, b"\xa5" * len(rest), "past the report")
        return C.string_at(self.out, r).hex()

    def test_reference(self):
        """The transliteration agrees with Core's published cases and the chain."""
        periods, after = mainnet()
        self.assertEqual(len(periods), 483)
        self.assertEqual(R.calculate_next_work_required(R.Index(32255, 1262152739, 0x1d00ffff), 1261130161, R.MAIN), 0x1d00d86a)
        rng = random.Random(2)
        for _ in range(500):
            a, w = rng.getrandbits(256), rng.getrandbits(32) or 1
            self.assertEqual(R.mul32(a, w), (a * w) & M256)
            self.assertEqual(R.divide(a, w), a // w)

    def test_native(self):
        codes = set()
        for c in self.cases:
            if c["op"] in (0, 2, 3):
                codes.add(int.from_bytes(bytes.fromhex(c["report"][:16]), "little"))
            if c["op"] == 0:
                codes.add(int.from_bytes(bytes.fromhex(c["report"][32:48]), "little"))
        self.assertEqual(codes, {R.OK, R.BAD_DIFFBITS, R.NOT_PERMITTED, R.PERIOD, R.PARAMS})
        for label, lib in self.libs:
            for c in self.cases:
                self.assertEqual(self.native(lib, c), c["report"], f"{label} op {c['op']} {c['input'][:120]}")
        print(f"\n{len(self.cases)} cases agree with Python, native checked and release", file=sys.stderr)

    def test_wasm(self):
        clang, sysroot, resources = Path(WASM_CLANG), Path(WASM_SYSROOT), Path(WASM_RESOURCES)
        self.assertTrue(clang.exists() and sysroot.exists() and resources.exists(), "a Wasm toolchain is required; see coax/web/wasm.sh")
        fixture = self.dir / "wasm.json"
        fixture.write_text(json.dumps(self.cases))
        for label in ["checked", "release"]:
            module = self.dir / (label + ".wasm")
            run(clang, "--target=wasm32-wasip1", "--sysroot=" + str(sysroot), "-resource-dir=" + str(resources),
                "-std=c11", "-O2", "-I" + str(self.dir), "-mexec-model=reactor",
                self.dir / (label + ".c"), HERE / "retarget_wasm.c",
                "-Wl,--import-memory,--export-memory,--export=__heap_base,--max-memory=67108864,-z,stack-size=1048576,--gc-sections",
                "-o", module)
            result = json.loads(run("node", HERE / "pow_wasm.cjs", module, fixture, CAP).stdout)
            self.assertEqual((result["agree"], result["cases"]), (len(self.cases), len(self.cases)), label)
        print(f"\n{len(self.cases)} cases agree with Python, native checked/release and Wasm checked/release",
              file=sys.stderr)


if __name__ == "__main__":
    unittest.main()
