#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Python and libsecp256k1 decide each case outside Coax)
"""Strict DER against a Python reference and libsecp256k1, natively and in Wasm.

Bitcoin From Scratch, chapter 21. Compiles coax/lib/bitcoin/der.coax to C in
checked and release builds and runs a corpus of script signatures: every
stack item of the pinned script_tests.json rows, the bitcoinjs/bip66
fixtures, the library's own check vectors, every one-byte change, insertion
and deletion of well-formed signatures, structured random encodings and
random bytes. For each input it compares

- der_check_vartime's refusal code with der_reference.py's transliteration
  of IsValidSignatureEncoding, rule by rule;
- der_signature_check_vartime under all eight flag sets with the Python
  CheckSignatureEncoding;
- der_read_lax_vartime with the Python lax parser and with libsecp256k1's
  contrib/lax_der_parsing.c (the parser Bitcoin Core copied), byte for byte;
- the BIP-66 verdict with libsecp256k1's strict parser: where it reads
  nonzero r and s, BIP-66 accepts exactly the encodings that
  secp256k1_ecdsa_signature_serialize_der gives back unchanged;
- chapter 19's ecdsa_der_read_vartime, which may accept only what BIP-66 does.

It runs all 89 script_tests.json rows through the Python reference, and
checks each signature verification the rows need with libsecp256k1 and the
Coax build (der_checksig_vartime). Then it builds the C for wasm32 and
compares the codes, the script checks and the lax reads in Node. Build
libsecp256k1 first with tools/coax-nostr/fuzz/build_oracle.py --fetch
--reviewed. Run from the repository root:

    python3 coax/tests/bitcoin/der.py
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
sys.path.insert(0, str(ROOT / "tools/coax-nostr/fuzz"))
from wasm_toolchain import WASM_CLANG, WASM_SYSROOT, WASM_RESOURCES  # noqa: E402
from oracle import DEFAULT_LIBRARY  # noqa: E402
import der_reference as ref  # noqa: E402
import make_der_vectors  # noqa: E402

COMPILER = Path(os.environ.get("COAX_TEST_COMPILER", ROOT / "coax/bin/coax"))
LIBRARY = ROOT / "coax/lib/bitcoin/der.coax"
SECP_SOURCE = ROOT / "tools/coax-nostr/fuzz/.cache/source"
N = ref.N


def run(*args, **kw):
    r = subprocess.run([str(a) for a in args], capture_output=True, text=True, **kw)
    if r.returncode:
        raise RuntimeError(f"{args[0]} failed ({r.returncode}): {r.stderr or r.stdout}")
    return r


def code(sig):
    """IsValidSignatureEncoding with the chapter's refusal codes, in Core's order."""
    n = len(sig)
    if n < 9: return 1
    if n > 73: return 2
    if sig[0] != 0x30: return 3
    if sig[1] != n - 3: return 4
    len_r = sig[3]
    if 5 + len_r >= n: return 5
    len_s = sig[5 + len_r]
    if len_r + len_s + 7 != n: return 6
    for tag, first in ((2, 7), (len_r + 4, 11)):
        length, value = sig[tag + 1], tag + 2
        if sig[tag] != 2: return first
        if length == 0: return first + 1
        if sig[value] & 0x80: return first + 2
        if length > 1 and sig[value] == 0 and not sig[value + 1] & 0x80: return first + 3
    return 0


SCRIPT_ERRORS = {None: 0, "SIG_DER": 1, "SIG_HIGH_S": 2, "SIG_HASHTYPE": 3}


def der(r, s):
    def integer(v):
        b = v.to_bytes(32, "big").lstrip(b"\0") or b"\0"
        if b[0] & 0x80: b = b"\0" + b
        return bytes([2, len(b)]) + b
    body = integer(r) + integer(s)
    return bytes([0x30, len(body)]) + body


def corpus(rows, seed=21):
    """Script signatures (DER plus a sighash byte) and other byte strings."""
    rng = random.Random(seed)
    items = set()
    for row in rows:
        items.update(bytes.fromhex(x) for x in row["stack"])
    valid, invalid = make_der_vectors.bip66()
    items.update(bytes.fromhex(d) + b"\x01" for d, _, _ in valid)
    items.update(bytes.fromhex(d) + b"\x01" for d, _ in invalid)
    text = LIBRARY.read_text()
    import re
    items.update(bytes.fromhex(h) for h in re.findall(r'"((?:[0-9a-f]{2})+)"', text) if len(h) <= 160)
    seeds = [der(rng.randrange(1, N), rng.randrange(1, N)) + bytes([rng.choice([1, 2, 3, 0x81, 0x82, 0x83])]) for _ in range(40)]
    seeds += [der(1, 1) + b"\x01", der(0x80, 0x7f) + b"\x01", der(N - 1, N // 2 + 1) + b"\x01"]
    for s in seeds:
        items.add(s)
        for i in range(len(s)):
            for v in {0, 1, 0x7f, 0x80, 0xff, s[i] ^ 1, s[i] + 1 & 0xff, s[i] - 1 & 0xff}:
                items.add(s[:i] + bytes([v]) + s[i + 1:])
            items.add(s[:i] + s[i + 1:])
            items.add(s[:i] + bytes([rng.randrange(256)]) + s[i:])
            items.add(s[:i])
    for _ in range(6000):
        # Structured: a sequence of two integers with lengths, tags, padding
        # and the sequence length each sometimes wrong, and sometimes long form.
        parts = []
        for _ in range(2):
            length = rng.choice([0, 1, 2, 31, 32, 33, 34, rng.randrange(0, 40)])
            body = bytes(rng.choice([0, 0, 0x7f, 0x80, 0xff, rng.randrange(256)]) for _ in range(length))
            tag = 2 if rng.random() < 0.9 else rng.randrange(256)
            if rng.random() < 0.1 and length < 128:
                parts.append(bytes([tag, 0x81, length]) + body)
            else:
                parts.append(bytes([tag, length if rng.random() < 0.9 else rng.randrange(256)]) + body)
        body = b"".join(parts)
        total = len(body) if rng.random() < 0.85 else rng.randrange(256)
        head = bytes([0x30 if rng.random() < 0.95 else rng.randrange(256), total & 0xff])
        if rng.random() < 0.05: head = bytes([0x30, 0x81, total & 0xff])
        tail = bytes(rng.randrange(256) for _ in range(rng.choice([0, 0, 0, 1, 2])))
        items.add(head + body + bytes([rng.randrange(256)]) + tail)
    for _ in range(3000):
        items.add(bytes(rng.randrange(256) for _ in range(rng.randrange(0, 80))))
    return sorted(items, key=lambda b: (len(b), b))


class View(C.Structure):
    _fields_ = [("data", C.POINTER(C.c_uint8)), ("count", C.c_int64)]


def view(data):
    buffer = (C.c_uint8 * max(1, len(data)))(*data)
    return View(C.cast(buffer, C.POINTER(C.c_uint8)), len(data)), buffer


def build(directory, label, mode):
    c = directory / (label + ".c")
    c.write_text(run(COMPILER, "c", LIBRARY, "--library", "--header", directory / "der.h", *mode).stdout)
    run("cc", "-std=c11", "-O2", "-shared", "-fPIC", c, "-lm", "-o", directory / (label + ".dylib"))
    lib = C.CDLL(str(directory / (label + ".dylib")))
    for name, args, result in [("der_check_vartime", [View], C.c_int64),
                               ("der_signature_check_vartime", [View, C.c_int64], C.c_int64),
                               ("der_read_lax_vartime", [View, View], C.c_bool),
                               ("der_low_s_vartime", [View], C.c_bool),
                               ("der_checksig_vartime", [View, View, View], C.c_bool),
                               ("ecdsa_der_read_vartime", [View, View], C.c_bool)]:
        getattr(lib, name).argtypes = args
        getattr(lib, name).restype = result
    return lib


class Coax:
    def __init__(self, lib):
        self.lib = lib

    def code(self, sig):
        v, _b = view(sig)
        return self.lib.der_check_vartime(v)

    def script(self, sig, flags):
        v, _b = view(sig)
        return self.lib.der_signature_check_vartime(v, flags)

    def low_s(self, sig):
        v, _b = view(sig)
        return self.lib.der_low_s_vartime(v)

    def lax(self, der_bytes):
        (d, _a), (o, out) = view(der_bytes), view(bytes(64))
        return bytes(out) if self.lib.der_read_lax_vartime(d, o) else None

    def strict19(self, der_bytes):
        (d, _a), (o, out) = view(der_bytes), view(bytes(64))
        return bytes(out) if self.lib.ecdsa_der_read_vartime(d, o) else None

    def checksig(self, digest, key, sig):
        (h, _a), (k, _b), (s, _c) = view(digest), view(key), view(sig)
        return self.lib.der_checksig_vartime(h, k, s)


class Libsecp:
    def __init__(self, directory):
        lax = directory / "lax.dylib"
        run("cc", "-std=c11", "-O2", "-shared", "-fPIC", "-I" + str(SECP_SOURCE / "include"),
            SECP_SOURCE / "contrib/lax_der_parsing.c", DEFAULT_LIBRARY, "-o", lax)
        self.lib = C.CDLL(str(DEFAULT_LIBRARY))
        self.lax_lib = C.CDLL(str(lax))
        L = self.lib
        for name, args in [("secp256k1_context_create", [C.c_uint]),
                           ("secp256k1_ecdsa_signature_parse_der", [C.c_void_p, C.c_void_p, C.c_char_p, C.c_size_t]),
                           ("secp256k1_ecdsa_signature_serialize_der", [C.c_void_p, C.c_void_p, C.c_void_p, C.c_void_p]),
                           ("secp256k1_ecdsa_signature_serialize_compact", [C.c_void_p, C.c_void_p, C.c_void_p]),
                           ("secp256k1_ecdsa_signature_normalize", [C.c_void_p, C.c_void_p, C.c_void_p]),
                           ("secp256k1_ecdsa_verify", [C.c_void_p, C.c_char_p, C.c_char_p, C.c_void_p]),
                           ("secp256k1_ec_pubkey_parse", [C.c_void_p, C.c_void_p, C.c_char_p, C.c_size_t])]:
            getattr(L, name).argtypes = args
        L.secp256k1_context_create.restype = C.c_void_p
        self.lax_lib.ecdsa_signature_parse_der_lax.argtypes = [C.c_void_p, C.c_void_p, C.c_char_p, C.c_size_t]
        self.ctx = L.secp256k1_context_create(1)

    def compact(self, sig):
        out = C.create_string_buffer(64)
        assert self.lib.secp256k1_ecdsa_signature_serialize_compact(self.ctx, out, sig)
        return out.raw

    def lax(self, data):
        sig = C.create_string_buffer(64)
        if not self.lax_lib.ecdsa_signature_parse_der_lax(self.ctx, sig, data or b"\0", len(data)):
            return None
        return self.compact(sig)

    def strict_roundtrip(self, data):
        """(r || s, re-serialized DER) from the strict parser, or None."""
        sig = C.create_string_buffer(64)
        if not self.lib.secp256k1_ecdsa_signature_parse_der(self.ctx, sig, data or b"\0", len(data)):
            return None
        out, size = C.create_string_buffer(80), C.c_size_t(80)
        assert self.lib.secp256k1_ecdsa_signature_serialize_der(self.ctx, out, C.byref(size), sig)
        return self.compact(sig), out.raw[:size.value]

    def checksig(self, digest, key, sig):
        """Core's CheckECDSASignature after the sighash: lax parse, normalize, verify."""
        L, parsed, pub = self.lib, C.create_string_buffer(64), C.create_string_buffer(64)
        if not sig or not L.secp256k1_ec_pubkey_parse(self.ctx, pub, key or b"\0", len(key)): return False
        if not self.lax_lib.ecdsa_signature_parse_der_lax(self.ctx, parsed, sig[:-1] or b"\0", len(sig) - 1): return False
        L.secp256k1_ecdsa_signature_normalize(self.ctx, parsed, parsed)
        return bool(L.secp256k1_ecdsa_verify(self.ctx, parsed, digest, pub))


def python_lax(data):
    rs = ref.lax(data)
    return None if rs is None else rs[0].to_bytes(32, "big") + rs[1].to_bytes(32, "big")


class StrictDer(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        assert DEFAULT_LIBRARY.exists(), "build libsecp256k1 first: python3 tools/coax-nostr/fuzz/build_oracle.py --fetch --reviewed"
        cls.temp = tempfile.TemporaryDirectory(prefix="coax-der-")
        cls.dir = Path(cls.temp.name)
        cls.secp = Libsecp(cls.dir)
        cls.builds = [Coax(build(cls.dir, label, mode)) for label, mode in [("checked", []), ("release", ["--release"])]]
        cls.rows = make_der_vectors.rows()
        cls.inputs = corpus(cls.rows)

    @classmethod
    def tearDownClass(cls):
        cls.temp.cleanup()

    def test_table_is_current(self):
        # The checked-in table is exactly what the generator writes from the pinned files.
        text = make_der_vectors.render(self.rows, *make_der_vectors.bip66())
        self.assertEqual((HERE / "der_vectors.coax").read_text(), text)

    def test_encodings(self):
        accepted = lax_read = strict_cases = 0
        for sig in self.inputs:
            want = code(sig)
            self.assertEqual(want == 0, ref.valid_encoding(sig), sig.hex())
            body = sig[:-1] if sig else b""
            secp_lax = self.secp.lax(sig)
            self.assertEqual(secp_lax, python_lax(sig), "lax: " + sig.hex())
            for coax in self.builds:
                self.assertEqual(coax.code(sig), want, sig.hex())
                for flags in range(8):
                    self.assertEqual(coax.script(sig, flags), SCRIPT_ERRORS[ref.signature_error(sig, flags)], f"{sig.hex()} flags {flags}")
                self.assertEqual(coax.low_s(sig), ref.low_s(sig) if sig else False, sig.hex())
                self.assertEqual(coax.lax(sig), secp_lax, "lax: " + sig.hex())
                read = coax.strict19(body)
                if read is not None:
                    self.assertEqual(want, 0, "chapter 19 reads what BIP-66 refuses: " + sig.hex())
            accepted += want == 0
            lax_read += secp_lax is not None
            if 9 <= len(sig) <= 73:
                parsed = self.secp.strict_roundtrip(body)
                if parsed and parsed[0][:32] != bytes(32) and parsed[0][32:] != bytes(32):
                    strict_cases += 1
                    self.assertEqual(want == 0, parsed[1] == body, "libsecp256k1 strict: " + sig.hex())
        self.assertGreater(accepted, 200)
        print(f"\n{len(self.inputs)} inputs: {accepted} pass BIP-66, {lax_read} read laxly, "
              f"{strict_cases} compared with libsecp256k1's strict parser", file=sys.stderr)

    def test_rows(self):
        verified = 0
        for row in self.rows:
            stack = [bytes.fromhex(x) for x in row["stack"]]
            hashes = [bytes.fromhex(x) for x in row["hashes"]]
            calls = []
            checksig = ref.checksig

            def spy(sig, key, digest):
                result = checksig(sig, key, digest)
                calls.append((sig, key, digest, result))
                return result
            ref.checksig = spy
            try:
                got = ref.run_row(stack, hashes, row["flags"], row["multisig"], row["negate"])
            finally:
                ref.checksig = checksig
            self.assertEqual(got, row["expected"], f"row {row['line']}")
            for sig, key, digest, result in calls:
                self.assertEqual(self.secp.checksig(digest, key, sig), result, f"row {row['line']} (libsecp256k1)")
                for coax in self.builds:
                    self.assertEqual(coax.checksig(digest, key, sig), result, f"row {row['line']} (Coax)")
                verified += result
        self.assertEqual((len(self.rows), verified), (89, 17))

    def test_wasm(self):
        clang, sysroot, resources = Path(WASM_CLANG), Path(WASM_SYSROOT), Path(WASM_RESOURCES)
        self.assertTrue(clang.exists() and sysroot.exists() and resources.exists(), "a Wasm toolchain is required; see coax/web/wasm.sh")
        fixture = self.dir / "wasm.json"
        cases = [dict(sig=s.hex(), code=code(s), script=[SCRIPT_ERRORS[ref.signature_error(s, f)] for f in range(8)],
                      lax=(python_lax(s) or b"").hex(), read=python_lax(s) is not None) for s in self.inputs]
        fixture.write_text(json.dumps(cases))
        for label in ["checked", "release"]:
            module = self.dir / (label + ".wasm")
            run(clang, "--target=wasm32-wasip1", "--sysroot=" + str(sysroot), "-resource-dir=" + str(resources),
                "-std=c11", "-O2", "-I" + str(self.dir), "-mexec-model=reactor",
                self.dir / (label + ".c"), HERE / "der_wasm.c",
                "-Wl,--import-memory,--export-memory,--export=__heap_base,--max-memory=67108864,-z,stack-size=1048576,--gc-sections",
                "-o", module)
            result = json.loads(run("node", HERE / "der_wasm.cjs", module, fixture).stdout)
            self.assertEqual((result["agree"], result["cases"]), (len(cases), len(cases)), label)
        print(f"\n{len(cases)} inputs agree in Wasm, checked and release; oracle {DEFAULT_LIBRARY.name}", file=sys.stderr)


if __name__ == "__main__":
    unittest.main()
