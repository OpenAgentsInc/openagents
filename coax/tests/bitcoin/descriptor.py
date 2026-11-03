#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: BIP-380's own Python and a separate Python reference decide each case outside Coax)
"""Output descriptors against BIP-380's code, Bitcoin Core's tests and a Python reference.

Bitcoin From Scratch, chapter 30. Compiles coax/lib/bitcoin/descriptor.coax
to C in checked and release builds and runs its three exported procedures
on:

- BIP-380's checksum and key-expression vectors, and the valid and invalid
  descriptors of BIP-381 to BIP-386 and BIP-389, pinned in
  coax/tests/vectors/upstream/;
- every Check, CheckMultipath and CheckUnparsable call in Bitcoin Core's
  src/test/descriptor_tests.cpp whose descriptors use only this chapter's
  functions (no script trees, miniscript or musig), with their apostrophe
  forms rewritten to h as Core's own test does;
- seeded descriptors built from those parts, and each with seeded edits.

Before any Coax code runs, BIP-380's descsum_create and descsum_check, run
from the pinned text, must agree with the reference's checksum on every
string, and the reference must reproduce each published script and accept
or refuse each published descriptor as its source says. A descriptor with
private keys is refused here (chapter 30 reads public descriptors); the
reference derives its public twin, with Bitcoin Core's normalization of a
hardened prefix into the xpub, or with each child key written in hex, and
the Coax code must give the published scripts from the twin. Every result
and script must match the reference, and nothing may be written past the
output. Run from the repository root:

    python3 coax/tests/bitcoin/descriptor.py
"""
import ctypes as C
import hashlib
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
sys.path.insert(0, str(HERE))
import bip32 as B32  # noqa: E402
import base58 as B58  # noqa: E402
import address as AD  # noqa: E402
import points as EC  # noqa: E402
import functools  # noqa: E402

# Derivation is pure, and the vectors derive the same keys many times.
B32.ckd_node = functools.lru_cache(maxsize=None)(B32.ckd_node)
B32.decode_text = functools.lru_cache(maxsize=None)(B32.decode_text)
AD.build = functools.lru_cache(maxsize=None)(AD.build)

COMPILER = Path(os.environ.get("COAX_TEST_COMPILER", ROOT / "coax/bin/coax"))
SOURCE = ROOT / "coax/lib/bitcoin/descriptor.coax"
CHECKS = ROOT / "bitcoin/examples/checks/descriptors.coax"
UPSTREAM = ROOT / "coax/tests/vectors/upstream"
PINS = {
    "bip380/bip-0380.mediawiki": "34e6510bb2eba9445a68eacf3d24d5a4e5f24481477488d5552f674ac81dd5fe",
    "bip381/bip-0381.mediawiki": "f0f6060332f82454f61015815c824eea4b7b6684ce4892ef42d23e4e782fa817",
    "bip382/bip-0382.mediawiki": "6cd2574998ca0d2141d1c487e70da2dfab47abd6a2dfcc3680af2c241f01fe9d",
    "bip383/bip-0383.mediawiki": "c46f75e5869386c1852dd187725293822b9e3b761342a966fbcf99adaa112aa2",
    "bip384/bip-0384.mediawiki": "9d6b32a619e021f50f4b1a07ed8f9d08cfea599c8f484e6b1360ada9aae8541c",
    "bip385/bip-0385.mediawiki": "8c4963492f1d5b492808e5ece8be75f7f66def37f35d091074fdbfa98a6ddd08",
    "bip386/bip-0386.mediawiki": "770121b4b33a50bfadfebcd94931dd0ad04159b9fbc1d3b3c3ae17fc0cc16de8",
    "bip389/bip-0389.mediawiki": "dfe2d4673bb008565bbb5bcd01438de1c568ba0f46b1dd3bf09ee5df3bd29284",
    "bitcoin-core/descriptor_tests.cpp": "823160973e3b91c52a75b265b22bd815cf826d76e29943b3509c166244b3254c",
}


def read(name):
    raw = (UPSTREAM / name).read_bytes()
    assert hashlib.sha256(raw).hexdigest() == PINS[name], name
    return raw.decode()


# ---- the reference
(BAD_CHAR, CHECKSUM_LENGTH, BAD_CHECKSUM, SYNTAX, UNKNOWN_FUNCTION, CONTEXT, BAD_KEY, UNCOMPRESSED, PRIVATE_KEY,
 BAD_ORIGIN, BAD_STEP, MULTIPATH, NEEDS_PRIVATE, THRESHOLD, KEY_COUNT, SCRIPT_TOO_LONG, BAD_HEX, BAD_ADDRESS,
 WRONG_NETWORK, TAPROOT_TREE, BAD_NETWORK, BAD_INDEX, BAD_BRANCH, NO_OUTPUT, CAPACITY) = range(-1, -26, -1)
BIP32, ADDRESS = -40, -60
TOP, P2SH, P2WSH, P2WPKH, P2TR = range(5)
MAINNET, TESTNET, REGTEST = 1, 2, 3
HARDENED = 2**31
MAX_DEPTH = 255
INPUT = "0123456789()[],'/*abcdefgh@:$%{}IJKLMNOPQRSTUVWXYZ&+-.;<=>?!^_|~ijklmnopqrstuvwxyzABCDEFGH`#\"\\ "
CHECK = "qpzry9x8gf2tvdw0s3jn54khce6mua7l"
GENERATOR = [0xf5dee51989, 0xa9fdca3312, 0x1bab10e32d, 0x3706b1677a, 0x644d626ffd]
HEXDIGITS = b"0123456789abcdefABCDEF"


def checksum(body):
    """The eight checksum characters of a body (bytes), or BAD_CHAR."""
    symbols = []
    groups = []
    for ch in body:
        if ch >= 128 or chr(ch) not in INPUT: return BAD_CHAR
        v = INPUT.index(chr(ch))
        symbols.append(v & 31); groups.append(v >> 5)
        if len(groups) == 3: symbols.append(groups[0] * 9 + groups[1] * 3 + groups[2]); groups = []
    if len(groups) == 1: symbols.append(groups[0])
    if len(groups) == 2: symbols.append(groups[0] * 3 + groups[1])
    c = 1
    for v in symbols + [0] * 8:
        top = c >> 35
        c = (c & 0x7ffffffff) << 5 ^ v
        for i in range(5):
            if (top >> i) & 1: c ^= GENERATOR[i]
    c ^= 1
    return "".join(CHECK[(c >> (5 * (7 - i))) & 31] for i in range(8))


def body(text):
    hash_at = text.find(b"#")
    if hash_at < 0:
        return BAD_CHAR if checksum(text) == BAD_CHAR else len(text)
    if len(text) - hash_at - 1 != 8: return CHECKSUM_LENGTH
    want = checksum(text[:hash_at])
    if want == BAD_CHAR: return BAD_CHAR
    return hash_at if want.encode() == text[hash_at + 1:] else BAD_CHECKSUM


def close(s, open_at):
    depth = 0
    for i in range(open_at, len(s)):
        if s[i] == ord("("): depth += 1
        if s[i] == ord(")"):
            depth -= 1
            if depth == 0: return i
    return -1


def arg_end(s, start):
    depth = 0
    for i in range(start, len(s)):
        if s[i] == ord("("): depth += 1
        if s[i] == ord(")"): depth -= 1
        if s[i] == ord(",") and depth == 0: return i
    return len(s)


def is_hex(s): return len(s) > 0 and len(s) % 2 == 0 and all(c in HEXDIGITS for c in s)


def step(s):
    """A path step as chapter 33 reads it, or BAD_STEP."""
    if not s: return BAD_STEP
    hard = s[-1:] in (b"h", b"'")
    digits = s[:-1] if hard else s
    if not digits or any(c < 48 or c > 57 for c in digits): return BAD_STEP
    v = int(digits)
    if v >= HARDENED: return BAD_STEP
    return v + HARDENED if hard else v


def is_multipath(s): return len(s) >= 2 and s[:1] == b"<" and s[-1:] == b">"


def origin(s):
    slash = s.find(b"/")
    if slash < 0: slash = len(s)
    if slash != 8 or not is_hex(s[:8]): return BAD_ORIGIN
    parts = s[slash + 1:].split(b"/") if slash < len(s) else []
    for n, part in enumerate(parts):
        if is_multipath(part): return MULTIPATH
        if step(part) < 0: return BAD_STEP
        if n + 1 > MAX_DEPTH: return BIP32 + B32.BAD_DEPTH
    return 0


def lift_ok(x_bytes):
    x = int.from_bytes(x_bytes, "big")
    return x < EC.P and EC.lift(x) is not None


def plain_key(s, ctx):
    if is_hex(s):
        if len(s) > 130: return BAD_KEY, b""
        b = bytes.fromhex(s.decode())
        if len(b) == 65 and b[0] in (6, 7): return BAD_KEY, b""
        if len(b) in (33, 65) and EC.decode(b) is not None and (len(b) == 65 or int.from_bytes(b[1:], "big") < EC.P):
            if len(b) == 65 and ctx not in (TOP, P2SH): return UNCOMPRESSED, b""
            return (32, b[1:33]) if ctx == P2TR else (len(b), b)
        if len(b) == 32 and ctx == P2TR and lift_ok(b): return 32, b
        return BAD_KEY, b""
    k, _ = B58.wif_decode(s)
    return (PRIVATE_KEY, b"") if k >= 5 else (BAD_KEY, b"")


class Env:
    def __init__(self, network, branch=0, index=0, expand=False):
        self.network, self.branch, self.index, self.expand = network, branch, index, expand
        self.branches, self.ranged, self.outputs = 0, False, 1


def ext_key(s, ctx, env):
    kend = s.find(b"/")
    if kend < 0: kend = len(s)
    if kend != 111: return BAD_KEY, b""
    r, node = B32.decode_text(s[:kend].decode("latin-1"))
    if r < 0: return BAD_KEY, b""
    if node[45] == 0: return PRIVATE_KEY, b""
    if (int.from_bytes(node[:4], "big") == B32.XPUB) != (env.network == MAINNET): return WRONG_NETWORK, b""
    parts = s[kend + 1:].split(b"/") if kend < len(s) else []
    steps, items, slot, star, hard = [], [], -1, -1, False
    for n, part in enumerate(parts):
        if len(steps) == MAX_DEPTH: return BIP32 + B32.BAD_DEPTH, b""
        if n == len(parts) - 1 and part in (b"*", b"*h", b"*'"):
            hard = hard or len(part) == 2
            star = len(steps); steps.append(0)
        elif is_multipath(part):
            if slot >= 0: return MULTIPATH, b""
            slot = len(steps); steps.append(0)
            for item in part[1:-1].split(b";"):
                if len(items) == MAX_DEPTH: return MULTIPATH, b""
                v = step(item)
                if v < 0: return v, b""
                if v in items: return MULTIPATH, b""
                items.append(v)
                hard = hard or v >= HARDENED
            if len(items) < 2: return MULTIPATH, b""
        else:
            v = step(part)
            if v < 0: return v, b""
            hard = hard or v >= HARDENED
            steps.append(v)
    if node[4] + len(steps) > MAX_DEPTH: return BIP32 + B32.BAD_DEPTH, b""
    if hard: return NEEDS_PRIVATE, b""
    if slot >= 0:
        if env.branches == 0: env.branches = len(items)
        elif env.branches != len(items): return MULTIPATH, b""
        steps[slot] = items[env.branch] if env.expand else items[0]
    if star >= 0:
        env.ranged = True
        steps[star] = env.index if env.expand else 0
    if env.expand:
        for i in steps:
            r, node = B32.ckd_node(node, i)
            if r < 0: return BIP32 + r, b""
    key = node[45:]
    return (32, key[1:]) if ctx == P2TR else (33, key)


def key(s, ctx, env):
    if s.count(b"]") > 1: return BAD_ORIGIN, b""
    rest = s
    if b"]" in s:
        if s[:1] != b"[": return BAD_ORIGIN, b""
        r = origin(s[1:s.index(b"]")])
        if r < 0: return r, b""
        rest = s[s.index(b"]") + 1:]
    kend = rest.find(b"/")
    if kend < 0: kend = len(rest)
    if kend == 0 or rest[:1] == b" " or rest[kend - 1:kend] == b" ": return BAD_KEY, b""
    if kend == len(rest):
        r = plain_key(rest, ctx)
        if r[0] != BAD_KEY: return r
    return ext_key(rest, ctx, env)


def push_number(n): return bytes([0x50 + n]) if n <= 16 else bytes([1, n])


def multi(inner, ctx, sort, env):
    e = arg_end(inner, 0)
    thr = inner[:e]
    if not thr or any(c < 48 or c > 57 for c in thr): return THRESHOLD, b""
    k = int(thr)
    keys, at = [], e
    while at < len(inner):
        end = arg_end(inner, at + 1)
        r, kb = key(inner[at + 1:end], ctx, env)
        if r < 0: return r, b""
        keys.append(kb); at = end
    if not keys or len(keys) > 20: return KEY_COUNT, b""
    if k < 1 or k > len(keys): return THRESHOLD, b""
    if ctx == TOP and len(keys) > 3: return KEY_COUNT, b""
    if sort: keys = sorted(keys)
    s = push_number(k) + b"".join(bytes([len(x)]) + x for x in keys) + push_number(len(keys)) + b"\xae"
    if ctx == P2SH and len(s) > 520: return SCRIPT_TOO_LONG, b""
    return len(s), s


def built(r):
    n, s = r
    return (ADDRESS + n, b"") if n < 0 else (n, s)


def script(s, ctx, env, nth):
    open_at = s.find(b"(")
    if open_at < 0: return UNKNOWN_FUNCTION, b""
    if close(s, open_at) != len(s) - 1: return SYNTAX, b""
    name, inner = s[:open_at], s[open_at + 1:-1]
    if name in (b"pk", b"pkh"):
        n, k = key(inner, ctx, env)
        if n < 0: return n, b""
        if name == b"pk": return n + 2, bytes([n]) + k + b"\xac"
        return built(AD.build("p2pkh", k))
    if name == b"wpkh":
        if ctx not in (TOP, P2SH): return CONTEXT, b""
        n, k = key(inner, P2WPKH, env)
        if n < 0: return n, b""
        return built(AD.build("p2wpkh", k))
    if name == b"combo":
        if ctx != TOP: return CONTEXT, b""
        n, k = key(inner, TOP, env)
        if n < 0: return n, b""
        env.outputs = 4 if n == 33 else 2
        if nth >= env.outputs: return NO_OUTPUT, b""
        if nth == 0: return n + 2, bytes([n]) + k + b"\xac"
        return built(AD.build(["", "p2pkh", "p2wpkh", "p2sh_p2wpkh"][nth], k))
    if name in (b"multi", b"sortedmulti"):
        return multi(inner, ctx, name == b"sortedmulti", env)
    if name in (b"sh", b"wsh"):
        if name == b"sh" and ctx != TOP: return CONTEXT, b""
        if name == b"wsh" and ctx not in (TOP, P2SH): return CONTEXT, b""
        n, sub = script(inner, P2SH if name == b"sh" else P2WSH, env, 0)
        if n < 0: return n, b""
        if name == b"sh":
            if n > 520: return SCRIPT_TOO_LONG, b""
            return built(AD.build("p2sh", sub))
        return built(AD.build("p2wsh", sub))
    if name == b"addr":
        if ctx != TOP: return CONTEXT, b""
        n, _, sc = AD.decode(env.network, inner)
        if n == AD.WRONG_NETWORK: return WRONG_NETWORK, b""
        if n < 0: return BAD_ADDRESS, b""
        return n, sc
    if name == b"raw":
        if ctx != TOP: return CONTEXT, b""
        if not is_hex(inner): return BAD_HEX, b""
        if len(inner) // 2 > 10000: return SCRIPT_TOO_LONG, b""
        return len(inner) // 2, bytes.fromhex(inner.decode())
    if name == b"tr":
        if ctx != TOP: return CONTEXT, b""
        if arg_end(inner, 0) != len(inner): return TAPROOT_TREE, b""
        n, k = key(inner, P2TR, env)
        if n < 0: return n, b""
        if not env.expand: return 34, b""
        return built(AD.build("p2tr", k))
    return UNKNOWN_FUNCTION, b""


def info(text, network):
    """(result, ranged, branches, outputs)."""
    if network not in (1, 2, 3): return BAD_NETWORK, False, 0, 0
    b = body(text)
    if b < 0: return b, False, 0, 0
    env = Env(network)
    r, _ = script(text[:b], TOP, env, 0)
    if r < 0: return r, False, 0, 0
    return 0, env.ranged, env.branches or 1, env.outputs


def expand(text, network, branch, index, nth):
    r, _, branches, outputs = info(text, network)
    if r < 0: return r, b""
    if not 0 <= branch < branches: return BAD_BRANCH, b""
    if not 0 <= index < HARDENED: return BAD_INDEX, b""
    if not 0 <= nth < outputs: return NO_OUTPUT, b""
    env = Env(network, branch, index, True)
    return script(text[:body(text)], TOP, env, nth)


# ---- public twins of descriptors with private keys
B58ALPHABET = B58.ALPHABET
KEYS = re.compile(r"(?<=[(,\]])((?:xprv|tprv|xpub|tpub)[1-9A-HJ-NP-Za-km-z]{107}(?:/[0-9<>;h'*]+)*|"
                  r"[59KLc][1-9A-HJ-NP-Za-km-z]{50,51})(?=[),])")


def child_public(text, branch, index):
    """The hex public key a key expression (no origin) gives, deriving privately where it must."""
    if len(text) in (51, 52) and "/" not in text:
        k, secret = B58.wif_decode(text.encode())
        point = EC.mul(int.from_bytes(secret, "big"))
        return EC.encode(point, 33 if k in (6, 8) else 65).hex()
    parts = text.split("/")
    r, node = B32.decode_text(parts[0])
    assert r == 78, text
    for p in parts[1:]:
        if p.startswith("<"): p = p[1:-1].split(";")[branch]
        if p in ("*", "*h", "*'"): i = index + (HARDENED if len(p) == 2 else 0)
        else: i = step(p.encode())
        r, node = B32.ckd_node(node, i)
        assert r == 78, text
    return B32.public_of(node).hex()


def hex_twin(text, branch, index):
    """text with every key written as its child's hex public key."""
    return KEYS.sub(lambda m: child_public(m.group(1), branch, index), text.split("#")[0])


def xpub_twin(text):
    """text with private keys made public and a hardened prefix moved into the xpub, as
    Bitcoin Core normalizes; None when a hardened step is left after an xpub."""
    stuck = []

    def one(m):
        t = m.group(1)
        if t[:4] in ("xpub", "tpub"): return t
        if len(t) in (51, 52) and "/" not in t: return child_public(t, 0, 0)
        parts = t.split("/")
        r, node = B32.decode_text(parts[0])
        fixed = [step(p.encode()) if not (p.startswith("<") or p.startswith("*")) else -1 for p in parts[1:]]
        last = max([k for k, v in enumerate(fixed) if v >= HARDENED], default=-1)
        if any(v < 0 for v in fixed[:last + 1]): stuck.append(t); return t
        for v in fixed[:last + 1]: r, node = B32.ckd_node(node, v)
        r, node = B32.neuter_node(node)
        return "/".join([B32.to_text(node)] + parts[last + 2:])
    out = KEYS.sub(one, text.split("#")[0])
    return None if stuck or info(out.encode(), MAINNET)[0] == NEEDS_PRIVATE else out


# ---- published vectors
def tt(line): return re.findall(r"<tt>(.*?)</tt>", line)


def bip_vectors():
    """(descriptor, [[script per output] per index] or None for an invalid one, source)."""
    out = []
    for n in (381, 382, 383, 384, 385, 386):
        text = read(f"bip{n}/bip-0{n}.mediawiki")
        tests = text[text.index("==Test Vectors=="):text.index("==Backwards")]
        valid, current = True, None
        for line in tests.split("\n"):
            if line.lower().startswith("invalid"): valid = False
            if line.startswith("* ") and tt(line):
                d = tt(line)[-1]
                current = [d, [] if valid else None, f"BIP-{n}"]
                out.append(current)
            elif line.startswith("** ") and valid and current and tt(line):
                current[1].append([tt(line)[0]])
            elif line.startswith("** Child") and valid: current[1].append([])
            elif line.startswith("*** ") and valid: current[1][-1].append(tt(line)[0])
    for d, scripts, src in out:
        if src == "BIP-384" and scripts and len(scripts) > 1 and all(len(s) == 1 for s in scripts) and "*" not in d:
            scripts[:] = [[s[0] for s in scripts]]          # combo(): one index, several outputs
    return out


def bip389_vectors():
    text = read("bip389/bip-0389.mediawiki")
    tests = text[text.index("==Test Vectors=="):text.index("==Backwards")]
    out, valid = [], True
    for line in tests.split("\n"):
        if line.lower().startswith("invalid"): valid = False
        if line.startswith("* ") and tt(line): out.append([tt(line)[-1], [] if valid else None])
        elif line.startswith("** ") and tt(line) and valid: out[-1][1].append(tt(line)[0])
    return out


def bip380_vectors():
    text = read("bip380/bip-0380.mediawiki")
    tests = text[text.index("==Test Vectors=="):text.index("==Backwards")]
    sums, keys_valid, keys_invalid = [], [], []
    part = 0
    for line in tests.split("\n"):
        if "cover key expressions" in line: part = 1
        if line.startswith("Invalid expression"): part = 2
        if line.startswith("* ") and tt(line):
            v = tt(line)[-1].replace("&lt;", "<").replace("&gt;", ">")
            [sums, keys_valid, keys_invalid][part].append((line[2:line.rindex("<tt>")].strip(" :"), v))
    return sums, keys_valid, keys_invalid


def descsum():
    """BIP-380's own descsum_create and descsum_check, run from the pinned text."""
    text = read("bip380/bip-0380.mediawiki")
    code = "\n".join(p for p in re.findall(r"<pre>\n(.*?)</pre>", text, re.S) if "def descsum" in p)
    space = {}
    exec(compile(code, "bip-0380.mediawiki", "exec"), space)
    return space["descsum_create"], space["descsum_check"]


# ---- Bitcoin Core's descriptor_tests.cpp
FLAGS = dict(DEFAULT=0, RANGE=1, HARDENED=2, UNSOLVABLE=4, SIGNABLE=8, DERIVE_HARDENED=16, MIXED_PUBKEYS=32,
             XONLY_KEYS=64, MISSING_PRIVKEYS=128, SIGNABLE_FAILS=256, MUSIG=512, MUSIG_DERIVATION=1024,
             MIXED_MUSIG=2048, UNIQUE_XPUBS=4096)


def cpp_tokens(src):
    pos, toks = 0, []
    pat = re.compile(r"""\s+|//[^\n]*|"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)'|[A-Za-z_][A-Za-z_0-9]*(?:::[A-Za-z_][A-Za-z_0-9]*)*|0x[0-9a-fA-F]+|\d+|.""", re.S)
    while pos < len(src):
        m = pat.match(src, pos)
        t = m.group(0); pos = m.end()
        if t.isspace() or t.startswith("//"): continue
        toks.append(t)
    return toks


def cpp_value(toks, i):
    """One argument: a string, an int, a {list}, a name or call, or an expression of them."""
    parts = []
    while toks[i] not in (",", ")", "}"):
        t = toks[i]
        if t.startswith('"'):
            s = ""
            while toks[i].startswith('"'):
                s += bytes(toks[i][1:-1], "latin-1").decode("unicode_escape"); i += 1
            parts.append(s); continue
        if t == "{":
            items, i = [], i + 1
            while toks[i] != "}":
                v, i = cpp_value(toks, i)
                items.append(v)
                if toks[i] != "}": i += 1          # a ',' or a stray ')' 
            parts.append(items); i += 1; continue
        if t == "(" or (re.match(r"[A-Za-z_]", t) and toks[i + 1] in ("(", "{")):
            name = "" if t == "(" else t
            if t != "(": i += 1
            close_tok = ")" if toks[i] == "(" else "}"
            args, i = [], i + 1
            while toks[i] != close_tok:
                v, i = cpp_value(toks, i)
                args.append(v)
                if toks[i] != close_tok: i += 1    # a ',' or a stray closer
            parts.append(("call", name, args)); i += 1; continue
        parts.append(t); i += 1
    return (parts[0] if len(parts) == 1 else ("expr", parts)), i


def core_calls():
    src = read("bitcoin-core/descriptor_tests.cpp")
    start = src.index("BOOST_AUTO_TEST_CASE(descriptor_test)")
    end = src.index("BOOST_AUTO_TEST_CASE", start + 10)
    toks = cpp_tokens(src[start:end])
    calls = []
    for i, t in enumerate(toks):
        if t in ("Check", "CheckUnparsable", "CheckMultipath") and toks[i + 1] == "(" and toks[i - 1] in (";", "{", "}"):
            j, args = i + 2, []
            while toks[j] != ")":
                v, j = cpp_value(toks, j)
                args.append(v)
                if toks[j] == ",": j += 1
            calls.append((t, args))
    return calls


def flag_value(v):
    if isinstance(v, str): return FLAGS[v]
    return sum(FLAGS[x] for x in v[1] if x != "|")


FUNCTIONS = {"pk", "pkh", "wpkh", "combo", "sh", "wsh", "multi", "sortedmulti", "addr", "raw", "tr"}


def in_scope(d):
    if not isinstance(d, str): return False          # built by an expression, such as a nesting test
    names = set(re.findall(r"([A-Za-z_]+)\(", d))
    if not names <= FUNCTIONS or "{" in d: return False
    m = re.match(r"tr\((.*)\)$", d)
    return not (m and arg_end(m.group(1).encode(), 0) != len(m.group(1)))


def core_vectors():
    """(text, expectation) pairs: ("scripts", branch -> index -> outputs) or ("refuse", None)."""
    out, skipped = [], 0
    for kind, args in core_calls():
        if kind == "CheckUnparsable":
            prv, pub = args[0], args[1]
            texts = [t for t in (prv, pub) if t]
            if not all(in_scope(t) for t in texts): skipped += 1; continue
            for t in texts: out.append((t, ("refuse", None), "Core"))
            continue
        if kind == "Check":
            prv, pub, norm, flags, scripts = args[0], args[1], args[2], flag_value(args[3]), args[4]
            branches = [(prv, pub, norm, flags, scripts)]
        else:
            prv, pub = args[0], args[1]
            fl = [flag_value(f) for f in args[5]]
            branches = [(prv, pub, args[4][b], fl[min(b, len(fl) - 1)], args[6][b]) for b in range(len(args[2]))]
        if not all(in_scope(t) for t in (prv, pub)): skipped += 1; continue
        forms = [(prv, pub)]
        if "'" in prv and "'" in pub: forms.append((prv.split("#")[0].replace("'", "h"), pub.split("#")[0].replace("'", "h")))
        for p, q in forms:
            out.append((p, ("scripts", [b[4] for b in branches], [b[3] for b in branches], q), "Core"))
        for b in branches: out.append((b[2], ("normalized", b[3]), "Core"))
    return out, skipped


# ---- what the published vectors say, checked against the reference
def scripts_of(text, branch, index):
    """The reference's outputs for a descriptor that may hold private keys: from its hex twin."""
    t = hex_twin(text, branch, index).encode()
    r, _, _, outputs = info(t, MAINNET)
    assert r == 0, (text, t, r)
    return [expand(t, MAINNET, 0, 0, k)[1].hex() for k in range(outputs)]


def has_private(text):
    return any(m.group(1)[:4] in ("xprv", "tprv") or len(m.group(1)) in (51, 52) for m in KEYS.finditer(text))


def published():
    """Cases with their published answers, after the reference reproduces each one."""
    create, check = descsum()
    cs = []
    sums, keys_valid, keys_invalid = bip380_vectors()
    for label, d in sums:
        good = label.startswith("Valid") or label.startswith("No checksum")
        r = info(d.encode(), MAINNET)[0]
        assert (r == 0) == good, (label, d, r)
        if "#" in d and len(d.split("#")[1]) == 8 and d.isascii(): assert check(d) == (r == 0), d
        cs.append(dict(text=d, network=MAINNET, source="BIP-380"))
    for label, k in keys_valid:
        d = f"pk({k})"
        r = info(d.encode(), MAINNET)[0]
        cs.append(dict(text=d, network=MAINNET, source="BIP-380"))
        if has_private(d):
            assert r == PRIVATE_KEY, (d, r)
            twin = xpub_twin(d)
            assert twin is None or scripts_of(twin, 0, 0) == scripts_of(d, 0, 0), d
            d = twin or hex_twin(d, 0, 0)
            assert info(d.encode(), MAINNET)[0] == 0, (label, d)
            cs.append(dict(text=d, network=MAINNET, source="BIP-380"))
        else:
            assert r in (0, NEEDS_PRIVATE), (label, d, r)
    for label, k in keys_invalid:
        d = f"pk({k})"
        r = info(d.encode(), MAINNET)[0]
        assert r < 0, (label, d)
        cs.append(dict(text=d, network=MAINNET, source="BIP-380"))
    n_scripts = 0
    for d, scripts, src in bip_vectors():
        r = info(d.encode(), MAINNET)[0]
        if scripts is None:
            assert r < 0, (src, d)
            cs.append(dict(text=d, network=MAINNET, source=src)); continue
        if not in_scope(d):
            assert r == TAPROOT_TREE, (d, r); cs.append(dict(text=d, network=MAINNET, source=src)); continue
        if has_private(d): assert r == PRIVATE_KEY, (d, r)
        else: assert r == 0, (d, r)
        cs.append(dict(text=d, network=MAINNET, source=src))
        for i, outs in enumerate(scripts):
            assert scripts_of(d, 0, i) == outs, (src, d, i, outs, scripts_of(d, 0, i))
            n_scripts += len(outs)
            twin = xpub_twin(d) if has_private(d) else d
            for t in ([twin] if twin else []) + [hex_twin(d, 0, i)]:
                for k, s in enumerate(outs): cs.append(dict(text=t, network=MAINNET, index=i, nth=k, want=s, source=src))
    for d, expansions in bip389_vectors():
        r = info(d.encode(), MAINNET)[0]
        if expansions is None:
            assert r < 0, d; cs.append(dict(text=d, network=MAINNET, source="BIP-389")); continue
        assert r == (PRIVATE_KEY if has_private(d) else 0), (d, r)
        twin = xpub_twin(d) if has_private(d) else d
        assert twin is None or info(twin.encode(), MAINNET)[2] == len(expansions), d
        for b, e in enumerate(expansions):
            for i in (0, 1):
                want = scripts_of(e, 0, i)[0]
                assert scripts_of(d, b, i)[0] == want, (d, b, i)
                n_scripts += 1
                if twin: cs.append(dict(text=twin, network=MAINNET, branch=b, index=i, want=want, source="BIP-389"))
                cs.append(dict(text=hex_twin(d, b, i), network=MAINNET, index=i, want=want, source="BIP-389"))
    core, skipped = core_vectors()
    n_core = 0
    for text, expect, src in core:
        r = info(text.encode(), MAINNET)[0]
        cs.append(dict(text=text, network=MAINNET, source=src))
        if expect[0] == "refuse":
            assert r < 0, (text, r); continue
        if expect[0] == "normalized":
            assert r == (NEEDS_PRIVATE if expect[1] & FLAGS["DERIVE_HARDENED"] else 0), (text, r, expect)
            continue
        _, per_branch, flags, pub = expect
        assert r == (PRIVATE_KEY if has_private(text) else
                     (NEEDS_PRIVATE if any(f & FLAGS["HARDENED"] for f in flags) else 0)), (text, r)
        rp = info(pub.encode(), MAINNET)[0]
        assert rp == (NEEDS_PRIVATE if any(f & FLAGS["HARDENED"] for f in flags) else 0), (pub, rp)
        cs.append(dict(text=pub, network=MAINNET, source=src))
        twin = xpub_twin(text) if has_private(text) else (text if r == 0 else None)
        for b, per_index in enumerate(per_branch):
            for i, outs in enumerate(per_index):
                got = scripts_of(text, b, i)
                assert got == outs, (text, b, i, outs, got)
                n_core += 1
                for t in ([twin] if twin else []) + [hex_twin(text, b, i)]:
                    for k, s in enumerate(outs):
                        cs.append(dict(text=t, network=MAINNET, branch=b if t == twin else 0, index=i, nth=k, want=s, source=src))
    return cs, dict(scripts=n_scripts, core_calls=len(core), core_skipped=skipped, core_script_sets=n_core)


def checks_file_copies():
    """Each long hex or descriptor string the checks file copies appears in a pinned source, and
    each descriptor with a checksum in the example's output passes BIP-380's descsum_check."""
    text = CHECKS.read_text()
    sources = "".join(read(n) for n in PINS)
    for s in re.findall(r'"([0-9a-f]{40,})"', text) + re.findall(r'"((?:[a-z]+\()+[^"]*\))"', text):
        assert s in sources, s
    _, check = descsum()
    for line in (ROOT / "bitcoin/fixtures/descriptors.out").read_text().split("\n"):
        d = line.split(" ")[0]
        if "#" in d and len(d.split("#")[1]) == 8: assert check(d) == (line.count("checksum") == 0), line


def table():
    """The published vectors the Coax checks file runs, as Coax source."""
    def q(s): return '"' + s + '"'
    sums, keys_valid, keys_invalid = bip380_vectors()
    rows_sum = [f"  DsvResult.{{source={q('BIP-380')}, text={q(d)}, result={info(d.encode(), MAINNET)[0]}}},"
                for _, d in sums if d.isascii()]
    rows_key = [f"  DsvResult.{{source={q('BIP-380')}, text={q('pk(' + k + ')')}, result={info(('pk(' + k + ')').encode(), MAINNET)[0]}}},"
                for _, k in keys_valid + keys_invalid]
    rows_script, rows_refused = [], []
    for d, scripts, src in bip_vectors():
        r = info(d.encode(), MAINNET)[0]
        if r < 0: rows_refused.append(f"  DsvResult.{{source={q(src)}, text={q(d)}, result={r}}},")
        if scripts is None or not in_scope(d): continue
        twin = xpub_twin(d) if has_private(d) else d
        for i, outs in enumerate(scripts):
            t = twin or hex_twin(d, 0, i)
            for k, s in enumerate(outs):
                rows_script.append(f"  DsvScript.{{source={q(src)}, text={q(t)}, branch=0, index={i}, nth={k}, script={q(s)}}},")
    for d, expansions in bip389_vectors():
        r = info(d.encode(), MAINNET)[0]
        if r < 0: rows_refused.append(f"  DsvResult.{{source={q('BIP-389')}, text={q(d)}, result={r}}},")
        if expansions is None: continue
        twin = xpub_twin(d) if has_private(d) else d
        for b, e in enumerate(expansions):
            want = scripts_of(e, 0, 0)[0]
            t = twin or hex_twin(d, b, 0)
            rows_script.append(f"  DsvScript.{{source={q('BIP-389')}, text={q(t)}, branch={b if twin else 0}, index=0, nth=0, script={q(want)}}},")
    out = ["// Generated by coax/tests/bitcoin/descriptor.py --write from the BIPs pinned in",
           "// coax/tests/vectors/upstream/ (see SOURCES.md there). Test data only.",
           "// Descriptors with private keys appear as published, refused; their scripts",
           "// are checked from the public twin the Python reference derives: the xpub at",
           "// the last hardened step (Bitcoin Core's normalization), or each child key in",
           "// hex where a hardened step follows the varying part.",
           "DsvResult :: struct {source: string; text: string; result: int;}",
           "DsvScript :: struct {source: string; text: string; branch: int; index: int; nth: int; script: string;}"]
    for name, rows, kind in [("DSV_SUMS", rows_sum, "DsvResult"), ("DSV_KEYS", rows_key, "DsvResult"),
                             ("DSV_SCRIPTS", rows_script, "DsvScript"), ("DSV_REFUSED", rows_refused, "DsvResult")]:
        out.append(f"{name}_COUNT :: {len(rows)};")
        out.append(f"{name} :: [{len(rows)}] {kind}.[")
        out += rows
        out.append("];")
    return "\n".join(out) + "\n"


TABLE = ROOT / "coax/tests/bitcoin/descriptor_vectors.coax"


# ---- seeded cases
def edits(rng, text):
    out = []
    for _ in range(3):
        if not text: break
        k = rng.randrange(len(text))
        c = rng.choice(INPUT + "Ü\t")
        op = rng.randrange(3)
        out.append(text[:k] + c + text[k + 1:] if op == 0 else text[:k] + c + text[k:] if op == 1 else text[:k] + text[k + 1:])
    return out


def seeded(rng, parts):
    keys, hexkeys = parts
    def k(ctx_tr=False):
        r = rng.random()
        if r < 0.4: base = rng.choice(hexkeys)
        else:
            base = rng.choice(keys) + "".join("/" + str(rng.choice([0, 1, 2, 7, 2147483647])) for _ in range(rng.randrange(3)))
            if rng.random() < 0.2: base += "/<" + ";".join(str(x) for x in rng.sample(range(5), rng.choice([2, 2, 3]))) + ">"
            if rng.random() < 0.4: base += "/*"
        if rng.random() < 0.2: base = "[" + bytes(rng.randrange(256) for _ in range(4)).hex() + "/44h/0'/1]" + base
        return base
    f = rng.choice(["pk", "pkh", "wpkh", "combo", "sh", "wsh", "multi", "sortedmulti", "tr", "raw", "addr", "shwsh"])
    if f in ("pk", "pkh", "wpkh", "combo", "tr"): return f"{f}({k()})"
    if f in ("multi", "sortedmulti"):
        n = rng.randrange(1, 5)
        return f"{f}({rng.randrange(0, n + 2)},{','.join(k() for _ in range(n))})"
    if f == "raw": return "raw(" + bytes(rng.randrange(256) for _ in range(rng.randrange(0, 40))).hex() + ")"
    if f == "addr": return "addr(" + rng.choice(["3PUNyaW7M55oKWJ3kDukwk9bsKvryra15j", "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4",
                                                 "tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx", "1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN2"]) + ")"
    inner = rng.choice(["pk", "pkh", "wpkh", "multi", "sortedmulti", "wsh"])
    sub = f"{inner}({k()})" if inner in ("pk", "pkh", "wpkh") else (
        f"{inner}(1,{k()},{k()})" if inner != "wsh" else f"wsh(pkh({k()}))")
    if f == "shwsh": return f"sh(wsh({sub}))"
    return f"{f}({sub})"


def cases():
    rng = random.Random(30)
    cs, counts = published()
    texts = sorted({c["text"] for c in cs if isinstance(c["text"], str) and c["text"].isascii()})
    keys = sorted({m.group(1).split("/")[0] for t in texts for m in KEYS.finditer(t) if m.group(1)[:4] == "xpub"})
    hexkeys = sorted(set(re.findall(r"(?<=[(,\]])(0[23][0-9a-f]{64}|04[0-9a-f]{128})(?=[),])", " ".join(texts))))
    for _ in range(1500):
        d = seeded(rng, (keys, hexkeys))
        cs.append(dict(text=d, network=rng.choice([1, 1, 2, 3]), branch=rng.choice([0, 0, 1, 2, -1]),
                       index=rng.choice([0, 1, 5, 2**31 - 1, 2**31, -1]), nth=rng.choice([0, 0, 1, 3, 4])))
        if rng.random() < 0.3: cs.append(dict(text=d + "#" + (checksum(d.encode()) if rng.random() < 0.7 else "qqqqqqqq"), network=1))
        if rng.random() < 0.02: cs.append(dict(text=d, network=rng.choice([0, 4, -1])))
    for t in texts[:]:
        for e in edits(rng, t): cs.append(dict(text=e, network=1))
    for c in cs:
        t = c["text"].encode("utf-8") if isinstance(c["text"], str) else c["text"]
        c["bytes"] = t
        c["info"] = info(t, c["network"])
        c["result"] = expand(t, c["network"], c.get("branch", 0), c.get("index", 0), c.get("nth", 0))
        if "want" in c: assert c["result"][1].hex() == c["want"], c
    # the checksum against BIP-380's own code
    create, check = descsum()
    for c in cs:
        t = c["bytes"].split(b"#")[0]
        mine = checksum(t)
        try: theirs = create(t.decode("latin-1"))
        except (AssertionError, TypeError): theirs = None
        if mine == BAD_CHAR: assert theirs is None, t
        else: assert theirs == t.decode() + "#" + mine and check(theirs), t
    return cs, counts


# ---- running the Coax code
class View(C.Structure):
    _fields_ = [("data", C.POINTER(C.c_uint8)), ("count", C.c_int64)]


class Info(C.Structure):
    _fields_ = [("result", C.c_int64), ("ranged", C.c_bool), ("branches", C.c_int64), ("outputs", C.c_int64)]


def run(*args, **kw):
    r = subprocess.run([str(a) for a in args], capture_output=True, text=True, **kw)
    if r.returncode:
        raise RuntimeError(f"{args[0]} failed ({r.returncode}): {r.stderr or r.stdout}")
    return r


def text_view(t):
    b = (C.c_uint8 * max(1, len(t)))(*t) if t else (C.c_uint8 * 1)()
    return b, View(C.cast(b, C.POINTER(C.c_uint8)), len(t))


def native(lib, c):
    keep, v = text_view(c["bytes"])
    i = lib.descriptor_info_vartime(v, c["network"])
    out = (C.c_uint8 * 10100)(*([0xA5] * 10100))
    r = lib.descriptor_script_vartime(v, c["network"], c.get("branch", 0), c.get("index", 0), c.get("nth", 0),
                                      View(C.cast(out, C.POINTER(C.c_uint8)), 10100))
    n = max(0, r)
    s = c["bytes"].split(b"#")[0]
    sk, sv = text_view(s)
    cs = (C.c_uint8 * 9)(*([0xA5] * 9))
    k = lib.descriptor_checksum_vartime(sv, View(C.cast(cs, C.POINTER(C.c_uint8)), 8))
    return ((i.result, i.ranged, i.branches, i.outputs) if i.result == 0 else (i.result, False, 0, 0),
            (r, bytes(out[:n])), k, bytes(cs[:8]) if k == 8 else None,
            all(b == 0xA5 for b in out[n:]) and cs[8] == 0xA5)


class Descriptor(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix="coax-descriptor-")
        cls.dir = Path(cls.temp.name)
        cls.libs = []
        for label, mode in [("checked", []), ("release", ["--release"])]:
            c = cls.dir / (label + ".c")
            c.write_text(run(COMPILER, "c", SOURCE, "--library", "--header", cls.dir / "descriptor.h", *mode).stdout)
            run("cc", "-std=c11", "-O2", "-shared", "-fPIC", c, "-lm", "-o", cls.dir / (label + ".dylib"))
            lib = C.CDLL(str(cls.dir / (label + ".dylib")))
            lib.descriptor_checksum_vartime.argtypes = [View, View]
            lib.descriptor_checksum_vartime.restype = C.c_int64
            lib.descriptor_info_vartime.argtypes = [View, C.c_int64]
            lib.descriptor_info_vartime.restype = Info
            lib.descriptor_script_vartime.argtypes = [View, C.c_int64, C.c_int64, C.c_int64, C.c_int64, View]
            lib.descriptor_script_vartime.restype = C.c_int64
            cls.libs.append((label, lib))
        checks_file_copies()
        assert TABLE.read_text() == table(), f"{TABLE.relative_to(ROOT)} is stale: run with --write"
        cls.cases, cls.counts = cases()

    @classmethod
    def tearDownClass(cls):
        cls.temp.cleanup()

    def test_native(self):
        for label, lib in self.libs:
            for c in self.cases:
                i, r, k, cs, clean = native(lib, c)
                self.assertEqual(i, c["info"], f"{label} info {c}")
                self.assertEqual(r, c["result"], f"{label} script {c}")
                want = checksum(c["bytes"].split(b"#")[0])
                self.assertEqual(k, BAD_CHAR if want == BAD_CHAR else 8, f"{label} {c}")
                if k == 8: self.assertEqual(cs.decode(), want, f"{label} {c}")
                self.assertTrue(clean, f"{label} wrote past its output: {c}")
        results = {c["result"][0] for c in self.cases if c["result"][0] < 0}
        self.assertTrue(set(range(-24, 0)) <= results, sorted(set(range(-24, 0)) - results))
        print(f"\n{len(self.cases)} cases agree with the Python reference, native checked and release; "
              f"{self.counts}", file=sys.stderr)


if __name__ == "__main__":
    if "--write" in sys.argv[1:]:
        TABLE.write_text(table())
        sys.exit(0)
    unittest.main()
