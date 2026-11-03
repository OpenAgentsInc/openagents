#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: BIP-158's published filters and headers, Bitcoin Core's own Python SipHash and element hash, and a transliteration of golombrice.h decide each case outside Coax)
"""Compact block filters against BIP-158 and Bitcoin Core.

Bitcoin From Scratch, chapter 74. Before any Coax code runs, this script
checks its Python reference against BIP-158's testnet-19.json: for every
block it reads the scripts, hashes them with Bitcoin Core's own
test_framework/blockfilter.py and crypto/siphash.py (pinned), codes them
with a transliteration of util/golombrice.h, and must reach the published
filter and filter header. testnet-19.json and Core's
src/test/data/blockfilters.json must be the same file.

Then it compiles coax/lib/bitcoin/blockfilter.coax to C in checked and
release builds and compares, against the reference: the basic filter and
header of every BIP-158 block; seeded element sets with duplicates, under
seeded keys; matches of members and non-members; and seeded damage to
filters (cut, extended, bytes changed), each read with Core's decode check.

    python3 coax/tests/bitcoin/blockfilter.py            # check
    python3 coax/tests/bitcoin/blockfilter.py --write    # regenerate the vectors
"""
import ctypes as C
import hashlib
import json
import os
from pathlib import Path
import random
import struct
import subprocess
import sys
import tempfile
import types
import unittest

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
COMPILER = Path(os.environ.get("COAX_TEST_COMPILER", ROOT / "coax/bin/coax"))
SOURCE = ROOT / "coax/lib/bitcoin/blockfilter.coax"
TABLE = HERE / "blockfilter_vectors.coax"
UPSTREAM = ROOT / "coax/tests/vectors/upstream"
P, M = 19, 784931


def read(name):
    raw = (UPSTREAM / name).read_bytes()
    manifest = json.loads((UPSTREAM.parent / "manifest.json").read_text())
    want = [f["sha256"] for f in manifest["files"] if f["path"] == "upstream/" + name]
    assert want and hashlib.sha256(raw).hexdigest() == want[0], f"{name} does not match its pin"
    return raw


def core_element_hash():
    """Core's bip158_basic_element_hash, loaded from the pinned test framework."""
    for pkg in ["core_tf", "core_tf.crypto"]:
        m = types.ModuleType(pkg)
        m.__path__ = []
        sys.modules[pkg] = m
    for name, path in [("core_tf.crypto.siphash", "bitcoin-core/siphash.py"), ("core_tf.blockfilter", "bitcoin-core/blockfilter.py")]:
        mod = types.ModuleType(name)
        mod.__package__ = name.rsplit(".", 1)[0]
        sys.modules[name] = mod
        exec(compile(read(path).decode(), str(UPSTREAM / path), "exec"), mod.__dict__)
    return sys.modules["core_tf.blockfilter"].bip158_basic_element_hash, sys.modules["core_tf.crypto.siphash"].siphash


# ---- the reference
def sha256d(b):
    return hashlib.sha256(hashlib.sha256(b).digest()).digest()


def compact(n):
    if n < 253: return bytes([n])
    if n <= 0xffff: return b"\xfd" + struct.pack("<H", n)
    if n <= 0xffffffff: return b"\xfe" + struct.pack("<I", n)
    return b"\xff" + struct.pack("<Q", n)


def read_compact(b, at):
    c = b[at]
    if c < 253: return c, at + 1
    w = {253: 2, 254: 4, 255: 8}[c]
    return int.from_bytes(b[at + 1:at + 1 + w], "little"), at + 1 + w


def block_scripts(block):
    """(output scripts, number of inputs after the coinbase) of a block."""
    n, at = read_compact(block, 80)
    outs, inputs = [], 0
    for t in range(n):
        at += 4
        segwit = block[at] == 0 and block[at + 1] == 1
        if segwit: at += 2
        nin, at = read_compact(block, at)
        for _ in range(nin):
            at += 36
            k, at = read_compact(block, at)
            at += k + 4
        if t: inputs += nin
        nout, at = read_compact(block, at)
        for _ in range(nout):
            at += 8
            k, at = read_compact(block, at)
            outs.append(block[at:at + k])
            at += k
        if segwit:
            for _ in range(nin):
                items, at = read_compact(block, at)
                for _ in range(items):
                    k, at = read_compact(block, at)
                    at += k
        at += 4
    assert at == len(block)
    return outs, inputs


def basic_elements(block, prevs):
    outs, inputs = block_scripts(block)
    assert inputs == len(prevs)
    s = {o for o in outs if o and o[0] != 0x6a}
    s |= {p for p in prevs if p}
    return s


class Bits:
    def __init__(self):
        self.bits = []

    def put(self, value, n):
        self.bits += [(value >> (n - 1 - i)) & 1 for i in range(n)]

    def bytes(self):
        b = self.bits + [0] * (-len(self.bits) % 8)
        return bytes(int("".join(map(str, b[i:i + 8])), 2) for i in range(0, len(b), 8))


def golomb_encode(values):
    """CompactSize(N), then GolombRiceEncode of each difference (golombrice.h)."""
    out, w, last = compact(len(values)), Bits(), 0
    for v in sorted(values):
        x = v - last
        q = x >> P
        w.put((1 << q) - 1, q)    # q ones
        w.put(0, 1)
        w.put(x & ((1 << P) - 1), P)
        last = v
    return out + (w.bytes() if values else b"")


def golomb_decode(f):
    """The values, or None when Core's decode check would throw."""
    try:
        n, at = read_compact(f, 0)
    except (IndexError, KeyError):
        return None
    if at > len(f) or n >= 1 << 32:
        return None
    bits = "".join(f"{x:08b}" for x in f[at:])
    pos, value, out = 0, 0, []
    for _ in range(n):
        q = 0
        while True:
            if pos >= len(bits): return None
            if bits[pos] == "0": break
            q, pos = q + 1, pos + 1
        pos += 1
        if pos + P > len(bits): return None
        value = (value + (q << P) + int(bits[pos:pos + P], 2)) & (2**64 - 1)
        out.append(value)
        pos += P
    if (pos + 7) // 8 != len(bits) // 8:
        return None
    return n, out


def key_of(block_hash_display):
    h = bytes.fromhex(block_hash_display)[::-1]
    return int.from_bytes(h[:8], "little"), int.from_bytes(h[8:16], "little")


def vectors():
    rows = json.loads(read("bip158/testnet-19.json"))
    assert rows == json.loads(read("bitcoin-core/blockfilters.json")), "BIP-158's and Core's vectors differ"
    return [dict(height=r[0], hash=r[1], block=bytes.fromhex(r[2]), prevs=[bytes.fromhex(p) for p in r[3]],
                 prev_header=r[4], filter=bytes.fromhex(r[5]), header=r[6], note=r[7]) for r in rows[1:]]


def check_reference(element_hash):
    for v in vectors():
        block = v["block"]
        assert sha256d(block[:80])[::-1].hex() == v["hash"]
        els = basic_elements(block, v["prevs"])
        values = [element_hash(e, len(els), v["hash"]) for e in els]
        f = golomb_encode(values)
        assert f == v["filter"], v["height"]
        assert golomb_decode(f) == (len(els), sorted(values))
        header = sha256d(sha256d(f) + bytes.fromhex(v["prev_header"])[::-1])[::-1].hex()
        assert header == v["header"], v["height"]


def table():
    rows = []
    for v in vectors():
        prevs = b"".join(compact(len(p)) + p for p in v["prevs"])
        rows.append(f'  BfVector.{{height={v["height"]}, hash="{v["hash"]}", block="{v["block"].hex()}", prevouts="{prevs.hex()}", '
                    f'prev_header="{v["prev_header"]}", filter="{v["filter"].hex()}", header="{v["header"]}", note="{v["note"]}"}},')
    return ("// Generated by coax/tests/bitcoin/blockfilter.py --write. Test data only.\n"
            "// BIP-158's testnet-19.json (the same file as Bitcoin Core's\n"
            "// src/test/data/blockfilters.json; see SOURCES.md): each block, its spent\n"
            "// scripts as CompactSize-prefixed bytes in input order, the previous\n"
            "// filter header, the basic filter and its header. Hashes are as Core\n"
            "// prints them (reversed).\n"
            "BfVector :: struct {height: int; hash: string; block: string; prevouts: string; prev_header: string; filter: string; header: string; note: string;}\n"
            f"BF_VECTORS_COUNT :: {len(rows)};\n"
            f"BF_VECTORS :: [{len(rows)}] BfVector.[\n" + "\n".join(rows) + "\n];\n")


# ---- ctypes
class View(C.Structure):
    _fields_ = [("data", C.POINTER(C.c_uint8)), ("count", C.c_int64)]


class View64(C.Structure):
    _fields_ = [("data", C.POINTER(C.c_uint64)), ("count", C.c_int64)]


def buf(n, fill=0xA5):
    return (C.c_uint8 * max(1, n))(*([fill] * max(1, n)))


def view(b, n):
    return View(C.cast(b, C.POINTER(C.c_uint8)), n)


def bview(data):
    b = (C.c_uint8 * max(1, len(data)))(*data) if data else buf(1)
    return b, view(b, len(data))


def u64view(values):
    a = (C.c_uint64 * max(1, len(values)))(*values)
    return a, View64(C.cast(a, C.POINTER(C.c_uint64)), len(values))


def run(*args, **kw):
    r = subprocess.run([str(a) for a in args], capture_output=True, text=True, **kw)
    if r.returncode:
        raise RuntimeError(f"{args[0]} failed ({r.returncode}): {r.stderr or r.stdout}")
    return r


class BlockFilterTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        element_hash, siphash = core_element_hash()
        cls.element_hash, cls.siphash = staticmethod(element_hash), staticmethod(siphash)
        check_reference(cls.element_hash)
        assert TABLE.read_text() == table(), f"{TABLE.relative_to(ROOT)} is stale: run with --write"
        cls.temp = tempfile.TemporaryDirectory(prefix="coax-blockfilter-")
        d = Path(cls.temp.name)
        cls.libs = []
        for label, mode in [("checked", []), ("release", ["--release"])]:
            c = d / (label + ".c")
            c.write_text(run(COMPILER, "c", SOURCE, "--library", "--header", d / "blockfilter.h", *mode).stdout)
            run("cc", "-std=c11", "-O2", "-shared", "-fPIC", c, "-lm", "-o", d / (label + ".dylib"))
            lib = C.CDLL(str(d / (label + ".dylib")))
            V, U = View, C.c_uint64
            lib.bf_build_c.argtypes = [U, U, V, View64, V, View64]
            lib.bf_match_c.argtypes = [U, U, V, V, View64, View64]
            lib.bf_read_c.argtypes = [V]
            lib.bf_basic_c.argtypes = [V, V, V, V, V, V, View64]
            for f in ["bf_build_c", "bf_match_c", "bf_read_c", "bf_basic_c"]:
                getattr(lib, f).restype = C.c_int64
            cls.libs.append((label, lib))
        cls.counts = {}

    @classmethod
    def tearDownClass(cls):
        cls.temp.cleanup()
        print(f"\n{cls.counts}", file=sys.stderr)

    def count(self, k, n=1):
        self.counts[k] = self.counts.get(k, 0) + n

    def build(self, lib, k0, k1, elements):
        pool = b"".join(elements)
        keep, pv = bview(pool)
        keep2, lv = u64view([len(e) for e in elements])
        size = 16 + 4 * len(elements) + 64
        out = buf(size + 1)
        f, fv = u64view([0, 0])
        code = lib.bf_build_c(k0, k1, pv, lv, view(out, size), fv)
        return code, bytes(out[:f[0]]), f[1]

    def reference(self, k0, k1, elements):
        s = set(elements)
        values = [(self.siphash(k0, k1, e) * len(s) * M) >> 64 for e in s]
        return golomb_encode(values), len(s), set(values)

    def test_bip158_blocks(self):
        for label, lib in self.libs:
            for v in vectors():
                prevs = b"".join(compact(len(p)) + p for p in v["prevs"])
                keep, bv = bview(v["block"]); keep2, pv = bview(prevs)
                keep3, hv = bview(bytes.fromhex(v["prev_header"])[::-1])
                pool, out, header = buf(len(v["block"]) + len(prevs)), buf(600), buf(33)
                f, fv = u64view([0, 0])
                code = lib.bf_basic_c(bv, pv, hv, view(pool, len(v["block"]) + len(prevs)), view(out, 599), view(header, 32), fv)
                self.assertEqual(code, 0, v["height"])
                self.assertEqual(bytes(out[:f[0]]), v["filter"], v["height"])
                self.assertEqual(bytes(header[:32])[::-1].hex(), v["header"])
                self.assertEqual(header[32], 0xA5)
                keep4, fv2 = bview(v["filter"])
                self.assertEqual(lib.bf_read_c(fv2), golomb_decode(v["filter"])[0])
                # Each element of the block matches; refusals for the spent scripts.
                k0, k1 = key_of(v["hash"])
                for e in basic_elements(v["block"], v["prevs"]):
                    self.assertEqual(self.match(lib, k0, k1, v["filter"], [e]), (0, True))
                    self.count("block elements matched")
                if v["prevs"]:
                    keep5, short = bview(prevs[:-1])
                    self.assertEqual(lib.bf_basic_c(bv, short, hv, view(pool, len(v["block"]) + len(prevs)), view(out, 599), view(header, 32), fv), 3)
                self.count("BIP-158 blocks")

    def match(self, lib, k0, k1, filt, queries):
        keep, fv = bview(filt)
        keep2, pv = bview(b"".join(queries))
        keep3, lv = u64view([len(q) for q in queries])
        f, out = u64view([7])
        code = lib.bf_match_c(k0, k1, fv, pv, lv, out)
        return code, bool(f[0]) if code == 0 else None

    def test_sets_and_matches(self):
        rng = random.Random(74)
        for label, lib in self.libs:
            for case in range(150):
                k0, k1 = rng.randrange(2**64), rng.randrange(2**64)
                n = rng.choice([0, 1, 2, 3, rng.randrange(0, 40), rng.randrange(0, 400)])
                elements = [rng.randbytes(rng.choice([0, 1, 20, 22, 25, 34, rng.randrange(0, 80)])) for _ in range(n)]
                elements += [rng.choice(elements) for _ in range(rng.randrange(0, 4))] if elements else []
                rng.shuffle(elements)
                code, filt, count = self.build(lib, k0, k1, elements)
                want, wn, values = self.reference(k0, k1, elements)
                self.assertEqual((code, filt, count), (0, want, wn), case)
                keep, fv = bview(filt)
                self.assertEqual(lib.bf_read_c(fv), wn)
                self.count("element sets")
                # Queries: members, strangers, and mixes.
                for _ in range(6):
                    qs = [rng.randbytes(rng.randrange(1, 40)) for _ in range(rng.randrange(1, 5))]
                    if elements and rng.random() < 0.5:
                        qs.append(rng.choice(elements))
                    rng.shuffle(qs)
                    expect = bool(wn) and any((self.siphash(k0, k1, q) * wn * M) >> 64 in values for q in qs)
                    self.assertEqual(self.match(lib, k0, k1, filt, qs), (0, expect))
                    self.count("matches")

    def test_damaged_filters(self):
        rng = random.Random(7474)
        for label, lib in self.libs:
            for case in range(300):
                k0, k1 = rng.randrange(2**64), rng.randrange(2**64)
                elements = [rng.randbytes(20) for _ in range(rng.randrange(0, 30))]
                code, filt, _ = self.build(lib, k0, k1, elements)
                kind = rng.randrange(4)
                bad = bytearray(filt)
                if kind == 0 and bad:
                    bad = bad[:rng.randrange(len(bad))]
                elif kind == 1:
                    bad += rng.randbytes(rng.randrange(1, 3))
                elif kind == 2 and bad:
                    bad[rng.randrange(len(bad))] = rng.randrange(256)
                else:
                    bad = bytearray(rng.randbytes(rng.randrange(0, 12)))
                keep, fv = bview(bytes(bad))
                ref = golomb_decode(bytes(bad))
                self.assertEqual(lib.bf_read_c(fv), -4 if ref is None else ref[0], bytes(bad).hex())
                if ref is not None:
                    q = rng.randbytes(20)
                    expect = bool(ref[0]) and (self.siphash(k0, k1, q) * ref[0] * M) >> 64 in set(ref[1])
                    self.assertEqual(self.match(lib, k0, k1, bytes(bad), [q]), (0, expect))
                else:
                    self.assertEqual(self.match(lib, k0, k1, bytes(bad), [b"x"])[0], 4)
                self.count("damaged filters")


if __name__ == "__main__":
    if "--write" in sys.argv[1:]:
        eh, _ = core_element_hash()
        check_reference(eh)
        TABLE.write_text(table())
        sys.exit(0)
    unittest.main()
