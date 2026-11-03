#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: a Python transliteration of Bitcoin Core's reader decides each case outside Coax)
"""The transaction reader against a transliteration of Bitcoin Core, natively and in Wasm.

Bitcoin From Scratch, chapter 34. Compiles coax/lib/bitcoin/tx.coax (through
the flat test entry point coax/tests/bitcoin/tx_report.coax) to C in checked
and release builds and reads, with witnesses, without them, and as Core's
DecodeTx:

- every transaction of Bitcoin Core's tx_valid.json and tx_invalid.json,
  pinned in coax/tests/vectors/upstream/bitcoin-core/;
- the six mainnet transactions of tx_vectors.coax, and the ambiguous
  zero-input cases;
- seeded edits of all of those: a byte changed, inserted or removed, the
  bytes cut short, a CompactSize rewritten in a longer form, a flag
  changed, a byte appended;
- seeded random transactions, with and without witness stacks, some of
  them empty, and counts written as large as MAX_SIZE and past it;
- each transaction again with room for one input, output or witness item
  fewer than it has;
- a transaction of more than a mebibyte, so the txid and the wtxid hash it
  in pieces.

Each report (the refusal code and byte, or the sizes, weight, IDs and both
serializations) must equal the one built from tx_reference.py, which shares
no code with Coax. The output buffer past the report must be untouched.
Then it builds the same C for wasm32 and runs every case again in Node.
From the repository root:

    python3 coax/tests/bitcoin/tx.py
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
import tx_reference as R  # noqa: E402
import make_tx_vectors as V  # noqa: E402
from wasm_toolchain import WASM_CLANG, WASM_SYSROOT, WASM_RESOURCES  # noqa: E402

COMPILER = Path(os.environ.get("COAX_TEST_COMPILER", ROOT / "coax/bin/coax"))
SOURCE = HERE / "tx_report.coax"
ROOM = 256          # TR_MAX in tx_report.coax
CAP = 3 * 1024 * 1024
MODES = (0, 1, 2)   # with witnesses, without, DecodeTx


def le(v):
    return (v & (1 << 64) - 1).to_bytes(8, "little")


def report(data, mode, rooms):
    """What tx_report_vartime writes for these bytes, from the reference."""
    caps = tuple(rooms)
    if mode == 2:
        tx, refused = R.decode(data, caps)
    else:
        tx, refused = R.attempt(data, mode == 0, caps)
    if tx is None:
        return (le(refused.code) + le(refused.at)).hex()
    stripped, full = R.serialize(tx, False), R.serialize(tx, True)
    nitems = sum(len(i["witness"]) for i in tx["ins"]) if tx["segwit"] else 0
    weight = R.weight(tx)
    values = [int(tx["segwit"]), len(tx["ins"]), len(tx["outs"]), nitems, len(stripped), len(full),
              weight, (weight + 3) // 4, int.from_bytes(tx["version"], "little"), int.from_bytes(tx["locktime"], "little")]
    assert full == data, "a read transaction serializes to its own bytes"
    return (le(R.OK) + le(0) + b"".join(le(v) for v in values) + R.txid(tx) + R.wtxid(tx) + stripped + full).hex()


def case(data, mode, rooms=(ROOM, ROOM, ROOM)):
    return dict(input=data.hex(), mode=mode, rooms=list(rooms), report=report(data, mode, rooms))


def random_tx(rng):
    """A random transaction as the reference's dict."""
    def script():
        n = rng.choice([0, 0, 1, 25, 34, rng.randrange(300)])
        return rng.randbytes(n)
    nin, nout = rng.choice([0, 1, 1, 2, 3, 5]), rng.choice([0, 1, 1, 2, 4])
    ins = [dict(prevout=rng.randbytes(36), script=script(), sequence=rng.randbytes(4), witness=[]) for _ in range(nin)]
    if rng.random() < 0.6:
        for i in ins:
            i["witness"] = [rng.randbytes(rng.choice([0, 1, 33, 64, 72, rng.randrange(600)])) for _ in range(rng.choice([0, 1, 2, 2, 4]))]
    outs = [dict(value=rng.randbytes(8), script=script()) for _ in range(nout)]
    return dict(version=rng.randbytes(4), ins=ins, outs=outs, locktime=rng.randbytes(4), segwit=False)


def longer(data, rng):
    """One CompactSize-looking byte below 0xfd rewritten in a longer form."""
    spots = [k for k in range(4, len(data)) if data[k] < 0xFD]
    if not spots:
        return data
    k = rng.choice(spots)
    width = rng.choice([2, 4, 8])
    return data[:k] + bytes([{2: 0xFD, 4: 0xFE, 8: 0xFF}[width]]) + data[k:k + 1] + bytes(width - 1) + data[k + 1:]


def edit(data, rng):
    kind = rng.randrange(8)
    k = rng.randrange(len(data))
    if kind == 0: return data[:k] + bytes([data[k] ^ (1 << rng.randrange(8))]) + data[k + 1:]
    if kind == 1: return data[:k] + bytes([rng.randrange(256)]) + data[k:]
    if kind == 2: return data[:k] + data[k + 1:]
    if kind == 3: return data[:k]
    if kind == 4: return longer(data, rng)
    if kind == 5: return data[:5] + bytes([rng.choice([0, 1, 2, 3, 0x81])]) + data[6:] if len(data) > 6 else data
    if kind == 6: return data + bytes([rng.randrange(256)])
    big = rng.choice([R.MAX_SIZE, R.MAX_SIZE + 1, 0xFFFFFFFF, 1 << 63])
    return data[:k] + R.compact(big) + data[k + 1:]


def big_tx():
    """More than a mebibyte both stripped and in full."""
    rng = random.Random(341)
    tx = dict(version=b"\x02\0\0\0", locktime=bytes(4), segwit=True,
              ins=[dict(prevout=rng.randbytes(36), script=b"\x51" * 550000, sequence=b"\xff" * 4,
                        witness=[rng.randbytes(550000), b""])],
              outs=[dict(value=le(1), script=b"\x51" * 550000)])
    return R.serialize(tx, True)


def cases():
    rng = random.Random(34)
    inputs = [bytes.fromhex(h) for h, _valid in V.core] + [bytes.fromhex(m["tx"]) for m in V.mainnet]
    inputs += [bytes.fromhex(h) for h in (V.segwit_no_inputs, V.zero_inputs, V.both_sane, V.legacy_sane, V.neither_sane, '01000000000000000000')]
    inputs += [R.serialize(random_tx(rng), True) for _ in range(400)]
    out = [case(d, mode) for d in inputs for mode in MODES]
    for d in inputs:
        for _ in range(4):
            out.append(case(edit(d, rng), rng.choice(MODES)))
    # one element too few room: inputs, outputs, witness items
    for d in inputs:
        tx, _ = R.attempt(d, True, (ROOM, ROOM, ROOM))
        if tx is None:
            continue
        counts = [len(tx["ins"]), len(tx["outs"]), sum(len(i["witness"]) for i in tx["ins"])]
        for k in range(3):
            if counts[k]:
                rooms = [ROOM, ROOM, ROOM]
                rooms[k] = counts[k] - 1
                out.append(case(d, rng.choice(MODES), rooms))
                rooms[k] = counts[k]
                out.append(case(d, 0, rooms))
    big = big_tx()
    out += [case(big, 0), case(big, 1), case(big[:-1], 0)]
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


class Transactions(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix="coax-tx-")
        cls.dir = Path(cls.temp.name)
        cls.libs = []
        for label, mode in [("checked", []), ("release", ["--release"])]:
            c = cls.dir / (label + ".c")
            c.write_text(run(COMPILER, "c", SOURCE, "--library", "--header", cls.dir / "tx.h", *mode).stdout)
            run("cc", "-std=c11", "-O2", "-shared", "-fPIC", c, "-lm", "-o", cls.dir / (label + ".dylib"))
            lib = C.CDLL(str(cls.dir / (label + ".dylib")))
            lib.tx_report_vartime.argtypes = [View, C.c_int64, C.c_int64, C.c_int64, C.c_int64, View]
            lib.tx_report_vartime.restype = C.c_int64
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
        r = lib.tx_report_vartime(view(inp, len(data)), c["mode"], *c["rooms"], view(self.out, CAP))
        self.assertGreaterEqual(r, 16)
        rest = C.string_at(C.addressof(self.out) + r, min(CAP - r, 4096))
        self.assertEqual(rest, b"\xa5" * len(rest), "past the report")
        return C.string_at(self.out, r).hex()

    def test_reference(self):
        """The reference gives the published IDs and weights, and Core's rows read."""
        for m in V.mainnet:
            tx, refused = R.decode(bytes.fromhex(m["tx"]))
            self.assertIsNone(refused)
            self.assertEqual(R.txid(tx)[::-1].hex(), m["txid"])
            self.assertEqual(R.wtxid(tx)[::-1].hex(), m["wtxid"])
            self.assertEqual(R.weight(tx), m["weight"])
        for h, _valid in V.core:
            self.assertIsNotNone(R.decode(bytes.fromhex(h))[0])

    def test_native(self):
        codes = {}
        for c in self.cases:
            code = int.from_bytes(bytes.fromhex(c["report"][:16]), "little")
            codes[code] = codes.get(code, 0) + 1
        self.assertEqual(set(codes), set(range(8)), codes)
        for label, lib in self.libs:
            for c in self.cases:
                self.assertEqual(self.native(lib, c), c["report"], f"{label} mode {c['mode']} {c['input'][:80]}")
        print(f"\n{len(self.cases)} cases; codes {dict(sorted(codes.items()))}", file=sys.stderr)

    def test_wasm(self):
        clang, sysroot, resources = Path(WASM_CLANG), Path(WASM_SYSROOT), Path(WASM_RESOURCES)
        self.assertTrue(clang.exists() and sysroot.exists() and resources.exists(), "a Wasm toolchain is required; see coax/web/wasm.sh")
        fixture = self.dir / "wasm.json"
        fixture.write_text(json.dumps(self.cases))
        for label in ["checked", "release"]:
            module = self.dir / (label + ".wasm")
            run(clang, "--target=wasm32-wasip1", "--sysroot=" + str(sysroot), "-resource-dir=" + str(resources),
                "-std=c11", "-O2", "-I" + str(self.dir), "-mexec-model=reactor",
                self.dir / (label + ".c"), HERE / "tx_wasm.c",
                "-Wl,--import-memory,--export-memory,--export=__heap_base,--max-memory=67108864,-z,stack-size=1048576,--gc-sections",
                "-o", module)
            result = json.loads(run("node", HERE / "tx_wasm.cjs", module, fixture).stdout)
            self.assertEqual((result["agree"], result["cases"]), (len(self.cases), len(self.cases)), label)
        print(f"\n{len(self.cases)} cases agree with Python, native checked/release and Wasm checked/release",
              file=sys.stderr)


if __name__ == "__main__":
    unittest.main()
