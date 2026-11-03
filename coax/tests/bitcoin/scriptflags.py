#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: read pinned Core's flags, deployments and exceptions, then compare Python's decisions with native Coax)
"""Bitcoin chapter 55. Never executes downloaded code.

  python3 -I coax/tests/bitcoin/scriptflags.py --coax coax/bin/coax

Each subprocess has a five-minute deadline. Checks both native profiles,
all pinned mainnet headers 0..2016, deployment edges on all five networks,
exception hashes, BIP30 ancestry, malformed contexts and exact transcripts.
"""
import argparse
import ctypes as C
import hashlib
import json
from pathlib import Path
import random
import re
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[3]
UP = ROOT / "coax/tests/vectors/upstream"
PINS = json.loads((UP.parent / "manifest.json").read_text())["files"]


def pinned(name):
    raw = (UP / name).read_bytes()
    pin = next(x for x in PINS if x["path"] == "upstream/" + name)
    assert hashlib.sha256(raw).hexdigest() == pin["sha256"], name
    return raw.decode()


def body(text, signature):
    start = text.index("{", text.index(signature))
    depth = 1
    end = start + 1
    while depth:
        depth += {"{": 1, "}": -1}.get(text[end], 0)
        end += 1
    return text[start:end]


def reference():
    val = pinned("bitcoin-core/validation.cpp")
    source = body(val, "script_verify_flags GetBlockScriptFlags(")
    assert "flags &= it->second;" in source
    base = re.search(r"flags\{([^}]+)\}", source)[1]
    ih = pinned("bitcoin-core/interpreter.h")
    names = re.findall(r"^\s+(SCRIPT_VERIFY_\w+),", body(ih, "enum class script_verify_flag_name"), re.M)
    assert len(names) == 21
    bits = {name: 1 << bit for bit, name in enumerate(names)}
    bits["SCRIPT_VERIFY_NONE"] = 0

    def mask(expr):
        return sum(bits[x.strip()] for x in expr.split("|"))

    base = mask(base)
    deployments = re.findall(r"if \(DeploymentActiveAt\(block_index, chainman, Consensus::DEPLOYMENT_(\w+)\)\) \{\s*flags \|= (SCRIPT_VERIFY_\w+);", source)
    assert len(deployments) == 4
    field = dict(DERSIG="BIP66Height", CLTV="BIP65Height", CSV="CSVHeight", SEGWIT="SegwitHeight")
    params = pinned("bitcoin-core/chainparams.cpp")
    networks = {}
    for i, cls in enumerate(("CMainParams", "CTestNetParams", "CTestNet4Params", "SigNetParams", "CRegTestParams"), 1):
        s = body(params, "class " + cls)
        heights = {name: int(re.search(r"consensus\." + name + r" = (\d+);", s)[1]) for name in field.values()}
        anchor = re.search(r'consensus.BIP34Hash = uint256\{"([0-9a-f]+)"\}', s)
        networks[i] = dict(height=int(re.search(r"consensus.BIP34Height = (\d+);", s)[1]),
                           anchor=bytes.fromhex(anchor[1])[::-1] if anchor else None,
                           deps=[(heights[field[d]], bits[f]) for d, f in deployments],
                           exceptions={bytes.fromhex(h)[::-1]: mask(f) for h, f in re.findall(
                               r'script_flag_exceptions.emplace\([^\n]*\n\s*uint256\{"([0-9a-f]+)"\}, ([^;]+)\);', s)})
    repeats = {int(h): bytes.fromhex(x)[::-1] for h, x in re.findall(
        r'block_index.nHeight==(\d+) && block_index.GetBlockHash\(\) == uint256\{"([0-9a-f]+)"\}', body(val, "bool IsBIP30Repeat("))}
    assert len(repeats) == 2
    limit = int(re.search(r"BIP34_IMPLIES_BIP30_LIMIT = (\d+);", val)[1])
    assert "fEnforceBIP30 = !IsBIP30Repeat(*pindex);" in val
    assert "pindex->pprev->GetAncestor(params.GetConsensus().BIP34Height)" in val
    assert "fEnforceBIP30 = fEnforceBIP30 && (!pindexBIP34height || !(pindexBIP34height->GetBlockHash() == params.GetConsensus().BIP34Hash));" in val
    assert "if (fEnforceBIP30 || pindex->nHeight >= BIP34_IMPLIES_BIP30_LIMIT)" in val

    def flags(chain, height, digest):
        if chain not in networks or not 0 <= height <= 2147483647 or len(digest) != 32:
            return -1
        p = networks[chain]
        answer = base & p["exceptions"].get(digest, base)
        for at, bit in p["deps"]:
            if height >= at:
                answer |= bit
        return answer

    def bip30(chain, height, digest, ancestor):
        if flags(chain, height, digest) < 0 or len(ancestor) not in (0, 32):
            return -1
        p = networks[chain]
        enforce = repeats.get(height) != digest
        enforce = enforce and not (height > p["height"] and p["anchor"] is not None and ancestor == p["anchor"])
        return int(enforce or height >= limit)

    return networks, repeats, flags, bip30


class View(C.Structure):
    _fields_ = [("data", C.POINTER(C.c_uint8)), ("count", C.c_int64)]


def view(data):
    storage = (C.c_uint8 * len(data)).from_buffer_copy(data)
    return View(storage, len(data))


def run(command):
    return subprocess.run([str(x) for x in command], cwd=ROOT, check=True,
                          text=True, capture_output=True, timeout=300).stdout


def native(lib, networks, repeats, flags, bip30):
    f, b = lib.sf_flags_probe, lib.sf_bip30_probe
    f.argtypes = [C.c_int64, C.c_int64, View]
    b.argtypes = [C.c_int64, C.c_int64, View, View]
    f.restype = b.restype = C.c_int64
    count = 0

    def check(chain, height, digest, ancestor=b""):
        nonlocal count
        assert f(chain, height, view(digest)) == flags(chain, height, digest), (chain, height, digest.hex())
        assert b(chain, height, view(digest), view(ancestor)) == bip30(chain, height, digest, ancestor), (chain, height, ancestor.hex())
        count += 1

    previous = bytes(32)
    rows = pinned("mainnet/headers-0-2016.txt").splitlines()
    assert len(rows) == 2017
    for height, row in enumerate(rows):
        h, shown, raw = row.split()
        header = bytes.fromhex(raw)
        digest = hashlib.sha256(hashlib.sha256(header).digest()).digest()
        assert int(h) == height and digest[::-1].hex() == shown and header[4:36] == previous
        check(1, height, digest)
        previous = digest

    rng = random.Random(55)
    for chain, p in networks.items():
        heights = {0, 1, p["height"], p["height"] + 1, 1983701, 1983702, 1983703, 2147483647}
        for at, _ in p["deps"]:
            heights.update(range(max(0, at - 2), at + 3))
        heights.update(rng.randrange(2500000) for _ in range(200))
        for height in sorted(heights):
            for ancestor in (b"", bytes(32), p["anchor"] or bytes([1]) * 32):
                check(chain, height, rng.randbytes(32), ancestor)
        for digest in p["exceptions"]:
            for height in sorted(heights):
                check(chain, height, digest)
            for i in range(32):
                changed = bytearray(digest)
                changed[i] ^= 1
                check(chain, 170060, bytes(changed))
    for height, digest in repeats.items():
        for chain in networks:
            for h in (height - 1, height, height + 1):
                check(chain, h, digest)
            for i in range(32):
                changed = bytearray(digest)
                changed[i] ^= 1
                check(chain, height, bytes(changed))
    for chain in (0, 1, 5, 6):
        for height in (-1, 0, 2147483647, 2147483648):
            for size in (0, 31, 32, 33):
                check(chain, height, bytes(size))
    for size in (1, 31, 33):
        check(1, 227932, bytes(32), bytes(size))
    return count


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--coax", default="coax/bin/coax")
    parser.add_argument("--mode", choices=("debug", "release", "all"), default="all")
    parser.add_argument("--examples-only", action="store_true", help="reuse an already passing native result")
    args = parser.parse_args()
    compiler = Path(args.coax).resolve()
    networks, repeats, flags, bip30 = reference()
    with tempfile.TemporaryDirectory(prefix="coax-scriptflags-") as directory:
        directory = Path(directory)
        modes = (False, True) if args.mode == "all" else (args.mode == "release",)
        for release in modes:
            mode = ["--release"] if release else []
            cfile, shared = directory / "flags.c", directory / ("flags-release.dylib" if release else "flags-debug.dylib")
            if not args.examples_only:
                cfile.write_text(run([compiler, "c", ROOT / "coax/tests/bitcoin/scriptflags-native.coax", "--library", *mode]))
                run(["cc", "-std=c11", "-O2", "-shared", "-fPIC", cfile, "-o", shared, "-lm"])
                count = native(C.CDLL(str(shared)), networks, repeats, flags, bip30)
                print(f"{'release' if release else 'debug'}: {count} pairs of native decisions match pinned Core rules", flush=True)
            for name, status in (("script-flags", 0), ("script-flags-refused", 0)):
                got = subprocess.run([str(compiler), "run", f"bitcoin/examples/{name}.coax", *mode],
                                     cwd=ROOT, text=True, capture_output=True, timeout=300)
                assert got.returncode == status, got.stderr
                assert got.stdout == (ROOT / f"bitcoin/fixtures/{name}.out").read_text(), got.stdout
            print(f"{'release' if release else 'debug'}: both transcripts and exit statuses match", flush=True)


if __name__ == "__main__":
    main()
