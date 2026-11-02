#!/usr/bin/env python3
"""Poly1305 and ChaCha20-Poly1305 against RFC 8439, Wycheproof and OpenSSL, natively and in Wasm.

Bitcoin From Scratch, chapter 12. Reads every Poly1305 and AEAD example of
RFC 8439 (sections 2.5.2, 2.6.2 and 2.8.2, appendices A.3, A.4 and A.5) from
the pinned RFC text, all 325 cases of the pinned Wycheproof
chacha20_poly1305_test.json, random Poly1305 tags from an arbitrary-precision
Python model, and random AEAD boxes sealed by OpenSSL through Node. Compiles
coax/lib/crypto/chacha20_poly1305.coax to C in checked and release builds and
runs every case through the one-shot MAC, the Poly1305 stream at four chunk
sizes, seal and open. Each valid box also opens with one tag bit flipped,
which must refuse and leave the output unwritten. Then it builds the same C
for wasm32 and runs every case again in Node. Run from the repository root:

    python3 coax/tests/bitcoin/chacha20_poly1305.py
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
from wasm_toolchain import WASM_CLANG, WASM_SYSROOT, WASM_RESOURCES  # noqa: E402

COMPILER = Path(os.environ.get("COAX_TEST_COMPILER", ROOT / "coax/bin/coax"))
SOURCE = ROOT / "coax/lib/crypto/chacha20_poly1305.coax"
RFC = ROOT / "coax/tests/vectors/upstream/rfc/rfc8439.txt"
RFC_SHA256 = "25bef70fbf7a07ff45c2fe4cb7c6ce954eac687413d8610603268b4e4415324c"
WYCHEPROOF = ROOT / "coax/tests/vectors/upstream/wycheproof/chacha20_poly1305_test.json"
WYCHEPROOF_SHA256 = "fe61d25f90e1bde4461d00eafe61049e5f29bd999f36b766df9cda90906ad53d"
CCP_NONCE, CCP_TAG = 2, 7


def run(*args, **kw):
    r = subprocess.run([str(a) for a in args], capture_output=True, text=True, **kw)
    if r.returncode:
        raise RuntimeError(f"{args[0]} failed ({r.returncode}): {r.stderr or r.stdout}")
    return r


def poly_model(key, msg):
    """Poly1305 with Python integers, straight from RFC 8439 section 2.5.1."""
    r = int.from_bytes(key[:16], "little") & 0x0ffffffc0ffffffc0ffffffc0fffffff
    s, acc, p = int.from_bytes(key[16:], "little"), 0, (1 << 130) - 5
    for at in range(0, len(msg), 16):
        acc = (acc + int.from_bytes(msg[at:at + 16] + b"\x01", "little")) * r % p
    return ((acc + s) % (1 << 128)).to_bytes(16, "little")


# RFC text parsing. A field is the hex bytes on the lines after its label:
# "000  4c 61 ..." rows with an offset, bare "FF FF ..." rows, or "1a:e1:..",
# on the label's line or the lines after it, up to the first other line.
ROW = re.compile(r"^\s*\d{3}  ((?:[0-9a-fA-F]{2}(?: |$)){1,16})")
BARE = re.compile(r"^\s*((?:[0-9a-fA-F]{2}[ :]?)+)\s*$")


def field(text, label):
    out = bytearray()
    # The bytes start on the label's own line (2.5.2's tag) or the next one.
    for line in text.split(label, 1)[1].splitlines():
        m = ROW.match(line) or BARE.match(line)
        if not m:
            if out:
                break
            continue
        out += bytes.fromhex(m.group(1).replace(":", " "))
    return bytes(out)


def section(text, start, end):
    body = text.split("\n" + start + "  ")[-1]
    return body.split("\n" + end + "  ", 1)[0]


def rfc_vectors():
    raw = RFC.read_bytes()
    assert hashlib.sha256(raw).hexdigest() == RFC_SHA256
    text = re.sub(r"\n+[^\n]*\[Page \d+\]\n\x0c\nRFC 8439 [^\n]*\n+", "\n", raw.decode())
    polys, keygens, aeads = [], [], []
    s252 = section(text, "2.5.2.", "2.6.")
    # The key is one colon-separated run that wraps onto a second line.
    material = re.sub(r"\s", "", s252.split("Key Material:", 1)[1].split("o  s as", 1)[0])
    polys.append(dict(id="2.5.2", key=bytes.fromhex(material.replace(":", "")),
                      msg=field(s252, "Message to be Authenticated:"), tag=field(s252, "Tag:")))
    a3 = section(text, "A.3.", "A.4.")
    for number, case in re.findall(r"Test Vector #(\d+):(.*?)(?=Test Vector #|\Z)", a3, re.S):
        if "One-time Poly1305 Key:" in case:
            key, msg = field(case, "One-time Poly1305 Key:"), field(case, "Text to MAC:")
        else:
            key, msg = field(case, "R:") + field(case, "S:"), field(case, "data:")
        polys.append(dict(id="A.3 #" + number, key=key, msg=msg, tag=field(case, "Tag:" if "Tag:" in case else "tag:")))
    s262 = section(text, "2.6.2.", "2.7.")
    keygens.append(dict(id="2.6.2", key=field(s262, "Key:"), nonce=field(s262, "Nonce:"), polykey=field(s262, "Output bytes:")))
    a4 = section(text, "A.4.", "A.5.")
    for number, case in re.findall(r"Test Vector #(\d+):(.*?)(?=Test Vector #|\Z)", a4, re.S):
        keygens.append(dict(id="A.4 #" + number, key=field(case, "The ChaCha20 Key"), nonce=field(case, "The nonce:"),
                            polykey=field(case, "Poly1305 one-time key:")))
    s282 = section(text, "2.8.2.", "3.")
    aeads.append(dict(id="2.8.2", key=field(s282, "Key:"), nonce=field(s282, "32-bit fixed-common part:") + field(s282, "IV:"),
                      aad=field(s282, "AAD:"), msg=field(s282, "Plaintext:"), ct=field(s282, "Ciphertext:"),
                      tag=field(s282, "Tag:"), result="valid"))
    a5 = section(text, "A.5.", "Appendix B.")
    aeads.append(dict(id="A.5", key=field(a5, "The ChaCha20 Key"), nonce=field(a5, "The nonce:"), aad=field(a5, "The AAD:"),
                      msg=field(a5, "Plaintext::"), ct=field(a5, "Ciphertext:"), tag=field(a5, "Received Tag:"), result="valid"))
    assert [len(polys), len(keygens), len(aeads)] == [12, 4, 2]
    assert all(len(v["key"]) == 32 and len(v["tag"]) == 16 for v in polys)
    assert all(len(v["key"]) == 32 and len(v["nonce"]) == 12 and len(v["polykey"]) == 32 for v in keygens)
    assert [len(v["msg"]) for v in aeads] == [len(v["ct"]) for v in aeads] == [114, 265]
    return polys, keygens, aeads


def wycheproof_vectors():
    raw = WYCHEPROOF.read_bytes()
    assert hashlib.sha256(raw).hexdigest() == WYCHEPROOF_SHA256
    doc = json.loads(raw)
    cases = []
    for group in doc["testGroups"]:
        for t in group["tests"]:
            assert t["result"] in ("valid", "invalid")
            cases.append(dict(id=f"wycheproof #{t['tcId']}", key=bytes.fromhex(t["key"]), nonce=bytes.fromhex(t["iv"]),
                              aad=bytes.fromhex(t["aad"]), msg=bytes.fromhex(t["msg"]), ct=bytes.fromhex(t["ct"]),
                              tag=bytes.fromhex(t["tag"]), result=t["result"],
                              status=CCP_NONCE if group["ivSize"] != 96 else CCP_TAG))
    assert len(cases) == doc["numberOfTests"] == 325
    return cases


class View(C.Structure):
    _fields_ = [("data", C.POINTER(C.c_uint8)), ("count", C.c_int64)]


def array(data):
    return (C.c_uint8 * max(1, len(data)))(*data)


def view(data, count=None):
    return View(C.cast(data, C.POINTER(C.c_uint8)), len(data) if count is None else count)


def wire(c):
    return {k: v.hex() if isinstance(v, bytes) else v for k, v in c.items()}


class ChaCha20Poly1305(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix="coax-ccp-")
        cls.dir = Path(cls.temp.name)
        polys, keygens, aeads = rfc_vectors()
        rng = random.Random(8439)
        for n in [0, 1, 15, 16, 17, 31, 32, 33, 63, 64, 65, 255, 256, 257, 1000] + [rng.randrange(1024) for _ in range(85)]:
            key, msg = rng.randbytes(32), rng.randbytes(n)
            polys.append(dict(id=f"model {n}", key=key, msg=msg, tag=poly_model(key, msg)))
        for v in polys:
            assert poly_model(v["key"], v["msg"]) == v["tag"], v["id"]
        # Section 2.6.2 and A.4: with no AAD and no message, the tag is
        # Poly1305 of the 16-byte length block under the one-time key.
        for v in keygens:
            aeads.append(dict(id=v["id"] + " key generation", key=v["key"], nonce=v["nonce"], aad=b"", msg=b"", ct=b"",
                              tag=poly_model(v["polykey"], bytes(16)), result="valid"))
        aeads += wycheproof_vectors()
        requests = []
        for n in [0, 1, 15, 16, 17, 63, 64, 65, 127, 128, 129, 4095, 4096, 4097] + [rng.randrange(2048) for _ in range(106)]:
            requests.append(dict(id=f"openssl {len(requests)}", key=rng.randbytes(32), nonce=rng.randbytes(12),
                                 aad=rng.randbytes(rng.choice([0, 1, 12, 16, 17, rng.randrange(300)])), msg=rng.randbytes(n)))
        request = cls.dir / "request.json"
        request.write_text(json.dumps([wire(c) for c in requests]))
        oracle = json.loads(run("node", HERE / "chacha20_poly1305_wasm.cjs", "oracle", request).stdout)
        cls.engine = oracle["engine"]
        for c, box in zip(requests, oracle["boxes"], strict=True):
            aeads.append(dict(c, ct=bytes.fromhex(box["ct"]), tag=bytes.fromhex(box["tag"]), result="valid"))
        for v in aeads:
            v["flip"] = rng.randrange(128)
        cls.polys, cls.aeads = polys, aeads
        cls.libs = []
        for label, mode in [("checked", []), ("release", ["--release"])]:
            c = cls.dir / (label + ".c")
            c.write_text(run(COMPILER, "c", SOURCE, "--library", "--header", cls.dir / "chacha20_poly1305.h", *mode).stdout)
            run("cc", "-std=c11", "-O2", "-shared", "-fPIC", c, "-lm", "-o", cls.dir / (label + ".dylib"))
            lib = C.CDLL(str(cls.dir / (label + ".dylib")))
            for name in ["chacha20_poly1305_seal", "chacha20_poly1305_open"]:
                getattr(lib, name).argtypes = [View, View, View, View, View, C.c_bool]
                getattr(lib, name).restype = C.c_uint32
            lib.poly1305_checked.argtypes = [View, View, View, C.c_bool]
            lib.poly1305_checked.restype = C.c_bool
            lib.poly1305_init.argtypes = [View, View, C.c_bool]
            for name in ["poly1305_update", "poly1305_final"]:
                getattr(lib, name).argtypes = [View, View, C.c_bool]
            for name in ["poly1305_init", "poly1305_update", "poly1305_final"]:
                getattr(lib, name).restype = C.c_uint32
            cls.libs.append(lib)

    @classmethod
    def tearDownClass(cls):
        cls.temp.cleanup()

    def test_poly1305(self):
        for lib in self.libs:
            for v in self.polys:
                key, msg = array(v["key"]), array(v["msg"])
                out = array([0xA5] * 24)
                self.assertTrue(lib.poly1305_checked(view(key), view(msg, len(v["msg"])), view(out), False))
                self.assertEqual(bytes(out)[:16], v["tag"], v["id"])
                for chunk in [1, 7, 16, 17]:
                    state = array([0] * 168)
                    self.assertEqual(lib.poly1305_init(view(state, 1), view(key), False), 0)
                    for at in range(0, len(v["msg"]), chunk):
                        piece = array(v["msg"][at:at + chunk])
                        self.assertEqual(lib.poly1305_update(view(state, 1), view(piece, min(chunk, len(v["msg"]) - at)), False), 0)
                    out = array([0xA5] * 24)
                    self.assertEqual(lib.poly1305_final(view(state, 1), view(out, 16), False), 0)
                    self.assertEqual(bytes(out), v["tag"] + b"\xa5" * 8, v["id"])
                    self.assertEqual(bytes(state), bytes(168), "final erases the state")

    def test_aead(self):
        counts = {"valid": 0, "invalid": 0}
        for lib in self.libs:
            for v in self.aeads:
                key, nonce, aad = array(v["key"]), array(v["nonce"]), array(v["aad"])
                k, n, a = view(key), view(nonce, len(v["nonce"])), view(aad, len(v["aad"]))
                box = v["ct"] + v["tag"]
                size = len(v["msg"])
                if v["result"] == "valid":
                    plain, out = array(v["msg"]), array([0xA5] * (len(box) + 8))
                    self.assertEqual(lib.chacha20_poly1305_seal(k, n, a, view(plain, size), view(out), False), 0, v["id"])
                    self.assertEqual(bytes(out), box + b"\xa5" * 8, v["id"])
                    sealed, back = array(box), array([0xA5] * (size + 8))
                    self.assertEqual(lib.chacha20_poly1305_open(k, n, a, view(sealed), view(back), False), 0, v["id"])
                    self.assertEqual(bytes(back), v["msg"] + b"\xa5" * 8, v["id"])
                    # One bit of the tag wrong: refused, and no plaintext released.
                    sealed[len(box) - 1 - v["flip"] % 16] ^= 1 << (v["flip"] % 8)
                    back = array([0xA5] * (size + 8))
                    self.assertEqual(lib.chacha20_poly1305_open(k, n, a, view(sealed), view(back), False), CCP_TAG, v["id"])
                    self.assertEqual(bytes(back), b"\xa5" * (size + 8), v["id"])
                else:
                    sealed, back = array(box), array([0xA5] * (size + 8))
                    self.assertEqual(lib.chacha20_poly1305_open(k, n, a, view(sealed), view(back), False), v["status"], v["id"])
                    self.assertEqual(bytes(back), b"\xa5" * (size + 8), v["id"])
                    if v["status"] == CCP_NONCE:
                        plain, out = array(v["msg"]), array([0xA5] * (len(box) + 8))
                        self.assertEqual(lib.chacha20_poly1305_seal(k, n, a, view(plain, size), view(out), False), CCP_NONCE)
                        self.assertEqual(bytes(out), b"\xa5" * (len(box) + 8))
                counts[v["result"]] += 1
        self.assertEqual(counts, {"valid": 2 * (6 + 256 + 120), "invalid": 2 * 69})

    def test_wasm(self):
        clang, sysroot, resources = Path(WASM_CLANG), Path(WASM_SYSROOT), Path(WASM_RESOURCES)
        self.assertTrue(clang.exists() and sysroot.exists() and resources.exists(), "a Wasm toolchain is required; see coax/web/wasm.sh")
        fixture = self.dir / "wasm.json"
        fixture.write_text(json.dumps([dict(wire(v), kind="poly") for v in self.polys] + [dict(wire(v), kind="aead") for v in self.aeads]))
        for label in ["checked", "release"]:
            module = self.dir / (label + ".wasm")
            run(clang, "--target=wasm32-wasip1", "--sysroot=" + str(sysroot), "-resource-dir=" + str(resources),
                "-std=c11", "-O2", "-I" + str(self.dir), "-mexec-model=reactor", self.dir / (label + ".c"), HERE / "chacha20_poly1305_wasm.c",
                "-Wl,--import-memory,--export-memory,--export=__heap_base,--max-memory=67108864,-z,stack-size=1048576,--gc-sections",
                "-o", module)
            result = json.loads(run("node", HERE / "chacha20_poly1305_wasm.cjs", module, fixture).stdout)
            self.assertEqual((result["poly"], result["valid"], result["refused"]), (len(self.polys), 6 + 256 + 120, 69))
        print(f"\n{len(self.polys)} Poly1305 tags and {len(self.aeads)} AEAD cases, native checked/release and Wasm; oracle {self.engine}",
              file=sys.stderr)


if __name__ == "__main__":
    unittest.main()
