#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: hashlib and OpenSSL decide each case outside Coax)
"""BIP-39 against a separate Python implementation and OpenSSL, natively and in Wasm.

Bitcoin From Scratch, chapter 31. Compiles coax/lib/crypto/bip39.coax to C
in checked and release builds and runs bip39_encode_ct, bip39_decode_ct and
bip39_seed_ct on:

- every English vector of trezor/python-mnemonic's vectors.json, and the
  first vector of each other language, all pinned in
  coax/tests/vectors/upstream/bip39/;
- phrases that use each of the 2,048 words, and seeded random entropy of
  every size with random ASCII passphrases;
- seeded edits of those phrases: a word replaced, swapped, removed or added,
  a letter changed or capitalized, a tab, a byte above 0x7f, extra spaces;
- the limits: 1,024 and 1,025 phrase bytes, 1,016 and 1,017 passphrase
  bytes, and output buffers one byte short.

Each result (the count written or the refusal code) and each output must
equal those of the reference below, which reads the pinned english.txt and
uses Python's hashlib (sha256 and pbkdf2_hmac) and shares no code with
Coax. Every seed the reference computes for the published vectors and a
sample of the random ones is also computed by the OpenSSL command line, a
second PBKDF2. The work buffer must come back as zeros. Then it builds the
same C for wasm32 and runs every case again in Node. From the repository
root:

    python3 coax/tests/bitcoin/bip39.py
"""
import ctypes as C
import hashlib
import json
import os
from pathlib import Path
import random
import shutil
import subprocess
import sys
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(ROOT / "coax/tests/nostr"))
from wasm_toolchain import WASM_CLANG, WASM_SYSROOT, WASM_RESOURCES  # noqa: E402

COMPILER = Path(os.environ.get("COAX_TEST_COMPILER", ROOT / "coax/bin/coax"))
SOURCE = ROOT / "coax/lib/crypto/bip39.coax"
UPSTREAM = ROOT / "coax/tests/vectors/upstream/bip39"
PINS = {
    "vectors.json": "fa3b937b7cff9c9b8ecd3aa011faeb8d6dd67993174b72326e83f4de8fdb30f8",
    "english.txt": "2f5eed53a4727b4bf8880d8f3f199efc90e58503646d9ff8eff3a2ed3b24dbda",
}
WORK = 1520
OUT = 2048

# ---- the Python reference
BAD_SIZE, BAD_COUNT, UNKNOWN_WORD, BAD_CHECKSUM, NOT_ASCII, CAPACITY, TOO_LONG = range(-1, -8, -1)


def read(name):
    raw = (UPSTREAM / name).read_bytes()
    assert hashlib.sha256(raw).hexdigest() == PINS[name], name
    return raw


WORDS = read("english.txt").decode().split("\n")[:-1]
INDEX = {w.encode(): i for i, w in enumerate(WORDS)}
assert len(WORDS) == 2048 == len(INDEX)


def encode(entropy, room):
    """(result, output, work touched)"""
    n = len(entropy)
    if n not in (16, 20, 24, 28, 32): return BAD_SIZE, b"", False
    cs = n // 4
    value = int.from_bytes(entropy, "big") << cs | hashlib.sha256(entropy).digest()[0] >> (8 - cs)
    count = n * 3 // 4
    text = " ".join(WORDS[value >> (11 * (count - 1 - i)) & 2047] for i in range(count)).encode()
    if room < len(text): return CAPACITY, b"", True
    return len(text), text, True


def read_phrase(phrase, room):
    """(entropy bytes or refusal, the words)"""
    if len(phrase) > 1024: return TOO_LONG, []
    if any(b > 0x7F for b in phrase): return NOT_ASCII, []
    words = [w for w in phrase.split(b" ") if w]
    if len(words) not in (12, 15, 18, 21, 24): return BAD_COUNT, []
    n = len(words) * 4 // 3
    if room < n: return CAPACITY, []
    if any(w not in INDEX for w in words): return UNKNOWN_WORD, []
    value = 0
    for w in words: value = value << 11 | INDEX[w]
    cs = len(words) // 3
    entropy = (value >> cs).to_bytes(n, "big")
    if hashlib.sha256(entropy).digest()[0] >> (8 - cs) != value & ((1 << cs) - 1): return BAD_CHECKSUM, []
    return entropy, words


def decode(phrase, room):
    r, _ = read_phrase(phrase, room)
    return (r, b"", True) if isinstance(r, int) else (len(r), r, True)


def seed(phrase, passphrase, room):
    if room < 64: return CAPACITY, b"", False
    if len(passphrase) > 1016: return TOO_LONG, b"", False
    if any(b > 0x7F for b in passphrase): return NOT_ASCII, b"", False
    r, words = read_phrase(phrase, 32)
    if isinstance(r, int): return r, b"", True
    return 64, hashlib.pbkdf2_hmac("sha512", b" ".join(words), b"mnemonic" + passphrase, 2048), True


def case(op, data, room=OUT, passphrase=b""):
    if op == "encode": result, output, touched = encode(data, room)
    elif op == "decode": result, output, touched = decode(data, room)
    else: result, output, touched = seed(data, passphrase, room)
    return dict(op=op, input=data.hex(), passphrase=passphrase.hex(), room=room,
                result=result, output=output.hex(), touched=touched)


def openssl_seed(phrase, passphrase):
    """The same PBKDF2 from the OpenSSL command line, for test data only."""
    r = subprocess.run(["openssl", "kdf", "-keylen", "64", "-kdfopt", "digest:SHA512",
                        "-kdfopt", "hexpass:" + phrase.hex(), "-kdfopt", "hexsalt:" + (b"mnemonic" + passphrase).hex(),
                        "-kdfopt", "iter:2048", "-binary", "PBKDF2"], capture_output=True, check=True)
    return r.stdout


def edit(rng, phrase):
    words = phrase.split(b" ")
    kind = rng.randrange(11)
    at = rng.randrange(len(words))
    if kind == 0: words[at] = rng.choice(WORDS).encode()
    elif kind == 1:
        other = rng.randrange(len(words)); words[at], words[other] = words[other], words[at]
    elif kind == 2: del words[at]
    elif kind == 3: words.insert(at, rng.choice(WORDS).encode())
    elif kind == 4: words[at] = words[at][:-1] + bytes([rng.randrange(97, 123)])
    elif kind == 5: words[at] = words[at][:1].upper() + words[at][1:]
    elif kind == 6: return phrase.replace(b" ", b"\t", 1)
    elif kind == 7: words[at] = words[at] + "é".encode()
    elif kind == 8: return b"  " + phrase.replace(b" ", b"   ") + b" "
    elif kind == 9: words[at] = words[at] + b"s"
    else: words[at] = words[at][:-1]
    return b" ".join(words)


def cases():
    out = []
    vectors = json.loads(read("vectors.json"))
    phrases = []
    for e, m, s, _x in vectors["english"]:
        out += [case("encode", bytes.fromhex(e)), case("decode", m.encode()),
                case("seed", m.encode(), passphrase=b"TREZOR")]
        phrases.append(m.encode())
    for lang, rows in vectors.items():
        if lang != "english":
            out += [case("decode", rows[0][1].encode()), case("seed", rows[0][1].encode(), passphrase=b"TREZOR")]
    rng = random.Random(31)
    # every word: 171 twelve-word phrases whose first 11 words run through the list
    for start in range(0, 2048, 11):
        value = 0
        for k in range(11): value = value << 11 | (start + k) % 2048
        value = value << 7 | rng.randrange(128)
        entropy = value.to_bytes(16, "big")
        out.append(case("encode", entropy))
        phrases.append(encode(entropy, OUT)[1])
    for i in range(300):
        entropy = rng.randbytes(rng.choice((16, 20, 24, 28, 32)))
        out.append(case("encode", entropy))
        phrases.append(encode(entropy, OUT)[1])
    for i, p in enumerate(phrases):
        out.append(case("decode", p))
        if i % 6 == 0:
            passphrase = bytes(rng.randrange(0x20, 0x7F) for _ in range(rng.randrange(40)))
            out.append(case("seed", p, passphrase=passphrase))
        e = edit(rng, p)
        out.append(case("decode", e))
        if i % 10 == 0: out.append(case("seed", e))
    # the limits
    long_phrase = phrases[0] + b" " * (1024 - len(phrases[0]))
    out += [case("decode", long_phrase), case("decode", long_phrase + b" "), case("seed", long_phrase + b" ")]
    out += [case("seed", phrases[0], passphrase=b"p" * 1016), case("seed", phrases[0], passphrase=b"p" * 1017),
            case("seed", phrases[0], passphrase="ü".encode()), case("seed", phrases[0], room=63),
            case("seed", phrases[0], passphrase=bytes(range(0x80)))]
    for p in phrases[:24]:
        out += [case("encode", read_phrase(p, 32)[0], room=len(p)), case("encode", read_phrase(p, 32)[0], room=len(p) - 1),
                case("decode", p, room=len(p.split()) * 4 // 3 - 1)]
    out += [case("encode", rng.randbytes(n)) for n in (0, 4, 12, 15, 17, 31, 33, 36, 64)]
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
    """One case through a native build: (result, output hex, work zero when touched)."""
    data, passphrase = bytes.fromhex(c["input"]), bytes.fromhex(c["passphrase"])
    inp = (C.c_uint8 * max(1, len(data))).from_buffer_copy(data.ljust(1, b"\0"))
    pas = (C.c_uint8 * max(1, len(passphrase))).from_buffer_copy(passphrase.ljust(1, b"\0"))
    out = (C.c_uint8 * OUT)(*([0xA5] * OUT))
    work = (C.c_uint8 * WORK)(*([0x5A] * WORK))
    i, w, o = view(inp, len(data)), view(work, WORK), view(out, c["room"])
    if c["op"] == "encode": r = lib.bip39_encode_ct(i, w, o)
    elif c["op"] == "decode": r = lib.bip39_decode_ct(i, w, o)
    else: r = lib.bip39_seed_ct(i, view(pas, len(passphrase)), w, o)
    wiped = all(b == 0 for b in work) if c["touched"] else all(b == 0x5A for b in work)
    return r, bytes(out[:max(0, r)]).hex(), wiped and all(b == 0xA5 for b in out[max(0, r):])


class Bip39(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix="coax-bip39-")
        cls.dir = Path(cls.temp.name)
        cls.libs = []
        for label, mode in [("checked", []), ("release", ["--release"])]:
            c = cls.dir / (label + ".c")
            c.write_text(run(COMPILER, "c", SOURCE, "--library", "--header", cls.dir / "bip39.h", *mode).stdout)
            run("cc", "-std=c11", "-O2", "-shared", "-fPIC", c, "-lm", "-o", cls.dir / (label + ".dylib"))
            lib = C.CDLL(str(cls.dir / (label + ".dylib")))
            lib.bip39_encode_ct.argtypes = [View, View, View]
            lib.bip39_decode_ct.argtypes = [View, View, View]
            lib.bip39_seed_ct.argtypes = [View, View, View, View]
            for name in ["bip39_encode_ct", "bip39_decode_ct", "bip39_seed_ct"]:
                getattr(lib, name).restype = C.c_int64
            cls.libs.append((label, lib))
        cls.cases = cases()

    @classmethod
    def tearDownClass(cls):
        cls.temp.cleanup()

    def test_reference_on_published_vectors(self):
        """The Python reference gives every published English answer."""
        for e, m, s, _x in json.loads(read("vectors.json"))["english"]:
            self.assertEqual(encode(bytes.fromhex(e), OUT)[1], m.encode())
            self.assertEqual(decode(m.encode(), OUT)[1], bytes.fromhex(e))
            self.assertEqual(seed(m.encode(), b"TREZOR", OUT)[1], bytes.fromhex(s))

    def test_openssl_agrees(self):
        """OpenSSL's PBKDF2 gives every seed of the published vectors and a sample of the rest."""
        self.assertIsNotNone(shutil.which("openssl"), "the OpenSSL command line is required")
        seeds = [c for c in self.cases if c["op"] == "seed" and c["result"] == 64]
        sample = seeds[:24] + seeds[24::5]
        for c in sample:
            phrase = b" ".join(w for w in bytes.fromhex(c["input"]).split(b" ") if w)
            self.assertEqual(openssl_seed(phrase, bytes.fromhex(c["passphrase"])).hex(), c["output"])
        print(f"\nOpenSSL agrees on {len(sample)} seeds", file=sys.stderr)

    def test_native(self):
        for label, lib in self.libs:
            for c in self.cases:
                self.assertEqual(native(lib, c), (c["result"], c["output"], True), f"{label} {c['op']} {c['input']}")
        results = {c["result"] for c in self.cases}
        self.assertTrue({BAD_SIZE, BAD_COUNT, UNKNOWN_WORD, BAD_CHECKSUM, NOT_ASCII, CAPACITY, TOO_LONG} <= results)

    def test_wasm(self):
        clang, sysroot, resources = Path(WASM_CLANG), Path(WASM_SYSROOT), Path(WASM_RESOURCES)
        self.assertTrue(clang.exists() and sysroot.exists() and resources.exists(), "a Wasm toolchain is required; see coax/web/wasm.sh")
        fixture = self.dir / "wasm.json"
        fixture.write_text(json.dumps(self.cases))
        for label in ["checked", "release"]:
            module = self.dir / (label + ".wasm")
            run(clang, "--target=wasm32-wasip1", "--sysroot=" + str(sysroot), "-resource-dir=" + str(resources),
                "-std=c11", "-O2", "-I" + str(self.dir), "-mexec-model=reactor",
                self.dir / (label + ".c"), HERE / "bip39_wasm.c",
                "-Wl,--import-memory,--export-memory,--export=__heap_base,--max-memory=67108864,-z,stack-size=1048576,--gc-sections",
                "-o", module)
            result = json.loads(run("node", HERE / "bip39_wasm.cjs", module, fixture).stdout)
            self.assertEqual((result["agree"], result["cases"]), (len(self.cases), len(self.cases)), label)
        print(f"\n{len(self.cases)} cases agree with Python, native checked/release and Wasm checked/release",
              file=sys.stderr)


if __name__ == "__main__":
    unittest.main()
