#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Bitcoin Core's own Python MuHash3072 and a transliteration of Core's compressor decide each case outside Coax)
"""The UTXO set's records and hashes against Bitcoin Core.

Bitcoin From Scratch, chapter 56. Before any Coax code runs, this script
checks its Python reference against Core's published values:

- Two regtest chains are rebuilt from Core's pinned sources (miner.cpp's
  coinbase, the test framework's deterministic key and MiniWallet). The
  chain of rpc_dumptxoutset.py must reach its asserted block hash, its
  txoutset_hash (hash_serialized_3) and the SHA-256 of its dumptxoutset
  file, which holds every coin in Core's compressed form. The chain of
  feature_utxo_set_hash.py must reach its asserted hash_serialized_3 and
  muhash. The expected values are read from the pinned test files.
- Core's own Python MuHash3072 (test_framework/crypto/muhash.py and
  chacha20.py, pinned) gives crypto_tests.cpp's vectors.

Then it compiles coax/lib/bitcoin/coins.coax (which loads muhash.coax) to
C in checked and release builds and compares, against the reference:
MuHash3072 on seeded sets with inserts and removes; compress_tests.cpp's
amount ranges and seeded amounts; VARINTs; every script shape with seeded
keys on and off the curve; coin records written, read back and cut short;
and both chains' hashes through the Coax TxOutSer and MuHash.

    python3 coax/tests/bitcoin/utxo.py            # check
    python3 coax/tests/bitcoin/utxo.py --write    # regenerate utxo_vectors.coax
"""
import ctypes as C
import hashlib
import importlib.util
import os
from pathlib import Path
import random
import re
import struct
import subprocess
import sys
import tempfile
import types
import unittest

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
COMPILER = Path(os.environ.get("COAX_TEST_COMPILER", ROOT / "coax/bin/coax"))
SOURCE = ROOT / "coax/lib/bitcoin/coins.coax"
TABLE = HERE / "utxo_vectors.coax"
UPSTREAM = ROOT / "coax/tests/vectors/upstream/bitcoin-core"
PINS = {
    "muhash.py": "af45c2451c06abd530ae7d65f8778f7e1ebb76f7b68dcded3459fdcee376405c",
    "chacha20.py": "c0b383e8c1ee47cd3d0cb02630ad316cd8e915b857fa19193a7111d79cbe4878",
    "crypto_tests.cpp": "6eefd34081d99a0ca68ce357a7eeef15864439a566b8563031c0df1fc2ffaa40",
    "compress_tests.cpp": "dba41e6bea1fc0c19d1611bb27d87964da75399b8d7edfa73df1c31433a4ded6",
    "feature_utxo_set_hash.py": "39882a860fc1bfaaf135bc82067542486c18beaf79397c749edfbe77bde835bc",
    "rpc_dumptxoutset.py": "78005ac31b4df70964714dcc407fb5571a89350a9fc8d78381d2a8b979dba487",
    "miner.cpp": "13fee188aaf49b13545d583b334b04517dad04bf43fa62691528a8abbcf0d559",
    "test_node.py": "e00688c4aa2fe2991376c4835941804421ca21fe05700052b2118286a1e92a20",
    "address.py": "10ff2a6ad2cb58200741fd345163d36315fe57b2583da0e919e72bdd825346d3",
    "wallet.py": "6787e3d41e80676260692946aaa448de71de0c772d6b7bb1d85fbf6c9535fbe4",
}


def read(name):
    raw = (UPSTREAM / name).read_bytes()
    assert hashlib.sha256(raw).hexdigest() == PINS[name], f"{name} does not match its pin"
    return raw.decode()


def sha256(b): return hashlib.sha256(b).digest()
def sha256d(b): return sha256(sha256(b))


def core_muhash():
    """Core's MuHash3072 class, loaded from the pinned test framework files."""
    pkg = types.ModuleType("core_crypto")
    pkg.__path__ = []
    sys.modules["core_crypto"] = pkg
    for name in ["chacha20", "muhash"]:
        text = read(name + ".py")
        mod = types.ModuleType("core_crypto." + name)
        mod.__package__ = "core_crypto"
        sys.modules["core_crypto." + name] = mod
        exec(compile(text, str(UPSTREAM / (name + ".py")), "exec"), mod.__dict__)
    return sys.modules["core_crypto.muhash"].MuHash3072


# ---- the reference: Core's serializers and compressor, transliterated

def compact(n):
    if n < 253: return bytes([n])
    if n <= 0xffff: return b"\xfd" + struct.pack("<H", n)
    if n <= 0xffffffff: return b"\xfe" + struct.pack("<I", n)
    return b"\xff" + struct.pack("<Q", n)


def varint(n):
    """serialize.h WriteVarInt."""
    out = bytearray()
    while True:
        out.insert(0, (n & 0x7f) | (0x80 if out else 0))
        if n <= 0x7f: return bytes(out)
        n = (n >> 7) - 1


def read_varint(b, at, maximum):
    """serialize.h ReadVarInt: (value, next) or an error string."""
    n = 0
    while True:
        if at >= len(b): return "end of data"
        ch = b[at]; at += 1
        if n > (maximum >> 7): return "size too large"
        n = (n << 7) | (ch & 0x7f)
        if ch & 0x80:
            if n == maximum: return "size too large"
            n += 1
        else:
            return n, at


def compress_amount(n):
    if n == 0: return 0
    e = 0
    while n % 10 == 0 and e < 9: n //= 10; e += 1
    if e < 9:
        d = n % 10; n //= 10
        return 1 + (n * 9 + d - 1) * 10 + e
    return 1 + (n - 1) * 10 + 9


def decompress_amount(x):
    if x == 0: return 0
    x -= 1
    e = x % 10; x //= 10
    if e < 9:
        d = x % 9 + 1; x //= 9
        n = (x * 10 + d) % 2**64
    else:
        n = x + 1
    while e:
        n = n * 10 % 2**64; e -= 1
    return n


P = 2**256 - 2**32 - 977
N = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141
G = (0x79BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798,
     0x483ADA7726A3C4655DA4FBFC0E1108A8FD17B448A68554199C47D08FFB10D4B8)


def ec_add(a, b):
    if a is None: return b
    if b is None: return a
    if a[0] == b[0] and (a[1] + b[1]) % P == 0: return None
    if a == b: lam = 3 * a[0] * a[0] * pow(2 * a[1], -1, P) % P
    else: lam = (b[1] - a[1]) * pow(b[0] - a[0], -1, P) % P
    x = (lam * lam - a[0] - b[0]) % P
    return (x, (lam * (a[0] - x) - a[1]) % P)


def ec_mul(k, pt):
    r = None
    while k:
        if k & 1: r = ec_add(r, pt)
        pt = ec_add(pt, pt); k >>= 1
    return r


def lift_x(x, odd):
    if x >= P: return None
    y = pow((x * x * x + 7) % P, (P + 1) // 4, P)
    if y * y % P != (x * x * x + 7) % P: return None
    return (x, y if y % 2 == odd else P - y)


def compress_script(s):
    if len(s) == 25 and s[0] == 0x76 and s[1] == 0xa9 and s[2] == 20 and s[23] == 0x88 and s[24] == 0xac:
        return b"\x00" + s[3:23]
    if len(s) == 23 and s[0] == 0xa9 and s[1] == 20 and s[22] == 0x87:
        return b"\x01" + s[2:22]
    if len(s) == 35 and s[0] == 33 and s[34] == 0xac and s[1] in (2, 3):
        return s[1:34]
    if len(s) == 67 and s[0] == 65 and s[66] == 0xac and s[1] == 4:
        x, y = int.from_bytes(s[2:34], "big"), int.from_bytes(s[34:66], "big")
        if x < P and y < P and (y * y - x * x * x - 7) % P == 0:
            return bytes([4 | (s[65] & 1)]) + s[2:34]
    return None


def decompress_script(kind, data):
    if kind == 0: return b"\x76\xa9\x14" + data + b"\x88\xac"
    if kind == 1: return b"\xa9\x14" + data + b"\x87"
    if kind in (2, 3): return bytes([33, kind]) + data + b"\xac"
    pt = lift_x(int.from_bytes(data, "big"), kind - 4)
    if pt is None: return b""
    return b"\x41\x04" + pt[0].to_bytes(32, "big") + pt[1].to_bytes(32, "big") + b"\xac"


def coin_write(value, height, coinbase, script):
    short = compress_script(script)
    body = varint(height * 2 + coinbase) + varint(compress_amount(value))
    return body + (short if short is not None else varint(len(script) + 6) + script)


def coin_read(b):
    """Coin::Unserialize: (code, next, value, height, coinbase, script, bad_key)."""
    r = read_varint(b, 0, 2**32 - 1)
    if isinstance(r, str): return r
    code, at = r
    r = read_varint(b, at, 2**64 - 1)
    if isinstance(r, str): return r
    amount, at = r
    r = read_varint(b, at, 2**32 - 1)
    if isinstance(r, str): return r
    size, at = r
    if size < 6:
        need = 20 if size < 2 else 32
        if at + need > len(b): return "end of data"
        s = decompress_script(size, b[at:at + need])
        return (at + need, decompress_amount(amount), code >> 1, code & 1, s, s == b"")
    size -= 6
    if at + size > len(b): return "end of data"
    if size > 10000: return (at + size, decompress_amount(amount), code >> 1, code & 1, b"\x6a", False)
    return (at + size, decompress_amount(amount), code >> 1, code & 1, b[at:at + size], False)


def txout_ser(txid, n, height, coinbase, value, script):
    """coinstats.cpp TxOutSer."""
    return txid + struct.pack("<II", n, height * 2 + coinbase) + struct.pack("<Q", value) + compact(len(script)) + script


# ---- the regtest chains of the two functional tests

B58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"


def b58check(text):
    n = 0
    for c in text: n = n * 58 + B58.index(c)
    raw = n.to_bytes(25, "big")
    assert sha256d(raw[:21])[:4] == raw[21:]
    return raw[:21]


def push_int(n):
    """CScript() << n for a positive height (script.h push_int64)."""
    if 1 <= n <= 16: return bytes([0x50 + n])
    v = n.to_bytes((n.bit_length() + 8) // 8, "little")
    while len(v) > 1 and v[-1] == 0 and not (v[-2] & 0x80): v = v[:-1]
    return bytes([len(v)]) + v


def coinbase(height, spk, value, witness_root=bytes(32)):
    """miner.cpp's coinbase: version 2, BIP-34 height (with OP_0 up to 16),
    sequence MAX_SEQUENCE_NONFINAL, nLockTime height - 1, and the witness
    commitment GenerateCoinbaseCommitment adds. Returns the stripped bytes."""
    sig = push_int(height) + (b"\x00" if height <= 16 else b"")
    commit = b"\x6a\x24\xaa\x21\xa9\xed" + sha256d(witness_root + bytes(32))
    outs = [(value, spk), (0, commit)]
    body = struct.pack("<i", 2) + b"\x01" + bytes(32) + b"\xff\xff\xff\xff" + compact(len(sig)) + sig
    body += struct.pack("<I", 0xfffffffe) + compact(len(outs))
    body += b"".join(struct.pack("<q", v) + compact(len(s)) + s for v, s in outs)
    return body + struct.pack("<I", height - 1)


def node0_script():
    """test_node.py's PRIV_KEYS[0] address, the target of TestNode.generate."""
    address = re.search(r"AddressKeyPair\('(\w+)'", read("test_node.py")).group(1)
    return b"\x76\xa9\x14" + b58check(address)[1:] + b"\x88\xac"


def tagged(tag, msg):
    t = sha256(tag.encode())
    return sha256(t + t + msg)


def miniwallet():
    """MiniWallet's ADDRESS_OP_TRUE output: internal key 1, one leaf OP_TRUE."""
    assert "internal_key = explicit_internal_key or (1).to_bytes(32, 'big')" in read("address.py")
    internal = (1).to_bytes(32, "big")
    leaf = tagged("TapLeaf", b"\xc0\x01\x51")
    t = int.from_bytes(tagged("TapTweak", internal + leaf), "big")
    q = ec_add(lift_x(1, 0), ec_mul(t, G))
    return b"\x51\x20" + q[0].to_bytes(32, "big"), bytes([0xc0 | (q[1] & 1)]) + internal


GENESIS = "0f9188f13cb7b2c71f2a335e3a4fc328bf5beb436012afca590b1a11466e2206"
GENESIS_TIME = 1296688602


def mine(txids_by_height):
    """Headers for the coinbase txids: CreateNewBlock's time under
    setmocktime(genesis + 1), then GenerateBlock's nonce search. Returns
    the tip hash in hash order."""
    prev = bytes.fromhex(GENESIS)[::-1]
    times = [GENESIS_TIME]
    target = 0x7fffff << (8 * 29)
    for root in txids_by_height:
        mtp = sorted(times[-11:])[len(times[-11:]) // 2]
        t = max(mtp + 1, GENESIS_TIME + 1)
        nonce = 0
        while True:
            h = sha256d(struct.pack("<i", 0x20000000) + prev + root + struct.pack("<III", t, 0x207fffff, nonce))
            if int.from_bytes(h, "little") <= target: break
            nonce += 1
        prev = h
        times.append(t)
    return prev


def dumptxoutset_chain():
    """rpc_dumptxoutset.py: 100 blocks to node 0. Returns (tip, coins)."""
    spk = node0_script()
    txids = [sha256d(coinbase(h, spk, 50 * 10**8)) for h in range(1, 101)]
    coins = [(txids[h - 1], 0, h, 1, 50 * 10**8, spk) for h in range(1, 101)]
    return mine(txids), sorted(coins)


def utxo_set_hash_chain():
    """feature_utxo_set_hash.py: block 1 to the MiniWallet, 2 to 100 to node 0,
    then a self-transfer of block 1's coinbase mined by generateblock."""
    spk, wspk = node0_script(), None
    wspk, control = miniwallet()
    cb = [sha256d(coinbase(h, wspk if h == 1 else spk, 50 * 10**8)) for h in range(1, 101)]
    # create_self_transfer: fee_rate 0.003 BTC/kvB at the 104 vbytes it asserts.
    wallet = read("wallet.py")
    assert 'fee_rate=Decimal("0.003")' in wallet and "vsize = Decimal(104)" in wallet
    value = 50 * 10**8 - 3 * 10**5 * 104 // 1000
    inp = cb[0] + struct.pack("<I", 0) + b"\x00" + struct.pack("<I", 0)
    out = struct.pack("<q", value) + compact(len(wspk)) + wspk
    stripped = struct.pack("<i", 2) + b"\x01" + inp + b"\x01" + out + struct.pack("<I", 0)
    witness = b"\x02\x01\x51" + compact(len(control)) + control
    full = struct.pack("<i", 2) + b"\x00\x01\x01" + inp + b"\x01" + out + witness + struct.pack("<I", 0)
    assert (3 * len(stripped) + len(full) + 3) // 4 == 104
    txid = sha256d(stripped)
    # generateblock: CreateNewTemplate without the mempool (subsidy only),
    # the transaction appended, then RegenerateCommitments.
    root = sha256d(bytes(32) + sha256d(full))
    cb101 = sha256d(coinbase(101, wspk, 50 * 10**8, root))
    coins = [(cb[h - 1], 0, h, 1, 50 * 10**8, spk) for h in range(2, 101)]
    coins += [(cb101, 0, 101, 1, 50 * 10**8, wspk), (txid, 0, 101, 0, value, wspk)]
    return sorted(coins)


def hash_serialized(coins):
    return sha256d(b"".join(txout_ser(*c) for c in coins))[::-1].hex()


def snapshot(tip, coins):
    """dumptxoutset's file: SnapshotMetadata, then per txid its coins."""
    out = b"utxo\xff" + struct.pack("<H", 2) + bytes.fromhex("fabfb5da") + tip + struct.pack("<Q", len(coins))
    for txid, n, h, cb, v, s in coins:
        out += txid + compact(1) + compact(n) + coin_write(v, h, cb, s)
    return out


def published():
    """The values the two functional tests assert, read from their files."""
    d = read("rpc_dumptxoutset.py")
    u = read("feature_utxo_set_hash.py")
    return {
        "base_hash": re.search(r"'base_hash'\],\s*'([0-9a-f]{64})'", d).group(1),
        "file_sha256": re.search(r"hex\(\),\s*'([0-9a-f]{64})'", d).group(1),
        "txoutset_hash": re.search(r"'txoutset_hash'\], '([0-9a-f]{64})'", d).group(1),
        "hash_serialized_3": re.search(r"\['hash_serialized_3'\], \"([0-9a-f]{64})\"", u).group(1),
        "muhash": re.search(r"\['muhash'\], \"([0-9a-f]{64})\"", u).group(1),
    }


def crypto_tests_vectors():
    t = read("crypto_tests.cpp")
    return {
        "ratio": re.findall(r'uint256\{"([0-9a-f]{64})"\}', t)[0],
        "serialized": re.search(r'std::string ser_exp = "([0-9a-f]+)"', t).group(1),
        "overflow": re.search(r'HexStr\(out4\), "([0-9a-f]{64})"', t).group(1),
    }


def check_reference(MuHash3072):
    """The reference reproduces every published value before Coax runs."""
    pub = published()
    tip, coins = dumptxoutset_chain()
    assert tip[::-1].hex() == pub["base_hash"], "block 100's hash"
    assert hash_serialized(coins) == pub["txoutset_hash"]
    assert hashlib.sha256(snapshot(tip, coins)).hexdigest() == pub["file_sha256"]
    for c in coins:
        rec = coin_write(c[4], c[2], c[3], c[5])
        assert coin_read(rec) == (len(rec), c[4], c[2], c[3], c[5], False)
    wcoins = utxo_set_hash_chain()
    assert hash_serialized(wcoins) == pub["hash_serialized_3"]
    m = MuHash3072()
    for c in wcoins: m.insert(txout_ser(*c))
    assert m.digest()[::-1].hex() == pub["muhash"]
    v = crypto_tests_vectors()
    m = MuHash3072()
    m.insert(bytes(32)); m.insert(b"\x01" + bytes(31)); m.remove(b"\x02" + bytes(31))
    assert m.digest()[::-1].hex() == v["ratio"]
    m = MuHash3072()
    m.insert(b"\x01" + bytes(31)); m.insert(b"\x02" + bytes(31))
    assert (m.numerator.to_bytes(384, "little") + (1).to_bytes(384, "little")).hex() == v["serialized"]
    assert sha256(((2**3072 - 1) % MuHash3072.MODULUS).to_bytes(384, "little")).hex() == v["overflow"]
    # compress_tests.cpp's pairs.
    t = read("compress_tests.cpp")
    for dec, enc in [(0, 0), (1, 1), (10**6, 7), (10**8, 9), (50 * 10**8, 0x32), (21 * 10**14, 0x1406f40)]:
        assert compress_amount(dec) == enc and decompress_amount(enc) == dec
    assert "TestPair(21000000*COIN, 0x1406f40)" in t
    return pub, tip, coins, wcoins


def table():
    """The two chains' coins and Core's values, as Coax source."""
    pub = published()
    tip, coins = dumptxoutset_chain()
    wcoins = utxo_set_hash_chain()
    def rows(cs):
        return "\n".join(f'  UtxoVector.{{txid="{c[0].hex()}", n={c[1]}, height={c[2]}, coinbase={"true" if c[3] else "false"}, value={c[4]}, script="{c[5].hex()}"}},' for c in cs)
    return f"""// Generated by coax/tests/bitcoin/utxo.py --write. Test data only.
// Two regtest UTXO sets rebuilt from Bitcoin Core's pinned sources (see
// SOURCES.md): rpc_dumptxoutset.py's 100 coinbases and
// feature_utxo_set_hash.py's 101 coins, each in chainstate order (txid in
// hash order, then n), with the values those tests assert.
UtxoVector :: struct {{txid: string; n: int; height: int; coinbase: bool; value: u64; script: string;}}
UTXO_DUMP_TIP :: "{tip.hex()}";
UTXO_DUMP_BASE_HASH :: "{pub['base_hash']}";
UTXO_DUMP_TXOUTSET_HASH :: "{pub['txoutset_hash']}";
UTXO_DUMP_FILE_SHA256 :: "{pub['file_sha256']}";
UTXO_DUMP_COUNT :: {len(coins)};
UTXO_DUMP :: [{len(coins)}] UtxoVector.[
{rows(coins)}
];
UTXO_SET_HASH_SERIALIZED :: "{pub['hash_serialized_3']}";
UTXO_SET_MUHASH :: "{pub['muhash']}";
UTXO_SET_COUNT :: {len(wcoins)};
UTXO_SET :: [{len(wcoins)}] UtxoVector.[
{rows(wcoins)}
];
"""


# ---- the native comparison

class View(C.Structure):
    _fields_ = [("data", C.POINTER(C.c_uint8)), ("count", C.c_int64)]


def buf(n, fill=0xA5):
    return (C.c_uint8 * max(1, n))(*([fill] * max(1, n)))


def view(b, n):
    return View(C.cast(b, C.POINTER(C.c_uint8)), n)


def bview(data):
    b = (C.c_uint8 * max(1, len(data)))(*data) if data else buf(1)
    return b, view(b, len(data))


def run(*args, **kw):
    r = subprocess.run([str(a) for a in args], capture_output=True, text=True, **kw)
    if r.returncode:
        raise RuntimeError(f"{args[0]} failed ({r.returncode}): {r.stderr or r.stdout}")
    return r


def random_key(rng, uncompressed):
    pt = ec_mul(rng.randrange(1, N), G)
    if uncompressed: return b"\x04" + pt[0].to_bytes(32, "big") + pt[1].to_bytes(32, "big")
    return bytes([2 + (pt[1] & 1)]) + pt[0].to_bytes(32, "big")


def scripts(rng):
    out = [b"", b"\x6a", b"\x51", bytes(10001), bytes(10000)]
    for _ in range(40):
        h = rng.randbytes(20)
        out += [b"\x76\xa9\x14" + h + b"\x88\xac", b"\xa9\x14" + h + b"\x87", b"\x00\x14" + h,
                b"\x76\xa9\x14" + h + b"\x88\xab", b"\xa9\x14" + h + b"\x88"]
        k = random_key(rng, False)
        out += [b"\x21" + k + b"\xac", b"\x21\x04" + k[1:] + b"\xac"]
        u = random_key(rng, True)
        out += [b"\x41" + u + b"\xac", b"\x41" + u[:-1] + bytes([u[-1] ^ 1]) + b"\xac", b"\x41\x06" + u[1:] + b"\xac"]
        out += [b"\x51\x20" + rng.randbytes(32), rng.randbytes(rng.randrange(0, 80))]
    return out


class Utxo(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.MuHash3072 = core_muhash()
        cls.pub, cls.tip, cls.coins, cls.wcoins = check_reference(cls.MuHash3072)
        assert TABLE.read_text() == table(), f"{TABLE.relative_to(ROOT)} is stale: run with --write"
        cls.temp = tempfile.TemporaryDirectory(prefix="coax-utxo-")
        d = Path(cls.temp.name)
        cls.libs = []
        for label, mode in [("checked", []), ("release", ["--release"])]:
            c = d / (label + ".c")
            c.write_text(run(COMPILER, "c", SOURCE, "--library", "--header", d / "coins.h", *mode).stdout)
            run("cc", "-std=c11", "-O2", "-shared", "-fPIC", c, "-lm", "-o", d / (label + ".dylib"))
            lib = C.CDLL(str(d / (label + ".dylib")))
            lib.muhash_new_bytes.argtypes = [View]
            lib.muhash_update_bytes_vartime.argtypes = [View, View, C.c_bool]
            lib.muhash_finalize_bytes_vartime.argtypes = [View, View]
            lib.coin_compress_amount_c.argtypes = [C.c_uint64]
            lib.coin_compress_amount_c.restype = C.c_uint64
            lib.coin_decompress_amount_c.argtypes = [C.c_uint64]
            lib.coin_decompress_amount_c.restype = C.c_uint64
            lib.coin_write_c.argtypes = [C.c_uint64, C.c_int64, C.c_bool, View, View]
            lib.coin_read_c.argtypes = [View, View, View]
            lib.coin_varint_c.argtypes = [View, C.c_uint64, View]
            lib.coin_varint_write_c.argtypes = [View, C.c_uint64]
            lib.coin_compress_script_c.argtypes = [View, View]
            lib.coin_txout_ser_c.argtypes = [View, C.c_int64, C.c_uint64, C.c_int64, C.c_bool, View, View]
            for f in ["muhash_new_bytes", "muhash_update_bytes_vartime", "muhash_finalize_bytes_vartime", "coin_write_c",
                      "coin_read_c", "coin_varint_c", "coin_varint_write_c", "coin_compress_script_c", "coin_txout_ser_c"]:
                getattr(lib, f).restype = C.c_int64
            cls.libs.append((label, lib))
        cls.counts = {}

    @classmethod
    def tearDownClass(cls):
        cls.temp.cleanup()
        print(f"\n{cls.counts}", file=sys.stderr)

    def count(self, k, n=1):
        self.counts[k] = self.counts.get(k, 0) + n

    def muhash(self, lib, ops):
        st = buf(768)
        assert lib.muhash_new_bytes(view(st, 768)) == 0
        for data, remove in ops:
            keep, v = bview(data)
            assert lib.muhash_update_bytes_vartime(view(st, 768), v, remove) == 0
        out = buf(33)
        assert lib.muhash_finalize_bytes_vartime(view(st, 768), view(out, 32)) == 0 and out[32] == 0xA5
        return bytes(out[:32]), bytes(st[:768])

    def test_muhash(self):
        rng = random.Random(56)
        sets = [[(bytes([0] + [0] * 31), False), (bytes([1] + [0] * 31), False), (bytes([2] + [0] * 31), True)], []]
        for _ in range(12):
            ops = [(rng.randbytes(rng.randrange(0, 120)), False) for _ in range(rng.randrange(0, 6))]
            ops += [(rng.randbytes(rng.randrange(0, 120)), True) for _ in range(rng.randrange(0, 3))]
            rng.shuffle(ops)
            sets.append(ops)
            # Add then remove the same element: the hash is unchanged.
            extra = rng.randbytes(57)
            sets.append(ops + [(extra, False), (extra, True)])
        for label, lib in self.libs:
            for ops in sets:
                m = self.MuHash3072()
                for data, remove in ops: (m.remove if remove else m.insert)(data)
                out, state = self.muhash(lib, ops)
                self.assertEqual(out, m.digest(), f"{label} {ops}")
                self.assertEqual(state[:384], m.numerator.to_bytes(384, "little"))
                self.assertEqual(state[384:], m.denominator.to_bytes(384, "little"))
                self.count("muhash sets")
            out, _ = self.muhash(lib, sets[0])
            self.assertEqual(out[::-1].hex(), crypto_tests_vectors()["ratio"])

    def test_amounts(self):
        rng = random.Random(5656)
        COIN, CENT = 10**8, 10**6
        values = list(range(0, 100001)) + [i * CENT for i in range(1, 10001)] + [i * COIN for i in range(1, 10001)]
        values += [i * 50 * COIN for i in range(1, 420001, 7)] + [rng.randrange(0, 21 * 10**14 + 1) for _ in range(20000)]
        for label, lib in self.libs:
            for v in values:
                c = lib.coin_compress_amount_c(v)
                self.assertEqual(c, compress_amount(v), v)
                self.assertEqual(lib.coin_decompress_amount_c(c), v)
            for x in list(range(100000)) + [rng.randrange(0, 2**64) for _ in range(20000)] + [2**64 - 1]:
                self.assertEqual(lib.coin_decompress_amount_c(x), decompress_amount(x), x)
            self.count("amounts", len(values) + 120001)

    def test_varints(self):
        rng = random.Random(565)
        values = [0, 1, 127, 128, 255, 256, 16383, 16384, 16511, 16512, 2**32 - 1, 2**32, 2**64 - 1]
        values += [rng.randrange(0, 2**rng.randrange(1, 65)) for _ in range(3000)]
        for label, lib in self.libs:
            for v in values:
                out = buf(11)
                n = lib.coin_varint_write_c(view(out, 10), v)
                self.assertEqual(bytes(out[:n]), varint(v)); self.assertEqual(out[10], 0xA5)
                for maximum in (2**32 - 1, 2**64 - 1):
                    keep, b = bview(varint(v))
                    f = (C.c_uint64 * 2)()
                    code = lib.coin_varint_c(b, maximum, view(C.cast(f, C.POINTER(C.c_uint8)), 2))
                    want = read_varint(varint(v), 0, maximum)
                    self.assertEqual(code, 0 if not isinstance(want, str) else 2, (v, maximum))
                    if code == 0: self.assertEqual((f[0], f[1]), want)
            # Seeded byte strings: every outcome as ReadVarInt's.
            for _ in range(3000):
                raw = bytes(rng.choice([0x80, 0xff, 0xfe, 0x7f, rng.randrange(256)]) for _ in range(rng.randrange(0, 12)))
                for maximum in (2**32 - 1, 2**64 - 1):
                    keep, b = bview(raw)
                    f = (C.c_uint64 * 2)()
                    code = lib.coin_varint_c(b, maximum, view(C.cast(f, C.POINTER(C.c_uint8)), 2))
                    want = read_varint(raw, 0, maximum)
                    self.assertEqual(code, {"end of data": 1, "size too large": 2}.get(want, 0) if isinstance(want, str) else 0, raw.hex())
                    if code == 0: self.assertEqual((f[0], f[1]), want)
            self.count("varints", len(values) * 2 + 6000)

    def test_coins(self):
        rng = random.Random(56565)
        all_scripts = scripts(rng)
        for label, lib in self.libs:
            for s in all_scripts:
                keep, sv = bview(s)
                out = buf(34)
                k = lib.coin_compress_script_c(sv, view(out, 33))
                want = compress_script(s)
                self.assertEqual(k, 0 if want is None else len(want), s.hex())
                if want: self.assertEqual(bytes(out[:k]), want)
                value = rng.choice([0, 1, 50 * 10**8, 21 * 10**14, rng.randrange(0, 21 * 10**14)])
                height, cb = rng.choice([0, 1, 2**31 - 1, rng.randrange(0, 900000)]), rng.randrange(2)
                rec = coin_write(value, height, cb, s)
                o = buf(len(rec) + 1)
                n = lib.coin_write_c(value, height, cb, sv, view(o, len(rec)))
                self.assertEqual(bytes(o[:n]), rec); self.assertEqual(o[len(rec)], 0xA5)
                self.assertEqual(lib.coin_write_c(value, height, cb, sv, view(o, len(rec) - 1)), -3)
                for cut in sorted({len(rec), len(rec) - 1, 0, rng.randrange(len(rec) + 1)}):
                    self.check_read(lib, rec[:cut])
                self.count("scripts and records")
            # Corrupted records: seeded edits of real ones, and records whose
            # special kind 4 or 5 holds an x with no point.
            for _ in range(1500):
                s = rng.choice(all_scripts)
                rec = bytearray(coin_write(rng.randrange(0, 10**9), rng.randrange(0, 10**6), rng.randrange(2), s))
                for _ in range(rng.randrange(1, 4)):
                    rec[rng.randrange(len(rec))] = rng.randrange(256)
                self.check_read(lib, bytes(rec))
            self.check_read(lib, bytes([2, 1, 4]) + (5).to_bytes(32, "big"))
            self.count("corrupted records", 1501)

    def check_read(self, lib, rec):
        keep, b = bview(rec)
        script = buf(10001)
        f = (C.c_uint64 * 7)()
        code = lib.coin_read_c(b, view(script, 10000), view(C.cast(f, C.POINTER(C.c_uint8)), 7))
        want = coin_read(rec)
        if isinstance(want, str):
            self.assertEqual(code, {"end of data": 1, "size too large": 2}[want], rec.hex())
            return
        self.assertEqual(code, 0, rec.hex())
        nxt, value, height, cb, s, bad = want
        self.assertEqual((f[1], f[2], f[3], f[4], f[6]), (nxt, value, height, cb, int(bad)), rec.hex())
        self.assertEqual(bytes(script[:f[5]]), s)
        self.assertEqual(script[10000], 0xA5)

    def test_chains(self):
        for label, lib in self.libs:
            for coins, want_hs, want_mh in [(self.coins, self.pub["txoutset_hash"], None),
                                            (self.wcoins, self.pub["hash_serialized_3"], self.pub["muhash"])]:
                stream = b""
                st = buf(768)
                assert lib.muhash_new_bytes(view(st, 768)) == 0
                for txid, n, h, cb, v, s in coins:
                    keep, t = bview(txid); keep2, sv = bview(s)
                    out = buf(200)
                    k = lib.coin_txout_ser_c(t, n, v, h, cb, sv, view(out, 199))
                    self.assertEqual(bytes(out[:k]), txout_ser(txid, n, h, cb, v, s))
                    stream += bytes(out[:k])
                    keep3, rv = bview(bytes(out[:k]))
                    assert lib.muhash_update_bytes_vartime(view(st, 768), rv, False) == 0
                self.assertEqual(sha256d(stream)[::-1].hex(), want_hs)
                fin = buf(32)
                assert lib.muhash_finalize_bytes_vartime(view(st, 768), view(fin, 32)) == 0
                if want_mh: self.assertEqual(bytes(fin)[::-1].hex(), want_mh)
                self.count("chain coins", len(coins))
            # The dumptxoutset file, coin records through Coax.
            body = b"utxo\xff" + struct.pack("<H", 2) + bytes.fromhex("fabfb5da") + self.tip + struct.pack("<Q", len(self.coins))
            for txid, n, h, cb, v, s in self.coins:
                keep, sv = bview(s)
                o = buf(64)
                k = lib.coin_write_c(v, h, cb, sv, view(o, 64))
                body += txid + compact(1) + compact(n) + bytes(o[:k])
            self.assertEqual(hashlib.sha256(body).hexdigest(), self.pub["file_sha256"])


if __name__ == "__main__":
    if "--write" in sys.argv[1:]:
        core_muhash()
        TABLE.write_text(table())
        sys.exit(0)
    if "--pins" in sys.argv[1:]:
        for n in PINS: print(n, hashlib.sha256((UPSTREAM / n).read_bytes()).hexdigest())
        sys.exit(0)
    unittest.main()
