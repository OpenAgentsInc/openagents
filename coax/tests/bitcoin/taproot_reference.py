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

Both files are loaded with their SHA-256 checked first, so a changed pin
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
                control = R.taproot_sign_script(p, t, num, [])[-1]
                assert control.hex() == ex["scriptPathControlBlocks"][leaf["id"]]
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


if __name__ == "__main__":
    print(f"BIP-341's reference code gives {check_vectors()} wallet test vectors' values")
