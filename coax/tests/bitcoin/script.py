#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: a Python transliteration of Bitcoin Core's interpreter decides each case outside Coax)
"""The script interpreter against a transliteration of Bitcoin Core, natively and in Wasm.

Bitcoin From Scratch, chapter 36. Compiles coax/lib/bitcoin/script.coax
(through the flat test entry point coax/tests/bitcoin/script_report.coax)
to C in checked and release builds and runs VerifyScript on:

- every runnable row of Bitcoin Core's script_tests.json under its flags,
  and again under seeded random flag sets, a few of them sets Core's
  VerifyScript asserts against;
- seeded edits of each row's scriptSig, scriptPubKey or witness: a byte
  changed, inserted or removed, the script cut short or doubled;
- every input of every transaction in tx_valid.json and tx_invalid.json,
  under each of the 4,532 flag sets transaction_tests.cpp runs them under
  (each set's verdict must be Core's), and under a seeded random set;
- P2WSH spends whose witness stacks hold 999 to 1,500 items, around the
  1,000-element limit and past the library's SCRIPT_ROOM;
- monotonicity: each row that succeeds still succeeds with seeded flags
  removed, as a soft fork requires.

It also computes legacy and BIP-143 signature hashes of the transactions
in Core's files for seeded scriptCodes, inputs and hash types, including
SIGHASH_SINGLE past the outputs. Every result must equal the one from
script_reference.py, which shares no code with Coax, and for the rows,
the error Core's test expects. Then it builds the same C for wasm32 and
runs every case again in Node. From the repository root:

    python3 coax/tests/bitcoin/script.py
"""
import ctypes as C
import hashlib
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
import script_reference as R  # noqa: E402
import tx_reference as T  # noqa: E402
from wasm_toolchain import WASM_CLANG, WASM_SYSROOT, WASM_RESOURCES  # noqa: E402

COMPILER = Path(os.environ.get("COAX_TEST_COMPILER", ROOT / "coax/bin/coax"))
SOURCE = HERE / "script_report.coax"
SEED = 36


def verify_case(tx_bytes, spk, index, amount, flags, witness_expected=None):
    tx = T.unserialize(tx_bytes, True)
    i = tx["ins"][index]
    want = R.verify_script(i["script"], spk, i["witness"], flags, R.Checker(tx, index, amount))
    return dict(tx=tx_bytes.hex(), spk=spk.hex(), index=index, amount=amount, flags=flags, want=want)


def sighash_case(tx_bytes, code, index, amount, hashtype, v0):
    tx = T.unserialize(tx_bytes, True)
    if v0: h = R.sighash_v0(code, tx, index, hashtype, amount)
    else: h = R.sighash_legacy(code, tx, index, hashtype)
    return dict(tx=tx_bytes.hex(), code=code.hex(), index=index, amount=amount, hashtype=hashtype, v0=v0, want=h.hex())


def random_flags(rng):
    """A flag set: mostly a valid one, now and then one Core asserts against."""
    flags = rng.getrandbits(len(R.FLAGS)) & ~R.F["TAPROOT"] if rng.random() < 0.8 else rng.getrandbits(len(R.FLAGS))
    return flags if rng.random() < 0.05 else R.fill_flags(flags)


def core_flag_sets(listed, valid):
    """The flag sets transaction_tests.cpp runs a transaction under, each
    with the verdict it wants. A valid transaction must pass with every flag
    but the listed ones, and with any one more removed, and must fail with
    a listed flag put back. An invalid one must fail under the listed flags
    and with any one added, and pass with a listed flag removed."""
    sets = []
    if valid:
        sets.append((R.ALL_FLAGS & ~listed, True))
        for f in R.F.values():
            sets.append((R.trim_flags(R.ALL_FLAGS & ~(listed | f)), True))
        for f in R.F.values():
            one = R.trim_flags(listed & ~f)
            if one != listed: sets.append((R.ALL_FLAGS & ~one, False))
    else:
        sets.append((listed, False))
        for f in R.F.values():
            sets.append((R.fill_flags(listed | f), False))
        for f in R.F.values():
            one = R.trim_flags(listed & ~f)
            if one != listed: sets.append((one, True))
    seen, out = set(), []
    for flags, verdict in sets:
        if flags not in seen:
            seen.add(flags)
            out.append((flags, verdict))
    return out


def edit(b, rng):
    kind = rng.randrange(6)
    if not b: return bytes([rng.randrange(256)])
    k = rng.randrange(len(b))
    if kind == 0: return b[:k] + bytes([b[k] ^ (1 << rng.randrange(8))]) + b[k + 1:]
    if kind == 1: return b[:k] + bytes([rng.choice([0, 0x51, 0x63, 0x67, 0x68, 0x76, 0x87, 0xac, 0xae, rng.randrange(256)])]) + b[k:]
    if kind == 2: return b[:k] + b[k + 1:]
    if kind == 3: return b[:k]
    if kind == 4: return b + b
    return bytes([rng.randrange(256)]) + b


def respend(row, sig, spk, witness):
    credit = R.crediting(spk, row["amount"])
    return T.serialize(R.spending(sig, witness, credit), True), spk


def big_witness(count, script):
    """A P2WSH spend whose witness is count - 1 items of 0x01 and the script."""
    spk = b"\x00\x20" + hashlib.sha256(script).digest()
    witness = [b"\x01"] * (count - 1) + [script]
    credit = R.crediting(spk, 0)
    return T.serialize(R.spending(b"", witness, credit), True), spk


def cases():
    rng = random.Random(SEED)
    out, rows = [], [r for r in R.script_rows() if r["runnable"]]
    assert len(rows) == 1256
    for r in rows:
        c = verify_case(r["tx"], r["spk"], 0, r["amount"], r["flags"])
        assert R.ERRORS[c["want"]] == r["expected"], r["line"]
        c["row"] = r["line"]
        out.append(c)
        for _ in range(3):
            out.append(verify_case(r["tx"], r["spk"], 0, r["amount"], random_flags(rng)))
        for _ in range(2):
            sig, spk, witness = r["sig"], r["spk"], list(r["witness"])
            part = rng.randrange(3)
            if part == 0: sig = edit(sig, rng)
            elif part == 1: spk = edit(spk, rng)
            elif witness:
                k = rng.randrange(len(witness))
                witness[k] = edit(witness[k], rng)
            tx, spk = respend(r, sig, spk, witness)
            out.append(verify_case(tx, spk, 0, r["amount"], rng.choice([r["flags"], random_flags(rng)])))
        if c["want"] == 0:
            for _ in range(2):
                fewer = R.trim_flags(r["flags"] & rng.getrandbits(len(R.FLAGS)))
                m = verify_case(r["tx"], r["spk"], 0, r["amount"], fewer)
                assert m["want"] == 0, ("monotonicity", r["line"], fewer)
                m["monotone"] = True
                out.append(m)
    sets = 0
    for name, valid in (("tx_valid.json", True), ("tx_invalid.json", False)):
        for row in R.tx_rows(name):
            if row["flags_text"] == "BADTX": continue
            tx = T.unserialize(row["tx"], True)
            listed = R.parse_flags(row["flags_text"])
            prevs = [row["prevouts"][(i["prevout"][:32], int.from_bytes(i["prevout"][32:], "little"))] for i in tx["ins"]]
            for flags, verdict in core_flag_sets(listed, valid):
                sets += 1
                inputs = [verify_case(row["tx"], spk, n, amount, flags) for n, (spk, amount) in enumerate(prevs)]
                assert all(c["want"] == 0 for c in inputs) == verdict, (name, row["tx"].hex()[:40], flags)
                out += inputs
            for n, (spk, amount) in enumerate(prevs):
                out.append(verify_case(row["tx"], spk, n, amount, random_flags(rng)))
            for _ in range(4):
                n = rng.randrange(len(tx["ins"]))
                code = rng.choice([b"", rng.randbytes(rng.randrange(60)), b"\xab" + rng.randbytes(30) + b"\xab\xac",
                                   rng.randbytes(300), b"\x4c\x05ab"])
                hashtype = rng.choice([1, 2, 3, 0x81, 0x82, 0x83, 0, rng.getrandbits(32), rng.randrange(256)])
                out.append(sighash_case(row["tx"], code, n, rng.randrange(1 << 40), hashtype, rng.randrange(2)))
    assert sets == 4532, sets
    # SIGHASH_SINGLE past the outputs, both versions
    many = next(row["tx"] for row in R.tx_rows("tx_valid.json")
                if len(T.unserialize(row["tx"], True)["ins"]) > len(T.unserialize(row["tx"], True)["outs"]))
    t = T.unserialize(many, True)
    for v0 in (0, 1):
        out.append(sighash_case(many, b"\x51", len(t["outs"]), 5, 3, v0))
        out.append(sighash_case(many, b"\x51", len(t["outs"]), 5, 0x83, v0))
    # Witness stacks around the 1,000-element limit and past SCRIPT_ROOM (1,100).
    for count in (999, 1000, 1001, 1100, 1101, 1102, 1200, 1500):
        for script in (b"\x75" * 200 + b"\x51" * 0, b"\x74\x6d" * 100, b"\x01\x05\x79\x75\x51",
                       b"\x02\xe8\x03\x79", b"\x02\x4c\x04\x79", b"\x02\x4c\x04\x7a"):
            tx, spk = big_witness(count, script)
            out.append(verify_case(tx, spk, 0, 0, R.fill_flags(R.F["WITNESS"])))
            out.append(verify_case(tx, spk, 0, 0, R.fill_flags(R.F["CLEANSTACK"])))
    return out


# ---- the native builds
class View(C.Structure):
    _fields_ = [("data", C.POINTER(C.c_uint8)), ("count", C.c_int64)]


def view(b):
    buf = (C.c_uint8 * max(1, len(b))).from_buffer_copy(b.ljust(1, b"\0"))
    return View(C.cast(buf, C.POINTER(C.c_uint8)), len(b)), buf


def run(*args, **kw):
    r = subprocess.run([str(a) for a in args], capture_output=True, text=True, **kw)
    if r.returncode:
        raise RuntimeError(f"{args[0]} failed ({r.returncode}): {r.stderr or r.stdout}")
    return r


class Script(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix="coax-script-")
        cls.dir = Path(cls.temp.name)
        cls.libs = []
        for label, mode in [("checked", []), ("release", ["--release"])]:
            c = cls.dir / (label + ".c")
            c.write_text(run(COMPILER, "c", SOURCE, "--library", "--header", cls.dir / "script.h", *mode).stdout)
            run("cc", "-std=c11", "-O2", "-shared", "-fPIC", c, "-lm", "-o", cls.dir / (label + ".dylib"))
            lib = C.CDLL(str(cls.dir / (label + ".dylib")))
            lib.script_report_vartime.argtypes = [View, View, C.c_int64, C.c_int64, C.c_int64]
            lib.script_report_vartime.restype = C.c_int64
            lib.script_sighash_report_vartime.argtypes = [View, View, C.c_int64, C.c_int64, C.c_int64, C.c_int64, View]
            lib.script_sighash_report_vartime.restype = C.c_int64
            cls.libs.append((label, lib))
        cls.cases = cases()

    @classmethod
    def tearDownClass(cls):
        cls.temp.cleanup()

    def native(self, lib, c):
        tx, keep1 = view(bytes.fromhex(c["tx"]))
        if "spk" in c:
            spk, keep2 = view(bytes.fromhex(c["spk"]))
            return lib.script_report_vartime(tx, spk, c["index"], c["amount"], c["flags"])
        code, keep2 = view(bytes.fromhex(c["code"]))
        out = (C.c_uint8 * 40)(*([0xA5] * 40))
        r = lib.script_sighash_report_vartime(tx, code, c["index"], c["amount"], c["hashtype"], c["v0"],
                                              View(C.cast(out, C.POINTER(C.c_uint8)), 32))
        self.assertEqual(bytes(out[32:]), b"\xa5" * 8, "past the hash")
        return bytes(out[:32]).hex() if r == 32 else str(r)

    def test_native(self):
        results = {}
        for c in self.cases:
            if "spk" in c: results[c["want"]] = results.get(c["want"], 0) + 1
        for label, lib in self.libs:
            for c in self.cases:
                self.assertEqual(self.native(lib, c), c["want"], f"{label}: {json.dumps(c)[:300]}")
        verify = sum(1 for c in self.cases if "spk" in c)
        named = {(R.ERRORS[k] if k < len(R.ERRORS) else {100: "TAPROOT_UNBUILT", 101: "BAD_FLAGS"}[k]): v
                 for k, v in sorted(results.items())}
        print(f"\n{verify} VerifyScript cases, {len(self.cases) - verify} signature hashes, "
              f"{sum(1 for c in self.cases if c.get('monotone'))} with flags removed; results {named}", file=sys.stderr)

    def test_wasm(self):
        clang, sysroot, resources = Path(WASM_CLANG), Path(WASM_SYSROOT), Path(WASM_RESOURCES)
        self.assertTrue(clang.exists() and sysroot.exists() and resources.exists(), "a Wasm toolchain is required; see coax/web/wasm.sh")
        fixture = self.dir / "wasm.json"
        fixture.write_text(json.dumps(self.cases))
        for label in ["checked", "release"]:
            module = self.dir / (label + ".wasm")
            run(clang, "--target=wasm32-wasip1", "--sysroot=" + str(sysroot), "-resource-dir=" + str(resources),
                "-std=c11", "-O2", "-I" + str(self.dir), "-mexec-model=reactor",
                self.dir / (label + ".c"), HERE / "script_wasm.c",
                "-Wl,--import-memory,--export-memory,--export=__heap_base,--max-memory=67108864,-z,stack-size=4194304,--gc-sections",
                "-o", module)
            result = json.loads(run("node", HERE / "script_wasm.cjs", module, fixture).stdout)
            self.assertEqual((result["agree"], result["cases"]), (len(self.cases), len(self.cases)), label)
        print(f"\n{len(self.cases)} cases agree with Python, native checked/release and Wasm checked/release",
              file=sys.stderr)


if __name__ == "__main__":
    unittest.main()
