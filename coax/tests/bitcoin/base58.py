#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Python integers and hashlib decide each case outside Coax)
"""Base58, Base58Check and WIF against a separate Python implementation, natively and in Wasm.

Bitcoin From Scratch, chapter 26. Compiles coax/lib/bitcoin/base58.coax to
C in checked and release builds and runs every procedure it exports on:

- every pair in Bitcoin Core's base58_encode_decode.json, every Base58 entry
  of key_io_valid.json, every string of key_io_invalid.json and every BIP-32
  extended public key, all pinned in coax/tests/vectors/upstream/;
- seeded random byte strings with leading zeros, random keys in WIF, and
  seeded edits of valid strings: a character replaced, inserted or removed,
  including characters outside the alphabet.

Each result (the count written, or the refusal code) and each output must
equal those of the Python reference below, which uses Python integers and
hashlib and shares no code with Coax. The WIF procedures must also leave
their work buffer zero. Then it builds the same C for wasm32 and runs every
case again in Node. Run from the repository root:

    python3 coax/tests/bitcoin/base58.py
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
sys.path.insert(0, str(ROOT / "coax/tests/nostr"))
from wasm_toolchain import WASM_CLANG, WASM_SYSROOT, WASM_RESOURCES  # noqa: E402

COMPILER = Path(os.environ.get("COAX_TEST_COMPILER", ROOT / "coax/bin/coax"))
SOURCE = ROOT / "coax/lib/bitcoin/base58.coax"
UPSTREAM = ROOT / "coax/tests/vectors/upstream"
PINS = {
    "bitcoin-core/base58_encode_decode.json": "20d51011f49339714c28b9244cc5238f4c78bb9206dc8fc61500aed6fc2682ca",
    "bitcoin-core/key_io_valid.json": "90bd1d35d12763e0d00c5400b2c9fe551e532a821e1e466c20cad3aced70a7fe",
    "bitcoin-core/key_io_invalid.json": "c3ca74ddad7c01faaca7c26063537feb73ff2be03eb7fc83a42a655d2979261d",
    "bip32/bip-0032.mediawiki": "e5e00a8289db2f681052cf24a745320afc225e66b25d1e489a7c884d2fc7f11f",
}

# ---- the Python reference
ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
BAD_CHAR, BAD_CHECKSUM, BAD_LENGTH, UNKNOWN_VERSION, BAD_FLAG, CAPACITY, TOO_LONG = range(-1, -8, -1)
MAX = 1024
VERSIONS = {0x00: 1, 0x05: 2, 0x6F: 3, 0xC4: 4}


def checksum(payload):
    return hashlib.sha256(hashlib.sha256(payload).digest()).digest()[:4]


def encode(data):
    if len(data) > MAX: return TOO_LONG, b""
    n, text = int.from_bytes(data, "big"), ""
    while n:
        n, r = divmod(n, 58)
        text = ALPHABET[r] + text
    text = "1" * (len(data) - len(data.lstrip(b"\0"))) + text
    return len(text), text.encode()


def decode(text):
    ones = len(text) - len(text.lstrip(b"1"))
    n = 0
    for c in text[ones:]:
        if chr(c) not in ALPHABET: return BAD_CHAR, b""
        n = n * 58 + ALPHABET.index(chr(c))
        if (n.bit_length() + 7) // 8 > MAX: return TOO_LONG, b""
    data = b"\0" * ones + n.to_bytes((n.bit_length() + 7) // 8, "big")
    return (TOO_LONG, b"") if len(data) > MAX else (len(data), data)


def check_encode(payload):
    if len(payload) > MAX - 4: return TOO_LONG, b""
    return encode(payload + checksum(payload))


def check_decode(text):
    n, data = decode(text)
    if n < 0: return n, b""
    if n < 4: return BAD_LENGTH, b""
    if checksum(data[:-4]) != data[-4:]: return BAD_CHECKSUM, b""
    return n - 4, data[:-4]


def address_decode(text):
    n, payload = check_decode(text)
    if n < 0: return n, b""
    kind = VERSIONS.get(payload[0] if payload else -1)
    if kind is None: return UNKNOWN_VERSION, b""
    if n != 21: return BAD_LENGTH, b""
    return kind, payload[1:]


def wif_encode(key, compressed, testnet):
    payload = bytes([0xEF if testnet else 0x80]) + key + (b"\1" if compressed else b"")
    return encode(payload + checksum(payload))


def wif_decode(text):
    if len(text) not in (51, 52): return BAD_LENGTH, b""
    if any(chr(c) not in ALPHABET for c in text): return BAD_CHAR, b""
    size, n = len(text) - 14, 0
    for c in text: n = n * 58 + ALPHABET.index(chr(c))
    if n >> (8 * size): return BAD_LENGTH, b""
    data = n.to_bytes(size, "big")
    if checksum(data[:-4]) != data[-4:]: return BAD_CHECKSUM, b""
    if data[0] not in (0x80, 0xEF): return UNKNOWN_VERSION, b""
    if size == 38 and data[33] != 1: return BAD_FLAG, b""
    return 5 + (size - 37) + (2 if data[0] == 0xEF else 0), data[1:33]


def case(op, data, **flags):
    """One case: the operation, its input and the reference's result."""
    if op == "wif_encode": result, output = wif_encode(data, flags["compressed"], flags["testnet"])
    else: result, output = REFERENCE[op](data)
    return dict(op=op, input=data.hex(), result=result, output=output.hex(), **flags)


REFERENCE = dict(encode=encode, decode=decode, check_encode=check_encode, check_decode=check_decode,
                 address_decode=address_decode, wif_decode=wif_decode)


def read(name):
    raw = (UPSTREAM / name).read_bytes()
    assert hashlib.sha256(raw).hexdigest() == PINS[name], name
    return raw


def cases():
    out = []
    texts = []
    for h, t in json.loads(read("bitcoin-core/base58_encode_decode.json")):
        out += [case("encode", bytes.fromhex(h)), case("decode", t.encode())]
        texts.append(t.encode())
    for t, _data, meta in json.loads(read("bitcoin-core/key_io_valid.json")):
        texts.append(t.encode())
    for (t,) in json.loads(read("bitcoin-core/key_io_invalid.json")):
        texts.append(t.encode())
    for line in read("bip32/bip-0032.mediawiki").decode().splitlines():
        if line.startswith("** ext pub: "):
            texts.append(line[12:].encode())
    rng = random.Random(26)
    # The limits: 1,024 bytes in or out, at the edge and one past it.
    for data in [rng.randbytes(1024), rng.randbytes(1025), b"\0" * 1024, b"\0" * 1025, rng.randbytes(1020), rng.randbytes(1021)]:
        out += [case("encode", data), case("check_encode", data)]
    texts += [b"1" * 1024, b"1" * 1025, b"2" * 1398, b"2" * 1399, b"z" * 1399, b"1" * 1000 + b"z" * 34,
              b"1" * 1000 + b"z" * 35, b"z" * 2000 + b"0"]
    for i in range(300):
        data = b"\0" * rng.randrange(4) + rng.randbytes(rng.randrange(80))
        out += [case("encode", data), case("check_encode", data)]
        texts.append(encode(data)[1]); texts.append(check_encode(data)[1])
    for i in range(200):
        key = rng.randbytes(32) if i else b"\0" * 31 + b"\1"
        compressed, testnet = bool(i % 2), bool(i // 2 % 2)
        out.append(case("wif_encode", key, compressed=compressed, testnet=testnet))
        texts.append(wif_encode(key, compressed, testnet)[1])
        payload = bytes([0x80]) + key + bytes([rng.randrange(256)])            # flag anything
        texts.append(encode(payload + checksum(payload))[1])
        payload = bytes([rng.randrange(256)]) + key                             # any version
        texts.append(encode(payload + checksum(payload))[1])
    edits = list(ALPHABET) + list("0OIl +-_/\x00\xff")
    for t in list(texts):
        s = bytearray(t)
        if s:
            at = rng.randrange(len(s))
            kind = rng.randrange(3)
            if kind == 0: s[at] = ord(rng.choice(edits)) & 0xFF
            elif kind == 1: s.insert(at, ord(rng.choice(edits)) & 0xFF)
            else: del s[at]
        texts.append(bytes(s))
    for t in texts:
        out += [case(op, t) for op in ("decode", "check_decode", "address_decode", "wif_decode")]
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


def native(lib, c):
    """Runs one case through a native build; returns (result, output hex, work wiped)."""
    data = bytes.fromhex(c["input"])
    inp = (C.c_uint8 * max(1, len(data))).from_buffer_copy(data.ljust(1, b"\0"))
    out = (C.c_uint8 * 2048)(*([0xA5] * 2048))
    work = (C.c_uint8 * 42)(*([0x5A] * 42))
    op, i, o, w = c["op"], view(inp, len(data)), view(out, 2048), view(work, 42)
    if op == "address_decode": r = lib.base58_address_decode_vartime(i, view(out, 20)); n = 20 if r > 0 else 0
    elif op == "wif_encode": r = lib.wif_encode_ct(view(inp, 32), c["compressed"], c["testnet"], w, o); n = r
    elif op == "wif_decode": r = lib.wif_decode_ct(i, w, view(out, 32)); n = 32 if r > 0 else 0
    else:
        r = getattr(lib, dict(encode="base58_encode_vartime", decode="base58_decode_vartime",
                              check_encode="base58check_encode_vartime", check_decode="base58check_decode_vartime")[op])(i, o)
        n = r
    if op == "wif_decode" and len(data) in (51, 52): wiped = all(b == 0 for b in work[:len(data) - 10])
    elif op == "wif_encode": wiped = all(b == 0 for b in work[:38 if c["compressed"] else 37])
    else: wiped = True
    return r, bytes(out[:max(0, n)]).hex(), wiped


class Base58(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix="coax-base58-")
        cls.dir = Path(cls.temp.name)
        cls.libs = []
        for label, mode in [("checked", []), ("release", ["--release"])]:
            c = cls.dir / (label + ".c")
            c.write_text(run(COMPILER, "c", SOURCE, "--library", "--header", cls.dir / "base58.h", *mode).stdout)
            run("cc", "-std=c11", "-O2", "-shared", "-fPIC", c, "-lm", "-o", cls.dir / (label + ".dylib"))
            lib = C.CDLL(str(cls.dir / (label + ".dylib")))
            for name in ["base58_encode_vartime", "base58_decode_vartime", "base58check_encode_vartime",
                         "base58check_decode_vartime", "base58_address_decode_vartime"]:
                getattr(lib, name).argtypes = [View, View]
            lib.wif_encode_ct.argtypes = [View, C.c_bool, C.c_bool, View, View]
            lib.wif_decode_ct.argtypes = [View, View, View]
            for name in ["base58_encode_vartime", "base58_decode_vartime", "base58check_encode_vartime",
                         "base58check_decode_vartime", "base58_address_decode_vartime", "wif_encode_ct", "wif_decode_ct"]:
                getattr(lib, name).restype = C.c_int64
            cls.libs.append((label, lib))
        cls.cases = cases()

    @classmethod
    def tearDownClass(cls):
        cls.temp.cleanup()

    def test_reference_on_published_vectors(self):
        """The Python reference itself gives the published answers."""
        for h, t in json.loads(read("bitcoin-core/base58_encode_decode.json")):
            self.assertEqual(encode(bytes.fromhex(h))[1], t.encode())
        for t, _data, meta in json.loads(read("bitcoin-core/key_io_valid.json")):
            if meta["isPrivkey"]: self.assertGreater(wif_decode(t.encode())[0], 4, t)
            elif not t.lower().startswith(("bc1", "tb1", "bcrt1")): self.assertGreater(address_decode(t.encode())[0], 0, t)
        for (t,) in json.loads(read("bitcoin-core/key_io_invalid.json")):
            self.assertLess(address_decode(t.encode())[0], 0, t)
            self.assertLess(wif_decode(t.encode())[0], 0, t)

    def test_native(self):
        for label, lib in self.libs:
            for c in self.cases:
                self.assertEqual(native(lib, c), (c["result"], c["output"], True), f"{label} {c['op']} {c['input']}")
        results = {c["result"] for c in self.cases}
        self.assertTrue({BAD_CHAR, BAD_CHECKSUM, BAD_LENGTH, UNKNOWN_VERSION, BAD_FLAG, TOO_LONG} <= results)

    def test_wasm(self):
        clang, sysroot, resources = Path(WASM_CLANG), Path(WASM_SYSROOT), Path(WASM_RESOURCES)
        self.assertTrue(clang.exists() and sysroot.exists() and resources.exists(), "a Wasm toolchain is required; see coax/web/wasm.sh")
        fixture = self.dir / "wasm.json"
        fixture.write_text(json.dumps(self.cases))
        for label in ["checked", "release"]:
            module = self.dir / (label + ".wasm")
            run(clang, "--target=wasm32-wasip1", "--sysroot=" + str(sysroot), "-resource-dir=" + str(resources),
                "-std=c11", "-O2", "-I" + str(self.dir), "-mexec-model=reactor",
                self.dir / (label + ".c"), HERE / "base58_wasm.c",
                "-Wl,--import-memory,--export-memory,--export=__heap_base,--max-memory=67108864,-z,stack-size=1048576,--gc-sections",
                "-o", module)
            result = json.loads(run("node", HERE / "base58_wasm.cjs", module, fixture).stdout)
            self.assertEqual((result["agree"], result["cases"]), (len(self.cases), len(self.cases)), label)
        print(f"\n{len(self.cases)} cases agree with Python, native checked/release and Wasm checked/release",
              file=sys.stderr)


if __name__ == "__main__":
    unittest.main()
