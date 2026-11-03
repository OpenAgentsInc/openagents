#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: a Python transliteration of Bitcoin Core's Merkle code decides each case outside Coax)
"""Merkle trees against a transliteration of Bitcoin Core, natively and in Wasm.

Bitcoin From Scratch, chapter 46. Compiles coax/lib/bitcoin/merkle.coax
(through the flat test entry point coax/tests/bitcoin/merkle_report.coax)
to C in checked and release builds and runs:

- trees: Core's merkle_tests.cpp construction (transactions with no
  inputs or outputs and locktimes 0, 1, ..., for 0 to 16 leaves and seeded
  larger counts, each with up to three rounds of repeated trailing
  transactions), Core's CVE-2012-2459 leaves, seeded random leaves with
  equal pairs planted at random levels, and the txids of the three pinned
  mainnet blocks. Each tree's root, mutated flag, header check, witness
  root and branches, whose expected values come from Core's
  constant-space MerkleComputation, not from a level-by-level tree;
- blocks: the three pinned mainnet blocks, block 514,999 with its last
  transaction repeated, blocks holding a 64-byte transaction with and
  without BIP-54's rule, and seeded edits of the small blocks (bytes cut,
  changed, inserted, appended, the count changed);
- proofs: every branch of seeded trees with the index, the count, the
  leaf, the root or the branch's length changed.

Each report must equal the one built from merkle_reference.py, which
shares no code with Coax, and the output buffer past it must be untouched.
Then it builds the same C for wasm32 and runs every case again in Node.
From the repository root:

    python3 coax/tests/bitcoin/merkle.py
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
import merkle_reference as M  # noqa: E402
import tx_reference as R  # noqa: E402
from wasm_toolchain import WASM_CLANG, WASM_SYSROOT, WASM_RESOURCES  # noqa: E402

COMPILER = Path(os.environ.get("COAX_TEST_COMPILER", ROOT / "coax/bin/coax"))
SOURCE = HERE / "merkle_report.coax"
UPSTREAM = ROOT / "coax/tests/vectors/upstream"
LEAVES = 4096       # MR_LEAVES in merkle_report.coax
ROOM = 1024         # MR_ROOM
CAP = 3 * 1024 * 1024
FILL = b"\xa5" * 32
B100000 = UPSTREAM / "mainnet/000000000003ba27aa200b1cecaad478d2b00432346c3f1f3986da1afd33e506.raw"
B514999 = UPSTREAM / "mainnet/00000000000000000008fda0e5bb3105dab72605332085684c42dceb0ea61097.raw"
B413567 = UPSTREAM / "bitcoin-core/block413567.raw"


def le(v):
    return (v & (1 << 64) - 1).to_bytes(8, "little")


def tree_report(header, leaves, step):
    n = len(leaves)
    root, mutated = M.compute_root(leaves)
    out = le(M.EMPTY if n == 0 else M.OK) + le(int(mutated)) + root
    out += le(M.check(leaves, header))
    out += le(M.EMPTY if n == 0 else M.OK) + M.witness_root(leaves)
    out += le(M.depth(n))
    for i in range(0, n, max(1, step)):
        branch = M.merkle_path(leaves, i)
        out += le(M.OK) + le(len(branch)) + b"".join(branch)
        out += le(M.OK) + M.root_from_branch(leaves[i], branch, i)
        out += le(M.verify(leaves[i], branch, i, n, root))
    return out


def block_report(raw, bip54):
    k = M.read_block(raw, bip54, LEAVES, ROOM)
    out = b"".join(le(k[f]) for f in ("code", "at", "tx_code", "count", "first64"))
    if k["code"] != M.OK:
        return out
    out += b"".join(k["txids"]) + b"".join(k["wtxids"])
    out += le(M.check(k["txids"], raw[36:68]))
    return out + le(M.EMPTY if not k["count"] else M.OK) + M.witness_root(k["wtxids"])


def proof_report(leaf, root, branch, index, count):
    if len(branch) % 32 or index < 0:
        out = le(M.CAPACITY) + FILL
    else:
        out = le(M.OK) + M.root_from_branch(leaf, [branch[i:i + 32] for i in range(0, len(branch), 32)], index)
    if len(branch) % 32:
        code = M.CAPACITY
    elif count < 1 or index < 0 or index >= count:
        code = M.BAD_INDEX
    elif len(branch) != 32 * M.depth(count):
        code = M.BAD_DEPTH
    else:
        code = M.OK if out[8:] == root else M.MISMATCH
    return out + le(code)


def tree_case(header, leaves, step=1):
    return dict(op=0, input=(header + b"".join(leaves)).hex(), index=step, count=0,
                report=tree_report(header, leaves, step).hex())


def block_case(raw, bip54=False):
    return dict(op=1, input=raw.hex(), index=int(bip54), count=0, report=block_report(raw, bip54).hex())


def proof_case(leaf, root, branch, index, count):
    return dict(op=2, input=(leaf + root + branch).hex(), index=index, count=count,
                report=proof_report(leaf, root, branch, index, count).hex())


def core_txids(n):
    """merkle_tests.cpp's transactions: version 2, no inputs or outputs,
    locktime j."""
    return [M.sha256d(b"\x02\0\0\0\0\0" + j.to_bytes(4, "little")) for j in range(n)]


def ctz(i):
    if i == 0:
        return 0
    j = 0
    while not i & 1:
        j += 1
        i >>= 1
    return j


def core_mutations(ntx):
    """merkle_test's lists: (leaves, mutated) for up to three rounds of
    repeating the last 2^ctz transactions."""
    out = []
    for mutate in range(4):
        dup1 = 1 << ctz(ntx) if mutate >= 1 else 0
        if dup1 >= ntx:
            break
        n1 = ntx + dup1
        dup2 = 1 << ctz(n1) if mutate >= 2 else 0
        if dup2 >= n1:
            break
        n2 = n1 + dup2
        dup3 = 1 << ctz(n2) if mutate >= 3 else 0
        if dup3 >= n2:
            break
        leaves = core_txids(ntx)
        for dup in (dup1, dup2, dup3):
            leaves = leaves + leaves[len(leaves) - dup:]
        out.append((leaves, bool(mutate)))
    return out


def repeat_last(raw):
    """Block bytes with the last transaction repeated and the count one
    more (the count must stay one byte)."""
    r = R.Reader(raw, None)
    r.at = 80
    n = r.compact()
    assert n < 0xFC
    start = 0
    for _ in range(n):
        _, start, _ = M.read_tx(r)
    return raw[:80] + bytes([n + 1]) + raw[81:] + raw[start:]


def with_tx64(raw, tx64, extra=b""):
    """The header and coinbase of raw, then tx64."""
    r = R.Reader(raw, None)
    r.at = 81
    _, _, end = M.read_tx(r)
    return raw[:80] + bytes([2]) + raw[81:end] + tx64 + extra


def edit(data, rng):
    kind = rng.randrange(6)
    k = rng.randrange(len(data))
    if kind == 0: return data[:k] + bytes([data[k] ^ (1 << rng.randrange(8))]) + data[k + 1:]
    if kind == 1: return data[:k] + bytes([rng.randrange(256)]) + data[k:]
    if kind == 2: return data[:k] + data[k + 1:]
    if kind == 3: return data[:k]
    if kind == 4: return data + bytes([rng.randrange(256)])
    return data[:80] + bytes([rng.choice([0, 1, data[80] - 1, data[80] + 1, 0xFD, 0xFE, 0xFF])]) + data[81:]


def cases():
    rng = random.Random(46)
    out = []
    # Core's merkle_test: 0 to 16 leaves, then seeded counts.
    for ntx in list(range(17)) + [rng.randrange(17, 700) for _ in range(6)]:
        root = M.compute_root(core_txids(ntx))[0]
        step = 1 if ntx <= 64 else 37
        for leaves, mutated in core_mutations(ntx):
            assert M.compute_root(leaves) == (root, mutated) and M.old_tree(leaves)[:2] == (root, mutated)
            out.append(tree_case(root, leaves, step))
        out.append(tree_case(b"\x01" + bytes(31) if ntx else bytes(32), core_txids(ntx), step))
    # The CVE-2012-2459 leaves of merkle_test_mutated_return_value.
    num = lambda v: bytes([v]) + bytes(31)
    six = [num(v) for v in range(1, 7)]
    out.append(tree_case(M.compute_root(six)[0], six))
    out.append(tree_case(M.compute_root(six)[0], six + six[4:]))
    out.append(tree_case(bytes(32), [num(1), num(1), num(3), num(4)]))
    # Seeded leaves with an equal pair planted at a random level.
    for _ in range(120):
        n = rng.randrange(1, 70)
        leaves = [rng.randbytes(32) for _ in range(n)]
        if rng.random() < 0.6 and n > 1:
            width = 1 << rng.randrange(0, max(1, (n - 1).bit_length()))
            start = rng.randrange(0, max(1, n - 2 * width + 1)) // (2 * width) * (2 * width)
            if start + 2 * width <= n:
                leaves[start + width:start + 2 * width] = leaves[start:start + width]
        if rng.random() < 0.3:
            leaves.append(leaves[-1])
        header = M.compute_root(leaves)[0] if rng.random() < 0.8 else rng.randbytes(32)
        out.append(tree_case(header, leaves))
    # The mainnet blocks' txids.
    raws = [B100000.read_bytes(), B514999.read_bytes(), B413567.read_bytes()]
    for raw in raws:
        txids = M.block_ids(raw)[0]
        out.append(tree_case(raw[36:68], txids, 97 if len(txids) > 100 else 1))
    # Blocks.
    for raw in raws:
        out += [block_case(raw), block_case(raw, True)]
    out.append(block_case(repeat_last(raws[1])))
    tx64 = bytes.fromhex((HERE / "merkle_vectors.coax").read_text().split('MK_TX64 :: "')[1].split('"')[0])
    out += [block_case(with_tx64(raws[0], tx64)), block_case(with_tx64(raws[0], tx64), True)]
    longer = tx64[:55] + b"\x05" + tx64[56:] + b"\x00"
    out += [block_case(with_tx64(raws[0], longer), True)]
    out += [block_case(raws[0][:80] + b"\x00"), block_case(raws[0][:79]), block_case(raws[0][:80]),
            block_case(raws[0][:80] + b"\xfd\x01\x10")]
    for raw in raws[:2]:
        for _ in range(60):
            out.append(block_case(edit(raw, rng), rng.random() < 0.2))
    # Proofs.
    for _ in range(80):
        n = rng.randrange(1, 40)
        leaves = [rng.randbytes(32) for _ in range(n)]
        root = M.compute_root(leaves)[0]
        i = rng.randrange(n)
        branch = b"".join(M.merkle_path(leaves, i))
        out.append(proof_case(leaves[i], root, branch, i, n))
        kind = rng.randrange(7)
        if kind == 0: out.append(proof_case(leaves[i], root, branch, rng.randrange(-2, 2 * n + 2), n))
        elif kind == 1: out.append(proof_case(leaves[i], root, branch, i, rng.randrange(-1, 4 * n + 2)))
        elif kind == 2: out.append(proof_case(rng.randbytes(32), root, branch, i, n))
        elif kind == 3: out.append(proof_case(leaves[i], rng.randbytes(32), branch, i, n))
        elif kind == 4: out.append(proof_case(leaves[i], root, branch[:rng.randrange(len(branch) + 1)], i, n))
        elif kind == 5: out.append(proof_case(leaves[i], root, branch + rng.randbytes(32), i, n))
        else: out.append(proof_case(leaves[i], root, branch, i + (rng.randrange(1, 4) << (len(branch) // 32)), n))
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


class Merkle(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix="coax-merkle-")
        cls.dir = Path(cls.temp.name)
        cls.libs = []
        for label, mode in [("checked", []), ("release", ["--release"])]:
            c = cls.dir / (label + ".c")
            c.write_text(run(COMPILER, "c", SOURCE, "--library", "--header", cls.dir / "merkle.h", *mode).stdout)
            run("cc", "-std=c11", "-O2", "-shared", "-fPIC", c, "-lm", "-o", cls.dir / (label + ".dylib"))
            lib = C.CDLL(str(cls.dir / (label + ".dylib")))
            lib.merkle_report_vartime.argtypes = [C.c_int64, View, C.c_int64, C.c_int64, View]
            lib.merkle_report_vartime.restype = C.c_int64
            cls.libs.append((label, lib))
        cls.cases = cases()
        cls.out = (C.c_uint8 * CAP)()

    @classmethod
    def tearDownClass(cls):
        cls.temp.cleanup()

    def native(self, lib, c):
        data = bytes.fromhex(c["input"])
        inp = (C.c_uint8 * max(1, len(data))).from_buffer_copy(data.ljust(1, b"\0"))
        C.memset(self.out, 0xA5, CAP)
        r = lib.merkle_report_vartime(c["op"], view(inp, len(data)), c["index"], c["count"], view(self.out, CAP))
        self.assertGreaterEqual(r, 16)
        rest = C.string_at(C.addressof(self.out) + r, min(CAP - r, 4096))
        self.assertEqual(rest, b"\xa5" * len(rest), "past the report")
        return C.string_at(self.out, r).hex()

    def test_reference(self):
        """The reference agrees with Core's older tree, the headers and the published roots."""
        for ntx in range(40):
            leaves = core_txids(ntx)
            root, mutated, tree = M.old_tree(leaves)
            self.assertEqual(M.compute_root(leaves), (root, mutated))
            for i in range(ntx):
                self.assertEqual(M.merkle_path(leaves, i), M.old_branch(ntx, tree, i))
        num = lambda v: bytes([v]) + bytes(31)
        self.assertTrue(M.compute_root([num(1), num(1), num(3), num(4)])[1])
        self.assertFalse(M.old_tree([num(1), num(1), num(3), num(4)])[1], "the older flag looked only at the end")
        for raw in (B100000.read_bytes(), B514999.read_bytes(), B413567.read_bytes()):
            self.assertEqual(M.check(M.block_ids(raw)[0], raw[36:68]), M.OK)

    def test_native(self):
        codes = {}
        for c in self.cases:
            code = int.from_bytes(bytes.fromhex(c["report"][:16]), "little")
            codes.setdefault(c["op"], {}).setdefault(code, 0)
            codes[c["op"]][code] += 1
        checks = {int.from_bytes(bytes.fromhex(c["report"][96:112]), "little") for c in self.cases if c["op"] == 0}
        self.assertEqual(checks, {M.OK, M.EMPTY, M.MUTATED, M.MISMATCH})
        self.assertTrue({M.OK, M.CAPACITY, M.BAD_BLOCK, M.TX_64} <= set(codes[1]), codes)
        for label, lib in self.libs:
            for c in self.cases:
                self.assertEqual(self.native(lib, c), c["report"], f"{label} op {c['op']} {c['input'][:80]}")
        print(f"\n{len(self.cases)} cases; first codes by op {codes}", file=sys.stderr)

    def test_wasm(self):
        clang, sysroot, resources = Path(WASM_CLANG), Path(WASM_SYSROOT), Path(WASM_RESOURCES)
        self.assertTrue(clang.exists() and sysroot.exists() and resources.exists(), "a Wasm toolchain is required; see coax/web/wasm.sh")
        fixture = self.dir / "wasm.json"
        fixture.write_text(json.dumps(self.cases))
        for label in ["checked", "release"]:
            module = self.dir / (label + ".wasm")
            run(clang, "--target=wasm32-wasip1", "--sysroot=" + str(sysroot), "-resource-dir=" + str(resources),
                "-std=c11", "-O2", "-I" + str(self.dir), "-mexec-model=reactor",
                self.dir / (label + ".c"), HERE / "merkle_wasm.c",
                "-Wl,--import-memory,--export-memory,--export=__heap_base,--max-memory=67108864,-z,stack-size=2097152,--gc-sections",
                "-o", module)
            result = json.loads(run("node", HERE / "merkle_wasm.cjs", module, fixture).stdout)
            self.assertEqual((result["agree"], result["cases"]), (len(self.cases), len(self.cases)), label)
        print(f"\n{len(self.cases)} cases agree with Python, native checked/release and Wasm checked/release",
              file=sys.stderr)


if __name__ == "__main__":
    unittest.main()
