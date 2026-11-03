#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Python's hmac, hashlib and integers decide each case outside Coax)
"""BIP-32 against a separate Python implementation, natively and in Wasm.

Bitcoin From Scratch, chapter 32. Compiles coax/lib/crypto/bip32.coax to C
in checked and release builds and runs bip32_master_ct, bip32_ckd_ct,
bip32_descend_ct, bip32_neuter_ct, bip32_fingerprint_ct, bip32_encode_ct
and bip32_decode_ct on:

- BIP-32's test vectors 1 to 5 and the xprv column of
  trezor/python-mnemonic's vectors.json, pinned in
  coax/tests/vectors/upstream/;
- seeded random seeds of 16 to 64 bytes, on mainnet and the test networks,
  and random paths of hardened and normal indexes below them, with CKDpub
  from every public node;
- the index boundaries 0, 2^31 - 1, 2^31, 2^32 - 1, -1 and 2^32;
- broken nodes: each field changed, unknown versions, wrong key bytes,
  depth 255, private keys of 0, n and above, public keys off the curve,
  and wrong sizes;
- texts: every node's xprv and xpub, one character changed, a character
  outside the alphabet, 110 and 112 characters, and payloads with a good
  checksum and a bad field;
- output and work buffers one byte short.

Each result (the count written or the refusal code) and each output must
equal those of the reference below, which uses Python's hmac, hashlib
(SHA-256, SHA-512 and RIPEMD-160) and integers and shares no code with
Coax. The work buffer must come back as zeros, or untouched after a refusal
that comes before it is used. Then it builds the same C for wasm32 and runs
every case again in Node. From the repository root:

    python3 coax/tests/bitcoin/bip32.py
"""
import ctypes as C
import hashlib
import hmac
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
SOURCE = ROOT / "coax/lib/crypto/bip32.coax"
UPSTREAM = ROOT / "coax/tests/vectors/upstream"
PINS = {
    "bip32/bip-0032.mediawiki": "e5e00a8289db2f681052cf24a745320afc225e66b25d1e489a7c884d2fc7f11f",
    "bip39/vectors.json": "fa3b937b7cff9c9b8ecd3aa011faeb8d6dd67993174b72326e83f4de8fdb30f8",
}
WORK = 326
OUT = 256

# ---- the Python reference
(BAD_SIZE, BAD_INDEX, INVALID_KEY, NEEDS_PRIVATE, CAPACITY, BAD_TEXT, BAD_CHECKSUM, BAD_VERSION,
 BAD_PREFIX, BAD_DEPTH) = range(-1, -11, -1)
P = 2**256 - 2**32 - 977
N = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141
G = (0x79BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798,
     0x483ADA7726A3C4655DA4FBFC0E1108A8FD17B448A68554199C47D08FFB10D4B8)
XPRV, XPUB, TPRV, TPUB = 0x0488ADE4, 0x0488B21E, 0x04358394, 0x043587CF
PRIVATE = {XPRV: XPUB, TPRV: TPUB}
HARDENED = 2**31
ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"


def read(name):
    raw = (UPSTREAM / name).read_bytes()
    assert hashlib.sha256(raw).hexdigest() == PINS[name], name
    return raw


def add(a, b):
    if a is None: return b
    if b is None: return a
    if a[0] == b[0] and (a[1] + b[1]) % P == 0: return None
    if a == b: slope = 3 * a[0] * a[0] * pow(2 * a[1], P - 2, P) % P
    else: slope = (b[1] - a[1]) * pow(b[0] - a[0], P - 2, P) % P
    x = (slope * slope - a[0] - b[0]) % P
    return x, (slope * (a[0] - x) - a[1]) % P


def mul(k, point=G):
    result = None
    while k:
        if k & 1: result = add(result, point)
        point = add(point, point); k >>= 1
    return result


def ser_p(point): return bytes([2 + (point[1] & 1)]) + point[0].to_bytes(32, "big")


def lift(key):
    if len(key) != 33 or key[0] not in (2, 3): return None
    x = int.from_bytes(key[1:], "big")
    if x >= P: return None
    y = pow((x**3 + 7) % P, (P + 1) // 4, P)
    if y * y % P != (x**3 + 7) % P: return None
    return x, (y if y & 1 == key[0] & 1 else P - y)


def checksum(b): return hashlib.sha256(hashlib.sha256(b).digest()).digest()[:4]


def hash160(b): return hashlib.new("ripemd160", hashlib.sha256(b).digest()).digest()


def to_text(payload):
    data = payload + checksum(payload)
    n, out = int.from_bytes(data, "big"), ""
    while n: n, r = divmod(n, 58); out = ALPHABET[r] + out
    return "1" * (len(data) - len(data.lstrip(b"\0"))) + out


def kind(node):
    """1 private, 0 public, or a refusal: the version, then the key byte."""
    version = int.from_bytes(node[:4], "big")
    if version in PRIVATE: return BAD_PREFIX if node[45] != 0 else 1
    if version in PRIVATE.values(): return BAD_PREFIX if node[45] not in (2, 3) else 0
    return BAD_VERSION


def private_ok(key): return 0 < int.from_bytes(key, "big") < N


def public_of(node):
    """The node's compressed public key, or None for a private key out of range."""
    if kind(node) == 0: return node[45:]
    k = int.from_bytes(node[46:], "big")
    return ser_p(mul(k)) if 0 < k < N else None


def master(seed, testnet):
    if not 16 <= len(seed) <= 64: return BAD_SIZE, b""
    i = hmac.new(b"Bitcoin seed", seed, "sha512").digest()
    if not private_ok(i[:32]): return INVALID_KEY, b""
    return 78, (TPRV if testnet else XPRV).to_bytes(4, "big") + bytes(9) + i[32:] + b"\0" + i[:32]


def ckd_node(node, index):
    k = kind(node)
    if k < 0: return k, b""
    if node[4] == 255: return BAD_DEPTH, b""
    if k == 1:
        parent = public_of(node)
        if parent is None: return INVALID_KEY, b""
        data = node[45:] if index >= HARDENED else parent
    else:
        if index >= HARDENED: return NEEDS_PRIVATE, b""
        parent = data = node[45:]
        if lift(parent) is None: return INVALID_KEY, b""
    i = hmac.new(node[13:45], data + index.to_bytes(4, "big"), "sha512").digest()
    il = int.from_bytes(i[:32], "big")
    if il >= N: return INVALID_KEY, b""
    if k == 1:
        child = (il + int.from_bytes(node[46:], "big")) % N
        if child == 0: return INVALID_KEY, b""
        key = b"\0" + child.to_bytes(32, "big")
    else:
        point = add(mul(il), lift(parent))
        if point is None: return INVALID_KEY, b""
        key = ser_p(point)
    return 78, node[:4] + bytes([node[4] + 1]) + hash160(parent)[:4] + index.to_bytes(4, "big") + i[32:] + key


def neuter_node(node):
    k = kind(node)
    if k < 0: return k, b""
    if k == 0: return 78, node
    key = public_of(node)
    if key is None: return INVALID_KEY, b""
    return 78, PRIVATE[int.from_bytes(node[:4], "big")].to_bytes(4, "big") + node[4:45] + key


def decode_text(text):
    if len(text) != 111 or any(c not in ALPHABET for c in text): return BAD_TEXT, b""
    n = 0
    for c in text: n = n * 58 + ALPHABET.index(c)
    data = n.to_bytes(82, "big")
    if checksum(data[:78]) != data[78:]: return BAD_CHECKSUM, b""
    node = data[:78]
    if int.from_bytes(node[:4], "big") not in (XPRV, XPUB, TPRV, TPUB): return BAD_VERSION, b""
    if node[4] == 0 and node[5:13] != bytes(8): return BAD_DEPTH, b""
    k = kind(node)
    if k < 0: return k, b""
    if k == 1 and not private_ok(node[46:]): return INVALID_KEY, b""
    if k == 0 and lift(node[45:]) is None: return INVALID_KEY, b""
    return 78, node


def reference(c):
    """(result, output, touched) for one case."""
    op, data, room = c["op"], bytes.fromhex(c["input"]), c["room"]
    if op == "master":
        if not 16 <= len(data) <= 64: return BAD_SIZE, b"", False
        if room < 78: return CAPACITY, b"", False
        return (*master(data, c["testnet"]), True)
    if op == "decode":
        if room < 78: return CAPACITY, b"", False
        return (*decode_text(data.decode("latin-1")), True)
    if len(data) != 78: return BAD_SIZE, b"", False
    if room < {"fingerprint": 4, "encode": 111}.get(op, 78): return CAPACITY, b"", False
    if op == "ckd":
        if not 0 <= c["index"] < 2**32: return BAD_INDEX, b"", False
        return (*ckd_node(data, c["index"]), True)
    if op == "descend":
        if any(not 0 <= i < 2**32 for i in c["indexes"]): return BAD_INDEX, b"", False
        r = kind(data)
        if r < 0: return r, b"", True
        node = data
        for i in c["indexes"]:
            r, node = ckd_node(node, i)
            if r < 0: return r, b"", True
        return 78, node, True
    if op == "neuter": return (*neuter_node(data), True)
    if op == "fingerprint":
        r, node = neuter_node(data)
        return (4, hash160(node[45:])[:4], True) if r > 0 else (r, b"", True)
    k = kind(data)  # encode
    return (k, b"", True) if k < 0 else (111, to_text(data).encode(), True)


def case(op, data, room=OUT, index=0, indexes=(), testnet=False):
    c = dict(op=op, input=data.hex(), room=room, index=index, indexes=list(indexes), testnet=testnet)
    result, output, touched = reference(c)
    return dict(c, result=result, output=output.hex(), touched=touched)


# ---- the cases
def published():
    text = read("bip32/bip-0032.mediawiki").decode()
    parts = re.split(r"^===Test vector (\d)===$", text, flags=re.M)
    seeds, chains, invalid = {}, [], []
    for number, body in zip(parts[1::2], parts[2::2]):
        body = body.split("==Acknowledgements==")[0]
        if number == "5":
            invalid = re.findall(r"^\* (\S+) \((.+)\)$", body, re.M)
            continue
        seeds[int(number)] = bytes.fromhex(re.search(r"^Seed \(hex\): ([0-9a-f]+)$", body, re.M).group(1))
        for chain, pub, prv in re.findall(r"^\* Chain (\S+)\n\*\* ext pub: (\w+)\n\*\* ext prv: (\w+)$", body, re.M):
            path = [int(p.rstrip("H")) + (HARDENED if p.endswith("H") else 0)
                    for p in chain.replace("<sub>H</sub>", "H").split("/")[1:]]
            chains.append((int(number), path, pub, prv))
    trezor = [(bytes.fromhex(s), x) for _e, _m, s, x in json.loads(read("bip39/vectors.json"))["english"]]
    assert len(chains) == 17 and len(invalid) == 16 and len(trezor) == 24
    return seeds, chains, invalid, trezor


def broken(rng, node):
    """One field of a node changed."""
    b = bytearray(node)
    kind_ = rng.randrange(12)
    if kind_ == 0: b[0:4] = rng.choice([XPRV + 1, XPUB - 1, 0x0488ADE5, 0, rng.getrandbits(32)]).to_bytes(4, "big")
    elif kind_ == 1: b[45] = rng.choice([0, 1, 2, 3, 4, 6, 0xFF])
    elif kind_ == 2: b[4] = 255
    elif kind_ == 3: b[4] = 0; b[5:9] = rng.randbytes(4)
    elif kind_ == 4: b[46:] = bytes(32)
    elif kind_ == 5: b[46:] = N.to_bytes(32, "big")
    elif kind_ == 6: b[46:] = (N + rng.randrange(2**128)).to_bytes(32, "big")
    elif kind_ == 7: b[46:] = rng.randbytes(32)
    elif kind_ == 8: b[13:45] = rng.randbytes(32)
    elif kind_ == 9: b[rng.randrange(78)] ^= 1 << rng.randrange(8)
    elif kind_ == 10: return bytes(b[:77])
    else: return bytes(b) + b"\0"
    return bytes(b)


def text_edits(rng, text):
    t = list(text)
    at = rng.randrange(111)
    out = []
    t2 = t[:]; t2[at] = rng.choice([c for c in ALPHABET if c != t[at]]); out.append("".join(t2))
    t2 = t[:]; t2[at] = rng.choice("0OIl+/ \x00\x80"); out.append("".join(t2))
    out.append(text[:110]); out.append(text + rng.choice(ALPHABET))
    a, b = rng.randrange(111), rng.randrange(111)
    t2 = t[:]; t2[a], t2[b] = t2[b], t2[a]; out.append("".join(t2))
    return out


def payload(rng, node):
    """node with one field set to a value the decoder refuses, and a good checksum."""
    b = bytearray(node)
    which = rng.randrange(7)
    if which == 0: b[0:4] = rng.getrandbits(32).to_bytes(4, "big")
    elif which == 1: b[4] = 0; b[9:13] = rng.randrange(1, 2**32).to_bytes(4, "big")
    elif which == 2: b[4] = 0; b[5:9] = rng.randrange(1, 2**32).to_bytes(4, "big")
    elif which == 3: b[45] = rng.choice([1, 4, 5, 0x80])
    elif which == 4: b[46:] = rng.choice([0, N, N + 1, 2**256 - 1]).to_bytes(32, "big")
    elif which == 5: b[0:4] = XPUB.to_bytes(4, "big"); b[45] = 2; b[46:] = rng.randbytes(32)
    else: b[0:4] = rng.choice([XPRV, XPUB, TPRV, TPUB]).to_bytes(4, "big")
    data = bytes(b) + checksum(bytes(b))
    n = int.from_bytes(data, "big")
    out = ""
    while n: n, r = divmod(n, 58); out = ALPHABET[r] + out
    return out


def cases():
    out = []
    seeds, chains, invalid, trezor = published()
    nodes = []
    for seed, xprv in trezor:
        out.append(case("master", seed))
        out.append(case("decode", xprv.encode()))
    for number, path, pub, prv in chains:
        out += [case("descend", master(seeds[number], False)[1], indexes=path), case("decode", pub.encode()), case("decode", prv.encode())]
        nodes.append(decode_text(prv)[1]); nodes.append(decode_text(pub)[1])
    for text, _reason in invalid:
        out.append(case("decode", text.encode()))
    rng = random.Random(32)
    for i in range(160):
        seed = rng.randbytes(rng.randrange(16, 65))
        testnet = i % 3 == 0
        out.append(case("master", seed, testnet=testnet))
        node = master(seed, testnet)[1]
        for depth in range(rng.randrange(1, 4)):
            index = rng.choice([rng.randrange(HARDENED), HARDENED + rng.randrange(HARDENED), 0, HARDENED - 1, HARDENED, 2**32 - 1])
            r, child = ckd_node(node, index)
            out.append(case("ckd", node, index=index))
            pub = neuter_node(node)[1]
            out.append(case("ckd", pub, index=index))
            node = child if r > 0 else node
        nodes.append(node); nodes.append(neuter_node(node)[1])
        out.append(case("descend", master(seed, testnet)[1],
                        indexes=[rng.choice([rng.randrange(HARDENED), HARDENED + rng.randrange(HARDENED)]) for _ in range(rng.randrange(6))]))
    for node in nodes:
        out += [case("neuter", node), case("fingerprint", node), case("encode", node)]
        if kind(node) == 0: out.append(case("descend", node, indexes=[rng.randrange(HARDENED) for _ in range(rng.randrange(1, 4))]))
        text = to_text(node)
        out += [case("decode", t.encode("latin-1")) for t in text_edits(rng, text)]
        out.append(case("decode", payload(rng, node).encode()))
        bad = broken(rng, node)
        out += [case("ckd", bad, index=rng.choice([0, HARDENED])), case("neuter", bad), case("fingerprint", bad), case("encode", bad)]
        out.append(case("descend", bad, indexes=[]))
    # the limits
    node = nodes[0]
    out += [case("ckd", node, index=i) for i in (-1, 2**32, -(2**40), 2**40)]
    out += [case("descend", node, indexes=[0, -1]), case("descend", node, indexes=[2**32])]
    out += [case("master", rng.randbytes(n)) for n in (0, 15, 16, 64, 65, 100)]
    out += [case("master", rng.randbytes(32), room=77), case("ckd", node, room=77), case("descend", node, room=77),
            case("neuter", node, room=77), case("fingerprint", node, room=3), case("encode", node, room=110),
            case("decode", to_text(node).encode(), room=77)]
    deep = bytearray(nodes[1]); deep[4] = 254
    child = ckd_node(bytes(deep), 0)[1]
    out += [case("ckd", bytes(deep), index=0), case("ckd", child, index=0), case("encode", child)]
    out += [case("decode", ("z" * 111).encode()), case("decode", ("1" * 111).encode()), case("decode", b"")]
    return out


# ---- the native builds
class View(C.Structure):
    _fields_ = [("data", C.POINTER(C.c_uint8)), ("count", C.c_int64)]


class Indexes(C.Structure):
    _fields_ = [("data", C.POINTER(C.c_int64)), ("count", C.c_int64)]


def view(buffer, n):
    return View(C.cast(buffer, C.POINTER(C.c_uint8)), n)


def run(*args, **kw):
    r = subprocess.run([str(a) for a in args], capture_output=True, text=True, **kw)
    if r.returncode:
        raise RuntimeError(f"{args[0]} failed ({r.returncode}): {r.stderr or r.stdout}")
    return r


def native(lib, c):
    """One case through a native build: (result, output hex, buffers as they should be)."""
    data = bytes.fromhex(c["input"])
    inp = (C.c_uint8 * max(1, len(data))).from_buffer_copy(data.ljust(1, b"\0"))
    out = (C.c_uint8 * OUT)(*([0xA5] * OUT))
    work = (C.c_uint8 * WORK)(*([0x5A] * WORK))
    i, w, o = view(inp, len(data)), view(work, WORK), view(out, c["room"])
    op = c["op"]
    if op == "master": r = lib.bip32_master_ct(i, c["testnet"], w, o)
    elif op == "ckd": r = lib.bip32_ckd_ct(i, c["index"], w, o)
    elif op == "descend":
        ix = (C.c_int64 * max(1, len(c["indexes"])))(*c["indexes"])
        r = lib.bip32_descend_ct(i, Indexes(ix, len(c["indexes"])), w, o)
    else: r = getattr(lib, f"bip32_{op}_ct")(i, w, o)
    wiped = all(b == 0 for b in work) if c["touched"] else all(b == 0x5A for b in work)
    return r, bytes(out[:max(0, r)]).hex(), wiped and all(b == 0xA5 for b in out[max(0, r):])


NAMES = ["bip32_master_ct", "bip32_ckd_ct", "bip32_descend_ct", "bip32_neuter_ct", "bip32_fingerprint_ct",
         "bip32_encode_ct", "bip32_decode_ct"]


class Bip32(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix="coax-bip32-")
        cls.dir = Path(cls.temp.name)
        cls.libs = []
        for label, mode in [("checked", []), ("release", ["--release"])]:
            c = cls.dir / (label + ".c")
            c.write_text(run(COMPILER, "c", SOURCE, "--library", "--header", cls.dir / "bip32.h", *mode).stdout)
            run("cc", "-std=c11", "-O2", "-shared", "-fPIC", c, "-lm", "-o", cls.dir / (label + ".dylib"))
            lib = C.CDLL(str(cls.dir / (label + ".dylib")))
            lib.bip32_master_ct.argtypes = [View, C.c_bool, View, View]
            lib.bip32_ckd_ct.argtypes = [View, C.c_int64, View, View]
            lib.bip32_descend_ct.argtypes = [View, Indexes, View, View]
            for name in NAMES[3:]: getattr(lib, name).argtypes = [View, View, View]
            for name in NAMES: getattr(lib, name).restype = C.c_int64
            cls.libs.append((label, lib))
        cls.cases = cases()

    @classmethod
    def tearDownClass(cls):
        cls.temp.cleanup()

    def test_reference_on_published_vectors(self):
        """The Python reference gives every published answer."""
        seeds, chains, invalid, trezor = published()
        for number, path, pub, prv in chains:
            node = master(seeds[number], False)[1]
            for i in path: node = ckd_node(node, i)[1]
            self.assertEqual(to_text(node), prv)
            self.assertEqual(to_text(neuter_node(node)[1]), pub)
            self.assertEqual(decode_text(prv), (78, node))
        for text, _reason in invalid: self.assertLess(decode_text(text)[0], 0)
        for seed, xprv in trezor: self.assertEqual(to_text(master(seed, False)[1]), xprv)

    def test_native(self):
        for label, lib in self.libs:
            for c in self.cases:
                self.assertEqual(native(lib, c), (c["result"], c["output"], True), f"{label} {c}")
        results = {c["result"] for c in self.cases}
        self.assertTrue(set(range(-10, 0)) - {INVALID_KEY} <= results and INVALID_KEY in results, results)
        print(f"\n{len(self.cases)} cases agree with Python, native checked and release", file=sys.stderr)

    def test_wasm(self):
        clang, sysroot, resources = Path(WASM_CLANG), Path(WASM_SYSROOT), Path(WASM_RESOURCES)
        self.assertTrue(clang.exists() and sysroot.exists() and resources.exists(), "a Wasm toolchain is required; see coax/web/wasm.sh")
        fixture = self.dir / "wasm.json"
        fixture.write_text(json.dumps(self.cases))
        for label in ["checked", "release"]:
            module = self.dir / (label + ".wasm")
            run(clang, "--target=wasm32-wasip1", "--sysroot=" + str(sysroot), "-resource-dir=" + str(resources),
                "-std=c11", "-O2", "-I" + str(self.dir), "-mexec-model=reactor",
                self.dir / (label + ".c"), HERE / "bip32_wasm.c",
                "-Wl,--import-memory,--export-memory,--export=__heap_base,--max-memory=67108864,-z,stack-size=1048576,--gc-sections",
                "-o", module)
            result = json.loads(run("node", HERE / "bip32_wasm.cjs", module, fixture).stdout)
            self.assertEqual((result["agree"], result["cases"]), (len(self.cases), len(self.cases)), label)
        print(f"\n{len(self.cases)} cases agree with Python in Wasm checked and release", file=sys.stderr)


if __name__ == "__main__":
    unittest.main()
