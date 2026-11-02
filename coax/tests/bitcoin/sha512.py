#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: the NIST CAVS vectors and Python's hashlib check the Coax hash)
"""SHA-512 against every NIST CAVS 11.0 ShortMsg and LongMsg vector, natively and in Wasm.

Bitcoin From Scratch, chapter 7. Compiles coax/lib/crypto/sha512-memory.coax
to C in checked and release builds, loads each as a shared library, and checks
all 257 pinned vectors through the one-shot call, the stream and the
`sha512` call that BIP-39 and BIP-32 use, against the published digest and
Python's hashlib. Then it builds the same C for wasm32 and runs the vectors in
Node. Run from the repository root:

    python3 coax/tests/bitcoin/sha512.py
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
SOURCE = ROOT / "coax/lib/crypto/sha512-memory.coax"
NIST = ROOT / "coax/tests/vectors/upstream/nist"
FILES = ["SHA512ShortMsg.rsp", "SHA512LongMsg.rsp"]


def run(*args, **kw):
    r = subprocess.run([str(a) for a in args], capture_output=True, text=True, **kw)
    if r.returncode:
        raise RuntimeError(f"{args[0]} failed ({r.returncode}): {r.stderr or r.stdout}")
    return r


def nist_vectors():
    """(message, digest) for every row. Len is in bits; Len = 0 has Msg = 00."""
    for name in FILES:
        text = (NIST / name).read_text()
        for bits, msg, md in re.findall(r"Len\s*=\s*(\d+)\s+Msg\s*=\s*([0-9a-fA-F]+)\s+MD\s*=\s*([0-9a-fA-F]+)", text):
            assert int(bits) % 8 == 0
            yield bytes.fromhex(msg)[: int(bits) // 8], bytes.fromhex(md)


class View(C.Structure):
    _fields_ = [("data", C.POINTER(C.c_uint8)), ("count", C.c_int64)]


class Info(C.Structure):
    _fields_ = [("error", C.c_uint32), ("phase", C.c_uint32), ("total", C.c_uint64), ("buffered", C.c_uint32)]


def array(data):
    return (C.c_uint8 * max(1, len(data)))(*data)


def view(data, at=0, count=None):
    return View(C.cast(C.byref(data, at), C.POINTER(C.c_uint8)), len(data) - at if count is None else count)


class Sha512(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix="coax-sha512-")
        cls.dir = Path(cls.temp.name)
        cls.libs = []
        for label, mode in [("checked", []), ("release", ["--release"])]:
            c = cls.dir / (label + ".c")
            c.write_text(run(COMPILER, "c", SOURCE, "--library", "--header", cls.dir / "sha512.h", *mode).stdout)
            run("cc", "-std=c11", "-O2", "-shared", "-fPIC", c, "-lm", "-o", cls.dir / (label + ".dylib"))
            lib = C.CDLL(str(cls.dir / (label + ".dylib")))
            lib.sha512_state_size.restype = C.c_int64
            lib.sha512_init.argtypes = [View]
            lib.sha512_init.restype = C.c_uint32
            lib.sha512_info.argtypes = [View]
            lib.sha512_info.restype = Info
            for name in ["sha512_update", "sha512_final", "sha512_hash"]:
                getattr(lib, name).argtypes = [View, View]
                getattr(lib, name).restype = C.c_uint32
            lib.sha512.argtypes = [View, View]
            lib.sha512.restype = C.c_bool
            cls.libs.append(lib)

    @classmethod
    def tearDownClass(cls):
        cls.temp.cleanup()

    def one_shot(self, lib, data):
        src, out = array(data), array([0xA5] * 72)
        self.assertEqual(lib.sha512_hash(view(src, count=len(data)), view(out)), 0)
        self.assertEqual(bytes(out)[64:], b"\xa5" * 8)
        return bytes(out)[:64]

    def legacy(self, lib, data):
        src, out = array(data), array([0] * 64)
        self.assertTrue(lib.sha512(view(src, count=len(data)), view(out)))
        return bytes(out)

    def stream(self, lib, data, chunk):
        state = array([0] * lib.sha512_state_size())
        self.assertEqual(lib.sha512_init(view(state)), 0)
        src = array(data)
        for at in range(0, len(data), chunk):
            self.assertEqual(lib.sha512_update(view(state), view(src, at, min(chunk, len(data) - at))), 0)
        info = lib.sha512_info(view(state))
        self.assertEqual((info.error, info.phase, info.total, info.buffered), (0, 1, len(data), len(data) % 128))
        out = array([0] * 64)
        self.assertEqual(lib.sha512_final(view(state), view(out)), 0)
        return bytes(out)

    def test_every_nist_vector(self):
        vectors = list(nist_vectors())
        self.assertEqual(len(vectors), 257)
        for data, digest in vectors:
            self.assertEqual(hashlib.sha512(data).digest(), digest)
            for lib in self.libs:
                self.assertEqual(self.one_shot(lib, data), digest)
                self.assertEqual(self.legacy(lib, data), digest)
                self.assertEqual(self.stream(lib, data, 113), digest)

    def test_stream_boundaries_against_hashlib(self):
        rng = random.Random(512)
        for size in [0, 1, 111, 112, 113, 127, 128, 129, 239, 240, 255, 256, 257, 1000, 4097]:
            data = rng.randbytes(size)
            for lib in self.libs:
                for chunk in [1, 7, 111, 112, 127, 128, 129, 4096]:
                    self.assertEqual(self.stream(lib, data, chunk), hashlib.sha512(data).digest())

    def test_refusals_preserve_state_and_output(self):
        for lib in self.libs:
            state, out, src = array([0xA5] * 208), array([0xA5] * 72), array([1, 2, 3])
            self.assertNotEqual(lib.sha512_update(view(state), view(src)), 0)
            for count in [-1, 0, 207, 209]:
                self.assertNotEqual(lib.sha512_init(view(state, count=count)), 0)
            self.assertEqual(bytes(state), b"\xa5" * 208)
            self.assertEqual(lib.sha512_init(view(state)), 0)
            for op in [lambda: lib.sha512_update(view(state), view(src, count=-1)),
                       lambda: lib.sha512_update(view(state), view(src, count=1048577)),
                       lambda: lib.sha512_final(view(state), view(out, count=63)),
                       lambda: lib.sha512_update(view(state), view(state, 80, 3)),
                       lambda: lib.sha512_final(view(state), view(state, 0, 64)),
                       lambda: lib.sha512_hash(view(src, count=-1), view(out))]:
                before = bytes(state)
                self.assertNotEqual(op(), 0)
                self.assertEqual(bytes(state), before)
                self.assertEqual(bytes(out), b"\xa5" * 72)
            self.assertFalse(lib.sha512(view(src, count=-1), view(out)))
            self.assertFalse(lib.sha512(view(src), view(out, count=63)))

    def test_wasm_every_nist_vector(self):
        clang, sysroot, resources = Path(WASM_CLANG), Path(WASM_SYSROOT), Path(WASM_RESOURCES)
        self.assertTrue(clang.exists() and sysroot.exists() and resources.exists(), "a Wasm toolchain is required; see coax/web/wasm.sh")
        vectors = [{"input": d.hex(), "digest": m.hex()} for d, m in nist_vectors()]
        streams = []
        for n in [0, 111, 112, 128, 129, 1000]:
            data = bytes(i % 256 for i in range(n))
            for chunk in [1, 7, 128, 129]:
                streams.append({"input": data.hex(), "digest": hashlib.sha512(data).hexdigest(), "chunk": chunk})
        fixture = self.dir / "wasm.json"
        fixture.write_text(json.dumps({"vectors": vectors, "streams": streams}))
        for label in ["checked", "release"]:
            module = self.dir / (label + ".wasm")
            run(clang, "--target=wasm32-wasip1", "--sysroot=" + str(sysroot), "-resource-dir=" + str(resources),
                "-std=c11", "-O2", "-I" + str(self.dir), "-mexec-model=reactor", self.dir / (label + ".c"), HERE / "sha512_wasm.c",
                "-Wl,--import-memory,--export-memory,--export=__heap_base,--max-memory=67108864,-z,stack-size=1048576,--gc-sections",
                "-o", module)
            result = json.loads(run("node", HERE / "sha512_wasm.cjs", module, fixture).stdout)
            self.assertEqual((result["nist_rows"], result["streams"]), (257, len(streams)))


if __name__ == "__main__":
    unittest.main()
