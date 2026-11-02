#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: libsecp256k1 and Python integers decide each signature outside Coax)
"""ECDSA signing against tiny-secp256k1's vectors, libsecp256k1 and Python, natively and in Wasm.

Bitcoin From Scratch, chapter 20. Compiles coax/lib/crypto/ecdsa_sign.coax
to C in checked and release builds and runs:

- all 2,020 signing rows of the pinned tiny-secp256k1 tests/fixtures/ecdsa.json
  and its 4 bad keys;
- seeded random signatures: keys including 1, n - 1, n - 2, small keys and
  keys with leading zero bytes, and hashes including 0, n, n - 1 and
  2^256 - 1.

Each signature must equal libsecp256k1 v0.8.0's (secp256k1_ecdsa_sign, RFC
6979, no extra entropy) byte for byte, compact and DER; be low-S; verify in
libsecp256k1 (secp256k1_ecdsa_verify), in a Python verifier with affine
integers and in chapter 19's Coax verifier (its own C build); and leave the
workspace all zero. The RFC 6979 nonce of each random row is also computed
with Python's hmac and integers, and r must equal x(k·G) mod n for it. Then
it builds the same C for wasm32 and signs the 2,020 rows again in Node.
libsecp256k1 comes from tools/coax-nostr/fuzz/build_oracle.py --fetch
--reviewed. Run from the repository root:

    python3 coax/tests/bitcoin/ecdsa_sign.py
"""
import ctypes as C
import hashlib
import hmac
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
sys.path.insert(0, str(ROOT / "tools/coax-nostr/fuzz"))
sys.path.insert(0, str(HERE))
from wasm_toolchain import WASM_CLANG, WASM_SYSROOT, WASM_RESOURCES  # noqa: E402
from oracle import DEFAULT_LIBRARY  # noqa: E402
from ecdsa_verify import N, G, View, add, mul, compress, der, reference, run, view  # noqa: E402

COMPILER = Path(os.environ.get("COAX_TEST_COMPILER", ROOT / "coax/bin/coax"))
SIGN = ROOT / "coax/lib/crypto/ecdsa_sign.coax"
VERIFY = ROOT / "coax/lib/crypto/ecdsa_verify.coax"
VECTORS = ROOT / "coax/tests/vectors/upstream/tiny-secp256k1/ecdsa.json"
VECTORS_SHA256 = "b830aa30d0ab3b6c51440caffac9206b24ae5450a387a2db73789f5ce795595d"
STEPS = ["--run-steps", "250000000"]


def rfc6979(secret, digest):
    """RFC 6979 section 3.2 for SHA-256 and secp256k1: the first valid k."""
    x, h = secret.to_bytes(32, "big"), (int.from_bytes(digest, "big") % N).to_bytes(32, "big")
    v, k = b"\1" * 32, b"\0" * 32
    mac = lambda key, data: hmac.new(key, data, hashlib.sha256).digest()
    k = mac(k, v + b"\0" + x + h); v = mac(k, v)
    k = mac(k, v + b"\1" + x + h); v = mac(k, v)
    while True:
        v = mac(k, v)
        candidate = int.from_bytes(v, "big")
        if 1 <= candidate < N:
            return candidate
        k = mac(k, v + b"\0"); v = mac(k, v)


def vectors():
    raw = VECTORS.read_bytes()
    assert hashlib.sha256(raw).hexdigest() == VECTORS_SHA256
    doc = json.loads(raw)
    valid = [(bytes.fromhex(v["d"]), bytes.fromhex(v["m"]), bytes.fromhex(v["signature"])) for v in doc["valid"]]
    bad = [(bytes.fromhex(v["d"]), bytes.fromhex(v["m"])) for v in doc["invalid"]["sign"]]
    assert len(valid) == 2020 and len(bad) == 4
    return valid, bad


class Libsecp:
    def __init__(self):
        self.lib = L = C.CDLL(str(DEFAULT_LIBRARY))
        for name, args in [("secp256k1_context_create", [C.c_uint]),
                           ("secp256k1_ecdsa_sign", [C.c_void_p, C.c_void_p, C.c_char_p, C.c_char_p, C.c_void_p, C.c_void_p]),
                           ("secp256k1_ecdsa_verify", [C.c_void_p, C.c_char_p, C.c_char_p, C.c_void_p]),
                           ("secp256k1_ecdsa_signature_parse_compact", [C.c_void_p, C.c_void_p, C.c_char_p]),
                           ("secp256k1_ecdsa_signature_serialize_compact", [C.c_void_p, C.c_void_p, C.c_void_p]),
                           ("secp256k1_ecdsa_signature_serialize_der", [C.c_void_p, C.c_void_p, C.c_void_p, C.c_void_p]),
                           ("secp256k1_ecdsa_signature_normalize", [C.c_void_p, C.c_void_p, C.c_void_p]),
                           ("secp256k1_ec_pubkey_create", [C.c_void_p, C.c_void_p, C.c_char_p])]:
            getattr(L, name).argtypes = args
        L.secp256k1_context_create.restype = C.c_void_p
        self.ctx = L.secp256k1_context_create(1)

    def sign(self, digest, secret):
        sig, compact = C.create_string_buffer(64), C.create_string_buffer(64)
        if not self.lib.secp256k1_ecdsa_sign(self.ctx, sig, digest, secret, None, None):
            return None
        assert self.lib.secp256k1_ecdsa_signature_serialize_compact(self.ctx, compact, sig)
        encoded, n = C.create_string_buffer(72), C.c_size_t(72)
        assert self.lib.secp256k1_ecdsa_signature_serialize_der(self.ctx, encoded, C.byref(n), sig)
        return compact.raw, encoded.raw[:n.value]

    def verify(self, digest, secret, compact):
        """True when the compact signature parses, is low-S and verifies."""
        L, sig, pub = self.lib, C.create_string_buffer(64), C.create_string_buffer(64)
        if not L.secp256k1_ecdsa_signature_parse_compact(self.ctx, sig, compact):
            return False
        if L.secp256k1_ecdsa_signature_normalize(self.ctx, None, sig):   # 1: it was high-S
            return False
        assert L.secp256k1_ec_pubkey_create(self.ctx, pub, secret)
        return bool(L.secp256k1_ecdsa_verify(self.ctx, sig, digest, pub))


def build(directory, source, label, mode, header, extra=()):
    c = directory / (label + ".c")
    c.write_text(run(COMPILER, "c", source, "--library", "--header", directory / header, *mode).stdout)
    run("cc", "-std=c11", "-O2", "-shared", "-fPIC", "-I" + str(directory), c, *extra, "-lm", "-o", directory / (label + ".dylib"))
    return C.CDLL(str(directory / (label + ".dylib")))


class EcdsaSign(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        assert DEFAULT_LIBRARY.exists(), "build libsecp256k1 first: python3 tools/coax-nostr/fuzz/build_oracle.py --fetch --reviewed"
        cls.temp = tempfile.TemporaryDirectory(prefix="coax-ecdsa-sign-")
        cls.dir = Path(cls.temp.name)
        cls.secp = Libsecp()
        cls.signers = []
        for label, mode in [("checked", STEPS), ("release", ["--release", *STEPS])]:
            lib = build(cls.dir, SIGN, label, mode, "ecdsa_sign.h", [HERE / "ecdsa_sign_wasm.c"])
            lib.test_sign.argtypes = [C.c_char_p, C.c_char_p, C.c_void_p]
            lib.test_sign.restype = C.c_uint32
            lib.ecdsa_der.argtypes = [View, View]
            lib.ecdsa_der.restype = C.c_int64
            cls.signers.append((label, lib))
        cls.verifier = build(cls.dir, VERIFY, "verify", ["--release"], "ecdsa_verify.h")
        cls.verifier.ecdsa_verify_vartime.argtypes = [View, View, View, C.c_bool]
        cls.verifier.ecdsa_verify_vartime.restype = C.c_bool
        cls.valid, cls.bad = vectors()

    @classmethod
    def tearDownClass(cls):
        cls.temp.cleanup()

    def coax_sign(self, lib, secret, digest):
        out = C.create_string_buffer(64)
        result = lib.test_sign(secret, digest, out)
        self.assertIn(result, (0, 1), "the workspace is cleared on every return")
        return out.raw if result == 1 else None

    def coax_der(self, lib, compact):
        (s, _a), (o, buffer) = view(compact), view(bytes(72))
        n = lib.ecdsa_der(s, o)
        return bytes(buffer)[:n] if n > 0 else None

    def check(self, secret, digest, label, published=None):
        """Both Coax builds sign as libsecp256k1 does, and every verifier accepts."""
        want, want_der = self.secp.sign(digest, secret)
        if published is not None:
            self.assertEqual(want, published, label + " (libsecp256k1 against the pinned row)")
        d = int.from_bytes(secret, "big")
        point = mul(d, G)
        for name, lib in self.signers:
            got = self.coax_sign(lib, secret, digest)
            self.assertEqual(got, want, f"{label} ({name})")
            self.assertEqual(self.coax_der(lib, got), want_der, f"{label} DER ({name})")
        r, s = int.from_bytes(want[:32], "big"), int.from_bytes(want[32:], "big")
        self.assertLessEqual(s, N // 2, label + " low-S")
        self.assertTrue(self.secp.verify(digest, secret, want), label + " (libsecp256k1 verify)")
        self.assertTrue(reference(digest, point, der(r, s), True), label + " (Python verify)")
        (h, _a), (k, _b), (g, _c) = view(digest), view(compress(point)), view(want)
        self.assertTrue(self.verifier.ecdsa_verify_vartime(h, k, g, True), label + " (chapter 19's verifier)")
        return r, s

    def test_pinned_rows(self):
        for i, (secret, digest, published) in enumerate(self.valid):
            self.check(secret, digest, f"row {i}", published)
        for i, (secret, digest) in enumerate(self.bad):
            self.assertIsNone(self.secp.sign(digest, secret))
            for name, lib in self.signers:
                self.assertIsNone(self.coax_sign(lib, secret, digest), f"bad key {i} ({name})")

    def test_random(self):
        rng = random.Random(20)
        keys = [1, N - 1, 2, N - 2, 0xff, 1 << 200]
        hashes = [bytes(32), N.to_bytes(32, "big"), (N - 1).to_bytes(32, "big"), b"\xff" * 32, b"\0" * 31 + b"\1"]
        for i in range(400):
            d = keys[i] if i < len(keys) else (rng.randrange(1, N) >> (8 * rng.choice([0, 0, 0, 1, 2]))) or 1
            digest = hashes[i % len(hashes)] if i < 3 * len(hashes) else rng.randbytes(32)
            r, s = self.check(d.to_bytes(32, "big"), digest, f"random {i}")
            k = rfc6979(d, digest)
            self.assertEqual(mul(k, G)[0] % N, r, f"random {i}: r = x(kG) for Python's RFC 6979 k")
            z = int.from_bytes(digest, "big") % N
            self.assertEqual(min(pow(k, -1, N) * (z + r * d) % N, N - pow(k, -1, N) * (z + r * d) % N), s, f"random {i}: s")

    def test_wasm(self):
        clang, sysroot, resources = Path(WASM_CLANG), Path(WASM_SYSROOT), Path(WASM_RESOURCES)
        self.assertTrue(clang.exists() and sysroot.exists() and resources.exists(), "a Wasm toolchain is required; see coax/web/wasm.sh")
        fixture = self.dir / "wasm.json"
        fixture.write_text(json.dumps([dict(d=d.hex(), m=m.hex(), sig=s.hex()) for d, m, s in self.valid] +
                                      [dict(d=d.hex(), m=m.hex(), sig=None) for d, m in self.bad]))
        for label in ["checked", "release"]:
            module = self.dir / (label + ".wasm")
            run(clang, "--target=wasm32-wasip1", "--sysroot=" + str(sysroot), "-resource-dir=" + str(resources),
                "-std=c11", "-O2", "-I" + str(self.dir), "-mexec-model=reactor",
                self.dir / (label + ".c"), HERE / "ecdsa_sign_wasm.c",
                "-Wl,--import-memory,--export-memory,--export=__heap_base,--max-memory=67108864,-z,stack-size=1048576,--gc-sections",
                "-o", module)
            result = json.loads(run("node", HERE / "ecdsa_sign_wasm.cjs", module, fixture).stdout)
            self.assertEqual((result["agree"], result["cases"]), (2024, 2024), label)
        print(f"\n2,020 pinned rows, 4 bad keys and 400 random signatures, native checked/release and Wasm; "
              f"oracle {DEFAULT_LIBRARY.name}", file=sys.stderr)


if __name__ == "__main__":
    unittest.main()
