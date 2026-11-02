#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Python integers and libsecp256k1 decide each case outside Coax)
"""ECDSA verification against Wycheproof, Python integers and libsecp256k1, natively and in Wasm.

Bitcoin From Scratch, chapter 19. Compiles coax/lib/crypto/ecdsa_verify.coax
to C in checked and release builds and runs:

- all 464 cases of the pinned Wycheproof ecdsa_secp256k1_sha256_bitcoin_test.json
  with the low-S rule, each key both uncompressed and compressed;
- the same cases without the low-S rule, and every valid case with s
  replaced by n - s, which must refuse under the rule and verify without it;
- seeded signatures made by coax/lib/crypto/ecdsa_sign.coax (its own C
  build) and by libsecp256k1, each accepted, then refused with one bit of
  the hash, r or s changed and under the wrong key.

Every decision is compared with a separate Python verifier (affine
integers and a strict DER reader) and with libsecp256k1 v0.8.0
(secp256k1_ecdsa_signature_parse_der, _normalize, _verify), built by
tools/coax-nostr/fuzz/build_oracle.py --fetch --reviewed. Then it builds
the same C for wasm32 and runs the Wycheproof cases again in Node. Run from
the repository root:

    python3 coax/tests/bitcoin/ecdsa_verify.py
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
sys.path.insert(0, str(ROOT / "tools/coax-nostr/fuzz"))
from wasm_toolchain import WASM_CLANG, WASM_SYSROOT, WASM_RESOURCES  # noqa: E402
from oracle import DEFAULT_LIBRARY  # noqa: E402

COMPILER = Path(os.environ.get("COAX_TEST_COMPILER", ROOT / "coax/bin/coax"))
VERIFY = ROOT / "coax/lib/crypto/ecdsa_verify.coax"
SIGN = ROOT / "coax/lib/crypto/ecdsa_sign.coax"
WYCHEPROOF = ROOT / "coax/tests/vectors/upstream/wycheproof/ecdsa_secp256k1_sha256_bitcoin_test.json"
WYCHEPROOF_SHA256 = "d3729ce83ff9740c45e74921009a4061eff6c09185b8dd1225ccd06ac57048a4"

P = 2**256 - 2**32 - 977
N = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141
G = (0x79BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798,
     0x483ADA7726A3C4655DA4FBFC0E1108A8FD17B448A68554199C47D08FFB10D4B8)


def run(*args, **kw):
    r = subprocess.run([str(a) for a in args], capture_output=True, text=True, **kw)
    if r.returncode:
        raise RuntimeError(f"{args[0]} failed ({r.returncode}): {r.stderr or r.stdout}")
    return r


# ---- the Python reference: affine points, integers, strict DER
def add(a, b):
    if a is None: return b
    if b is None: return a
    if a[0] == b[0] and (a[1] + b[1]) % P == 0: return None
    if a == b: slope = 3 * a[0] * a[0] * pow(2 * a[1], -1, P) % P
    else: slope = (b[1] - a[1]) * pow(b[0] - a[0], -1, P) % P
    x = (slope * slope - a[0] - b[0]) % P
    return x, (slope * (a[0] - x) - a[1]) % P


def mul(k, point):
    result = None
    while k:
        if k & 1: result = add(result, point)
        point, k = add(point, point), k >> 1
    return result


def der_read(der):
    """r and s from one canonical DER signature, or None."""
    if not 8 <= len(der) <= 72 or der[0] != 0x30 or der[1] != len(der) - 2: return None
    values, at = [], 2
    for _ in range(2):
        if at + 2 > len(der) or der[at] != 2: return None
        n = der[at + 1]; body = der[at + 2:at + 2 + n]
        if not 1 <= n <= 33 or len(body) != n or body[0] & 0x80: return None
        if n > 1 and body[0] == 0 and not body[1] & 0x80: return None
        values.append(int.from_bytes(body, "big")); at += 2 + n
    return tuple(values) if at == len(der) and max(values) < 2**256 else None


def reference(digest, key, der, low_s):
    rs = der_read(der)
    if rs is None: return False
    r, s = rs
    if not (0 < r < N and 0 < s < N) or (low_s and s > N // 2): return False
    z, w = int.from_bytes(digest, "big") % N, pow(s, -1, N)
    point = add(mul(z * w % N, G), mul(r * w % N, key))
    return point is not None and point[0] % N == r


def der(r, s):
    def integer(v):
        b = v.to_bytes(32, "big").lstrip(b"\0") or b"\0"
        if b[0] & 0x80: b = b"\0" + b
        return bytes([2, len(b)]) + b
    body = integer(r) + integer(s)
    return bytes([0x30, len(body)]) + body


def compress(point):
    return bytes([2 | point[1] & 1]) + point[0].to_bytes(32, "big")


def uncompressed(point):
    return b"\4" + point[0].to_bytes(32, "big") + point[1].to_bytes(32, "big")


def wycheproof():
    raw = WYCHEPROOF.read_bytes()
    assert hashlib.sha256(raw).hexdigest() == WYCHEPROOF_SHA256
    doc = json.loads(raw)
    cases = []
    for group in doc["testGroups"]:
        key = bytes.fromhex(group["publicKey"]["uncompressed"])
        point = (int.from_bytes(key[1:33], "big"), int.from_bytes(key[33:], "big"))
        for t in group["tests"]:
            cases.append(dict(id=t["tcId"], key=point, digest=hashlib.sha256(bytes.fromhex(t["msg"])).digest(),
                              der=bytes.fromhex(t["sig"]), valid=t["result"] == "valid"))
    assert len(cases) == doc["numberOfTests"] == 464
    return cases


# ---- the native builds
class View(C.Structure):
    _fields_ = [("data", C.POINTER(C.c_uint8)), ("count", C.c_int64)]


def view(data):
    buffer = (C.c_uint8 * max(1, len(data)))(*data)
    return View(C.cast(buffer, C.POINTER(C.c_uint8)), len(data)), buffer


def build(directory, source, label, mode, header):
    c = directory / (label + ".c")
    c.write_text(run(COMPILER, "c", source, "--library", "--header", directory / header, *mode).stdout)
    run("cc", "-std=c11", "-O2", "-shared", "-fPIC", c, "-lm", "-o", directory / (label + ".dylib"))
    return C.CDLL(str(directory / (label + ".dylib")))


class Libsecp:
    def __init__(self):
        self.lib = C.CDLL(str(DEFAULT_LIBRARY))
        L = self.lib
        for name, args in [("secp256k1_context_create", [C.c_uint]),
                           ("secp256k1_ecdsa_verify", [C.c_void_p, C.c_char_p, C.c_char_p, C.c_void_p]),
                           ("secp256k1_ecdsa_sign", [C.c_void_p, C.c_void_p, C.c_char_p, C.c_char_p, C.c_void_p, C.c_void_p]),
                           ("secp256k1_ecdsa_signature_parse_der", [C.c_void_p, C.c_void_p, C.c_char_p, C.c_size_t]),
                           ("secp256k1_ecdsa_signature_serialize_compact", [C.c_void_p, C.c_void_p, C.c_void_p]),
                           ("secp256k1_ecdsa_signature_normalize", [C.c_void_p, C.c_void_p, C.c_void_p]),
                           ("secp256k1_ec_pubkey_parse", [C.c_void_p, C.c_void_p, C.c_char_p, C.c_size_t])]:
            getattr(L, name).argtypes = args
        L.secp256k1_context_create.restype = C.c_void_p
        self.ctx = L.secp256k1_context_create(1)

    def verify(self, digest, key, der, low_s):
        L, sig, pub = self.lib, C.create_string_buffer(64), C.create_string_buffer(64)
        if not L.secp256k1_ec_pubkey_parse(self.ctx, pub, key, len(key)): return False
        if not L.secp256k1_ecdsa_signature_parse_der(self.ctx, sig, der, len(der)): return False
        high = L.secp256k1_ecdsa_signature_normalize(self.ctx, sig, sig)   # 1: it was high-S
        if high and low_s: return False
        return bool(L.secp256k1_ecdsa_verify(self.ctx, sig, digest, pub))

    def sign(self, digest, secret):
        sig, out = C.create_string_buffer(64), C.create_string_buffer(64)
        assert self.lib.secp256k1_ecdsa_sign(self.ctx, sig, digest, secret, None, None)
        assert self.lib.secp256k1_ecdsa_signature_serialize_compact(self.ctx, out, sig)
        return out.raw


def coax_verify(lib, digest, key, der, low_s):
    (d, _a), (k, _b), (s, _c) = view(digest), view(key), view(der)
    return lib.ecdsa_verify_der_vartime(d, k, s, low_s)


class EcdsaVerify(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        assert DEFAULT_LIBRARY.exists(), "build libsecp256k1 first: python3 tools/coax-nostr/fuzz/build_oracle.py --fetch --reviewed"
        cls.temp = tempfile.TemporaryDirectory(prefix="coax-ecdsa-verify-")
        cls.dir = Path(cls.temp.name)
        cls.secp = Libsecp()
        cls.libs = []
        for label, mode in [("checked", []), ("release", ["--release"])]:
            lib = build(cls.dir, VERIFY, label, mode, "ecdsa_verify.h")
            lib.ecdsa_verify_der_vartime.argtypes = [View, View, View, C.c_bool]
            lib.ecdsa_verify_der_vartime.restype = C.c_bool
            cls.libs.append(lib)
        cls.signer = build(cls.dir, SIGN, "sign", ["--release", "--run-steps", "250000000"], "ecdsa_sign.h")
        cls.signer.ecdsa_sign.argtypes = [View, View, View, View, C.c_bool]
        cls.signer.ecdsa_sign.restype = C.c_bool
        cls.cases = wycheproof()

    @classmethod
    def tearDownClass(cls):
        cls.temp.cleanup()

    def decide(self, digest, point, der, low_s, want, label):
        """Every verifier, with the key in both SEC1 forms, gives want."""
        self.assertEqual(reference(digest, point, der, low_s), want, label + " (Python)")
        for key in [uncompressed(point), compress(point)]:
            self.assertEqual(self.secp.verify(digest, key, der, low_s), want, label + " (libsecp256k1)")
            for lib in self.libs:
                self.assertEqual(coax_verify(lib, digest, key, der, low_s), want, label)

    def test_wycheproof(self):
        valid = 0
        for c in self.cases:
            label = f"Wycheproof #{c['id']}"
            self.decide(c["digest"], c["key"], c["der"], True, c["valid"], label)
            if not c["valid"]:
                continue
            valid += 1
            # Without the rule the case still verifies; its other half
            # (r, n - s) verifies only without it.
            r, s = der_read(c["der"])
            self.decide(c["digest"], c["key"], c["der"], False, True, label + " without low-S")
            self.decide(c["digest"], c["key"], der(r, N - s), True, False, label + " high-S")
            self.decide(c["digest"], c["key"], der(r, N - s), False, True, label + " high-S without low-S")
        self.assertEqual((valid, len(self.cases)), (162, 464))

    def test_signatures(self):
        rng = random.Random(19)
        count = 0
        for i in range(60):
            secret = rng.randrange(1, N) if i else 1
            digest = rng.randbytes(32) if i % 7 else b"\xff" * 32
            point = mul(secret, G)
            secret_bytes = secret.to_bytes(32, "big")
            if i % 2:
                compact = self.secp.sign(digest, secret_bytes)
            else:
                (k, _a), (h, _b), (o, out) = view(secret_bytes), view(digest), view(bytes(64))
                work = C.create_string_buffer(65536)
                self.assertTrue(self.signer.ecdsa_sign(k, h, View(C.cast(work, C.POINTER(C.c_uint8)), 1), o, False))
                compact = bytes(out)
                self.assertEqual(compact, self.secp.sign(digest, secret_bytes), "Coax signs as libsecp256k1 does")
            r, s = int.from_bytes(compact[:32], "big"), int.from_bytes(compact[32:], "big")
            self.decide(digest, point, der(r, s), True, True, f"signature {i}")
            changed = bytearray(digest); changed[rng.randrange(32)] ^= 1 << rng.randrange(8)
            self.decide(bytes(changed), point, der(r, s), True, False, f"signature {i}, hash changed")
            self.decide(digest, point, der(r ^ (1 << rng.randrange(256)), s), True, False, f"signature {i}, r changed")
            self.decide(digest, point, der(r, s ^ (1 << rng.randrange(255))), True, False, f"signature {i}, s changed")
            self.decide(digest, mul(secret + 1, G), der(r, s), True, False, f"signature {i}, wrong key")
            count += 1
        self.assertEqual(count, 60)

    def test_wasm(self):
        clang, sysroot, resources = Path(WASM_CLANG), Path(WASM_SYSROOT), Path(WASM_RESOURCES)
        self.assertTrue(clang.exists() and sysroot.exists() and resources.exists(), "a Wasm toolchain is required; see coax/web/wasm.sh")
        fixture = self.dir / "wasm.json"
        fixture.write_text(json.dumps([dict(key=uncompressed(c["key"]).hex(), compressed=compress(c["key"]).hex(),
                                            digest=c["digest"].hex(), der=c["der"].hex(), valid=c["valid"], id=c["id"])
                                       for c in self.cases]))
        for label in ["checked", "release"]:
            module = self.dir / (label + ".wasm")
            run(clang, "--target=wasm32-wasip1", "--sysroot=" + str(sysroot), "-resource-dir=" + str(resources),
                "-std=c11", "-O2", "-I" + str(self.dir), "-mexec-model=reactor",
                self.dir / (label + ".c"), HERE / "ecdsa_verify_wasm.c",
                "-Wl,--import-memory,--export-memory,--export=__heap_base,--max-memory=67108864,-z,stack-size=1048576,--gc-sections",
                "-o", module)
            result = json.loads(run("node", HERE / "ecdsa_verify_wasm.cjs", module, fixture).stdout)
            self.assertEqual((result["agree"], result["cases"]), (2 * 464, 464), label)
        print(f"\n{len(self.cases)} Wycheproof cases and 60 signatures, native checked/release and Wasm; "
              f"oracle {DEFAULT_LIBRARY.name}", file=sys.stderr)


if __name__ == "__main__":
    unittest.main()
