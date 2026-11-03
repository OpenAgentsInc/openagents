#!/usr/bin/env python3
# Firm reason: 3 (runs BIP-340's and BIP-341's own reference Python, pinned, that we test against)
"""BIP-341 in Python, from the BIPs' own reference code, for the Taproot
chapters (Bitcoin chapters 41-43). It shares no code with Coax.

- bip-0340/reference.py (pinned) gives the curve, tagged hashes and
  BIP-340 signing and verification.
- bip-0341.mediawiki (pinned) holds BIP-341's Python functions:
  taproot_tweak_pubkey, taproot_tweak_seckey, taproot_tree_helper,
  taproot_output_script, taproot_sign_key and taproot_sign_script. They are
  read out of the BIP's <source lang="python"> blocks and run as written.
- sipa/bech32's segwit_addr.py (pinned) gives Bech32m addresses.
- Bitcoin Core's test_framework/script.py (pinned) gives SigMsg with the
  annex and the script-path extension: TaprootSignatureMsg and the
  BIP341_sha_* functions, read out of the file and run as written.

Every file is loaded with its SHA-256 checked first, so a changed pin
refuses. Run this file to check the reference against BIP-341's wallet
test vectors."""
import hashlib
import json
from pathlib import Path
import re
import types

HERE = Path(__file__).resolve().parent
UPSTREAM = HERE.parent / "vectors/upstream"
PINS = {
    "bip340/reference.py": "4b1d4ad9e60820df4a6d239733d52b014bc03e4c47afd757a2fe4d3b586d892c",
    "bip341/bip-0341.mediawiki": "463690d4c409587759c0ff6e8b8fc3a2bf6cb2cdda0c06ff193d317e0ed898ec",
    "bip341/wallet-test-vectors.json": "403e19fb81dd1f31e745699216308f61fb403774b2aafa87b631b8f7c042d37f",
    "sipa-bech32/segwit_addr.py.txt": "2884ce04a36c8374c4249177cd90e91e545ac73b5c9260307bbeb74ae2e9de0f",
}


def read(name):
    raw = (UPSTREAM / name).read_bytes()
    assert hashlib.sha256(raw).hexdigest() == PINS[name], name
    return raw


def ser_script(script):
    """BIP-341's ser_script: the script with its CompactSize length."""
    n = len(script)
    if n < 0xfd:
        return bytes([n]) + script
    if n <= 0xffff:
        return b"\xfd" + n.to_bytes(2, "little") + script
    return b"\xfe" + n.to_bytes(4, "little") + script


def load():
    """BIP-340's reference module with BIP-341's functions added to it."""
    ref = types.ModuleType("bip340_reference")
    exec(compile(read("bip340/reference.py"), str(UPSTREAM / "bip340/reference.py"), "exec"), ref.__dict__)
    ref.SECP256K1_ORDER = ref.n
    ref.ser_script = ser_script
    text = read("bip341/bip-0341.mediawiki").decode()
    blocks = re.findall(r'<source lang="python">\n(.*?)</source>', text, re.DOTALL)
    assert len(blocks) == 4, len(blocks)
    for block in blocks:
        exec(compile(block, "bip-0341.mediawiki", "exec"), ref.__dict__)
    for name in ("taproot_tweak_pubkey", "taproot_tweak_seckey", "taproot_tree_helper",
                 "taproot_output_script", "taproot_sign_key", "taproot_sign_script"):
        assert callable(getattr(ref, name)), name
    return ref


R = load()
ADDR = types.ModuleType("segwit_addr")
exec(compile(read("sipa-bech32/segwit_addr.py.txt"), "segwit_addr.py", "exec"), ADDR.__dict__)
VECTORS = json.loads(read("bip341/wallet-test-vectors.json"))


def tree(node):
    """A wallet-vector scriptTree as BIP-341's code takes it."""
    if node is None:
        return None
    if isinstance(node, dict):
        return (node["leafVersion"], bytes.fromhex(node["script"]))
    return [tree(node[0]), tree(node[1])]


def address(spk):
    return ADDR.encode("bc", spk[0] - 0x50 if spk[0] else 0, spk[2:])


def verify_commitment(c, s, q):
    """BIP-341's script-path rules, as its text gives them: the control
    block's size, the tapleaf hash, the sorted path, the tweak, then x(Q)
    and the parity. Returns None when they hold, else the rule that fails."""
    if len(c) < 33 or len(c) > 33 + 32 * 128 or (len(c) - 33) % 32:
        return "size"
    P = R.lift_x(int.from_bytes(c[1:33], "big"))
    if P is None:
        return "mismatch"
    v = c[0] & 0xfe
    k = R.tagged_hash("TapLeaf", bytes([v]) + ser_script(s))
    for j in range((len(c) - 33) // 32):
        e = c[33 + 32 * j:65 + 32 * j]
        k = R.tagged_hash("TapBranch", k + e if k < e else e + k)
    t = int.from_bytes(R.tagged_hash("TapTweak", c[1:33] + k), "big")
    if t >= R.n:
        return "mismatch"
    Q = R.point_add(P, R.point_mul(R.G, t))
    if Q is None or R.x(Q) != int.from_bytes(q, "big") or (c[0] & 1) != R.y(Q) % 2:
        return "mismatch"
    return None


def check_vectors():
    """BIP-341's code gives every scriptPubKey vector's values."""
    count = 0
    for v in VECTORS["scriptPubKey"]:
        g, im, ex = v["given"], v["intermediary"], v["expected"]
        p = bytes.fromhex(g["internalPubkey"])
        t = tree(g["scriptTree"])
        root = b"" if t is None else R.taproot_tree_helper(t)[1]
        assert root.hex() == (im["merkleRoot"] or "")
        assert R.tagged_hash("TapTweak", p + root).hex() == im["tweak"]
        assert R.taproot_tweak_pubkey(p, root)[1].hex() == im["tweakedPubkey"]
        spk = R.taproot_output_script(p, t)
        assert spk.hex() == ex["scriptPubKey"] and address(spk) == ex["bip350Address"]
        if t is not None:
            info, _ = R.taproot_tree_helper(t)
            leaves = []
            def flat(node):
                if isinstance(node, dict):
                    leaves.append(node)
                else:
                    for c in node:
                        flat(c)
            flat(g["scriptTree"])
            for k, leaf in enumerate(leaves):
                num = [n for n, (lv, _) in enumerate(info) if lv == (leaf["leafVersion"], bytes.fromhex(leaf["script"]))][0]
                script, control = R.taproot_sign_script(p, t, num, [])[-2:]
                assert control.hex() == ex["scriptPathControlBlocks"][leaf["id"]]
                assert verify_commitment(control, script, spk[2:]) is None
        count += 1
    for i in VECTORS["keyPathSpending"][0]["inputSpending"]:
        g, im = i["given"], i["intermediary"]
        key = bytes.fromhex(g["internalPrivkey"])
        root = bytes.fromhex(g["merkleRoot"]) if g["merkleRoot"] else b""
        assert R.pubkey_gen(key).hex() == im["internalPubkey"]
        assert R.taproot_tweak_seckey(key, root).hex() == im["tweakedPrivkey"]
        R.sighash = lambda hash_type, h=bytes.fromhex(im["sigHash"]): h
        assert R.taproot_sign_key(None if not root else _Root(root), key, g["hashType"], bytes(32))[0].hex() == i["expected"]["witness"][0]
        count += 1
    return count


class _Root:
    """Stands for a script tree whose Merkle root is given: taproot_sign_key
    only needs the root, through taproot_tree_helper."""
    def __init__(self, root):
        self.root = root


_helper = R.taproot_tree_helper


def _tree_helper(script_tree):
    if isinstance(script_tree, _Root):
        return ([], script_tree.root)
    return _helper(script_tree)


R.taproot_tree_helper = _tree_helper


# ---- Bitcoin Core's SigMsg (Bitcoin chapter 43)
CORE_SCRIPT = "bitcoin-core/script.py"
PINS[CORE_SCRIPT] = "82e250fadd393b972ae9d5d508f458fc872ea0dd947ca162693a006c3e488bbf"
CORE_NAMES = ("BIP341_sha_prevouts", "BIP341_sha_amounts", "BIP341_sha_scriptpubkeys",
              "BIP341_sha_sequences", "BIP341_sha_outputs", "TaprootSignatureMsg")


def load_core():
    """Bitcoin Core's TaprootSignatureMsg and the BIP341_sha_* functions it
    calls, read out of test_framework/script.py with the constants they use
    and run as written. script.py's own imports (the rest of Core's test
    framework) are not loaded; the four names it needs from them are given
    here: sha256, ser_string, TaggedHash and assert_equal."""
    import ast
    text = read(CORE_SCRIPT).decode()
    tree = ast.parse(text)
    keep = []
    for node in tree.body:
        if isinstance(node, ast.FunctionDef) and node.name in CORE_NAMES:
            keep.append(node)
        elif isinstance(node, ast.Assign) and len(node.targets) == 1 and isinstance(node.targets[0], ast.Name) \
                and (node.targets[0].id.startswith("SIGHASH_") or node.targets[0].id == "LEAF_VERSION_TAPSCRIPT"):
            keep.append(node)
    names = {n.name for n in keep if isinstance(n, ast.FunctionDef)}
    assert names == set(CORE_NAMES), names
    ns = {"sha256": lambda b: hashlib.sha256(b).digest(), "ser_string": ser_script,
          "TaggedHash": R.tagged_hash, "assert_equal": _assert_equal}
    exec(compile(ast.Module(body=keep, type_ignores=[]), "script.py", "exec"), ns)
    return types.SimpleNamespace(**ns)


def _assert_equal(a, b):
    assert a == b, (a, b)


class _Out:
    def __init__(self, raw):
        self.raw = raw

    def serialize(self):
        return self.raw


class _In:
    def __init__(self, prevout, sequence):
        self.prevout = _Out(prevout)
        self.nSequence = sequence


class _Utxo:
    def __init__(self, value, spk):
        self.nValue = value
        self.scriptPubKey = spk


def compact(b, at):
    n = b[at]
    if n < 0xfd:
        return n, at + 1
    size = {0xfd: 2, 0xfe: 4, 0xff: 8}[n]
    return int.from_bytes(b[at + 1:at + 1 + size], "little"), at + 1 + size


def parse_tx(raw):
    """An unsigned transaction (no witness) as the objects Core's
    TaprootSignatureMsg reads: version, nLockTime, vin and vout."""
    tx = types.SimpleNamespace(version=int.from_bytes(raw[0:4], "little"), vin=[], vout=[])
    n, at = compact(raw, 4)
    for _ in range(n):
        prevout = raw[at:at + 36]
        size, at = compact(raw, at + 36)
        at += size
        tx.vin.append(_In(prevout, int.from_bytes(raw[at:at + 4], "little")))
        at += 4
    n, at = compact(raw, at)
    for _ in range(n):
        start = at
        size, at = compact(raw, at + 8)
        at += size
        tx.vout.append(_Out(raw[start:at]))
    tx.nLockTime = int.from_bytes(raw[at:at + 4], "little")
    assert at + 4 == len(raw)
    return tx


CORE = load_core()
SPEND = VECTORS["keyPathSpending"][0]
TX = parse_tx(bytes.fromhex(SPEND["given"]["rawUnsignedTx"]))
UTXOS = [_Utxo(u["amountSats"], bytes.fromhex(u["scriptPubKey"])) for u in SPEND["given"]["utxosSpent"]]


def sigmsg(index, hash_type, annex=None, leaf_script=None, codesep=0xffffffff):
    """Core's SigMsg for an input of BIP-341's transaction; a leaf script
    makes it a script-path message (leaf version 0xc0)."""
    if leaf_script is None:
        return CORE.TaprootSignatureMsg(TX, UTXOS, hash_type, index, annex=annex)
    return CORE.TaprootSignatureMsg(TX, UTXOS, hash_type, index, scriptpath=True, leaf_script=leaf_script,
                                    codeseparator_pos=codesep, annex=annex)


def check_core():
    """Core's SigMsg gives BIP-341's seven published key-path messages and
    signature hashes."""
    for i in SPEND["inputSpending"]:
        g, im = i["given"], i["intermediary"]
        m = sigmsg(g["txinIndex"], g["hashType"])
        assert m.hex() == im["sigMsg"], g["txinIndex"]
        assert R.tagged_hash("TapSighash", m).hex() == im["sigHash"]
    return len(SPEND["inputSpending"])


if __name__ == "__main__":
    print(f"BIP-341's reference code gives {check_vectors()} wallet test vectors' values")
    print(f"Bitcoin Core's TaprootSignatureMsg gives {check_core()} published key-path signature messages")
