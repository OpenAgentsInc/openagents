#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
"""Independent reference for chapter 104's signatures: the treasury's
2-of-3 spend of KY_PSBT signed by alice and keyco.

Python computes, sharing no code with Coax: each agent's account key at
m/48h/1h/0h/2h (keys_reference.py's pure-integer BIP-32), the child keys at
receive 0 and 1, the P2WSH witness scripts, and the BIP-143 SIGHASH_ALL
digests (written here from BIP-143's text). The pinned libsecp256k1 v0.8.0
oracle (tools/coax-nostr/fuzz) makes the RFC 6979, low-S ECDSA signatures.
Python then assembles each signer's PSBT (BIP-174 PSBT_IN_PARTIAL_SIG) and
the final transaction, and checks the transaction as a verifier would:
each input's witness script hashes to its P2WSH program, the signatures
are in the witness script's key order, and libsecp256k1 verifies each one
over this file's own sighash. Test keys only (seeds are public strings).

  python3 coax/tests/nostr/keys_sign_reference.py      # writes keys_sign_fixtures.coax
"""
import ctypes as C, hashlib, sys
from pathlib import Path
HERE = Path(__file__).resolve().parent
R = HERE.parents[2]
sys.path.insert(0, str(HERE)); sys.path.insert(0, str(R / "tools/coax-nostr/fuzz"))
import keys_reference as K
from oracle import NativeOracle, DEFAULT_LIBRARY

def dsha(b): return K.sha(K.sha(b))
def ci(n): return K.ci(n)
H = 2**31
PATH = [48 + H, 1 + H, 0 + H, 2 + H]
NAMES = ("alice", "keyco", "enclave")

def account_priv(name):
    k, c = K.master(K.sha(("chapter 104 " + name).encode()))
    for idx in PATH: k, c = K.ckd_priv(k, c, idx)
    return k, c

o = NativeOracle(DEFAULT_LIBRARY); L = o.lib; ctx = o.context
L.secp256k1_ecdsa_sign.argtypes = [C.c_void_p, C.c_void_p, C.c_char_p, C.c_char_p, C.c_void_p, C.c_void_p]
L.secp256k1_ecdsa_signature_serialize_der.argtypes = [C.c_void_p, C.c_void_p, C.c_void_p, C.c_void_p]
L.secp256k1_ecdsa_signature_parse_der.argtypes = [C.c_void_p, C.c_void_p, C.c_char_p, C.c_size_t]
L.secp256k1_ecdsa_verify.argtypes = [C.c_void_p, C.c_char_p, C.c_char_p, C.c_void_p]
L.secp256k1_ec_pubkey_parse.argtypes = [C.c_void_p, C.c_void_p, C.c_char_p, C.c_size_t]
def ecdsa(key, h):
    sig = C.create_string_buffer(64); assert L.secp256k1_ecdsa_sign(ctx, sig, h, key, None, None)
    der = C.create_string_buffer(72); n = C.c_size_t(72)
    assert L.secp256k1_ecdsa_signature_serialize_der(ctx, der, C.byref(n), sig)
    return der.raw[:n.value]
def verify(pub, h, der):
    sig = C.create_string_buffer(64); pk = C.create_string_buffer(64)
    return (L.secp256k1_ecdsa_signature_parse_der(ctx, sig, der, len(der)) and
            L.secp256k1_ec_pubkey_parse(ctx, pk, pub, 33) and L.secp256k1_ecdsa_verify(ctx, sig, h, pk))

# The PSBT keys_reference.py wrote, and its unsigned transaction.
psbt = K.make_psbt()
assert psbt[:5] == b"psbt\xff" and psbt[5:8] == b"\x01\x00\xa6"
tx = psbt[8:8 + 0xa6]
version, body, locktime = tx[:4], tx[4:-4], tx[-4:]
ins = [(tx[5 + 41 * j:5 + 41 * j + 36], tx[5 + 41 * j + 37:5 + 41 * j + 41]) for j in range(2)]
outs_raw = tx[5 + 82 + 1:-4]
amounts = (60000, 40000)

accounts = [account_priv(n) for n in NAMES]
def child(acct, br, i):
    k, c = K.ckd_priv(acct[0], acct[1], br); k, c = K.ckd_priv(k, c, i); return k
witness_scripts, sighashes = [], []
for j, idx in enumerate((0, 1)):                     # inputs at receive 0 and receive 1
    pubs = sorted(K.ser(K.mul(child(a, 0, idx))) for a in accounts)
    ws = bytes([0x52]) + b"".join(b"\x21" + p for p in pubs) + bytes([0x53, 0xae])
    assert b"\0\x20" + K.sha(ws) == K.SPK[(0, idx)]
    witness_scripts.append(ws)
    # BIP-143: version, hashPrevouts, hashSequence, outpoint, scriptCode,
    # amount, nSequence, hashOutputs, locktime, hash type.
    pre = (version + dsha(b"".join(p for p, _ in ins)) + dsha(b"".join(s for _, s in ins)) + ins[j][0] +
           ci(len(ws)) + ws + amounts[j].to_bytes(8, "little") + ins[j][1] + dsha(outs_raw) + locktime +
           (1).to_bytes(4, "little"))
    sighashes.append(dsha(pre))

partial = [[(K.ser(K.mul(child(a, 0, idx))), ecdsa(child(a, 0, idx).to_bytes(32, "big"), sighashes[j]) + b"\x01")
            for j, idx in enumerate((0, 1))] for a in accounts]
# Each signer's PSBT: its partial signature appended to each input map.
def signed_psbt(sigs):
    out = psbt[:8 + 0xa6] + b"\0"
    for j, v in enumerate(amounts):
        s = K.SPK[(0, j)]
        out += K.kv(b"\x01", v.to_bytes(8, "little") + ci(len(s)) + s) + K.kv(b"\x02" + sigs[j][0], sigs[j][1]) + b"\0"
    return out + b"\0\0"
assert signed_psbt([(b"", b"")] * 2) != psbt
# The final transaction from alice's and keyco's signatures.
wit = b""
for j in range(2):
    ws = witness_scripts[j]; keys = [ws[2 + 34 * k:2 + 34 * k + 33] for k in range(3)]
    have = {partial[a][j][0]: partial[a][j][1] for a in (0, 1)}
    sigs = [have[k] for k in keys if k in have]
    assert len(sigs) == 2
    wit += ci(4) + b"\0" + b"".join(ci(len(x)) + x for x in sigs) + ci(len(ws)) + ws
final = version + b"\0\1" + body + wit + locktime
# Check it as a verifier: program, key order, signatures over our sighash.
for j in range(2):
    ws = witness_scripts[j]; assert K.sha(ws) == K.SPK[(0, j)][2:]
    keys = [ws[2 + 34 * k:2 + 34 * k + 33] for k in range(3)]
    for a in (0, 1):
        pub, sig = partial[a][j]
        assert sig[-1] == 1 and verify(pub, sighashes[j], sig[:-1]) and pub in keys
    assert not verify(partial[0][j][0], sighashes[1 - j], partial[0][j][1][:-1])
txid = dsha(tx)[::-1].hex()

lines = ["// Chapter 104: the treasury's 2-of-3 spend of KY_PSBT, written by",
         "// coax/tests/nostr/keys_sign_reference.py (Python BIP-32 and BIP-143, libsecp256k1",
         "// v0.8.0 signatures). Test keys only: the seeds are SHA-256(\"chapter 104 <name>\").",
         "// Loaded by nipkeys_sign.coax."]
put = lambda n, v: lines.append("%s :: %s;" % (n, v))
strs = lambda xs: "[%d] string.[%s]" % (len(xs), ",".join('"%s"' % x for x in xs))
put("KY_ACCOUNT_KEY", strs([a[0].to_bytes(32, "big").hex() for a in accounts]))
put("KY_ACCOUNT_CHAIN", strs([a[1].hex() for a in accounts]))
def wscript(br, i):
    pubs = sorted(K.ser(K.mul(child(a, br, i))) for a in accounts)
    return bytes([0x52]) + b"".join(b"\x21" + p for p in pubs) + bytes([0x53, 0xae])
table = [wscript(br, i) for br in (0, 1) for i in (0, 1)]
assert table[:2] == witness_scripts and all(b"\0\x20" + K.sha(table[2 * br + i]) == K.SPK[(br, i)] for br in (0, 1) for i in (0, 1))
put("KY_WITNESS", strs([w.hex() for w in table]))   # receive 0, 1, change 0, 1
put("KY_SIGHASH", strs([h.hex() for h in sighashes]))
put("KY_CHILD_PUB", strs([partial[a][j][0].hex() for a in range(3) for j in range(2)]))
put("KY_PARTIAL", strs([partial[a][j][1].hex() for a in range(3) for j in range(2)]))
put("KY_PSBT_SIGNED", strs([signed_psbt(partial[a]).hex() for a in range(3)]))
put("KY_FINAL_TX", '"%s"' % final.hex())
put("KY_TXID", '"%s"' % txid)
(HERE / "keys_sign_fixtures.coax").write_text("\n".join(lines) + "\n")
print("final", final.hex()); print("txid", txid)
