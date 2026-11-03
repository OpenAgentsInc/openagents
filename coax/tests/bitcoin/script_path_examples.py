#!/usr/bin/env python3
# Firm reason: 3 (recomputes chapter 45's example output with BIP-340's and BIP-341's own reference code and a transliteration of Bitcoin Core's interpreter, outside Coax)
"""Checks the example of Bitcoin chapter 45, script-path spends, against
references that share no code with Coax: BIP-341's own Python builds each
control block and witness stack (taproot_sign_script), BIP-340's
reference signs, Bitcoin Core's TaprootSignatureMsg gives each signature
message, and script_reference.py, a transliteration of Core's
interpreter, verifies every spend.

  python3 coax/tests/bitcoin/script_path_examples.py bitcoin/fixtures/script-path-spend.out

It writes the whole output the example should print and compares it with
the fixture line by line."""
import hashlib
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import script_reference as S  # noqa: E402
import taproot_reference as T  # noqa: E402
import tx_reference as X  # noqa: E402

R = T.R
UNSIGNED = bytes.fromhex("0200000001" + "ab" * 32 + "0000000000ffffffff01905f010000000000160014" + "11" * 20 + "00000000")
AMOUNT = 100000
FLAGS = S.parse_flags("P2SH,WITNESS,TAPROOT")
OP_CHECKSIG, OP_CHECKSIGADD, OP_NUMEQUAL, OP_SHA256, OP_EQUALVERIFY = 0xac, 0xba, 0x9c, 0xa8, 0x88


def verify(items, program):
    """VerifyScript for the unsigned transaction's input, spending OP_1
    <program> with witness items."""
    tx = X.unserialize(UNSIGNED, False)
    tx["ins"][0]["witness"] = list(items)
    tx = X.unserialize(X.serialize(tx, True), True)    # as the signed transaction reads
    spk = b"\x51\x20" + program
    return S.ERRORS[S.verify_script(b"", spk, tx["ins"][0]["witness"], FLAGS, S.Checker(tx, 0, AMOUNT))]


def witness_bytes(items):
    return X.compact(len(items)) + b"".join(X.compact(len(i)) + i for i in items)


def leaves_of(node):
    if isinstance(node, dict):
        return [node]
    return [x for c in node for x in leaves_of(c)]


def numbered(info, version, script):
    return [n for n, (lv, _) in enumerate(info) if lv == (version, script)][0]


def sighash(program, hashtype, script, annex):
    tx = X.unserialize(UNSIGNED, False)
    checker = S.Checker(tx, 0, AMOUNT)
    checker.spk = b"\x51\x20" + program
    msg = checker.sigmsg(hashtype, S.TAPSCRIPT, dict(annex=annex, script=script, codesep=0xffffffff))
    return R.tagged_hash("TapSighash", msg)


def sign(secret, program, hashtype, script, annex=None):
    sig = R.schnorr_sign(sighash(program, hashtype, script, annex), secret, bytes(32))
    return sig + (bytes([hashtype]) if hashtype else b"")


def lines():
    out = ["BIP-341's script trees, spent by each leaf"]
    parity = path = count = paths = 0
    for i, v in enumerate(T.VECTORS["scriptPubKey"]):
        g, ex = v["given"], v["expected"]
        if g["scriptTree"] is None:
            continue
        p = bytes.fromhex(g["internalPubkey"])
        tree = T.tree(g["scriptTree"])
        info, _ = R.taproot_tree_helper(tree)
        program = bytes.fromhex(ex["scriptPubKey"])[2:]
        for k, leaf in enumerate(leaves_of(g["scriptTree"])):
            version, script = leaf["leafVersion"], bytes.fromhex(leaf["script"])
            keyed = len(script) == 34 and script[0] == 32 and script[33] == OP_CHECKSIG
            items = R.taproot_sign_script(p, tree, numbered(info, version, script), [b""] if keyed else [])
            control = items[-1]
            out.append("  vector %d, leaf %d (0x%02x, %d bytes): control block %s, %s: %s" % (
                i, k, version, len(script), "as BIP-341's" if control.hex() == ex["scriptPathControlBlocks"][leaf["id"]] else "differs",
                "an empty signature" if keyed else "no stack", verify(items, program)))
            count += 1
            flipped = bytes([control[0] ^ 1]) + control[1:]
            if verify(items[:-1] + [flipped], program) == "WITNESS_PROGRAM_MISMATCH":
                parity += 1
            if len(control) > 33:
                paths += 1
                changed = control[:40] + bytes([control[40] ^ 1]) + control[41:]
                if verify(items[:-1] + [changed], program) == "WITNESS_PROGRAM_MISMATCH":
                    path += 1
    out.append("  the parity bit flipped: %d of %d WITNESS_PROGRAM_MISMATCH" % (parity, count))
    out.append("  a path hash changed: %d of %d WITNESS_PROGRAM_MISMATCH" % (path, paths))

    secrets = [bytes([0x44]) * 32, bytes([0x11]) * 32, bytes([0x22]) * 32, bytes([0x33]) * 32]
    keys = [R.pubkey_gen(d) for d in secrets]
    preimage = b"Bitcoin chapter 45"
    scripts = [bytes([32]) + keys[1] + bytes([OP_CHECKSIG]),
               bytes([32]) + keys[2] + bytes([OP_CHECKSIG, 32]) + keys[3] + bytes([OP_CHECKSIGADD, 0x52, OP_NUMEQUAL]),
               bytes([OP_SHA256, 32]) + hashlib.sha256(preimage).digest() + bytes([OP_EQUALVERIFY, 32]) + keys[2] + bytes([OP_CHECKSIG])]
    tree = [(0xc0, scripts[0]), [(0xc0, scripts[1]), (0xc0, scripts[2])]]
    info, root = R.taproot_tree_helper(tree)
    y, q = R.taproot_tweak_pubkey(keys[0], root)
    nums = [numbered(info, 0xc0, s) for s in scripts]
    spend = lambda k, stack: R.taproot_sign_script(keys[0], tree, nums[k], stack)
    out.append("a tree of our own")
    out.append("  internal key P: " + keys[0].hex())
    labels = ["<key 1> CHECKSIG", "<key 2> CHECKSIG <key 3> CHECKSIGADD 2 NUMEQUAL", "SHA256 <hash> EQUALVERIFY <key 2> CHECKSIG"]
    for k, depth in enumerate([1, 2, 2]):
        out.append("  leaf %d, depth %d, %d bytes: %s" % (k, depth, len(scripts[k]), labels[k]))
    out.append("  root: " + root.hex())
    out.append("  output key Q: " + q.hex())
    out.append("  Q's y is " + ("odd" if y else "even"))

    out.append("spending it")
    items = spend(0, [sign(secrets[1], q, 0, scripts[0])])
    out.append("  leaf 0, %d bytes of witness: %s" % (len(witness_bytes(items)), verify(items, q)))
    out.append("    " + witness_bytes(items).hex())
    s2, s3 = sign(secrets[2], q, 0, scripts[1]), sign(secrets[3], q, 0, scripts[1])
    items = spend(1, [s3, s2])
    out.append("  leaf 1, %d bytes of witness: %s" % (len(witness_bytes(items)), verify(items, q)))
    out.append("  leaf 1, key 2's signature alone: " + verify(spend(1, [b"", s2]), q))
    items = spend(2, [sign(secrets[2], q, 0, scripts[2]), preimage])
    out.append("  leaf 2, %d bytes of witness: %s" % (len(witness_bytes(items)), verify(items, q)))
    out.append("  leaf 2, the preimage cut short: " + verify(spend(2, [sign(secrets[2], q, 0, scripts[2]), preimage[:17]]), q))
    sig = sign(secrets[1], q, 0x83, scripts[0])
    out.append("  leaf 0, signed SINGLE|ANYONECANPAY, %d bytes: %s" % (len(sig), verify(spend(0, [sig]), q)))
    annex = bytes([0x50, 0x04, 0x05])
    sig = sign(secrets[1], q, 0, scripts[0], annex)
    out.append("  leaf 0, with an annex it signs: " + verify(spend(0, [sig]) + [annex], q))
    out.append("  the same signature, the annex left off: " + verify(spend(0, [sig]), q))

    out.append("what refuses")
    out.append("  leaf 0, key 1 signed leaf 1's hash: " + verify(spend(0, [sign(secrets[1], q, 0, scripts[1])]), q))
    sig = sign(secrets[1], q, 0, scripts[0])
    other = spend(1, [])[-1]
    out.append("  leaf 0 with leaf 1's control block: " + verify([sig, scripts[0], other], q))
    control = spend(0, [])[-1]
    out.append("  leaf 0, the parity bit flipped: " + verify([sig, scripts[0], bytes([control[0] ^ 1]) + control[1:]], q))
    swapped = other[:33] + other[65:97] + other[33:65]
    out.append("  leaf 1, its two path hashes swapped: " + verify([s3, s2, scripts[1], swapped], q))
    out.append("  leaf 1, its last path hash dropped: " + verify([s3, s2, scripts[1], other[:65]], q))
    out.append("  the witness builder, a 64-byte control block: refused")
    out.append("  the witness builder, an annex starting 0x51: refused")

    tx = X.unserialize(UNSIGNED, False)
    checker = S.Checker(tx, 0, AMOUNT)
    checker.spk = b"\x51\x20" + q
    h = R.tagged_hash("TapSighash", checker.sigmsg(0, S.TAPROOT, dict(annex=None)))
    sig = R.schnorr_sign(h, R.taproot_tweak_seckey(secrets[0], root), bytes(32))
    out.append("the key path, signed by P tweaked with the root: " + verify([sig], q))
    return out


def main():
    want = lines()
    got = Path(sys.argv[1]).read_text().splitlines()
    for n, (a, b) in enumerate(zip(want, got)):
        if a != b:
            sys.exit("line %d differs:\n  reference: %s\n  fixture:   %s" % (n + 1, a, b))
    if len(want) != len(got):
        sys.exit("the reference gives %d lines, the fixture %d" % (len(want), len(got)))
    print("all %d lines agree" % len(want))


if __name__ == "__main__":
    main()
