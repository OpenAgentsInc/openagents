#!/usr/bin/env python3
# Firm reason: 3 (runs BIP-350's reference Python implementation, segwit_addr.py, that we test against)
"""Bech32m and segwit addresses against BIP-350's reference implementation, natively and in Wasm.

Bitcoin From Scratch, chapter 28. Compiles coax/lib/bitcoin/segwit.coax to
C in checked and release builds and runs the three procedures it exports
(segwit_variant_vartime, segwit_decode_vartime, segwit_encode_vartime) on:

- every Bech32m string and segwit address in BIP-350 and BIP-173, the
  Bech32 strings of BIP-173, and every Bech32 or Bech32m address and invalid
  string in Bitcoin Core's key_io files, all pinned in
  coax/tests/vectors/upstream/;
- every witness version 0 to 17, program length 0 to 41 and network, encoded;
- seeded edits of valid addresses: a character replaced, inserted, removed
  or its case flipped, two characters swapped, 'q's inserted before a final
  'p', and the checksum recomputed with the other constant;
- output buffers one byte or character too small.

The oracle is sipa/bech32's ref/python/segwit_addr.py at 7a7d7ab1, pinned as
coax/tests/vectors/upstream/sipa-bech32/segwit_addr.py.txt and loaded only
after its digest matches. It decides whether each string is Bech32 or
Bech32m, whether each address is valid and what it holds, and what each
encoding is. It returns no reasons, so the refusal Coax must give is
computed by the short classifier below, which applies the chapter's rules in
the chapter's order and must agree with the oracle on every accept and
refuse. Then it builds the same C for wasm32 and runs every case again in
Node. Run from the repository root:

    python3 coax/tests/bitcoin/segwit.py
"""
import ctypes as C
import hashlib
import json
import os
from pathlib import Path
import random
import re
import subprocess
import sys
import tempfile
import types
import unittest

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(ROOT / "coax/tests/nostr"))
from wasm_toolchain import WASM_CLANG, WASM_SYSROOT, WASM_RESOURCES  # noqa: E402

COMPILER = Path(os.environ.get("COAX_TEST_COMPILER", ROOT / "coax/bin/coax"))
SOURCE = ROOT / "coax/lib/bitcoin/segwit.coax"
UPSTREAM = ROOT / "coax/tests/vectors/upstream"
PINS = {
    "sipa-bech32/segwit_addr.py.txt": "2884ce04a36c8374c4249177cd90e91e545ac73b5c9260307bbeb74ae2e9de0f",
    "bip350/bip-0350.mediawiki": "63634b06aa8bae88b31929674736e74964c4598684269ac2b0b140c43a7a0dec",
    "bip173/bip-0173.mediawiki": "b19eab06b264bbd15ad7f7d640fa34de8f4a8809fba8a6ecc3e165545b27a543",
    "bitcoin-core/key_io_valid.json": "90bd1d35d12763e0d00c5400b2c9fe551e532a821e1e466c20cad3aced70a7fe",
    "bitcoin-core/key_io_invalid.json": "c3ca74ddad7c01faaca7c26063537feb73ff2be03eb7fc83a42a655d2979261d",
}


def read(name):
    raw = (UPSTREAM / name).read_bytes()
    assert hashlib.sha256(raw).hexdigest() == PINS[name], name
    return raw


def load_oracle():
    # Run the bytes whose hash was checked, and write no __pycache__ into
    # the pinned corpus.
    raw = read("sipa-bech32/segwit_addr.py.txt")
    module = types.ModuleType("segwit_addr")
    exec(compile(raw, str(UPSTREAM / "sipa-bech32/segwit_addr.py.txt"), "exec"), module.__dict__)
    return module


REF = load_oracle()

# ---- the chapter's codes and the classifier
TOO_LONG, MIXED_CASE, BAD_FORMAT, BAD_CHECKSUM, UNKNOWN_HRP, NO_VERSION, BAD_VERSION, WRONG_VARIANT, \
    BAD_PADDING, BAD_LENGTH, BAD_V0_LENGTH, CAPACITY = range(-1, -13, -1)
BECH32, BECH32M = 1, 2
HRPS = {1: "bc", 2: "tb", 3: "bcrt"}


def variant(text):
    """1 or 2 for Bech32 or Bech32m, or the refusal, in the chapter's order."""
    s = text.decode("latin-1")
    if len(s) > 90: return TOO_LONG
    if re.search("[A-Z]", s) and re.search("[a-z]", s): return MIXED_CASE
    low = s.lower()
    pos = low.rfind("1")
    if len(s) < 8 or any(ord(c) < 33 or ord(c) > 126 for c in s) or pos < 1 or len(s) - pos < 7 \
            or any(c not in REF.CHARSET for c in low[pos + 1:]):
        return BAD_FORMAT
    residue = REF.bech32_polymod(REF.bech32_hrp_expand(low[:pos]) + [REF.CHARSET.find(c) for c in low[pos + 1:]])
    return {1: BECH32, REF.BECH32M_CONST: BECH32M}.get(residue, BAD_CHECKSUM)


def decode(text, capacity=40):
    """(length or refusal, network, version, program) in the chapter's order."""
    v = variant(text)
    if v < 0: return v, 0, 0, b""
    low = text.decode("latin-1").lower()
    pos = low.rfind("1")
    network = {h: n for n, h in HRPS.items()}.get(low[:pos])
    if network is None: return UNKNOWN_HRP, 0, 0, b""
    data = [REF.CHARSET.find(c) for c in low[pos + 1:-6]]
    if not data: return NO_VERSION, 0, 0, b""
    if data[0] > 16: return BAD_VERSION, 0, 0, b""
    if (data[0] == 0) != (v == BECH32): return WRONG_VARIANT, 0, 0, b""
    program = REF.convertbits(data[1:], 5, 8, False)
    if program is None: return BAD_PADDING, 0, 0, b""
    rule = program_rule(data[0], len(program))
    if rule: return rule, 0, 0, b""
    if capacity < len(program): return CAPACITY, 0, 0, b""
    return len(program), network, data[0], bytes(program)


def program_rule(version, length):
    if not 0 <= version <= 16: return BAD_VERSION
    if not 2 <= length <= 40: return BAD_LENGTH
    if version == 0 and length not in (20, 32): return BAD_V0_LENGTH
    return 0


def encode(network, version, program, capacity=90):
    if network not in HRPS: return UNKNOWN_HRP, b""
    rule = program_rule(version, len(program))
    if rule: return rule, b""
    text = REF.encode(HRPS[network], version, list(program)).encode()
    if capacity < len(text): return CAPACITY, b""
    return len(text), text


# ---- agreement between the classifier and the oracle
def oracle_variant(text):
    hrp, _data, spec = REF.bech32_decode(text.decode("latin-1"))
    return None if hrp is None else {REF.Encoding.BECH32: BECH32, REF.Encoding.BECH32M: BECH32M}[spec]


def oracle_decode(text):
    found = [(n, REF.decode(h, text.decode("latin-1"))) for n, h in HRPS.items()]
    found = [(n, v, bytes(p)) for n, (v, p) in found if v is not None]
    assert len(found) <= 1
    return found[0] if found else None


def agree(case):
    """The classifier's result must be the oracle's whenever the oracle has one."""
    op = case["op"]
    if op == "variant":
        want = oracle_variant(bytes.fromhex(case["input"]))
        assert (case["result"] if case["result"] > 0 else None) == want, case
    elif op == "decode":
        want = oracle_decode(bytes.fromhex(case["input"]))
        got = case["result"]
        if want is None:
            assert got < 0 and got != CAPACITY, case
        elif got != CAPACITY:
            assert (case["network"], case["version"], bytes.fromhex(case["output"])) == want, case
    else:
        if case["network"] in HRPS and 0 <= case["version"] <= 31:
            want = REF.encode(HRPS[case["network"]], case["version"], list(bytes.fromhex(case["input"])))
            if case["result"] == CAPACITY: assert want is not None
            else: assert (want.encode().hex() if want else None) == (case["output"] if case["result"] > 0 else None), case


def case(op, data, **kw):
    if op == "variant":
        return dict(op=op, input=data.hex(), result=variant(data), output="", network=0, version=0, capacity=0)
    if op == "decode":
        cap = kw.get("capacity", 40)
        n, network, version, program = decode(data, cap)
        return dict(op=op, input=data.hex(), result=n, output=program.hex(), network=network, version=version, capacity=cap)
    cap = kw.get("capacity", 90)
    n, text = encode(kw["network"], kw["version"], data, cap)
    return dict(op=op, input=data.hex(), result=n, output=text.hex(), network=kw["network"], version=kw["version"], capacity=cap)


def vector_texts():
    texts = []
    for name in ("bip350/bip-0350.mediawiki", "bip173/bip-0173.mediawiki"):
        body = read(name).decode()
        tests = body[body.index("==Test vectors"):]
        texts += [t.encode() for t in re.findall(r"^\* <tt>([^<]+)</tt>", tests, re.M)]
        texts += [bytes.fromhex(h) + t.encode() for h, t in re.findall(r"^\* 0x([0-9A-F]{2}) \+ <tt>([^<]+)</tt>", tests, re.M)]
    texts.append(b"de1lg7wt\xff")
    for t, _spk, _meta in json.loads(read("bitcoin-core/key_io_valid.json")):
        texts.append(t.encode())
    for (t,) in json.loads(read("bitcoin-core/key_io_invalid.json")):
        texts.append(t.encode())
    return texts


def other_variant(text):
    """The same HRP and data with the checksum of the other constant."""
    s = text.decode().lower()
    hrp, data, spec = REF.bech32_decode(s)
    other = REF.Encoding.BECH32M if spec == REF.Encoding.BECH32 else REF.Encoding.BECH32
    return REF.bech32_encode(hrp, data, other).encode()


def cases():
    out = []
    texts = vector_texts()
    rng = random.Random(28)
    valid = []
    for version in range(18):
        for length in range(42):
            for network in (1, 2, 3):
                program = rng.randbytes(length)
                out.append(case("encode", program, network=network, version=version))
                n, text = encode(network, version, program)
                if n > 0:
                    valid.append(text)
                    out.append(case("encode", program, network=network, version=version, capacity=n - 1))
    for network in (0, 4, -1):
        out.append(case("encode", rng.randbytes(32), network=network, version=1))
    for version in (-1, 31, 32):
        out.append(case("encode", rng.randbytes(32), network=1, version=version))
    texts += valid
    texts += [other_variant(t) for t in valid]
    alphabet = REF.CHARSET + REF.CHARSET.upper() + "1bio0 \x7f"
    for t in list(texts):
        if not t: continue
        s = bytearray(t)
        at = rng.randrange(len(s))
        kind = rng.randrange(5)
        if kind == 0: s[at] = ord(rng.choice(alphabet))
        elif kind == 1: s.insert(at, ord(rng.choice(alphabet)))
        elif kind == 2: del s[at]
        elif kind == 3: s[at] = s[at] ^ 0x20 if chr(s[at]).isalpha() else s[at]
        elif at + 1 < len(s): s[at], s[at + 1] = s[at + 1], s[at]
        texts.append(bytes(s))
    # The weakness: 'q's before a final 'p' of Bech32 and Bech32m strings.
    ends_in_p = [t for t in valid + [other_variant(t) for t in valid] if t.endswith(b"p")]
    for t in ends_in_p[:200]:
        for k in (1, 2, 3, 5, 8):
            texts.append(t[:-1] + b"q" * k + b"p")
    texts += [b"q" * 91, b"bc1" + b"q" * 88, b"bc1" + b"q" * 87, b"a" * 90, b""]
    for t in texts:
        out += [case("variant", t), case("decode", t)]
        n = decode(t)[0]
        if n > 0: out.append(case("decode", t, capacity=n - 1))
    for c in out: agree(c)
    return out


# ---- the native builds
class View(C.Structure):
    _fields_ = [("data", C.POINTER(C.c_uint8)), ("count", C.c_int64)]


class Address(C.Structure):
    _fields_ = [("length", C.c_int64), ("network", C.c_int64), ("version", C.c_int64)]


def view(buffer, n):
    return View(C.cast(buffer, C.POINTER(C.c_uint8)), n)


def run(*args, **kw):
    r = subprocess.run([str(a) for a in args], capture_output=True, text=True, **kw)
    if r.returncode:
        raise RuntimeError(f"{args[0]} failed ({r.returncode}): {r.stderr or r.stdout}")
    return r


def native(lib, c):
    """Runs one case through a native build; returns (result, network, version, output hex, untouched tail)."""
    data = bytes.fromhex(c["input"])
    inp = (C.c_uint8 * max(1, len(data))).from_buffer_copy(data.ljust(1, b"\0"))
    out = (C.c_uint8 * 128)(*([0xA5] * 128))
    if c["op"] == "variant":
        return lib.segwit_variant_vartime(view(inp, len(data))), 0, 0, "", True
    if c["op"] == "decode":
        a = lib.segwit_decode_vartime(view(inp, len(data)), view(out, c["capacity"]))
        n, network, version = a.length, a.network, a.version
    else:
        n = lib.segwit_encode_vartime(c["network"], c["version"], view(inp, len(data)), view(out, c["capacity"]))
        network, version = c["network"], c["version"]
    written = max(0, n)
    return n, network, version, bytes(out[:written]).hex(), all(b == 0xA5 for b in out[written:])


class Segwit(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix="coax-segwit-")
        cls.dir = Path(cls.temp.name)
        cls.libs = []
        for label, mode in [("checked", []), ("release", ["--release"])]:
            c = cls.dir / (label + ".c")
            c.write_text(run(COMPILER, "c", SOURCE, "--library", "--header", cls.dir / "segwit.h", *mode).stdout)
            run("cc", "-std=c11", "-O2", "-shared", "-fPIC", c, "-lm", "-o", cls.dir / (label + ".dylib"))
            lib = C.CDLL(str(cls.dir / (label + ".dylib")))
            lib.segwit_variant_vartime.argtypes = [View]
            lib.segwit_variant_vartime.restype = C.c_int64
            lib.segwit_decode_vartime.argtypes = [View, View]
            lib.segwit_decode_vartime.restype = Address
            lib.segwit_encode_vartime.argtypes = [C.c_int64, C.c_int64, View, View]
            lib.segwit_encode_vartime.restype = C.c_int64
            cls.libs.append((label, lib))
        cls.cases = cases()

    @classmethod
    def tearDownClass(cls):
        cls.temp.cleanup()

    def test_reference_on_published_vectors(self):
        """The oracle and the classifier give BIP-350's answers."""
        body = read("bip350/bip-0350.mediawiki").decode()
        for t in re.findall(r"^\* <tt>([^<]+)</tt>$", body[body.index("===Test vectors for Bech32m"):body.index("The following string are not")], re.M):
            self.assertEqual(variant(t.encode()), BECH32M, t)
        for t, spk in re.findall(r"^\* <tt>([^<]+)</tt>: <tt>([0-9a-f]+)</tt>$", body, re.M):
            n, _network, version, program = decode(t.encode())
            self.assertEqual(bytes([0x50 + version if version else 0, n]) + program, bytes.fromhex(spk), t)
        for t, reason in re.findall(r"^\* <tt>([^<]+)</tt>: (.+)$", body[body.index("The following list gives invalid segwit"):], re.M):
            self.assertLess(decode(t.encode())[0], 0, t)
            self.assertIsNone(oracle_decode(t.encode()), t)

    def test_native(self):
        for label, lib in self.libs:
            for c in self.cases:
                want = (c["result"], c["network"], c["version"], c["output"], True)
                self.assertEqual(native(lib, c), want, f"{label} {c}")
        results = {c["result"] for c in self.cases}
        self.assertTrue(set(range(-12, 0)) <= results, sorted(results))

    def test_wasm(self):
        clang, sysroot, resources = Path(WASM_CLANG), Path(WASM_SYSROOT), Path(WASM_RESOURCES)
        self.assertTrue(clang.exists() and sysroot.exists() and resources.exists(), "a Wasm toolchain is required; see coax/web/wasm.sh")
        fixture = self.dir / "wasm.json"
        fixture.write_text(json.dumps(self.cases))
        for label in ["checked", "release"]:
            module = self.dir / (label + ".wasm")
            run(clang, "--target=wasm32-wasip1", "--sysroot=" + str(sysroot), "-resource-dir=" + str(resources),
                "-std=c11", "-O2", "-I" + str(self.dir), "-mexec-model=reactor",
                self.dir / (label + ".c"), HERE / "segwit_wasm.c",
                "-Wl,--import-memory,--export-memory,--export=__heap_base,--max-memory=67108864,-z,stack-size=1048576,--gc-sections",
                "-o", module)
            result = json.loads(run("node", HERE / "segwit_wasm.cjs", module, fixture).stdout)
            self.assertEqual((result["agree"], result["cases"]), (len(self.cases), len(self.cases)), label)
        print(f"\n{len(self.cases)} cases agree with segwit_addr.py, native checked/release and Wasm checked/release",
              file=sys.stderr)


if __name__ == "__main__":
    unittest.main()
