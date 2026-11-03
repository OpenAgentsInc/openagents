#!/usr/bin/env python3
# Firm reason: 3 (recomputes the Taproot examples' output with BIP-340's and BIP-341's own reference code, outside Coax)
"""Checks the Taproot examples of Bitcoin chapters 41-43 against
taproot_reference.py, which runs BIP-340's reference.py and the Python in
BIP-341 itself, and shares no code with Coax.

  python3 coax/tests/bitcoin/taproot_examples.py key-path bitcoin/fixtures/taproot-key-path.out
  python3 coax/tests/bitcoin/taproot_examples.py script-trees bitcoin/fixtures/taproot-script-trees.out

Each mode writes the whole output the example should print, from the
reference, and compares it with the fixture line by line."""
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import taproot_reference as T  # noqa: E402

R = T.R
N = R.n
KEY_PATH = T.VECTORS["keyPathSpending"][0]


def outcome(code):
    return {0: "y even", 1: "y odd", "tweak": "bad tweak", "key": "bad key", "root": "bad root"}[code]


def output_key(p, root):
    """(parity, Q's x) by BIP-341's taproot_tweak_pubkey, or a refusal name."""
    if len(root) not in (0, 32):
        return "root", b""
    if R.lift_x(int.from_bytes(p, "big")) is None:
        return "key", b""
    try:
        return R.taproot_tweak_pubkey(p, root)
    except ValueError:
        return "tweak", b""


def add_tweak(p, t):
    """Q = lift_x(p) + tG for a t given directly, with BIP-341's refusals."""
    P = R.lift_x(int.from_bytes(p, "big"))
    if P is None:
        return "key", b""
    if t >= N:
        return "tweak", b""
    Q = R.point_add(P, R.point_mul(R.G, t))
    if Q is None:
        return "tweak", b""
    return 0 if R.has_even_y(Q) else 1, R.bytes_from_int(R.x(Q))


def commit(lines, p, root):
    lines.append("  t = H_TapTweak(P || root): " + R.tagged_hash("TapTweak", p + root).hex())
    parity, q = output_key(p, root)
    lines.append("  Q = P + tG, x: " + q.hex())
    lines.append("  Q's " + outcome(parity))


def sig_hashtype(sig):
    """BIP-341's key-path rule: 64 bytes is SIGHASH_DEFAULT, 65 bytes carry a
    nonzero hash type, anything else fails. -1 and -2 are the Coax codes."""
    if len(sig) == 64:
        return 0
    if len(sig) != 65:
        return -1
    return -2 if sig[64] == 0 else sig[64]


def key_path():
    lines = ["no script tree: BIP-86 m/86'/0'/0'/0/0"]
    p = bytes.fromhex("cc8a4bc64d897bddc5fbc2f670f7a8ba0b386779106cf1223c6fc5d7cd6fc115")
    lines.append("  internal key P: " + p.hex())
    commit(lines, p, b"")
    spk = R.taproot_output_script(p, None)
    lines.append("  scriptPubKey: " + spk.hex())
    lines.append("  address: " + T.address(spk))

    lines.append("BIP-341 scriptPubKey vectors")
    for i, v in enumerate(T.VECTORS["scriptPubKey"]):
        g, ex = v["given"], v["expected"]
        p = bytes.fromhex(g["internalPubkey"])
        t = T.tree(g["scriptTree"])
        spk = R.taproot_output_script(p, t)
        root = b"" if t is None else R.taproot_tree_helper(t)[1]
        parity, _ = output_key(p, root)
        count = 0 if t is None else len(R.taproot_tree_helper(t)[0])
        leaves = "no script tree" if count == 0 else "1 leaf" if count == 1 else f"{count} leaves"
        lines.append(f"  vector {i}: {leaves}, Q's {outcome(parity)}, scriptPubKey "
                     f"{'matches' if spk.hex() == ex['scriptPubKey'] else 'differs'}, address "
                     f"{'matches' if T.address(spk) == ex['bip350Address'] else 'differs'}")

    lines.append("the commitment: vector 3 with its root, then one bit of the root changed")
    v = T.VECTORS["scriptPubKey"][3]
    p = bytes.fromhex(v["given"]["internalPubkey"])
    root = bytearray(bytes.fromhex(v["intermediary"]["merkleRoot"]))
    commit(lines, p, bytes(root))
    root[31] ^= 1
    commit(lines, p, bytes(root))

    lines.append("the tweaked secret key")
    for i in KEY_PATH["inputSpending"]:
        g, im = i["given"], i["intermediary"]
        d = bytes.fromhex(g["internalPrivkey"])
        root = bytes.fromhex(g["merkleRoot"]) if g["merkleRoot"] else b""
        odd = not R.has_even_y(R.point_mul(R.G, int.from_bytes(d, "big")))
        same = R.taproot_tweak_seckey(d, root).hex() == im["tweakedPrivkey"]
        lines.append(f"  input {g['txinIndex']}: dG has {'an odd y' if odd else 'an even y'}, so "
                     f"{'negate' if odd else 'keep'} d; d' = d + t {'matches BIP-341' if same else 'differs'}")

    lines.append("key-path spends of BIP-341's transaction")
    spent = KEY_PATH["given"]["utxosSpent"]
    for i in KEY_PATH["inputSpending"]:
        g, im = i["given"], i["intermediary"]
        index, ht = g["txinIndex"], g["hashType"]
        d = bytes.fromhex(g["internalPrivkey"])
        root = bytes.fromhex(g["merkleRoot"]) if g["merkleRoot"] else b""
        h = bytes.fromhex(im["sigHash"])
        R.sighash = lambda hash_type, h=h: h
        sig = R.taproot_sign_key(None if not root else T._Root(root), d, ht, bytes(32))[0]
        q = bytes.fromhex(spent[index]["scriptPubKey"])[2:]
        lines.append(f"  input {index} (hash type 0x{ht:02x}), {len(sig)} bytes: witness "
                     f"{'matches' if sig.hex() == i['expected']['witness'][0] else 'differs'}, verifies with Q "
                     f"{str(R.schnorr_verify(h, q, sig[:64])).lower()}, with P "
                     f"{str(R.schnorr_verify(h, bytes.fromhex(im['internalPubkey']), sig[:64])).lower()}")

    lines.append("refusals")
    g = R.bytes_from_int(R.G[0])
    lines.append("  t = n: " + outcome(add_tweak(g, N)[0]))
    lines.append("  P = G, t = n - 1, so Q is infinity: " + outcome(add_tweak(g, N - 1)[0]))
    lines.append("  a 31-byte root: " + outcome(output_key(g, bytes(31))[0]))
    lines.append("  x = 5, not on the curve: " + outcome(output_key((5).to_bytes(32, "big"), b"")[0]))
    # taproot_tweak_seckey's check is t >= SECP256K1_ORDER; a TapTweak hash
    # never reaches it, so the example gives t directly.
    lines.append("  the secret tweak with t = 2^256 - 1: " + ("refused" if (1 << 256) - 1 >= R.SECP256K1_ORDER else "accepted"))
    lines.append(f"  a 65-byte signature ending in 0x00: hash type {sig_hashtype(bytes(65))}")
    lines.append(f"  a 63-byte signature: hash type {sig_hashtype(bytes(63))}")
    return lines


def name(rule):
    return {None: "ok", "size": "bad control block size", "mismatch": "mismatch"}[rule]


def is_tree(depths):
    """Whether leaf depths, left to right, make a binary tree: read the
    list recursively, a subtree at depth d being one leaf at d or two
    subtrees at d + 1. (Coax uses a stack; this is a separate method.)"""
    def take(i, d):
        if i >= len(depths) or depths[i] < d:
            return None
        if depths[i] == d:
            return i + 1
        j = take(i, d + 1)
        return None if j is None else take(j, d + 1)
    return take(0, 0) == len(depths)


def leaves_of(node, depth=0):
    if isinstance(node, dict):
        return [(depth, node)]
    return leaves_of(node[0], depth + 1) + leaves_of(node[1], depth + 1)


def matches(ok):
    return "matches" if ok else "differs"


def script_trees():
    lines = ["BIP-341's script trees"]
    for i, v in enumerate(T.VECTORS["scriptPubKey"]):
        g, im, ex = v["given"], v["intermediary"], v["expected"]
        if g["scriptTree"] is None:
            continue
        p = bytes.fromhex(g["internalPubkey"])
        t = T.tree(g["scriptTree"])
        _, root = R.taproot_tree_helper(t)
        spk = R.taproot_output_script(p, t)
        lines.append(f"  vector {i}: root {matches(root.hex() == im['merkleRoot'])}")
        for k, (depth, leaf) in enumerate(leaves_of(g["scriptTree"])):
            lv, script = leaf["leafVersion"], bytes.fromhex(leaf["script"])
            h = R.tagged_hash("TapLeaf", bytes([lv]) + T.ser_script(script))
            control = R.taproot_sign_script(p, t, k, [])[-1]
            lines.append(f"    leaf {k} (version 0x{lv:02x}, depth {depth}): hash "
                         f"{matches(h.hex() == im['leafHashes'][leaf['id']])}, control block of "
                         f"{len(control)} bytes {matches(control.hex() == ex['scriptPathControlBlocks'][leaf['id']])}"
                         f", commits: {name(T.verify_commitment(control, script, spk[2:]))}")

    lines.append("vector 5, step by step")
    g = T.VECTORS["scriptPubKey"][5]["given"]
    p = bytes.fromhex(g["internalPubkey"])
    t = T.tree(g["scriptTree"])
    info, root = R.taproot_tree_helper(t)
    hs = [R.tagged_hash("TapLeaf", bytes([lv]) + T.ser_script(sc)) for (lv, sc), _ in info]
    for k in range(3):
        lines.append(f"  leaf {k} TapLeaf: {hs[k].hex()}")
    lines.append(f"  leaves 1 and 2 join with leaf {1 if hs[1] < hs[2] else 2} first, as the smaller hash")
    b12 = R.tagged_hash("TapBranch", min(hs[1], hs[2]) + max(hs[1], hs[2]))
    lines.append(f"  TapBranch(1, 2): {b12.hex()}")
    lines.append(f"  the root joins leaf 0 and that branch with {'leaf 0 first' if hs[0] < b12 else 'the branch first'}")
    lines.append(f"  root: {root.hex()}")
    c = bytes(R.taproot_sign_script(p, t, 1, [])[-1])
    lines.append(f"  leaf 1's control block, {len(c)} bytes:")
    lines.append(f"    first byte 0x{c[0]:02x}: leaf version 0x{c[0] & 0xfe:02x}, Q's parity {c[0] & 1}")
    lines.append(f"    internal key P: {c[1:33].hex()}")
    lines.append(f"    path 0 (leaf 2): {c[33:65].hex()}")
    lines.append(f"    path 1 (leaf 0): {c[65:97].hex()}")

    lines.append("leaf 1 of vector 5, changed")
    q = R.taproot_output_script(p, t)[2:]
    script = info[1][0][1]

    def check(cc, ss=script):
        return name(T.verify_commitment(bytes(cc), bytes(ss), q))

    def flip(data, at, bit):
        d = bytearray(data)
        d[at] ^= bit
        return bytes(d)
    lines.append(f"  as built: {check(c)}")
    lines.append(f"  the parity bit flipped: {check(flip(c, 0, 1))}")
    lines.append(f"  leaf version 0xc2: {check(flip(c, 0, 2))}")
    lines.append(f"  one bit of a path hash: {check(flip(c, 40, 1))}")
    lines.append(f"  the path in the other order: {check(c[:33] + c[65:97] + c[33:65])}")
    lines.append(f"  the last path hash left out: {check(c[:65])}")
    lines.append(f"  one bit of the script: {check(c, flip(script, 2, 1))}")
    lines.append(f"  34 bytes: {check(c[:34])}")
    lines.append(f"  33 + 32 * 129 bytes: {check(bytes(33 + 32 * 129))}")

    lines.append("depths that make no tree")
    for d in ([1, 2, 2], [1, 2, 1], [1, 2], [0, 0]):
        lines.append(f"  {' '.join(map(str, d))}: {'ok' if is_tree(d) else 'bad tree'}")
    # BIP-341: the control block's low bit is the parity, so a leaf version is even.
    lines.append(f"  an odd leaf version, 0xc1: {'bad leaf version' if 0xc1 & 1 else 'ok'}")
    return lines


MODES = {"script-trees": script_trees, "key-path": key_path}

if __name__ == "__main__":
    mode, fixture = sys.argv[1], Path(sys.argv[2])
    want = MODES[mode]()
    got = fixture.read_text().splitlines()
    for k, (a, b) in enumerate(zip(want, got)):
        assert a == b, f"line {k + 1}:\n  reference: {a}\n  fixture:   {b}"
    assert len(want) == len(got), (len(want), len(got))
    print(f"{fixture}: all {len(got)} lines agree with BIP-341's reference code")
