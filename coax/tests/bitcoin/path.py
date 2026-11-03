#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Python's int(), hmac and integers decide each case outside Coax)
"""Derivation paths against a separate Python implementation.

Bitcoin From Scratch, chapter 33. Compiles coax/lib/bitcoin/path.coax to C
in checked and release builds and runs every procedure it exports on:

- the paths of BIP-44, BIP-49, BIP-84, BIP-86 and NIP-06, and the account
  keys and addresses of BIP-84's, BIP-86's and BIP-49's vectors, pinned in
  coax/tests/vectors/upstream/;
- seeded path strings: valid ones of every depth from 0 to 257, steps
  around 2^31, and each with one seeded edit (a character replaced,
  inserted or removed);
- index lists to format, with both markers, indexes out of range and
  buffers one byte too small;
- every purpose from 0 to 100 and seeded fields for the standard paths;
- seeded seeds with seeded paths, and account nodes (private and public,
  at depths 2 to 4) with every change and seeded indexes.

The reference below shares no code with Coax: Python's str and int for
paths, and chapter 32's bip32.py and chapter 29's address.py references for
derivation and scripts. Each result and output must match, and nothing may
be written past the output. Run from the repository root:

    python3 coax/tests/bitcoin/path.py
"""
import ctypes as C
import hashlib
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
sys.path.insert(0, str(HERE))
import bip32 as B32  # noqa: E402
import address as AD  # noqa: E402

COMPILER = Path(os.environ.get("COAX_TEST_COMPILER", ROOT / "coax/bin/coax"))
SOURCE = ROOT / "coax/lib/bitcoin/path.coax"
UPSTREAM = ROOT / "coax/tests/vectors/upstream"
PINS = {
    "bip44/bip-0044.mediawiki": "6a541c79f94077ac529941a8cbd373ce077a5d6e97b6dd8209b45ac10f737eaf",
    "bip84/bip-0084.mediawiki": "1900feec6cafca65b8c09906ca0658d2d742b4c9b44cb15678996985b6bfe627",
    "bip86/bip-0086.mediawiki": "d8d01dee331da07c2562615bc1f064c1868ec3fce61184973da76d1196c7f5b0",
    "bip49/bip-0049.mediawiki": "307b62faf7b9927233b5112e93d591272ba0c48303b5dbb6f0bbfbaf2347e695",
}
NIP06 = ROOT / "nostr/nips/official/06.md"
NIP06_PIN = "a7d86f43c3723b7a67765e81910a2478007d4655024e2899a163a09e2c834b3a"


def read(name):
    raw = (UPSTREAM / name).read_bytes()
    assert hashlib.sha256(raw).hexdigest() == PINS[name], name
    return raw.decode()


# ---- the Python reference
NO_ROOT, EMPTY_STEP, BAD_STEP, OVERFLOW, TOO_DEEP, CAPACITY, BAD_PURPOSE, BAD_FIELD, BAD_NETWORK, NOT_ACCOUNT = range(-1, -11, -1)
BIP32, ADDRESS = -20, -40
H = 2**31
KINDS = {44: AD.P2PKH, 49: AD.P2SH, 84: AD.P2WPKH, 86: AD.P2TR}
BUILD = {44: "p2pkh", 49: "p2sh_p2wpkh", 84: "p2wpkh", 86: "p2tr"}


def step(s):
    if s == "": return EMPTY_STEP
    hard = s[-1] in "h'"
    body = s[:-1] if hard else s
    if body == "" or any(c not in "0123456789" for c in body): return BAD_STEP
    v = int(body)
    if v >= H: return OVERFLOW
    return v + H if hard else v


def parse(text):
    """(count, indexes) or (refusal, [])."""
    if text == "m": return 0, []
    if not text.startswith("m/"): return NO_ROOT, []
    out = []
    for s in text[2:].split("/"):
        i = step(s)
        if i < 0: return i, []
        if len(out) == 255: return TOO_DEEP, []
        out.append(i)
    return len(out), out


def fmt(indexes, marker):
    if marker not in "h'": return BAD_STEP, ""
    if len(indexes) > 255: return TOO_DEEP, ""
    if any(not 0 <= i < 2**32 for i in indexes): return OVERFLOW, ""
    t = "m" + "".join("/" + (str(i - H) + marker if i >= H else str(i)) for i in indexes)
    return len(t), t


def standard(purpose, network, account, change, index):
    if purpose not in KINDS: return BAD_PURPOSE, []
    if network not in (1, 2, 3): return BAD_NETWORK, []
    if not (0 <= account < H and change in (0, 1) and 0 <= index < H): return BAD_FIELD, []
    return 5, [purpose + H, (0 if network == 1 else 1) + H, account + H, change, index]


def script(purpose, key):
    if purpose not in KINDS: return BAD_PURPOSE, b""
    if len(key) != 33: return ADDRESS + AD.BAD_KEY, b""
    r, s = AD.build(BUILD[purpose], key if purpose != 86 else key[1:])
    return (ADDRESS + r, b"") if r < 0 else (r, s)


def derive(seed, testnet, text):
    n, path = parse(text)
    if n < 0: return n, b""
    r, node = B32.master(seed, testnet)
    if r < 0: return BIP32 + r, b""
    for i in path:
        r, node = B32.ckd_node(node, i)
        if r < 0: return BIP32 + r, b""
    return 78, node


def account_script(purpose, node, change, index):
    if purpose not in KINDS: return BAD_PURPOSE, b""
    if change not in (0, 1) or not 0 <= index < H: return BAD_FIELD, b""
    if len(node) != 78: return BIP32 + B32.BAD_SIZE, b""
    if node[4] != 3 or node[9] < 0x80: return NOT_ACCOUNT, b""
    for i in (change, index):
        r, node = B32.ckd_node(node, i)
        if r < 0: return BIP32 + r, b""
    r, node = B32.neuter_node(node)
    if r < 0: return BIP32 + r, b""
    return script(purpose, node[45:])


# ---- the cases
ABANDON = bytes.fromhex("5eb00bbddcf069084889a8ab9155568165f5c453ccb85e70811aaed6f6da5fc1"
                        "9a5ac40b389cd370d086206dec8aa6c43daea6690f20ad3d8d48b2d2ce9e38e4")


def published():
    """Checks the reference against the pinned vectors; returns their seeds, paths and account nodes."""
    assert hashlib.sha256(NIP06.read_bytes()).hexdigest() == NIP06_PIN
    bip44 = read("bip44/bip-0044.mediawiki")
    for p in ["m / 44' / 0' / 0' / 0 / 1", "m / 44' / 1' / 0' / 0 / 1", "m / 44' / 0' / 1' / 1 / 0"]:
        assert p in bip44, p
    assert "gap limit" in bip44 and "20" in bip44
    seeds = [(ABANDON, False), (ABANDON, True)]
    nodes = []
    # BIP-86 gives xprvs and addresses; BIP-84 and BIP-49 give keys and addresses.
    t86 = read("bip86/bip-0086.mediawiki")
    for i, (path, addr) in enumerate([("m/86'/0'/0'/0/0", "bc1p5cyxnuxmeuwuvkwfem96lqzszd02n6xdcjrs20cac6yqjjwudpxqkedrcr"),
                                      ("m/86'/0'/0'/0/1", "bc1p4qhjn9zdvkux4e44uhx8tc55attvtyu358kutcqkudyccelu0was9fqzwh"),
                                      ("m/86'/0'/0'/1/0", "bc1p3qkhfews2uk44qtvauqyr2ttdsw7svhkl9nkm9s9c3x4ax5h60wqwruhk7")]):
        assert addr in t86
        r, node = derive(ABANDON, False, path)
        assert B32.to_text(node) in t86
        r, s = script(86, B32.neuter_node(node)[1][45:])
        assert AD.encode(AD.MAINNET, s) == (len(addr), addr.encode())
    t84 = read("bip84/bip-0084.mediawiki")
    for path, addr in [("m/84'/0'/0'/0/0", "bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu"),
                       ("m/84'/0'/0'/0/1", "bc1qnjg0jd8228aq7egyzacy8cys3knf9xvrerkf9g"),
                       ("m/84'/0'/0'/1/0", "bc1q8c6fshw2dlwun7ekn9qwf37cu2rn755upcp6el")]:
        r, node = derive(ABANDON, False, path)
        pub = B32.neuter_node(node)[1][45:]
        assert pub.hex() in t84 and addr in t84
        assert AD.encode(AD.MAINNET, script(84, pub)[1]) == (len(addr), addr.encode())
    t49 = read("bip49/bip-0049.mediawiki")
    r, node = derive(ABANDON, True, "m/49'/1'/0'/0/0")
    assert node[46:].hex() in t49
    assert AD.encode(AD.TESTNET, script(49, B32.neuter_node(node)[1][45:])[1]) == (35, b"2Mww8dCYPUpKHofjgcXcBCEGmniw9CoaiD2")
    for purpose, coin in [(44, 0), (49, 0), (84, 0), (86, 0), (49, 1), (84, 1)]:
        r, node = derive(ABANDON, coin == 1, f"m/{purpose}'/{coin}'/0'")
        nodes += [(purpose, node), (purpose, B32.neuter_node(node)[1])]
    # NIP-06's vector: m/44'/1237'/0'/0/0 from its phrase.
    nip = hashlib.pbkdf2_hmac("sha512", b"leader monkey parrot ring guide accident before fence cannon height naive bean",
                              b"mnemonic", 2048)
    r, node = derive(nip, False, "m/44'/1237'/0'/0/0")
    assert node[46:].hex() == "7f7ff03d123792d6ac594bfa67bf6d0c0ab55b6b1fdb6249303fe861f1ccba9a"
    assert node[46:].hex() in NIP06.read_text()
    seeds.append((nip, False))
    paths = ["m/44'/0'/0'/0/1", "m/44'/1'/0'/0/1", "m/44'/0'/1'/1/0", "m/84'/0'/0'/0/5", "m/84h/0h/0h/0/5",
             "m/86'/0'/0'/0/0", "m/49'/1'/0'/0/0", "m/44'/1237'/0'/0/0", "m", "m/0h/1"]
    return seeds, paths, nodes


ALPHABET = "m/0123456789h'H+-x <>;*"


def edits(rng, text):
    out = []
    for _ in range(3):
        i = rng.randrange(len(text) + 1)
        c = rng.choice(ALPHABET)
        out += [text[:i] + c + text[i:], text[:i] + c + text[i + 1:], text[:i] + text[i + 1:]]
    return out


def random_path(rng, depth):
    parts = []
    for _ in range(depth):
        v = rng.choice([rng.randrange(10), rng.randrange(2**31), H - 1, H, H + 1, 10**rng.randrange(1, 25)])
        s = ("0" * rng.choice([0, 0, 0, 1, 2])) + str(v)
        parts.append(s + rng.choice(["", "", "h", "'"]))
    return "m" + "".join("/" + p for p in parts)


def cases():
    rng = random.Random(33)
    seeds, paths, nodes = published()
    cs = []
    texts = list(paths)
    for depth in range(258):
        texts.append(random_path(rng, depth))
    for _ in range(600):
        texts.append(random_path(rng, rng.randrange(8)))
    texts += ["", "m/", "M", "m//", "/m", "mm", "m/0/", "m//0", "m/ 0", "m/0 ", "m/0x1"]
    texts += [e for t in texts[:400] for e in edits(rng, t or "m")]
    for t in texts:
        for cap in ([255, 5, 0] if len(t) < 40 else [255]):
            n, ix = parse(t)
            r = CAPACITY if n > cap else n
            cs.append(dict(op="parse", text=t, cap=cap, result=r, out=ix if r >= 0 else []))
        for s in t.split("/")[1:3]:
            cs.append(dict(op="step", text=s, result=step(s)))
    for _ in range(400):
        ix = [rng.choice([rng.randrange(2**32), rng.randrange(100), H, H - 1, 2**32 - 1]) for _ in range(rng.randrange(8))]
        if rng.random() < 0.1: ix.append(rng.choice([-1, 2**32, 2**40]))
        for marker in "h'H":
            n, t = fmt(ix, marker)
            for cap in [400, max(0, n - 1)]:
                r = CAPACITY if n > cap else n
                cs.append(dict(op="format", indexes=ix, marker=ord(marker), cap=cap, result=r, out=t if r >= 0 else ""))
    cs.append(dict(op="format", indexes=[0] * 256, marker=ord("h"), cap=4000, result=TOO_DEEP, out=""))
    cs.append(dict(op="format", indexes=[0] * 255, marker=ord("h"), cap=4000, result=1 + 2 * 255, out="m" + "/0" * 255))
    for purpose in range(101):
        cs.append(dict(op="kind", purpose=purpose, result=KINDS.get(purpose, BAD_PURPOSE)))
    for _ in range(400):
        f = [rng.choice([44, 49, 84, 86, 45, 0]), rng.choice([0, 1, 2, 3, 4]), rng.choice([0, 1, H - 1, H, -1]),
             rng.choice([0, 1, 2, -1]), rng.choice([0, 5, H - 1, H, -1])]
        for cap in [5, 4]:
            n, ix = standard(*f)
            r = CAPACITY if n > cap else n
            cs.append(dict(op="standard", fields=f, cap=cap, result=r, out=ix if r >= 0 else []))
    for _ in range(20):
        k = B32.ser_p(B32.mul(rng.randrange(1, B32.N)))
        for purpose in [44, 49, 84, 86, 48]:
            for key in [k, k[:32], b"\x04" + k[1:], k[:1] + bytes(32)]:
                for cap in [42, 21]:
                    n, s = script(purpose, key)
                    # p2tr_script_vartime checks its buffer before the key; the others after.
                    early = purpose == 86 and len(key) == 33 and cap < 34
                    r = ADDRESS + AD.CAPACITY if n > cap or early else n
                    cs.append(dict(op="script", purpose=purpose, key=key.hex(), cap=cap, result=r, out=s.hex() if r >= 0 else ""))
    for seed, testnet in seeds:
        for t in paths:
            n, node = derive(seed, testnet, t)
            cs.append(dict(op="derive", seed=seed.hex(), testnet=testnet, text=t, result=n, out=node.hex()))
    for _ in range(30):
        seed = bytes(rng.randrange(256) for _ in range(rng.choice([15, 16, 32, 64, 65])))
        t = rng.choice(paths + [random_path(rng, rng.randrange(5)), "m/x"])
        testnet = rng.random() < 0.5
        n, node = derive(seed, testnet, t)
        cs.append(dict(op="derive", seed=seed.hex(), testnet=testnet, text=t, result=n, out=node.hex()))
    extra = []
    for purpose, node in nodes:
        extra.append((purpose, node))
        r, child = B32.ckd_node(node, 0)
        extra.append((purpose, child))
        if node[4] == 3 and B32.kind(node) == 1:
            r, parent = derive(ABANDON, False, f"m/{purpose}'/0'")
            extra.append((purpose, parent))
            r, normal = derive(ABANDON, False, f"m/{purpose}'/0'/0")
            extra.append((purpose, normal))
    for purpose, node in extra:
        for change in [0, 1, 2, -1]:
            for index in [0, 1, rng.randrange(H), H, -1]:
                for p in [purpose, 47]:
                    n, s = account_script(p, node, change, index)
                    cs.append(dict(op="account", purpose=p, node=node.hex(), change=change, index=index, result=n, out=s.hex()))
    return cs


# ---- running the Coax code
class View(C.Structure):
    _fields_ = [("data", C.POINTER(C.c_uint8)), ("count", C.c_int64)]


class Indexes(C.Structure):
    _fields_ = [("data", C.POINTER(C.c_int64)), ("count", C.c_int64)]


def run(*args, **kw):
    r = subprocess.run([str(a) for a in args], capture_output=True, text=True, **kw)
    if r.returncode:
        raise RuntimeError(f"{args[0]} failed ({r.returncode}): {r.stderr or r.stdout}")
    return r


def buf(data, n=None):
    n = max(1, len(data) if n is None else n)
    b = (C.c_uint8 * n)(*([0xA5] * n))
    for i, x in enumerate(data): b[i] = x
    return b


def view(b, n):
    return View(C.cast(b, C.POINTER(C.c_uint8)), n)


def native(lib, c):
    """(result, output) for one case, and whether nothing was written past it."""
    op = c["op"]
    if op == "step":
        t = c["text"].encode()
        return lib.path_step_vartime(view(buf(t), len(t))), None, True
    if op == "kind":
        return lib.path_purpose_kind(c["purpose"]), None, True
    if op in ("parse", "standard"):
        cap = c["cap"]
        ix = (C.c_int64 * max(1, cap))(*([-99] * max(1, cap)))
        if op == "parse":
            t = c["text"].encode()
            r = lib.path_parse_vartime(view(buf(t), len(t)), Indexes(ix, cap))
        else:
            r = lib.path_standard_vartime(*c["fields"], Indexes(ix, cap))
        n = max(0, r)
        return r, list(ix[:n]), all(x == -99 for x in ix[n:])
    out = (C.c_uint8 * 4000)(*([0xA5] * 4000))
    if op == "format":
        ix = c["indexes"]
        arr = (C.c_int64 * max(1, len(ix)))(*ix)
        r = lib.path_format_vartime(Indexes(arr, len(ix)), c["marker"], view(out, c["cap"]))
        n = max(0, r)
        return r, bytes(out[:n]).decode(), all(b == 0xA5 for b in out[n:])
    work = (C.c_uint8 * B32.WORK)(*([0x5A] * B32.WORK))
    if op == "script":
        k = bytes.fromhex(c["key"])
        r = lib.path_script_vartime(c["purpose"], view(buf(k), len(k)), view(out, c["cap"]))
    elif op == "derive":
        s, t = bytes.fromhex(c["seed"]), c["text"].encode()
        r = lib.path_derive_ct(view(buf(s), len(s)), c["testnet"], view(buf(t), len(t)), view(work, B32.WORK), view(out, 78))
    else:
        nd = bytes.fromhex(c["node"])
        r = lib.path_account_script_ct(c["purpose"], view(buf(nd), len(nd)), c["change"], c["index"],
                                       view(work, B32.WORK), view(out, 42))
    n = max(0, r)
    clean = all(b == 0xA5 for b in out[n:]) and all(b in (0, 0x5A) for b in work)
    if op != "script": clean = clean and (all(b == 0 for b in work) or all(b == 0x5A for b in work))
    return r, bytes(out[:n]).hex(), clean


class Path_(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix="coax-path-")
        cls.dir = Path(cls.temp.name)
        cls.libs = []
        for label, mode in [("checked", []), ("release", ["--release"])]:
            c = cls.dir / (label + ".c")
            c.write_text(run(COMPILER, "c", SOURCE, "--library", "--header", cls.dir / "path.h", *mode).stdout)
            run("cc", "-std=c11", "-O2", "-shared", "-fPIC", c, "-lm", "-o", cls.dir / (label + ".dylib"))
            lib = C.CDLL(str(cls.dir / (label + ".dylib")))
            lib.path_step_vartime.argtypes = [View]
            lib.path_parse_vartime.argtypes = [View, Indexes]
            lib.path_format_vartime.argtypes = [Indexes, C.c_uint8, View]
            lib.path_purpose_kind.argtypes = [C.c_int64]
            lib.path_standard_vartime.argtypes = [C.c_int64] * 5 + [Indexes]
            lib.path_script_vartime.argtypes = [C.c_int64, View, View]
            lib.path_derive_ct.argtypes = [View, C.c_bool, View, View, View]
            lib.path_account_script_ct.argtypes = [C.c_int64, View, C.c_int64, C.c_int64, View, View]
            for name in ["path_step_vartime", "path_parse_vartime", "path_format_vartime", "path_purpose_kind",
                         "path_standard_vartime", "path_script_vartime", "path_derive_ct", "path_account_script_ct"]:
                getattr(lib, name).restype = C.c_int64
            cls.libs.append((label, lib))
        cls.cases = cases()

    @classmethod
    def tearDownClass(cls):
        cls.temp.cleanup()

    def test_native(self):
        for label, lib in self.libs:
            for c in self.cases:
                r, out, clean = native(lib, c)
                self.assertEqual(r, c["result"], f"{label} {c}")
                if c.get("out") is not None and r >= 0 and c["op"] not in ("step", "kind"):
                    self.assertEqual(out, c["out"], f"{label} {c}")
                self.assertTrue(clean, f"{label} wrote past its output: {c}")
        results = {c["result"] for c in self.cases}
        self.assertTrue(set(range(-10, 0)) <= results, sorted(results))
        print(f"\n{len(self.cases)} cases agree with the Python reference, native checked and release", file=sys.stderr)


if __name__ == "__main__":
    unittest.main()
