#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: a Python dict, a separate Python writer and reader of the log format, Bitcoin Core's Obfuscation transliterated, and real files with fsync, rename and truncate decide each case outside Coax)
"""The chainstate's key-value store against a reference, on real files.

Bitcoin From Scratch, chapter 62. This script compiles
coax/lib/bitcoin/kvstore.coax to C in checked and release builds and checks:

- Random workloads: seeded batches of puts and deletes over a small key
  space. After each commit the table, read in order and by key and from
  seeks, must equal a Python dict, and each frame must equal the bytes a
  separate Python writer of the format makes (with Bitcoin Core's
  Obfuscation, util/obfuscation.h, transliterated for the mask).
- Crashes on real files: a host in Python writes the log the way the
  chapter says (header by write, fsync, rename; each commit by append and
  fsync; compaction by a new file, fsync, rename and a directory fsync).
  For a recorded workload it truncates the file at every byte of every
  append, and pads the cut with zeros to the frame's end, then reopens:
  the store must hold exactly the last committed batch, the host cuts the
  torn tail and later commits must replay. A compaction that crashes
  before its rename leaves the old log; after it, the new one.
- Damage: every single byte of committed frames flipped, and every cut of
  the header. The Coax reader and the Python reader must agree on each:
  open, cut or refuse, with the same table.

    python3 coax/tests/bitcoin/kvstore.py
"""
import bisect
import ctypes as C
import hashlib
import os
from pathlib import Path
import random
import struct
import subprocess
import sys
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
COMPILER = Path(os.environ.get("COAX_TEST_COMPILER", ROOT / "coax/bin/coax"))
SOURCE = ROOT / "coax/lib/bitcoin/kvstore.coax"
UPSTREAM = ROOT / "coax/tests/vectors/upstream/bitcoin-core"
OBFUSCATION_PIN = "obfuscation.h"

OK, NOT_FOUND, FULL, TOO_LONG, BAD_BATCH, CORRUPT, BAD_HEADER = range(7)
FRAME_MAX = 1 << 26


# ---- Bitcoin Core's Obfuscation (util/obfuscation.h), transliterated: the
# key is a little-endian uint64_t; m_rotations[i] is the key rotated right by
# 8 * i bits; operator()(target, key_offset) XORs each 8-byte word with the
# rotation for its offset, so byte j of the target meets key byte
# (key_offset + j) mod 8.
def core_obfuscate(target, key8, key_offset=0):
    key = int.from_bytes(key8, "little")
    rot = [((key >> (8 * i)) | (key << (64 - 8 * i))) & (2**64 - 1) if i else key for i in range(8)]
    out = bytearray(target)
    pos = 0
    rk = rot[key_offset % 8]
    while pos < len(out):
        word = out[pos:pos + 8]
        raw = int.from_bytes(word, "little") ^ rk
        out[pos:pos + len(word)] = raw.to_bytes(8, "little")[:len(word)]
        pos += 8
    return bytes(out)


def check_obfuscation_pin():
    text = (UPSTREAM / OBFUSCATION_PIN).read_text()
    # The two lines the transliteration rests on.
    assert "m_rotations[i] = std::rotr(key, key_rotation_bits);" in text
    assert "KeyType rot_key{m_rotations[key_offset % KEY_SIZE]};" in text


# ---- the format, written and read in Python
def sha256d(b):
    return hashlib.sha256(hashlib.sha256(b).digest()).digest()


def header(mask):
    h = b"OAKV" + bytes([1, 1 if mask else 0, 0, 0]) + struct.pack("<Q", mask)
    return h + sha256d(h)[:4]


def body(seq, ops, mask):
    out = struct.pack("<QI", seq, len(ops))
    key8 = struct.pack("<Q", mask)
    for kind, k, v in ops:
        if kind == "put":
            out += bytes([1]) + struct.pack("<HI", len(k), len(v)) + k + core_obfuscate(v, key8)
        else:
            out += bytes([2]) + struct.pack("<H", len(k)) + k
    return out


def frame(seq, ops, mask):
    b = body(seq, ops, mask)
    return struct.pack("<I", len(b)) + sha256d(b)[:4] + b


def parse_body(b, mask):
    """The operations, or None when the bytes are not a batch."""
    if len(b) < 12:
        return None
    seq, count = struct.unpack_from("<QI", b, 0)
    at, ops, key8 = 12, [], struct.pack("<Q", mask)
    for _ in range(count):
        if len(b) - at < 3:
            return None
        kind, kn = b[at], struct.unpack_from("<H", b, at + 1)[0]
        if not 1 <= kn <= 1024:
            return None
        if kind == 2:
            if kn > len(b) - at - 3:
                return None
            ops.append(("del", b[at + 3:at + 3 + kn], None))
            at += 3 + kn
        elif kind == 1 and len(b) - at >= 7:
            vn = struct.unpack_from("<I", b, at + 3)[0]
            if vn > 1 << 20 or kn + vn > len(b) - at - 7:
                return None
            v = at + 7 + kn
            ops.append(("put", b[at + 7:v], core_obfuscate(b[v:v + vn], key8)))
            at = v + vn
        else:
            return None
    return (seq, ops) if at == len(b) else None


def replay(file):
    """(code, state, seq, keep, cut), read the way the chapter says."""
    if len(file) < 20 or file[:4] != b"OAKV" or file[4] != 1 or sha256d(file[:16])[:4] != file[16:20]:
        return (BAD_HEADER, None, 0, 0, False)
    mask = struct.unpack_from("<Q", file, 8)[0]
    state, seq, frames, at = {}, 0, 0, 20
    while at < len(file):
        if len(file) - at < 8:
            return (OK, state, seq, at, True)
        n = struct.unpack_from("<I", file, at)[0]
        if 12 <= n <= FRAME_MAX and n > len(file) - at - 8:
            return (OK, state, seq, at, True)
        if not 12 <= n <= FRAME_MAX:
            if not any(file[at + 4:]):
                return (OK, state, seq, at, True)
            return (CORRUPT, None, 0, at, False)
        end = at + 8 + n
        b = file[at + 8:end]
        if sha256d(b)[:4] != file[at + 4:at + 8]:
            if not any(file[end:]):
                return (OK, state, seq, at, True)
            return (CORRUPT, None, 0, at, False)
        parsed = parse_body(b, mask)
        if parsed is None or parsed[0] == 0 or (frames and parsed[0] != seq + 1):
            return (CORRUPT, None, 0, at, False)
        for kind, k, v in parsed[1]:
            if kind == "put":
                state[k] = v
            else:
                state.pop(k, None)
        seq, frames, at = parsed[0], frames + 1, end
    return (OK, state, seq, at, False)


# ---- ctypes
class View(C.Structure):
    _fields_ = [("data", C.POINTER(C.c_uint8)), ("count", C.c_int64)]


class View64(C.Structure):
    _fields_ = [("data", C.POINTER(C.c_uint64)), ("count", C.c_int64)]


def buf(n, fill=0xA5):
    return (C.c_uint8 * max(1, n))(*([fill] * max(1, n)))


def view(b, n):
    return View(C.cast(b, C.POINTER(C.c_uint8)), n)


def bview(data):
    b = (C.c_uint8 * max(1, len(data)))(*data) if data else buf(1)
    return b, view(b, len(data))


def fields(n):
    f = (C.c_uint64 * n)()
    return f, View64(C.cast(f, C.POINTER(C.c_uint64)), n)


def run(*args, **kw):
    r = subprocess.run([str(a) for a in args], capture_output=True, text=True, **kw)
    if r.returncode:
        raise RuntimeError(f"{args[0]} failed ({r.returncode}): {r.stderr or r.stdout}")
    return r


class Store:
    """The Coax store driven through C, with a fixed table."""

    def __init__(self, lib, table_size=1 << 16):
        self.lib, self.size = lib, table_size
        self.table = buf(table_size)
        self.used, self.seq, self.mask = 0, 0, 0

    def open(self, file):
        keep, fv = bview(file)
        f, f6 = fields(6)
        code = self.lib.kv_open_c(fv, view(self.table, self.size), f6)
        self.used, self.seq, self.mask = f[0], f[1], f[5]
        return code, f[3], bool(f[4])

    def commit(self, ops):
        b = buf(1 << 16)
        at = self.lib.kv_batch_new(view(b, 1 << 16))
        for kind, k, v in ops:
            keep, kv = bview(k)
            if kind == "put":
                keep2, vv = bview(v)
                at = self.lib.kv_batch_put_vartime(view(b, 1 << 16), at, kv, vv)
            else:
                at = self.lib.kv_batch_delete_vartime(view(b, 1 << 16), at, kv)
            assert at > 0
        out = buf(at + 9)
        f, f2 = fields(2)
        code = self.lib.kv_commit_c(view(self.table, self.size), self.used, self.seq, view(b, at), self.mask, view(out, at + 8), f2)
        assert code == OK and out[at + 8] == 0xA5, code
        self.used, self.seq = f[0], self.seq + 1
        return bytes(out[:f[1]])

    def items(self):
        f, f5 = fields(5)
        at, out = 0, []
        while self.lib.kv_entry_c(view(self.table, self.size), self.used, at, f5) == OK:
            t = bytes(self.table[:self.used])
            out.append((t[f[1]:f[1] + f[2]], t[f[3]:f[3] + f[4]]))
            at = f[0]
        return out

    def get(self, k):
        keep, kv = bview(k)
        out = buf(2000)
        n = self.lib.kv_get_c(view(self.table, self.size), self.used, kv, view(out, 1999))
        assert out[1999] == 0xA5
        return None if n == -NOT_FOUND else bytes(out[:n])

    def seek(self, k):
        keep, kv = bview(k)
        at = self.lib.kv_seek_c(view(self.table, self.size), self.used, kv)
        f, f5 = fields(5)
        if self.lib.kv_entry_c(view(self.table, self.size), self.used, at, f5) != OK:
            return None
        return bytes(self.table[f[1]:f[1] + f[2]])

    def snapshot(self):
        out = buf(self.used * 2 + 200)
        n = self.lib.kv_snapshot_vartime(view(self.table, self.size), self.used, self.seq, self.mask, view(out, self.used * 2 + 199))
        assert n > 0
        return bytes(out[:n])


class Host:
    """The I/O the chapter describes, on real files in a directory."""

    def __init__(self, d):
        self.dir, self.log, self.tmp = d, d / "chainstate.log", d / "chainstate.tmp"

    def fsync_dir(self):
        fd = os.open(self.dir, os.O_RDONLY)
        try:
            os.fsync(fd)
        finally:
            os.close(fd)

    def replace(self, data):
        with open(self.tmp, "wb") as f:
            f.write(data)
            f.flush()
            os.fsync(f.fileno())
        os.rename(self.tmp, self.log)
        self.fsync_dir()

    def append(self, data):
        with open(self.log, "ab") as f:
            f.write(data)
            f.flush()
            os.fsync(f.fileno())

    def read(self):
        if self.tmp.exists():
            self.tmp.unlink()     # a compaction that never reached its rename
        return self.log.read_bytes()

    def cut(self, keep):
        with open(self.log, "r+b") as f:
            f.truncate(keep)
            f.flush()
            os.fsync(f.fileno())


def random_ops(rng, keys, n):
    ops = []
    for _ in range(n):
        k = rng.choice(keys)
        if rng.random() < 0.3:
            ops.append(("del", k, None))
        else:
            ops.append(("put", k, rng.randbytes(rng.choice([0, 1, 7, 8, 9, 40, rng.randrange(0, 300)]))))
    return ops


def apply(state, ops):
    s = dict(state)
    for kind, k, v in ops:
        if kind == "put":
            s[k] = v
        else:
            s.pop(k, None)
    return s


class KvStoreTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        check_obfuscation_pin()
        cls.temp = tempfile.TemporaryDirectory(prefix="coax-kvstore-")
        d = Path(cls.temp.name)
        cls.libs = []
        for label, mode in [("checked", []), ("release", ["--release"])]:
            c = d / (label + ".c")
            c.write_text(run(COMPILER, "c", SOURCE, "--library", "--header", d / "kvstore.h", *mode).stdout)
            run("cc", "-std=c11", "-O2", "-shared", "-fPIC", c, "-lm", "-o", d / (label + ".dylib"))
            lib = C.CDLL(str(d / (label + ".dylib")))
            V, I, U = View, C.c_int64, C.c_uint64
            sig = {
                "kv_batch_new": [V], "kv_batch_put_vartime": [V, I, V, V], "kv_batch_delete_vartime": [V, I, V],
                "kv_header_vartime": [U, V], "kv_snapshot_vartime": [V, I, U, U, V], "kv_open_c": [V, V, View64],
                "kv_commit_c": [V, I, U, V, U, V, View64], "kv_get_c": [V, I, V, V], "kv_set_c": [V, I, V, V],
                "kv_delete_c": [V, I, V], "kv_seek_c": [V, I, V], "kv_entry_c": [V, I, I, View64],
            }
            for name, args in sig.items():
                getattr(lib, name).argtypes = args
                getattr(lib, name).restype = C.c_int64
            cls.libs.append((label, lib))
        cls.counts = {}

    @classmethod
    def tearDownClass(cls):
        cls.temp.cleanup()
        print(f"\n{cls.counts}", file=sys.stderr)

    def count(self, k, n=1):
        self.counts[k] = self.counts.get(k, 0) + n

    def header_of(self, lib, mask):
        out = buf(21)
        assert lib.kv_header_vartime(mask, view(out, 20)) == 20 and out[20] == 0xA5
        return bytes(out[:20])

    def same(self, store, state, rng, keys):
        self.assertEqual(store.items(), sorted(state.items()))
        for k in keys:
            self.assertEqual(store.get(k), state.get(k))
        ordered = sorted(state)
        for _ in range(10):
            probe = rng.choice(keys) + rng.choice([b"", b"\x00", b"\xff"])
            i = bisect.bisect_left(ordered, probe)
            self.assertEqual(store.seek(probe), ordered[i] if i < len(ordered) else None)

    def test_workloads(self):
        rng = random.Random(62)
        for label, lib in self.libs:
            for mask in [0, 0x0123456789ABCDEF, rng.randrange(1, 2**64)]:
                keys = [b"C" + rng.randbytes(rng.choice([1, 2, 33])) for _ in range(40)] + [b"a", b"ab", b"b", b"\xff" * 3]
                store, state = Store(lib), {}
                store.mask = mask
                for seq in range(1, 61):
                    ops = random_ops(rng, keys, rng.randrange(0, 12))
                    fr = store.commit(ops)
                    self.assertEqual(fr, frame(seq, ops, mask))
                    state = apply(state, ops)
                    self.same(store, state, rng, keys)
                    self.count("commits")
                # The table's own put and delete, without a log.
                for _ in range(200):
                    k = rng.choice(keys)
                    keep, kv = bview(k)
                    if rng.random() < 0.4:
                        store.used = lib.kv_delete_c(view(store.table, store.size), store.used, kv)
                        state.pop(k, None)
                    else:
                        v = rng.randbytes(rng.randrange(0, 50))
                        keep2, vv = bview(v)
                        store.used = lib.kv_set_c(view(store.table, store.size), store.used, kv, vv)
                        state[k] = v
                    self.count("table operations")
                self.same(store, state, rng, keys)
                # A snapshot reopens to the same table.
                snap = store.snapshot()
                self.assertEqual(snap, header(mask) + frame(store.seq, [("put", k, v) for k, v in sorted(state.items())], mask))
                again = Store(lib)
                self.assertEqual(again.open(snap), (OK, len(snap), False))
                self.assertEqual((again.items(), again.seq, again.mask), (sorted(state.items()), store.seq, mask))

    def test_crashes(self):
        rng = random.Random(6262)
        for label, lib in self.libs:
            mask = 0x5A5A5A5A5A5A5A5A if label == "release" else 0
            keys = [b"C" + bytes([i]) for i in range(12)]
            d = Path(self.temp.name) / ("crash-" + label)
            d.mkdir()
            host = Host(d)
            host.replace(self.header_of(lib, mask))
            store = Store(lib)
            self.assertEqual(store.open(host.read()), (OK, 20, False))
            store.mask = mask
            state = {}
            for step in range(16):
                if step in (6, 12):
                    # Compaction: crash before the rename keeps the old log.
                    snap = store.snapshot()
                    with open(host.tmp, "wb") as f:
                        f.write(snap[:rng.randrange(len(snap))])
                    reopened = Store(lib)
                    self.assertEqual(reopened.open(host.read())[0], OK)
                    self.assertEqual(reopened.items(), sorted(state.items()))
                    host.replace(snap)
                    reopened = Store(lib)
                    self.assertEqual(reopened.open(host.read()), (OK, len(snap), False))
                    self.assertEqual((reopened.items(), reopened.seq), (sorted(state.items()), store.seq))
                    self.count("compactions")
                    continue
                ops = random_ops(rng, keys, rng.randrange(1, 6))
                before = host.read()
                fr = store.commit(ops)
                after = apply(state, ops)
                # A crash during the append: every prefix, and every prefix
                # padded with zeros to the frame's end.
                for k in range(len(fr) + 1):
                    for pad in (False, True):
                        if pad and k == len(fr):
                            continue
                        data = before + fr[:k] + (bytes(len(fr) - k) if pad else b"")
                        host.replace(data)
                        s = Store(lib)
                        code, keep, cut = s.open(host.read())
                        # Padding can complete a frame whose last bytes are
                        # zeros: then the commit happened.
                        whole = data == before + fr
                        want = after if whole else state
                        self.assertEqual(code, OK)
                        self.assertEqual(s.items(), sorted(want.items()), (label, step, k, pad))
                        self.assertEqual(cut, not whole and data != before)
                        self.assertEqual(replay(host.read())[:2], (OK, want))
                        if cut:
                            host.cut(keep)
                            self.assertEqual(host.read(), before)
                        self.count("crash points")
                # After the last torn case is cut, the commit is appended for real.
                host.replace(before)
                host.append(fr)
                state = after
                s = Store(lib)
                self.assertEqual(s.open(host.read()), (OK, len(before) + len(fr), False))
                self.assertEqual(s.items(), sorted(state.items()))

    def test_damage(self):
        rng = random.Random(626262)
        for label, lib in self.libs:
            mask = 0 if label == "checked" else 0x0102030405060708
            store = Store(lib)
            log = self.header_of(lib, mask)
            store.open(log)
            keys = [b"k" + bytes([i]) for i in range(6)]
            for _ in range(5):
                log += store.commit(random_ops(rng, keys, rng.randrange(1, 4)))
            outcomes = {}
            for i in range(len(log)):
                for flip in (0x01, 0x80, 0xFF):
                    bad = bytearray(log)
                    bad[i] ^= flip
                    want = replay(bytes(bad))
                    s = Store(lib)
                    code, keep, cut = s.open(bytes(bad))
                    self.assertEqual((code, cut), (want[0], want[4]), (i, flip))
                    if code == OK:
                        self.assertEqual((s.items(), s.seq, keep), (sorted(want[1].items()), want[2], want[3]))
                    else:
                        self.assertEqual(keep if code == CORRUPT else 0, want[3] if code == CORRUPT else 0)
                    outcomes[(code, cut)] = outcomes.get((code, cut), 0) + 1
                    self.count("damaged logs")
            for cut in range(20):
                self.assertEqual(Store(lib).open(log[:cut])[0], BAD_HEADER)
            # Every outcome happens: refuse a bad header, refuse damage, cut.
            self.assertIn((BAD_HEADER, False), outcomes)
            self.assertIn((CORRUPT, False), outcomes)
            self.assertIn((OK, True), outcomes)


if __name__ == "__main__":
    unittest.main()
