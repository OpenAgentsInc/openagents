#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Python integers, hashlib and BIP-350's reference decide each case outside Coax)
"""Addresses against a separate Python implementation, natively and in Wasm.

Bitcoin From Scratch, chapter 29. Compiles coax/lib/bitcoin/address.coax to
C in checked and release builds and runs every procedure it exports on:

- every address of Bitcoin Core's key_io_valid.json and every string of
  key_io_invalid.json, on all three networks; every string of BIP-350's and
  BIP-173's test-vector sections; BIP-86's key-path outputs, BIP-341's case
  with no script tree, BIP-84's keys and BIP-49's key, all pinned in
  coax/tests/vectors/upstream/;
- seeded public keys (compressed and uncompressed), x-only internal keys
  (on the curve and not), corrupted keys, redeem and witness scripts at the
  520-byte and 10,000-byte limits, and scriptPubKeys of every template with
  each byte changed;
- every address produced, with seeded edits: a character replaced,
  inserted or removed, or its case flipped;
- output buffers one byte or character too small.

The reference below shares no code with Coax. It uses hashlib for SHA-256,
RIPEMD-160 and the tagged hash, Python integers for secp256k1 (points.py,
chapter 17's test reference), base58.py's reference (chapter 26) and
segwit.py's classifier (chapter 28) for the two encodings, and BIP-350's
reference implementation, segwit_addr.py, for Bech32 and Bech32m. Each
result, kind and output must match, and nothing may be written past the
output. Then it builds the same C for wasm32 and runs every case again in
Node. Run from the repository root:

    python3 coax/tests/bitcoin/address.py
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
import unittest

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(ROOT / "coax/tests/nostr"))
sys.path.insert(0, str(HERE))
from wasm_toolchain import WASM_CLANG, WASM_SYSROOT, WASM_RESOURCES  # noqa: E402
import base58 as B58  # noqa: E402
import points as EC  # noqa: E402
import segwit as SW  # noqa: E402

COMPILER = Path(os.environ.get("COAX_TEST_COMPILER", ROOT / "coax/bin/coax"))
SOURCE = ROOT / "coax/lib/bitcoin/address.coax"
UPSTREAM = ROOT / "coax/tests/vectors/upstream"
PINS = {
    "bitcoin-core/key_io_valid.json": "90bd1d35d12763e0d00c5400b2c9fe551e532a821e1e466c20cad3aced70a7fe",
    "bitcoin-core/key_io_invalid.json": "c3ca74ddad7c01faaca7c26063537feb73ff2be03eb7fc83a42a655d2979261d",
    "bip86/bip-0086.mediawiki": "d8d01dee331da07c2562615bc1f064c1868ec3fce61184973da76d1196c7f5b0",
    "bip341/wallet-test-vectors.json": "403e19fb81dd1f31e745699216308f61fb403774b2aafa87b631b8f7c042d37f",
    "bip84/bip-0084.mediawiki": "1900feec6cafca65b8c09906ca0658d2d742b4c9b44cb15678996985b6bfe627",
    "bip49/bip-0049.mediawiki": "307b62faf7b9927233b5112e93d591272ba0c48303b5dbb6f0bbfbaf2347e695",
}


def read(name):
    raw = (UPSTREAM / name).read_bytes()
    assert hashlib.sha256(raw).hexdigest() == PINS[name], name
    return raw


# ---- the Python reference
P2PKH, P2SH, P2WPKH, P2WSH, P2TR, WITNESS = range(1, 7)
BAD_NETWORK, WRONG_NETWORK, NO_ADDRESS, BAD_KEY, UNCOMPRESSED, SCRIPT_TOO_LONG, BAD_TWEAK, CAPACITY = range(-1, -9, -1)
BASE58, SEGWIT = -20, -40
MAINNET, TESTNET, REGTEST = 1, 2, 3
CHAINS = {"main": MAINNET, "testnet4": TESTNET, "signet": TESTNET, "regtest": REGTEST}
BUILDERS = ["p2pkh", "p2wpkh", "p2sh_p2wpkh", "p2sh", "p2wsh", "p2tr"]


def hash160(b):
    return hashlib.new("ripemd160", hashlib.sha256(b).digest()).digest()


def tagged(tag, msg):
    t = hashlib.sha256(tag.encode()).digest()
    return hashlib.sha256(t + t + msg).digest()


def is_key(b):
    return EC.decode(bytes(b)) is not None


def witness(version, program):
    return bytes([0x50 + version if version else 0, len(program)]) + program


def tweak(internal):
    """BIP-341's taproot_tweak_pubkey with an empty script tree: (parity, Q's x) or a refusal."""
    if len(internal) != 32: return BAD_KEY, b""
    p = EC.lift(int.from_bytes(internal, "big"))
    if p is None: return BAD_KEY, b""
    t = int.from_bytes(tagged("TapTweak", internal), "big")
    if t >= EC.N: return BAD_TWEAK, b""
    q = EC.add(p, EC.mul(t))
    if q is None: return BAD_TWEAK, b""
    return q[1] & 1, q[0].to_bytes(32, "big")


def build(which, data):
    """A scriptPubKey from what it commits to, or a refusal."""
    if which == "p2pkh":
        return (BAD_KEY, b"") if not is_key(data) else (25, b"\x76\xa9\x14" + hash160(data) + b"\x88\xac")
    if which in ("p2wpkh", "p2sh_p2wpkh"):
        if not is_key(data): return BAD_KEY, b""
        if len(data) != 33: return UNCOMPRESSED, b""
        s = witness(0, hash160(data))
        return (22, s) if which == "p2wpkh" else (23, b"\xa9\x14" + hash160(s) + b"\x87")
    if which == "p2sh":
        return (SCRIPT_TOO_LONG, b"") if len(data) > 520 else (23, b"\xa9\x14" + hash160(data) + b"\x87")
    if which == "p2wsh":
        return (SCRIPT_TOO_LONG, b"") if len(data) > 10000 else (34, witness(0, hashlib.sha256(data).digest()))
    parity, q = tweak(data)
    return (parity, b"") if parity < 0 else (34, witness(1, q))


def kind(s):
    n = len(s)
    if n == 25 and s[:3] == b"\x76\xa9\x14" and s[23:] == b"\x88\xac": return P2PKH
    if n == 23 and s[:2] == b"\xa9\x14" and s[22] == 0x87: return P2SH
    if not (4 <= n <= 42 and (s[0] == 0 or 0x51 <= s[0] <= 0x60) and s[1] == n - 2): return NO_ADDRESS
    if s[0] == 0: return {22: P2WPKH, 34: P2WSH}.get(n, NO_ADDRESS)
    return P2TR if s[0] == 0x51 and n == 34 else WITNESS


def encode(network, s):
    if network not in (1, 2, 3): return BAD_NETWORK, b""
    k = kind(s)
    if k < 0: return k, b""
    if k in (P2PKH, P2SH):
        version = {(P2PKH, True): 0x00, (P2SH, True): 0x05, (P2PKH, False): 0x6F, (P2SH, False): 0xC4}[k, network == MAINNET]
        payload = bytes([version]) + (s[3:23] if k == P2PKH else s[2:22])
        return B58.encode(payload + B58.checksum(payload))
    text = SW.REF.encode(SW.HRPS[network], 0 if s[0] == 0 else s[0] - 0x50, list(s[2:]))
    return len(text), text.encode()


def is_segwit(text):
    s = text.decode("latin-1")
    at = s.rfind("1", 1)
    return at > 0 and s[:at].lower() in ("bc", "tb", "bcrt")


def decode(network, text):
    """(script length or refusal, kind, script)."""
    if network not in (1, 2, 3): return BAD_NETWORK, 0, b""
    if is_segwit(text):
        n, net, version, program = SW.decode(text)
        if n < 0: return SEGWIT + n, 0, b""
        if net != network: return WRONG_NETWORK, 0, b""
        s = witness(version, program)
    else:
        k, h = B58.address_decode(text)
        if k < 0: return BASE58 + k, 0, b""
        if (k in (1, 2)) != (network == MAINNET): return WRONG_NETWORK, 0, b""
        s = b"\x76\xa9\x14" + h + b"\x88\xac" if k in (1, 3) else b"\xa9\x14" + h + b"\x87"
    return len(s), kind(s), s


def case(op, data, network=0, which="", capacity=None):
    c = dict(op=op, input=data.hex(), network=network, which=BUILDERS.index(which) + 1 if which else 0, kind=0)
    if op == "build":
        n, out = build(which, data)
    elif op == "tweak":
        n, out = tweak(data)
    elif op == "kind":
        n, out = kind(data), b""
    elif op == "encode":
        n, out = encode(network, data)
    else:
        n, c["kind"], out = decode(network, data)
    full = {"build": 42, "tweak": 32, "kind": 0, "encode": 90, "decode": 42}[op]
    c["capacity"] = full if capacity is None else capacity
    # The tweak and P2TR check the buffer before the key; the others after.
    first = op == "tweak" or which == "p2tr"
    if op != "kind" and (c["capacity"] < len(out) or first and c["capacity"] < full - 8 * (op == "build")):
        n, out, c["kind"] = CAPACITY, b"", 0
    c["result"], c["output"] = n, out.hex()
    return c


def vectors():
    """Published keys, scripts and addresses, with the check that the reference gives their answers."""
    keys, internals, texts = [], [], []
    for t, spk, meta in json.loads(read("bitcoin-core/key_io_valid.json")):
        if meta["isPrivkey"]: continue
        net = CHAINS[meta["chain"]]
        assert decode(net, t.encode())[2] == bytes.fromhex(spk) and encode(net, bytes.fromhex(spk))[1] == t.encode(), t
        texts.append(t.encode())
        if meta.get("tryCaseFlip"): texts.append(t.swapcase().encode())
    for (t,) in json.loads(read("bitcoin-core/key_io_invalid.json")):
        assert all(decode(n, t.encode())[0] < 0 for n in (1, 2, 3)), t
        texts.append(t.encode())
    b86 = read("bip86/bip-0086.mediawiki").decode()
    for ik, ok, spk, addr in re.findall(r"internal_key = (\w+)\noutput_key   = (\w+)\nscriptPubKey = (\w+)\naddress      = (\w+)", b86):
        assert build("p2tr", bytes.fromhex(ik)) == (34, bytes.fromhex(spk)) and spk[4:] == ok, ik
        assert encode(MAINNET, bytes.fromhex(spk))[1] == addr.encode()
        internals.append(bytes.fromhex(ik)); texts.append(addr.encode())
    for s in json.loads(read("bip341/wallet-test-vectors.json"))["scriptPubKey"]:
        ik = bytes.fromhex(s["given"]["internalPubkey"])
        internals.append(ik)
        if s["given"]["scriptTree"] is None:
            assert build("p2tr", ik)[1].hex() == s["expected"]["scriptPubKey"]
            assert tagged("TapTweak", ik).hex() == s["intermediary"]["tweak"]
        texts.append(s["expected"]["bip350Address"].encode())
    for key, addr in re.findall(r"pubkey  = (\w+)\n  address = (\w+)", read("bip84/bip-0084.mediawiki").decode()):
        assert encode(MAINNET, build("p2wpkh", bytes.fromhex(key))[1])[1] == addr.encode()
        keys.append(bytes.fromhex(key)); texts.append(addr.encode())
    b49 = read("bip49/bip-0049.mediawiki").decode()
    key = bytes.fromhex(re.search(r"account0recvPublicKeyHex = 0x(\w+)", b49).group(1))
    addr = re.search(r"address = base58check\(prefix \| addressBytes\) = (\w+)", b49).group(1)
    assert encode(TESTNET, build("p2sh_p2wpkh", key)[1])[1] == addr.encode()
    keys.append(key); texts.append(addr.encode())
    texts += SW.vector_texts()
    return keys, internals, texts


def cases():
    rng = random.Random(29)
    keys, internals, texts = vectors()
    g = EC.G
    keys += [EC.encode(g, 33), EC.encode(g, 65)]
    for _ in range(60):
        pt = EC.mul(rng.randrange(1, EC.N))
        keys += [EC.encode(pt, 33), EC.encode(pt, 65)]
        internals.append(EC.encode(pt, 32))
    for _ in range(40):
        internals.append(rng.randbytes(32))                                   # about half are not x coordinates
    bad = [b"", rng.randbytes(31), rng.randbytes(33), b"\x06" + EC.encode(g, 65)[1:], b"\x07" + EC.encode(g, 65)[1:],
           b"\x02" + (5).to_bytes(32, "big"), b"\x04" + EC.encode(g, 65)[1:33] + bytes(32), EC.encode(g, 32),
           b"\x02" + EC.P.to_bytes(32, "big"), b"\x05" + EC.encode(g, 33)[1:], b"\x04" + EC.encode(g, 33)[1:]]
    for k in list(keys[:20]):
        s = bytearray(k); s[rng.randrange(len(s))] ^= 1 << rng.randrange(8); bad.append(bytes(s))
    out, scripts = [], []
    for k in keys + bad:
        for which in ("p2pkh", "p2wpkh", "p2sh_p2wpkh"):
            c = case("build", k, which=which); out.append(c)
            if c["result"] > 0:
                scripts.append(bytes.fromhex(c["output"]))
                out.append(case("build", k, which=which, capacity=c["result"] - 1))
    for k in internals + bad + [EC.P.to_bytes(32, "big"), bytes(32), b"\xff" * 32]:
        out += [case("tweak", k), case("tweak", k, capacity=31), case("build", k, which="p2tr")]
        if out[-1]["result"] > 0:
            scripts.append(bytes.fromhex(out[-1]["output"]))
            out.append(case("build", k, which="p2tr", capacity=33))
    for n in [0, 1, 2, 22, 34, 100, 519, 520, 521, 600, 9999, 10000, 10001]:
        data = rng.randbytes(n)
        for which in ("p2sh", "p2wsh"):
            c = case("build", data, which=which); out.append(c)
            if c["result"] > 0:
                scripts.append(bytes.fromhex(c["output"]))
                out.append(case("build", data, which=which, capacity=c["result"] - 1))
    for version in range(17):
        for length in range(42):
            scripts.append(witness(version, rng.randbytes(length)))
    for s in list(scripts[:200]):
        for _ in range(3):
            t = bytearray(s); t[rng.randrange(len(t))] ^= 1 << rng.randrange(8); scripts.append(bytes(t))
        scripts += [s[:-1], s + b"\x00"]
    scripts += [b"", b"\x6a\x04\xde\xad\xbe\xef", b"\x21" + EC.encode(g, 33) + b"\xac", b"\x4f\x02\x75\x1e", b"\x61\x02\x75\x1e"]
    produced = []
    for i, s in enumerate(scripts):
        out.append(case("kind", s))
        for network in (0, 1, 2, 3, 4) if i < 50 else (1, 2, 3):
            c = case("encode", s, network=network); out.append(c)
            if c["result"] > 0:
                produced.append(bytes.fromhex(c["output"]))
                out.append(case("encode", s, network=network, capacity=c["result"] - 1))
    texts += produced
    alphabet = B58.ALPHABET + SW.REF.CHARSET + "10OIl \x7f"
    for t in list(texts):
        if not t: continue
        s = bytearray(t); at = rng.randrange(len(s)); kind_ = rng.randrange(4)
        if kind_ == 0: s[at] = ord(rng.choice(alphabet))
        elif kind_ == 1: s.insert(at, ord(rng.choice(alphabet)))
        elif kind_ == 2: del s[at]
        elif chr(s[at]).isalpha(): s[at] ^= 0x20
        texts.append(bytes(s))
    for i, t in enumerate(texts):
        for network in (0, 1, 2, 3, 4) if i < 50 else (1, 2, 3):
            c = case("decode", t, network=network); out.append(c)
            if c["result"] > 0: out.append(case("decode", t, network=network, capacity=c["result"] - 1))
    return out


# ---- the native builds
class View(C.Structure):
    _fields_ = [("data", C.POINTER(C.c_uint8)), ("count", C.c_int64)]


class Script(C.Structure):
    _fields_ = [("length", C.c_int64), ("kind", C.c_int64)]


def view(buffer, n):
    return View(C.cast(buffer, C.POINTER(C.c_uint8)), n)


def run(*args, **kw):
    r = subprocess.run([str(a) for a in args], capture_output=True, text=True, **kw)
    if r.returncode:
        raise RuntimeError(f"{args[0]} failed ({r.returncode}): {r.stderr or r.stdout}")
    return r


def native(lib, c):
    """Runs one case through a native build; returns (result, kind, output hex, untouched tail)."""
    data = bytes.fromhex(c["input"])
    inp = (C.c_uint8 * max(1, len(data))).from_buffer_copy(data.ljust(1, b"\0"))
    out = (C.c_uint8 * 128)(*([0xA5] * 128))
    i, o, k = view(inp, len(data)), view(out, c["capacity"]), 0
    if c["op"] == "build": n = getattr(lib, BUILDERS[c["which"] - 1] + "_script_vartime")(i, o)
    elif c["op"] == "tweak": n = lib.taproot_tweak_vartime(i, o)
    elif c["op"] == "kind": n = lib.script_kind_vartime(i)
    elif c["op"] == "encode": n = lib.address_encode_vartime(c["network"], i, o)
    else:
        a = lib.address_decode_vartime(c["network"], i, o)
        n, k = a.length, a.kind
    written = {"build": max(0, n), "tweak": 32 if n >= 0 else 0, "kind": 0}.get(c["op"], max(0, n))
    return n, k, bytes(out[:written]).hex(), all(b == 0xA5 for b in out[written:])


class Address(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix="coax-address-")
        cls.dir = Path(cls.temp.name)
        cls.libs = []
        for label, mode in [("checked", []), ("release", ["--release"])]:
            c = cls.dir / (label + ".c")
            c.write_text(run(COMPILER, "c", SOURCE, "--library", "--header", cls.dir / "address.h", *mode).stdout)
            run("cc", "-std=c11", "-O2", "-shared", "-fPIC", c, "-lm", "-o", cls.dir / (label + ".dylib"))
            lib = C.CDLL(str(cls.dir / (label + ".dylib")))
            for name in [b + "_script_vartime" for b in BUILDERS] + ["taproot_tweak_vartime"]:
                getattr(lib, name).argtypes = [View, View]
                getattr(lib, name).restype = C.c_int64
            lib.script_kind_vartime.argtypes = [View]
            lib.script_kind_vartime.restype = C.c_int64
            lib.address_encode_vartime.argtypes = [C.c_int64, View, View]
            lib.address_encode_vartime.restype = C.c_int64
            lib.address_decode_vartime.argtypes = [C.c_int64, View, View]
            lib.address_decode_vartime.restype = Script
            cls.libs.append((label, lib))
        cls.cases = cases()

    @classmethod
    def tearDownClass(cls):
        cls.temp.cleanup()

    def test_native(self):
        for label, lib in self.libs:
            for c in self.cases:
                want = (c["result"], c["kind"], c["output"], True)
                self.assertEqual(native(lib, c), want, f"{label} {c}")
        results = {c["result"] for c in self.cases}
        self.assertTrue({BAD_NETWORK, WRONG_NETWORK, NO_ADDRESS, BAD_KEY, UNCOMPRESSED, SCRIPT_TOO_LONG, CAPACITY} <= results)
        self.assertTrue(set(range(1, 7)) <= {c["result"] for c in self.cases if c["op"] == "kind"})

    def test_wasm(self):
        clang, sysroot, resources = Path(WASM_CLANG), Path(WASM_SYSROOT), Path(WASM_RESOURCES)
        self.assertTrue(clang.exists() and sysroot.exists() and resources.exists(), "a Wasm toolchain is required; see coax/web/wasm.sh")
        fixture = self.dir / "wasm.json"
        fixture.write_text(json.dumps(self.cases))
        for label in ["checked", "release"]:
            module = self.dir / (label + ".wasm")
            run(clang, "--target=wasm32-wasip1", "--sysroot=" + str(sysroot), "-resource-dir=" + str(resources),
                "-std=c11", "-O2", "-I" + str(self.dir), "-mexec-model=reactor",
                self.dir / (label + ".c"), HERE / "address_wasm.c",
                "-Wl,--import-memory,--export-memory,--export=__heap_base,--max-memory=67108864,-z,stack-size=1048576,--gc-sections",
                "-o", module)
            result = json.loads(run("node", HERE / "address_wasm.cjs", module, fixture).stdout)
            self.assertEqual((result["agree"], result["cases"]), (len(self.cases), len(self.cases)), label)
        print(f"\n{len(self.cases)} cases agree with the Python reference, native checked/release and Wasm checked/release",
              file=sys.stderr)


if __name__ == "__main__":
    unittest.main()
