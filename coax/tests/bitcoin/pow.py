#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: a Python transliteration of Bitcoin Core's proof-of-work code decides each case outside Coax)
"""Headers and proof of work against a transliteration of Bitcoin Core,
natively and in Wasm.

Bitcoin From Scratch, chapter 47. Compiles coax/lib/bitcoin/pow.coax
(through the flat test entry point coax/tests/bitcoin/pow_report.coax) to C
in checked and release builds and runs:

- nBits: arith_uint256_tests.cpp's and pow_tests.cpp's cases, every size
  byte 0 to 255 with seeded mantissas and both signs, the edges of each
  overflow rule, and seeded random words. For each: the target, both flags,
  GetCompact with and without the sign, DeriveTarget's verdict under the
  mainnet and regtest limits, and GetBitsProof's work;
- GetCompact of seeded numbers of every bit length, with and without the
  sign;
- arithmetic: seeded pairs of every bit length, and the edges (zero, one,
  2^256 - 1, equal numbers), through addition, subtraction, complement,
  division, comparison, bits and shifts of 0 to 300 bits;
- the check: hashes at, just below and just above each target, and
  seeded hashes against seeded nBits;
- headers: the fifteen pinned mainnet headers, each with every one of its
  640 bits flipped in turn for the genesis block and 64 seeded flips for
  the others, short and long byte strings, and seeded random headers.

Each report must equal the one built from pow_reference.py, which shares no
code with Coax, and the output buffer past it must be untouched. Then it
builds the same C for wasm32 and runs every case again in Node. From the
repository root:

    python3 coax/tests/bitcoin/pow.py
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
from wasm_toolchain import WASM_CLANG, WASM_SYSROOT, WASM_RESOURCES  # noqa: E402

COMPILER = Path(os.environ.get("COAX_TEST_COMPILER", ROOT / "coax/bin/coax"))
SOURCE = HERE / "pow_report.coax"
UPSTREAM = ROOT / "coax/tests/vectors/upstream"
CAP = 4096
FILL = b"\xa5" * 32
MAIN = P.POW_LIMIT_MAIN.to_bytes(32, "little")
REGTEST = P.POW_LIMIT_REGTEST.to_bytes(32, "little")
M256 = (1 << 256) - 1


def le(v):
    return (v & (1 << 64) - 1).to_bytes(8, "little")


def num(v):
    return (v & M256).to_bytes(32, "little")


def compact_report(bits, limit):
    t, negative, overflow = P.set_compact(bits)
    code, _ = P.derive_target(bits, int.from_bytes(limit, "little"))
    return (num(t) + le(int(negative)) + le(int(overflow)) + le(P.get_compact(t)) + le(P.get_compact(t, negative))
            + le(code) + num(P.proof(bits)))


def header_report(limit, raw):
    h = P.read_header(raw)
    out = le(P.OK if h else P.CAPACITY)
    if h:
        out += le(h["version"]) + le(h["time"]) + le(h["bits"]) + le(h["nonce"]) + h["prev"] + h["merkle_root"]
        out += P.write_header(h)
        out += le(P.OK) + P.sha256d(raw)
    else:
        out += le(P.CAPACITY) + FILL
    out += le(P.check_header(raw, int.from_bytes(limit, "little")))
    return out + num(P.proof(h["bits"]) if h else 0)


def arith_report(a, c, k):
    pa, pc = P.limbs(a), P.limbs(c)
    q = P.div(pa, pc)
    out = num(P.value(P.add(pa, pc))) + num(P.value(P.sub(pa, pc))) + num(a ^ M256)
    out += le(int(q is not None)) + num(P.value(q) if q is not None else 0)
    out += le(P.compare(pa, pc)) + le(P.bits(pa))
    return out + num(P.value(P.shl(pa, k)) if k < 256 else 0) + num(P.value(P.shr(pa, k)) if k < 256 else 0)


def compact_case(bits, limit=MAIN):
    return dict(op=0, input=limit.hex(), index=bits, report=compact_report(bits, limit).hex())


def header_case(raw, limit=MAIN):
    return dict(op=1, input=(limit + raw).hex(), index=0, report=header_report(limit, raw).hex())


def encode_case(v, negative):
    return dict(op=2, input=num(v).hex(), index=int(negative), report=le(P.get_compact(v, negative)).hex())


def arith_case(a, c, k):
    return dict(op=3, input=(num(a) + num(c)).hex(), index=k, report=arith_report(a, c, k).hex())


def check_case(bits, h, limit=MAIN):
    return dict(op=4, input=(limit + h).hex(), index=bits,
                report=le(P.CAPACITY if len(h) != 32 else P.check_pow(h, bits, int.from_bytes(limit, "little"))).hex())


def pinned_headers():
    """The fifteen pinned headers, genesis first."""
    out = [bytes.fromhex(p.read_text()) for p in sorted((UPSTREAM / "mainnet").glob("*.header"))]
    for name in ("000000000003ba27aa200b1cecaad478d2b00432346c3f1f3986da1afd33e506",
                 "00000000000000000008fda0e5bb3105dab72605332085684c42dceb0ea61097"):
        out.append((UPSTREAM / f"mainnet/{name}.raw").read_bytes()[:80])
    genesis = [h for h in out if P.sha256d(h)[::-1].hex().startswith("000000000019d6")]
    return genesis + [h for h in out if h not in genesis]


def random_number(rng):
    n = rng.randrange(0, 257)
    return rng.getrandbits(n) if n else 0


def cases():
    rng = random.Random(47)
    out = []
    # nBits: Core's cases, every size byte, the overflow edges, random words.
    core = [0, 0x00123456, 0x01003456, 0x02000056, 0x03000000, 0x04000000, 0x00923456, 0x01803456, 0x02800056,
            0x03800000, 0x04800000, 0x01123456, 0x01fedcba, 0x02123456, 0x03123456, 0x04123456, 0x04923456,
            0x05009234, 0x20123456, 0xff123456, 0x80010203, 0x21012345, 0x1d00ffff, 0x1d00d86a, 0x1b04864c,
            0x17514a49, 0x17023774, 0xff7fffff, 0x207fffff, 0x1d80ffff, 0x1e00ffff, 0x1d01ffff]
    for bits in core:
        out += [compact_case(bits), compact_case(bits, REGTEST)]
    for size in range(256):
        for word in (0, 1, 0xff, 0x100, 0xffff, 0x10000, 0x7fffff, rng.randrange(1 << 23)):
            for sign in (0, 0x00800000):
                out.append(compact_case((size << 24) | sign | word, MAIN if rng.random() < 0.7 else REGTEST))
    for _ in range(300):
        out.append(compact_case(rng.getrandbits(32), MAIN if rng.random() < 0.7 else REGTEST))
    # GetCompact of numbers of every length.
    for n in range(0, 257):
        v = (1 << n) - 1 if n % 3 == 0 else (rng.getrandbits(n) | (1 << (n - 1)) if n else 0)
        out += [encode_case(v, False), encode_case(v, True)]
    for v in (0x80, 0x7f, 0x8000, 0x800000, 0x7fffff, P.POW_LIMIT_MAIN, P.POW_LIMIT_REGTEST, M256):
        out += [encode_case(v, False), encode_case(v, True)]
    # Arithmetic.
    edges = [0, 1, 2, 0xffffffff, 1 << 32, 1 << 255, M256, P.POW_LIMIT_MAIN, M256 - 1]
    for a in edges:
        for c in edges:
            out.append(arith_case(a, c, rng.randrange(0, 300)))
    for _ in range(300):
        a, c = random_number(rng), random_number(rng)
        if rng.random() < 0.1:
            c = a
        out.append(arith_case(a, c, rng.randrange(0, 300)))
    for k in range(0, 300, 7):
        out.append(arith_case(M256 if k % 2 else 1 << rng.randrange(256), 1, k))
    # The check at and around each target.
    for bits in [0x1d00ffff, 0x1d00d86a, 0x1b04864c, 0x17514a49, 0x17023774, 0x207fffff, 0x03123456]:
        t, _, _ = P.set_compact(bits)
        for h in (t, t - 1, t + 1, 0, M256):
            out += [check_case(bits, num(h)), check_case(bits, num(h), REGTEST)]
    for _ in range(200):
        bits = rng.choice([rng.getrandbits(32), (rng.randrange(1, 34) << 24) | rng.randrange(1 << 23)])
        out.append(check_case(bits, num(random_number(rng) >> rng.randrange(0, 64)), MAIN if rng.random() < 0.6 else REGTEST))
    out.append(check_case(0x1d00ffff, bytes(31)))
    out.append(check_case(0x1d00ffff, bytes(33)))
    # Headers: the pinned ones, flipped bits, wrong lengths and random ones.
    headers = pinned_headers()
    for i, raw in enumerate(headers):
        out += [header_case(raw), header_case(raw, REGTEST)]
        for bit in (range(640) if i == 0 else rng.sample(range(640), 64)):
            flipped = bytearray(raw)
            flipped[bit // 8] ^= 1 << (bit % 8)
            out.append(header_case(bytes(flipped)))
    for n in (0, 1, 79, 81, 160):
        out.append(header_case(rng.randbytes(n)))
    for _ in range(60):
        raw = bytearray(rng.randbytes(80))
        if rng.random() < 0.5:
            raw[72:76] = (0x207fffff).to_bytes(4, "little")
        out.append(header_case(bytes(raw), REGTEST if rng.random() < 0.5 else MAIN))
    return out


# ---- the native builds
class View(C.Structure):
    _fields_ = [("data", C.POINTER(C.c_uint8)), ("count", C.c_int64)]


def view(buffer, n):
    return View(C.cast(buffer, C.POINTER(C.c_uint8)), n)


def run(*args, **kw):
    r = subprocess.run([str(a) for a in args], capture_output=True, text=True, **kw)
    if r.returncode:
        raise RuntimeError(f"{args[0]} failed ({r.returncode}): {r.stderr or r.stdout}")
    return r


class Pow(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix="coax-pow-")
        cls.dir = Path(cls.temp.name)
        cls.libs = []
        for label, mode in [("checked", []), ("release", ["--release"])]:
            c = cls.dir / (label + ".c")
            c.write_text(run(COMPILER, "c", SOURCE, "--library", "--header", cls.dir / "pow.h", *mode).stdout)
            run("cc", "-std=c11", "-O2", "-shared", "-fPIC", c, "-lm", "-o", cls.dir / (label + ".dylib"))
            lib = C.CDLL(str(cls.dir / (label + ".dylib")))
            lib.pow_report_vartime.argtypes = [C.c_int64, View, C.c_int64, View]
            lib.pow_report_vartime.restype = C.c_int64
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
        r = lib.pow_report_vartime(c["op"], view(inp, len(data)), c["index"], view(self.out, CAP))
        self.assertGreaterEqual(r, 8)
        rest = C.string_at(C.addressof(self.out) + r, CAP - r)
        self.assertEqual(rest, b"\xa5" * len(rest), "past the report")
        return C.string_at(self.out, r).hex()

    def test_reference(self):
        """The transliteration agrees with Python's integers and Core's published values."""
        rng = random.Random(1)
        for _ in range(2000):
            bits = rng.getrandbits(32)
            self.assertEqual(P.proof(bits), P.proof_direct(bits))
            t, _, overflow = P.set_compact(bits)
            if not overflow:
                self.assertEqual(t, ((bits & 0x7FFFFF) * 256 ** ((bits >> 24) - 3)) & M256
                                 if bits >> 24 >= 3 else (bits & 0x7FFFFF) >> (8 * (3 - (bits >> 24))))
        for _ in range(500):
            a, c = random_number(rng), random_number(rng) or 1
            pa, pc = P.limbs(a), P.limbs(c)
            self.assertEqual(P.value(P.div(pa, pc)), a // c)
            self.assertEqual(P.value(P.sub(pa, pc)), (a - c) & M256)
            self.assertEqual(P.bits(pa), a.bit_length())
            k = rng.randrange(256)
            self.assertEqual(P.value(P.shl(pa, k)), (a << k) & M256)
            self.assertEqual(P.value(P.shr(pa, k)), a >> k)
        self.assertEqual(P.proof(0x1d00ffff), 0x100010001)
        for raw in pinned_headers():
            self.assertEqual(P.check_header(raw, P.POW_LIMIT_MAIN), P.OK)

    def test_native(self):
        codes = {}
        for c in self.cases:
            if c["op"] in (0, 4):
                at = 0 if c["op"] == 4 else 32 + 32
                code = int.from_bytes(bytes.fromhex(c["report"][2 * at:2 * at + 16]), "little")
                codes.setdefault(c["op"], set()).add(code)
        self.assertEqual(codes[0], {P.OK, P.NEGATIVE, P.ZERO, P.OVERFLOW, P.ABOVE_LIMIT})
        self.assertEqual(codes[4], {P.OK, P.NEGATIVE, P.ZERO, P.OVERFLOW, P.ABOVE_LIMIT, P.HIGH_HASH, P.CAPACITY})
        for label, lib in self.libs:
            for c in self.cases:
                self.assertEqual(self.native(lib, c), c["report"], f"{label} op {c['op']} {c['index']} {c['input'][:80]}")
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
                self.dir / (label + ".c"), HERE / "pow_wasm.c",
                "-Wl,--import-memory,--export-memory,--export=__heap_base,--max-memory=67108864,-z,stack-size=1048576,--gc-sections",
                "-o", module)
            result = json.loads(run("node", HERE / "pow_wasm.cjs", module, fixture).stdout)
            self.assertEqual((result["agree"], result["cases"]), (len(self.cases), len(self.cases)), label)
        print(f"\n{len(self.cases)} cases agree with Python, native checked/release and Wasm checked/release",
              file=sys.stderr)


if __name__ == "__main__":
    unittest.main()
