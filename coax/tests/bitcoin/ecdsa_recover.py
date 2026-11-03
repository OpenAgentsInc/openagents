#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: libsecp256k1 and Python integers decide each case outside Coax)
"""Public key recovery and signed messages against tiny-secp256k1's vectors,
libsecp256k1's recovery module, Bitcoin Core's cases and Python, natively
and in Wasm.

Bitcoin From Scratch, chapter 22. Compiles coax/lib/bitcoin/signmessage.coax
(which loads ecdsa_recover.coax and ecdsa_sign.coax) to C in checked and
release builds and runs:

- all 2,020 valid rows of the pinned tiny-secp256k1 tests/fixtures/ecdsa.json:
  ecdsa_sign_recoverable gives the row's signature and recoveryId, equal to
  libsecp256k1's secp256k1_ecdsa_sign_recoverable, and recovery with each of
  the IDs 0 to 4, compressed and uncompressed, agrees with libsecp256k1's
  secp256k1_ecdsa_recover and with a Python recovery that names its reason;
- the 8 invalid.recover rows and libsecp256k1's recovery edge cases;
- seeded random signatures, including small r, so that IDs 2 and 3 recover,
  r at p - n and around it, r and s at 0 and n, and random 64 bytes;
- Bitcoin Core's message_sign and message_verify cases and the
  rpc_signmessagewithprivkey.py case, then seeded random messages (lengths
  0, 1, 252, 253, 65535 and 65536 among them) signed with random keys,
  compared with a Python signmessage over libsecp256k1, and every header
  byte 0 to 255 on one signature, compared with Bitcoin Core's
  MessageVerify rules written in Python.

Then it builds the same C for wasm32 and runs the rows, the refusals and
the message cases again in Node. libsecp256k1 comes from
tools/coax-nostr/fuzz/build_oracle.py --fetch --reviewed, which enables its
recovery module. Run from the repository root:

    python3 coax/tests/bitcoin/ecdsa_recover.py
"""
import base64
import binascii
import ctypes as C
import hashlib
import json
import os
from pathlib import Path
import random
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
from ecdsa_verify import N, P, G, add, mul, compress, uncompressed, run  # noqa: E402

COMPILER = Path(os.environ.get("COAX_TEST_COMPILER", ROOT / "coax/bin/coax"))
SOURCE = ROOT / "coax/lib/bitcoin/signmessage.coax"
UPSTREAM = ROOT / "coax/tests/vectors/upstream"
TINY_SHA256 = "b830aa30d0ab3b6c51440caffac9206b24ae5450a387a2db73789f5ce795595d"
STEPS = ["--run-steps", "250000000"]

# Codes of ecdsa_recover.coax and signmessage.coax.
BAD_LENGTH, BAD_ID, BAD_SCALAR, NO_X, NO_POINT, INFINITY = -1, -2, -3, -4, -5, -6
SM = dict(OK=0, INVALID_ADDRESS=-1, ADDRESS_NO_KEY=-2, MALFORMED_SIGNATURE=-3, NOT_RECOVERED=-4, NOT_SIGNED=-5,
          BAD_LENGTH=-6, BAD_HEADER=-7)
CORE = dict(ERR_INVALID_ADDRESS="INVALID_ADDRESS", ERR_ADDRESS_NO_KEY="ADDRESS_NO_KEY",
            ERR_MALFORMED_SIGNATURE="MALFORMED_SIGNATURE", ERR_PUBKEY_NOT_RECOVERED="NOT_RECOVERED",
            ERR_NOT_SIGNED="NOT_SIGNED", OK="OK")


# ---- Python references
def py_recover(digest, sig, rid):
    """(key point, None) or (None, the reason), as ecdsa_recover.coax names it."""
    if len(digest) != 32 or len(sig) != 64: return None, BAD_LENGTH
    if not 0 <= rid <= 3: return None, BAD_ID
    r, s = int.from_bytes(sig[:32], "big"), int.from_bytes(sig[32:], "big")
    if not (0 < r < N and 0 < s < N): return None, BAD_SCALAR
    x = r + N * (rid >> 1)
    if x >= P: return None, NO_X
    y2 = (x * x * x + 7) % P; y = pow(y2, (P + 1) // 4, P)
    if y * y % P != y2: return None, NO_POINT
    if y & 1 != rid & 1: y = P - y
    z = int.from_bytes(digest, "big") % N; rinv = pow(r, -1, N)
    q = add(mul(-z * rinv % N, G), mul(s * rinv % N, (x, y)))
    return (q, None) if q is not None else (None, INFINITY)


def compact_size(n):
    if n < 0xfd: return bytes([n])
    if n <= 0xffff: return b"\xfd" + n.to_bytes(2, "little")
    if n <= 0xffffffff: return b"\xfe" + n.to_bytes(4, "little")
    return b"\xff" + n.to_bytes(8, "little")


def message_hash(message):
    magic = b"Bitcoin Signed Message:\n"
    data = compact_size(len(magic)) + magic + compact_size(len(message)) + message
    return hashlib.sha256(hashlib.sha256(data).digest()).digest()


def hash160(b):
    return hashlib.new("ripemd160", hashlib.sha256(b).digest()).digest()


B58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"


def base58check(payload):
    data = payload + hashlib.sha256(hashlib.sha256(payload).digest()).digest()[:4]
    n, text = int.from_bytes(data, "big"), ""
    while n: n, digit = divmod(n, 58); text = B58[digit] + text
    return "1" * (len(data) - len(data.lstrip(b"\0"))) + text


def address_hash(address, testnet):
    """('p2pkh' | 'p2sh', hash) for a Base58 address of the network, or None."""
    if not address or any(c not in B58 for c in address): return None
    n = 0
    for c in address: n = n * 58 + B58.index(c)
    body = n.to_bytes((n.bit_length() + 7) // 8, "big")
    data = b"\0" * (len(address) - len(address.lstrip("1"))) + body
    if len(data) != 25 or hashlib.sha256(hashlib.sha256(data[:21]).digest()).digest()[:4] != data[21:]: return None
    kinds = {0x6f: "p2pkh", 0xc4: "p2sh"} if testnet else {0x00: "p2pkh", 0x05: "p2sh"}
    return (kinds[data[0]], data[1:21]) if data[0] in kinds else None


def strict_base64(text):
    """Bitcoin Core's DecodeBase64: padded, the alphabet only, no stray bits."""
    try:
        raw = base64.b64decode(text, validate=True)
    except (binascii.Error, ValueError):
        return None
    return raw if base64.b64encode(raw).decode() == text else None


def core_verify(secp, address, signature, message, testnet):
    """Bitcoin Core's MessageVerify (src/common/signmessage.cpp) for Base58
    addresses: the header is masked, not range-checked."""
    kind = address_hash(address, testnet)
    if kind is None: return "INVALID_ADDRESS"
    if kind[0] != "p2pkh": return "ADDRESS_NO_KEY"
    raw = strict_base64(signature)
    if raw is None: return "MALFORMED_SIGNATURE"
    if len(raw) != 65: return "NOT_RECOVERED"
    rid, compressed = (raw[0] - 27) & 3, ((raw[0] - 27) & 4) != 0
    key = secp.recover(message_hash(message), raw[1:], rid, compressed)
    if key is None: return "NOT_RECOVERED"
    return "OK" if hash160(key) == kind[1] else "NOT_SIGNED"


def our_verify(secp, address, signature, message, testnet):
    """The same rules with this verifier's two extra refusals."""
    kind = address_hash(address, testnet)
    if kind is None: return "INVALID_ADDRESS"
    if kind[0] != "p2pkh": return "ADDRESS_NO_KEY"
    raw = strict_base64(signature)
    if raw is None: return "MALFORMED_SIGNATURE"
    if len(raw) != 65: return "BAD_LENGTH"
    if not 27 <= raw[0] <= 34: return "BAD_HEADER"
    return core_verify(secp, address, signature, message, testnet)


class Libsecp:
    def __init__(self):
        self.lib = L = C.CDLL(str(DEFAULT_LIBRARY))
        for name, args in [("secp256k1_context_create", [C.c_uint]),
                           ("secp256k1_ecdsa_sign_recoverable", [C.c_void_p, C.c_void_p, C.c_char_p, C.c_char_p, C.c_void_p, C.c_void_p]),
                           ("secp256k1_ecdsa_recoverable_signature_serialize_compact", [C.c_void_p, C.c_void_p, C.c_void_p, C.c_void_p]),
                           ("secp256k1_ecdsa_recoverable_signature_parse_compact", [C.c_void_p, C.c_void_p, C.c_char_p, C.c_int]),
                           ("secp256k1_ecdsa_recover", [C.c_void_p, C.c_void_p, C.c_void_p, C.c_char_p]),
                           ("secp256k1_ec_pubkey_serialize", [C.c_void_p, C.c_void_p, C.c_void_p, C.c_void_p, C.c_uint])]:
            assert hasattr(L, name), "rebuild libsecp256k1 with its recovery module: python3 tools/coax-nostr/fuzz/build_oracle.py --fetch --reviewed"
            getattr(L, name).argtypes = args
        L.secp256k1_context_create.restype = C.c_void_p
        self.ctx = L.secp256k1_context_create(1)

    def sign(self, digest, secret):
        sig, out, rid = C.create_string_buffer(65), C.create_string_buffer(64), C.c_int()
        if not self.lib.secp256k1_ecdsa_sign_recoverable(self.ctx, sig, digest, secret, None, None): return None
        assert self.lib.secp256k1_ecdsa_recoverable_signature_serialize_compact(self.ctx, out, C.byref(rid), sig)
        return out.raw + bytes([rid.value])

    def recover(self, digest, compact, rid, compressed=True):
        sig, pub = C.create_string_buffer(65), C.create_string_buffer(64)
        if not self.lib.secp256k1_ecdsa_recoverable_signature_parse_compact(self.ctx, sig, compact, rid): return None
        if not self.lib.secp256k1_ecdsa_recover(self.ctx, pub, sig, digest): return None
        out, n = C.create_string_buffer(65), C.c_size_t(65)
        assert self.lib.secp256k1_ec_pubkey_serialize(self.ctx, out, C.byref(n), pub, 258 if compressed else 2)
        return out.raw[:n.value]


def buffer(data, size=None):
    b = C.create_string_buffer(bytes(data), size or max(1, len(data)))
    return C.addressof(b), b


class EcdsaRecover(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        assert DEFAULT_LIBRARY.exists(), "build libsecp256k1 first: python3 tools/coax-nostr/fuzz/build_oracle.py --fetch --reviewed"
        cls.temp = tempfile.TemporaryDirectory(prefix="coax-ecdsa-recover-")
        cls.dir = Path(cls.temp.name)
        cls.secp = Libsecp()
        cls.libs = []
        for label, mode in [("checked", STEPS), ("release", ["--release", *STEPS])]:
            c = cls.dir / (label + ".c")
            c.write_text(run(COMPILER, "c", SOURCE, "--library", "--header", cls.dir / "signmessage.h", *mode).stdout)
            run("cc", "-std=c11", "-O2", "-shared", "-fPIC", "-I" + str(cls.dir), c, HERE / "signed_messages_wasm.c",
                "-lm", "-o", cls.dir / (label + ".dylib"))
            lib = C.CDLL(str(cls.dir / (label + ".dylib")))
            p, i = C.c_size_t, C.c_int32
            lib.test_recover.argtypes = [p, i, p, i, i, p]; lib.test_recover.restype = i
            lib.test_sign.argtypes = [p, p, p]; lib.test_sign.restype = C.c_uint32
            lib.test_message_sign.argtypes = [p, C.c_uint32, p, i, p]; lib.test_message_sign.restype = i
            lib.test_message_verify.argtypes = [p, i, p, i, p, i, C.c_uint32]; lib.test_message_verify.restype = i
            lib.test_message_hash.argtypes = [p, i, p]; lib.test_message_hash.restype = C.c_uint32
            cls.libs.append((label, lib))
        raw = (UPSTREAM / "tiny-secp256k1/ecdsa.json").read_bytes()
        assert hashlib.sha256(raw).hexdigest() == TINY_SHA256
        doc = json.loads(raw)
        cls.rows = [(bytes.fromhex(v["d"]), bytes.fromhex(v["m"]), bytes.fromhex(v["signature"]), v["recoveryId"]) for v in doc["valid"]]
        cls.refuse = [(bytes.fromhex(v["m"]), bytes.fromhex(v["signature"]), v.get("recoveryId", 0)) for v in doc["invalid"]["recover"]]
        assert len(cls.rows) == 2020 and len(cls.refuse) == 8
        cls.wasm_recover, cls.wasm_sign, cls.wasm_verify, cls.wasm_message = [], [], [], []

    @classmethod
    def tearDownClass(cls):
        cls.temp.cleanup()

    # ---- calls into both native builds
    def recover(self, digest, sig, rid, form=33):
        results = set()
        for name, lib in self.libs:
            (h, _a), (s, _b), (o, out) = buffer(digest), buffer(sig, 64), buffer(b"", 65)
            n = lib.test_recover(h, len(digest), s, rid, form, o)
            results.add((n, out.raw[:n] if n > 0 else None))
        self.assertEqual(len(results), 1, "checked and release agree")
        return results.pop()

    def check_recovery(self, digest, sig, rid, label, record=False):
        point, reason = py_recover(digest, sig, rid)
        for form, encode, compressed in [(33, compress, True), (65, uncompressed, False)]:
            n, key = self.recover(digest, sig, rid, form)
            theirs = self.secp.recover(digest, sig, rid, compressed) if len(digest) == 32 and 0 <= rid <= 3 else None
            if point is None:
                self.assertEqual((n, key), (reason, None), f"{label}, ID {rid}: the reason")
                self.assertIsNone(theirs, f"{label}, ID {rid}: libsecp256k1 refuses too")
            else:
                self.assertEqual((n, key), (form, encode(point)), f"{label}, ID {rid}")
                self.assertEqual(theirs, key, f"{label}, ID {rid} (libsecp256k1)")
        if record:
            self.wasm_recover.append(dict(m=digest.hex(), sig=sig.hex(), id=rid, key=compress(point).hex() if point else None,
                                          code=reason))
        return point

    def test_rows(self):
        ids = [0, 0, 0, 0]
        for i, (d, m, sig, rid) in enumerate(self.rows):
            want = self.secp.sign(m, d)
            self.assertEqual(want, sig + bytes([rid]), f"row {i}: libsecp256k1 against the pinned row")
            for name, lib in self.libs:
                (k, _a), (h, _b), (o, out) = buffer(d), buffer(m), buffer(b"", 65)
                self.assertEqual(lib.test_sign(k, h, o), 1, f"row {i} ({name}): signed, workspace cleared")
                self.assertEqual(out.raw[:65], want, f"row {i} ({name})")
            key = mul(int.from_bytes(d, "big"), G)
            for other in range(5):
                point = self.check_recovery(m, sig, other, f"row {i}", record=other == rid or other == 4)
                if other == rid: self.assertEqual(point, key, f"row {i}: the signing key")
                elif point is not None: self.assertNotEqual(point, key, f"row {i}, ID {other}")
            ids[rid] += 1
            self.wasm_sign.append(dict(d=d.hex(), m=m.hex(), sig=want.hex()))
        self.assertEqual(ids, [1000, 1020, 0, 0])

    def test_refusals_and_edges(self):
        for i, (m, sig, rid) in enumerate(self.refuse):
            self.assertIsNone(self.check_recovery(m, sig, rid, f"refusal {i}", record=True))
        msg = b"This is a very secret message..."
        edge = UPSTREAM / "secp256k1/recovery/tests_impl.h"
        text = edge.read_text()
        self.assertIn("0x67, 0xCB, 0x28, 0x5F", text)
        key0 = bytes.fromhex("67cb285f9cd194e840d629397af5569662fde44649995963179a7dd17bd235324b1b7df34ce1f68e694ff6f11ac751dd7dd73e387ee4fc866e1be8ecc7dd9557")
        reasons = []
        for sig in [key0, bytes(31) + b"\4" + bytes(31) + b"\4", bytes(31) + b"\1" + bytes(31) + b"\1",
                    bytes(64), bytes(31) + b"\1" + bytes(32), bytes(63) + b"\1"]:
            for rid in range(4):
                self.check_recovery(msg, sig, rid, "edge", record=True)
                reasons.append(py_recover(msg, sig, rid)[1])
        self.assertEqual(reasons[:8], [INFINITY, None, NO_X, NO_X, None, None, None, None])
        self.assertEqual(reasons[8:12], [None, None, NO_POINT, NO_POINT])

    def test_random(self):
        rng = random.Random(22)
        seen = set()
        boundary = P - N
        for i in range(600):
            digest = rng.randbytes(32)
            kind = i % 6
            if kind == 0: r = rng.randrange(1, 1 << 24)                       # small: IDs 2 and 3 may recover
            elif kind == 1: r = boundary + rng.randrange(-3, 3)                  # around p - n
            elif kind == 2: r = rng.choice([0, 1, N - 1, N, 2**256 - 1])
            else: r = rng.randrange(0, 2**256)
            s = rng.choice([0, 1, N - 1, N, rng.randrange(1, N)]) if kind == 2 else rng.randrange(1, N)
            sig = (r % 2**256).to_bytes(32, "big") + s.to_bytes(32, "big")
            for rid in range(4):
                point = self.check_recovery(digest, sig, rid, f"random {i}", record=i % 10 == 0)
                seen.add((rid, py_recover(digest, sig, rid)[1]))
        for rid in range(4): self.assertIn((rid, None), seen, f"ID {rid} recovered at least once")
        for reason in [BAD_SCALAR, NO_X, NO_POINT]: self.assertTrue(any(r == reason for _, r in seen), reason)
        for name, lib in self.libs:
            (h, _a), (s, _b), (o, out) = buffer(bytes(31)), buffer(bytes(64)), buffer(b"", 65)
            self.assertEqual(lib.test_recover(h, 31, s, 0, 33, o), BAD_LENGTH)

    def verify(self, address, signature, message, testnet):
        results = set()
        for name, lib in self.libs:
            (a, _a), (s, _b), (m, _c) = buffer(address.encode()), buffer(signature.encode()), buffer(message)
            results.add(lib.test_message_verify(a, len(address), s, len(signature), m, len(message), testnet))
        self.assertEqual(len(results), 1)
        return results.pop()

    def decide(self, address, signature, message, testnet, label):
        got = self.verify(address, signature, message, testnet)
        want = our_verify(self.secp, address, signature, message, testnet)
        self.assertEqual(got, SM[want], f"{label}: {want}")
        core = core_verify(self.secp, address, signature, message, testnet)
        if want not in ("BAD_HEADER", "BAD_LENGTH"): self.assertEqual(want, core, label + " (Core's rules)")
        if want == "BAD_LENGTH": self.assertEqual(core, "NOT_RECOVERED", label)
        self.wasm_verify.append(dict(address=address, sig=signature, message=message.hex(), testnet=testnet, code=got))
        return want, core

    def message_sign(self, d, compressed, message):
        results = set()
        for name, lib in self.libs:
            (k, _a), (m, _b), (o, out) = buffer(d), buffer(message), buffer(b"", 88)
            n = lib.test_message_sign(k, compressed, m, len(message), o)
            results.add(out.raw[:n].decode() if n == 88 else n)
            (h, _c), (q, digest) = buffer(b"", 32), buffer(b"", 32)
            self.assertEqual(lib.test_message_hash(m, len(message), q), 1)
            self.assertEqual(digest.raw[:32], message_hash(message), "the message hash")
        self.assertEqual(len(results), 1)
        return results.pop()

    def python_sign(self, d, compressed, message):
        signed = self.secp.sign(message_hash(message), d)
        return base64.b64encode(bytes([27 + signed[64] + (4 if compressed else 0)]) + signed[:64]).decode()

    def test_core_cases(self):
        util = (UPSTREAM / "bitcoin-core/util_tests.cpp").read_text()
        key = bytes.fromhex("d97f5108f11cda6eeebaaa420fef0726b1f898060b98489fa3098463c0032866")
        self.assertIn("0xD9, 0x7F, 0x51, 0x08", util)
        sig = self.message_sign(key, True, b"Trust no one")
        self.assertEqual(sig, "IPojfrX2dfPnH26UegfbGQQLrdK844DlHq5157/P6h57WyuS/Qsl+h/WSVGDF4MUi4rWSswW38oimDYfNNUBUOk=")
        self.assertIn(sig, util)
        cases = [("invalid address", "signature should be irrelevant", b"message too", "ERR_INVALID_ADDRESS"),
                 ("3B5fQsEXEaV8v6U3ejYc8XaKXAkyQj2MjV", "signature should be irrelevant", b"message too", "ERR_ADDRESS_NO_KEY"),
                 ("1KqbBpLy5FARmTPD4VZnDDpYjkUvkr82Pm", "invalid signature, not in base64 encoding", b"message should be irrelevant", "ERR_MALFORMED_SIGNATURE"),
                 ("1KqbBpLy5FARmTPD4VZnDDpYjkUvkr82Pm", "A" * 87 + "=", b"message should be irrelevant", "ERR_PUBKEY_NOT_RECOVERED"),
                 ("15CRxFdyRpGZLW9w8HnHvVduizdL5jKNbs", sig, b"I never signed this", "ERR_NOT_SIGNED"),
                 ("15CRxFdyRpGZLW9w8HnHvVduizdL5jKNbs", sig, b"Trust no one", "OK"),
                 ("11canuhp9X2NocwCq7xNrQYTmUgZAnLK3", "IIcaIENoYW5jZWxsb3Igb24gYnJpbmsgb2Ygc2Vjb25kIGJhaWxvdXQgZm9yIGJhbmtzIAaHRtbCeDZINyavx14=", b"Trust me", "OK")]
        for address, signature, message, result in cases:
            self.assertIn(f'"{address}"', util)
            want, core = self.decide(address, signature, message, False, address)
            self.assertEqual(core, CORE[result], "Python's Core rules give Core's result")
            self.assertEqual(want, "BAD_HEADER" if signature.startswith("AAAA") else CORE[result])
        functional = (UPSTREAM / "bitcoin-core/rpc_signmessagewithprivkey.py").read_text()
        wif, message = "cUeKHd5orzT3mz8P9pxyREHfsWtVfgsfDjiZZBcjUBAaGk1BTj7N", b"This is just a test message"
        expected = "INbVnW4e6PeRmsv2Qgu8NuopvrVjkcxob+sX8OcZG0SALhWybUjzMLPdAsXI46YZGb0KQTRii+wWIQzRpG/U+S0="
        self.assertIn(wif, functional); self.assertIn(expected, functional)
        n = 0
        for c in wif: n = n * 58 + B58.index(c)
        d = n.to_bytes(38, "big")[1:33]
        self.assertEqual(self.message_sign(d, True, message), expected)
        point = mul(int.from_bytes(d, "big"), G)
        p2pkh = base58check(b"\x6f" + hash160(compress(point)))
        self.assertEqual(p2pkh, "mpLQjfK79b7CCV4VMJWEWAj5Mpx8Up5zxB")
        self.assertEqual(self.decide(p2pkh, expected, message, True, "functional")[0], "OK")
        self.assertEqual(self.decide(base58check(b"\xc4" + hash160(b"\0\x14" + hash160(compress(point)))), expected, message, True, "P2SH-P2WPKH")[0], "ADDRESS_NO_KEY")
        self.assertEqual(self.decide("invalid_addr", expected, message, True, "invalid_addr")[0], "INVALID_ADDRESS")
        self.assertEqual(self.decide(p2pkh, "invalid_sig", message, True, "invalid_sig")[0], "MALFORMED_SIGNATURE")
        self.wasm_message.append(dict(d=key.hex(), compressed=True, message=b"Trust no one".hex(), sig=sig))

    def test_random_messages(self):
        rng = random.Random(2222)
        lengths = [0, 1, 252, 253, 254, 65535, 65536, 70000]
        headers = {}
        for i in range(60):
            d = rng.randrange(1, N).to_bytes(32, "big")
            compressed = i % 3 != 0
            message = rng.randbytes(lengths[i] if i < len(lengths) else rng.randrange(0, 300))
            sig = self.message_sign(d, compressed, message)
            self.assertEqual(sig, self.python_sign(d, compressed, message), f"message {i}")
            point = mul(int.from_bytes(d, "big"), G)
            key = compress(point) if compressed else uncompressed(point)
            testnet = i % 2 == 1
            address = base58check(bytes([0x6f if testnet else 0]) + hash160(key))
            self.assertEqual(self.decide(address, sig, message, testnet, f"message {i}")[0], "OK")
            other = base58check(bytes([0x6f if testnet else 0]) + hash160(compress(point) if not compressed else uncompressed(point)))
            self.assertEqual(self.decide(other, sig, message, testnet, f"message {i}, the other form")[0], "NOT_SIGNED")
            self.assertEqual(self.decide(address, sig, message, not testnet, f"message {i}, other network")[0], "INVALID_ADDRESS")
            self.assertEqual(self.decide(address, sig, message + b"!", testnet, f"message {i}, changed")[0] in ("NOT_SIGNED", "NOT_RECOVERED"), True)
            if i < 6 and len(message) < 300:
                self.wasm_message.append(dict(d=d.hex(), compressed=compressed, message=message.hex(), sig=sig))
            if i == 1:
                raw = base64.b64decode(sig)
                for header in range(256):
                    edited = base64.b64encode(bytes([header]) + raw[1:]).decode()
                    want, core = self.decide(address, edited, message, testnet, f"header {header}")
                    headers[header] = (want, core)
        inside = [h for h in range(27, 35) if headers[h][0] == "OK"]
        self.assertEqual(len(inside), 1, "one header in 27 to 34 verifies")
        aliases = [h for h in range(256) if headers[h][1] == "OK" and not 27 <= h <= 34]
        print(f"\nCore's masking accepts {len(aliases)} header bytes outside 27 to 34 for one signature: {aliases}", file=sys.stderr)
        self.assertTrue(aliases and all(headers[h][0] == "BAD_HEADER" for h in aliases))

    def test_wasm(self):
        clang, sysroot, resources = Path(WASM_CLANG), Path(WASM_SYSROOT), Path(WASM_RESOURCES)
        self.assertTrue(clang.exists() and sysroot.exists() and resources.exists(), "a Wasm toolchain is required; see coax/web/wasm.sh")
        if not self.wasm_sign:
            self.test_rows(); self.test_refusals_and_edges(); self.test_core_cases(); self.test_random_messages()
        fixture = self.dir / "wasm.json"
        fixture.write_text(json.dumps(dict(recover=self.wasm_recover, sign=self.wasm_sign, verify=self.wasm_verify, message=self.wasm_message)))
        total = sum(len(x) for x in [self.wasm_recover, self.wasm_sign, self.wasm_verify, self.wasm_message])
        for label in ["checked", "release"]:
            module = self.dir / (label + ".wasm")
            run(clang, "--target=wasm32-wasip1", "--sysroot=" + str(sysroot), "-resource-dir=" + str(resources),
                "-std=c11", "-O2", "-I" + str(self.dir), "-mexec-model=reactor",
                self.dir / (label + ".c"), HERE / "signed_messages_wasm.c",
                "-Wl,--import-memory,--export-memory,--export=__heap_base,--max-memory=67108864,-z,stack-size=1048576,--gc-sections",
                "-o", module)
            result = json.loads(run("node", HERE / "signed_messages_wasm.cjs", module, fixture).stdout)
            self.assertEqual((result["agree"], result["cases"]), (total, total), label)
        print(f"\nWasm: {len(self.wasm_recover)} recoveries, {len(self.wasm_sign)} recoverable signatures, "
              f"{len(self.wasm_verify)} verifications and {len(self.wasm_message)} signed messages, checked and release; "
              f"oracle {DEFAULT_LIBRARY.name}", file=sys.stderr)


if __name__ == "__main__":
    unittest.main()
